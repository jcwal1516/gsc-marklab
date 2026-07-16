use super::*;

pub(super) fn run(workload: &str, out: PathBuf) -> Result<()> {
    let text = format!(
        r#"# mmrspace Profiling Plan

Workload: `{workload}`

## Samply

```bash
cargo build --profile profiling --bin mmrspace --features cli
samply record ./target/profiling/mmrspace analyze --cells fixtures/{workload}.parquet --mask fixtures/{workload}.geojson --config examples/config.toml --out out/profile_run
```

## Flamegraph

```bash
cargo flamegraph --profile profiling --bin mmrspace -- analyze --cells fixtures/{workload}.parquet --mask fixtures/{workload}.geojson --config examples/config.toml --out out/flamegraph_run
```

## DHAT

```bash
cargo run --profile profiling --features "cli dhat-heap" --bin mmrspace -- analyze --cells fixtures/{workload}.parquet --mask fixtures/{workload}.geojson --config examples/config.toml --out out/dhat_run --heap-profile out/dhat-heap.json
```

## Assembly

```bash
cargo asm mmrspace::AnalysisEngine::analyze_pattern --profile profiling
```

## Criterion

```bash
cargo bench --bench structure_factor
cargo bench --bench permutation_engine
cargo bench --bench periodogram
cargo bench --bench wavelet
cargo bench --bench random_labeling_envelope
cargo bench --bench pattern_load
```
"#
    );

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out, text)?;
    Ok(())
}
