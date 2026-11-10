//! Bounded file reads for the local checkout trust boundary.

use std::fs::File;
use std::io::{self, Read as _};
use std::path::Path;

use crate::budget::{AnalysisBudget, Resource, units};

/// Only an absent directory entry is optional. Dangling links, inaccessible
/// ancestors and other metadata failures must reach the caller as errors.
pub(crate) fn optional_metadata(path: &Path) -> io::Result<Option<std::fs::Metadata>> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => std::fs::metadata(path).map(Some),
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Match actual directory components rather than checkout-name substrings.
pub(crate) fn excluded_component(path: &Path) -> bool {
    path.components().any(|part| {
        ["node_modules", "dist", "build", ".next", ".git"]
            .iter()
            .any(|name| part.as_os_str() == *name)
    })
}

/// A configured exclude/ignore glob. Relative patterns match the path relative
/// to the walk root and ignore a leading `./`; absolute patterns match the full
/// path, as they did before walks became root-relative.
#[derive(Debug, Clone)]
pub(crate) struct PathPattern {
    pattern: glob::Pattern,
    absolute: bool,
}

/// Walked paths never start with `./` or end with `/`, so a pattern spelled
/// that way would silently match nothing; strip both (and `.\` / `\` on
/// Windows, where `\` is a separator). An empty relative pattern means the
/// walk root, `.`. Absolute patterns are put in [`absolute_form`].
pub(crate) fn pattern_spelling(text: &str) -> std::borrow::Cow<'_, str> {
    let absolute = Path::new(text).is_absolute();
    let spelling = spelling_with(text, absolute, cfg!(windows));
    if absolute && cfg!(windows) {
        std::borrow::Cow::Owned(windows_user_form(spelling))
    } else {
        std::borrow::Cow::Borrowed(spelling)
    }
}

fn spelling_with(text: &str, absolute: bool, windows: bool) -> &str {
    let separator = |c: char| c == '/' || (windows && c == '\\');
    let mut spelling = text;
    if !absolute {
        while let Some(rest) = spelling
            .strip_prefix('.')
            .filter(|rest| rest.starts_with(separator))
        {
            spelling = rest.trim_start_matches(separator);
        }
    }
    let trimmed = spelling.trim_end_matches(separator);
    if trimmed.is_empty() {
        if spelling.starts_with(separator) {
            spelling.get(..1).unwrap_or("/")
        } else {
            "."
        }
    } else {
        trimmed
    }
}

/// An absolute walked path in the form users spell absolute patterns: the
/// lexically normalized path, and on Windows without the verbatim prefix that
/// canonical roots carry (`\\?\C:\` is `C:\`, `\\?\UNC\s\h` is `\\s\h`),
/// with an uppercase drive letter.
pub(crate) fn absolute_form(path: &Path) -> std::path::PathBuf {
    let normalized = crate::tsconfig::normalize_module_path(path);
    if cfg!(windows)
        && let Some(text) = normalized.to_str()
    {
        return std::path::PathBuf::from(windows_user_form(text));
    }
    normalized
}

/// String form of [`absolute_form`]'s Windows rule, testable on every host.
fn windows_user_form(text: &str) -> String {
    let plain = text.strip_prefix(r"\\?\UNC\").map_or_else(
        || text.strip_prefix(r"\\?\").unwrap_or(text).to_owned(),
        |rest| format!(r"\\{rest}"),
    );
    let mut characters = plain.chars();
    match (characters.next(), characters.next()) {
        (Some(drive), Some(':')) if drive.is_ascii_lowercase() => {
            format!(
                "{}{}",
                drive.to_ascii_uppercase(),
                plain.get(1..).unwrap_or_default()
            )
        },
        _ => plain,
    }
}

impl PathPattern {
    pub(crate) fn new(text: &str) -> Result<Self, glob::PatternError> {
        Ok(Self {
            pattern: glob::Pattern::new(&pattern_spelling(text))?,
            absolute: Path::new(text).is_absolute(),
        })
    }

    pub(crate) const fn is_absolute(&self) -> bool {
        self.absolute
    }

    /// Match one already-selected spelling regardless of pattern kind.
    pub(crate) fn matches_path(&self, path: &Path) -> bool {
        self.pattern.matches_path(path)
    }

    /// `relative` is the walk-root-relative path, `full` the walked path.
    pub(crate) fn matches(&self, relative: &Path, full: &Path) -> bool {
        if self.absolute {
            self.pattern.matches_path(&absolute_form(full))
        } else {
            self.pattern.matches_path(relative)
        }
    }
}

/// A fallible filtered walk: charge even excluded entries, prune excluded
/// directories, and stop immediately on exhaustion rather than draining them.
///
/// `skip` receives each entry's path relative to `root` and the walked path;
/// it is never consulted for the root itself. Exclusions therefore apply only beneath the walk root: a checkout
/// located under an ancestor named `build`, `dist`, `node_modules`, ... is still
/// scanned instead of being silently excluded as a whole.
pub(crate) fn walk<'a>(
    root: &Path,
    budget: &'a AnalysisBudget,
    skip: impl Fn(&Path, &Path) -> bool + 'a,
) -> impl Iterator<Item = io::Result<walkdir::DirEntry>> + 'a {
    let root = root.to_path_buf();
    let mut iterator = walkdir::WalkDir::new(&root).follow_links(false).into_iter();
    let mut done = false;
    std::iter::from_fn(move || {
        if done {
            return None;
        }
        loop {
            if let Err(error) = budget.check() {
                done = true;
                return Some(Err(io::Error::other(error)));
            }
            let entry = iterator.next()?;
            if let Err(error) = budget
                .io(Resource::Entries, 1)
                .and_then(|()| budget.io(Resource::Work, 1))
            {
                done = true;
                return Some(Err(error));
            }
            match entry {
                Err(error) => return Some(Err(io::Error::other(error))),
                Ok(entry) => {
                    // walkdir yields `root.join(..)` paths, so the prefix is
                    // always present; the full-path fallback can only exclude more.
                    let relative = entry
                        .path()
                        .strip_prefix(&root)
                        .unwrap_or_else(|_| entry.path());
                    // The walk root was selected explicitly (an operand, include
                    // root or package); patterns only prune beneath it.
                    let excluded = entry.depth() > 0 && skip(relative, entry.path());
                    if let Err(error) = budget.check() {
                        done = true;
                        return Some(Err(io::Error::other(error)));
                    }
                    if excluded {
                        if entry.file_type().is_dir() {
                            iterator.skip_current_dir();
                        }
                    } else {
                        return Some(Ok(entry));
                    }
                },
            }
        }
    })
}

/// Maximum bytes in a source file or configuration file (16 MiB).
pub(crate) const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// Read a regular UTF-8 file, detecting growth beyond the byte budget.
pub(crate) fn read_text(path: &Path) -> io::Result<String> {
    if !std::fs::metadata(path)?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "input must be a regular file",
        ));
    }
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "input must be a regular file",
        ));
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} must be a regular file of at most {MAX_FILE_BYTES} bytes",
                path.display()
            ),
        ));
    }
    read_utf8_limited(file, MAX_FILE_BYTES)
}

/// Read one extra byte to distinguish exact EOF from a growing/oversized input.
fn read_utf8_limited(reader: impl io::Read, limit: u64) -> io::Result<String> {
    let sentinel = limit.checked_add(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "input limit cannot reserve a sentinel byte",
        )
    })?;
    let mut contents = String::new();
    let _read_bytes = reader.take(sentinel).read_to_string(&mut contents)?;
    if units(contents.len()) > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("file exceeds {limit} bytes"),
        ));
    }
    Ok(without_bom(contents))
}

#[cfg(test)]
mod spelling_tests {
    use super::{spelling_with, windows_user_form};

    #[test]
    fn pattern_spellings_drop_current_directory_and_trailing_separators() {
        let cases = [
            ("./src/legacy/", false, "src/legacy"),
            ("././src", false, "src"),
            (".//src", false, "src"),
            ("./", false, "."),
            ("", false, "."),
            ("/", true, "/"),
            ("/repo/src/", true, "/repo/src"),
            (".hidden/", false, ".hidden"),
            // `\` is an ordinary character off Windows.
            (r".\src\", false, r".\src\"),
        ];
        for (text, absolute, expected) in cases {
            assert_eq!(spelling_with(text, absolute, false), expected, "{text:?}");
        }
        let windows = [
            (r".\src\legacy\", false, r"src\legacy"),
            (r".\.\src", false, "src"),
            (r"src\", false, "src"),
            (r"C:\repo\src\", true, r"C:\repo\src"),
            (r".\", false, "."),
            (r"\", true, r"\"),
        ];
        for (text, absolute, expected) in windows {
            assert_eq!(spelling_with(text, absolute, true), expected, "{text:?}");
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn only_absolute_windows_patterns_get_the_windows_form() {
        // Off Windows a relative pattern is never rewritten, even one that
        // looks like a drive.
        assert_eq!(super::pattern_spelling("a:b/"), "a:b");
        assert_eq!(super::pattern_spelling("/repo/x/"), "/repo/x");
    }

    #[test]
    fn windows_paths_lose_verbatim_prefixes_and_gain_uppercase_drives() {
        let cases = [
            (r"\\?\C:\repo\src", r"C:\repo\src"),
            (r"\\?\c:\repo", r"C:\repo"),
            (r"c:\repo", r"C:\repo"),
            (r"C:/repo", "C:/repo"),
            (r"\\?\UNC\server\share\x", r"\\server\share\x"),
            (r"\\server\share", r"\\server\share"),
            ("/unix/path", "/unix/path"),
        ];
        for (text, expected) in cases {
            assert_eq!(windows_user_form(text), expected, "{text:?}");
        }
    }
}

#[cfg(test)]
mod bounded_reader_tests {
    use super::*;

    #[test]
    fn exact_eof_and_one_extra_byte_have_different_results() {
        assert_eq!(
            read_utf8_limited(b"abcd".as_slice(), 4).expect("exact EOF"),
            "abcd",
            "exact limit accepted"
        );
        let error = read_utf8_limited(b"abcde".as_slice(), 4)
            .expect_err("extra byte must not be truncated");
        assert_eq!(
            error.kind(),
            io::ErrorKind::InvalidData,
            "growing input rejected"
        );
        assert!(
            read_utf8_limited(b"".as_slice(), u64::MAX).is_err(),
            "sentinel overflow rejected before reading"
        );
        assert!(
            read_utf8_limited(b"\xff".as_slice(), 4).is_err(),
            "non-UTF-8 input rejected"
        );
    }

    #[test]
    fn bounded_reads_do_not_drain_the_input_and_never_return_a_failed_prefix() {
        struct FailsAfterPrefix(bool);
        impl io::Read for FailsAfterPrefix {
            fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
                if self.0 {
                    return Err(io::Error::other("injected read failure"));
                }
                self.0 = true;
                b"x".as_slice().read(output)
            }
        }
        let mut reader = io::Cursor::new(b"abcdefghij");
        assert!(
            read_utf8_limited(&mut reader, 4).is_err(),
            "oversized input rejected"
        );
        assert_eq!(
            reader.position(),
            5,
            "read only the admitted data and one sentinel"
        );
        let error = read_utf8_limited(FailsAfterPrefix(false), 4)
            .expect_err("prefix is not a successful read");
        assert_eq!(
            error.kind(),
            io::ErrorKind::Other,
            "original read error preserved"
        );
    }
}

/// Metadata reads reserve a file slot before opening. Source queues reserve
/// their slots earlier so a directory cannot first materialize an oversized list.
pub(crate) fn read_text_budgeted(path: &Path, budget: &AnalysisBudget) -> io::Result<String> {
    budget.io(Resource::Files, 1)?;
    read_admitted(path, budget)
}

pub(crate) fn read_admitted(path: &Path, budget: &AnalysisBudget) -> io::Result<String> {
    budget.io(Resource::Work, 1)?;
    if !std::fs::metadata(path)?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "input must be a regular file",
        ));
    }
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "input must be regular",
        ));
    }
    budget.io(Resource::FileBytes, metadata.len())?;
    let mut data = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let next = data
            .len()
            .checked_add(count)
            .ok_or_else(|| io::Error::other("input size overflow"))?;
        budget.io(Resource::FileBytes, units(next))?;
        budget.io(Resource::Bytes, units(count))?;
        // count was returned by Read for this exact fixed buffer.
        if let Some(chunk) = buffer.get(..count) {
            data.extend_from_slice(chunk);
        }
    }
    String::from_utf8(data)
        .map(without_bom)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// Drop a leading UTF-8 byte-order mark, which Windows editors often write
/// and which npm, TypeScript and JavaScript parsers ignore.
pub(crate) fn without_bom(mut text: String) -> String {
    if text.starts_with('\u{feff}') {
        let _bom = text.drain(..'\u{feff}'.len_utf8());
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_budget_accepts_limit_and_rejects_next_byte() {
        let file = tempfile::NamedTempFile::new().expect("temporary input");
        file.as_file()
            .set_len(MAX_FILE_BYTES)
            .expect("set file size");
        assert_eq!(
            read_text(file.path())
                .expect("exact limit is accepted")
                .len(),
            usize::try_from(MAX_FILE_BYTES).expect("limit fits usize"),
            "exact byte limit"
        );
        file.as_file()
            .set_len(MAX_FILE_BYTES + 1)
            .expect("grow file");
        assert_eq!(
            read_text(file.path())
                .expect_err("first excessive byte is rejected")
                .kind(),
            io::ErrorKind::InvalidData,
            "oversized file is a boundary error"
        );
    }
}

#[cfg(test)]
mod aggregate_tests {
    use super::*;
    use crate::budget::AnalysisLimits;

    #[test]
    fn repeated_reads_consume_total_bytes_and_file_slots() {
        let file = tempfile::NamedTempFile::new().expect("temporary input");
        std::fs::write(file.path(), b"1234").expect("input");
        let budget = AnalysisBudget::new(AnalysisLimits {
            max_total_bytes: 8,
            max_file_bytes: 4,
            ..AnalysisLimits::default()
        })
        .expect("limits");
        for _ in 0..2 {
            assert_eq!(
                read_text_budgeted(file.path(), &budget).expect("exact cumulative limit"),
                "1234",
                "bounded read"
            );
        }
        assert!(
            read_text_budgeted(file.path(), &budget).is_err(),
            "re-reading is not free"
        );
        assert_eq!(
            budget.check().expect_err("exhaustion").resource(),
            Resource::Bytes,
            "aggregate quota"
        );
        assert_eq!(budget.used(Resource::Bytes), 8, "failed chunk not retained");
        assert_eq!(
            budget.used(Resource::FileBytes),
            4,
            "per-file maximum stays separate"
        );
        assert_eq!(
            budget.used(Resource::Files),
            3,
            "file slots charged before reading"
        );
    }
}
