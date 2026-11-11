//! Bounded text adapters. These functions never access the filesystem.
use std::borrow::Cow;

/// Decode the `StringValue` of a quoted JS module literal. Reject non-Unicode
/// values (unpaired UTF-16 surrogates) because paths/specifiers use UTF-8.
pub(crate) fn module_literal(text: &str) -> Result<Cow<'_, str>, &'static str> {
    let quote = text.chars().next().ok_or("empty module literal")?;
    if !matches!(quote, '\'' | '"') {
        return Err("module specifier must be a quoted literal");
    }
    let inner = text
        .strip_prefix(quote)
        .and_then(|rest| rest.strip_suffix(quote))
        .ok_or("unterminated module literal")?;
    if !inner.contains('\\') {
        return Ok(Cow::Borrowed(inner));
    }
    let mut chars = inner.chars().peekable();
    let mut units = Vec::new();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            let mut buffer = [0; 2];
            units.extend_from_slice(ch.encode_utf16(&mut buffer));
            continue;
        }
        let escaped = chars.next().ok_or("unfinished module escape")?;
        let value = match escaped {
            '\n' | '\u{2028}' | '\u{2029}' => continue,
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    let _ = chars.next();
                }
                continue;
            },
            'b' => 8,
            'f' => 12,
            'n' => 10,
            'r' => 13,
            't' => 9,
            'v' => 11,
            '0' if !chars.peek().is_some_and(char::is_ascii_digit) => 0,
            '0'..='9' => return Err("legacy numeric module escapes are unsupported"),
            'x' => hex(&mut chars, 2)?,
            'u' if chars.peek() == Some(&'{') => {
                let _ = chars.next();
                let value = unicode_code_point(&mut chars)?;
                // ECMAScript code points include surrogate code units. Validate
                // the final UTF-16 sequence after adjacent escapes can pair.
                push_code_point(&mut units, value)?;
                continue;
            },
            'u' => hex(&mut chars, 4)?,
            other => {
                let mut buffer = [0; 2];
                units.extend_from_slice(other.encode_utf16(&mut buffer));
                continue;
            },
        };
        units.push(u16::try_from(value).map_err(|_error| "module escape exceeds code unit")?);
    }
    String::from_utf16(&units)
        .map(Cow::Owned)
        .map_err(|_error| "module specifier contains an unpaired surrogate")
}

fn push_code_point(units: &mut Vec<u16>, value: u32) -> Result<(), &'static str> {
    if let Ok(unit) = u16::try_from(value) {
        units.push(unit);
    } else {
        let scalar = char::from_u32(value).ok_or("invalid Unicode module code point")?;
        let mut buffer = [0; 2];
        units.extend_from_slice(scalar.encode_utf16(&mut buffer));
    }
    Ok(())
}

/// Parse the braced ECMAScript code point, including surrogate code units.
fn unicode_code_point(chars: &mut impl Iterator<Item = char>) -> Result<u32, &'static str> {
    let mut value = 0u32;
    let mut saw_digit = false;
    loop {
        let digit = chars.next().ok_or("unterminated Unicode module escape")?;
        if digit == '}' {
            return if saw_digit {
                Ok(value)
            } else {
                Err("empty Unicode module escape")
            };
        }
        saw_digit = true;
        value = value
            .checked_mul(16)
            .and_then(|previous| previous.checked_add(digit.to_digit(16)?))
            .filter(|point| *point <= 0x10_ffff)
            .ok_or("invalid Unicode module escape")?;
    }
}
fn hex(chars: &mut impl Iterator<Item = char>, count: usize) -> Result<u32, &'static str> {
    let mut result = 0u32;
    for _ in 0..count {
        let digit = chars
            .next()
            .and_then(|ch| ch.to_digit(16))
            .ok_or("invalid hexadecimal module escape")?;
        result = result
            .checked_mul(16)
            .and_then(|value| value.checked_add(digit))
            .ok_or("module escape overflow")?;
    }
    Ok(result)
}

/// Replace JSONC comments/trailing commas with whitespace without joining tokens
/// or changing byte offsets. Strict JSON parsing still checks structure/strings.
#[expect(
    clippy::indexing_slicing,
    reason = "Cursors are checked against the immutable buffer length before every indexed access; lookahead uses get"
)]
pub(crate) fn jsonc(input: &str) -> Result<String, &'static str> {
    let mut bytes = input.as_bytes().to_vec();
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        bytes[0..3].fill(b' ');
    }
    let mut cursor = 0;
    let mut string = false;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' if string => {
                cursor += 1;
            },
            b'"' => string = !string,
            b'/' if !string && bytes.get(cursor + 1) == Some(&b'/') => {
                while cursor < bytes.len() && !matches!(bytes[cursor], b'\n' | b'\r') {
                    bytes[cursor] = b' ';
                    cursor += 1;
                }
                continue;
            },
            b'/' if !string && bytes.get(cursor + 1) == Some(&b'*') => {
                bytes[cursor] = b' ';
                cursor += 1;
                bytes[cursor] = b' ';
                cursor += 1;
                let mut closed = false;
                while cursor < bytes.len() {
                    if bytes[cursor] == b'*' && bytes.get(cursor + 1) == Some(&b'/') {
                        bytes[cursor] = b' ';
                        cursor += 1;
                        bytes[cursor] = b' ';
                        cursor += 1;
                        closed = true;
                        break;
                    }
                    if !matches!(bytes[cursor], b'\n' | b'\r') {
                        bytes[cursor] = b' ';
                    }
                    cursor += 1;
                }
                if !closed {
                    return Err("unterminated JSONC comment");
                }
                continue;
            },
            _ => {},
        }
        cursor += 1;
    }
    cursor = 0;
    string = false;
    let mut previous = None;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        match byte {
            b'\\' if string => {
                cursor += 1;
            },
            b'"' => string = !string,
            b',' if !string => {
                let next = bytes.get(cursor + 1..).and_then(|tail| {
                    tail.iter()
                        .find(|candidate| !candidate.is_ascii_whitespace())
                });
                if matches!(next, Some(b'}' | b']'))
                    && !matches!(previous, None | Some(b'{' | b'[' | b',' | b':'))
                {
                    bytes[cursor] = b' ';
                }
            },
            _ => {},
        }
        if !byte.is_ascii_whitespace() {
            previous = Some(byte);
        }
        cursor += 1;
    }
    String::from_utf8(bytes).map_err(|_error| "invalid JSONC text")
}

/// Compare the lexical `IdentifierName` value without allocating a decoded name.
/// Only Unicode escapes are legal in identifiers; module-string escapes have
/// different rules and must not be used to reinterpret a callee.
pub(crate) fn is_require_identifier(text: &str) -> bool {
    if !text.contains('\\') {
        return text == "require";
    }
    let mut chars = text.chars().peekable();
    for expected in "require".chars() {
        let Some(character) = chars.next() else {
            return false;
        };
        let point = if character == '\\' {
            if chars.next() != Some('u') {
                return false;
            }
            let decoded = if chars.peek() == Some(&'{') {
                let _opening = chars.next();
                unicode_code_point(&mut chars)
            } else {
                hex(&mut chars, 4)
            };
            let Ok(point) = decoded else {
                return false;
            };
            point
        } else {
            u32::from(character)
        };
        if point != u32::from(expected) {
            return false;
        }
    }
    chars.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn require_identifier_values_match_independently_encoded_unicode_escapes() {
        use std::fmt::Write as _;

        for mask in 0_u8..128 {
            for braced in [false, true] {
                let mut identifier = String::new();
                for (position, letter) in "require".chars().enumerate() {
                    if mask & (1 << position) == 0 {
                        identifier.push(letter);
                    } else if braced {
                        write!(identifier, "\\u{{000{:x}}}", u32::from(letter))
                            .expect("format into String");
                    } else {
                        write!(identifier, "\\u{:04x}", u32::from(letter))
                            .expect("format into String");
                    }
                }
                assert!(
                    is_require_identifier(&identifier),
                    "same IdentifierName value"
                );
                assert!(
                    !is_require_identifier(&format!("{identifier}x")),
                    "a longer identifier is a different callee"
                );
                assert!(
                    !is_require_identifier(&format!("x{identifier}")),
                    "a prefixed identifier is a different callee"
                );
            }
        }
    }

    #[test]
    fn require_identifiers_reject_other_names_and_non_identifier_escapes() {
        for text in [
            "",
            "requi",
            "Require",
            "requires",
            "object.require",
            r"requ\u0066re",
            r"\x72equire",
            r"\require",
            r"\u{72equire",
            r"\u{}equire",
            r"\u{110000}equire",
            r"\u00equire",
            r"\ud800equire",
            r"requir\u0065x",
            r"\u0072equi",
        ] {
            assert!(
                !is_require_identifier(text),
                "callee {text:?} is not require"
            );
        }
    }
    #[test]
    fn module_values_and_rejections() {
        for (literal, value) in [
            (r"'./\u0062'", "./b"),
            (r#""\x62""#, "b"),
            (r"'\u{1f600}'", "😀"),
            (r"'\ud83d\ude00'", "😀"),
            ("'a\\\r\nb'", "ab"),
            (r"'\q'", "q"),
            (r"'\0'", "\0"),
            (r"'\b\f\n\r\t\v'", "\u{8}\u{c}\n\r\t\u{b}"),
            ("'a\\\nb'", "ab"),
            ("'a\\\rb'", "ab"),
            ("'a\\\u{2028}b'", "ab"),
            ("'a\\\u{2029}b'", "ab"),
        ] {
            assert_eq!(
                module_literal(literal).expect("valid literal"),
                value,
                "decoded StringValue"
            );
        }
        for literal in [
            "",
            "unquoted",
            "'",
            "'unfinished\\'",
            r"'\u{}'",
            r"'\u{110000}'",
            r"'\ud800'",
            r"'\01'",
            r"'\xzz'",
        ] {
            assert!(
                module_literal(literal).is_err(),
                "invalid or unsupported Unicode value rejected"
            );
        }
    }
    #[test]
    fn jsonc_preserves_strings_offsets_and_rejects_token_joining() {
        let text = "{ /*c*/ \"url\": \"https://a/\\\"b\", \"v\": [1,],}";
        let clean = jsonc(text).expect("JSONC");
        assert_eq!(clean.len(), text.len(), "offsets preserved");
        assert!(
            serde_json::from_str::<serde_json::Value>(&clean).is_ok(),
            "comments and trailing commas accepted"
        );
        for invalid in ["{,}", "[1,,]", "[1/*c*/2]", "{/*unfinished", "/*/", "x/*/"] {
            assert!(
                jsonc(invalid).is_err()
                    || serde_json::from_str::<serde_json::Value>(
                        &jsonc(invalid).expect("closed comments")
                    )
                    .is_err(),
                "invalid structure stays invalid"
            );
        }
    }

    #[test]
    fn jsonc_handles_backslashes_quotes_and_eof_without_changing_values() {
        for input in ["/*/", "x/*/", r#"/*/{"ok":true}"#] {
            assert!(
                jsonc(input).is_err(),
                "a closing comment cannot overlap its opening delimiter"
            );
        }
        // Each value is independently encoded as strict JSON; the adapter must
        // preserve it exactly even when escapes precede comment/comma tokens.
        for value in [
            "\\",
            "\\\\",
            "\"",
            "\\\"//still a string",
            "\\\"/* text */",
            "\\\",]",
            "é😀",
        ] {
            let encoded = serde_json::to_string(value).expect("JSON string");
            assert_eq!(
                jsonc(&encoded).expect("strict JSON string"),
                encoded,
                "strings stay byte-for-byte unchanged"
            );
            let document = format!("{{\"value\":{encoded},/*comment*/}}");
            let clean = jsonc(&document).expect("valid JSONC");
            let decoded: serde_json::Value = serde_json::from_str(&clean).expect("valid JSON");
            assert_eq!(decoded["value"], value, "escaped string value preserved");
        }
        for input in [
            "",
            "// comment",
            "// comment\r",
            "// comment\n",
            "/*closed*/",
            "[1, ]",
        ] {
            assert_eq!(
                jsonc(input).expect("closed text").len(),
                input.len(),
                "EOF offset preserved"
            );
        }
        assert_eq!(
            jsonc("// comment").expect("EOF comment"),
            "          ",
            "entire EOF comment is blanked"
        );
        for (input, expected) in [
            ("/*x*/", "     "),
            ("a/*x*/b", "a     b"),
            ("/*/*/", "     "),
            ("a/*x*y*/b", "a       b"),
            ("a/*x/y*/b", "a       b"),
            ("/*x\ny\rz*/", "   \n \r   "),
            (r#"["a,b,]",]"#, r#"["a,b,]" ]"#),
            (r#"["a,}",]"#, r#"["a,}" ]"#),
        ] {
            assert_eq!(
                jsonc(input).expect("closed text"),
                expected,
                "only comments and trailing delimiters change"
            );
        }
    }
}

#[cfg(test)]
mod generated_tests {
    #[test]
    fn json_string_values_round_trip_through_the_js_literal_decoder() {
        for point in (0..=0x0010_ffff).step_by(997) {
            if let Some(ch) = char::from_u32(point) {
                let value = format!("a{ch}\\\"é😀");
                let literal = serde_json::to_string(&value).expect("JSON string");
                assert_eq!(
                    super::module_literal(&literal).expect("valid JS subset"),
                    value,
                    "independent JSON encoding oracle for Unicode scalar {point}"
                );
            }
        }
    }
}
