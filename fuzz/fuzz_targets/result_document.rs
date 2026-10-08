#![no_main]

use libfuzzer_sys::fuzz_target;
use marklab::ResultDocument;

fuzz_target!(|bytes: &[u8]| {
    if let Ok(text) = std::str::from_utf8(bytes) {
        let _ = ResultDocument::from_json(text);
    }
});
