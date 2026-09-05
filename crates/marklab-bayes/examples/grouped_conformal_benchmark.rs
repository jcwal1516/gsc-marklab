//! Development-only complete native service timing; no production benchmark dispatch.
use marklab_bayes::{fit_grouped_conformal, GroupedConformalSpec};
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
        let spec: GroupedConformalSpec = serde_json::from_slice(black_box(&bytes))?;
        last = serde_json::to_vec(&fit_grouped_conformal(spec)?)?;
        black_box(&last);
        timings.push(start.elapsed().as_nanos());
    }
    println!(
        "{}",
        serde_json::json!({"nanoseconds":timings,"result":serde_json::from_slice::<serde_json::Value>(&last)?})
    );
    Ok(())
}
