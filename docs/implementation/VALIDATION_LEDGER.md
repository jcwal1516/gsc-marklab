# Validation ledger

Implementation base: `55fce12f10684a9081ca1f744f87d6f5feedcb24`

No command below is marked passing until it executes successfully on this branch.

| Gate | Exact command | Status | Result/evidence | Owner |
|---|---|---|---|---|
| Format | `cargo +1.96.0 fmt --all --check` | pass | Exit 0; 0.119 s | a02_baseline |
| Clippy | `cargo +1.96.0 clippy --locked --all-targets --all-features -- -D warnings` | pass | Exit 0; 11.315 s wall (Cargo 11.39 s) | a02_baseline |
| All-feature tests | `cargo +1.96.0 nextest run --locked --all-features` | pass | Exit 0; 402/402 passed, 22 skipped, 1 slow; 63.528 s test summary (~90 s including build) | a02_baseline |
| Documentation tests | `cargo +1.96.0 test --locked --doc --all-features` | pass | Exit 0; 0 passed, 0 failed, 0 ignored; 0.354 s | a02_baseline |
| No-default build | `cargo +1.96.0 check --locked --no-default-features` | pass | Exit 0; 5.840 s wall (Cargo 5.95 s) | a02_baseline |
| WSI integration | `cargo +1.96.0 test --locked --features wsi,cli --test wsi_integration` | pass | Exit 0; 10 passed, 0 failed, 1 ignored external fixture; 9.510 s wall | a02_baseline |
| Audit | `cargo audit` | pass with reviewed warnings | Exit 0; 334 dependencies/1,225 advisories; no vulnerabilities; allowed unmaintained `encoding`/RUSTSEC-2021-0153 and `paste`/RUSTSEC-2024-0436; 0.854 s | a02_baseline |
| Dependency policy | `cargo deny check advisories licenses bans sources` | pass with duplicate warnings | Exit 0; all four checks ok; duplicate versions reported for `getrandom`, `hashbrown`, `r-efi`, `thiserror`, `thiserror-impl`, `wit-bindgen`; 0.371 s | a02_baseline |
| Unused dependencies | `cargo machete` | pass | Exit 0; no unused dependencies; under 0.1 s | a02_baseline |
| Package | `cargo package --locked` | pass after expected dirty-tree retry | Initial exit 101 because 12 WS-A files were uncommitted; after commit `fc986c0`, exact rerun exited 0 in 15.6 s, packaged 237 files (1.7 MiB/381.0 KiB compressed), and verified by compiling the package | Lead/a02_baseline |
| Fuzz build | `cargo +nightly fuzz check` | pass | Exit 0; 13.510 s wall | a02_baseline |
| Benchmark smoke | `env MARKLAB_BENCH_PROFILE=smoke cargo +1.96.0 bench --locked --all-features -- --quick` | pass | Exit 0; release build 2m42s, about 2m44s total; eight workload intervals recorded in performance ledger; Gnuplot unavailable and Criterion used Plotters | a02_baseline |
| Heap regression | `cargo +1.96.0 test --locked --no-default-features --features dhat-heap --lib dhat_ -- --test-threads=1` | pass with build warnings | Exit 0; 3 passed, 0 failed, 0 ignored, 180 filtered; 11.944 s; build emitted 14 unused/dead-code warnings in this narrow feature combination | a02_baseline |
| Synthetic smoke | `cargo +1.96.0 run --release --locked --features wsi --bin marklab -- smoke --suite synthetic --replicates 10 --out /tmp/marklab-a02-smoke.xEna92` | pass | Exit 0; release build 1m33s/about 1m34s total; 12/12 scenarios, 120 attempted/completed, 0 failed; `smoke.json` 14,043 bytes | a02_baseline |
| WS-A bootstrap path/scope | `git diff --check` plus immutable-plan hash and required-file assertions | pass | 2026-08-22: corrected wrapper exited 0; status contained only `AGENTS.md` and `docs/implementation/` as untracked | Lead |
| A-03 Cargo inventory | `cargo +1.96.0 metadata --locked --format-version 1 --no-deps` | pass | One package/workspace member at baseline; features, targets, tests, examples, and benches captured | Lead |
| Requirement-ID coverage | `comm -23 <(rg -o '\b[A-Z]{2,}(?:-[A-Z0-9]+){1,3}\b' docs/implementation/MASTER_PLAN.md \| sort -u) <(rg -o '\b[A-Z]{2,}(?:-[A-Z0-9]+){1,3}\b' docs/implementation/REQUIREMENTS.md \| sort -u)` | pass | Only license/prose tokens remained: `AGPL-3`, `BSD-3`, `GPL-2`, `GPL-3`, `DEC-ID`, `MMR-IHC`, `NO-AI-SLOP`, `SHA-256`; every actual requirement/workstream ID is present | Lead |
| Semantic server ownership | `lsp server list` before/after pinned-root inspection | pass with fallback | No pre-existing servers; one Rust server created for `/Users/user/Bench/gsc-marklab`; first outline succeeded, second query failed; targeted reads used; exact task server stopped and final list reported no servers | Lead |
| Remote WSI/embedding inventory | Read-only authenticated SSH; exact commands retained in `handoffs/SLIDE-INV.md` | pass for inventory | 677 high-confidence WSI-compatible objects (~334.7 GB); checkpoint/model/run/slide hashes; safe NPY/H5/snappy schema inspection; no `.pt` deserialization; no patient data copied | remote_slide_inventory |

## Harness failures

- 2026-08-22: the first combined WS-A path-check wrapper exited 127 at its final `git` calls because the loop variable `path` shadowed zsh's special `path`/`PATH` array. This did not exercise or fail a repository gate. The wrapper was corrected to use `required_doc`; all assertions and `git diff --check` then exited 0.
- 2026-08-22: the first `cargo package --locked` exited 101 solely because the required WS-A bootstrap was uncommitted. It was not weakened with `--allow-dirty`. The exact command passed after commit `fc986c0c4e06216cc85d55d2315482a0107bf5b7`.

## Tool/environment evidence

- Host: macOS 26.5.2 arm64, Apple M4 Pro, 48 GiB memory, 12 logical CPUs.
- Compiler: Rust 1.96.0, LLVM 22.1.2.
- Tools: Nextest 0.9.136, cargo-audit 0.22.1, cargo-deny 0.19.4, cargo-machete 0.9.2, cargo-fuzz 0.13.1; nightly toolchain present.
- Optional documentation/workflow linters: `command -v markdownlint-cli2` and `command -v actionlint` each exited 1; neither executable is installed locally. Markdown source/path checks and repository workflow-contract tests ran instead; they are not claimed as equivalent lint passes.
- Generated ignored/output roots: `target/`, `fuzz/target/`, and `/tmp/marklab-a02-smoke.xEna92`. `cargo audit` refreshed the user-level advisory database. No tracked file was changed by A-02.

## Scientific evidence state

- Pinned-HEAD V-RUN evidence: 402/402 executable Nextest tests plus the documented feature, WSI, fuzz-build, DHAT, benchmark, and smoke gates reproduced under A-02.
- Historical evidence: parent commits describe extensive tests/calibration/benchmarks; historical only until reproduced.
- New scientific methods: none introduced in WS-A.
- Real-data and external-cohort assets: public TCGA/CPTAC and additional validation cohorts are present remotely, but no new Marklab scientific method has yet been validated against them.
- CellViT embedding provenance: checkpoint/source/run/output evidence is available; stable row identity, safe `.pt` conversion, extraction-layer/pooling/normalization fields, and patch-link contracts remain unresolved.
