# Phase 0 Codebase Inventory

Inventory SHA: `a642fbcdd80b5baf784cd633b707dc0283a24d11`

## Size

The repository has 115 Rust files under `src/`.

| Category | Lines | Method |
| --- | ---: | --- |
| Production-oriented `src` files | 15,048 | Excludes dedicated files named `tests.rs`, `*_tests.rs`, and `algorithm_tests.rs`; inline test modules remain included. |
| Dedicated test files under `src` | 4,523 | Files named `tests.rs`, `*_tests.rs`, or `algorithm_tests.rs`. |
| Integration tests under `tests` | 3,518 | All Rust files, including two fixture generators. |
| Fuzz targets | 38 | All Rust files under `fuzz`. |
| Benchmarks | 347 | All Rust files under `benches`. |

Largest source files at baseline:

| Lines | File |
| ---: | --- |
| 1,345 | `src/spectra/structure_factor.rs` |
| 794 | `src/validation/tests.rs` |
| 612 | `src/neighborhood/tests.rs` |
| 585 | `src/cli/multimodal/analyze.rs` |
| 574 | `src/prepost/deltas.rs` |
| 560 | `src/config.rs` |
| 520 | `src/output/tests.rs` |
| 502 | `src/validation.rs` |
| 495 | `src/output/result_types.rs` |
| 455 | `src/api.rs` |
| 447 | `src/cli.rs` |
| 412 | `src/output/writer.rs` |
| 408 | `src/io/parquet.rs` |

A brace-counting source scan identified these largest production functions as review candidates: `AnalysisEngine::analyze_pattern_inner` (approximately 253 lines), `load_pattern_csv_with_diagnostics` (202), result assembly in `src/api/assembly.rs` (196), `summarize_permutation_whitening` (189), `load_pattern_parquet_with_diagnostics` (184), `AnalysisConfig::validate` (131), `MultimodalEngine::analyze` (125), the marked and multimodal output workflows (109 and 110), and `write_pattern_parquet` (100). These are approximate structural counts and are review triggers, not refactor targets by line count alone.

## Workspace and features

The root `Cargo.toml` defines one library crate and one `marklab` binary. Default features are `cli`, `parallel`, `parquet`, and `csv`. Optional features are `wsi`, `allocator-mimalloc`, and `dhat-heap`; the binary requires `cli`. The pinned toolchain is Rust 1.96.0 with Clippy and rustfmt.

CI exercises all features, no default features, `wsi,cli`, dependency policy, fuzz-target builds, a smoke Criterion profile, and a main-branch DHAT configuration. Scheduled workflows run full declared benchmarks and an independent public WSI oracle.

## Public API surface

`cargo public-api -sss --all-features --color never` reports 606 simplified public API lines. The no-default-feature surface still reports 551 lines, demonstrating that most analytical and result schema types are unconditionally public. The inventory confirms public filesystem and output boundaries such as `Pattern::from_paths`, `OutputWriter`, broad configuration structs, and marked/multimodal schema types.

## Dependency direction evidence

A literal dependency scan already shows boundary inversions that require architectural review:

- `output -> io` and `output -> multimodal`;
- `io -> spectra`, `io -> output`, and `io -> qc`;
- `registration -> output`;
- `diagnostics -> output` and `diagnostics -> multimodal`;
- extensive `api -> output` coupling.

Grouped imports mean these counts are incomplete; they are evidence of existing edges, not a complete dependency graph.

## Required textual audit results

- Production wildcard parent imports exist in the distributed `api` workflow and multiple CLI child modules. Additional `use super::*` occurrences are test-only.
- Task-scaffolding terminology exists in `src/registration/transform.rs` and a compatibility alias comment exists in `src/validation.rs`.
- Duplicate median, mean, finite-mean, min/max, and effective-length helpers were found across registration, diagnostics, pre/post, spectra, QC, API, validation, and enrichment modules.
- Non-finite or sentinel result construction is present in enrichment, territory QC overlap, profile/pre-post curve test errors, and empty pair-correlation bins.
- Empty placeholder vectors are produced for territory-profile enrichment/cross curves and multimodal timings.
- Domain enrichment and substantial cell-table behavior are gated by the `cli` feature.

## Semantic-navigation note

The LSP workspace root was pinned to `/Users/user/Bench/marklab-refactor`. No server existed before the task; the first `src/lib.rs` outline succeeded and created one Rust server for that exact root. A later outline of `src/spectra/structure_factor.rs` failed with a client capability error, so large-file inventory used targeted reads and textual structural scans instead of attempting to repair the LSP.
