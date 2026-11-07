//! Visible representations of untrusted text in terminal labels.

/// Preserve ordinary Unicode while exposing controls, Unicode line separators
/// and the twelve Unicode `Bidi_Control` characters as visible escapes.
pub(crate) fn label(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if character.is_control()
            || matches!(
                character,
                '\u{061c}' | '\u{200e}'..='\u{200f}' | '\u{2028}'..='\u{202e}'
                    | '\u{2066}'..='\u{2069}'
            )
        {
            escaped.extend(character.escape_unicode());
        } else {
            escaped.push(character);
        }
    }
    escaped
}
