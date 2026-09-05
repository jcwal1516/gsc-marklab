//! Development-only complete native service timing; no production benchmark dispatch.
use marklab::grouped_conformal::execute_native_request;
use std::{
    hint::black_box,
    io::{self, Read},
    time::Instant,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repeats: usize = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "10".into())
        .parse()?;
    if !(1..=1000).contains(&repeats) {
        return Err("repeat count outside 1..=1000".into());
    }
    let mut bytes = Vec::new();
    io::stdin()
        .take(128 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 128 * 1024 * 1024 {
        return Err("input exceeds 128 MiB".into());
    }
    let mut timings = Vec::new();
    let mut last = Vec::new();
    for _ in 0..repeats {
        let start = Instant::now();
        last = execute_native_request(black_box(bytes.clone()))?;
        black_box(&last);
        timings.push(start.elapsed().as_nanos());
    }
    // Preserve the exact production f64 serialization; do not decode into serde_json::Value.
    println!(
        "{{\"nanoseconds\":{},\"result\":{}}}",
        serde_json::to_string(&timings)?,
        std::str::from_utf8(&last)?
    );
    Ok(())
}
