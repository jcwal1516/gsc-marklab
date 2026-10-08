#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Keep this smoke target within a bounded input corpus; the production parser also enforces
    // row, edge, inference-work and retained-memory admission before its scientific calculations.
    if data.len() <= 65_536 {
        let _ = marklab::analyze_multiplex_study(data);
    }
});
