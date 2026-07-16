#![no_main]

use libfuzzer_sys::fuzz_target;
use marklab::Pattern;

const FULL_PLANE_MASK: &str = r#"{"type":"MultiPolygon","coordinates":[[[[-1000000.0,-1000000.0],[1000000.0,-1000000.0],[1000000.0,1000000.0],[-1000000.0,1000000.0],[-1000000.0,-1000000.0]]]]}"#;

fuzz_target!(|bytes: &[u8]| {
    let Ok(directory) = tempfile::tempdir() else {
        return;
    };
    let cells = directory.path().join("cells.csv");
    let mask = directory.path().join("mask.geojson");
    if std::fs::write(&cells, bytes).is_err() || std::fs::write(&mask, FULL_PLANE_MASK).is_err() {
        return;
    }
    let _ = Pattern::from_paths(cells, mask);
});
