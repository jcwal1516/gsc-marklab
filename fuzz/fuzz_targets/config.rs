#![no_main]

use libfuzzer_sys::fuzz_target;
use mmrspace::AnalysisConfig;

fuzz_target!(|bytes: &[u8]| {
    if let Ok(text) = std::str::from_utf8(bytes) {
        let _ = AnalysisConfig::from_toml_overrides(text);
    }
});
