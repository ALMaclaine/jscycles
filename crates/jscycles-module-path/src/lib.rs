//! Lexical JavaScript module path spelling: directory-naming specifiers and
//! dot-segment normalization. Nothing here touches the filesystem.

use std::path::{Path, PathBuf};

/// Whether a module path names a directory explicitly.
///
/// It does when it ends in a separator or its final segment is `.` or `..`.
/// Node and TypeScript resolve such a specifier only as a directory (index
/// file), never as `<name>.ts`. Lexical normalization loses this fact, so
/// callers must test the raw spelling.
#[must_use]
pub fn names_directory(path: &Path) -> bool {
    spelled_directory(path.as_os_str().as_encoded_bytes(), cfg!(windows), true)
}

/// Whether a `paths` substitution names a directory.
///
/// TypeScript appends extensions to a substituted `.` or `..` (so
/// `"@root": ["."]` can select a sibling `<dir>.ts`); only a trailing
/// separator forces directory lookup.
#[must_use]
pub fn ends_with_separator(path: &Path) -> bool {
    spelled_directory(path.as_os_str().as_encoded_bytes(), cfg!(windows), false)
}

/// Host-independent core of [`names_directory`] so both separator policies are
/// tested on every platform. A backslash separates components only on Windows.
fn spelled_directory(bytes: &[u8], windows: bool, dot_segments: bool) -> bool {
    let last = bytes
        .rsplit(|byte| *byte == b'/' || (windows && *byte == b'\\'))
        .next()
        .unwrap_or(bytes);
    last.is_empty() || (dot_segments && (last == b"." || last == b".."))
}

#[cfg(test)]
mod spelled_directory_tests {
    use super::spelled_directory;

    #[test]
    fn separators_and_dot_segments_name_directories_per_platform() {
        for windows in [false, true] {
            for spelled in ["./b/", ".", "..", "a/.", "a/..", ".//"] {
                assert!(
                    spelled_directory(spelled.as_bytes(), windows, true),
                    "{spelled}"
                );
            }
            for spelled in ["./b", "b", "a/.b", "a/..b", "./b.ts"] {
                assert!(
                    !spelled_directory(spelled.as_bytes(), windows, true),
                    "{spelled}"
                );
            }
            // `paths` substitutions: only a trailing separator.
            assert!(spelled_directory(b"src/lib/", windows, false));
            for spelled in [".", "..", "src/.", "src/lib"] {
                assert!(
                    !spelled_directory(spelled.as_bytes(), windows, false),
                    "{spelled}"
                );
            }
        }
        // A trailing backslash is a separator only on Windows; elsewhere it is
        // an ordinary file-name byte.
        assert!(spelled_directory(br".\b\", true, true));
        assert!(!spelled_directory(br".\b\", false, true));
        assert!(spelled_directory(br"a\..", true, true));
        assert!(!spelled_directory(br"a\..", false, true));
    }
}

/// Collapse dot segments, as JavaScript and TypeScript module resolution does
/// before filesystem lookup.
///
/// Native bytes and root or prefix semantics are kept. This lexical
/// operation neither follows symlinks nor provides filesystem containment.
#[must_use]
pub fn normalize_module_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut components: Vec<Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {},
            Component::ParentDir if matches!(components.last(), Some(Component::Normal(_))) => {
                let _ = components.pop();
            },
            Component::ParentDir if matches!(components.last(), Some(Component::RootDir)) => {},
            comp @ (Component::Prefix(_)
            | Component::RootDir
            | Component::ParentDir
            | Component::Normal(_)) => components.push(comp),
        }
    }
    components.iter().collect()
}

#[cfg(test)]
mod lexical_path_tests {
    use std::path::PathBuf;

    use super::normalize_module_path;

    #[test]
    fn lexical_parents_preserve_relative_escape_and_absolute_roots() {
        for (input, expected) in [
            ("../x", "../x"),
            ("../../x", "../../x"),
            ("a/../../x", "../x"),
            ("/../x", "/x"),
            ("/a/../../x", "/x"),
            ("a/./b/../c", "a/c"),
        ] {
            assert_eq!(
                normalize_module_path(&PathBuf::from(input)),
                PathBuf::from(expected),
                "lexical module path {input}"
            );
        }
    }
}
