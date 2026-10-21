#![no_main]

use libfuzzer_sys::fuzz_target;
use marklab::TumorMask;

// Input: GeoJSON text, a NUL byte, then query points as little-endian f64 (x, y) pairs.
fuzz_target!(|bytes: &[u8]| {
    let (geojson, points) = match bytes.iter().position(|&byte| byte == 0) {
        Some(split) => (&bytes[..split], &bytes[split + 1..]),
        None => (bytes, &[][..]),
    };
    let Ok(text) = std::str::from_utf8(geojson) else {
        return;
    };
    let Ok(mask) = TumorMask::from_geojson_str(text) else {
        return;
    };
    assert!(mask.area_um2().is_finite());
    for pair in points.chunks_exact(16) {
        let x = f64::from_le_bytes(pair[..8].try_into().expect("8-byte x"));
        let y = f64::from_le_bytes(pair[8..].try_into().expect("8-byte y"));
        let inside = mask.contains(x, y);
        assert_eq!(inside, mask.contains(x, y), "containment must be deterministic");
        if !(x.is_finite() && y.is_finite()) {
            assert!(!inside, "non-finite points are never inside");
        }
    }
});
