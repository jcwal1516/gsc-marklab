#![no_main]

use libfuzzer_sys::fuzz_target;
use marklab::TumorMask;

fuzz_target!(|bytes: &[u8]| {
    if let Ok(text) = std::str::from_utf8(bytes) {
        let _ = TumorMask::from_geojson_str(text);
    }
});
