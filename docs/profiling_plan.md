# mmrspace Profiling Plan

This document records the supported profiling commands and uses the same
workload names and command shapes as the CLI `profile-plan` output. It does not
include sample patient data.

## Representative Workload

Use a non-patient representative fixture supplied at runtime.
The example commands assume a workload stem named `representative`:

```bash
mmrspace analyze \
  --cells /data/representative.parquet \
  --mask /data/representative.geojson \
  --config /opt/mmrspace/examples/config.toml \
  --out /work/out/profile_run
```

## Samply

```bash
cargo build --profile profiling --bin mmrspace --features cli
samply record ./target/profiling/mmrspace analyze \
  --cells fixtures/representative.parquet \
  --mask fixtures/representative.geojson \
  --config examples/config.toml \
  --out out/profile_run
```

## Flamegraph

```bash
cargo flamegraph --profile profiling --bin mmrspace -- analyze \
  --cells fixtures/representative.parquet \
  --mask fixtures/representative.geojson \
  --config examples/config.toml \
  --out out/flamegraph_run
```

## DHAT

```bash
cargo run --profile profiling --features "cli dhat-heap" --bin mmrspace -- analyze \
  --cells fixtures/representative.parquet \
  --mask fixtures/representative.geojson \
  --config examples/config.toml \
  --out out/dhat_run \
  --heap-profile out/dhat-heap.json
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
