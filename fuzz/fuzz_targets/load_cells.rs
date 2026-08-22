#![no_main]

use libfuzzer_sys::fuzz_target;
use marklab::{PatternLoader, TumorMask};

const FULL_PLANE_MASK: &str = r#"{"type":"MultiPolygon","coordinates":[[[[-1000000.0,-1000000.0],[1000000.0,-1000000.0],[1000000.0,1000000.0],[-1000000.0,1000000.0],[-1000000.0,-1000000.0]]]]}"#;

fuzz_target!(|bytes: &[u8]| {
    let Ok(directory) = tempfile::tempdir() else {
        return;
    };
    let cells = directory.path().join("cells.csv");
    if std::fs::write(&cells, bytes).is_err() {
        return;
    }
    let Ok(mask) = TumorMask::from_geojson_str(FULL_PLANE_MASK) else {
        return;
    };
    let _ = PatternLoader::new(&mask).load(cells);
});
