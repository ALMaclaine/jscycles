#![no_main]
use std::fmt::Write as _;

use libfuzzer_sys::fuzz_target;

// Compile the actual private production adapters without enlarging the public
// library API solely for a fuzz harness. This module has no I/O dependencies.
#[path = "../../src/syntax.rs"]
#[expect(
    dead_code,
    reason = "each target exercises only part of the shared private adapter module"
)]
mod syntax;

fuzz_target!(|bytes: &[u8]| {
    if bytes.len() > 32_768 {
        return;
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return;
    };
    let _arbitrary_literal = syntax::module_literal(text);
    let normalized = syntax::jsonc(text);
    if let Ok(clean) = &normalized {
        assert_eq!(clean.len(), text.len(), "JSONC byte offsets");
    }
    if let Ok(original) = serde_json::from_str::<serde_json::Value>(text) {
        let clean = normalized.as_ref().expect("strict JSON must normalize");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(clean).expect("valid JSON stays valid"),
            original,
            "normalization preserves strict JSON values"
        );
    }
    let encoded = serde_json::to_string(text).expect("UTF-8 JSON string");
    assert_eq!(
        syntax::module_literal(&encoded).expect("shared JSON/JS string grammar"),
        text,
        "independent JSON encoding oracle"
    );
    let annotated = format!("/* prefix */ [{encoded},] // end");
    let clean = syntax::jsonc(&annotated).expect("generated valid JSONC");
    assert_eq!(clean.len(), annotated.len(), "generated JSONC byte offsets");
    let values: Vec<String> =
        serde_json::from_str(&clean).expect("comments/trailing comma normalize");
    assert_eq!(values.len(), 1, "one generated element");
    assert_eq!(
        values.first().map(String::as_str),
        Some(text),
        "generated JSONC value"
    );

    // At most 256 scalars / 512 UTF-16 units: bounds the generated literal and
    // exercises leading zeros and paired surrogate code units independently.
    let prefix: String = text.chars().take(256).collect();
    let mut escaped = String::from("\"");
    for unit in prefix.encode_utf16() {
        write!(&mut escaped, "\\u{{000000{unit:x}}}").expect("String formatter");
    }
    escaped.push('"');
    assert_eq!(
        syntax::module_literal(&escaped).expect("valid UTF-16 sequence"),
        prefix,
        "braced code units preserve the complete Unicode value"
    );
});
