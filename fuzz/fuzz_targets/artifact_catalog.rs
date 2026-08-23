#![no_main]

use libfuzzer_sys::fuzz_target;
use marklab::ArtifactCatalog;

fuzz_target!(|bytes: &[u8]| {
    let _ = ArtifactCatalog::from_json(bytes);
});
