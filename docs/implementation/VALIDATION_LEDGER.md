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

## B-01 workspace architecture evidence

| Gate | Exact command | Status | Result/evidence |
|---|---|---|---|
| Behavior-first workspace contract | `cargo +1.96.0 test --locked --test workspace_contract` | red, then pass | Initial run exited 101 because `Cargo.toml` had no `[workspace]`; subsequent contract refinements caught missing fuzz exclusion, premature explicit members, and explicit-root nested exclusion. Final state: 1 passed, 0 failed. The test invokes locked Cargo metadata and asserts the observed/default package is exactly root `marklab`, with unchanged lib/bin names and safety boundary. |
| Standalone fuzz metadata | `cargo +1.96.0 metadata --locked --format-version 1 --no-deps --manifest-path fuzz/Cargo.toml` | red during design, then pass | With explicit root membership, Cargo exited 101: the fuzz package believed it was in a workspace when it was not. With the root implicit and `exclude = ["fuzz"]`, exit 0 and fuzz resolves as its own workspace root. |
| Root metadata | `cargo +1.96.0 metadata --locked --format-version 1 --no-deps` | pass | Exit 0; one workspace/default member and one package, `marklab`; no lockfile delta. |
| Workflow compatibility | `cargo +1.96.0 test --locked --test workflow_contract` | pass | 6 passed, 0 failed. |
| API compatibility | `cargo +1.96.0 test --locked --test api_contract` | pass | 6 passed, 0 failed. |
| Workspace all-target/all-feature build | `cargo +1.96.0 check --locked --workspace --all-targets --all-features` | pass | Exit 0. |
| Workspace Clippy | `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings` | pass | Exit 0. |
| Format | `cargo +1.96.0 fmt --all --check` | pass | Exit 0. |
| Fuzz build | `cargo +nightly fuzz check` | pass | Exit 0; existing standalone package and targets compile unchanged. |
| Scope/diff | `git diff --check` and targeted `git diff -- Cargo.lock src/lib.rs src/config src/output .github/workflows fuzz/Cargo.toml fuzz/Cargo.lock` | pass | No whitespace error; no lockfile, production API/config/result/output, workflow, or fuzz-manifest delta. |
| Read-only architecture review | B-01 reviewer inspection of Cargo/public API/tests/CI | pass with recommendation applied | No edits. Confirmed fuzz must remain standalone; recommended deferring ceremonial crates to B-04 and using Cargo-observed assertions. Review-created semantic server was stopped. |

## Harness failures

- 2026-08-22: the first combined WS-A path-check wrapper exited 127 at its final `git` calls because the loop variable `path` shadowed zsh's special `path`/`PATH` array. This did not exercise or fail a repository gate. The wrapper was corrected to use `required_doc`; all assertions and `git diff --check` then exited 0.
- 2026-08-22: the first `cargo package --locked` exited 101 solely because the required WS-A bootstrap was uncommitted. It was not weakened with `--allow-dirty`. The exact command passed after commit `fc986c0c4e06216cc85d55d2315482a0107bf5b7`.
- 2026-08-22: B-01's first Cargo-observed integration test exited 101 for the intended reason: the root manifest had no workspace. Intermediate red runs caught a missing fuzz exclusion, premature project/workflow members, and explicit root membership. The direct fuzz metadata command also exited 101 under explicit root membership. These were test-driven design failures, not ignored gates; final forms pass and neither lockfile changed.
- 2026-08-22: one intermediate `workspace_contract` run invoked `env!("CARGO")` with a rustup `+1.96.0` argument. The direct toolchain Cargo executable correctly rejected that rustup-only selector. The test now invokes the exact Cargo executable without a selector; the outer test command remains pinned to 1.96.0 and passes.

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
