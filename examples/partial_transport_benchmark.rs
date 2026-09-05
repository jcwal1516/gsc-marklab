//! Development-only timing of the actual CSV application, including source binding/serialization.
use std::{hint::black_box, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 7 {
        return Err("expected repeats source.csv target.csv cost.csv mass epsilon timeout".into());
    }
    let repeats: usize = args[0].parse()?;
    if !(1..=100000).contains(&repeats) {
        return Err("repeat count outside 1..=100000".into());
    }
    let source = std::fs::read(&args[1])?;
    let target = std::fs::read(&args[2])?;
    let cost = std::fs::read(&args[3])?;
    let mass = args[4].parse()?;
    let epsilon = args[5].parse()?;
    let timeout = args[6].parse()?;
    let mut timings = Vec::with_capacity(repeats);
    let mut last = Vec::new();
    for _ in 0..repeats {
        let start = Instant::now();
        last = serde_json::to_vec(&marklab::transport::fit_partial_csv(
            black_box(&source),
            black_box(&target),
            black_box(&cost),
            mass,
            epsilon,
            timeout,
        )?)?;
        black_box(&last);
        timings.push(start.elapsed().as_nanos());
    }
    println!(
        "{{\"nanoseconds\":{},\"result\":{}}}",
        serde_json::to_string(&timings)?,
        std::str::from_utf8(&last)?
    );
    Ok(())
}
