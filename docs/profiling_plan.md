# Profiling

Commands for measuring where `marklab analyze` spends time and memory. This is the plan that
`marklab profile-plan --workload representative --out profiling_plan.md` writes. No sample data is
included: put your own representative, non-patient cell table and mask at
`fixtures/representative.parquet` and `fixtures/representative.geojson`.

## Samply

```bash
cargo build --profile profiling --bin marklab --features cli
samply record ./target/profiling/marklab analyze \
  --cells fixtures/representative.parquet \
  --mask fixtures/representative.geojson \
  --config examples/config.toml \
  --out out/profile_run
```

## Flamegraph

```bash
cargo flamegraph --profile profiling --bin marklab -- analyze \
  --cells fixtures/representative.parquet \
  --mask fixtures/representative.geojson \
  --config examples/config.toml \
  --out out/flamegraph_run
```

## DHAT

```bash
cargo run --profile profiling --features "cli dhat-heap" --bin marklab -- analyze \
  --cells fixtures/representative.parquet \
  --mask fixtures/representative.geojson \
  --config examples/config.toml \
  --out out/dhat_run \
  --heap-profile out/dhat-heap.json
```

## Assembly

```bash
cargo asm marklab::AnalysisEngine::analyze_pattern --profile profiling
```

## Criterion

```bash
cargo bench --bench structure_factor
cargo bench --bench permutation_engine
cargo bench --bench periodogram
cargo bench --bench multiscale_residual
cargo bench --bench random_labeling_envelope
cargo bench --bench pattern_load
cargo bench --bench spatial_stages
cargo bench --bench cohort_hierarchy
cargo bench --bench cell_embeddings
cargo bench --bench patch_embeddings
```
