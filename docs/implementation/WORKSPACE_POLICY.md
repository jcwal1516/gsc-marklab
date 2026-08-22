# Workspace policy

Status: active from B-03. Cargo-observed tests in `tests/workspace_contract.rs` and workflow assertions in `tests/workflow_contract.rs` enforce this document.

## Package and dependency direction

Local package layers are explicit and descending:

```text
marklab (compatibility facade/application integration)
  -> marklab-workflow (typed DAG and execution)
    -> marklab-project (project and artifact ownership)
```

A package may depend only on a lower numbered layer. `marklab-project` cannot depend on workflow or compatibility code; `marklab-workflow` cannot depend on `marklab`. The root package owns adapters that invoke existing Marklab engines through workflow nodes, so scientific implementations are not copied downward.

The root package may gate the exact adapter and test-module files frozen in `workspace_contract`, including CLI and synthetic-smoke entry points. Non-root project/workflow/core libraries may not define or use a `cli` feature, depend on command-line parsing, or hide stable scientific definitions behind CLI compilation.

## Repository-wide versus package-specific commands

Formatting, compilation, Clippy, Nextest, documentation, packaging, and Criterion commands use `--workspace`. Commands intentionally exercising the current compatibility binary or its WSI/DHAT integration name `--package marklab`. `fuzz/` is a deliberately excluded standalone cargo-fuzz workspace with its own lockfile.

No `xtask` is introduced in B-03. Direct Cargo commands plus the CI feature matrix contain no repeated repository-specific orchestration that warrants another package or command layer.

## Compile and lint feature matrix

CI runs `cargo check --locked --workspace --all-targets` with each argument set below:

| Category | Arguments | Contract exercised |
|---|---|---|
| Default | none | Supported aggregate package defaults |
| Minimal | `--no-default-features` | Stable library without adapters |
| Aggregate | `--all-features` | All optional adapters and allocators compile together |
| CSV only | `--no-default-features --features csv` | CSV adapter without Parquet/CLI |
| Parquet only | `--no-default-features --features parquet` | Parquet adapter without CSV/CLI |
| CLI | `--no-default-features --features cli` | CLI/CSV without Parquet or parallel execution |
| WSI library | `--no-default-features --features wsi` | WSI reader without command-line adapters |
| WSI CLI | `--no-default-features --features wsi,cli` | WSI commands without unrelated defaults |

Warnings-denied Clippy runs for all features and the cfg-sensitive CLI-only combination. Other narrow matrix rows are compile gates and are not claimed warning-clean: current test instrumentation emits warnings without default features, and Parquet-only also exposes unused internal writer code whose caller is a CLI adapter. The existing no-default `dhat-heap` regression remains a separate root-package gate. Feature-distinct `assert_cmd::cargo_bin` suites run serially in one checkout because they share the built binary path.

## Test categories

| Category | Owners/examples | Execution policy |
|---|---|---|
| Unit and property | Module tests, `proptest`, exact small oracles | Nextest workspace all-features; focused test while developing |
| Public integration | `tests/api_contract.rs`, CLI/config/result/multimodal suites | Compatibility gate; feature-specific suites remain explicit |
| Architecture/workflow | `workspace_contract`, `workflow_contract` | Every workspace/policy change |
| WSI local | `wsi_integration` local fixtures | Root package with `wsi,cli`; external Aperio case remains scheduled |
| Documentation | Rustdoc/doctests and public Markdown contracts | Workspace doc test at phase boundaries; path/workflow checks when local linters are unavailable |
| Heap | DHAT-filtered library tests | Main-branch regression under its supported feature set |
| Fuzz build | Standalone `fuzz/` targets | Build in pull-request CI; campaigns remain scheduled/manual |
| Synthetic validation | Deterministic `smoke --suite synthetic` | Main-branch smoke artifact; never represented as formal calibration |
| Calibration | Ignored 1,000-replicate negative controls | Scheduled/manual, never represented as pull-request smoke evidence |

## Benchmark categories

| Category | Profile | Execution policy |
|---|---|---|
| Pull-request smoke | `MARKLAB_BENCH_PROFILE=smoke`, Criterion `--quick` | Representative correctness-checked workloads and resource artifact |
| Scheduled full | `MARKLAB_BENCH_PROFILE=full` | Full declared workloads and retained Criterion/resource artifacts |
| Heap regression | DHAT assertions | Allocation/peak-memory correctness, not a timing comparison |

New benchmark targets require a real workload, equivalent-work assertion or checksum, and ownership in `Cargo.toml`/`tests/workflow_contract.rs`. An `xtask` becomes eligible only when direct Cargo/CI composition causes duplicated semantics that cannot be represented safely in the matrix.
