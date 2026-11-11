#![no_main]
use libfuzzer_sys::fuzz_target;

// One pure, bounded ingress: no filesystem, process, clock, or network adapters.
fuzz_target!(|bytes: &[u8]| {
    if bytes.len() > 32_768 {
        return;
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        let _result = jscycles::Config::parse(text);
    }
});
