# Validation ledger

Implementation base: `55fce12f10684a9081ca1f744f87d6f5feedcb24`

No command below is marked passing until it executes successfully on this branch.

## WS-30-KL-01 classical workflow checkpoint — 2026-08-24

Implementation base: `a1a335239c8bc8ca17afdd2f2f2865c03a1df71c` after the separate cadence commit.

| Gate | Exact command | Status | Result/evidence | Owner |
|---|---|---|---|---|
| Behavior-first domain red/green | `cargo +1.96.0 test --locked --all-features --test classical_spatial_domain` | expected missing-surface red, then pass | Initial compile failed on absent window/config/result APIs. Final 12/12 prove the hand K/L formula, brute-force indexed pair parity, rectangle/donut/concave/disconnected geometry, canonical order identity, closed boundaries, empty/singleton/duplicate/outside/malformed states, deterministic CSR, and one-short point/vertex/memory/pair/draw limits. | `/root` |
| Behavior-first project red/green | `cargo +1.96.0 test --locked --all-features --test classical_spatial_workflow` | expected missing-node/document red, then pass | Final 3/3 prove failure atomicity, miss/hit, seed invalidation, strict unknown/inconsistent-field rejection, and codec fixed point without result 0.3. | `/root` |
| Behavior-first CLI red/green | `cargo +1.96.0 test --locked --all-features --test classical_spatial_cli` | expected unknown-command red, then pass | Final 4/4 prove exact three-artifact atomic output, CSV/Parquet equality, typed zero/singleton results, malformed-input noncommit, and non-overwrite of occupied output. | `/root` |
| Affected compatibility | `cargo +1.96.0 test --locked --all-features --test cli analyze_cli_writes_result_json_from_csv_and_geojson_mask`; `cargo +1.96.0 test --locked --all-features --lib geom::tests` | pass | Existing analyze CSV/mask/result-0.3 workflow passes 1/1; existing geometry/mask/index suite passes 12/12. | `/root` |
| Formatting and warnings | `cargo +1.96.0 fmt --all --check`; `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings` | pass after one actual finding | First Clippy attempt rejected an explicit ring loop counter; the loop now uses `enumerate`. Final formatting and warning-denied Clippy exit 0. | `/root` |
| Minimal and feature matrix | the eight exact `cargo +1.96.0 check --locked --workspace --all-targets` rows in `WORKSPACE_POLICY.md` for default, no-default, all-features, CSV, Parquet, CLI, WSI, and WSI+CLI | pass with documented narrow warnings | All eight exit 0. Narrow feature rows emit only the pre-existing instrumentation/Parquet writer warnings explicitly allowed by workspace policy; all-feature warning-denied Clippy is clean. | `/root` |
| Strict documentation | `env RUSTDOCFLAGS=-Dwarnings cargo +1.96.0 doc --locked --no-deps --workspace --all-features`; `cargo +1.96.0 test --locked --workspace --doc --all-features` | pass | Strict workspace docs exit 0; all six package doc-test targets exit 0 with zero doctests. | `/root` |
| Architecture regression | `cargo +1.96.0 test --locked --all-features --test workspace_contract workspace_dependencies_descend_layers_and_core_libraries_are_not_cli_gated` | expected checkpoint finding, then pass | First workspace run found CLI cfg gates inside core adapter files. The seams remain compiled with narrowly reasoned dead-code allowances; the focused architecture contract passes 1/1. | `/root` |
| Full workspace | `cargo +1.96.0 nextest run --locked --workspace --all-features` | pass after architecture correction | Final run passes 950/950 in 65.360 s, with one expected slow synthetic test and 23 documented skips. The preceding run stopped at 833 pass/1 architecture failure and was not claimed green. | `/root` |
| Specialized evidence | benchmarks, fuzzing, DHAT/RSS, packaging, dependency audits, remote checks | not run; not applicable | The increment adds a correct exact algorithm/workflow with no optimization, parser-format, dependency, packaging, remote, or release claim requiring specialized evidence. Existing master-plan scale/oracle gates remain open for stable PP-01 promotion. | `/root` |

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
| Clean package | `cargo +1.96.0 package --locked` | pass | At clean commit `e8d57eed0c4b39bd651b7393e7af25b5a10a7558`: exit 0; 242 files, 1.7 MiB/389.6 KiB compressed; package verification compiled successfully. |
| Scope/diff | `git diff --check` and targeted `git diff -- Cargo.lock src/lib.rs src/config src/output .github/workflows fuzz/Cargo.toml fuzz/Cargo.lock` | pass | No whitespace error; no lockfile, production API/config/result/output, workflow, or fuzz-manifest delta. |
| Read-only architecture review | B-01 reviewer inspection of Cargo/public API/tests/CI | pass with recommendation applied | No edits. Confirmed fuzz must remain standalone; recommended deferring ceremonial crates to B-04 and using Cargo-observed assertions. Review-created semantic server was stopped. |

## B-02 compatibility-shell evidence

| Gate | Exact command | Status | Result/evidence |
|---|---|---|---|
| Marked direct/CLI parity | `cargo +1.96.0 test --locked --test cli analyze_cli_writes_result_json_from_csv_and_geojson_mask` | pass | 1 passed; identical mask/table/config produce exact CLI versus direct-library result-core equality after excluding execution timing telemetry. |
| Default API/config/result/CLI parity | `cargo +1.96.0 test --locked --test api_contract --test cli --test config_v02 --test result_v03 --test multimodal_cli` | pass | 65 passed, 0 failed: API 6, CLI 17, config 8, multimodal CLI 21, result 0.3 13. Includes exact marked and multimodal library/CLI core parity. |
| WSI feature/CLI parity | `cargo +1.96.0 test --locked --features wsi,cli --test wsi_integration --test cli` | pass with scheduled oracle ignored | CLI 16/16 and local WSI 10/10 passed; one public Aperio/OpenSlide oracle remains explicitly ignored because the checksummed external fixture/oracle is not local. |
| Output transactions/artifacts | `cargo +1.96.0 test --locked --lib output::tests` | pass | Read-only reviewer run: 16 passed, 0 failed. |
| No-default library | `cargo +1.96.0 check --locked --no-default-features` | pass | Exit 0; root library remains independently buildable. |
| CLI without default features | `cargo +1.96.0 test --locked --no-default-features --features cli --test cli --test multimodal_cli` | pass with existing warnings | Read-only reviewer run: 35/35 passed (13 CLI, 22 multimodal). Build emitted cfg-specific unused-import warnings in `src/cli/batch.rs`; B-03 owns the feature-matrix policy/fix because all-feature Clippy does not observe this combination. |
| Read-only shell audit | `git diff --exit-code 55fce12f10684a9081ca1f744f87d6f5feedcb24..HEAD -- src Cargo.lock fuzz .github` plus facade/target inspection | pass | No compatibility-surface delta through B-01; root library keeps private modules and re-exports, and the configured binary delegates only to `marklab::run_cli()`. Reviewer made no edits, accessed no remote system, and created no LSP server. |
| Workspace/workflow continuity | `cargo +1.96.0 test --locked --test workflow_contract --test workspace_contract` | pass | 7 passed, 0 failed. |
| Workspace Clippy | `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings` | pass | Exit 0. |
| Format/scope | `cargo +1.96.0 fmt --all --check` and `git diff --check` | pass | Exit 0; no production source, manifest, dependency, lockfile, config, result DTO, or CI change. |

## B-03 workspace-policy evidence

| Gate | Exact command | Status | Result/evidence |
|---|---|---|---|
| Behavior-first workflow policy | `cargo +1.96.0 test --locked --test workflow_contract workspace_policy_is_explicit` | red, then pass | Initial run failed on missing `cargo fmt --all --check`; final run passed after workspace/package-explicit commands and the feature matrix were added. |
| Affected-feature Clippy | `cargo +1.96.0 clippy --locked --workspace --all-targets --no-default-features --features cli -- -D warnings` | red, then pass | Initial run failed on the parallel-only `AnalysisConfig`, `MarklabError`, and `ThreadSetting` imports in `src/cli/batch.rs`; cfg-correct import gating fixed the warning without runtime change. Final exit 0. |
| Workflow contracts | `cargo +1.96.0 test --locked --test workflow_contract` | pass | 7 passed, 0 failed; covers PR/scheduled/release/public-WSI commands, matrix rows, fuzz, and benchmarks. |
| Dependency/CLI-gating architecture | `cargo +1.96.0 test --locked --test workspace_contract` | pass after characterization refinement | 2 passed, 0 failed. Cargo metadata enforces explicit descending package layers; recursive source scanning freezes the root CLI-gated adapter/test files and rejects CLI gating in future non-root libraries. |
| Compile matrix: default | `cargo +1.96.0 check --locked --workspace --all-targets` | pass | Exit 0 without warnings. |
| Compile matrix: no default | `cargo +1.96.0 check --locked --workspace --all-targets --no-default-features` | pass with known warnings | Exit 0; 14 existing test-instrumentation unused/dead-code warnings. |
| Compile matrix: all features | `cargo +1.96.0 check --locked --workspace --all-targets --all-features` | pass | Exit 0 without warnings. |
| Compile matrix: CSV | `cargo +1.96.0 check --locked --workspace --all-targets --no-default-features --features csv` | pass with known warnings | Exit 0; same 14 test-instrumentation warnings. |
| Compile matrix: Parquet | `cargo +1.96.0 check --locked --workspace --all-targets --no-default-features --features parquet` | pass with known warnings | Exit 0; 14 test-instrumentation warnings plus four internal Parquet-writer unused/dead-code warnings because its production caller is CLI-gated. |
| Compile matrix: CLI | `cargo +1.96.0 check --locked --workspace --all-targets --no-default-features --features cli` | pass | Exit 0 without warnings after the import fix. |
| Compile matrix: WSI | `cargo +1.96.0 check --locked --workspace --all-targets --no-default-features --features wsi` | pass with known warnings | Exit 0; same 14 test-instrumentation warnings. |
| Compile matrix: WSI CLI | `cargo +1.96.0 check --locked --workspace --all-targets --no-default-features --features wsi,cli` | pass | Exit 0 without warnings after the import fix. |
| CLI-only behavior | `cargo +1.96.0 test --locked --no-default-features --features cli --test cli --test multimodal_cli` | pass | 35 passed, 0 failed; no compile warnings after the cfg-only import fix. |
| All-feature Clippy | `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings` | pass | Exit 0. |
| Workflow YAML syntax | `ruby -e 'require "yaml"; ARGV.each { \|path\| YAML.load_file(path); puts path }' .github/workflows/benchmarks.yml .github/workflows/calibration.yml .github/workflows/ci.yml .github/workflows/release.yml .github/workflows/wsi-public.yml` | pass with limitation | All five files parsed. `actionlint` remains unavailable locally, so this is not claimed as an equivalent Actions semantic lint. |
| Read-only policy audit | B-03 reviewer inspection plus feature-specific check/Clippy runs | pass with recommendations applied | Confirmed workspace/package command gaps, compile-only matrix limitation, recursive source-scan need, downward dependency direction, category documentation, and no-`xtask` decision. Reviewer made no edits, used no LSP, accessed no remote system, and installed nothing. |
| Clean workspace package syntax | `cargo +1.96.0 package --locked --workspace` | pass before dirty implementation | At clean B-02 SHA `f1bcc94d4a5f96825fae32676630304f31d332ea`: 244 files, 1.7 MiB/393.4 KiB compressed; verification build passed. Must rerun after B-03 commit. |
| Format/scope | `cargo +1.96.0 fmt --all --check`; `git diff --check`; protected manifest/lock diff | pass | No manifest, lockfile, dependency, API, result/config/schema, scientific algorithm, or fuzz change. |

## B-04 project/workflow vertical-slice evidence

| Gate | Exact command | Status | Result/evidence |
|---|---|---|---|
| Behavior-first workspace membership | `cargo +1.96.0 test --locked --test workspace_contract` | red, then pass | Initial run rejected the missing two child members. Final contract passes and observes exactly `marklab`, `marklab-project`, and `marklab-workflow`, with root as the sole default member and fuzz still excluded. |
| Behavior-first project/workflow API | `cargo +1.96.0 test --locked --test project_workflow` | red, then pass | Initial compile exited 101 on unresolved project/workflow exports. Final run: 4 passed, 0 failed. It covers digest-key invalidation, deterministic cycle/missing/duplicate rejection, uncataloged and tampered inputs, execution/size/miss-decode/hit-decode/noncanonical-codec failure atomicity, scheduler recreation, typed result parity, committed-byte identity, deterministic QC/report bytes, and output inventory parity. |
| Canonical artifact regression | `cargo +1.96.0 test --locked --test project_workflow marked_workflow_matches_direct_compatibility_path` | red, then pass | Stronger byte verification first found that pre-normalization JSON was committed while the decoded value was returned (`expected_len = 7674`, `observed_len = 7648`). The scheduler now commits only a decode/re-encode fixed point; the exact artifact bytes verify against `OutputWriter`'s `result.json`. |
| Noncanonical-codec regression | `cargo +1.96.0 test --locked --test project_workflow` | red, then pass | Test-first compile exited 101 because `WorkflowError::NonCanonicalCodec` did not exist. A deliberately drifting codec now receives that typed error after execution and before any success/cache mutation. |
| Project child tests | `cargo +1.96.0 test --locked -p marklab-project -p marklab-workflow` | pass | Project 4/4 and workflow 1/1 passed; child doc-tests 0/0. Includes standard SHA-256 vector/streaming count, reference-only input state, immutable project-level inline cap, and conflicting-success atomicity. |
| Compatibility/architecture contracts | `cargo +1.96.0 test --locked --test workspace_contract --test workflow_contract --test api_contract` | pass | 15 passed, 0 failed (2 workspace, 7 workflow, 6 API). Dependency direction is exactly root → workflow → project and child libraries are not CLI-gated. |
| Child public documentation | `RUSTDOCFLAGS='-D missing-docs' cargo +1.96.0 doc --locked --no-deps -p marklab-project -p marklab-workflow` | pass | Exit 0 after the initial missing-doc failures were corrected; both child crates document their public surface. |
| All-feature workspace Clippy | `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings` | pass | Exit 0 after the final fixed-point codec change. |
| CLI-only workspace Clippy | `cargo +1.96.0 clippy --locked --workspace --all-targets --no-default-features --features cli -- -D warnings` | pass | Exit 0. |
| No-default workspace | `cargo +1.96.0 check --locked --workspace --all-targets --no-default-features` | pass with known warnings | Exit 0; the same 14 pre-recorded test-instrumentation warnings remain. No new child warning. |
| WSI integration | `cargo +1.96.0 test --locked --features wsi,cli --test wsi_integration` | pass with scheduled oracle ignored | 10 passed, 0 failed, 1 ignored external public-slide/OpenSlide oracle. |
| Root/fuzz metadata | `cargo +1.96.0 metadata --locked --format-version 1 --no-deps`; same with `--manifest-path fuzz/Cargo.toml` | pass | Root reports three packages and one default member; fuzz reports only `marklab-fuzz` with `/Users/user/Bench/gsc-marklab/fuzz` as its independent workspace root. |
| Dependency audit | `cargo audit` | pass with reviewed warnings | Exit 0; 336 dependencies/1,225 advisories; no vulnerability; the same allowed unmaintained `encoding` and `paste` warnings. |
| Dependency policy | `cargo deny check advisories licenses bans sources` | pass with duplicate warnings | Exit 0; all policy checks ok; existing duplicate-version warnings only. |
| Unused dependencies | `cargo machete` | pass | Exit 0; no unused dependencies in the repository crates. |
| Fuzz build | `cargo +nightly fuzz check` | pass | Exit 0 after the two local packages were added to the standalone fuzz resolution. |
| Format/scope | `cargo +1.96.0 fmt --all --check`; `git diff --check`; targeted manifest/lock/source diff | pass | Format exit 0 and reviewer diff check passed. Lock deltas contain only the two local package records/local edges in root and fuzz locks; no registry source/version/checksum delta. Existing algorithms, config/result 0.3 DTO, `OutputWriter`, CLI, features, CI, fuzz targets, and benchmarks are unchanged. |
| Clean workspace package | `cargo +1.96.0 package --locked --workspace` | pass | At clean implementation SHA `bcc9450`, project/workflow packages each contained 5 files and root contained 251 files; all verification builds passed. After the benchmark-harness fix, the clean `ac7da28` rerun also passed: 22.2 KiB/6.3 KiB project, 27.8 KiB/7.6 KiB workflow, and 1.8 MiB/413.1 KiB root. Child manifests emit non-fatal missing documentation/homepage/repository metadata warnings. |
| Independent B-04 audit | `b04_vertical_slice_audit` read-only review plus focused test/Clippy/diff check | pass after findings applied | Project-owned cap, byte-level parity, missing/tampered/hit-decode coverage, and fixed-point codec validation were added. Final re-review found no remaining material correctness issue; it retained the documented limitation that the artifact cap is not a peak serializer-memory bound. Reviewer edited no files and accessed no remote system. |

## WS-B phase-boundary evidence

| Gate | Exact command | Status | Result/evidence |
|---|---|---|---|
| All-feature workspace tests | `cargo +1.96.0 nextest run --locked --workspace --all-features` | pass | 414/414 passed, 22 skipped, 1 slow; 62.769 s summary. This includes all five child unit tests and all four project/workflow integration tests. |
| Workspace doc tests | `cargo +1.96.0 test --locked --workspace --doc --all-features` | pass | All three package doc-test binaries passed; each contains 0 executable doc tests. |
| Eight-row feature matrix | Eight `cargo +1.96.0 check --locked --workspace --all-targets ...` commands from `WORKSPACE_POLICY.md` | pass with recorded warnings | Default, no-default, all, CSV, Parquet, CLI, WSI, and WSI+CLI all exit 0. Default/all/CLI/WSI+CLI are warning-free; no-default/CSV/WSI retain 14 test-instrumentation warnings, and Parquet retains those plus four production unused/dead-code warnings. |
| Warnings-denied Clippy | All-feature and no-default CLI commands from CI | pass | Both exit 0 after the child benchmark metadata fix. |
| WSI boundary | `cargo +1.96.0 test --locked --package marklab --features wsi,cli --test wsi_integration` | pass with scheduled oracle ignored | 10 passed, 0 failed, 1 ignored external fixture/oracle. |
| Dependency boundary | `cargo audit`; `cargo deny check advisories licenses bans sources`; `cargo machete` | pass with reviewed warnings | No vulnerabilities or unused dependencies; same two allowed unmaintained advisories and same duplicate-version policy warnings. |
| Fuzz boundary | `cargo +nightly fuzz check` | pass | Root plus both child packages and all standalone fuzz targets compile in release fuzz mode. |
| Benchmark smoke | `env MARKLAB_BENCH_PROFILE=smoke cargo +1.96.0 bench --locked --workspace --all-features -- --quick` | red, then pass | First workspace run completed all Criterion measurements, then exited 101 because `--quick` reached a new child libtest harness. A manifest contract reproduced the missing `[lib] bench = false`; after both child manifests declared it, the exact command exited 0 and ran only the six explicit Criterion targets/eight workload intervals. Gnuplot absent; Plotters used. |
| Heap regression | `cargo +1.96.0 test --locked --package marklab --no-default-features --features dhat-heap --lib dhat_ -- --test-threads=1` | pass with recorded warnings | 3 passed, 0 failed, 180 filtered; same 14 narrow-feature warnings. |
| Synthetic smoke | `cargo +1.96.0 run --release --locked --package marklab --features wsi --bin marklab -- smoke --suite synthetic --replicates 10 --out /tmp/marklab-wsb-smoke.cbLqL7` | pass | Exit 0; 12/12 scenarios passed, 120 attempted/completed, 0 failed; `smoke.json` 14,043 bytes. This is smoke evidence, not formal calibration. |
| Clean package | `cargo +1.96.0 package --locked --workspace` | pass | Clean `ac7da28da082f795381da0a03e8437fc4a774262`; all three packages created and verification-compiled successfully. |

## C-01 typed identity and hierarchy evidence

Implementation commit: `a1260445a383c59381b2c7c7881cebb10e10c156`.

| Gate | Exact command | Status | Result/evidence |
|---|---|---|---|
| Behavior-first workspace boundary | `cargo +1.96.0 test --locked --test workspace_contract` | red, then pass | Initial contract rejected the absent core/data members. Final contract reports five packages, root as sole default member, standalone fuzz exclusion, exact descending layers core 0 → data 1 → project 2 → workflow 3 → root 4, and `bench = false` on all libraries. |
| Behavior-first hierarchy API | `cargo +1.96.0 test --locked --test data_hierarchy` | red, then pass | Initial compile failed because `marklab-data` and project hierarchy APIs did not exist. Final suite: 14 passed, 0 failed in 0.05 s. It covers every ID kind/bound, duplicate/conflicting/missing/unsupported structure, deterministic cycles, source failures, unresolved branches, role-kind safety, nested patient/specimen lineage, TMA donors, repeated-link ambiguity, factual summaries, filename non-inference, project atomicity, and 10,000-node chain/cycle-tail inputs. |
| Role-kind regression | same focused hierarchy suite | red, then pass | Test-first compile failed on missing `ReplicationRoleKind` and `UnsupportedReplicationRole`; the explicit matrix now prevents site/timepoint/cell/patch and lower physical objects from becoming biological units. |
| Nested-level regressions | `cargo +1.96.0 test --locked --test data_hierarchy nested_biological_levels_support_explicit_patient_level_designs` | red twice, then pass | The first run failed because patient-level repeated biological specimens resolved only to themselves. After lineage-aware validation/summaries, a second compile-first run failed on the absent membership query. Final behavior keeps nearest specimen resolution while explicit patient membership, patient-level pairing, and enclosing core/region summaries pass. |
| Affected package tests | `cargo +1.96.0 test --locked -p marklab-core -p marklab-data -p marklab-project` | pass | Core 1/1, data 1/1, and project 4/4 unit tests passed; all three doc-test binaries passed with 0 executable doc tests. |
| Compatibility/architecture contracts | `cargo +1.96.0 test --locked --test workspace_contract --test workflow_contract --test api_contract` | pass | 15 passed, 0 failed: workspace 2, workflow 7, API 6. Current result/config/CLI facade contracts remain unchanged. |
| Full all-feature workspace tests | `cargo +1.96.0 nextest run --locked --workspace --all-features` | pass | 430/430 passed, 22 skipped, 1 slow; 64.028 s summary. |
| Public documentation | `RUSTDOCFLAGS='-D missing-docs' cargo +1.96.0 doc --locked --no-deps -p marklab-core -p marklab-data -p marklab-project` | pass | All public items in the two new packages and affected project package document successfully. |
| All-feature workspace Clippy | `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings` | pass | Exit 0 after removing an unreachable `len`/`is_empty` surface from the non-empty hierarchy. |
| CLI-only workspace Clippy | `cargo +1.96.0 clippy --locked --workspace --all-targets --no-default-features --features cli -- -D warnings` | pass | Exit 0. |
| Root/fuzz metadata | `cargo +1.96.0 metadata --locked --format-version 1 --no-deps`; same with `--manifest-path fuzz/Cargo.toml` | pass | Root reports five workspace packages with root as sole default member; fuzz remains its own one-package workspace. |
| Standalone fuzz lock/build | `cargo +1.96.0 check --locked --manifest-path fuzz/Cargo.toml`; `cargo +1.96.0 check --offline --manifest-path fuzz/Cargo.toml`; `cargo +nightly fuzz check` | red, regenerated, then pass | Locked check first exited 101 on the expected stale independent lock. Offline Cargo added only local core/data records/edges; the final nightly fuzz build passed all targets. |
| Dependency boundary | `cargo audit`; `cargo deny check advisories licenses bans sources`; `cargo machete` | pass with reviewed warnings | Audit scanned 338 dependencies with no vulnerabilities and the same allowed unmaintained `encoding`/`paste` warnings. Deny passed with the existing duplicate-version warnings. Machete found no unused dependencies. |
| Hierarchy benchmark smoke/full | `env MARKLAB_BENCH_PROFILE=smoke cargo +1.96.0 bench --locked --workspace --all-features --bench cohort_hierarchy -- --quick`; same with `MARKLAB_BENCH_PROFILE=full` | pass | Correctness assertions ran on every iteration. Final smoke 10,000-cell interval: 1.1645–1.1726 ms; full 1,000,000-cell/10,000-specimen interval: 147.13–147.27 ms. No detected local performance change; Plotters used because Gnuplot is absent. |
| Lock/scope/format | `cargo +1.96.0 fmt --all --check`; `git diff --check`; registry source/checksum delta assertion; root-source diff and duplicate-owner search | pass | Only local package records/edges changed in root/fuzz locks; root `src/**` is unchanged; no current formula/config/result/writer owner appears in core/data; no registry source/version/checksum, current science, config 0.2, result 0.3, output writer, CLI, CI, remote data, or root public re-export changed. |
| Clean five-package archive | `cargo +1.96.0 package --locked --workspace` | pass | At clean `a1260445`, core packaged 6 files/10.9 KiB, data 7/41.0 KiB, project 5/23.6 KiB, workflow 5/28.3 KiB, and root 255/1.8 MiB; every archive verification-compiled. Child metadata warnings remain non-fatal. |
| Independent C-01 audit | `c01_identity_audit` read-only design/implementation/final re-review | pass after findings applied | Added TMA-safe biological subsamples, role-kind enforcement, unit/kind repeated-set uniqueness, missing/reused/deep boundary tests, and bounded nested biological ancestry. Final review found no material issue; reviewer edited no files and used no remote service or LSP. |

## C-02 coordinate substrate evidence

Implementation commit: `c676cfd732d02d6202bff8cea47fcf74b5bfd8e7`.

| Gate | Exact command | Status | Result/evidence |
|---|---|---|---|
| Behavior-first coordinate API | `cargo +1.96.0 test --locked --test coordinate_frames` | red in three waves, then pass | Missing IDs/frames, then transform/registry, then serial-section APIs each failed to compile before implementation. Final suite: 10 passed. It covers exact typed-ID bounds, axes/units/spaces, finite coordinates/matrices, explicit 2-D/3-D chains, declaration errors and precedence, target-frame uncertainty, iterative deep cycles, serial state/order/cross-references, gaps/distortion, and axis-permuted embedding. |
| Duplicate/reference precedence regression | focused declaration-order test | red, then pass | Audit case `[uncertainty -> missing frame, duplicate uncertainty]` initially returned the missing reference. Frame, uncertainty, and transform ID indexing now completes before reference validation; same-kind and cross-kind competition tests pass. |
| Affected package tests | `cargo +1.96.0 test --locked -p marklab-core -p marklab-data` | pass | Core 1/1 and data 1/1 unit tests passed; both doc-test binaries passed with 0 executable tests. |
| Compatibility contracts | `cargo +1.96.0 test --locked --test api_contract --test result_v03 --test workspace_contract` | pass | 21 passed: API 6, result 13, workspace 2. Current registration, config/result, and workspace layers remain unchanged. |
| Full all-feature workspace tests | `cargo +1.96.0 nextest run --locked --workspace --all-features` | pass | 440/440 passed, 22 skipped, 1 existing slow synthetic smoke; 63.613 s summary. |
| Public documentation | `RUSTDOCFLAGS='-D missing-docs' cargo +1.96.0 doc --locked --no-deps -p marklab-core -p marklab-data` | pass | Every new public coordinate and serial-section symbol documents successfully. |
| All-feature and CLI-only Clippy | both contract commands with `--workspace --all-targets` and `-D warnings` | pass | All-feature and no-default/CLI configurations exited 0. |
| Fuzz build | `cargo +nightly fuzz check` | pass | Root and standalone fuzz dependency graph compiled in release fuzz mode. |
| Format/scope | `cargo +1.96.0 fmt --all --check`; `git diff --check`; staged/final scope review | pass | Only core/data coordinate sources, focused exports, the integration test, and implementation docs changed. No manifest/lock/current root source changed. |
| Exact clean workspace package | `cargo +1.96.0 package --locked --workspace` | blocked | All five archives were created; verification exited 101 when normalized data resolved published `marklab-core 0.1.0` without C-02 IDs. The exact gate is not claimed green and remains release-blocking under DEC-0018. |
| Supplemental local package verification | exact workspace command plus ephemeral CLI `patch.crates-io` paths for core/data/project/workflow | pass | All five archives and verification builds completed against the current local dependency graph. A data-only run with a local core patch also passed. No tracked patch, manifest, lock, CI, or registry state changed. |
| Independent C-02 audit | `c02_coordinate_audit` design/implementation/final read-only review | pass after findings applied | Replaced dishonest axis-only serial embedding before freeze; later fixed validation-phase ordering and coverage. Final recheck found both findings resolved and no other correctness, complexity, placement, scope, or API drift issue. Reviewer edited no files and used no remote service. |

## C-03 artifact-catalog contract evidence

| Gate | Exact command | Status | Result/evidence |
|---|---|---|---|
| Independent ownership/security contract audit | `c03_artifact_audit` read-only owner/contract review | pass after findings applied | The frozen boundary now requires store verification before cache lookup, hard-link no-replace plus directory sync, iterative hostile-cycle checks, deterministic replica union, exact primary-key types, catalog-valid versus available state separation, reserved/file-type recovery controls, privacy-bounded metadata, and an exact legacy cache-key vector. It explicitly does not close DATA-01, FND-07, WS-11, WS-25, WS-C, physical table conformance, or C-01/C-02 codecs. Reviewer edited no files, accessed no remote system, and created no LSP server. |
| Contract scope/format | `git diff --check`; targeted status/diff review | pass | Production, tests, manifests, and locks remain clean at the C-02 closure SHA; only C-03 contract/control-plane documents changed before red tests. |

## C-03 artifact catalog, table manifest, and local-store evidence

Implementation commits: primary `97119cdfec7dcfe9da9375515e3d780991003888`; final locator-boundary regression `488d3bd65b5a9e17c7cc1849700e13d2d8eb771f`.

| Gate | Exact command | Status | Result/evidence |
|---|---|---|---|
| Behavior-first catalog/table API | `cargo +1.96.0 test --locked -p marklab-project --test artifact_catalog` | red compile, then pass | The first focused suite failed on absent schema/ID/table/catalog/location APIs. The final suite covers strict digest text, schema/table/key bounds, exact compatibility, canonical fixed-point JSON, unknown/duplicate fields, identity recomputation, missing/self/cyclic dependencies, deterministic replica union/conflict, 10,000-record graph inputs, hostile tail cycles, 16 MiB bounds, and fixed semantic/artifact/catalog vectors. |
| Query-free locator regression | `cargo +1.96.0 test --locked -p marklab-project --test artifact_catalog locators_and_metadata_reject_reserved_or_privacy_unsafe_values` | red, then pass | Final closure audit found `ArtifactKey` accepted `external/object?token=x` despite the frozen no-query locator contract. The focused test failed for that exact reason before production changed; literal `?` is now rejected and the focused test plus full 32-test project suite pass. |
| Behavior-first capability store | `cargo +1.96.0 test --locked -p marklab-project --test artifact_store` | red compile and regression waves, then pass | The store API was absent at red. Final coverage includes root/key traversal, symlink and non-regular rejection, missing/short/long/same-length-wrong bytes, streaming publication, oversize callbacks, idempotency, conflict/no-overwrite, concurrent publishers, shared-publisher/exclusive-recovery coordination, staging ownership, fault phases, partial quarantine targets, and conservative FIFO/socket/directory/symlink recovery. |
| Behavior-first project/workflow integration | `cargo +1.96.0 test --locked --test project_workflow --test workflow_contract` | red compile, then pass | Final 14 tests include project insertion atomicity, catalog/coordinate slots, the semantic-input store requirement, mandatory verification before cache lookup, zero execution on integrity failure, semantic-ID invalidation, caller-order key framing, and exact empty-semantic-input legacy key `ddadc700530efba19202b9a4e2a6f6f5644c089aeac85f2e2a11442319cfc0b2`. |
| Affected package tests | `cargo +1.96.0 test --locked -p marklab-project`; `cargo +1.96.0 test --locked -p marklab-workflow` | pass | Project: 32 tests passed across unit/catalog/store suites; workflow: 1 unit test passed. Both doc-test binaries contain 0 executable tests. |
| Compatibility contracts | `cargo +1.96.0 test --locked --test api_contract --test result_v03 --test workspace_contract` | pass | 21 passed: API 6, result 13, workspace 2. Result 0.3, current public facade, and dependency layers remain compatible. |
| Full all-feature workspace tests | `cargo +1.96.0 nextest run --locked --workspace --all-features` | pass | Final locator-regression SHA rerun: 471/471 passed, 22 skipped, 1 existing slow synthetic smoke test; 63.039 s summary. |
| Workspace docs/no-default/WSI | `cargo +1.96.0 test --locked --workspace --doc --all-features`; `cargo +1.96.0 check --locked --workspace --no-default-features`; `cargo +1.96.0 test --locked --package marklab --features wsi,cli --test wsi_integration` | pass | Workspace doc tests and no-default compilation pass. WSI integration: 10 passed, 1 existing external public-oracle test ignored. |
| Public documentation | `RUSTDOCFLAGS='-D missing-docs' cargo +1.96.0 doc --locked --no-deps -p marklab-project -p marklab-workflow` | pass | Every new public project/workflow item documents successfully. |
| Warnings-denied Clippy | all-feature and no-default/CLI workspace commands with `--all-targets` and `-D warnings` | pass | Both commands exit 0. |
| Catalog fuzz boundary | `cargo +nightly fuzz check` | red contract, then pass | The manifest contract first rejected the missing target. The final standalone fuzz workspace builds the strict bounded `artifact_catalog` decoder target with all existing targets. |
| Dependency boundary | `cargo audit`; `cargo deny check advisories licenses bans sources`; `cargo machete` | pass with reviewed warnings | Audit reports no vulnerability and only existing allowed unmaintained `encoding`/`paste` warnings. Deny passes all categories with reviewed duplicate-version warnings, including capability-platform transitive versions. Machete finds no unused dependency. |
| Format/scope | `cargo +1.96.0 fmt --all --check`; `git diff --check`; final implementation diff/status review | pass | Changes are confined to the declared project/workflow/artifact/fuzz/test/re-export/lock/documentation scope. Current result/config/CLI/writer/Parquet/science owners and remote content are unchanged. |
| Exact clean workspace package | `cargo +1.96.0 package --locked --workspace` | blocked | All five archives were created; verification exited 101 when normalized data resolved published pre-C-02 `marklab-core 0.1.0`. DEC-0018 remains release-blocking; the exact gate is not claimed green. |
| Supplemental local package verification | exact workspace package command plus ephemeral CLI `patch.crates-io` paths for core/data/project/workflow | pass | Every archive and verification build completed against the current local dependency graph. This is source/archive compatibility evidence, not registry resolvability. |
| Windows target gate | `cargo +1.96.0 check --locked --target x86_64-pc-windows-msvc -p marklab-project` | not run to project compilation | Exit 101 because the target's `core` crate is not installed. Source review found a safe capability-relative writable directory-handle path, but Windows directory durability and crash recovery remain an explicit runtime gate under DEC-0021. |
| Independent C-03 implementation audit | `c03_implementation_audit` read-only implementation/final closure review | pass after findings applied | Fixed staging-guard ownership, publisher/recovery coordination, partial quarantine reporting, lock-namespace aliasing, golden vectors, same-length mutation coverage, and the final query-bearing-key contract mismatch. Final review found no other material correctness, security-boundary, complexity, placement, or scope issue. Reviewer edited no files and did not deserialize remote artifacts. |

## C-04 contract and source-profile evidence

| Gate | Exact command | Status | Result/evidence |
|---|---|---|---|
| Privacy-safe real source profile | `ssh -o BatchMode=yes user@100.82.140.72 python3 - /Volumes/500GB/marklab/raw/schurch-crc-he-cellvit-2020 < docs/implementation/audits/c04_remote_source_profile.py` | pass | Audit program SHA-256 `e1123aa5a2d25dae44bcce3cdb069ce414c2f76fadbfe19d930e01ce05a4e85b`. Aggregate JSON: `{"aggregate_digest":"75335c9aca2ab167a823cfbcae6d6783482b1cffea71903ac03f61b8775113b6","all_finite":true,"all_qc_pass_true":true,"bundles":32,"dimension":1280,"format":"marklab.c04_remote_source_profile_audit","maximum_bundle_rows":3356,"minimum_bundle_rows":48,"npy_versions":{"1.0":32},"rows":60191,"source_rows_exact":true,"version":1}`. The stdlib-only audit reads NPY/CSV/manifest files, prints no identifiers/paths, and never opens `.pt`, `.pth`, pickle, or checkpoint content. |
| Privacy-safe manifest shape | `ssh -o BatchMode=yes user@100.82.140.72 python3 - /Volumes/500GB/marklab/raw/schurch-crc-he-cellvit-2020 < docs/implementation/audits/c04_remote_manifest_shape.py` | pass | Audit program SHA-256 `ea015f7755d7c4184a35e85b134ac5b478960aadd0e1af3f06a4723b086545a6`. The audit rejects files over 64 KiB before reading, duplicate JSON keys, shape drift, multiple sources, and control characters. All 32 manifests are 1,106–1,108 bytes with one exact 10-key top-level shape and exactly one exact nine-key source shape; every observed JSON type agrees, SHA fields are 64 bytes, and other source strings are 15–174 bytes. Output contains only aggregate key/type/length vocabularies and no identifiers, values, or paths. |
| Rust aggregate-only authorized reconciliation | `rsync --archive --quiet --prune-empty-dirs --include='*/' --include='embeddings.npy' --include='cells.csv' --include='bundle_manifest.json' --exclude='*' user@100.82.140.72:/Volumes/500GB/marklab/raw/schurch-crc-he-cellvit-2020/ /tmp/marklab-c04-authorized.mH1uVl/`; aggregate-only stage validation; `MARKLAB_AUTHORIZED_CELLVIT_ROOT=/tmp/marklab-c04-authorized.mH1uVl cargo +1.96.0 test --locked -p marklab --features csv --test authorized_cellvit_reconciliation -- --ignored --exact reconciles_all_authorized_bundles`; validated marker then `trash /tmp/marklab-c04-authorized.mH1uVl` | pass | Test SHA-256 `3564c6df0142a447332fe9cd43a19ef4653f4f580f3d1e0b821fbb7c90fabbfa`. The unique owned stage contained exactly 96 allowed source files plus its ownership marker, no unexpected names or symlinks, and no `.pt`, `.pth`, pickle, or checkpoint. Rust reconciled exactly 32 bundles, 60,191 finite/QC-pass rows, width 1,280, ledger aggregate `75335c9aca2ab167a823cfbcae6d6783482b1cffea71903ac03f61b8775113b6`, and framed report `e5aeb0a426a7f9f2b38d538c73dd867818a589e3ccbe1b1b955b4273b65996a8`. The stage was moved to Trash after the passing run. |
| Post-review authorized reconciliation rerun | Same allowlisted `rsync` staging profile into `/tmp/marklab-c04-authorized.RzB2KK`; aggregate-only stage validation; `MARKLAB_AUTHORIZED_CELLVIT_ROOT=/tmp/marklab-c04-authorized.RzB2KK cargo +1.96.0 test --locked -p marklab --features csv --test authorized_cellvit_reconciliation -- --ignored --exact reconciles_all_authorized_bundles`; validated marker then `trash /tmp/marklab-c04-authorized.RzB2KK` | pass | Repeated after the final compositional retained-budget and two-phase candidate/graph corrections. The stage again contained exactly 96 allowlisted inputs plus one marker, no unexpected files or symlinks; the exact 32 / 60,191 / 1,280 counts and both `75335c9a…` / `e5aeb0a…` digests passed in 15.70 seconds. The stage was moved to Trash. |
| Checkpoint/source content pins | `ssh -o BatchMode=yes user@100.82.140.72 'shasum -a 256 /Volumes/500GB/marklab/env/models/CellViT-SAM-H-x40-AMP.pth /Users/user/Bench/CellViT-plus-plus/cellvit/models/cell_segmentation/cellvit_sam.py /Users/user/Bench/CellViT-plus-plus/cellvit/inference/postprocessing.py /Users/user/Bench/CellViT-plus-plus/cellvit/inference/output.py'` | pass | Exact SHA-256 values: checkpoint `356418f19d9d478f164c7a31f85274584fefaa02355815c09f52346c658c8ec4`; model `15023c40a1a8ae5f2abce6cb5b5be7ecc5f6add8028b1170b0575e78cff3ce9a`; postprocessing `49b4c5e258589d13019afc630cc0d4a7112f14dec84f81725b9023a94a1645c5`; output `62c0e1687eee8d58498ab4de07ddd036608b08fd36e3e983c7c872fce33a9cfa`. `shasum` streamed checkpoint bytes; no model deserialization occurred. |
| Hash-pinned extraction semantics | `ssh -o BatchMode=yes user@100.82.140.72 'grep -nE "self.embed_dim = 1280|self.extract_layers = \[8, 16, 24, 32\]|out_dict\[.tokens.\] = z4|bb_index =|cell_token = torch.mean" /Users/user/Bench/CellViT-plus-plus/cellvit/models/cell_segmentation/cellvit_sam.py /Users/user/Bench/CellViT-plus-plus/cellvit/inference/postprocessing.py'` | pass | The pinned model source reports SAM-H width 1,280, extraction layers 8/16/24/32, and returned `z4`; pinned postprocessing divides the nucleus bounding box by token patch size and takes the arithmetic mean of intersecting token vectors. Complete run input-normalization/license provenance remains unavailable, so real promotion stays prohibited. |
| Independent semantic/ownership audit | `c04_embedding_audit` iterative read-only contract review | pass after findings applied | Three review rounds resolved source-local-versus-canonical identity, expected/map/context artifacts, status/link semantics, exact digest/provenance/dependency identity, real reconciliation/promotion separation, privacy, schema metadata, deterministic writers, mmap deferral, scale gates, complete root constructor types, uncertainty drift, and explicit-filler wording. Final reviewer response approved freeze with no material blocker; no file or remote mutation occurred. |
| Independent Arrow/Parquet feasibility audit | `c04_feasibility_audit` iterative read-only API/allocation review | pass after findings applied | The contract now uses a retained borrowed verified reader, one validated row group, preflighted Arrow Footer/Message and Parquet compact-Thrift inputs, non-null status-owned filler semantics, `publish_send`, exact feasible writer settings, locked dependencies, and executable memory/benchmark gates. Final reviewer response approved freeze on Rust 1.96/Arrow-Parquet 56.2.1 with no remaining contradiction or unbounded-allocation gap. |
| Contract scope/format | `git diff --check`; targeted status/diff review | pass at the contract-only checkpoint | At the recorded frozen-contract checkpoint, production, tests, manifests, and locks were clean at the C-03 closure SHA; dirty scope was limited to the contract/accepted decisions/ownership/status, this evidence, and the privacy-safe audit program. Subsequent C-04 implementation evidence is recorded below and does not preserve that historical clean-tree condition. |

## C-04 Arrow embedding-table physical evidence

| Gate | Exact command | Status | Result/evidence |
|---|---|---|---|
| Physical-profile decision review | `c04_embedding_audit` and `c04_feasibility_audit` independent read-only DEC-0027/C-04 reviews | pass after findings applied | Reviews froze the `item` IPC child, separate initial-Schema-message verifier, structural rather than byte-replay reader claim, status-derived nullable validity, truthful unlocated fresh draft, Parquet `[PLAIN,RLE]`, 1,024-row internal write batches, encoding statistics/type orders, and dual annotations. The final Arrow and Parquet responses independently approved the revised decision; neither reviewer edited files. |
| Arrow writer/raw preflight behavior first | `cargo +1.96.0 test --locked --offline --test cell_embedding_arrow --features parquet` | red compile, then pass | Initial compile failed on absent columnar APIs. Current suite: 14/14. It covers exact schema/metadata, 64-byte V5 framing, malformed magic/footer/header padding/EOS, absent batch vectors, initial-schema/footer disagreement, negative/misaligned blocks, wrong nodes, aligned-out-of-range/giant/misaligned buffers, staged/exact file/decoded/row-group/retained budgets, empty/1/8,191/8,192/8,193/100,000-row boundaries, complete decoded-buffer and footer/block-vector accounting, two in-process writes, two fresh child-process writes, managed fresh publication/idempotency, and pinned SHA-256 `44e1326b36546c793bb768f67afae9d6f5a6728ed13d47caece7514294b63570`. |
| Provenance-gated Arrow materialization | `cargo +1.96.0 test --locked --offline --test cellvit_embedding_artifact_graph --features parquet verified_graph_gates_full_arrow_table_materialization -- --exact` | red compile, then pass | The red compile lacked the reader API. The passing path requires a verified 13-dependency graph, exact artifact record/manifest/content/dependencies, bounded raw preflight before `FileReaderBuilder`, recursive `validate_full`, exact cell/link/status/value checks, and recomputed logical digest before returning `CellEmbeddingTable`. |
| Arrow focused regression | `cargo +1.96.0 test --locked --offline --test cellvit_embedding_artifact_graph --features parquet`; `cargo +1.96.0 test --locked --offline -p marklab-embeddings --features csv,parquet` | pass | Artifact graph: 13/13. Embedding package: 12/12 across seven unit and five domain-contract tests. The suite covers verified managed-reader parity on a hostile block; binding/integrity precedence; non-finite, negative-zero, hidden-nonzero, malformed-offset, unknown-status, unexpected-cell, status/link, and logical-digest failures. Shared-parser unit cases reject dictionary batches, compression, variadic buffers, oversized/table-heavy Footer and Message inputs before stock decode. |
| Fresh draft and durable publication | `cargo +1.96.0 test --locked --offline -p marklab-project` | red compile, then pass | 41/41 across project unit/catalog/store/verified-IO suites. `ArtifactDraft` has stable location-free identity and cannot enter `ArtifactCatalog`; `publish_new_send` returns only the truthful managed record after exact durable publication, is idempotent, and removes staging/returns no record after integrity failure. Existing `publish` and `publish_send` behavior remains covered. |
| Shared borrowed/managed preflight | `cargo +1.96.0 test --locked --offline --features parquet --test cell_embedding_arrow --test cellvit_embedding_artifact_graph` | pass | 27/27 total. Both borrowed bytes and `with_verified_reader` call the same seek-based Footer/Message/block validator. The managed path holds one descriptor, reads only the bounded footer/schema and one precharged block, then uses stock Arrow only after raw validation; pre/post store verification remains authoritative. A hostile negative block yields the identical `InvalidBlock` error on both paths. |
| Arrow warnings/docs/features | `cargo +1.96.0 clippy --locked --offline -p marklab-embeddings --all-targets --features csv,parquet -- -D warnings`; `cargo +1.96.0 clippy --locked --offline -p marklab --test cell_embedding_arrow --test cellvit_embedding_artifact_graph --features parquet -- -D warnings`; `RUSTDOCFLAGS='-D missing-docs' cargo +1.96.0 doc --locked --offline --no-deps -p marklab-embeddings -p marklab-project`; `cargo +1.96.0 check --locked --offline -p marklab-embeddings --no-default-features`; `cargo +1.96.0 fmt --all --check`; `git diff --check` | pass | Both warnings-denied Clippy scopes, missing-docs builds, feature-free embedding compilation, format check, and diff whitespace check exit 0 on the current tree. |
| Locked dependency feasibility | `cargo +1.96.0 tree --locked --offline -p marklab-embeddings --features parquet -e features`; root lock diff review | pass | This Arrow slice resolves already locked Arrow/IPC 56.2.1 and FlatBuffers 25.12.19. `Cargo.lock` adds only those three direct edges to the local embedding package; no registry package/version/checksum changes. The independent Parquet feasibility audit separately confirmed already locked Parquet 56.2.1, Bytes 1.11.1, and Thrift 0.17.0 for the subsequent implementation slice; they are not added early. |
| Standalone embedding fuzz lock | mechanical TOML comparison of pre-C-04/root/current locks; `cargo +1.96.0 metadata --locked --manifest-path fuzz/Cargo.toml --format-version 1 --no-deps`; `cargo +nightly fuzz check embedding_inputs` | pass after DEC-0028 dual review | No pre-C-04 identity tuple was removed; all 40 new registry tuples are exact root-lock tuples. Six retained dependency lists changed exactly as disclosed by accepted DEC-0028, including `tempfile 3.27.0` re-resolving from retained `getrandom 0.4.3` to root-reviewed `0.3.4`. Locked metadata and the target build exit 0. |
| Bounded embedding fuzz execution | `cargo +nightly fuzz run embedding_inputs /tmp/marklab-c04-fuzz.tJrNgg -- -runs=5000 -max_len=4096`; validated task-local corpus then `trash /tmp/marklab-c04-fuzz.tJrNgg` | pass | 5,000 executions completed without crash at coverage 3,041 / features 4,410, peak RSS 85 MiB. The runtime seed contains one present and one missing row, so structured mutation reaches a real Footer Block, RecordBatch Message, nodes, buffers, status bytes, validity, and component body. The generated working corpus was outside the repository and moved to Trash after the run. |
| Partial-checkpoint claim ceiling | implementation/reviewer scope audit | partial by design | This checkpoint closes only the embedding-table Arrow/store/fuzz slice. Row-link Arrow, all Parquet codecs/preflight/one-row-group decode, streaming scan/QC, differential/property corpus reuse, Criterion/DHAT, the mandatory 10k and 1M scale gates, and full C-04 phase gates remain open. No C-04, EMB-CORE, FND-05, WS-24, or phase-completion claim is made. |

## C-04 Arrow row-link physical evidence

| Gate | Exact command | Status | Result/evidence |
|---|---|---|---|
| Row-link writer/raw preflight behavior first | `cargo test --features parquet --test cell_embedding_row_link_arrow` | red compile, red behavior, then pass | The initial test compile failed on absent row-link Arrow APIs. After the first implementation, the 1-row all-present boundary failed with `InvalidBuffers`, exposing Arrow 56.2.1's synthesized all-ones unused nullable tail; the bounded streaming canonicalizer then made the frozen status-derived bitmap exact. Current suite: 15/15, with pinned mixed-fixture SHA-256 `0edeab689aa350263d3057098648a17c72bbf56066af2ed7e4a3bcab031f58a5`. |
| Canonical nullable and streaming writer | Focused tests `canonical_row_link_writer_uses_exact_batch_boundaries`, `canonical_row_link_writer_preserves_mixed_validity_across_batch_boundary`, and `canonical_row_link_writer_honors_fragmenting_write_sinks` | pass | Exact bitmap bytes and node null counts pass at 1/7/8/9-row tails; empty/1/8,191/8,192/8,193-row counts use exact 8,192-row batches; mixed validity crosses that boundary; and irregular 1–11-byte short writes reproduce byte-identical output, digest, length, and successful preflight. Two in-process and two fresh-child writes are deterministic. |
| Row-link hostile preflight and privacy | Focused row-link integration suite under the command above | pass | Regressions reject malformed magic/header padding/footer length/EOS, absent empty batch vectors, dictionary Footer blocks, Footer custom metadata, negative/misaligned blocks, wrong nodes/null counts, misaligned/giant buffers, initial-Schema/footer disagreement, application-metadata drift, exact nullable-bitmap drift including unused bits, noncanonical hidden null fillers, wrong source rows, and wrong cell bytes. Error strings do not disclose injected metadata or cell sentinels. Shared embedding-parser unit tests separately cover compression, variadic counts, oversized/table-heavy Footer and Message declarations; direct row-link reuse remains required before C-04 closure. |
| Exact row-link resource accounting | Focused tests `row_link_arrow_writer_and_preflight_enforce_exact_resource_edges` and `row_link_arrow_writer_charges_many_batch_retained_peak_at_exact_edge` | pass | Exact-pass and one-byte-short edges cover file bytes, decoded buffers, physical record block, Footer-stage retained bytes, block-stage retained bytes, writer peak, and the 100,000-row block-vector/Footer peak. No stock decoder receives bytes before bounded raw preflight. |
| Record/store/publication validation | Focused tests `canonical_row_link_arrow_writer_preflight_and_publication_are_exact`, `row_link_arrow_reader_rejects_every_record_binding_surface_without_leaking_values`, `row_link_arrow_borrowed_and_managed_readers_reject_the_same_hostile_block`, and `row_link_arrow_managed_reader_prioritizes_store_integrity_failure` | pass | Fresh publication returns one truthful managed locator, verifies exact bytes, and returns `AlreadyPresent` with the identical record on replay. Kind, digest, length, schema/version, manifest format/encoding/count/nullability/key, dependencies, and semantic metadata are exact-bound. Borrowed and managed readers return the same `InvalidBlock` for a self-consistent hostile object; store integrity failure takes precedence over the callback. |
| Focused and neighboring regressions | `cargo test -p marklab-embeddings --all-features`; `cargo test --features parquet --test cell_embedding_arrow`; `cargo test --features parquet --test cellvit_embedding_artifact_graph`; `cargo test --features parquet --test cell_embedding_row_link_arrow` | pass | Embedding package 12/12, embedding-table Arrow 14/14, graph/materialization 13/13, and row-link Arrow 15/15 all exit 0 on the current tree. |
| Row-link warnings/docs/features | `cargo clippy -p marklab-embeddings --all-targets --all-features -- -D warnings`; `cargo clippy --all-targets --features parquet --test cell_embedding_row_link_arrow -- -D warnings`; `cargo check -p marklab-embeddings --no-default-features`; `RUSTDOCFLAGS='-D warnings' cargo doc -p marklab-embeddings --features parquet --no-deps`; `cargo +1.96.0 check --manifest-path fuzz/Cargo.toml --bin embedding_inputs --locked --offline`; `cargo fmt --all -- --check`; `cargo fmt --manifest-path fuzz/Cargo.toml -- --check`; `git diff --check` | pass | Both warnings-denied Clippy scopes, feature-free compilation, warnings-denied public docs, locked/offline fuzz compilation, both format scopes, and diff whitespace validation exit 0. |
| Dual-seed bounded fuzz execution | `cargo +nightly fuzz run embedding_inputs /tmp/marklab-c04-row-link-fuzz.GYpjcj -- -runs=5000 -max_len=4096`; then `trash /tmp/marklab-c04-row-link-fuzz.GYpjcj` | pass | 5,000 executions completed without crash at coverage 3,831 / features 6,127, peak RSS 93 MiB. The target now selects both the prior embedding table and a real mixed present/missing row-link Arrow seed, with raw-byte and structured canonical-file mutation paths. The corpus lived outside the repository and was moved to Trash; no fuzz artifact was created. |
| Independent row-link checkpoint audits | `c04_embedding_audit` and `c04_feasibility_audit` second read-only reviews | pass after test/evidence findings applied | Both reviewers first rejected the four-test happy-path slice. After exact nullable corruption, short-write, resource, record/privacy, managed parity, idempotency, mixed-boundary, and fuzz evidence was added, both found no production correctness, allocation, decode-order, publication, or API blocker. They require direct compression/variadic/table-heavy row-link case reuse before C-04 closure but approve this bounded checkpoint. Neither edited files. |
| Partial-checkpoint claim ceiling | implementation/reviewer scope audit | partial by design | This checkpoint closes only the row-link Arrow/store/fuzz slice. All Parquet codecs/preflight/one-row-group decode, streaming scan/QC, shared differential/property corpus reuse, direct row-link compression/variadic/table-heavy Message regressions, Criterion/DHAT, mandatory 10k and 1M scale gates, and full C-04 phase gates remain open. No C-04, EMB-CORE, FND-05, WS-24, or phase-completion claim is made. |

## C-04 canonical Parquet physical evidence

| Gate | Exact command | Status | Result/evidence |
|---|---|---|---|
| Canonical table and row-link profiles | `cargo +1.96.0 test --locked --offline --features parquet --test cell_embedding_parquet --test cell_embedding_row_link_parquet` | red compile, then pass | Behavior-first tests initially failed to compile because the Parquet APIs were absent. Current suites pass 10/10 table and 11/11 row-link cases. Exact Parquet 2.0/data-page-v2, uncompressed PLAIN/RLE declarations, schemas, annotations, sorted metadata, `created_by`, 8,192-row groups/public batches, 1,024 internal write/page-row properties, and disabled dictionary/statistics/index/bloom/Arrow-schema surfaces are frozen. Pinned SHA-256 values are table `7a54901c82e241e467c04cc6bd58a4c1ae88f27171656d7db01aeb0a499e9b88` and row link `82729ed1e8853c51578286e059a4d35028c658785cc87e525016ed97049dafa2`; the 8,192-row pins are `1a8c42b5d65d70fdbc40b0d44ca748a982e929b90956347d17fd0c25a5ea6a27` and `9af303ea0a54f5e993b678c9e81dd7dee0fc9fd852bd6926a84e09962be3dc52`. |
| Raw-before-stock bounded preflight | `cargo +1.96.0 test --locked --offline -p marklab-embeddings --features parquet` | pass | Fourteen Parquet/Arrow unit cases plus five domain cases pass. Marklab's compact-Thrift protocol rejects nonminimal/truncated/overflowing varints, giant strings/collections, depth/field/aggregate excess, duplicates, and skips before reserve. Footer and every page header are canonically replayed; exact schema, metadata, ranges, encodings, sizes, levels, values, row groups, and hard page/footer/resource ceilings are checked before Arrow-rs stock metadata or page decoding. Larger physical row-link IDs are rejected against the supplied logical link at the exact decoded budget; one byte less returns the exact decoded-budget error first. |
| One-row-group stable decode decision | `DEC-0029`; package `row_group_window_rejects_every_out_of_range_translation`; 8,193-row integration boundaries | pass for this partial checkpoint | Parquet 56.2.1 exposes the previously named low-level `RowGroups` path only behind its broad experimental feature. After raw validation, the accepted stable adapter moves one cached metadata tree into `ArrowReaderMetadata`, selects exactly one row-group index, and decodes only a copied validated contiguous group through `RowGroupWindow`. Direct tests reject before-base, end-crossing, arithmetic-overflow, and after-end requests while accepting exact empty-at-end and valid translated ranges. A dedicated three-group selected-middle confinement regression remains mandatory before C-04 closure. |
| Exact memory and budget accounting | Focused exact-edge tests in both Parquet integration suites and artifact-graph suite | pass | Writer and preflight use the same semantic decoded-byte formula. Exact-pass and one-byte-short gates cover file, decoded, row-group-local peak, and retained memory. Accounting includes public input batches, status builder capacity, FixedSizeList level workspaces, page assembly/header/vector capacities, retained chunks, metadata, one copied row group, Arrow output, and final table storage. Retained enforcement precedes supplied-schema and stock-reader construction. |
| Determinism, partition, and logical parity | Fresh-child, fragmenting-sink, 0/1/8,191/8,192/8,193 boundary tests; 64-case Proptest cases per physical table | pass | Two in-process writes and two fresh child processes produce identical bytes and pins. Irregular 1/3/2/7/5/11/4-byte sinks at 0/1/8,193 rows preserve bytes/digests. Arrow and Parquet preserve identical declared rows, statuses, nullable link rows, logical digest, and QC shape across property-generated cases; encoded bytes are not claimed equal across formats. |
| Store, record, and hostile path parity | `cargo +1.96.0 test --locked --offline --features parquet --test cellvit_embedding_artifact_graph`; focused row-link borrowed/managed hostile test | pass | Artifact graph suite passes 16/16. Both physical tables require exact record kind/digest/length/schema/manifest/dependencies plus verified graph/link inputs. Fresh two-pass publication is truthful and idempotent. Borrowed paths hash the supplied bytes; managed paths rely on pre/post store integrity. Digest-matching malformed table and row-link page headers produce the same closed callback error on borrowed and managed paths. Full decode rejects cell/source-row/status/filler/finiteness/logical-digest drift. |
| Dependency and feature closure | root/fuzz lock tuple comparison; `cargo +1.96.0 tree --locked --offline -p marklab-embeddings --features parquet -e features`; `cargo +1.96.0 check --manifest-path fuzz/Cargo.toml --bin embedding_inputs --locked --offline` | pass after correction | The root lock adds only Bytes 1.11.1, Parquet 56.2.1, and Thrift 0.17.0 direct edges to the local embedding package. The standalone fuzz lock adds only exact root-reviewed registry tuples; an audit caught Cargo's initial `twox-hash 2.1.3` selection and it was pinned to the root's `2.1.2` before acceptance. Parquet enables only `arrow`; `experimental`/variant support is absent. Locked offline fuzz compilation exits 0. |
| Seven-route structured fuzz execution | Post-fix `cargo +nightly fuzz run embedding_inputs /tmp/marklab-c04-parquet-postfix-fuzz.YAcyjm -- -runs=5000 -max_len=4096`; then `trash /tmp/marklab-c04-parquet-postfix-fuzz.YAcyjm` | pass | After the raw-ID and lock corrections, 5,000 executions completed without crash at coverage 9,369 / features 13,956 and peak RSS 127 MiB. The target routes raw or structured mutation through NPY, CSV, provenance, table Arrow, row-link Arrow, table Parquet, and row-link Parquet seeds; the Parquet routes reached Marklab compact-Thrift footer/page preflight. The task-local corpus was moved to Trash and no crash artifact was created. |
| Warnings, docs, features, and neighboring regressions | `cargo +1.96.0 clippy --locked --offline -p marklab-embeddings --all-targets --features parquet -- -D warnings`; focused root warnings-denied Clippy; no-default package tests; missing-docs/warnings rustdoc; both format scopes; `git diff --check`; Arrow table/row-link and project package tests | pass | Embedding and focused root Clippy scopes are warning-free; feature-free package tests pass 7/7; missing-docs/warnings documentation builds pass; both format scopes and diff whitespace pass. Arrow table 14/14, Arrow row link 15/15, and project package 41/41 remain green. |
| Independent Parquet checkpoint audits | `c04_embedding_audit` and `c04_feasibility_audit` iterative read-only reviews | pass after findings applied | Reviews first rejected forged borrowed digest handling, inconsistent decoded accounting, precharge ordering, batch-size conflation, status/row-group undercharging, missing hostile parity, raw row-link ID trust, and an out-of-policy fuzz lock tuple. Each finding received a focused correction/regression. Final reviews found no remaining code, allocation, parser-order, store/publication, dependency, or checkpoint test blocker; neither reviewer edited files. |
| Partial-checkpoint claim ceiling | implementation/reviewer scope audit | partial by design | This checkpoint closes only canonical Parquet table/row-link writing, raw preflight, one-row-group materialization/validation, publication, parity/property cases, and structured fuzz routing. Streaming scan/QC, shared in-memory/NPY/CSV/Arrow/Parquet declarative cases, direct row-link compression/variadic/table-heavy Message cases, selected-middle confinement, Criterion/DHAT, mandatory 10k/1M scale gates, and full C-04 phase gates remain open. No C-04, EMB-CORE, FND-05, WS-24, or phase-completion claim is made. |

## Harness failures

- 2026-08-22: the first combined WS-A path-check wrapper exited 127 at its final `git` calls because the loop variable `path` shadowed zsh's special `path`/`PATH` array. This did not exercise or fail a repository gate. The wrapper was corrected to use `required_doc`; all assertions and `git diff --check` then exited 0.
- 2026-08-22: the first `cargo package --locked` exited 101 solely because the required WS-A bootstrap was uncommitted. It was not weakened with `--allow-dirty`. The exact command passed after commit `fc986c0c4e06216cc85d55d2315482a0107bf5b7`.
- 2026-08-22: B-01's first Cargo-observed integration test exited 101 for the intended reason: the root manifest had no workspace. Intermediate red runs caught a missing fuzz exclusion, premature project/workflow members, and explicit root membership. The direct fuzz metadata command also exited 101 under explicit root membership. These were test-driven design failures, not ignored gates; final forms pass and neither lockfile changed.
- 2026-08-22: one intermediate `workspace_contract` run invoked `env!("CARGO")` with a rustup `+1.96.0` argument. The direct toolchain Cargo executable correctly rejected that rustup-only selector. The test now invokes the exact Cargo executable without a selector; the outer test command remains pinned to 1.96.0 and passes.
- 2026-08-22: default-feature and WSI-feature CLI suites were initially launched concurrently into the same target directory. The default-only `slide_commands_are_absent_without_wsi_feature` assertion then observed the WSI-enabled `target/debug/marklab` and failed 1 of 17 CLI tests. This was a shared-binary harness race, not a product failure: the isolated default rerun passed 65/65 across all B-02 default suites, and the isolated WSI rerun passed 26/26 with one external oracle ignored. Feature-distinct `assert_cmd::cargo_bin` suites are now run serially.
- 2026-08-22: B-03's first library-source CLI-gating assertion expected two root gates but found three because `synthetic_smoke` is also a legitimate CLI adapter. Recursive scanning then found two additional test-module files that use CLI in compound test cfgs. The final test freezes all nine exact current adapter/test files and recursively rejects additions elsewhere; these were policy-test characterization corrections, not product failures.
- 2026-08-22: B-04 test refinement exposed two intended contract corrections before green: the fake artifact kind initially contained a space forbidden by the visible-ASCII/no-whitespace kind syntax, and identical fake outputs deduplicated to one content-addressed artifact rather than two. Assertions were corrected to the frozen artifact contract; no production behavior was weakened.
- 2026-08-22: B-04's first cache miss/hit parity attempt returned the raw execution value on a miss and a result-0.3 decoded value on a hit; tiny timing-number representation differences failed equality. Returning the exact validated committed representation fixed parity. The later byte-level regression then showed that one additional canonical fixed-point round was required so emitted result bytes and the committed digest also agree.
- 2026-08-22: the first WS-B workspace benchmark exit run measured all eight Criterion workloads successfully but then exited 101 when Cargo ran `marklab-project`'s implicit library benchmark harness with Criterion's `--quick` argument. A new manifest contract failed first on the missing child `[lib]` table. Both child crates now set `bench = false`; commit `ac7da28` contains that focused fix, and the exact workspace command then exited 0.
- 2026-08-22: C-01's root metadata-only fuzz query succeeded despite the independent fuzz lock being stale. The real `cargo +1.96.0 check --locked --manifest-path fuzz/Cargo.toml` correctly exited 101. Offline regeneration added only the expected local `marklab-core`/`marklab-data` records and project edge; locked compilation and nightly fuzz then passed.
- 2026-08-22: the first C-01 role-matrix test compile exited 101 because the new typed role category/error did not exist; the first nested-biological-level run then failed with `ObservationDoesNotResolve`, and the membership-query refinement compile failed on the absent method. These were intended behavior-first failures. Final focused and full suites pass without weakening assertions.
- 2026-08-22: C-02's clean unpatched `cargo +1.96.0 package --locked --workspace` created all five archives and then exited 101 because Cargo removed the data crate's core path and resolved the published pre-C-02 `marklab-core 0.1.0`. An ephemeral CLI-patched verification passed for every archive. DEC-0018 keeps the exact registry-resolution failure visible until an authorized version and dependency-ordered publication boundary; no tracked release configuration was weakened.
- 2026-08-23: the first row-link boundary run failed only at one all-present row with `InvalidBuffers`. The regression exposed Arrow 56.2.1 replacing an all-valid nullable buffer with a synthesized byte whose unused bits are ones. A bounded streaming tail rewrite preserves the frozen status-derived bitmap; 1/7/8/9 and mixed multi-batch cases now pass, including fragmenting sinks.
- 2026-08-23: an attempted direct variadic-count hostile fixture assumed enough in-place FlatBuffer vtable padding and panicked in the test helper before calling product code. The unsupported mutation helper was removed rather than weakening its assertion. Shared bounded-parser unit tests still cover valid variadic/compression/table-heavy declarations, and direct row-link reuse remains explicitly open before C-04 closure.
- 2026-08-23: the first warnings-denied root row-link Clippy run failed only on `vec_init_then_push` in the new record-binding test. Initializing the fixed prefix with `vec![...]` resolved it; the exact warnings-denied rerun exits 0.
- 2026-08-23: the first row-link raw-ID regression failed because same-length hostile physical cell bytes passed preflight and were rejected only during stock materialization. Pagewise allocation-free PLAIN comparison now rejects them before stock decode; a canonically reframed larger-payload exact/one-short case and borrowed/managed hostile parity both pass.
- 2026-08-23: Cargo's first standalone Parquet fuzz-lock resolution selected `twox-hash 2.1.3`, which was not present in the root lock and violated DEC-0028. The independent audit caught it before commit; `cargo update --manifest-path fuzz/Cargo.toml -p twox-hash --precise 2.1.2 --offline` pinned the exact root-reviewed tuple, and locked builds/tuple checks were rerun.

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

## C-04 streaming scan, QC, and verified compact-artifact evidence

| Gate | Exact command | Status | Result/evidence |
|---|---|---|---|
| Status-safe blocks and shared QC owner | `cargo +1.96.0 test --locked --offline --test cellvit_embedding_table`; `cargo +1.96.0 test --locked --offline -p marklab-embeddings --features csv,parquet` | red, then pass | The first aggregate-`Debug` assertion failed because derived output exposed raw `values`; custom table/block formatting now contains only shape, QC, and artifact identities. Six table tests and 25 package tests pass. One constant-state accumulator now owns constructor, arbitrary nonzero block-partition, Arrow, and Parquet QC/logical digest semantics. A 64-case independently framed oracle covers statuses, signed zero, values, and partitions. |
| Verified compact artifact and dimension binding | Focused `compact_embedding_artifact_binds_records_shape_dtype_digest_and_qc`; `verified_graph_rejects_embedding_dimension_drift_from_provenance`; package `candidate_finalization_requires_the_exact_verified_graph_binding` | red compile/behavior, then pass | The initial record-plus-table constructor was rejected by review because declarations could bind unrelated physical content. Private verified table/row-link receipts now arise only after full physical validation and must cross-bind exact embedding, row-link, expected-set, provenance, row-link digest, shape, QC, and verified graph identities. A valid one-dimensional physical table initially passed a 1,280-dimensional provenance graph; the graph now carries `output_dimension`, and source finalization plus both readers reject drift. Arrow and Parquet embedding receipts share the same logical compact artifact semantics while retaining distinct physical IDs. |
| Physical streaming and retained separation | `cargo +1.96.0 test --locked --offline --features csv,parquet --test cellvit_embedding_artifact_graph` | red, then pass | Twenty-five artifact-graph tests pass. Borrowed and managed Arrow/Parquet scans validate raw-before-stock, return identical `EmbeddingQcSummary`, and retain no full table. The 8,193 × 1,280 two-batch/two-row-group case converges to each exact scan limit and proves materialization needs strictly more retained bytes. Arrow table materialization and row-link validation initially omitted their coexisting stock-decoded batch; focused exact/one-short regressions failed first, and both paths now checked-add the maximum decoded batch before stock decode. |
| Shared declarative, source, and generated semantics | Focused `every_embedding_status_has_identical_domain_arrow_and_parquet_qc`; `all_present_npy_csv_domain_arrow_and_parquet_paths_share_exact_semantics`; `property_generated_domain_arrow_and_parquet_rows_are_semantically_identical`; `cargo +1.96.0 test --locked --offline --features csv --test cellvit_source_import` | pass | All four statuses produce exact `1/1/1/1` counts and one format-independent digest. A full synthetic 2 × 1,280 NPY/CSV import is checked bit-for-bit, finalized through the graph, and reproduced by materialized/block, Arrow read/scan, and Parquet read/scan paths. Twelve generated mixed-status/value cases traverse both physical formats; five source-import cases compare complete values, summaries, row links, and borrowed/chunked/managed paths. |
| Direct hostile and row-group confinement | Package direct `row_link_arrow_hostile`; row-link Parquet unit/integration suites; selected-middle package test | pass | Direct public row-link Arrow preflight rejects compression, variadic buffers, and table-heavy Messages. A first in-place mutation harness lacked enough canonical padding; the final helper rebuilds aligned metadata and patches the Footer block without weakening assertions. Row-link Parquet directly rejects schema, metadata, codec, dictionary encoding/page, and index drift with the matching closed reason; chunk-confined numeric mutations reject source-cell and non-null source-embedding drift. The checked virtual Parquet reader decodes only the selected middle group of a three-group file. |
| Managed parity, integrity, and redaction | Focused borrowed/managed hostile scan tests; `managed_embedding_scans_report_store_integrity_before_columnar_callbacks`; row-link Arrow/Parquet integrity tests | pass | Borrowed and managed materialized/scan paths return identical closed Arrow/Parquet callback reasons for self-consistent hostile objects. Pre-read store content-integrity failure takes precedence for table scans and both row-link formats. Aggregate-only debug/error surfaces contain no physical component, private filler, raw source value, or hostile token. |
| Focused regression suites | `cargo +1.96.0 test --locked --offline --features csv,parquet --test cellvit_embedding_table --test cellvit_embedding_artifact_graph --test cell_embedding_arrow --test cell_embedding_parquet --test cell_embedding_row_link_parquet --test cellvit_source_import`; `cargo +1.96.0 test --locked --offline --features parquet --test cell_embedding_row_link_arrow` | pass | Arrow table 14/14, Parquet table 10/10, row-link Parquet 12/12, artifact graph 25/25, table 6/6, source import 5/5, and row-link Arrow 15/15 pass. Fresh-process determinism subprocesses also exit 0. |
| Features, warnings, docs, and formatting | `cargo +1.96.0 check --locked --offline --workspace --all-targets --no-default-features --features csv`; same with `--features parquet`; package/focused root Clippy with `-D warnings`; missing-docs rustdoc; `cargo +1.96.0 fmt --all -- --check`; `git diff --check` | pass with documented pre-existing narrow warnings | Both exact narrow all-target checks exit 0 after the graph target was gated on Parquet and only its source differential was gated on CSV. They emit only the already documented WS-B narrow test/library warnings. Package and changed-root Clippy scopes are warning-free; missing-docs rustdoc, format, and diff whitespace checks exit 0. |
| Structured fuzz | `cargo +1.96.0 check --manifest-path fuzz/Cargo.toml --bin embedding_inputs --locked --offline`; `cargo +nightly fuzz run embedding_inputs /tmp/marklab-c04-scan-fuzz.b1ym5n -- -runs=5000 -max_len=4096`; `trash /tmp/marklab-c04-scan-fuzz.b1ym5n` | pass | Locked standalone compilation exits 0. Five thousand seven-route executions complete without crash at coverage 9,528 / features 14,312 and peak RSS 137 MiB. The task-owned empty corpus lived outside the repository and was moved to Trash; no crash artifact was reported. |
| Independent final audits | `c04_embedding_audit`; `c04_feasibility_audit`; targeted re-audit after corrections | pass after findings applied | Read-only reviewers first found forgeable compact binding, filler `Debug`, missing cross-path/resource/integrity evidence, Arrow table and row-link decoded-batch undercharging, missing graph dimension binding, and narrow-feature compile failures. Each received a focused correction/regression. Final reviews report no blocker/high finding and approve this bounded checkpoint before benchmark/scale gates; neither reviewer edited files. |
| Partial-checkpoint claim ceiling | implementation/reviewer scope audit | partial by design | This checkpoint closes the bounded block/QC/scan/receipt/differential/direct-hostile/selected-group slice only. Criterion, DHAT, mandatory 10,000 × 1,280 and 1,000,000 × 256 executions, authorized 32-bundle Rust reconciliation, and full C-04 phase gates remain open. No C-04, EMB-CORE, FND-05, WS-24, or phase-completion claim is made. |

Additional red/green and harness notes:

- The first direct row-link Arrow hostile rewrite tried to fit a larger FlatBuffer into 248 bytes of canonical padding; the 264-byte payload failed in the test helper before product code. Rebuilding the aligned metadata block and patching the exact Footer `Block.metadataLength` produced a valid hostile container; all three production preflight assertions pass.
- One attempted Cargo invocation supplied two exact integration-test filter arguments; Cargo rejected the second argument before compiling tests. The two exact tests were rerun as separate pinned commands and both passed. This was a command-shape error, not a repository failure.
- Independent review caught the CSV-only/Parquet-only graph-target resolution failures and row-link Arrow decoded-batch accounting before commit. Both exact feature commands now exit 0, and the row-link full-reader budget regression records the intended initial failure followed by exact-pass/one-short green evidence.

## C-04 deterministic benchmark, DHAT, and scale evidence

| Gate | Exact command | Status | Result/evidence |
|---|---|---|---|
| Behavior-first heap contract | `cargo +1.96.0 test --locked --features csv,parquet,dhat-heap --test cellvit_embedding_heap -- --exact embedding_10k_1280_peak` | red, then pass | The first compile failed because the shared benchmark workload did not exist. The first implemented run then failed before profiling with `RowGroupByteBudgetExceeded { required: 524209160, maximum: 268435456 }`; the Parquet row-group category alone was corrected to 512 MiB. Final review found that physical publication was still outside profiling, so a second compile-red contract required fresh disk-backed Arrow/Parquet writes inside the measured path. The final exact command passes in 137.51 s. A diagnostic `--nocapture` run reported zero current tracked bytes, 324,194,945 peak bytes, and a 603,979,776-byte cap. |
| Immutable outcome calibration | exact smoke/full benchmark profiles against initially unset digests | red, then pinned | The first checked iterations intentionally failed and emitted the complete outcomes. Reviewed constants now bind row count, dimension, canonical logical digest, and numeric digest. Smoke pins `df74ee3588f3c5dbf4fa81ffc285dd9f84daf8bb1101e7294fba6536785931de` / `0396c2d78ff98c7307e7dcf383d8579cf1a06c2501f9fc67c91a285308de08b4`; full pins `d5dc753238330e9be60fdee94c6c24d7151f26660527689b15e8895e983c5633` / `133dc720d9775d8d2a3c4140a36529a750ab844acd6970eb6ca2ff86d8e7d49a`. DEC-0031 freezes generation, all 4,096 random accesses, arithmetic, and framing. |
| Mandatory 10,000 × 1,280 smoke | `cargo +1.96.0 bench --locked --bench cell_embeddings --features csv,parquet -- '10k_x_1280' --noplot` | pass | The filter selected exactly 10,000 rows × 1,280 components (12,800,000 values). Ten correctness-checked flat samples took 4.1653–4.1803 s, or 3.0619–3.0730 million values/s. Each iteration performs source import/finalization, sequential domain/QC work, bounded Arrow/Parquet scans, physical round trips, and exact logical/numeric parity. Criterion used Plotters because Gnuplot is unavailable. |
| Mandatory 1,000,000 × 256 scale | prebuild: `cargo +1.96.0 bench --locked --bench cell_embeddings --features csv,parquet --no-run`; runtime: `/usr/bin/time -l cargo +1.96.0 bench --locked --bench cell_embeddings --features csv,parquet -- '1m_x_256' --noplot` | pass | The timed command reported no compilation and selected exactly 1,000,000 rows × 256 components (256,000,000 values). Ten checked samples took 10.446–10.488 s, or 24.408–24.506 million values/s; command wall time was 126.19 s. Maximum RSS was 1,117,552,640 bytes, 41.6% of the 2.5-GiB threshold. It executed sequential scan/QC, ordered mean/sum-of-squares/norm, all 4,096 random row accesses, 16 × 16 population covariance, and a 64 × 64 linear-kernel block. |
| Runtime/build RSS distinction | cold calibration followed by current-binary calibration and exact current-binary closure run | characterized | A cold thin-LTO rebuild inside `/usr/bin/time` reached 4,904,665,088 bytes because compiler/linker work shared the command tree; the immediately repeated current-binary workload reached 1,098,268,672 bytes. DEC-0031 therefore requires the locked no-run prebuild outside the timed runtime gate. Cold build memory remains disclosed and is not relabeled embedding-table retention. |
| Focused warnings and independent review | `cargo +1.96.0 clippy --locked --package marklab --bench cell_embeddings --features csv,parquet -- -D warnings`; same package/test target with `--features csv,parquet,dhat-heap`; `c04_embedding_audit`; `c04_feasibility_audit` | pass after findings applied | Review first rejected an under-budget Parquet row group, self-derived expectations, an undocumented random-access reduction, one large enum variant, a missing Criterion target, and ambiguous RSS ownership. Both prepared fixtures are boxed outside timing; outcome goldens are immutable; DEC-0031 closes arithmetic/RSS semantics; both focused Clippy commands exit 0. Final reviewers report no remaining code or resource blocker. |
| Claim ceiling | implementation/reviewer scope audit | partial by design | This checkpoint closes Criterion, DHAT, and the mandatory synthetic 10k/1M scale evidence. C-04 remains open until the authorized 32-bundle Rust reconciliation and complete phase-boundary gates pass. No real-corpus promotion, embedding-science, C-04, EMB-CORE, FND-05, WS-24, or phase-completion claim is made here. |

Additional benchmark harness notes:

- The first full calibration was deliberately checked against an unset golden and exited 101 after one workload iteration; it was identity capture, not an accepted benchmark pass.
- One warnings-denied no-default `csv,parquet,dhat-heap` all-target command exposed only the already documented narrow root-library warnings. The two changed targets pass their exact warnings-denied Clippy commands; no unrelated warning suppression or product edit was made.

## C-04 final authorized-corpus reconciliation rerun

| Gate | Exact command | Status | Result/evidence |
|---|---|---|---|
| Read-only stage construction | allowlisted `rsync` from the authorized remote root into `/tmp/marklab-c04-authorized.g8Z0iv`; marker and aggregate-only local validation | pass | The existing authenticated SSH connection succeeded without password input. The owned stage contained exactly 32 `embeddings.npy`, 32 `cells.csv`, 32 `bundle_manifest.json`, and one ownership marker; zero symlinks, unexpected files, or incomplete bundles were present. Write permissions were removed before the Rust test. No descendant identifier, source value, model, checkpoint, `.pt`, `.pth`, or pickle entered output or the repository. |
| Exact Rust reconciliation | `MARKLAB_AUTHORIZED_CELLVIT_ROOT=/tmp/marklab-c04-authorized.g8Z0iv cargo +1.96.0 test --locked -p marklab --features csv --test authorized_cellvit_reconciliation -- --ignored --exact reconciles_all_authorized_bundles` | pass | Test SHA-256 `3564c6df0142a447332fe9cd43a19ef4653f4f580f3d1e0b821fbb7c90fabbfa`. The test passed in 16.47 s and reproduced exactly 32 bundles, 60,191 finite/QC-pass rows, width 1,280, aggregate digest `75335c9aca2ab167a823cfbcae6d6783482b1cffea71903ac03f61b8775113b6`, and reconciliation digest `e5aeb0a426a7f9f2b38d538c73dd867818a589e3ccbe1b1b955b4273b65996a8`. The report remains aggregate-only and non-promotable with four explicit missing provenance fields. |
| Owned-stage cleanup | marker/count/symlink revalidation; exact `trash /tmp/marklab-c04-authorized.g8Z0iv`; absence check | pass after harness correction | The first Trash attempt failed because the deliberately read-only root denied macOS Trash access; its shell wrapper also lacked fail-fast behavior and incorrectly continued to a success print. Direct inspection confirmed the stage still existed. After revalidating the exact marker, 97-file allowlist, and zero symlinks, the orchestrator restored owner write permission only on that stage, reran Trash under `set -e`, and verified the original path absent. The stage is recoverable from Trash. |
| Claim ceiling | implementation/reviewer scope audit | partial by design | Authorized aggregate reconciliation, synthetic scale, DHAT, and physical-format evidence are complete. C-04 remains open only for the canonical phase-boundary command set and final handoff/requirements audit. No real-corpus promotion or biological/scientific claim is made. |

## C-04 final phase-boundary and closure evidence

Implementation SHA: `55d1c8b8a07b2dbf40f77b5590127456f9042295`.

| Gate | Exact command | Status | Result/evidence |
|---|---|---|---|
| Full workspace tests | `cargo +1.96.0 nextest run --locked --workspace --all-features` | red, then pass | The first run stopped after 529 passes when `workflow_contract` tried to read `benches/support/` as a file; 82 tests did not run. Recursive Rust-only benchmark discovery failed first on that regression, then the final exact rerun passed 612/612 with 23 skipped and one known slow test in 64.529 s. |
| Focused domain/project/compatibility | `cargo +1.96.0 test --locked --test cellvit_embedding_table`; `cargo +1.96.0 test --locked -p marklab-embeddings`; `cargo +1.96.0 test --locked -p marklab-project`; `cargo +1.96.0 test --locked --test project_workflow --test api_contract --test result_v03 --test workspace_contract` | pass | Respectively 6/6, 8/8 under the exact default package command, 41/41, and 28/28 pass. The all-feature Nextest run separately covers every optional physical-format test. |
| Documentation | `RUSTDOCFLAGS='-D missing-docs' cargo +1.96.0 doc --locked --no-deps -p marklab-embeddings -p marklab-project`; `cargo +1.96.0 test --locked --workspace --doc --all-features` | pass | Missing public docs are denied; all six workspace doc-test binaries pass with zero executable doc tests. |
| Eight-row feature matrix | `cargo +1.96.0 check --locked --workspace --all-targets`; `cargo +1.96.0 check --locked --workspace --all-targets --no-default-features`; `cargo +1.96.0 check --locked --workspace --all-targets --all-features`; `cargo +1.96.0 check --locked --workspace --all-targets --no-default-features --features csv`; `cargo +1.96.0 check --locked --workspace --all-targets --no-default-features --features parquet`; `cargo +1.96.0 check --locked --workspace --all-targets --no-default-features --features cli`; `cargo +1.96.0 check --locked --workspace --all-targets --no-default-features --features wsi`; `cargo +1.96.0 check --locked --workspace --all-targets --no-default-features --features wsi,cli` | pass with documented narrow warnings | All exact commands exit 0 on final code. Default/all/CLI/WSI+CLI are clean. Minimal/CSV/WSI retain 14 existing instrumentation warnings; Parquet-only also retains four existing compatibility-writer warnings. No warning-clean claim is made for narrow rows. |
| Warning-denied Clippy | `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`; `cargo +1.96.0 clippy --locked --workspace --all-targets --no-default-features --features cli -- -D warnings` | red, then pass | The first all-feature run exposed five DHAT-only benchmark helper items when mimalloc excluded the heap test. Matching helper cfg to the test's allocator predicate removed the dead surface; both final exact commands exit 0 without suppression. |
| WSI compatibility | `cargo +1.96.0 test --locked --package marklab --features wsi,cli --test wsi_integration` | pass with scheduled oracle ignored | 10 local tests pass; one checksummed public Aperio/OpenSlide oracle remains explicitly ignored. |
| Fuzz/dependency boundary | `cargo +nightly fuzz check`; `cargo audit`; `cargo deny check advisories licenses bans sources`; `cargo machete` | pass with reviewed warnings | Fuzz check passes in 3.19 s. Audit scans 370 dependencies / 1,225 advisories with no vulnerability and two allowed unmaintained warnings. All deny categories pass with 17 configured duplicate-version warnings. Machete finds no unused dependency. |
| Heap gates | `cargo +1.96.0 test --locked --package marklab --no-default-features --features dhat-heap --lib dhat_ -- --test-threads=1`; `cargo +1.96.0 test --locked --features csv,parquet,dhat-heap --test cellvit_embedding_heap -- --exact embedding_10k_1280_peak` | pass | Legacy 3/3 pass with 180 filtered and the 14 documented warnings. Final embedding phase rerun passes in 138.51 s; the diagnostic run records current 0, peak 324,194,945, cap 603,979,776 bytes. |
| Criterion smoke/full | `env MARKLAB_BENCH_PROFILE=smoke cargo +1.96.0 bench --locked --workspace --all-features -- --quick`; prebuild `cargo +1.96.0 bench --locked --bench cell_embeddings --features csv,parquet --no-run`; timed `/usr/bin/time -l cargo +1.96.0 bench --locked --bench cell_embeddings --features csv,parquet -- '1m_x_256' --noplot` | pass | Workspace smoke executes 8 targets / 10 intervals; embedding is 4.1952–4.2284 s and existing targets report no detected regression. Full is 10.446–10.488 s, 24.408–24.506 million values/s, and 1,117,552,640-byte RSS under 2.5 GiB. |
| Release synthetic smoke | `cargo +1.96.0 run --release --locked --package marklab --features wsi --bin marklab -- smoke --suite synthetic --replicates 10 --out /tmp/marklab-c04-smoke.cdxpkS` | pass | `smoke.json` is 14,043 bytes; 12/12 scenarios and 120/120 replicates pass with zero failed. The validated output root was moved to Trash. An initial aggregate-only `jq` query assumed results were an array, exited nonzero, and was corrected to the actual object schema before counts were accepted. |
| Authorized reconciliation | `MARKLAB_AUTHORIZED_CELLVIT_ROOT=/tmp/marklab-c04-authorized.g8Z0iv cargo +1.96.0 test --locked -p marklab --features csv --test authorized_cellvit_reconciliation -- --ignored --exact reconciles_all_authorized_bundles` | pass | Exact final source evidence remains 32 bundles / 60,191 rows / width 1,280 and both pinned digests; privacy-safe owned-stage creation/cleanup evidence is recorded immediately above. |
| Clean package characterization | `cargo +1.96.0 package --locked --workspace` | known exit 101; deliberately non-green | All six archives are created and core verifies; normalized data then resolves published pre-C-02 core and fails on missing coordinate IDs. DEC-0018 remains release-blocking. |
| Supplemental archive compatibility | `cargo +1.96.0 package --locked --workspace --config 'patch.crates-io.marklab-core.path="crates/marklab-core"' --config 'patch.crates-io.marklab-data.path="crates/marklab-data"' --config 'patch.crates-io.marklab-project.path="crates/marklab-project"' --config 'patch.crates-io.marklab-embeddings.path="crates/marklab-embeddings"' --config 'patch.crates-io.marklab-workflow.path="crates/marklab-workflow"'` | pass; non-equivalent | Every archive verifies against the current local source graph. This does not establish registry resolvability and no permanent patch/version/publish action is introduced. |
| Format/scope | `cargo +1.96.0 fmt --all --check`; `git diff --check`; final `git status --short` | pass | Formatting and whitespace gates exit 0; final closure scope is documentation only over clean implementation SHA `55d1c8b`. |
| Independent closure | `c04_embedding_audit`; `c04_feasibility_audit` final read-only audits | pass after documentation findings applied | Reviewers approve C-04 and EMB-CORE completion, require DATA-01/FND-05/WS-24/WS-C to remain active, and retain real promotion, registry packaging, Windows admission, broad interchange, patch links, general status, and embedding science as explicit future work. No code/security/privacy/resource/claim blocker remains. |

## C-05 authorized aggregate/header inventory — 2026-08-23

Inventory base/current SHA: `e3dadae179957d7d4704a00531ed07f29b919934`. The audit was read-only. Its remote wrapper was `ssh -o BatchMode=yes 100.82.140.72 "$remote_py" - "$root_a" "$root_b" "$root_c" <<'PY' ... PY`; the authorized roots and audited Python executable were derived at runtime from the existing `handoffs/SLIDE-INV.md` command record. Two initial wrapper attempts exited 1 locally with `zsh:1: unmatched '\''`; the corrected wrapper printed `remote_ok`, and every substantive scan exited 0. Exact here-document bodies were not retained as a repository script, so this characterization is not a reproducible/pinned corpus validator and cannot support promotion.

| Gate | Read-only operation | Status | Aggregate evidence / limit |
|---|---|---|---|
| Candidate inventory | excluded build/dependency/trash roots; aggregate filename-token/extension counts and bytes | pass | Candidate classes were selected only for bounded header/schema inspection; path names alone carried no semantic admission. |
| NPY/CSV/JSON headers | NPY magic/version/header readers with 64-KiB header cap; RFC-style CSV row counting; recursive allowlisted key-name aggregation after the defect | pass for aggregate characterization | Patch candidates: 2 C-order `f32 [N,6]` / 25,047 rows, 3 `f32 [N,384]` / 44,914 rows, and 200 `f32 [N,1024]` / 434,372 rows; each has at least one row-count-matched CSV. Width 6 conflicts with adjacent 1,024 declarations. Width 384 is explicitly development-only/non-lock-eligible. Width 1,024 spans seven materially different source-local CSV signatures and incomplete context/provenance. CSV rows and JSON documents were machine-parsed for counts/allowlisted aggregates; nonallowlisted values and every payload component were not selected or emitted. |
| Patch HDF5 metadata | `h5py` dataset metadata and allowlisted attributes only | pass | 163 readable containers, zero open errors, 215,481 matched nonempty rows (82–8,962/container). Each has source-local barcode `[N,1]` object, coordinate `[N,2]` int64, and image `[N,224,224,3]` uint8 datasets with equal N; no recognized vector dataset. All declare downsample/source crop/target crop/pixel size; none declares stride, overlap, frame convention, or effective receptive field. No dataset payload was read. |
| Region/slide/NPZ negative evidence | embedded NPY headers via `zipfile`; allowlisted CSV/JSON header/key counts | pass | Thirteen region-path `f32` NPYs have widths 28/32/65/74; proved row parity is cell-row, not `RegionId`. Ten region NPZs have mixed ranks/dtypes/shapes without stable region rows. One slide-path `f32 [310856,1280]` matrix is not one vector per slide. No region/slide source profile is admitted. |
| Link/context/promotion ceiling | aggregate schema-presence audit | incomplete by source, as expected | Some source-local patch/region/center-cell fields exist, but no canonical identity map, `CellPatchLink`, `PatchRegionLink`, stride, overlap, effective receptive field, C-02 frame/transform, full observation window, shared-vector contract, or uniform complete provenance exists. No real table/link is promoted. |
| Safe-reader availability | module/tool probes | characterized | Audited Python had `h5py`; it lacked `pyarrow.parquet`; `parquet-tools` and `duckdb` were absent. No Parquet schema claim is made. |
| Local scope | `git rev-parse HEAD`; `git status --short`; `git diff --check` | pass at inventory completion | HEAD was the SHA above, status was clean, and diff check passed. No Cargo/Clippy/Nextest/fuzz/package/benchmark gate applied to this read-only inventory. |

No aggregate or audit digest was computed; no digest is invented or pinned. No NPY/NPZ vector payload, HDF5 image/barcode/coordinate payload, `.pt`/`.pth`/pickle/checkpoint, WSI pixel payload, or patient/sample row value was intentionally inspected. No full WSI hash, tissue-mask/window construction, canonical-ID mapping, link construction, or local/remote mutation occurred.

Procedure defect: one intermediate adjacent-JSON script recursively treated every dictionary key as a possible schema key, so several sample-like map keys appeared in a private tool transcript. No associated values, vectors, credentials, or remote paths were emitted, and those tokens were neither repeated in the handoff nor retained in repository files. Subsequent scans used an exact allowlist. Therefore the truthful claim is that final/ledger evidence is aggregate-only and no identifier was intentionally retained or forwarded—not that every intermediate tool output was identifier-free. Any committed audit utility must inspect only exact known locations, report map cardinality/type without keys, prohibit recursive free-form key output, emit a closed deterministic schema, and pass stdout/stderr/`Debug` sentinel tests.

## C-05 contract and inventory freeze — 2026-08-23

Contract base/current implementation SHA: `e3dadae179957d7d4704a00531ed07f29b919934`. This checkpoint changes documentation only and admits no real source adapter or artifact.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-contract compatibility | `cargo +1.96.0 test --locked --test workflow_contract` | pass | 7 passed, 0 failed. Existing workflow, release, fuzz-manifest, and benchmark-contract assertions remain green before production C-05 work. |
| Format/scope | `cargo +1.96.0 fmt --all --check`; `git diff --check`; `git status --short` | pass | Both commands exit 0; pre-commit status contains only the C-05 contract, decisions, ownership/interface/status/requirements/claims/repository/validation documentation, and the prior-inventory handoff qualification. No production source, test, manifest, dependency, or lockfile is dirty. |
| Independent contract review | `c04_embedding_audit`; `c04_feasibility_audit` iterative read-only audits | pass after findings applied | Red reviews caught incomplete source-role admissions, normalization bounds, contradictory digest shorthand, opaque vector/coordinate correspondence overclaim, missing scalar/null digest framing and goldens, and a stale observation-window claim. Final whole-diff reviews independently report no remaining blocker/high and approve DEC-0032–0034 plus the contract freeze. Reviewers made no edits and performed no remote access. |

## C-05 first logical expected/context/footprint/table checkpoint — 2026-08-23

Implementation base: `4e1d5b5f6dfea2332e62ce9aaf8752cb46351a1f`. This is an intermediate logical checkpoint, not C-05 or EMB-PATCH closure. It adds no manifest, dependency, lockfile, source adapter, physical reader/writer, artifact publication, or real-corpus promotion.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first compile contract | `cargo +1.96.0 test --locked --test multiscale_embedding_tables` | red for the expected reason, then pass | The first run failed only because the frozen C-05 public expected-set/context/footprint/table owners were unresolved. After the smallest logical implementation and resource corrections, the modular suite passes 7/7. |
| Canonical domain/wire/digest | focused expected/context/table cases in `multiscale_embedding_tables` | pass | Three entity-specific expected schemas and three entity-table domains have fixed golden bytes/digests; context covers null/array/scalar framing, signed-zero rules, centered/full receptive fields, boundary variants, and exact bindings. Strict order, hierarchy, singleton slide, all four statuses, and hidden non-present fillers are enforced. |
| Boundary and resource gates | focused resource/boundary cases in `multiscale_embedding_tables` | red under review, then pass | Reviewer findings drove pre-allocation output sizing, encoded/decoded/retained budgets, owned metadata and caller-capacity accounting, construction-peak checks, checked signed endpoints against full positive `u64` source extents, exact/one-short independent size oracles, and aggregate-safe `Debug`/error surfaces. |
| Block/scan parity | focused table partition regressions | red under review, then pass | Construction and bounded scans share one digest/QC accumulator. Patch, region, and slide views expose only present vectors; all four statuses and partitions 1–4 preserve identical counts and logical digests. |
| Focused default/no-default | `cargo +1.96.0 test --locked --test multiscale_embedding_tables`; `cargo +1.96.0 test --locked --no-default-features --test multiscale_embedding_tables` | pass | Both configurations pass 7/7 in final reviewer verification. |
| Affected package | `cargo +1.96.0 test --locked -p marklab-embeddings --all-features` | pass | All package unit/integration/doc targets pass; C-04 domain/source behavior remains green. |
| Compatibility regressions | `cargo +1.96.0 test --locked --test api_contract --test cellvit_embedding_table` | pass | Existing facade/API and C-04 cell-embedding contracts pass 6/6 and 6/6. |
| Documentation and features | `RUSTDOCFLAGS='-D missing-docs' cargo +1.96.0 doc --locked --no-deps -p marklab-embeddings`; `cargo +1.96.0 check --locked --workspace --no-default-features` | pass | Public documentation is complete and the narrow workspace build succeeds. |
| Warning/format/scope | `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`; `cargo +1.96.0 fmt --all --check`; `git diff --check` | pass | Warning-denied Clippy, formatting, and whitespace gates pass. The source is split by responsibility; no generated file, manifest, dependency, or lockfile changed. |
| Independent implementation review | `c04_embedding_audit`; `c04_feasibility_audit` iterative read-only audits | pass after findings applied | Initial reviews rejected incomplete resource accounting, premature allocation, accidental `i64` source narrowing, and missing scan parity. Final independent re-audits report no blocker/high and approve only this first logical checkpoint; overlap, links, records, physical profiles, graph validation, scale evidence, and C-05 closure remain open. Reviewers made no edits. |

## C-05 deterministic patch-overlap checkpoint — 2026-08-23

Implementation base: `c4557f3eddd44d6c48cbca2ecc3e2ae81923a81e`. This checkpoint adds only the footprint-derived logical overlap graph and focused evidence. It is not a cell/region link, physical profile, artifact receipt, or C-05/EMB-PATCH closure.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first compile contract | `cargo +1.96.0 test --locked --test multiscale_embedding_tables` | red for the expected reason, then pass | The red run failed only for unresolved `PatchOverlapGraph` and its explicit peak-working-budget error. The final modular suite passes 11/11. |
| Indexed/brute-force parity | focused sparse PRNG plus table-driven exhaustive small cases | pass after review expansion | Independent O(p²) oracles agree for empty, singleton, identical-origin dense clique, half-open edge/corner contact, adjacent-bucket nonedge, negative `div_euclid` boundaries, a late bridge joining earlier components, and a fixed 64-patch fixture. Edges are unique, `left < right`, lexicographic, and every patch receives the minimum-ID component including isolates. |
| Digest/binding/privacy | independent framed digest oracle, binding-drift and `Debug`/error sentinels | pass | The frozen domain/binding/edge digest has golden `b512cee465515aa5a077c0fc2f8d87869de8cdcbbb398182ed67fc0f5139d1f4`; expected/context drift is rejected, and IDs/origins are absent from aggregate debug/error text. |
| Resource and hard limits | exact/one-short retained and peak-working oracles; private counter boundary | pass | Both passes share one bucket traversal, retain no candidate list, reserve exact/fallible edge/component capacity, charge bucket/union-find/IDs/edges/components, and accept exactly 400,000,000 edges while rejecting 400,000,001 without constructing a giant fixture. |
| Focused minimal configuration | `cargo +1.96.0 test --locked --no-default-features --test multiscale_embedding_tables` | pass | 11/11 pass without default features. |
| Affected package | `cargo +1.96.0 test --locked -p marklab-embeddings --all-features` | pass | 21 unit tests, including the private hard-limit boundary, plus 6 domain and 1 hostile Arrow integration test pass; doc tests have zero executable cases. |
| Documentation/features/Clippy | `RUSTDOCFLAGS='-D missing-docs' cargo +1.96.0 doc --locked --no-deps -p marklab-embeddings`; `cargo +1.96.0 check --locked --workspace --no-default-features`; package and final workspace all-target/all-feature Clippy with `-D warnings` | red in the final workspace scope, then pass | Public overlap APIs document cleanly and the minimal workspace builds. Package Clippy was green; the first final workspace run caught one collapsible nested conditional in the 64-patch test oracle. The mechanical test cleanup preserved 11/11 behavior, and the exact workspace Clippy rerun exits 0. |
| Format/scope | `cargo +1.96.0 fmt --all --check`; `git diff --check`; final diff/status review | pass | Only overlap production, focused exports/error surface, modular tests, and implementation records changed; no manifest, dependency, lockfile, generated file, source adapter, or physical profile changed. |
| Independent implementation review | `c04_embedding_audit`; `c04_feasibility_audit` iterative read-only audits | pass after evidence finding applied | Both found no production algorithm/API/resource blocker. Feasibility review initially rejected shallow pathological coverage; after the exhaustive small-case matrix, binding/privacy cases, and private hard-limit test, both independently approve only this bounded overlap checkpoint. Reviewers made no edits. |

## C-05 vector-free cell-patch logical checkpoint — 2026-08-23

Implementation base: `2c5c513d3504236d23ea2bbf3d68e511abcd4c7c`. This checkpoint adds only canonical cell assignment groups and vector-free patch edges. It is not a producer JSON record, physical assignment/edge pair, artifact receipt, real-corpus link, or C-05/EMB-PATCH closure.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first compile contract | `cargo +1.96.0 test --locked --test cell_patch_link` | red for the expected reason, then pass | The red run failed only for the missing cell-link public owners and closed error variants. After implementation and review corrections, the modular suite passes 7/7. |
| Contained-shared semantics | focused half-open, brute-force, negative/fractional/large-coordinate, dense/shared, empty, and zero-edge cases | pass | Every expected cell has one canonical finite anchor/group; edges equal all containing footprints, preserve overlap, exclude right/bottom contact, retain outside-support zero states, and remain sorted by assignment row/`PatchId`. A deterministic 64-wave randomized oracle agrees with brute force. |
| Declared interpolation | focused valid/invalid group matrix | pass | Empty groups become `interpolation_unavailable`; assigned groups have 1–4 strict patch IDs, positive common denominators, checked unit sums, and whole-group GCD one. Valid `2/4 + 1/4 + 1/4` is retained; reducible whole groups, mixed denominators, nonunit sums, duplicates/order drift, unknown patches, zero values, and excess contributors fail. |
| Logical identity/privacy | independent 128-bit framed oracle and fixed mode goldens | pass | Contained/interpolation digests pin `ffadec571282725c30a94178358bbe885642c043948a3529d23c00125f3a1b73` and `c7317ffb9ec145963dd2d311eec5ea97a7f4254176226a9d6b61f903d8d4bb06`. Signed zero canonicalizes; `Debug`/errors omit IDs, anchors, weights, paths, and source values. No vector component exists in any link type. |
| Bindings/hierarchy | expected/frame/artifact/hierarchy/context/footprint drift cases | pass | Exact expected cells/patches, same-slide hierarchy, image frame, context/footprint digests, producer content, and distinct artifact roles are required; empty expected cells remain valid. |
| Byte/resource accounting | exact/one-short independent contained/interpolation oracles, including over-capacity input | red under review, then pass | Review caught omitted owning-`SlideId` text; predicted/recomputed retained and peak formulas plus both oracles now include it. Outer/nested caller capacities, bucket scratch, assignments, edges, typed-ID text, and link metadata are charged before exact/fallible allocation. |
| CPU-work and hard limits | explicit candidate budget; private edge counter and large-integer containment tests | red under review, then pass | Review found a zero-output `Theta(c * p)` dense-bucket path not bounded by bytes/edges. DEC-0035 adds per-pass checks before comparison and exact pass parity; the degenerate fixture passes at 12 checks and fails at 11. Cell-patch edges accept 400,000,000 and reject 400,000,001 through the private counter without giant allocation. |
| Focused minimal configuration | `cargo +1.96.0 test --locked --no-default-features --test cell_patch_link` | pass | Both logical modes and their negative/resource cases pass without default features. |
| Affected package/docs | `cargo +1.96.0 test --locked -p marklab-embeddings --all-features`; `RUSTDOCFLAGS='-D missing-docs' cargo +1.96.0 doc --locked --no-deps -p marklab-embeddings`; `cargo +1.96.0 check --locked --workspace --no-default-features` | pass | Package unit/domain/hostile tests, public documentation, and the minimal workspace build pass. |
| Warning/format/scope | package then final workspace all-target/all-feature Clippy with `-D warnings`; `cargo +1.96.0 fmt --all --check`; `git diff --check`; final status review | red during development and first final workspace scope, then pass | Package Clippy first replaced one manual range check with the canonical range API. The first final workspace run then found identity arithmetic and manual slice sizing in the independent test oracle; both were mechanically clarified. Final exact warning/format/whitespace gates pass. No manifest, dependency, lockfile, physical profile, producer record, or real-source adapter changed. |
| Independent implementation review | `c04_embedding_audit`; `c04_feasibility_audit` iterative read-only audits | pass after two highs and one accounting finding applied | Reviews first caught owning-slide undercharge, then rejected the unbounded candidate-work path and shallow empty/dense/hierarchy/binding evidence. After the accounting fix, explicit CPU budget, pathological regression, and expanded matrix, both independently approve only this bounded logical cell-link checkpoint. Reviewers made no edits. |

## C-05 exhaustive patch-region logical checkpoint — 2026-08-23

Implementation base: `5ddcc285c59778f7ab15c829d283768dbac538da`. This checkpoint adds only the runtime exhaustive declaration and canonical sparse logical link. It is not the canonical assessment JSON record, converter/assessment graph verification, physical link profile, geometric proof, real-corpus link, or C-05/EMB-PATCH closure.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first compile contract | `cargo +1.96.0 test --locked --test patch_region_link` | red for the expected reason, then pass | The red run failed only for unresolved patch-region public owners and closed error variants. The modular suite now passes 9/9. |
| Exhaustive sparse semantics | focused Cartesian-count, empty-product, canonical-order, duplicate, unknown-ID, fraction, and assessment-to-link equality cases | pass | The assessment derives the exact checked expected-patch × expected-region count; only canonical nonzero rows are stored, absent expected pairs mean producer-declared zero, and link construction accepts no independent row argument that could omit a declared relation. Empty products derive empty links. |
| Fraction/geometry boundary | full/partial constructor matrix and overlapping-region fixture | pass | `fully_contained` is exactly `1/1`; `partial_overlap` is positive, reduced, and strictly below one. Fractions for one patch may exceed unit total because regions may overlap. Public documentation and errors call these producer declarations, never independent geometric proof. |
| Bindings/privacy | expected-set and owning-slide drift, distinct dependency roles, assessment-evidence drift, and `Debug` sentinels | pass | Expected patch/region membership, context/footprint identities, one slide, converter evidence, and assessment evidence are exact. Artifact aliases fail; IDs/fractions/source values do not enter errors or `Debug`. No link type owns a vector component. |
| Logical identity | independent 128-bit-framed relations/link oracles and fixed goldens | pass | Relations digest pins `e8380a102ecc8530cef24a9e823e278a8ceb6a0992f0f715ee46ce2d54c5cad6`; link digest pins `290ee111de5f60c016e1f28fe33cb8989a1a9f70b2b507e9fef5c01b031e5b28`. Evidence artifact/content drift changes link identity. |
| Byte/resource accounting | independent exact/one-short retained and peak-working oracles; over-capacity input; empty link | red under review, then pass | Input capacity and typed-ID text, owning-slide text, value metadata, exact-capacity output rows, and allocation failures are charged. Feasibility review found the first link peak omitted the still-live borrowed assessment; the corrected preflight checked-adds assessment retained plus link retained before cloning, and the one-short regression fails with the exact count. |
| Hard arithmetic limits | private row-count and Cartesian-product helpers | pass | The row helper accepts 400,000,000 and rejects 400,000,001 without giant allocation. Checked product accepts `u64::MAX × 1` and rejects `u64::MAX × 2` with `PatchRegionPairCountOverflow`. |
| Focused minimal configuration | `cargo +1.96.0 test --locked --no-default-features --test patch_region_link` | pass | All logical, negative, evidence, and resource cases pass without default features. |
| Affected package/docs | package tests with default/no-default features; doc tests; missing-docs build; minimal workspace check | pass | The affected library/domain suites, public documentation, and minimal configuration compile cleanly. |
| Full compatibility | `cargo +1.96.0 test --locked --workspace --all-features` | pass | Every executed workspace unit, integration, and doc test passes; only the repository's explicitly documented manual performance, scheduled calibration, public-fixture/oracle, and authorized-corpus tests remain ignored. C-04 logical/profile/source goldens and current API/config/result/workflow behavior remain green. |
| Warning/format/scope | final workspace all-target/all-feature Clippy with `-D warnings`; `cargo +1.96.0 fmt --all --check`; `git diff --check`; final status review | pass | No manifest, dependency, lockfile, generated file, strict record, physical profile, source adapter, or real-source promotion changed. |
| Independent implementation review | `c04_embedding_audit`; `c04_feasibility_audit` iterative read-only audits | pass after high/medium findings applied | Semantic review approved the initial bounded design. Feasibility review found the compositional link-peak undercharge and missing direct product-overflow evidence. After red/green corrections, both independently report no remaining finding and approve only this logical checkpoint. Reviewers made no edits. |

## C-05 strict patch source-record checkpoint — 2026-08-23

Implementation base: `309b06616d3e028a1f3c4acd9bcff3c6ea5934ee`. This checkpoint adds only strict canonical values/codecs for the patch source-entity set, source-to-expected identity map, embedding source-row link, and input normalization. It is not a producer/support/derivation/assessment/provenance record, artifact graph/receipt, physical profile, source adapter, real-corpus promotion, or C-05/EMB-PATCH closure.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first compile contract | `cargo +1.96.0 test --locked --test multiscale_embedding_records` | red for the expected reason, then pass | The first run failed only for unresolved intended source set/map/row-link/normalization public owners. After the smallest record implementation and review corrections, the modular suite passes 7/7. |
| Source-chain semantics | focused source identity and empty-chain cases | pass | Source keys are bounded, control-free, trimmed, and strict bytewise ordered; source rows and non-null vector rows are unique gap-free zero-based ranges. The identity map exactly covers expected patches and the row link preserves expected order, mapped source identity, and all four statuses; only present/QC-rejected rows carry source-vector rows. Empty chains remain canonical. |
| Canonical wire and identity | focused independent byte/golden/fixed-point cases | pass | Exact key order, closed tokens, lower-case fixed-width artifact/digest hex, decimal grammar, unknown-field rejection, and canonical re-encoding are frozen. Logical SHA-256 goldens are source set `1b7ef7d5899d89b17e2309455dd4c07df4c7211924f53b6036552e7d3e6e76e2`, identity map `cea2e3570e1d10cfd1a7de08110eca3c6d5c210164c5b6022e4b5568971638e6`, row link `37ef6645c0a1ab664be6182eb00b9953dcdc79890ee682014427fae60920da76`, and normalization `bd7e6b0644392a32a26f8e38e6d32e5a6fa35e25f27378657a3a898a06773e0b`. |
| Structural/resource preflight | focused exact/one-short and private boundary tests | red under review, then pass | High-volume JSON uses a bounded structural/count pass, exact-capacity typed collection, and streaming canonical fixed-point comparison without `serde_json::Value`. Tests cover the 100,000,000-row/100,000,001-row boundary, 1-GiB encoded cap, depth 8, 256 object fields, exact/max+1 raw strings including escapes/unterminated input, exact typed allocation failure, and exact encoded/decoded/retained plus over-capacity construction budgets. Allocation failures remain typed instead of collapsing into invalid-JSON errors. |
| Privacy and binding | focused aliases, domain/range/order/status/vector-gap, drift, and `Debug` sentinels | pass | Source, expected-set, normalization, artifact, and content-digest bindings are exact; dependency roles are distinct. IDs, source keys, and source values do not enter aggregate error or `Debug` surfaces. Stack-only hex encoding avoids fixed-token heap allocations. |
| Focused default/no-default | `cargo +1.96.0 test --locked --test multiscale_embedding_records`; `cargo +1.96.0 test --locked --no-default-features --test multiscale_embedding_records` | pass | Both configurations pass 7/7. |
| Affected package/features/docs | `cargo +1.96.0 test --locked -p marklab-embeddings --all-features`; `cargo +1.96.0 test --locked -p marklab-embeddings --no-default-features`; `RUSTDOCFLAGS='-D missing-docs' cargo +1.96.0 doc --locked --no-deps -p marklab-embeddings`; `cargo +1.96.0 check --locked --workspace --no-default-features` | pass | All-feature package: 30 library, 6 domain, and 1 hostile integration tests pass. No-default package: 14 library and 6 domain tests pass. Public documentation and the minimal workspace build are clean. |
| Full compatibility | `cargo +1.96.0 test --locked --workspace --all-features` | pass | Every executed workspace unit, integration, and doc test passes. Only the repository's explicitly documented authorized-corpus and public-oracle fixtures remain ignored; C-04 and earlier C-05 behavior/goldens remain green. |
| Warning/format/scope | `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`; `cargo +1.96.0 fmt --all --check`; `git diff --check`; final status review | red during review, then pass | Independent review exposed one manual slice-size calculation in the root integration test; the mechanical correction preserved behavior. Final warning, formatting, and whitespace gates pass. No manifest, dependency, lockfile, generated file, physical profile, graph, receipt, adapter, or real-source promotion changed. |
| Independent implementation review | `c04_embedding_audit`; `c04_feasibility_audit` iterative read-only audits | pass after findings applied | Reviews caught the root Clippy issue, loss of typed large-allocation errors inside Serde, fixed-hex heap allocations, oversized mixed parser/domain owners, and missing direct raw-string boundary evidence. After centralized helpers, stack-only hex, split wire owners, explicit fallible allocation outside Serde, and added boundary tests, both independently report no remaining finding and approve only this source-record checkpoint. Reviewers made no edits. |

## C-05 derivation/support/producer/assessment record checkpoint — 2026-08-23

Implementation base: `b5b45d159c491e32c2d5645c012aa99b8fbf415e`. This checkpoint adds only the two deterministic derivation values, four support variants, two cell-link producer modes, and strict canonical validation of the existing exhaustive assessment descriptor. It is not multiscale provenance, an artifact-record/profile graph, a runtime receipt, a physical profile, a source adapter, real-corpus promotion, or C-05/EMB-PATCH closure.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first compile contract | `cargo +1.96.0 test --locked --test multiscale_embedding_records` | red for the expected reason, then pass | The red run failed only for the intended missing derivation/support/binding/producer owners, new error variants, and assessment wire methods. After the smallest implementation and review corrections, the combined modular suite passes 11/11. |
| Derivation logical contract | independent exact-wire and 128-bit-framed digest oracles | pass | `weighted_mean` and `arithmetic_mean` are deterministic, require `exclude_non_present_require_one`, `f64` accumulation, `f32` output, and one bounded algorithm-version token. Logical goldens are `2b8565ee1ae00615e7f877ad145a1143fcefe51eff341818b96f912a0e4da022` and `40bd3bfef7d12d241baf7a5c31ba9aefe97fbc9673c49e282d209db9a7ace59c`. |
| Four support variants | independent exact suffix-wire, binding-order, framed-digest, and sorted-dependency oracles | pass | Patch, region-from-patches, slide-from-patches, and slide-from-regions enforce their implied entity kind and exact non-null binding set. Logical goldens are `54e656dc1122f1d25779964b8d81c934ec0b0ea918c47cb5a70d014f9035ff1a`, `e4ab39eddb379cbdbbffb21adf7ccf6e9f90da9e2fa071e651948b4a63ca74f1`, `7c63abd92dabf0efa9f950bd9660fe796567e631b8eb67c2f4e752c53ce20051`, and `de73b950c2e74d2591cd131345d9e6fdcb984f79e426464e60e3519ca7347b82`. |
| Producer and assessment content | exact canonical byte/content-digest and dependency-set oracles | pass | Contained mode fixes `all_half_open_anchor_containment`; declared interpolation requires a different explicit token. Both bind exactly four distinct roles. Producer content goldens are `4a643365fa4a49afc410202640c106b0e4cdbdf1ab83273a311128047dad42e3` and `49bbb0e70f00ecfd03d9c8dc96c73b1c2d0e15b1343e8b0fce3a7b777d4336bc`. The validate-only assessment descriptor exactly binds five dependencies, counts, and relation digest with content golden `dfcaabc52dfd86a403c32d6aea6e953f538f09c564ba87941b783aca444670d3`; it does not pretend to reconstruct sparse rows or invent a logical digest. |
| Strictness and privacy | missing/extra/duplicate/reordered keys, final-newline, uppercase-hex, escaped-token, variant/entity/mode/algorithm drift, token 128/129, pairwise aliases, and `Debug` sentinels | pass after review expansion | Manual borrowed visitors plus streaming fixed-point comparison reject every alternate encoding. Aliases compare artifact IDs even when logical digests differ; the containment algorithm is reserved from interpolation. Errors and debug output omit slide IDs, algorithm versions, artifact IDs, and source values. |
| Resource boundaries | exact/one-short constructor and decoder budgets plus hostile hard-cap input | pass after review expansion | Derivation, support, and producer constructors charge over-capacity owned token inputs and retained ID/token text. All decoders/assessment validation enforce exact encoded, decoded, and retained boundaries as applicable; a 256-KiB-plus-one input reports the 256-KiB hard maximum even with an unbounded caller limit. Parsing remains borrowed until preflight passes, and dependency sorting uses at most five stack-resident IDs. |
| Focused default/no-default | `cargo +1.96.0 test --locked --test multiscale_embedding_records`; `cargo +1.96.0 test --locked --no-default-features --test multiscale_embedding_records` | pass | Both configurations pass 11/11. |
| Affected package/features/docs | `cargo +1.96.0 test --locked -p marklab-embeddings --all-features`; `cargo +1.96.0 test --locked -p marklab-embeddings --no-default-features`; `RUSTDOCFLAGS='-D missing-docs' cargo +1.96.0 doc --locked --no-deps -p marklab-embeddings`; `cargo +1.96.0 check --locked --workspace --no-default-features` | pass | All-feature package: 30 library, 6 domain, and 1 hostile integration tests pass. No-default package: 14 library and 6 domain tests pass. Public documentation and the minimal workspace build are clean. |
| Full compatibility | `cargo +1.96.0 test --locked --workspace --all-features` | pass | Every executed workspace unit, integration, and doc test passes; root library reports 293 passed and 21 documented manual/scheduled tests ignored. The authorized-corpus and public-oracle integration fixtures remain explicitly ignored, and all C-04/earlier C-05 goldens remain green. |
| Warning/format/scope | package/root-focused then final workspace all-target/all-feature Clippy with `-D warnings`; `cargo +1.96.0 fmt --all --check`; `git diff --check`; final status review | red during test-contract review, then pass | Reusing a broad neighboring fixture first exposed warnings; the focused fixture and four cohesive test owners are warning-clean. Final warning, formatting, and whitespace gates pass. No manifest, dependency, lockfile, generated file, provenance, physical profile, graph, receipt, adapter, or real-source promotion changed. |
| Independent implementation review | `c04_embedding_audit`; `c04_feasibility_audit` iterative read-only audits | pass after findings applied | Reviews rejected an invalid retained-budget probe, under-pinned support/producer variants, incomplete dependency/resource/fixed-point evidence, the reserved containment algorithm in interpolation, and missing hard-cap separation. After exact oracles, modular fixtures/tests, pairwise alias cases, full budget matrix, reserved-token rejection, and independent 256-KiB clamps, both reviewers report no remaining finding and approve only this small-record checkpoint. Reviewers made no edits. |

## C-05 multiscale provenance-value checkpoint — 2026-08-23

Implementation base: `b3f3177341b8bc16a0e56f41deef1e0e1690d0c1`. This checkpoint adds only the four closed multiscale provenance values/codecs and focused shared escaped-string preflight accounting. It is not an artifact-record/profile graph, runtime receipt, physical profile, source adapter, real-corpus promotion, or C-05/EMB-PATCH closure.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first contract | `cargo +1.96.0 test --locked --test multiscale_embedding_records` | red for the expected reasons, then pass | The initial compile failed only for the five intended provenance owners. A valid escaped citation then reproduced `InvalidCanonicalJson`; converting the exact parser view to owned decoded text made that fixed point pass. The final modular suite passes 21/21. |
| Four exact provenance identities | exact canonical-wire assertions plus an independent 128-bit-length framing oracle and fixed hex goldens | pass | Direct patch, derived region, slide from patches, and slide from regions use their exact frozen suffix order, entity kind, `f32` dtype, pooling/aggregation, and scalar wire types. Logical goldens are `943c403154d9a13cfc8940dbc1e71cb02c5197a9b30639e32c5730ef3d326d42`, `aa5bfb72b9233bf0155d47042f5ca001f0e3cd71c48b8d1bbf3a69ac148760bd`, `59c3f11b08e375f0e6230b52f49aa61452c84e1b923b84acabf1915215c14084`, and `28608b2c6d1c176edb760ceda1fe55d179eabb39f3da8cf3d23642e1400006b2`. |
| Exact role and semantic closure | exact sorted 14/8/7/7 dependency arrays; every pairwise role alias; constructor and decoded-wire drift matrix | pass | All artifact roles are distinct. Support variant/owning-slide and weighted/arithmetic derivation coupling are enforced. Dimension 1/65,536, token 128/129, citation 4,096/4,097 including UTF-8 bytes, extraction-layer zero, whitespace/control, uppercase hex, escaped token, missing/extra/duplicate/reordered keys, aggregation/entity drift, and the final newline are covered. |
| Escaped canonical values and privacy | quoted/backslash citation and typed `SlideId` fixed-point round trips; `Debug` sentinels | pass | Canonical escaped values decode and re-encode exactly; alternate escape spellings remain noncanonical. Model, execution, input-role, and top-level debug output disclose none of the tested declared text, slide, or artifact values. |
| Constructor and decoder resources | exact/one-short model, execution, all four top-level constructors, and encoded/decoded/retained decode budgets; 256-KiB-plus-one hostile input | red after audit, then pass | Owned constructor capacities are charged before shrinking to retained boxes. Allocation-free preflight charges inline parsed storage, an input-sized aggregate decoded-string bound, and the conservative `max(8, 2 * raw escaped-string bytes)` reusable scratch-capacity bound before Serde allocation. The independent near-limit regression failed at a 4,101-byte versus 8,198-byte delta before the capacity correction, then passed exact/one-short with a minimally escaped 4,096-byte citation and escaped typed slide ID. |
| Focused default/no-default | `cargo +1.96.0 test --locked --test multiscale_embedding_records`; `cargo +1.96.0 test --locked --no-default-features --test multiscale_embedding_records` | pass | Both configurations pass 21/21. |
| Affected package/features/docs | `cargo +1.96.0 test --locked -p marklab-embeddings --all-features`; `cargo +1.96.0 test --locked -p marklab-embeddings --no-default-features`; `RUSTDOCFLAGS='-D missing-docs' cargo +1.96.0 doc --locked --no-deps -p marklab-embeddings`; `cargo +1.96.0 check --locked --workspace --no-default-features` | pass | All-feature package: 30 library, 6 domain, and 1 hostile integration tests pass. No-default package: 14 library and 6 domain tests pass. Public documentation and the minimal workspace build are clean. |
| Full compatibility and execution order | `cargo +1.96.0 test --locked --workspace --all-features`, run alone | pass after correcting command scheduling | An initial concurrent launch alongside a no-default build violated the documented shared `target/debug/marklab` rule and caused seven WSI CLI cases to observe a non-WSI binary. The exact all-feature command rerun alone passes every executed workspace unit, integration, and doc test; root library reports 293 passed and 21 documented manual/scheduled tests ignored, while WSI reports 10 passed and its public oracle remains explicitly ignored. |
| Warning/format/semantic navigation/scope | `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`; `cargo +1.96.0 fmt --all --check`; `git diff --check`; `lsp server list`; final diff/status review | pass | Warnings, formatting, and whitespace are clean. Semantic navigation confirms no task-owned language server remains (`No servers running`). No manifest, dependency, lockfile, artifact graph/receipt, physical profile, adapter, or real-source promotion changed. |
| Independent implementation review | `c04_embedding_audit`; `c04_feasibility_audit` iterative read-only audits | pass after findings applied | Reviews first rejected missing independent digest/dependency proofs, incomplete boundary coverage, an oversized parser/serializer owner, and decoded allocations omitted from the budget. After module separation, all-variant oracles, pairwise aliases, boundary/privacy expansion, and two successive escaped-string scratch fixes (logical length, then allocator capacity), both reviewers report no remaining finding and approve only this bounded provenance-value checkpoint. Reviewers made no edits. |

## C-05 direct-patch structural artifact graph checkpoint — 2026-08-23

Implementation base: `22358e1366ae8585b0bf557d5333042ae765b228`. This checkpoint adds only exact direct-patch record/profile admission and one runtime-only structural graph token. It does not decode footprint/overlap bytes, issue physical/support/table receipts, prove source-vector component correspondence, admit a source adapter, validate derived/cell/link graphs, promote the authorized corpus, or close C-05/EMB-PATCH.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first contract | `cargo +1.96.0 test --locked --no-default-features --test multiscale_embedding_artifact_graph` | red for the expected reasons, then pass | The initial compile failed only for the intended missing `MultiscaleEmbeddingArtifactRole`, `VerifiedDirectPatchEmbeddingArtifactGraph`, and `validate_direct_patch_artifact_graph` API. The final modular target passes 11/11 in default and no-default configurations. |
| Exact direct graph and record profiles | eighteen role-preserving records; seven dependency shapes; checkpoint-content binding; Arrow/Parquet footprint and overlap manifest matrices | pass after review expansion | The graph checks exact schema/version/kind/empty semantic metadata, leaf/identity/row-link/footprint/overlap/support/provenance dependencies, and catalog-backed acyclic IDs. Both physical roles accept their exact Arrow and Parquet structural manifests and reject missing manifest, kind, format, encoding, columns, nullability, primary key, and row-count drift. Arbitrary digest-valid physical bytes intentionally pass because no physical receipt is issued. |
| Canonical managed payloads | eight same-length role drifts; zero/middle/final truncation; suffix; escaped/multibyte values; one-byte and 8,192-byte readers | pass after review correction | Every C-05-owned direct JSON value serializes directly into a fixed-buffer comparator inside `with_verified_reader`; no second payload allocation occurs. Exact final newline/EOF is required. Descriptor-consistent mismatch returns only its role, while injected reader I/O maps to redacted `StoreAccess`. |
| Availability, integrity, and privacy | catalog-only license/source; same-length/truncated/suffixed corruption; redacted error/token sentinels | pass after review correction | Review first rejected a pre-reader length shortcut because it bypassed managed verification. After removal, missing locators and corrupt managed bytes take precedence over payload mismatch, including truncated/suffixed records. Errors/tokens expose no source text, path, artifact ID, digest, or payload excerpt. |
| Functional resource evidence | generic 2-MiB comparator probe; full 4,096-row expected/source/identity/row-link/footprint/overlap/support/provenance graph | pass | The comparator never requests more than 8,192 bytes, accepts one-byte short reads, and reports injected reader failure without its source text. The high-cardinality graph reaches the actual supplied-store path and retains exact 14-dependency/output-dimension evidence. This is not Criterion, DHAT, RSS, or closure-scale evidence. |
| Focused and affected package | `cargo +1.96.0 test --locked --test multiscale_embedding_artifact_graph`; `cargo +1.96.0 test --locked --no-default-features --test multiscale_embedding_artifact_graph`; `cargo +1.96.0 test --locked --package marklab-embeddings --all-features`; `cargo +1.96.0 test --locked --package marklab-embeddings --no-default-features` | pass | Graph target: 11/11 in both configurations. All-feature embeddings package: 34 library, 6 domain, and 1 hostile integration tests pass. No-default package: 18 library and 6 domain tests pass. |
| Documentation/build/warnings | `env RUSTDOCFLAGS='-D missing-docs' cargo +1.96.0 doc --locked --no-deps --package marklab-embeddings`; `cargo +1.96.0 check --locked --workspace --no-default-features`; `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`; `cargo +1.96.0 fmt --all --check`; `git diff --check` | pass | Public documentation, minimal workspace compilation, all-target/all-feature warnings, formatting, and whitespace are clean. No manifest, dependency, lockfile, generated file, physical reader/writer, adapter, or corpus mutation changed. |
| Full compatibility | `cargo +1.96.0 test --locked --workspace --all-features`, run alone | pass | Every executed workspace unit, integration, and doc test passes. Root library reports 293 passed and 21 documented manual/scheduled tests ignored; the direct graph target passes 11/11; WSI reports 10 passed and its public oracle remains explicitly ignored. |
| Semantic navigation and cleanup | task-owned Rust LSP rooted at `/Users/user/Bench/gsc-marklab`; `lsp server list` | partial, safe fallback | The server initially started, but symbol searches returned no matches or an internal server error, so exact text search/manual inspection supplied navigation. Final server listing reports `No servers running`; no server resource remains. |
| Independent implementation review | `c04_embedding_audit`; `c04_feasibility_audit`, two read-only rounds | pass after findings applied | Reviews rejected managed-length precedence, two avoidable dependency allocations, thin canonical-role/dependency/manifest evidence, insufficient high-cardinality coverage, and an overbroad replica claim. After exact supplied-store wording, fixed arrays, every-role payload drift, all dependency/manifest matrices, corrupt truncation/suffix precedence, failing-reader mapping, 4,096 rows, and fixture/module separation, both reviewers report no remaining blocker and approve only this structural checkpoint. Reviewers made no edits. |

## C-05 footprint/overlap Arrow and Parquet physical checkpoint — 2026-08-23

Implementation base: `6609eab`. This checkpoint adds only the first two of eight physical profile families: footprint and overlap Arrow IPC/Parquet writers, bounded raw-before-stock validation, deterministic publication, and separate graph-bound runtime receipts. It is not a support or matrix-table receipt, cell-assignment/edge or patch-region physical proof, source-adapter correspondence, derived/cell/link graph, real-corpus promotion, scale closure, or C-05/EMB-PATCH completion.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first physical contract | initial `cargo +1.96.0 test --locked --features parquet --test multiscale_embedding_arrow --test multiscale_embedding_parquet --test multiscale_embedding_artifact_graph` compile | red for expected reasons, then pass | The initial contract failed only on the intended missing footprint/overlap writer, preflight, validation, publication, and receipt APIs. The final targets pass Arrow 11/11, Parquet 12/12, and graph 16/16. |
| Exact physical identities and determinism | fixed byte/digest goldens; repeated, fragmenting-sink, and fresh-child writers; empty/1/8,192/8,193 boundaries | pass | Arrow footprint/overlap SHA-256 values are `ab4e2b3599066f7b3fedabc8033ce64a4dbccb3f53ff6112ff6204adf47775de` and `cf05c10ba0c280c98009935b49eefb573aee5b81dc20817affa8b86745cd3b16`; Parquet values are `9ff3c9c135ea3160b4e96384b81dbd18ec7a53ca8213d79a1ef9b37ead43d862` and `1ce5574426a54c708c395c0e26aea1235772fa208ad0a63efd0167e2cc721f25`. Exact schema IDs, sorted metadata, roots, kinds, encodings, dependencies, row counts, batch/group boundaries, negative Reflect origins, and empty overlap graphs are covered. |
| Raw-before-stock and hostile profiles | C-05-specific unit guards plus `spatial_arrow_raw_preflight_rejects_header_and_canonical_row_drift`, `spatial_parquet_raw_preflight_rejects_magic_metadata_and_row_drift`, and row-group precedence regression | pass after review expansion | Arrow directly rejects dictionaries, compression, variadic buffers, declared features, field dictionaries/extra fields, table-heavy Footer/Message values, oversized footer, block/node/buffer/padding/validity drift, and wrong canonical rows. Parquet directly rejects annotation/extra-column drift, compression, dictionary encoding/page, indexes, statistics, footer/page-header/page/range excess, compact-wire/magic/metadata drift, and wrong canonical rows. A one-short row-group budget wins before malformed-page read/allocation. |
| Independent writer and full-reader resource oracles | private writer scalar formulas; public `spatial_{arrow,parquet}_full_readers_have_exact_group_and_retained_edges`; three-batch/group decode tests | pass after review corrections | Both writers enforce domain/file/decoded/group/retained limits before schema/batch allocation with exact/one-short footprint and overlap cases. Independent integration formulas parse the emitted files and reproduce raw-preflight plus stock-decode peaks; full public `validate_*_bytes` passes exact and fails one short for group and retained budgets in both roles/formats. The 16,386-footprint/16,385-edge fixtures fully decode three batches/groups, including a middle group. |
| Publication, managed integrity, and receipt authority | publication/managed tests plus `tests/multiscale_embedding_artifact_graph/physical_receipts.rs` | pass | Borrowed and managed validation agree. Managed pre/post integrity failure takes precedence over decode errors and remains source-private. A structural graph or placeholder bytes cannot mint a physical receipt; footprint and overlap receipts require full decode, and an overlap receipt rejects a format-distinct footprint receipt. Publications use exact records/dependencies and deterministic managed bytes. |
| Focused feature matrix | `cargo +1.96.0 test --locked -p marklab-embeddings --features parquet`; same with `--no-default-features`; `cargo +1.96.0 test --locked --no-default-features --test multiscale_embedding_artifact_graph` | pass | Parquet-feature package: 42 library, 6 domain, and 1 hostile integration tests. No-default package: 18 library and 6 domain tests. Structural graph without physical features: 11/11. |
| Full compatibility | `cargo +1.96.0 test --locked --workspace --all-features`, run alone | pass | Every executed workspace unit, integration, and doc test passes. Root library reports 293 passed and 21 documented manual/scheduled tests ignored; all-feature embeddings reports 44 library, 6 domain, and 1 hostile test; Arrow 11/11, Parquet 12/12, graph 16/16, and WSI 10/10 with its public oracle explicitly ignored. |
| Clean package characterization | `cargo +1.96.0 package --locked --workspace` | known exit 101; deliberately non-green | All six archives are created and `marklab-core` verifies. Normalized `marklab-data` then resolves the published pre-C-02 `marklab-core 0.1.0` and fails on the missing coordinate identity API. This exactly reproduces DEC-0018; no `--allow-dirty`, patch, version, or publication workaround is claimed. |
| Docs, features, warnings, and formatting | `env RUSTDOCFLAGS=-Dmissing-docs cargo +1.96.0 doc --locked --no-deps -p marklab-embeddings --features parquet`; `cargo +1.96.0 check --locked --workspace --no-default-features`; exact package and final workspace all-target/all-feature Clippy with `-D warnings`; `cargo +1.96.0 fmt --all -- --check`; `git diff --check` | red during final review, then pass | Public docs and minimal compilation pass. One final review caught an Arrow test module before production helpers; moving the module to the file end fixed `clippy::items-after-test-module`. The exact previously failing package command and final workspace Clippy pass. Formatting/whitespace are clean. No manifest, dependency, lockfile, generated file, adapter, or corpus mutation changed. |
| Independent implementation review | `c04_embedding_audit`; `c04_feasibility_audit`, iterative read-only rounds | pass after findings applied | Reviews rejected late Parquet row-group enforcement, missing Arrow footer bound, incomplete writer memory categories/order, insufficient full-reader exact edges, footprint-only fresh-process/multi-group evidence, thin hostile-profile coverage, and the final Clippy ordering defect. After two-pass footer validation, corrected formulas/order, both-role public reader edges, both-role three-group/fresh-process tests, direct C-05 hostile guards, and the ordering fix, both reviewers report no remaining checkpoint blocker. Reviewers made no edits. |
| Partial-checkpoint claim ceiling | implementation/reviewer scope audit | partial by design | Only footprint and overlap physical families and receipts are active. Six physical families, support/matrix/link receipts, remaining graphs, C-05 fuzz routes, cross-family differential cases, Criterion/DHAT/RSS/scale evidence, adapter correspondence, real-corpus promotion, and C-05/EMB-PATCH closure remain open. |

## C-05 vector-independent cell-patch input graph checkpoint — 2026-08-23

Implementation base: `44b454669ed259216f234fa0d3ab6d7419f25f2c`. This checkpoint adds only a dedicated nine-role input graph for later cell-assignment/edge persistence. It does not write or validate assignment/edge files, issue their pair receipt, prove opaque coordinate-to-anchor correspondence, bind patch-vector bytes, admit a source adapter, promote the authorized corpus, or close C-05/EMB-PATCH.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first graph contract | initial `cargo +1.96.0 test --locked --features parquet --test cell_patch_input_artifact_graph` compile, then exact reruns | red for the expected reason, then pass | The first compile failed only for the intended missing `validate_cell_patch_input_artifact_graph` API. The final focused target passes 4/4. |
| Exact vector-independent graph | nine distinct roles; exact record/profile/dependency and domain-binding cases | pass | Expected cells/patches, context, footprints, producer, opaque source coordinates, run config, environment, and converter must be distinct. Every record has its exact schema/version/kind, empty semantic metadata, table shape, and dependency set. Producer ID/content/mode and all link support bindings are exact; no vector artifact is accepted. |
| Canonical payload and physical proof | same-profile expected-cell/patch/context/producer drift; full Arrow footprint validation | pass | Managed readers byte-compare the reused C-04 expected-cell binary and all three C-05 JSON values. Expected-patch/context/producer drift reports its exact role. The footprint role is not merely structural: its Arrow or Parquet bytes undergo the existing full raw-before-stock managed decode before the token is issued. |
| Streaming/resource/privacy | 4,096 expected cells under 1-, 3-, and 8,192-byte reads; zero/middle/final truncation; suffix/drift; injected I/O | pass | Expected-cell comparison uses constant 8-KiB scratch with checked encoded length and exact EOF. Truncation/suffix/value drift is a payload mismatch; reader failure is a redacted availability category. Token `Debug` and errors expose only mode, aggregate counts, role, and category—not IDs, source values, paths, digests, coordinates, or credentials. |
| Managed availability and precedence | catalog-only opaque coordinates; same-length corrupt managed footprint | pass | Catalog presence is not availability evidence. Missing managed evidence fails by role, and store integrity takes precedence over the physical callback. The opaque source remains uninterpreted and explicitly proves no anchor correspondence. |
| Focused package/features/docs | `cargo +1.96.0 test --locked -p marklab-embeddings --all-features`; same with `--no-default-features`; `env RUSTDOCFLAGS=-Dmissing-docs cargo +1.96.0 doc --locked --no-deps -p marklab-embeddings --features parquet`; `cargo +1.96.0 check --locked --workspace --no-default-features` | pass | All-feature embeddings passes 47 library, 6 domain, and 1 hostile test; no-default passes 21 library and 6 domain tests. Public docs and the minimal workspace compile are clean. |
| Warning/format/scope | package all-feature and no-default all-target Clippy, final workspace all-target/all-feature Clippy with `-D warnings`; `cargo +1.96.0 fmt --all -- --check`; `git diff --check`; final diff/status review | red under independent review, then pass | Feasibility review caught feature-independent comparator helpers whose only production caller was Parquet-gated. Narrow `cfg` ownership—not a lint suppression—keeps unit coverage while making the exact failing no-default Clippy command green. All final warning, formatting, and whitespace gates pass; no manifest, dependency, lockfile, generated file, source adapter, or physical assignment/edge profile changed. |
| Full compatibility | `cargo +1.96.0 test --locked --workspace --all-features`, run alone | pass | Every executed workspace unit, integration, and doc test passes. Root library reports 293 passed and 21 documented manual/scheduled tests ignored; the new graph passes 4/4; all-feature embeddings reports 47 library, 6 domain, and 1 hostile test; WSI reports 10 passed and its public oracle remains explicitly ignored. |
| Independent implementation review | `c04_embedding_audit`; `c04_feasibility_audit`, iterative read-only rounds | pass after finding applied | Both reviewers approved semantics, bounded streaming, managed-integrity precedence, and privacy. Feasibility review first rejected the no-default dead-code failure and noted thin payload/profile evidence; after exact `cfg` gating plus expected-patch/context/producer payload and footprint dependency/schema regressions, both independently report no remaining blocker. Reviewers made no edits. |
| Partial-checkpoint claim ceiling | implementation/reviewer scope audit | partial by design | This token is only the prerequisite for later assignment and edge physical proofs. Those two formats, their separate receipts and pair receipt, the other four pending physical families, derived/link graphs, fuzz, scale/DHAT/RSS evidence, adapter correspondence, real-corpus promotion, and C-05/EMB-PATCH closure remain open. |

## C-05 cell-patch assignment/edge Arrow and Parquet physical checkpoint — 2026-08-23

Implementation base: `fb96f8bd7fa0c33e415bef849e6c8f86c00dade8`. This checkpoint adds only the cell-assignment and cell-edge Arrow IPC/Parquet families, separate graph-bound runtime receipts for each physical half, and one format-neutral receipt for their exact pair. It does not prove opaque coordinate-to-anchor correspondence, bind patch-vector bytes, admit a source adapter, implement any matrix or patch-region physical family, promote the authorized corpus, or close C-05/EMB-PATCH.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first physical contract | initial focused integration compile, then `cargo +1.96.0 test --locked --features parquet --test cell_patch_columnar --test cell_patch_columnar_adversarial --test cell_patch_columnar_resources --test cell_patch_input_artifact_graph` | red for the expected reasons, then pass | The initial contract exposed the intended missing assignment/edge physical and receipt APIs. The final targets pass 7/7 behavior, 5/5 hostile-profile, 3/3 independent-resource, and 7/7 graph/receipt cases. |
| Exact profiles and application metadata | both modes; Arrow and Parquet; exact schema/version/kind/encoding/table manifests; eleven metadata entries; five sorted dependencies | pass | Assignment and edge rows retain exact counts, columns, nullability, primary keys, mode/status wire values, link identity, producer and support bindings, digests, and empty semantic metadata. Tests independently parse and compare exact application metadata in all four mode/format profiles rather than accepting writer output as its own oracle. |
| Raw-before-stock hostile profiles | direct Arrow footer/message/schema/node/buffer/validity/filler mutations; direct Parquet compact footer/schema/annotation/codec/encoding/page/index/statistics/definition-level/range mutations | pass | Both formats reject metadata, row/status/ID, schema, optional-value, prefix, and declared-resource drift before stock decode. Arrow rejects compression and variadic buffers in a private synthetic FlatBuffer regression. Parquet validates both optional definition streams, nullable counts, modern/legacy `u64` annotations, and one-row-group window bounds. |
| Independent writer and reader resource oracles | `tests/cell_patch_columnar_resources.rs`; exact/one-short file, decoded, group, retained limits; 8,192/8,193/16,386 rows | pass after review correction | Independent formulas cover fourteen assignment and nine edge Arrow buffers; Parquet plain values, optional definition levels, page overhead, workspace, mode-dependent payloads, six/four-column cached metadata, and footer replay. Production metadata estimation was generalized by exact column count while the existing three-column wrapper stayed unchanged. Both formats fully decode three batches/groups including the middle group. This is functional resource evidence, not a benchmark. |
| Publication, readers, and integrity precedence | borrowed/managed full validation, idempotent publication, fragmenting sinks, malformed callback parity, managed-byte corruption | pass | Deterministic writers publish only exact managed records. Borrowed and managed readers agree, and content-integrity failure remains the exact `VerifiedReaderError::Store(ArtifactStoreError::ContentIntegrity { .. })` category before malformed decode callbacks. |
| Receipt authority and mixed-format pairing | separate assignment/edge receipt entry points plus `VerifiedCellPatchLinkArtifact::from_verified_halves` | pass | A nine-role input graph token and full physical decode are required for each half. Pairing rejects wrong graph tokens, different logical links, dependency/mode/digest/count drift, aliases, and non-distinct physical IDs. All four Arrow/Arrow, Arrow/Parquet, Parquet/Arrow, and Parquet/Parquet combinations succeed because the pair contract is intentionally format-neutral. |
| Determinism | repeated writers, fragmenting sinks, and fresh child processes for both formats and both modes | pass | Each emitted byte stream and content digest is stable within and across processes; child-process helpers are excluded from the parent harness except when explicitly selected. |
| Focused and library gates | focused 22-test command above; `cargo +1.96.0 test --locked -p marklab-embeddings --features parquet --lib` | pass | The four focused targets pass 22/22. The Parquet-feature embedding library passes 46/46, including the private Arrow feature guard. |
| Docs, minimal features, warnings, and formatting | `env RUSTDOCFLAGS=-Dmissing-docs cargo +1.96.0 doc --locked --no-deps -p marklab-embeddings --features parquet`; `cargo +1.96.0 check --locked --workspace --no-default-features`; `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`; `cargo +1.96.0 fmt --all --check`; `git diff --check` | pass | Public documentation, warning-free minimal compilation, all-target/all-feature Clippy, formatting, and whitespace pass. A final no-default warning was removed with exact `parquet` ownership rather than suppression. No manifest, dependency, lockfile, generated file, adapter, or corpus mutation changed. |
| Full compatibility | `cargo +1.96.0 test --locked --workspace --all-features`, run alone | pass | Every executed workspace unit, integration, and doc test passes. Root library reports 293 passed and 21 documented manual/scheduled tests ignored; all-feature embeddings reports 48 passed; the new targets pass 7/5/3/7; WSI reports 10 passed and its public oracle remains explicitly ignored. |
| Independent implementation review | `c04_embedding_audit`; `c04_feasibility_audit`, iterative read-only rounds | pass after findings applied | Reviews rejected incomplete exact-manifest/integrity precedence coverage, thin independent writer oracles, and a three-column-only Parquet metadata estimate. After exact literal/profile assertions, managed corruption precedence, both-mode and large-shape independent formulas, and column-aware metadata charging, both reviewers report no remaining checkpoint blocker. Reviewers made no edits. |
| Partial-checkpoint claim ceiling | implementation/reviewer scope audit | partial by design | Four of eight physical families are active. The three matrix families, patch-region family, support/matrix/patch-region receipts, remaining derived graphs, C-05 fuzz routes, cross-family differential cases, Criterion/DHAT/RSS/scale evidence, adapter correspondence, real-corpus promotion, and C-05/EMB-PATCH closure remain open. |

## C-05 patch-region Arrow and Parquet physical checkpoint — 2026-08-23

Implementation base: `8977faa2f63b71d3ec4685a848d0e25c5344ff5c`. This checkpoint adds only the patch-region Arrow IPC/Parquet family, a dedicated six-role input graph, and one graph-bound runtime receipt. It persists producer-declared exhaustive assessment output; it does not prove region geometry, bind source vectors, admit a source adapter, implement a matrix family, promote the authorized corpus, or close C-05/EMB-PATCH.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first physical contract | initial `cargo +1.96.0 test --locked --features parquet --test patch_region_columnar`; initial `cargo +1.96.0 test --locked --features parquet --test patch_region_input_artifact_graph`; final four-target focused command | red for the expected reasons, then pass | The first compile failed only for the intended missing write/preflight APIs; the graph compile failed only for the intended missing graph method. The final behavior, hostile-profile, independent-resource, and graph/receipt targets pass 8/8, 3/3, 3/3, and 8/8. |
| Exact profiles and metadata | both formats; exact schema/version/kind/encoding/table manifest; thirteen sorted metadata entries; six sorted dependencies | pass | Five non-null columns preserve patch/region/relation plus full-width unsigned numerator/denominator. Exact owning-slide, expected sets, context, footprint, converter, assessment, policy, assessed count, logical digest, schema, and encoding bindings are parsed independently and reject extras, omissions, aliases, or alternate values. |
| Raw-before-stock hostile profiles | direct Arrow footer/message/schema/node/buffer/validity/padding/numeric mutations; direct Parquet compact footer/schema/annotation/codec/encoding/page/index/statistics/encoding-statistics/column-order/range mutations | pass after review expansion | Both formats reject canonical row and schema drift before stock decode. Regressions include high-bit `u64` values, direct numerator/denominator drift, Arrow zero padding, Parquet modern/legacy unsigned annotations, required encoding statistics and column order, forbidden compression/dictionary/statistics/indexes, and truncation/range excess. |
| Independent writer and reader resource oracles | `tests/patch_region_columnar_resources.rs`; exact/one-short file, decoded, group, retained limits; 8,192/8,193/16,386 rows | pass | The Arrow oracle independently covers five validity buffers, three offset/value pairs, two `u64` arrays, thirteen buffers, alignment, footer, blocks, and workspace. The Parquet oracle covers `28 * rows + text`, five columns, pages, encoder, cached metadata, footer replay, compact-Thrift state, and one copied row-group window. Both formats fully decode three batches/groups including the middle group. This is functional resource evidence, not a benchmark. |
| Six-role graph and canonical evidence | all role profile/dependency/content-kind branches; non-table/table metadata drift; missing/corrupt evidence; canonical expected/context/assessment payloads; full Arrow/Parquet footprint decode | pass after review expansion | Exact expected-patch, expected-region, context, footprint, converter, and assessment roles are distinct. Fixed-buffer comparison requires exact payload bytes/newline/EOF; converter content binding and managed integrity precede decoding; footprint manifests and bytes are fully verified. A private duplicate-ID regression covers the role-alias defense that safe logical constructors cannot otherwise express. |
| Publication, readers, receipt, and determinism | repeated and fragmenting writers; fresh child processes; borrowed/managed validation; malformed callback parity; integrity precedence; wrong-graph receipt | pass | Empty sparse links with positive assessed counts and high-bit fractions round-trip in both formats. Deterministic bytes and digests agree across processes. Receipt issuance requires the exact six-role token and full link decode, binds graph/link/converter/assessment/digest/count values, rejects a token for another link, and exposes neither IDs nor values through `Debug`. |
| Focused implementation and review | focused four-target command; `c04_embedding_audit`; `c04_feasibility_audit` | pass after findings applied | Delegated Arrow and Parquet implementation stayed within recorded disjoint file ownership. Final read-only rounds rejected missing empty/high-bit cases and incomplete hostile/profile/graph branches; after the exact regressions above, both reviewers approved with no remaining blocker or high-severity finding. |
| Focused package/features/docs | `cargo +1.96.0 test --locked -p marklab-embeddings --features parquet --lib`; `cargo +1.96.0 test --locked -p marklab-embeddings --no-default-features`; `env RUSTDOCFLAGS=-Dmissing-docs cargo +1.96.0 doc --locked --no-deps -p marklab-embeddings --features parquet`; `cargo +1.96.0 check --locked --workspace --no-default-features` | pass | The Parquet-feature embedding library passes 48/48, including the private role-alias and Arrow feature guards. Minimal features pass 21 library plus 6 domain tests. Public documentation and warning-free minimal workspace compilation are clean. |
| Warning/format/scope | package all-target Clippy in `parquet` and no-default modes, then workspace all-target/all-feature Clippy, each with `-D warnings`; `cargo +1.96.0 fmt --all --check`; `git diff --check`; final diff/status review | pass | All warning-denied Clippy configurations, formatting, and whitespace pass. Final review contains only the scoped source, tests, exports, and implementation ledgers; no manifest, dependency, lockfile, generated file, source adapter, or corpus mutation changed. |
| Full compatibility | `cargo +1.96.0 test --locked --workspace --all-features`, run alone | pass | Every executed workspace unit, integration, and doc test passes. Root library reports 293 passed and 21 documented manual/scheduled tests ignored; all-feature embeddings reports 50 passed; the new targets pass 8/3/3/8; WSI reports 10 passed and its public oracle remains explicitly ignored. |
| Partial-checkpoint claim ceiling | implementation/reviewer scope audit | partial by design | Five of eight physical families are active. The three matrix families, support/matrix receipts, remaining derived graphs, C-05 fuzz routes, cross-family differential cases, Criterion/DHAT/RSS/scale evidence, adapter correspondence, geometric patch-region proof, real-corpus promotion, and C-05/EMB-PATCH closure remain open. |

## C-05 patch/region/slide matrix Arrow and Parquet physical checkpoint — 2026-08-23

Implementation base: `d53bf3bc17fb584e7612bb89908dadaacd52b167`. This checkpoint adds the final three of eight physical profile families: patch, region, and slide embedding matrices in Arrow IPC and Parquet. It deliberately adds no support graph, matrix graph/receipt, materializing columnar read API, source-vector correspondence proof, derived finalization, source adapter, real-corpus promotion, or C-05/EMB-PATCH closure.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first contracts | initial focused logical alias test; initial `cargo +1.96.0 test --locked --features parquet --test multiscale_matrix_columnar`; final four-target matrix command | red for expected reasons, then pass | The logical red failed only for the missing duplicate dependency-role error. The physical red failed only for the intended typed write/preflight/validate/publish APIs. The final logical, behavior, hostile-profile, and resource targets pass 12/12, 10/10, 3/3, and 3/3. |
| Typed shared ownership and exact records | three sealed typed wrappers; exact Arrow/Parquet schemas/manifests; nine sorted metadata entries; three sorted distinct dependencies | pass | Patch, region, and slide retain distinct typed IDs, schema IDs, roots, content kinds, encoding IDs, logical domains, owning slide, expected-set/support/provenance bindings, dimension, status rows, QC, and logical digest. Expected/support/provenance role aliasing is rejected before matrix allocation, and dependency drift is rejected before physical decode. C-04 cell types/codecs were not generalized into this core. |
| Exact bytes and determinism | repeated and fragmenting sinks; fresh child processes; six fixed length/SHA-256 goldens | red placeholder, then pass | Arrow lengths/digests are patch `3394` / `5db36dbb4b430dc13bf20b4674d7d54489a08183ec34436c101f472090c7926e`, region `3394` / `70802935d5585917d7607609721bf99a2da39763de6c816a516bb91281404fa4`, and slide `3394` / `fadc6f04d785a619156b144e988fe83bed75ae204c3abc8cd66c4de510e5d0c4`. Parquet values are patch `1224` / `93dba0b4f55c93d3870aac853cff1c4aebfe7b9d443f93886c204e3ea051de1a`, region `1133` / `68f71780e528de67214dc4d707f669a7c4bbc9ba9b65c2ea9e283735497725db`, and slide `1096` / `644856f69f0fa323fb20c39ebc97f9afd63ad47489eb97209e02b9e43d8959a4`. |
| Raw-before-stock hostile profiles | direct Arrow and Parquet magic/schema/metadata/row/status/vector/filler/validity/level/padding/feature mutations plus a private Arrow feature guard | red for nonzero message padding, then pass | Arrow validates bounded Footer, initial Schema Message, each RecordBatch Message, exact 4 nodes/9 buffers, all-ones validity including unused tails, canonical positive-zero non-present fillers, body and message padding, and absent dictionary/compression/features/message metadata/variadic buffers before `FileReader`. The audit reproduced acceptance of nonzero trailing Schema/RecordBatch message padding; bounded expected-message-length reconstruction now rejects both without whole-message replay. Parquet validates canonical bounded compact-Thrift footer/pages, exact nested LIST/`element`, annotations, page V2/PLAIN/RLE levels, encoding statistics/type order, and forbidden compression/dictionary/statistics/index features before stock metadata/Arrow decoding. |
| Independent writer/full-reader resource oracles | `tests/multiscale_matrix_columnar_resources.rs`; exact/one-short file, decoded, group, retained budgets; 8,192/8,193/16,386 rows | pass | The Arrow oracle independently charges three row validity buffers, component validity, two offset vectors, ID/status text, `4*r*D` values, nine-buffer alignment, blocks/footer, bounded message reconstruction, and stock batch workspace. The Parquet oracle charges exact plain values, four fixed-list level bounds, three-column page overhead, compact metadata/footer state, one copied row-group window, stock metadata, and decoded group output. Every budget category passes exactly and fails one byte short; both formats fully decode one, two, and three groups including the middle group. |
| Boundary, stock, publication, and integrity paths | empty/max dimension and batch boundaries; one row × 65,536; all six publications; borrowed/managed malformed parity; managed corruption | pass | Empty patch/region tables at maximum dimension remain structural. A one-row 65,536-dimensional patch table reaches both managed stock decoders. Publications are idempotent and carry exact records. Digest-matching malformed rows yield the same borrowed and managed callback error, while subsequent on-disk corruption yields store `ContentIntegrity` before decoder callbacks. No runtime receipt is minted. |
| Focused compatibility | `cargo +1.96.0 test --locked --features parquet --test multiscale_embedding_tables --test multiscale_matrix_columnar --test multiscale_matrix_columnar_adversarial --test multiscale_matrix_columnar_resources --test cell_embedding_arrow --test cell_embedding_parquet` | pass | Logical/matrix/adversarial/resource targets pass 12/10/3/3; unchanged C-04 Arrow and Parquet targets pass 14/14 and 10/10, including their original goldens and fresh-process behavior. |
| Docs, features, warnings, formatting | package/root Parquet-feature all-target Clippy with `-D warnings`; strict package docs; no-default package and logical-table tests; no-default workspace check; final workspace all-target/all-feature Clippy; `cargo +1.96.0 fmt --all --check`; `git diff --check` | pass | Public docs, narrow/default-free compilation, 21 package unit plus 6 domain tests, 12 logical-table tests, warning-denied checks, formatting, and whitespace are clean. No manifest, dependency, lockfile, generated file, adapter, corpus, CLI/config/result, or C-04 wire behavior changed. |
| Full compatibility | `cargo +1.96.0 test --locked --workspace --all-features`, run alone | pass | Every executed workspace unit, integration, and doc test passes. Root reports 293 passed and 21 documented ignores; `marklab-embeddings` reports 51 unit tests plus 6 domain tests; the matrix targets pass 10/3/3; WSI reports 10 passed with one public-oracle test explicitly ignored. |
| Independent implementation/re-review | disjoint Arrow `c04_embedding_audit` and Parquet `c04_feasibility_audit` implementation; repeated read-only reviews after root evidence and padding fix | pass | Recorded disjoint ownership prevented concurrent symbol/file overlap. Reviews independently confirmed exact formats, compact limits, row-group windows, resource formulas, managed integrity, public exports, C-04 compatibility, message-padding allocation bounds, and the 65,536-dimension stock path. No remaining blocker/high finding; reviewers made no final edits. |
| Partial-checkpoint claim ceiling | implementation/reviewer scope audit | partial by design | All eight physical families exist, but the three matrix APIs prove bytes only against supplied sealed logical tables. Support verification, direct/derived matrix graphs and receipts, derivation finalization, source correspondence, cross-family/fuzz/scale/DHAT/RSS evidence, real-corpus promotion, and C-05/EMB-PATCH closure remain open. |

## C-05 direct-patch support and matrix receipt checkpoint — 2026-08-23

Implementation base: `18de846f736f16913727c37557c65707d05bfb14`. This checkpoint composes the already verified direct graph, footprint, overlap, and matrix profiles into runtime-only patch-support and direct-patch table receipts. It does not prove opaque source-vector component correspondence, admit a source adapter, construct derived region/slide graphs, finalize derived values, promote real corpus data, or close C-05/EMB-PATCH.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first public contract | `cargo +1.96.0 test --locked --all-features --test multiscale_embedding_artifact_graph matrix_receipts --no-fail-fast` before and after implementation | red for expected reason, then pass | The red run failed only for the intended missing support receipt, table receipt, and four verifier exports. The final focused module passes 10/10. |
| Patch-support receipt authority | exact direct graph plus independently fully decoded footprint and overlap receipts; same-format, mixed-format, and cross-graph cases | pass after review expansion | `VerifiedPatchEmbeddingSupportArtifact` is unforgeable outside the crate and requires every graph/footprint/overlap artifact and logical binding. All four Arrow/Parquet footprint/overlap combinations can compose when the overlap depends on that exact footprint artifact; a format-distinct receipt from another graph is rejected. |
| Direct table receipt authority | full Arrow/Parquet raw-before-stock validation followed by source-row and graph binding; borrowed and managed entry points | pass | `VerifiedPatchEmbeddingTableArtifact` binds physical artifact ID, expected set, support, provenance ID and logical digest, source-row link, dimension, logical digest, and recomputed QC. Finalization checks graph-bound row-link identity/count and exact expected-order PatchId/status equality before table dimension/support/provenance/logical comparisons. The physical artifact cannot alias any of its three direct dependencies. |
| Status, boundary, and format matrix | all four statuses; zero rows; 8,193 rows; one row × 65,536 dimensions; Arrow/Parquet matrices over Arrow/Parquet support | pass | Present, missing-vector, extraction-failed, and QC-rejected states agree exactly with the source-row link. Empty direct chains remain valid, the public Arrow batch boundary is crossed, the maximum dimension reaches managed Parquet stock decode, and support/table physical formats remain independent. |
| Failure precedence and privacy | malformed borrowed Arrow with binding drift; same-length corrupt managed artifact with binding drift; wrong expected/support/provenance/dimension/status cases; manual Debug sentinels | pass | Physical errors precede receipt checks, and managed corruption returns exact store `ContentIntegrity` even when receipt bindings also drift. Binding failures are one redacted category; receipt Debug shows only aggregate row/dimension values and exposes no slide/source/vector/path/artifact identity. |
| Provenance forgery regression | wrong table provenance logical digest under the correct provenance artifact ID | red during design audit, then pass | The direct graph token now retains the logical digest of the exact canonical managed provenance it validated. Receipt construction compares both provenance ID and logical digest, closing the previously missing binding. |
| Focused compatibility | `cargo +1.96.0 test --locked --all-features --test multiscale_embedding_artifact_graph --no-fail-fast`; matrix behavior/adversarial/resource three-target command | pass | The complete graph target passes 26/26. Unchanged matrix targets pass 10/10, 3/3, and 3/3. |
| Docs, feature, warning, format gates | strict package docs; no-default package Clippy; no-default workspace check; workspace all-target/all-feature Clippy, both with `-D warnings`; `cargo +1.96.0 fmt --all --check`; `git diff --check` | pass | Public docs and both feature configurations are warning-clean. No manifest, dependency, lockfile, generated file, source adapter, corpus, CLI/config/result, or C-04 wire behavior changed. |
| Full compatibility | `cargo +1.96.0 test --locked --workspace --all-features`, run alone | pass | Every executed workspace unit, integration, and doc test passes. Root reports 293 passed and 21 documented ignores; `marklab-embeddings` reports 51 unit tests plus 6 domain tests; the graph target passes 26/26; WSI reports 10 passed with one public-oracle test explicitly ignored. |
| Independent implementation review | `c04_embedding_audit`; `c04_feasibility_audit` read-only reviews and focused re-review | pass after one medium and one low finding applied | Security review approved the closed capability chain. Feasibility review required the frozen source-row-first finalization order and positive mixed footprint/overlap format coverage. After both corrections, both reviewers report no remaining finding; reviewers made no edits. |
| Partial-checkpoint claim ceiling | implementation/reviewer scope audit | partial by design | Direct patch support/table receipts exist, but source components remain opaque and unproved. Region/slide support receipts, derived graphs/finalization/table receipts, fuzz/scale/DHAT/RSS evidence, real-corpus promotion, and C-05/EMB-PATCH closure remain open. |

## C-05 region-from-patches support and derived graph checkpoint — 2026-08-23

Implementation base: `8f38969cb5c51b46be877bad8c5e96827ccee929`. This checkpoint composes existing lower receipts into managed region-support and nine-role derived-region provenance capabilities. It deliberately contains no region-vector recomputation, output table, physical region-table receipt, geometry proof, source adapter, real-corpus promotion, or C-05/EMB-PATCH closure claim.

| Evidence | Command or method | Result | Notes |
|---|---|---|---|
| Behavior-first authority regression | focused `region_support_receipt_rejects_individually_valid_cross_lineage_receipts` before and after the fix | red `MissingRecord { RegionSupport }`, then pass with `DomainBindingMismatch { RegionSupport }` | An independently valid Arrow patch-support receipt and Parquet-footprint/link receipt share the same expected-patch identity but differ in exact footprint lineage. Support verification now rejects exact context or footprint artifact divergence before catalog access. |
| Complete graph/receipt integration target | `cargo +1.96.0 test --locked --all-features --test multiscale_embedding_artifact_graph --no-fail-fast` | pass, 42/42 | Sixteen derived-region cases cover all four patch-table/link format combinations, all source statuses, empty and 8,193-row boundaries, variant/binding/cross-fixture drift, each record/dependency layer, canonical payloads, catalog-only and corrupt managed records for every role, and privacy-redacted errors; the prior 26 direct graph/receipt cases remain green. |
| Physical profile/dependency adversarial unit cases | `cargo +1.96.0 test --locked -p marklab-embeddings --all-features --lib --no-fail-fast` | pass, 55/55 | Four new unit cases accept both formats and reject source-table/link schema, content kind, semantic metadata, missing/wrong-format/wrong-row/wrong-dimension manifests, and dependency drift. |
| Full workspace compatibility | `cargo +1.96.0 test --locked --workspace --all-features --no-fail-fast`, run alone | pass | Every executed unit, integration, and doc test passes. Root reports 293 passed and 21 documented ignores; `marklab-embeddings` reports 55 unit tests plus 6 domain tests; the combined graph target passes 42/42; WSI reports 10 passed with one checksummed public-oracle case explicitly ignored. |
| Feature/warning/document gates | no-default package Clippy; no-default workspace check; all-feature package and workspace Clippy with `-D warnings`; strict package docs; `cargo +1.96.0 fmt --all --check`; `git diff --check` | pass | The private receipt projections are gated at the physical feature boundary; public receipt types remain available without default features. No manifest, dependency, or lockfile changed. |
| Independent security and feasibility review | `c04_embedding_audit`; `c04_feasibility_audit`, then focused re-review | pass after findings applied | Security review found and then approved the closed context/footprint lineage join. Feasibility review required adversarial physical-profile evidence and a cohesive test split; both are present. Reviewers made no edits. |
| Claim ceiling | implementation and dual-review scope audit | partial by design | The graph proves exact declared support/provenance authority only. It does not recompute weighted means, validate a claimed region table, prove geometry/tissue/window semantics, prove opaque source-vector correspondence, authorize either slide path, or close C-05/EMB-PATCH. |

## C-05 deterministic region finalization and physical receipt milestone — 2026-08-23

Implementation base: `0a6cc4deb04cf855b377ed195d093aaae8965f0c`. This milestone deterministically recomputes region vectors from the verified source patch table and declared exhaustive link, then requires complete candidate-bound Arrow or Parquet validation before issuing a runtime receipt. It adds no geometry proof, source-component correspondence, real-corpus promotion, slide authority, or C-05/EMB-PATCH closure claim.

| Evidence | Command or method | Result | Notes |
|---|---|---|---|
| Behavior-first finalizer and receipt boundaries | focused compile-red tests for the missing finalizer and public receipt exports; focused scalar golden red/green; final `cargo +1.96.0 test --locked --all-features --test multiscale_embedding_artifact_graph` | red for expected missing behavior, then pass 52/52 | Seven finalizer cases cover frozen weighted bits, order-sensitive cancellation/positive zero, all non-present statuses, empty source/regions, dimensions 1/65,536, graph mixing before budgets, exact work/memory edges, and aggregate-only candidate Debug. Three receipt cases cover Arrow/Parquet borrowed/managed parity, exact lineage, candidate mismatch, malformed bytes, managed integrity precedence, and redacted Debug. |
| Frozen scalar and ordering semantics | private large-`u64` fraction golden plus end-to-end weighted and cancellation goldens | pass | Fractions cast numerator and denominator separately to `f64` before division; each promoted component is multiplied separately and added sequentially in canonical link/component order, divided once, cast once, and signed-zero canonicalized. No present contributor produces `MissingVector`; non-present source categories never propagate. |
| Explicit resource closure | contributor/component and retained/working exact-edge integration oracles | pass | Binding checks precede zero budgets. Sparse relations and present component operations have independent maxima; peak checks cover live inputs, graph authority, `R × (D + 1)` `f64` scratch, materialized rows/vectors, and table-construction coexistence. The no-default Clippy gate exposed private graph retention outside its physical feature owner; feature-gated retention and accounting corrected it. |
| Candidate-only physical authority | `DerivedRegionEmbeddingTableCandidate`; four region verifier entry points; exact receipt getter assertions | pass | Arbitrary `RegionEmbeddingTable` values can use existing validation but cannot mint `VerifiedRegionEmbeddingTableArtifact`. The receipt snapshots output artifact/logical/QC plus expected-region, support, provenance, source-patch-table, patch-region-link, derivation, and dimension identities without retaining vectors or bytes. |
| Focused package compatibility | `cargo +1.96.0 test --locked --all-features -p marklab-embeddings` | pass | Embeddings reports 56 unit tests, 6 domain tests, 1 hostile-row-link test, and doc tests green. |
| Feature, documentation, warning, and format gates | no-default package Clippy; no-default workspace check; strict all-feature package docs; workspace all-target/all-feature Clippy, both Clippy commands with `-D warnings`; `cargo +1.96.0 fmt --all`; `git diff --check` | pass after the feature-boundary correction | Public docs, default-free compilation, all-feature targets, formatting, and whitespace are clean. No manifest, dependency, lockfile, generated file, adapter, corpus, CLI/config/result, or wire identity changed. |
| Final full compatibility | `cargo +1.96.0 test --locked --workspace --all-features --no-fail-fast`, run once | pass | Root reports 293 passed and 21 documented ignores; the graph target passes 52/52; embeddings reports 56 unit plus 6 domain tests; WSI reports 10 passed with one checksummed public-oracle test ignored; all remaining executed integration and doc tests pass. |
| Independent milestone review | one bounded `c04_embedding_audit` design audit; disjoint `c05_region_receipt` implementation ownership | pass | The single review required an unforgeable candidate, candidate-bound physical receipt, exact lineage, fixed `u64` conversion/order, and explicit work/memory limits. Those findings are implemented. The parallel implementation task owned only the receipt core and two matrix readers; root owned exports, tests, ledgers, and final verification. No second review was requested. |
| Claim ceiling | implementation and review scope audit | partial by design | Deterministic synthetic region values and physical receipt authority are active. Fractions remain producer declarations; region geometry, opaque source-vector correspondence, both slide paths, C-05 fuzz/scale/DHAT/RSS evidence, real-corpus promotion, and C-05/EMB-PATCH closure remain open. |

## C-05 derived-slide support, graph, finalization, and physical receipt milestone — 2026-08-23

Implementation base: `17467e36f74fcc34af924d45056d1e898bf85dd7`. This milestone closes the already contracted patch-sourced and region-sourced singleton-slide data flows using the existing lower receipts and slide physical profile. It adds no format, source adapter, general lower-table abstraction, geometry/window proof, real-corpus promotion, or C-05/EMB-PATCH closure claim.

| Evidence | Command or method | Result | Notes |
|---|---|---|---|
| Decoded-provenance ownership regression | exact `derived_slide_graph_rejects_decoded_provenance_for_a_foreign_slide` before and after the source-bound lineage fix | red with an incorrectly minted graph, then pass | Canonical JSON was decoded for slide B while retaining slide A's source/support artifacts and exact managed records. The graph now rejects it with the provenance role before capability minting for both source variants. |
| Both support and graph chains | four slide graph cases inside `cargo +1.96.0 test --locked --all-features --test multiscale_embedding_artifact_graph` | pass | Patch and region support receipts require exact verified lower support/table identities, managed payloads, profiles, dependencies, and source ownership. Seven-role provenance graphs reject cross-path, cross-lineage, role, record, dependency, payload, availability, and managed-integrity drift with redacted errors. |
| Frozen arithmetic and resource boundaries | five slide finalizer cases for both lower levels | pass | Fixed-order goldens include distinct patch/region means and the arithmetic-sensitive `[+0, 1/3]` result. Non-present statuses are excluded; zero present sources yield `missing_vector`; dimensions 1 and 65,536 pass. Bindings precede budgets; exact contributor 2/component 6 plus discovered retained/working edges accept exactly and reject one short. |
| Candidate-only physical authority | two slide receipt cases across both source levels, Arrow/Parquet, and borrowed/managed readers | pass | Arbitrary slide tables cannot mint `VerifiedSlideEmbeddingTableArtifact`. Candidate receipts preserve expected-slide, support, provenance, lower table, derivation, logical/QC, and dimension identities; malformed physical bytes and managed integrity precede candidate mismatch, and Debug/errors remain private. |
| Focused integration and package suites | graph target; `cargo +1.96.0 test --locked --all-features -p marklab-embeddings` | pass, 63/63; pass | The graph target contains eleven new slide cases and all prior 52 cases. Embeddings reports 56 unit, 6 domain, 1 hostile-row-link, and doc tests green. |
| Feature, docs, warning, and format gates | no-default package Clippy; no-default workspace check; strict all-feature package docs; workspace all-target/all-feature Clippy; `cargo +1.96.0 fmt --all --check`; `git diff --check` | pass after one test-fixture correction | Clippy first rejected a 1.7-KiB inline test enum variant; boxing the three region-fixture values corrected it without production changes. No manifest, dependency, lockfile, generated file, CLI/config/result, adapter, or physical profile changed. |
| Final full compatibility | `cargo +1.96.0 test --locked --workspace --all-features`, run once | pass | Root reports 293 passed and 21 documented ignores; graph 63/63; embeddings 56 unit plus 6 domain and 1 hostile-row-link; WSI 10 passed/1 public-oracle ignore; every remaining executed integration/package/doc test passes. |
| Independent milestone review | one read-only `c04_feasibility_audit` review | pass after finding applied | Review identified the high-severity decoded-provenance cross-slide capability gap and no other correctness, arithmetic, resource, privacy, feature, API, or Immediate-Caller issue. The regression and fixed-size source binding close it; no second review was requested. |
| Claim ceiling | implementation and review scope audit | partial by design | Both deterministic synthetic slide paths and candidate-bound physical receipts are active. Transitive sampled support is not a tissue/window claim; opaque source components, declared region geometry, C-05 fuzz/scale/allocation/RSS evidence, real-corpus promotion, and C-05/EMB-PATCH closure remain open. |

## C-05 fuzz, differential, scale, and phase closure — 2026-08-24

Implementation base: `966acaac95e9b0416ac14b504d41592696722aad`. This single closure milestone adds no production API, physical profile, validator, receipt, dependency, workflow, source adapter, corpus promotion, or scientific estimand. It closes only the already contracted fuzz, cross-family differential, shared-vector allocation, deterministic scale, and phase evidence.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior contracts | exact `workflow_contract` Criterion and fuzz-manifest tests | red for expected absence, then pass | The benchmark target/support and C-05 fuzz routes were absent at red. Both exact focused tests pass after adding only the immediate private callers and existing-target routes. |
| Cross-family differential | `cargo +1.96.0 test --locked --features parquet --test multiscale_embedding_differential` | pass | One declarative independent row/status/vector matrix agrees with all three typed tables, five nonzero scan partitions, empty scan behavior, Arrow/Parquet, and borrowed/managed validation. |
| Focused C-05 domains | exact table, cell/patch link, patch/region link, record, four graph, Arrow, Parquet, differential, and embeddings-package commands | pass | Tables 12/12; cell-patch 7/7; patch-region 9/9; graph/record owners 7 + 63 + 21 + 8; Arrow 11/11; Parquet 12/12; differential 1/1; default embeddings 22 unit + 6 domain. The obsolete frozen `multiscale_embedding_provenance` target name failed discovery once and the contract now names its four real owners. |
| Structured fuzz | `cargo +nightly fuzz check`; `cargo +nightly fuzz run embedding_inputs /tmp/marklab-c05-fuzz.Ka9ijA -- -runs=20000 -max_len=4096` | pass | All fuzz targets build. The bounded campaign finishes 20,000 runs with no crash (`cov 16432`, `ft 30248`, 897 corpus entries); its task-local corpus was moved to Trash. |
| Criterion smoke | `cargo +1.96.0 bench --locked --bench patch_embeddings --features parquet -- '10k_x_1024_100k_links' --noplot` | intentional golden red, then pass | Independent generator and link oracles passed before the placeholder mismatch. Final 10-sample interval is 29.961–30.261 ms; the four immutable identities are in DEC-0038. |
| Fresh-publication DHAT | exact no-default `parquet,dhat-heap` `patch_embedding_heap` command | pass | 1/1 in 309.77 s; current 0, peak 180,729,962, cap 335,544,320, forbidden copied-vector bytes 409,600,000. |
| Full scale and RSS | frozen timed prebuild then timed `100k_x_1024_1m_links` command | intentional golden red, then pass | Final prebuild compile/link RSS is 5,221,138,432 bytes. The current-binary run performs no compilation, checks 100,000 × 1,024 values and 1,000,000 edges, reports 283.76–284.82 ms, and uses 1,311,342,592-byte maximum RSS versus the 2,684,354,560-byte cap. |
| Feature and warning matrix | exact eight `WORKSPACE_POLICY.md` checks plus both warning-denied Clippy commands | pass with documented narrow warnings | Default/all/CLI/WSI+CLI and both Clippy gates are clean. Minimal/CSV/WSI emit the 14 existing instrumentation warnings; Parquet-only additionally emits four existing compatibility-writer warnings. All rows exit 0. |
| Docs, WSI, and policy | strict embeddings docs; workspace doctests; WSI integration; audit; deny; Machete | pass with configured warnings/ignore | Strict docs and all doc-test binaries pass; WSI is 10 passed/1 scheduled public oracle ignored; audit has no vulnerability and two allowed unmaintained warnings; deny passes with configured duplicate warnings; Machete finds no unused dependency. |
| Heap, benchmark, and workflow phase gates | legacy no-default DHAT; all-workspace all-feature quick benchmark; release synthetic smoke | pass | Legacy DHAT is 3/3. Nine Criterion targets pass with no detected regression; patch smoke is 30.191–30.329 ms. Synthetic smoke is 12/12 scenarios and 120/120 replicates with zero failures; its 14,043-byte output was moved to Trash. |
| Full workspace | `cargo +1.96.0 nextest run --locked --workspace --all-features`; workspace doc tests | pass | 846/846 passed in 64.632 s, 23 skipped, one expected slow synthetic test; all six doc-test binaries pass with zero executable examples. This is the milestone's sole full-workspace run. |
| Packaging | exact package; dirty `--allow-dirty`; supplemental local-path patched package | known non-green registry blocker; supplemental pass | The precommit exact command stops at Cargo's dirty-manifest guard. Dirty characterization creates all six archives and reproduces DEC-0018 during data verification against published pre-coordinate core. Ephemeral local paths verify all six archives but are non-equivalent. The exact clean command is run immediately after the self-referential closure commit and reported before C-06 work; no release-ready claim is made. |
| Independent review | `c05_scale_closure_map`, one read-only pre-implementation review | pass after findings applied | The sole review corrected four-bucket candidate maxima to 391,510/3,987,010 and froze exact checksum fields/types/order. No second review was requested; no high-risk issue emerged. Reviewer edited no files. |
| Closure claim | implementation, scope, and Immediate-Caller audit | complete within claim ceiling | C-05 and `EMB-PATCH` synthetic infrastructure close. Opaque source components/coordinates remain unproved; fractions remain declarations; geometry/window truth, real-source promotion, embedding quality, confounder analysis, prediction, and biological conclusions remain outside the claim. |

## C-06 first measurement-status computation slice — 2026-08-24

Implementation base: `e0efb60a8110f606bc2964c5947e443c843f9394`. This milestone adds the minimum measurement-origin vocabulary and its immediate C-05 patch caller. It adds no general MarkTable, physical schema, receipt, validator framework, dependency, source adapter, compatibility-document field, spatial-weights/window abstraction, inferential statistic, or real-corpus claim.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Status red/green | `cargo +1.96.0 test --locked --test measurement_status` | expected compile red, then pass 1/1 | Red named only the absent enum and provenance method. Direct patch maps to `MorphologyPrediction`; region and both slide variants map to `DerivedSummary`. |
| Observable computation red/green | `cargo +1.96.0 test --locked --test patch_overlap_embedding_dispersion` | expected compile red, then pass 6/6 | Three-edge hand golden is `50/3`; an independent linear oracle agrees bitwise. Exact signed permutation preserves the value; repeated fixed-order calls agree. All three non-present states exclude incident edges, all-zero present rows remain eligible, and zero eligible pairs return typed `InsufficientPairs` with no value. |
| Bindings and work | same 6-case integration target | pass | Status, unsupported derived provenance, expected-set, provenance, support, and overlap drift fail before arithmetic. Required component work is checked `edge_count * dimension`; 6 succeeds and 5 fails even when every row is non-present. No pair/vector allocation is retained. |
| Feature boundary | `cargo +1.96.0 test --locked --no-default-features --test measurement_status --test patch_overlap_embedding_dispersion` | pass 7/7 | The status/provenance/computation path is feature-independent and does not require Arrow, Parquet, CSV, CLI, or a new dependency. |
| Focused packages | `cargo +1.96.0 test --locked --package marklab-data`; `cargo +1.96.0 test --locked --package marklab-embeddings --all-features` | pass | Data is 1/1 plus docs; embeddings is 56 unit, 6 domain, 1 hostile integration, plus docs. Existing physical and provenance suites remain green. |
| Warnings, docs, and format | focused integration all-feature Clippy; all-target/all-feature data+embeddings Clippy; strict `marklab`/data/embeddings docs; `cargo +1.96.0 fmt --all --check`; `git diff --check` | pass | All lint invocations use `-D warnings`; public docs are warning-clean. No manifest, dependency, lock, wire, schema, CLI/config/result, or generated file changed. |
| Full workspace | `cargo +1.96.0 nextest run --locked --workspace --all-features`, run once | pass | 853/853 passed in 64.524 s, 23 skipped, one expected slow synthetic test. This is the milestone's sole full-workspace run. |
| Independent review | one read-only `c06_contract_map` review after implementation | pass; no findings | Review checked compatibility, public API, numeric finiteness/order, all bindings and precedence, resource bounds, status versus extraction missingness, Immediate-Caller compliance, and claim ceiling. Reviewer made no edits and no second review was requested. |
| Claim ceiling | implementation and Immediate-Caller audit | partial by design | The result is descriptive squared embedding distance over declared overlap adjacency. FND-04/C-06 general marks and EMB-01 geometry/weights/window/null/inference remain open; no variogram, autocorrelation, biology, or real-source claim is admitted. |

## C-06 declared scalar-pattern workflow — 2026-08-24

| Gate | Command/evidence | Result | Notes |
|---|---|---|---|
| Behavior-first input boundary | `cargo +1.96.0 test --locked --test scalar_mark_input` | expected red, then 9/9 pass | Initial compile failed on the absent declarations/input/error surface. Focused reds then reproduced target-project bypass and unsupported missing rows before the minimum fixes. Final cases cover row/text exact edges, required columns, hierarchy/slide/frame, probability/threshold semantics, exact provenance profiles/dependencies, and role aliases. |
| Behavior-first engine/project workflow | `cargo +1.96.0 test --locked --test declared_marked_workflow` | expected red, then 8/8 pass | Initial compile failed on the absent engine/node surface. Direct binary/probability paths match legacy numerics; target-project revalidation precedes catalog mutation; runtime identity and routing survive miss/hit; CellId/frame/status/provenance/threshold drift changes cache identity; result bytes remain exact 0.3; missing/corrupt semantic objects commit no success. |
| Focused no-default behavior | `cargo +1.96.0 test --locked --no-default-features --test scalar_mark_input --test declared_marked_workflow` | 17/17 pass | The same boundary and workflow behavior passes without default features. |
| Relevant compatibility | `cargo +1.96.0 test --locked --test api_contract --test config_v02 --test result_v03 --test project_workflow --test workflow_contract` | 41/41 pass | Existing public API, config 0.2, result 0.3, project/store workflow, and repository workflow contracts remain green. |
| Features, warnings, docs | focused all-feature root Clippy; `cargo +1.96.0 check --locked --workspace --no-default-features`; `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`; `env RUSTDOCFLAGS=-Dwarnings cargo +1.96.0 doc --locked --no-deps --package marklab --all-features` | pass | No manifest, lock, dependency, Pattern/loader, config/result/CLI, physical format, or generated file changed. |
| Broader documentation probe | `env RUSTDOCFLAGS=-Dmissing-docs cargo +1.96.0 doc --locked --no-deps --package marklab --all-features` | non-green baseline limitation | The command fails on thousands of pre-existing undocumented root compatibility items. It also identified four new threshold-variant fields, which were documented; no `missing-docs`-clean root claim is made. Strict warning-denied docs pass instead. |
| Single independent review | `c06_contract_map` read-only final diff review | finding reproduced and fixed | The reviewer found cache-visible CellId/frame identity absent from typed runtime outputs. Exact red assertions failed, then pass after compact `DeclaredScalarIdentity` is returned directly and reattached on miss/hit. No second review was requested. |
| Sole full-workspace gate | `cargo +1.96.0 nextest run --locked --workspace --all-features` | 870/870 pass in 64.923 s | 63 binaries, 23 documented skips, and one expected slow synthetic test; no failure. The gate was run once after final source changes. |
| Format and final scope | `cargo +1.96.0 fmt --all --check`; `git diff --check`; final status/diff review | pass | Changes are confined to the declared scalar domain, direct engine/local node callers, focused tests, and affected implementation ledgers. No C-05 fuzz/DHAT/RSS/benchmark/packaging gate was rerun because this milestone cannot invalidate it. |

## C-06 patch-to-derived-region aggregation dispersion — 2026-08-24

Implementation base: `c370685fd0f6cbfd01efa0d2bcfb12a4aa7e14e9`. This milestone adds one concrete scientific consumer of the completed C-05 region finalizer. It adds no physical format, receipt, validator, workflow node, codec, general statistics layer, geometry/weights owner, dependency, source adapter, or result/config/CLI change.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first red/green | `cargo +1.96.0 test --locked --all-features --test multiscale_embedding_artifact_graph region_dispersion` | expected missing-surface red, then 6/6 pass | Red named only the absent function/result/error/status surface after one test-harness shadowing typo was corrected. Final cases cover a hand oracle, exact identities/statuses, present/non-present/zero/unavailable behavior, all binding drift before a zero budget, repeat determinism, sign flips, and exact/one-short work caps. |
| Full changed boundary | `cargo +1.96.0 test --locked --all-features --test multiscale_embedding_artifact_graph` | 69/69 pass | All direct/derived graph, finalizer, receipt, and new computation paths remain green. After the review corrected one test claim, the affected 6-case filter was rerun and passed on the final test code. |
| Embedding package | `cargo +1.96.0 test --locked --package marklab-embeddings --all-features` | pass | 56 unit, 6 domain, 1 hostile-row-link integration, and doc tests pass. |
| Features, warnings, docs, format | package all-target/all-feature Clippy; root no-default Clippy; `cargo +1.96.0 check --locked --workspace --no-default-features`; final workspace all-target/all-feature Clippy; strict warning-denied embeddings/root docs; `cargo +1.96.0 fmt --all --check` | pass | The no-default module/facade and all-feature test target are warning-clean. Workspace Clippy was rerun after the review-driven test correction because that changed an all-target input. |
| Single independent review | `c06_contract_map`, read-only final diff review | finding corrected; no second review | The reviewer supplied a valid high-dynamic-range counterexample to the claimed bitwise axis-permutation invariant under sequential `f64` component accumulation. The contract/test now promise only true sign-flip invariance and explicitly disclose component-order last-bit rounding. No other finding; reviewer edited and executed nothing. |
| Final full workspace | `cargo +1.96.0 nextest run --locked --workspace --all-features` | 876/876 pass in 69.194 s | 63 binaries, 23 documented skips, and one expected slow synthetic test. A preliminary run was interrupted before completion when the review finding arrived; this is the sole completed gate after the correction. |
| Scope and claim ceiling | final diff/Immediate-Caller audit | pass | Output is declared-fraction-weighted dispersion around materialized derived means with exact source/link/candidate/provenance identity. It proves no geometry, window, spatial dependence, independence, inference, quality, real-source result, or biology. C-05 fuzz/DHAT/RSS/benchmark/packaging evidence was not rerun because no affected path changed. |

## C-06 runtime-only declared marked pre/post — 2026-08-24

Implementation base: `0c21c83ef55d6b2df397018a7a8427f5b1599460`. This milestone adds one observable comparison over two existing declared scheduler outputs. It adds no scheduler node, codec, result/schema version, physical format, receipt, validator, generalized comparison/mark abstraction, dependency, source adapter, or inference surface.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first red/green | `cargo +1.96.0 test --locked --test declared_marked_prepost` | expected missing-surface red, then 4/4 pass | The clean red named the absent function/result/error surface after two test-fixture-only typos were corrected. Final cases use real scheduler outputs; prove legacy/result-0.3 parity and exact identity/evidence/timepoint retention; cover semantic mismatch precedence; preserve legacy flags/unavailability; and reject every available public-wrapper binding drift. |
| Affected default boundary | `cargo +1.96.0 test --locked --test declared_marked_prepost --test declared_marked_workflow --test scalar_mark_input --test result_v03`; `cargo +1.96.0 test --locked --lib prepost::tests` | 34/34 plus 13/13 pass | Existing declared input/scheduler/cache behavior, result-0.3 compatibility, and all legacy pre/post unit behavior remain green. |
| No-default boundary | `cargo +1.96.0 test --locked --no-default-features --test declared_marked_prepost --test declared_marked_workflow --test scalar_mark_input` | 21/21 pass | The runtime semantic gate and existing declared workflow remain independent of default physical/CLI features. |
| Features, warnings, docs, format | focused root all-feature Clippy; `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`; `cargo +1.96.0 check --locked --workspace --no-default-features`; `env RUSTDOCFLAGS=-Dwarnings cargo +1.96.0 doc --locked --no-deps --package marklab --all-features`; `cargo +1.96.0 fmt --all --check` | pass | The final borrowed-timepoint API is warning-clean and documented. No manifest, lock, dependency, Pattern/loader, config/result/CLI, physical format, or generated file changed. |
| Single independent review | `c06_caller_map`, one read-only final diff review | two findings corrected; no second review | The reviewer found that public runtime fields initially allowed unchecked result/mark-use/identity mixing and that exact timepoints were documented but not retained. The nonbreaking available binding checks now cover row count, label, and recomputed declaration identity; both timepoints are borrowed. Making the pre-existing runtime type opaque would be breaking and was not authorized. |
| Final full workspace | `cargo +1.96.0 nextest run --locked --workspace --all-features` | 880/880 pass in 64.416 s | 64 binaries, 23 documented skips, and one expected slow synthetic test. This is the sole completed full-workspace gate after the final source change. |
| Scope and residual limit | final diff/Immediate-Caller audit | pass within claim ceiling | The wrapper preserves two caller-supplied declared contexts around the unchanged descriptive comparator. Arbitrary same-row/same-label numeric-result substitution cannot be authenticated because the existing public runtime fields/result 0.3 contain no private producer proof; this is disclosed as caller assertion, not receipt authority. C-05/C-06 fuzz, DHAT, RSS, benchmark, packaging, remote, and dependency gates were not rerun because no affected path changed. |

## C-06 declared binary prevalence change — 2026-08-24

Implementation base: `d0822bde67a93648b1a73cf61e1886c8463b6ca7`. This milestone adds one O(1) descriptive computation over two existing declared scheduler outputs. It adds no node, cache codec, result/config/CLI field, physical format, receipt, validator framework, general comparison/marks/inference abstraction, randomization, p-value, dependency, or geometry surface.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first red/green | `cargo +1.96.0 test --locked --test declared_marked_prepost prevalence` | expected missing-surface red, then 4/4 pass | Red named only the absent prevalence function/status. Green covers genuine scheduler outputs with different contexts/evidence, exact `+0.25`/`-0.25` direction, probability-spectrum routing with binary counts, semantic mismatch, public count/prevalence drift, and a legitimate empty side. |
| Complete focused target | `cargo +1.96.0 test --locked --test declared_marked_prepost`; same command with `--no-default-features` | 5/5 pass in both modes | Existing declared pre/post parity, evidence/identity/timepoint retention, result-0.3 bytes, typed unavailability, and the new prevalence behavior remain green. Both modes were rerun after the review correction. |
| Affected compatibility | `cargo +1.96.0 test --locked --test declared_marked_prepost --test declared_marked_workflow --test result_v03`; `cargo +1.96.0 test --locked --lib prepost::tests` | 26/26 plus 13/13 pass | Declared scheduler/cache behavior, result-format 0.3, and legacy pre/post units remain green. The later review correction touched only lazy unavailable-path arithmetic and could not invalidate these successful unaffected paths, so they were not rerun. |
| Features, warnings, docs, format | focused all-feature target Clippy; `cargo +1.96.0 check --locked --workspace --no-default-features`; `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`; `env RUSTDOCFLAGS=-Dwarnings cargo +1.96.0 doc --locked --no-deps --package marklab --all-features`; `cargo +1.96.0 fmt --all --check` | pass | Public lifetimes/types and the strengthened shared binding path are warning-clean in the affected feature modes. No manifest, lock, dependency, Pattern/loader, config/result/CLI, physical format, or generated file changed. |
| Single independent review | `c06_contract_map`, one read-only final diff review | one P2 semantic finding corrected; no second review | `bool::then_some` eagerly evaluated the discarded subtraction when a side was empty. An explicit branch now performs the one `f64` subtraction only when both sides contain rows. No other API, binding, routing, precedence, lifetime/resource, compatibility, Immediate-Caller, or claim finding was reported. |
| Final full workspace | `cargo +1.96.0 nextest run --locked --workspace --all-features` | 881/881 pass in 64.648 s | 64 binaries, 23 documented skips, and one expected slow synthetic test. This is the milestone's sole full-workspace run after the final source change. |
| Scope and residual limit | final diff/Immediate-Caller audit | pass within claim ceiling | The output is descriptive binary marked-row proportion change across two supplied row collections. Consistent public-field substitution remains caller-asserted, not producer-authenticated. No correspondence, biological unit, patient/specimen prevalence, treatment effect, inference, calibration, equivalence, noninferiority, causality, or biology is admitted. C-05/C-06 fuzz, DHAT, RSS, benchmark, packaging, remote, and dependency gates were not rerun because no affected path changed. |

## C-06 slide aggregation-path discrepancy — 2026-08-24

Implementation base: `767c0e961e4fabcfdb931cd1aef42b446fe97542`. This milestone adds one concrete scientific consumer of both completed C-05 slide finalizers. It adds no candidate, graph, receipt, physical profile, validator framework, serializer, general comparison/statistics layer, dependency, source adapter, or result/config/CLI change.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first red/green | `cargo +1.96.0 test --locked --all-features --test multiscale_embedding_artifact_graph slide_path_discrepancy` | expected missing-surface red, then 5/5 pass | Red named only the absent function/result/error/status surface; one crate-root status import was corrected during green. Final cases cover the exact 0.625 hand oracle, all retained identities/statuses, repeat bits, positive zero, typed missingness, exact/one-short work, provenance/candidate/common-context drift, and same-slide foreign patch lineage. |
| Changed integration boundary | same target filtered by `derived_slide`; then unfiltered | 16/16; 74/74 pass | Both slide support/graph/finalization/receipt paths and every combined direct/derived graph path remain green. The later review correction only moved an unforgeable defensive row-ID check ahead of the cap; the affected 5/5 filter was rerun on final source. |
| Embedding package | `cargo +1.96.0 test --locked --package marklab-embeddings --all-features` | pass | 56 unit, 6 domain, 1 hostile-row-link integration, and doc tests pass. |
| Features, warnings, docs, format | focused all-feature embedding Clippy; `cargo +1.96.0 check --locked --workspace --no-default-features`; `env RUSTDOCFLAGS=-Dwarnings cargo +1.96.0 doc --locked --no-deps --package marklab --all-features`; final `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`; formatting | pass | Public types and both feature boundaries compile warning-free. No manifest, lock, dependency, physical code/profile, generated file, source adapter, config/result/CLI, or Arrow/Parquet subsystem changed. |
| Single independent review | `c06_contract_map`, one read-only final diff review | one P2 precedence finding corrected; no second review | Singleton row lookup/common-slide binding initially followed the exact-D cap. Both row identities now precede the cap, while status/vector traversal remains after it so missingness cannot evade work admission. The invalid row state cannot be forged through the public candidate API; all forgeable binding-before-budget cases remain behavior-covered. No other finding was reported. |
| Final full workspace | `cargo +1.96.0 nextest run --locked --workspace --all-features` | 886/886 pass in 64.649 s | 64 binaries, 23 documented skips, and one expected slow synthetic test. This is the milestone's sole full-workspace run after the final source change. |
| Scope and claim ceiling | final diff/Immediate-Caller audit | pass | The fixed-size output is descriptive mean squared component discrepancy over one exact patch-direct and patch→region→slide lineage. It proves no agreement, quality, geometry, window, spatial dependence, information preservation, inference, path preference, real-source result, or biology. C-05 fuzz/DHAT/RSS/benchmark/packaging evidence was not rerun because no affected path changed. |

## C-06 declared binary cell-embedding centroid discrepancy — 2026-08-24

Implementation base: `f00a9a4f4b4ffbda97ef606195f0b291abb8d10a`. This milestone adds one observable scientific computation joining the existing declared binary-mark boundary to a verified C-04 cell-embedding table/artifact. It adds no mark kind, embedding/scalar infrastructure, physical profile, receipt, graph, validator framework, serializer, workflow node, result/config/CLI field, general statistics abstraction, dependency, source adapter, geometry/weights/null/inference surface, or real-source promotion.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first red/green | `cargo +1.96.0 test --locked --all-features --test cellvit_embedding_artifact_graph declared_binary_centroid` | expected missing-surface red, then final 4/4 pass | The clean production red named the absent function/status/error surface. During green, one root-only import was corrected to the existing workflow facade and the test fixture was corrected from an inadmissible two-component CellViT profile to the canonical 1,280-component profile. Final cases prove the exact 20.0 oracle, probability-opposed binary routing, assignment digest, positive zero, all extraction statuses, typed unavailability, binding precedence, repeat bits, and exact/one-short limits. |
| Changed integration boundary | unfiltered `cargo +1.96.0 test --locked --all-features --test cellvit_embedding_artifact_graph`; final focused command above after review correction | 29/29 pass; affected 4/4 rerun | The complete verified CellViT graph/physical/artifact boundary remains green. The review correction changed only the new computation and focused regression, so the successful 25 unaffected cases were not rerun. |
| Declared and embedding dependencies | `cargo +1.96.0 test --locked --test scalar_mark_input`; `cargo +1.96.0 test --locked --package marklab-embeddings --all-features` | pass | Declared scalar input passes 9/9. Embeddings passes 56 unit, 6 domain, 1 hostile-row-link integration, and doc tests. |
| Features, warnings, docs, format | `cargo +1.96.0 check --locked --workspace --no-default-features`; focused all-feature target Clippy; `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`; `env RUSTDOCFLAGS=-Dwarnings cargo +1.96.0 doc --locked --no-deps --package marklab --all-features`; `cargo +1.96.0 fmt --all --check`; `git diff --check` | pass | Public types, bounded allocation, test target, and both feature boundaries are warning-clean. No manifest, lock, dependency, generated file, Arrow/Parquet owner, scalar owner, or remote path changed. |
| Single independent review | `c06_contract_map`, one read-only final diff review | one P1 identity finding corrected; no second review | The reviewer found that `DeclaredScalarIdentity` binds declaration metadata but not the exact binary row assignments. The result now carries a domain-separated digest over ordered CellId-bound binary values; an unchanged-declaration/count/value swapped-assignment regression proves the distinction. No other binding, routing, status, resource, allocation, arithmetic, API, dependency, Immediate-Caller, or claim issue was reported. |
| Final full workspace | `cargo +1.96.0 nextest run --locked --workspace --all-features` | 890/890 pass in 64.580 s | 64 binaries, 23 documented skips, and one expected slow synthetic test. This is the sole completed workspace gate after the final source change. |
| Scope and claim ceiling | final diff/Immediate-Caller audit | pass | The output is the descriptive mean squared component difference between exact declared binary-group centroids over present verified cell embeddings. It proves no spatial association, classification/separability, embedding quality, independence, patient/specimen effect, inference, calibration, real-source result, WS-50/EMB-01 closure, or biology. C-05 fuzz/DHAT/RSS/benchmark/packaging, remote, and dependency gates were not rerun because no affected path changed. |

## C-06 declared probability–cell-embedding cross-covariance energy — 2026-08-24

Implementation base: `ec9ca190b45c04d8e7e844201dee725f783c608d`. This milestone adds one observable WS-50 scientific computation joining the existing dense declared probability modality to a verified C-04 cell-embedding table/artifact. It adds no mark kind, scalar/embedding owner, physical profile, receipt, graph, validator framework, serializer, workflow node/codec, result/config/CLI field, general statistics abstraction, dependency, source adapter, geometry/weights/null/inference surface, or real-source promotion.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first red/green | `cargo +1.96.0 test --locked --all-features --test cellvit_embedding_artifact_graph probability_cross_covariance` | expected missing-surface red, then final 5/5 pass | The clean red named only the absent function/status/error surface. Final cases prove the 1,280-component 0.625 population oracle, repeat bits, binary non-use, probability-value identity, changed-probability 0.453125 oracle, positive zero, all extraction statuses, both typed unavailable states, required probability modality, binding precedence, and exact/one-short limits. |
| Changed integration boundary | unfiltered `cargo +1.96.0 test --locked --all-features --test cellvit_embedding_artifact_graph` | 34/34 pass in 34.46 s | Every verified CellViT graph/physical/artifact path plus both declared binary/probability scientific child modules remains green. |
| Declared and embedding dependencies | `cargo +1.96.0 test --locked --test scalar_mark_input`; `cargo +1.96.0 test --locked --package marklab-embeddings --all-features` | pass | Declared scalar input passes 9/9. Embeddings passes 56 unit, 6 domain, 1 hostile-row-link integration, and doc tests. |
| Focused warning gate | `cargo +1.96.0 clippy --locked --package marklab --test cellvit_embedding_artifact_graph --all-features -- -D warnings` | initial concrete lint failure, then pass | Clippy rejected loading `tests/support/declared_scalar.rs` independently in both child modules. The existing parent integration target now declares that shared test support once; both child modules consume it immediately. No allowance or production abstraction was added, and focused 5/5 was rerun after the correction. |
| Features, warnings, docs, format | `cargo +1.96.0 check --locked --workspace --no-default-features`; `env RUSTDOCFLAGS=-Dwarnings cargo +1.96.0 doc --locked --no-deps --package marklab --all-features`; `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`; `cargo +1.96.0 fmt --all --check`; `git diff --check` | pass | Public types, two-pass arithmetic, bounded allocation, test target, and both feature boundaries are warning-clean. No manifest, lock, dependency, generated file, Arrow/Parquet owner, scalar/embedding owner, or remote path changed. |
| Single independent review | `c06_caller_map`, one read-only final diff review | no concrete findings; no second review | The reviewer checked probability/table/CellId binding and precedence, raw-bit probability identity, binary non-use, typed availability, two-pass population arithmetic, conservative caps/allocation order, API/docs, Immediate-Caller compliance, compatibility/dependency scope, and claim ceiling. The reviewer ran nothing and made no edits. |
| Final full workspace | `cargo +1.96.0 nextest run --locked --workspace --all-features` | 895/895 pass in 64.693 s | 64 binaries, 23 documented skips, and one expected slow synthetic test. This is the milestone's sole workspace gate after final source changes. |
| Scope and claim ceiling | final diff/Immediate-Caller audit | pass | The output is descriptive unstandardized mean squared component population cross-covariance energy over exact declared probabilities and present verified cell embeddings. It proves no correlation, explained variance, spatial association/dependence, classification, calibration, embedding quality, independence, patient/specimen effect, inference, real-source result, WS-50/EMB-01 closure, or biology. C-05 fuzz/DHAT/RSS/benchmark/packaging, remote, and dependency gates were not rerun because no affected path changed. |

## C-06 declared binary cell-centroid project workflow — 2026-08-24

Implementation base: `45a203e08ee0cd8d5e41eb06e43234117ac34c47`. This milestone exposes the existing S7 binary-centroid computation as one semantic-store-verified project execution. Its private fixed codec has that exact scheduler node as its immediate production caller. It adds no new estimand, engine/CLI/config/result-0.3 field, general codec/result framework, physical format, receipt, validator, dependency, source adapter, or Arrow/Parquet subsystem change.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first red/green | `cargo +1.96.0 test --locked --all-features --test cellvit_embedding_artifact_graph declared_binary_centroid_workflow` | expected missing-node red, then final 8/8 pass in 6.81 s | Miss/hit equals direct S7 with an independent exact 18-byte oracle; swapped assignments, optional probability/threshold evidence, embedding bytes, every scientific limit, and scheduler limit alter cache identity; target-project/catalog/binding failures precede reference registration; all four semantic roles fail missing/corrupt before miss and would-be-hit replay; unavailable status round-trips; strict length/magic/version/tag/value/status cases reject; and scheduler/project 18/17-byte edges are failure-atomic. |
| Changed integration and compatibility boundaries | unfiltered `cargo +1.96.0 test --locked --all-features --test cellvit_embedding_artifact_graph`; `cargo +1.96.0 test --locked --all-features --test scalar_mark_input --test declared_marked_workflow --test project_workflow --test result_v03` | 42/42 pass in 35.64 s; 37/37 pass | The complete verified CellViT graph/physical/artifact boundary, direct S7 and S8 computations, declared scheduler/cache behavior, project atomicity, scalar provenance, and exact result-format 0.3 compatibility remain green. |
| Project/workflow/embedding packages | `cargo +1.96.0 test --locked --package marklab-project --package marklab-workflow`; `cargo +1.96.0 test --locked --package marklab-embeddings --all-features` | pass | Project/workflow pass 42 tests plus docs; embeddings pass 56 unit, 6 domain, 1 hostile-row-link integration, and docs. |
| Focused warning gate | `cargo +1.96.0 clippy --locked --all-features --test cellvit_embedding_artifact_graph -- -D warnings` | initial test-helper arity lint, then pass | The test-only runner now takes one limits tuple. No allowance or production abstraction was added, and focused 8/8 was rerun after the mechanical correction. |
| Features, warnings, docs, format | `cargo +1.96.0 check --locked --workspace --no-default-features`; `env RUSTDOCFLAGS=-Dwarnings cargo +1.96.0 doc --locked --no-deps --package marklab --all-features`; `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`; `cargo +1.96.0 fmt --all --check`; `git diff --check` | pass | The public node, private codec, narrow binding seam, all targets, and both feature boundaries are warning-clean. No manifest, lock, dependency, generated file, physical owner, scalar owner, source adapter, or remote path changed. |
| Single independent review | `c06_s9_review`, one read-only final diff review | no actionable findings; no second review | The reviewer checked correctness, cache identity/order, semantic-store verification, failure atomicity, codec canonicality, numerical/resource behavior, public compatibility, and Immediate-Caller compliance. The reviewer ran nothing and made no edits. |
| Final full workspace | `cargo +1.96.0 nextest run --locked --workspace --all-features` | 903/903 pass in 64.749 s | 64 binaries, 23 documented skips, and one expected slow synthetic test. This is the milestone's sole full-workspace run after final source changes. |
| Scope and residual limit | final diff/Immediate-Caller audit | pass within claim ceiling | The cache reattaches exact current identities within existing project state, but it is not a durable producer-authenticated receipt or portable result format. S7's descriptive centroid claim ceiling is unchanged. C-05/C-06 fuzz, DHAT, RSS, benchmark, packaging, remote, and dependency gates were not rerun because no affected path changed. |

## C-06 declared nucleus-area cell-embedding cross-covariance energy — 2026-08-24

Implementation base: `45caf8991533ed16e65a6d756be40b662c49428a`. This milestone adds one observable WS-50 scientific computation joining the existing dense `Pattern::nucleus_area_um2` column to a verified C-04 cell-embedding table/artifact. Its fixed declaration and exact provenance profile are immediately consumed by that function. It adds no generic continuous mark or unit registry, shared covariance/statistics abstraction, physical profile, receipt, graph, validator framework, serializer, workflow node/codec, result/config/CLI field, dependency, source adapter, geometry/weights/null/inference surface, or real-source promotion.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first red/green | `cargo +1.96.0 test --locked --all-features --test cellvit_embedding_artifact_graph nucleus_area_cross_covariance` | expected missing-surface red; final 7/7 pass in 2.37 s | The clean red named only the four absent public exports. Final cases prove the 1,280-component 62.5 population oracle, changed-area 45.3125 oracle, repeat bits, binary non-use, raw-area identity, positive zero, all extraction statuses, both typed unavailable states, exact declaration/provenance/project requirements, binding precedence, and exact/one-short limits. One first-green assertion expected the wrong pre-existing project-validation precedence; correcting that test expectation produced the final green without a production change. |
| Private resource-helper unit | `cargo +1.96.0 test --locked --all-features --lib checked_resource_helpers_expose_overflow_and_allocation_failures` | 1/1 pass | Checked component/byte overflow and impossible-capacity fallible allocation retain typed errors even though safe public inputs cannot construct those states. |
| Changed CellViT integration boundary | unfiltered `cargo +1.96.0 test --locked --all-features --test cellvit_embedding_artifact_graph` | 49/49 pass in 36.38 s | Every verified CellViT graph/physical/artifact path plus the declared binary, probability, workflow, and nucleus-area scientific child modules remains green. |
| Declared/result compatibility | `cargo +1.96.0 test --locked --all-features --test scalar_mark_input --test declared_marked_workflow --test result_v03` | 30/30 pass | Scalar declaration/provenance behavior passes 9/9, the declared workflow 8/8, and exact result-format 0.3 compatibility 13/13. |
| Embedding package | `cargo +1.96.0 test --locked --package marklab-embeddings --all-features` | pass | Embeddings passes 56 unit, 6 domain, 1 hostile-row-link integration, and doc tests. |
| Features, warnings, docs, format | `cargo +1.96.0 check --locked --workspace --no-default-features`; `env RUSTDOCFLAGS=-Dwarnings cargo +1.96.0 doc --locked --no-deps --package marklab --all-features`; `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`; `cargo +1.96.0 fmt --all --check`; `git diff --check` | pass | The public declaration/result surface, exact provenance validation, two-pass arithmetic, bounded allocation, all targets, and both feature boundaries are warning-clean. One narrowly scoped test helper used only by the nucleus-area child carries `allow(dead_code)` because other integration targets compile the shared fixture without that child; no production warning allowance or abstraction was added. No manifest, lock, dependency, generated file, Arrow/Parquet owner, embedding owner, or remote path changed. |
| Single independent review | `c06_s10_review`, one read-only final diff review | no actionable findings; no second review | The reviewer checked provenance/binding precedence, finite-positive values, raw-bit identity, typed availability, two-pass population arithmetic, conservative caps/allocation order, API/docs, Immediate-Caller compliance, compatibility scope, and claim ceiling. The reviewer made no edits and requested no additional test. |
| Final full workspace | `cargo +1.96.0 nextest run --locked --workspace --all-features` | 911/911 pass in 65.167 s | 64 binaries, 23 documented skips, and one expected slow synthetic test. This is the milestone's sole full-workspace run after final source changes. |
| Scope and claim ceiling | final diff/Immediate-Caller audit | pass | The output is descriptive unstandardized mean squared component population cross-covariance energy over exact declared nucleus areas and present verified cell embeddings. It proves no correlation, explained variance, size normalization, segmentation accuracy, spatial association/dependence, classification, calibration, embedding quality, independence, patient/specimen effect, inference, real-source result, WS-50/EMB-01 closure, or biology. C-05 fuzz/DHAT/RSS/benchmark/packaging, remote, and dependency gates were not rerun because no affected path changed. |

## C-06 contained-patch cell-embedding local dispersion — 2026-08-24

Implementation base: `e44a7319c69b83913678881b30938b8d35f1ab62`. This milestone adds one observable WS-50/WS-51 computation joining a verified C-04 cell table/artifact to the completed C-05 contained-shared link, managed input graph, and paired physical assignment/edge receipt. The only binding closure retains two already-verified expected-cell identities on the compact cell artifact and is immediately consumed by the computation. No Arrow/Parquet implementation, schema, format, receipt, graph, validator, interpolation statistic, shared statistics abstraction, workflow/node/codec, result/config/CLI field, dependency, source adapter, or real-source promotion was added.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first red/green | `cargo +1.96.0 test --locked --all-features --test cellvit_embedding_artifact_graph contained_cell_patch_embedding_dispersion` | expected missing-surface red; final 7/7 pass in 3.99 s | The clean red named only the absent function/result/status/error exports and two expected-cell getters. The first post-implementation run reached all seven tests but the test fixture incorrectly declared a two-component CellViT tensor; the established provenance validator rejected it before S11 execution. The fixture was corrected to canonical 1,280 dimensions with half-axis variation preserving the exact 0.5 oracle; production was unchanged. Final cases prove overlapping-incidence counts/identities/repeat bits, vector sensitivity, positive zero, every non-present status, singleton/full unavailability, interpolation rejection, expected-cell/table/graph/receipt precedence, and exact/one-short limits. |
| Private resource-helper unit | `cargo +1.96.0 test --locked --package marklab-embeddings --all-features multiscale::contained_cell_patch_dispersion::tests::checked_resource_helpers_expose_overflow_and_allocation_failure` | 1/1 pass | Checked `3ED` and working-byte overflow plus impossible-capacity fallible allocation retain typed errors. |
| Changed integration boundaries | `cargo +1.96.0 test --locked --all-features --test cellvit_embedding_artifact_graph --test cell_patch_input_artifact_graph` | CellViT 56/56 in 36.90 s; cell-patch graph 7/7 in 2.16 s | The full verified cell artifact, declared scientific callers, contained-link managed graph, Arrow/Parquet physical halves, paired receipt, integrity precedence, and drift cases remain green. |
| Embedding package | `cargo +1.96.0 test --locked --package marklab-embeddings --all-features` | pass | Embeddings passes 57 unit, 6 domain, 1 hostile-row-link integration, and doc tests. |
| Features, warnings, docs, format | `cargo +1.96.0 check --locked --workspace --no-default-features`; `env RUSTDOCFLAGS=-Dwarnings cargo +1.96.0 doc --locked --no-deps --package marklab --package marklab-embeddings --all-features`; focused package/test Clippy; `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`; `cargo +1.96.0 fmt --all --check`; `git diff --check` | pass | The all-feature-only caller, compact artifact getters, bounded sorted traversal, public docs, every target, and unchanged no-default surface are warning-clean. No manifest, lock, dependency, generated file, physical implementation, or remote path changed. |
| Single independent review | `c06_s10_review`, one read-only S11 final diff review | no actionable findings; no second review | The reviewer checked the C-04→C-05 authority chain, contained-only semantics, grouping/oracle/counting, `3ED` work and exact storage, finite/positive-zero behavior, feature compatibility, Immediate-Caller compliance, and claim ceiling. The reviewer made no edits and ran no tests. |
| Final full workspace | `cargo +1.96.0 nextest run --locked --workspace --all-features` | 919/919 pass in 63.359 s | 64 binaries, 23 documented skips, and one expected slow synthetic test. This is the milestone's sole full-workspace run after final source changes. |
| Scope and claim ceiling | final diff/Immediate-Caller audit | pass | The output is descriptive incidence-weighted mean squared component dispersion around exact producer-declared contained-patch centroids. It proves no source-anchor correspondence, tissue window, spatial autocorrelation/dependence, independent-patch evidence, interpolation weighting, cell/patch vector alignment, embedding quality, patient/specimen effect, inference, real-source result, WS-50/WS-51/EMB-01 closure, or biology. C-05 fuzz/DHAT/RSS/benchmark/packaging, remote, and dependency gates were not rerun because no affected path changed. |

## C-06 declared binary-group nucleus-area contrast — 2026-08-24

Implementation base: `a770cdffd78e61cc185eb4a95d9142d684d1c6d5`. This milestone adds one observable pathology-facing computation joining the existing exact binary declaration to the fixed S10 positive nucleus-area measurement/provenance. It adds no generic continuous/group-statistics abstraction, physical format, receipt, validator framework, workflow/node/codec, result/config/CLI field, dependency, missingness model, source adapter, inferential surface, or Arrow/Parquet change.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first red/green | `cargo +1.96.0 test --locked --all-features --test binary_nucleus_area_contrast` | expected missing-surface red; final 6/6 pass | The clean red named only the absent function/result/status/error exports. Final cases prove marked/unmarked/difference oracles `25`/`12`/`13`, changed binary assignments and paired digest, optional-probability non-use, repeat bits, positive zero, both typed one-group unavailable states, exact project/provenance/value/length precedence, and exact/one-short row limits. |
| Affected declared/nucleus behavior | `cargo +1.96.0 test --locked --all-features --test binary_nucleus_area_contrast --test scalar_mark_input`; `cargo +1.96.0 test --locked --all-features --test cellvit_embedding_artifact_graph nucleus_area_cross_covariance` | 6/6 plus 9/9; 7/7 pass | The declared scalar/project/provenance boundary and existing S10 nucleus-area–embedding computation remain green. |
| Focused warning gates | warning-denied root library and `binary_nucleus_area_contrast` test Clippy | initial test-support dead-code warnings, then pass | This standalone integration target loads an established shared fixture whose other helpers are intentionally unused. One module-scoped test-only `allow(dead_code)` suppresses only those fixture warnings; no production allowance or abstraction was added, and the focused behavior target was rerun green. |
| Features, warnings, docs, format | `cargo +1.96.0 check --locked --workspace --no-default-features`; `env RUSTDOCFLAGS=-Dwarnings cargo +1.96.0 doc --locked --no-deps --package marklab --all-features`; `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`; `cargo +1.96.0 fmt --all --check`; `git diff --check` | pass | The public fixed-size result, deterministic single pass, all targets, and both feature boundaries are warning-clean. No manifest, lock, dependency, generated file, physical subsystem, scalar profile, or remote path changed. |
| Single independent review | `c06_s10_review`, one read-only S12 final diff review | no actionable findings; no second review | The reviewer checked project/provenance/binding precedence, finite-positive values, paired-value identity, optional-probability non-use, fixed-order means and positive zero, typed availability, row cap, API/docs, Immediate-Caller compliance, and claim ceiling. The reviewer made no edits and ran no tests. |
| Final full workspace | `cargo +1.96.0 nextest run --locked --workspace --all-features` | 925/925 pass in 63.813 s | 65 binaries, 23 documented skips, and one expected slow synthetic test. This is the milestone's sole full-workspace run after final source changes. |
| Scope and claim ceiling | final diff/Immediate-Caller audit | pass | The output is a descriptive within-input marked-minus-unmarked mean nucleus-area contrast over exact supplied rows. It proves no patient/specimen effect, pairing, segmentation accuracy or validation, classification, calibration, independence, spatial dependence, causal effect, inference, real-source result, MRK-01/WS-50 closure, or biology. C-05/C-06 fuzz, DHAT, RSS, benchmark, packaging, remote, and dependency gates were not rerun because no affected path changed. |

## C-06 contained-patch binary-group nucleus-area contrast — 2026-08-24

Implementation base: `1907e6a59ca8a7c29bc97ef61bf7ba48fc8dd14d`. This milestone adds one observable WS-51 computation composing the exact S12 declared scalar/nucleus-area result with the existing contained-shared link, managed input graph, and paired physical assignment/edge receipt. It adds no Arrow/Parquet implementation, physical format, graph, receipt, validator framework, shared S11 traversal or S12 group-statistics abstraction, workflow/node/codec, result/config/CLI field, dependency, observation-window/spatial/null/inference surface, source adapter, or real-source promotion.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first red/green | `cargo +1.96.0 test --locked --all-features --test cellvit_embedding_artifact_graph contained_patch_binary_nucleus_area` | expected missing-surface red; final 6/6 pass | The clean red named only the absent function/result/status/error exports, with no fixture or warning error. Final cases cover exact S12 identity, link/receipt identities and counts, positive zero, global and patch-local unavailable states, S12 project/provenance/value/row precedence, contained/slide/CellId/graph/receipt drift, and exact resource edges. |
| Weighting oracle after sole review | same focused command | 6/6 pass in 1.61 s | The corrected unequal-incidence fixture has patch contrasts `8` and `6`, equal-patch result `7`, whole-input result `8`, and incidence-weighted alternative `7.2`; all three are bitwise distinguished. Eligible incidence counts are marked 2, unmarked 3, total 5. |
| Affected integration boundaries | `cargo +1.96.0 test --locked --all-features --test cellvit_embedding_artifact_graph --test binary_nucleus_area_contrast --test cell_patch_input_artifact_graph` | CellViT 62/62; S12 6/6; cell-patch graph 7/7 pass | The complete declared/CellViT callers and existing contained-link graph/physical receipt authorities remain green. The later review correction changed only the six S13 test cases and feature gate; the focused filter was rerun rather than repeating 56 unaffected CellViT cases. |
| Features, warnings, docs, format | `cargo +1.96.0 check --locked --workspace --no-default-features`; focused root/test Clippy; `env RUSTDOCFLAGS=-Dwarnings cargo +1.96.0 doc --locked --no-deps --package marklab --all-features`; `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`; `cargo +1.96.0 fmt --all --check`; `git diff --check` | pass | The graph/receipt-bound module and export use the existing `parquet` feature, while no-default remains unchanged. Public docs, bounded sorted traversal, test fixture, and every all-feature target are warning-clean. No manifest, lock, dependency, generated file, physical implementation, or remote path changed. |
| Single independent review | `c06_s10_review`, one read-only S13 final diff review | two findings corrected; no second review | The reviewer found the missing Parquet feature gate and an equal-size oracle unable to distinguish equal-patch from whole-input/incidence-weighted arithmetic. Both exact regressions are green. No additional binding, finite/panic, count, resource, API, Immediate-Caller, compatibility, or claim finding was reported. |
| Final full workspace | `cargo +1.96.0 nextest run --locked --workspace --all-features` | 931/931 pass in 67.390 s | 65 binaries, 23 documented skips, and one expected slow synthetic test. This is the milestone's sole full-workspace run after final source changes. |
| Scope and claim ceiling | final diff/Immediate-Caller audit | pass | The output is the descriptive equal-patch mean of within-patch marked-minus-unmarked nucleus-area contrasts over producer-declared contained incidences. It proves no observation window, spatial association/autocorrelation, independent-patch design, patient/specimen effect, paired-cell analysis, segmentation validation, classification, calibration, independence, inference, real-source result, MRK-01/WS-51 closure, or biology. C-05/C-06 fuzz, DHAT, RSS, benchmark, packaging, remote, and dependency gates were not rerun because no affected path changed. |
## Full-program simulation checkpoint 25 — 2026-08-25

This checkpoint adds three consumed Part X workflows without advancing paused PLAT-DUR-01: scalar
periodic reaction–diffusion/pattern diagnostics, static-speed level-set evolution, and declared-flow
vascular transport.

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Behavior-first reaction–diffusion | `cargo +1.96.0 test --locked --features cli --test simulation_reaction_diffusion_cli` | expected missing-command red; later 11-versus-10 work and non-equilibrium-instability reds; final 3/3 pass | Exact doubling/planned work, known 2-um Fourier mode, periodic checkerboard eigenmode, and equilibrium-gated instability status pass. |
| Behavior-first level set | `cargo +1.96.0 test --locked --features cli --test simulation_level_set_cli` | expected missing-command red; boundary inconsistency exposed and corrected; final 2/2 pass | Unit planar translation remains exact with zero planar curvature; signed-distance reinitialization retains the planar zero contour and reports bounded visits. |
| Behavior-first vascular transport | `cargo +1.96.0 test --locked --features cli --test simulation_vascular_transport_cli` | expected missing-command red; later missing hypoxic-region red; final 3/3 pass | Exact source/uptake, conservative diffusion, conservative upwind advection, mass balance, mapping, and four-neighbor hypoxic regions pass. |
| Simulation package stabilization | `cargo +1.96.0 fmt --all --check`; `cargo +1.96.0 clippy --locked --package marklab-simulation --all-targets --all-features -- -D warnings`; package all-feature unit/doc and no-default commands | pass | Formatting, warnings, public compilation, unit/doc tests, and no-default package boundary are green. |
| Six consumed simulator workflows | one serial `cargo +1.96.0 test --locked --features cli` invocation naming all six simulation integration targets; trailing spatial/vascular pair rerun directly after the combined command yielded | 15/15 pass | Growth-front 3, spatial competition 2, agent competition 2, reaction–diffusion 3, level set 2, and vascular transport 3 all pass. |
| Excluded broad gates | workspace-wide Clippy/nextest/docs/no-default/feature matrix | not run by design | These gates compile the paused inherited PLAT-DUR-01 work. Scoped task-owned evidence was run instead and is not represented as equivalent workspace coverage. |
## Full-program simulation/SBI-summary checkpoint 26 — 2026-08-25

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Resource-response TDD | `cargo +1.96.0 test --locked --features cli --test bayes_distance_to_resource_cli` | expected missing-command red; missing resource-PPC red; final 1/1 pass | Exact segment distances, linear/hinge recovery, patient/resource predictive checks, and claim ceiling pass. |
| Mechanistic-coupling TDD | `cargo +1.96.0 test --locked --features cli --test simulation_mechanistic_tissue_cli` | expected missing-command red; coupled death corrected the initial inert-agent oracle; final 1/1 pass | Oxygen→density/interface/agent exchange and second-interval absorbing extinction pass. |
| Differentiable-summary TDD | `cargo +1.96.0 test --locked --features cli --test simulation_summary_matching_cli` | expected missing-command red; final 1/1 pass | Closed-form Gaussian peak, positive displacement loss, and point-order-invariant exact zero pass. |
| Affected packages | all-feature `marklab-bayes`/`marklab-simulation` tests and docs; no-default checks; warning-denied all-target/all-feature Clippy | pass | Bayes 39/39 plus both doc suites; simulation unit/doc boundary; both no-default builds; no warnings. |
| Format/diff | `cargo +1.96.0 fmt --all --check`; `git diff --check` | pass | Exit zero after final production changes. |
| Neural admission | backend lock/data/master-plan audit recorded in `NEURAL-GEN-01` | blocked with exact resume condition | Eight §§76–79 functions are not replaced by orphan helpers or non-neural baselines. |
| Excluded broad gates | workspace Clippy/nextest/docs/feature matrix | not run by design | They compile paused inherited PLAT-DUR-01; scoped evidence is not represented as equivalent workspace coverage. |
## Full-program SBI checkpoint 27 — 2026-08-25

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| Rejection ABC | `cargo +1.96.0 test --locked --features cli --test sbi_rejection_abc_growth_front_cli` | expected missing-command red; pass | Thirty accepted draws recover rate one, satisfy epsilon, and byte replay. |
| SMC-ABC | `cargo +1.96.0 test --locked --features cli --test sbi_smc_abc_growth_front_cli` | expected missing-command red; pass | Three tolerance stages recover rate one, normalize 64 weights, retain ESS, and byte replay. |
| Synthetic likelihood | `cargo +1.96.0 test --locked --features cli --test sbi_synthetic_likelihood_growth_front_cli` | expected missing-command red; pass | Two-summary noisy likelihood/MCMC recovers rate one, has bounded acceptance, retains 1,000 draws, and byte replay. |
| SBI package | all-feature unit/doc, no-default, warning-denied all-target/all-feature Clippy | pass | New dedicated layer and its simulator dependency compile/test without warnings. |
| Format/diff | `cargo +1.96.0 fmt --all --check`; `git diff --check` | pass | Exit zero after final production changes. |
| Excluded broad gates | workspace gates | not run by design | Paused inherited PLAT-DUR-01 would be compiled; scoped evidence is not represented as equivalent. |
## Full-program SBI reliability checkpoint 28 — 2026-08-25

| Gate | Exact command/review | Status | Result/evidence |
|---|---|---|---|
| SBC | `cargo +1.96.0 test --locked --features cli --test sbi_growth_front_sbc_cli` | expected missing-command red; pass | 100 ranks, coverage, zero failures, and byte replay. |
| Simulation OOD | `cargo +1.96.0 test --locked --features cli --test sbi_simulation_ood_cli` | expected missing-command red; one output-policy fixture correction; pass | Far observation rejected and calibration-like control retained without fit/calibration leakage. |
| Posterior-predictive lab | `cargo +1.96.0 test --locked --features cli --test sbi_posterior_predictive_lab_cli` | expected missing-command red; pass | Both analytic summaries retained; zero flags/failures; byte replay. |
| SBI package | all-feature unit/doc, no-default, warning-denied all-target/all-feature Clippy | pass | Reliability modules and simulator boundary compile/test without warnings. |
| Format/diff | formatting and `git diff --check` | pass | Exit zero after final production changes. |
| Excluded workspace | broad gates | not run by design | Paused inherited PLAT-DUR-01 remains outside task evidence. |

## Full-program longitudinal milestone 29a — 2026-08-25

- `cargo +1.96.0 fmt --all --check` — passed.
- `cargo +1.96.0 test --locked --package marklab-longitudinal` — passed 2 unit tests and doc tests. Coverage includes multivariate partial/all-missing observations and indefinite process-covariance rejection.
- `cargo +1.96.0 test --locked --features cli --test longitudinal_kalman_smooth_cli` — passed the scalar independent hand oracle and byte replay.
- `git diff --check` — passed before ledger update.
- Workspace-wide gates and a commit were not run because the dirty checkout contains paused inherited PLAT-DUR-01 work.

## Full-program longitudinal milestone 29b — 2026-08-25

- Expected red: `cargo +1.96.0 test --locked --features cli --test longitudinal_nonlinear_filter_cli` failed because `nonlinear-filter` did not exist.
- `cargo +1.96.0 fmt --all --check` — passed.
- `cargo +1.96.0 clippy --locked --package marklab-longitudinal --all-targets --all-features -- -D warnings` — initially found one complex tuple return; passed after replacing it with the cohesive `SigmaPoints` internal type.
- `cargo +1.96.0 test --locked --package marklab-longitudinal` — passed 3 unit tests and doc tests.
- `cargo +1.96.0 test --locked --features cli --test longitudinal_nonlinear_filter_cli` — passed EKF/UKF exact linear-limit and byte-replay oracle.
- `cargo +1.96.0 check --locked --package marklab-longitudinal --no-default-features` — passed.

## Full-program longitudinal checkpoint 29 — 2026-08-25

- Expected red: `cargo +1.96.0 test --locked --features cli --test longitudinal_particle_smooth_cli` failed because `particle-smooth` did not exist.
- `cargo +1.96.0 test --locked --package marklab-longitudinal` — passed 4 unit tests plus doc tests, including forced low-ESS systematic resampling.
- `cargo +1.96.0 test --locked --features cli --test longitudinal_particle_smooth_cli` — passed deterministic filtering/ancestry smoothing and byte replay.
- `cargo +1.96.0 clippy --locked --package marklab-longitudinal --all-targets --all-features -- -D warnings` — passed.
- `cargo +1.96.0 check --locked --package marklab-longitudinal --no-default-features` — passed.
- `cargo +1.96.0 test --locked --features cli --test longitudinal_kalman_smooth_cli --test longitudinal_nonlinear_filter_cli --test longitudinal_particle_smooth_cli` — all 3 checkpoint CLI tests passed together.
- `cargo +1.96.0 fmt --all --check` and `git diff --check` — passed after final production changes. Workspace-wide gates and a commit remain excluded because of paused inherited PLAT-DUR-01.

## Full-program 3-D statistics milestone 30a — 2026-08-25

- Expected red: `cargo +1.96.0 test --locked --features cli --test spatial3d_k_function_cli` failed because `spatial3d` did not exist.
- `cargo +1.96.0 test --locked --package marklab-spatial3d` — passed 2 package tests and doc tests, covering anisotropic eligibility and non-SPD rejection.
- `cargo +1.96.0 test --locked --features cli --test spatial3d_k_function_cli` — passed all three correction hand values, physical-unit normalization, and byte replay.
- `cargo +1.96.0 clippy --locked --package marklab-spatial3d --all-targets --all-features -- -D warnings` — initially identified index-only symmetry loops; passed after iterator-based correction.
- `cargo +1.96.0 check --locked --package marklab-spatial3d --no-default-features`, formatting, and `git diff --check` — passed.
- Workspace-wide gates and a commit were not run because paused inherited PLAT-DUR-01 remains outside this evidence.

## Full-program 3-D statistics checkpoint 30 — 2026-08-25

- Expected red: `cargo +1.96.0 test --locked --features cli --test spatial3d_weighted_k_cli` failed because `inhomogeneous-k` did not exist.
- Expected boundary red: the focused zero-radius cross-g package test failed to compile while cross-g was an unconditional `f64`; it passed after the output represented the zero-volume shell as unavailable.
- `cargo +1.96.0 test --locked --package marklab-spatial3d` — passed 3 package tests and doc tests.
- `cargo +1.96.0 test --locked --features cli --test spatial3d_k_function_cli --test spatial3d_weighted_k_cli` — both homogeneous and weighted/directed CLI suites passed together.
- `cargo +1.96.0 clippy --locked --package marklab-spatial3d --all-targets --all-features -- -D warnings`, no-default check, formatting, and `git diff --check` — passed.
- Workspace-wide gates and a commit remain excluded because paused inherited PLAT-DUR-01 is outside this evidence.

## Full-program randomized interference milestone 32a — 2026-08-25

- Expected red: `cargo +1.96.0 test --locked --features cli --test causal_randomized_interference_cli` failed because `causal` did not exist.
- The first multicluster package oracle exposed improper whole-workflow failure for structurally impossible non-target exposures; it passed after per-exposure positivity became an explicit unavailable state while target contrast positivity remained mandatory.
- `cargo +1.96.0 test --locked --package marklab-causal` — passed 2 package tests and doc tests.
- `cargo +1.96.0 test --locked --features cli --test causal_randomized_interference_cli` — passed exact six-state probabilities and byte replay.
- Warning-denied package Clippy, no-default check, formatting, and `git diff --check` — passed.

## Full-program causal exposure milestone 32b — 2026-08-25

- Expected red: `cargo +1.96.0 test --locked --features cli --test causal_exposure_mapping_cli` failed because `exposure-mapping` did not exist.
- `cargo +1.96.0 test --locked --package marklab-causal` — passed 2 package tests and doc tests.
- Both causal CLI suites passed together; the exposure suite covers all six mapping branches and the randomized-interference suite protects the prior inference boundary.
- Warning-denied package Clippy, no-default check, formatting, and `git diff --check` — passed after final production changes.

## Full-program causal/design checkpoint 32 — 2026-08-25

- Expected red: `cargo +1.96.0 test --locked --features cli --test causal_gaussian_eig_cli` failed because `gaussian-eig` did not exist.
- `cargo +1.96.0 test --locked --package marklab-causal` — passed 3 package tests and doc tests after the EIG addition.
- All three causal CLI suites passed together; Gaussian EIG retains 1,000 outer values, matches the analytic oracle within its uncertainty gate, executes 501,000 likelihood evaluations, and byte replays.
- Warning-denied package Clippy, no-default check, formatting, and `git diff --check` — passed. Workspace-wide gates and a commit remain excluded by paused inherited PLAT-DUR-01.

## Full-program causal sensitivity checkpoint 33 — 2026-08-25

- Each new CLI first failed with its expected missing subcommand: `causal_bias_sensitivity_cli`, `causal_manski_bounds_cli`, and `causal_rosenbaum_sensitivity_cli`.
- The bias CLI initially assumed input row order; the corrected oracle selects canonicalized scenarios by stable ID and passes.
- `cargo +1.96.0 test --locked --package marklab-causal` passed 3 package tests and doc tests.
- All six causal CLI suites passed together, covering checkpoint 32 plus the three sensitivity/partial-ID workflows.
- Warning-denied package Clippy, no-default check, formatting, and `git diff --check` passed. Workspace-wide gates and a commit remain excluded by paused inherited PLAT-DUR-01.

## Full-program execution-policy checkpoint 34 — 2026-08-25

- Expected reds: `policy_result_maturity_cli` failed because `policy` did not exist; `policy_execution_mode_cli` failed because `select-mode` did not exist.
- `cargo +1.96.0 test --locked --package marklab-policy` passed 2 package tests and doc tests.
- Both policy CLI suites passed together, covering maturity precedence and approximation denial/approval with complete assessments.
- Warning-denied package Clippy, no-default check, formatting, and `git diff --check` passed. The prior numerics scoped gates remained green; workspace-wide gates and a commit remain excluded by paused inherited PLAT-DUR-01.

## Full-program validation-ladder milestone 35a — 2026-08-25

- Expected red: `policy_validation_ladder_cli` failed because `validation-ladder` did not exist.
- Initial warning-denied Clippy found one obfuscated test conditional; the explicit branch replacement passed.
- `cargo +1.96.0 test --locked --package marklab-policy` passed 3 package tests and doc tests; all three policy CLI suites passed together.
- Warning-denied package Clippy, no-default, formatting, and `git diff --check` passed. Workspace-wide gates and a commit remain excluded by paused inherited PLAT-DUR-01.

## Full-program stable numerics milestone 34a — 2026-08-25

- Expected red: `cargo +1.96.0 test --locked --features cli --test numerics_stable_primitives_cli` failed because `numerics` did not exist.
- `cargo +1.96.0 test --locked --package marklab-numerics` passed the effective-sample failure unit test and docs.
- `cargo +1.96.0 test --locked --features cli --test numerics_stable_primitives_cli` passed overflow/cancellation/covariance hand oracles.
- Warning-denied package Clippy, no-default check, formatting, and `git diff --check` passed. Workspace-wide gates and a commit remain excluded by paused inherited PLAT-DUR-01.

## Full-program Part XI checkpoint 31 stabilization — 2026-08-25

- `marklab-spatial3d`: warning-denied all-target/all-feature Clippy, 4 package tests/docs, no-default check, and all 3 3-D CLI suites passed after the final graph digest change.
- `marklab-longitudinal`: warning-denied all-target/all-feature Clippy, 4 package tests/docs, no-default check, and all 4 longitudinal/evolutionary CLI suites passed after the final association change.
- `cargo +1.96.0 fmt --all --check` and `git diff --check` passed after the blocker audit/documentation update; final status preserved all inherited PLAT-DUR-01 files/hunks.
- Workspace-wide gates and a commit were not run because they would include paused inherited PLAT-DUR-01; scoped evidence is not represented as equivalent.

## Full-program evolutionary association milestone 31b — 2026-08-25

- Expected red: `cargo +1.96.0 test --locked --features cli --test longitudinal_phylogenetic_spatial_association_cli` failed because the command did not exist.
- `cargo +1.96.0 test --locked --package marklab-longitudinal` — passed 4 package tests and doc tests.
- All four longitudinal CLI suites passed together, including the four-clone tree/spatial oracle and byte replay.
- `cargo +1.96.0 clippy --locked --package marklab-longitudinal --all-targets --all-features -- -D warnings`, no-default check, formatting, and `git diff --check` — passed.
- Workspace-wide gates and a commit remain excluded because paused inherited PLAT-DUR-01 is outside this evidence.

## Full-program 3-D graph milestone 31a — 2026-08-25

- Expected red: `cargo +1.96.0 test --locked --features cli --test spatial3d_graph_cli` failed because `spatial-graph` did not exist.
- Digest regression red: two radius specifications with identical realized edges initially produced the same SHA-256; passed after normalized window/spacing/full metric and all rule/weight parameters were bound.
- `cargo +1.96.0 test --locked --package marklab-spatial3d` — passed 4 package tests and doc tests.
- All three 3-D CLI suites passed together; package warning-denied Clippy, no-default check, formatting, and `git diff --check` passed.
- Workspace-wide gates and a commit remain excluded because paused inherited PLAT-DUR-01 is outside this evidence.

## Full-program anisotropic 3-D GP milestone 35b — 2026-08-25

- Expected red: `cargo +1.96.0 test --locked --features cli --test bayes_anisotropic_gp3d_cli` failed because `anisotropic-gp-3d` did not exist.
- `cargo +1.96.0 test --locked --package marklab-bayes anisotropic_gp` passed the exact zero/one-scaled-distance covariance oracles and the every-axis-variation rejection.
- `cargo +1.96.0 test --locked --features cli --test bayes_anisotropic_gp3d_cli -- --nocapture` passed the pinned PyMC 6.3.0 12-point fit, complete diagnostic gate, and two finite positive-uncertainty predictions.
- All 41 `marklab-bayes` package tests, warning-denied package Clippy, no-default check, and Python syntax compilation passed. Workspace-wide gates remain excluded because they compile paused PLAT-DUR-01.

## Full-program cohort residual checkpoint 36 — 2026-08-25

- Expected missing-command reds were observed for `repeated-freedman-lane`, `functional-equivalence`, `bootstrap-equivalence`, and `multisite-inference` before production edits.
- Focused CLI oracles passed: common repeated slope `2.1`; simultaneous three-scale equivalence; patient-first interval containment; fixed multisite pooled effect `2`, SE `1/sqrt(3)`, Q `2`, chi-square p `exp(-1)`, and exact leave-one-out effects.
- `cargo +1.96.0 test --locked --package marklab-cohort` passed 20 unit tests and all existing differential integration tests. The random-effects package oracle requires positive REML tau for site effects `0,3,6` at SE `0.5`.
- Warning-denied cohort Clippy, no-default check, formatting, and eight affected CLI suites passed after replacing one Clippy-reported needless range loop. Workspace-wide gates remain excluded because they compile paused PLAT-DUR-01.

## Full-program graph spectral milestone 37a — 2026-08-25

- Expected red: `cargo +1.96.0 test --locked --features cli --test graph_spectral_cli` failed because `graph` did not exist.
- `cargo +1.96.0 test --locked --package marklab-graph` passed the two-node `0,2` spectrum oracle and docs; the real CLI passed the three-node `0,1,3` spectrum, coefficient-energy, canonical-order, and pair-work oracle.
- Warning-denied graph-package Clippy initially reported the eigendecomposition tuple type; a named internal decomposition record fixed it. Clippy, no-default, formatting, focused tests, and `git diff --check` then passed.

## Full-program graph heat/wavelet checkpoint 37 — 2026-08-25

- Expected missing-command reds were observed for `graph heat` and `graph wavelet` before production edits.
- The heat CLI passed exact zero-time identity/application/signature/`sqrt(2)` distance and positive-time row-mass/smoothing checks. The wavelet CLI passed the lambda-one `2 exp(-2)` band-pass/low-pass energy oracle.
- `cargo +1.96.0 clippy --locked --package marklab-graph --all-targets --all-features -- -D warnings`, no-default check, package tests/docs, all three graph CLI suites, formatting, and `git diff --check` passed.

## Full-program graph-spectrum null milestone 37b — 2026-08-25

- Expected red: `graph_spectrum_null_cli` failed because `spectrum-null` did not exist. The first green attempt reached the result but failed its low-band energy oracle because a roundoff-negative zero eigenvalue fell below the band's zero lower bound; canonical zero normalization fixed production and both spectral regressions passed.
- `marklab-numerics` two-test package suite and the pre-existing embedding ERL unit/CLI regressions passed after shared-owner extraction. The constant four-node graph null returns low/high energies `4/0`, identical ERL bounds, and global/scalar p-values one.
- Warning-denied Clippy passed for numerics, graph, and bayes; graph/numerics package tests/docs, four graph CLI suites, embedding-envelope CLI, formatting, and `git diff --check` passed.

## Full-program Part VII graph checkpoint 38 — 2026-08-25

- Expected red: `graph_chebyshev_heat_cli` failed because `chebyshev-heat` did not exist.
- The adaptive workflow selected an order at or below 32, met `1e-8` reference-tail/grid and exact-signal gates, and reproduced the analytic path eigenmode. One compile warning for an unused local was removed before checkpoint verification.
- The affected graph/numerics/bayes warning-denied Clippy checks, package tests/docs, all five graph CLI suites, embedding ERL regression, no-default graph check, formatting, and `git diff --check` are the scoped checkpoint gates. Workspace-wide gates remain excluded because they compile paused PLAT-DUR-01.

## Full-program Part VII completion checkpoint 39 — 2026-08-25

- Expected missing-command reds were observed before production edits for `graph hodge`, `graph cellular-complex`, and `graph validate`; earlier focused reds covered diffusion-wavelet, scattering, heterogeneous-message, hypergraph, and motif-triangle.
- Exact focused oracles passed for diffusion ranks/reconstruction, scattering features/stability, typed messages, hand-incidence hypergraph, exhaustive motif triangle/null, filled-triangle `B1*B2=0`/`L1=3I`/Hodge reconstruction/filter, and perturbation-stable cellular incidence.
- `cargo +1.96.0 fmt --all --check` — passed.
- `cargo +1.96.0 clippy --locked --package marklab-graph --all-targets --all-features -- -D warnings` — initially found one needless scattering scale range loop; passed after iterator correction.
- `cargo +1.96.0 check --locked --package marklab-graph --no-default-features` — passed.
- `cargo +1.96.0 test --locked --package marklab-graph` — passed one unit test and doc tests.
- One command running all 13 `graph_*_cli` integrations passed. The built-in validation ledger records ten passed exact/sensitivity entries and four explicit unsupported/not-applicable stress dimensions.
- Workspace-wide gates and a commit were not run because the dirty checkout includes paused inherited PLAT-DUR-01 changes; scoped evidence is not represented as equivalent.

## Full-program Part VIII topology checkpoint 40 — 2026-08-25

- Expected missing-command reds were observed for all four topology CLIs before production edits.
- Equilateral alpha persistence passed exact H0/H1, boundary, landscape, integrated-image, and Euler oracles; the five-point witness line passed landmark/coverage/tree/persistence oracles; supplied-raster and connectivity hand controls passed.
- Topology warning-denied Clippy initially found two manual range patterns and passed after correction. Formatting, no-default/package checks, all four CLI suites, three worker syntax checks, and `git diff --check` passed.
- Workspace-wide gates and a commit remain excluded because they include paused inherited PLAT-DUR-01.

## Full-program Part VIII completion checkpoint 41 — 2026-08-25

- Missing-command reds were observed for comparison, stability, and validation before production edits; all three focused oracles then passed.
- The complete topology checkpoint passed formatting, warning-denied all-target Clippy, no-default/package/doc checks, all seven topology CLI suites, six Python worker syntax checks, and `git diff --check`.
- The validation ledger passes nine exact/analytic rows and retains `sparse_memory_scaling=not_verified`; no representative scale claim is made. Workspace-wide gates and a commit remain excluded by paused PLAT-DUR-01.

## Full-program Part IX multimodal checkpoint 42 — 2026-08-25

- Expected missing-command reds were observed for pCCA, Bayesian pCCA, and MOFA before production edits.
- The Bayesian fixture initially returned one divergence at 40/60 draws and target 0.9; the unchanged zero-divergence assertion passes with 100/80 and target 0.99. EM pCCA and MOFA shared-factor/masked-target oracles pass.
- One command passed all three CLI suites after formatting; all three workers passed Python syntax compilation and `git diff --check`. Workspace-wide gates/commit remain excluded by paused PLAT-DUR-01.

## Full-program Part IX multimodal checkpoint 43 — 2026-08-25

- Expected missing-command reds were observed for `matrix-factor`, `hierarchical-factor`,
  `spatial-matrix-factor`, and both `tensor-factor` decomposition tests. The first spatial optimizer
  run failed truthfully at its 500-iteration cap; alternating exact conditional initialization made
  the unchanged oracle converge.
- Matrix MOFA recovered four masked rank-one entries below RMSE 0.35 with improving ELBO. The exact
  hierarchy hand oracle returned five nodes, four conditional edges, and three attachments. The
  graph-spatial fixture recovered four held-out smooth entries below RMSE 0.35 with positive
  uncertainty. CP and Tucker each recovered three masked 3x3x3 rank-one entries below RMSE 0.25.
- `cargo +1.96.0 fmt --all --check` passed.
- `cargo +1.96.0 clippy --locked --package marklab --bin marklab --features cli -- -D warnings`
  could not complete: preserved paused PLAT-DUR code at `src/cli/classical.rs:121` triggers
  `clippy::needless_borrow`. That unrelated hunk was not changed and this gate is not claimed green.
- `cargo +1.96.0 check --locked --package marklab --no-default-features` passed.
- One command running all seven multimodal integration files passed eight tests. Root doc tests
  passed (zero tests), all six multimodal workers passed `py_compile`, and `git diff --check` passed.
- Workspace-wide gates and a commit remain excluded by paused inherited PLAT-DUR-01.

## Full-program Part VI registration/atlas revisit checkpoint 45 — 2026-08-25

- Six expected missing-command reds were observed before production wiring. SimpleITK nonrigid,
  JAX SVF, landmark LDDMM, probabilistic SVF, landmark uncertainty/correspondence, and atlas focused
  tests all pass.
- The SVF test initially exposed a nonpositive boundary Jacobian from clipped absolute-map
  composition; displacement-field composition corrected it without changing the oracle.
- Checkpoint formatting, scoped tests, worker syntax, no-default, and diff results are appended after
  execution. Root Clippy remains subject to the preserved paused PLAT-DUR lint.
- `cargo +1.96.0 fmt --all --check` passed.
- Root warning-denied Clippy again failed only on preserved paused `src/cli/classical.rs:121`
  (`clippy::needless_borrow`); it was not fixed or suppressed.
- `cargo +1.96.0 check --locked --package marklab --no-default-features` passed.
- One command passed all six registration/atlas CLI tests. Root doc tests passed (zero tests), all
  six workers passed `py_compile`, the runtime asserted SimpleITK 2.5.5, and `git diff --check`
  passed. Workspace-wide gates/commit remain excluded by paused PLAT-DUR-01.

## Full-program Part IX completion checkpoint 44 — 2026-08-25

- Expected missing-command reds were observed for spatial latent, multiresolution, dropout,
  joint-pathology, multimodal comparison, and validation workflows. All focused tests subsequently
  passed; the original complementarity alias passed after shared-owner routing.
- PyMC spatial NUTS recovered three masks below RMSE 0.35 with zero divergences. Multiresolution and
  dropout oracles passed. Joint Laplace predicted its masked patient outcome below RMSE 0.6.
  Canonical comparison retained six models/six increments. The validation ledger passes eleven
  controls and retains three exact external/backend gaps.
- Checkpoint gate results follow after execution. Root Clippy's preserved PLAT-DUR lint is never
  represented as green.
- `cargo +1.96.0 fmt --all --check` passed.
- `cargo +1.96.0 clippy --locked --package marklab --bin marklab --features cli -- -D warnings`
  failed only at preserved paused `src/cli/classical.rs:121` with `clippy::needless_borrow`; it was
  not fixed, suppressed, or claimed green.
- `cargo +1.96.0 check --locked --package marklab --no-default-features` passed.
- One command running all 13 multimodal CLI files plus the original complementarity regression
  passed 15 tests. Root doc tests passed (zero tests), all 11 Part IX workers passed `py_compile`,
  and `git diff --check` passed.
- Workspace-wide gates and a commit remain excluded by paused inherited PLAT-DUR-01.

## Full-program Part X neural/generative revisit checkpoint 46 — 2026-08-25

- Expected missing-command reds were observed for marked Cox, point-set generators, neural SBI,
  and model-card commands. Cox training initially reached its iteration cap; corrected numerical
  convergence tolerances made the unchanged likelihood/mark/quadrature oracle pass.
- Flow/diffusion permutation/support/diversity, sbi analytic posterior/state/round isolation, and
  repeated-artifact model-card focused tests pass.
- Checkpoint gate results follow after execution; paused PLAT-DUR remains outside scope.
- `cargo +1.96.0 fmt --all --check` passed.
- Root warning-denied Clippy again failed only at preserved paused `src/cli/classical.rs:121`; it
  was not fixed or suppressed.
- `cargo +1.96.0 check --locked --package marklab --no-default-features` passed.
- One command passed all four neural/generative CLI files. Root doc tests passed (zero tests), all
  four workers passed `py_compile`, runtime asserted sbi 0.26.1/Torch 2.13.0, and
  `git diff --check` passed. Workspace-wide gates/commit remain excluded by paused PLAT-DUR-01.

## Full-program Part XI advanced 3-D revisit checkpoint 47 — 2026-08-25

- Five expected missing-command reds were observed. Serial stack, exact 3-D alpha,
  deformation/biology, clone models, and umbrella validation focused tests subsequently passed.
- Checkpoint gate results follow after execution; real longitudinal/clone evidence remains explicit
  and paused PLAT-DUR remains outside scope.
- `cargo +1.96.0 fmt --all --check` passed.
- Root warning-denied Clippy again failed only at preserved paused `src/cli/classical.rs:121`; it
  was not fixed or suppressed.
- `cargo +1.96.0 check --locked --package marklab --no-default-features` passed.
- One command passed all five advanced 3-D CLI tests. Root doc tests passed (zero tests), all five
  workers passed `py_compile`, and `git diff --check` passed. Workspace-wide gates/commit remain
  excluded by paused PLAT-DUR-01.

## Full-program Part XII causal/active revisit checkpoint 48 — 2026-08-25

- Four expected missing-command reds were observed. Observational, perturbation/mediation,
  active-design, and umbrella-validation focused tests subsequently passed.
- The perturbation fixture initially reused mediator noise in the outcome with a second coefficient,
  contradicting its declared controlled-path oracle; the generator was corrected without weakening
  the 1.0/0.4/1.2 assertions.
- Checkpoint formatting, scoped tests, worker syntax, no-default, docs, and diff results are appended
  after execution. Root Clippy remains subject to the preserved paused PLAT-DUR lint.
- `cargo +1.96.0 fmt --all --check` passed. One command passed all ten causal CLI suites; root doc
  tests passed (zero tests), no-default compilation passed, the new worker passed `py_compile`, and
  `git diff --check` passed.
- `cargo +1.96.0 clippy --locked --package marklab --bin marklab --features cli -- -D warnings`
  failed only at preserved paused `src/cli/classical.rs:121` with `clippy::needless_borrow`; it was
  not fixed, suppressed, or claimed green. Workspace-wide gates/commit remain excluded.

## Full-program unified runtime and residual Bayesian checkpoint 49 — 2026-08-25

- Expected missing-command reds were observed for HMC, advanced cluster inference, and runtime
  validation; a missing-public-symbol compile red preceded `execute_algorithm` extraction.
- Fixed-step HMC and advanced cluster focused tests pass. Six durable integration tests pass through
  the extracted owner. Runtime validation passes its HMC diagnostic, 4,000-fit bias/coverage, fixed
  partition, and checksum-equivalent 1k/2k/4k smoke assertions.
- The runtime calibration initially exposed an incomplete inverse-normal approximation; the complete
  bounded quantile approximation passes the unchanged 92–98% coverage gate.

## Full-program terminal SPDE checkpoint 50 — 2026-08-25

- The expected missing-command red preceded the shared rectangular SPDE workflow. Mesh/precision/
  projection, LGCP quadrature/intensity, one-factor reconstruction, and two-resolution sensitivity
  assertions pass.
- The first tau-one LGCP oracle was correctly too smooth for its declared right-heavy contrast; the
  synthetic prior was changed to tau 0.2 without weakening the ratio, count, precision, projection,
  reconstruction, or factor-correlation assertions.
- Terminal scoped and major-checkpoint commands and exact results follow after execution.

## Full-program terminal verification checkpoint 51 — 2026-08-26

- `cargo +1.96.0 test --locked --workspace --all-features -- --test-threads=1` executed the complete
  260-binary manifest serially. Every preceding binary passed; the final `workspace_contract` binary
  found two stale architecture assertions after the scientific packages were added. The allowlist,
  explicit dependency layers, and workspace policy were updated; the exact two-test
  `workspace_contract` rerun then passed.
- Audit of the neural-SBI run found undeclared default TensorBoard output. A new working-directory
  assertion failed first, the pinned sbi 0.26.1 adapter was given an explicit no-op tracker, and the
  unchanged analytic NPE/NLE/NRE/sequential oracle plus the no-side-effect assertion passed. The
  session-generated `sbi-logs/` and Python bytecode cache were moved to Trash.
- `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings` passed in
  13m29s. `cargo +1.96.0 check --locked --workspace --no-default-features` passed. The all-features
  workspace doc-test command passed for all 17 packages (zero doctests).
- `cargo +1.96.0 fmt --all --check`, `git diff --check`, and
  `python3 docs/implementation/verify_pseudocode_pack.py` passed. A literal multiset audit found 283
  declarations and 283 live entries (282 unique names because `SimulationBasedCalibration` is the
  documented duplicate), with no missing or extra entry.
- The exact canonical `cargo +1.96.0 nextest run --locked --workspace --all-features` remains
  unavailable on this Mac: after a successful 6m21s all-target build, nextest's concurrent discovery
  left 24 `--list` children asleep in the macOS loader, including children older than seven minutes,
  before test execution. The command was interrupted to stop the repeated verifier loop. Earlier
  monolithic and batched attempts reproduced the same condition. The successful serial Cargo
  execution plus focused post-fix reruns are recorded as separate evidence, not called equivalent to
  a green nextest command.

## PLAT-DUR-01 durable replay closure checkpoint 52 — 2026-08-26

- `native_build_provenance_marks_untracked_source_state_dirty` failed first with `left:
  Some(false)`, `right: Some(true)` and passed after the build Git query included normal untracked
  state.
- `changed_source_bytes_are_a_miss_even_when_the_parsed_pattern_is_unchanged` failed first because
  the second process returned `"hit"`; it passed after exact stable cell/window source references
  joined the node inputs.
- `cargo +1.96.0 test --locked --package marklab-project --lib durable::tests::` passed 6/6.
- `cargo +1.96.0 test --locked --package marklab --features cli --test
  durable_classical_project` passed 13/13. `cargo +1.96.0 test --locked --package marklab --features
  cli --test classical_spatial_workflow --test classical_spatial_cli` passed 3/3 and 4/4.
- `cargo +1.96.0 fmt --all --check` passed. `cargo +1.96.0 clippy --locked --workspace
  --all-targets --all-features -- -D warnings` passed in 11m35s. `cargo +1.96.0 check --locked
  --workspace --no-default-features`, workspace all-feature doc tests, and strict warning-denied
  workspace docs passed.
- `cargo +1.96.0 test --locked --workspace --all-features -- --test-threads=1` compiled and linked
  the full test inventory in 6m28s. The root suite passed 294 with 21 intentional ignores; API
  contract passed 6/6; the authorized reconciliation binary retained its one intentional ignore;
  and the next two Bayesian integration binaries passed. The command was then interrupted with exit
  130 because macOS imposed roughly tens of seconds of loader verification on each fresh integration
  binary, projecting hours for the remaining inventory. This partial run is not claimed green. The
  affected suites above are complete and checkpoint 51 retains the last complete serial workspace
  run.
- No Nextest retry was made: checkpoint 51 already records its reproduced concurrent macOS loader
  stall. No commit, stage, push, deployment, history rewrite, or worktree was created.

## Durable external-backend checkpoint 53 — 2026-08-26

- Expected reds: `cargo +1.96.0 test --locked --package marklab --features cli --test
  durable_pymc_project -- --nocapture` failed because `project normal-mean` was unrecognized;
  `durable_pot_project` failed for the same missing-command reason. The first POT green attempt then
  exposed noncanonical floating JSON after typed decode/re-encode; the unchanged test passed after
  the POT codec normalized its strict typed result to a stable fixed point.
- `cargo +1.96.0 test --locked --package marklab --features cli --bin marklab
  static_backend_identity_binds_environment_worker_and_request_bytes` passed 1/1. It proves exact
  lock digest, worker digest, request/config bytes, and backend/version participate in or are checked
  by the closed static descriptor.
- One serial command running `durable_classical_project`, `durable_pymc_project`,
  `durable_pot_project`, `bayes_normal_mean_cli`, and `bayes_fused_gromov_wasserstein_cli` passed
  13/13, 1/1, 1/1, 2/2, and 1/1 respectively. Each durable external test performs one real backend
  miss and a second-process hit with backend startup disabled, then proves changed config and raw
  input are misses under the same disabled control. Exact analytic/differential oracles and
  repository lock/worker digests pass.
- `cargo +1.96.0 test --locked --package marklab-bayes` passed 41 unit tests and zero doc tests.
  `cargo +1.96.0 clippy --locked --package marklab-bayes --all-targets --all-features -- -D
  warnings` and `cargo +1.96.0 clippy --locked --package marklab --bin marklab --features cli -- -D
  warnings` passed. `cargo +1.96.0 check --locked --package marklab-bayes --no-default-features`
  and the corresponding root-package no-default check passed.
- A first attempted focused Cargo invocation supplied two positional test filters and exited with a
  usage error; the valid `normal_mean` filter and then the complete 41-test package command passed.
  Affected files were formatted directly with Rustfmt and `git diff --check` passed. No broad
  workspace/Nextest/feature-matrix/specialized gate was run or claimed. The user authorized one
  local cohesive-checkpoint commit before the next workstream; no push, deployment, publication,
  history rewrite, or worktree was created.

## Dependency, geometry, marks, and design checkpoint 54 — 2026-08-26

- Expected workflow reds were observed first. The focused `dependent_node_consumes_the_exact_registered_upstream_output`
  command failed with `DependenciesUnsupported`; `marked_prepost_composes_two_produced_analysis_artifacts`
  then failed to compile for the missing `MarkedPrePostNode`. The first
  `durable_marked_prepost_project` run failed because `project marked-prepost` was unrecognized.
  After implementation, `cargo +1.96.0 test --locked --package marklab --features cli --test
  durable_marked_prepost_project` passed 1/1 and proved miss/miss/miss then hit/hit/hit across
  processes with identical 0.3 output and exactly three ledger records.
- Direct diff review found that duplicated content references from two distinct dependencies were
  incorrectly counted twice. `cargo +1.96.0 test --locked --package marklab --features cli --test
  project_workflow byte_identical_dependency_outputs_remain_two_valid_edges -- --nocapture` failed
  with `AmbiguousDependencyOutput { matches: 2 }`, then passed after the scheduler counted distinct
  produced artifacts. The strengthened genuinely ambiguous two-output regression and durable reopen
  regression pass. The complete pre-fix project-workflow target passed 11/11; the two exact
  post-fix edge tests passed separately and no broader successful gate is implied.
- FND-02's exact signed-distance test first failed with missing-method `E0599`, then the complete
  `cargo +1.96.0 test --locked --package marklab --test classical_spatial_domain` target passed
  13/13. FND-04's missing-table behavior red preceded a 1/1 typed-table declared-workflow pass;
  `cargo +1.96.0 test --locked --package marklab --test scalar_mark_input typed_mark_table --
  --nocapture` passed both adversarial table tests. FND-06's partial-block regression failed before
  the compiled design boundary and the focused patient-permutation reference target passed 2/2.
- `cargo +1.96.0 test --locked --package marklab-project --lib` passed 15/15 and the chained
  `marklab-workflow --lib` command passed 1/1. A separate `cargo +1.96.0 test --locked --package
  marklab-cohort` attempt passed all 20 library tests plus energy, equivalence, functional,
  hierarchical-bootstrap, Max-T, and MMD reference binaries, then was interrupted while macOS was
  verifying the next binary. This is partial evidence, not a green package result, and the loop was
  not retried.
- `cargo +1.96.0 clippy --locked --package marklab-project --package marklab-workflow --package
  marklab-cohort --lib -- -D warnings` passed. The affected root library, binary, and five changed
  integration targets passed warning-denied Clippy with `--features cli --lib --bin marklab --test
  project_workflow --test durable_marked_prepost_project --test scalar_mark_input --test
  declared_marked_workflow --test classical_spatial_domain`.
- `cargo +1.96.0 check --locked --package marklab-project --package marklab-workflow --package
  marklab-cohort --package marklab --no-default-features` passed. Strict warning-denied public docs
  passed for those four packages. `cargo +1.96.0 fmt --all --check` and `git diff --check` passed.
  No workspace-wide tests/Clippy, Nextest, full feature matrix, specialized benchmark/fuzz/memory/
  packaging/dependency gate, commit, stage, push, publication, deployment, history rewrite, or
  worktree was run or created.

## Typed spatial autocorrelation checkpoint 55 — 2026-08-26

- `cargo +1.96.0 test --locked --package marklab --test global_moran_typed_workflow
  typed_frame_mark_and_compartment_design_drive_global_moran_inference -- --nocapture` first failed
  for the missing Moran API and `ObservationWindow2D::with_coordinate_frame`; after correcting the
  fixture's compatibility codes from strings to its actual dense `u32` representation, the exact
  behavior passed. The final two-test target passes the hand binary-symmetric `I=0.4`,
  row-standardized `I=0.54`, deterministic replay, typed conditioning identity/status, invalid
  frame, unbound frame, isolated row, invalid category/unit, and permutation-work boundaries.
- `cargo +1.96.0 test --locked --package marklab-cohort --test inference_design_reference --
  --nocapture` first failed for the missing public design symbols. The final chained
  `inference_design_reference` and `patient_permutation_reference` command passes 2/2 and 2/2,
  proving exact block preservation, deterministic whole-unit schedules, partial/singleton block
  rejection, replicate bounds, and unchanged slow patient-reference agreement.
- The affected root command running `scalar_mark_input`, `declared_marked_workflow`,
  `classical_spatial_domain`, and `global_moran_typed_workflow` passed 11/11, 9/9, 13/13, and 2/2.
  Two shared-support dead-code warnings in targets that do not use the new metadata fixture were
  removed with a test-only targeted annotation; production code remained warning-clean.
- Warning-denied Clippy passed for the `marklab-cohort` library and its two affected design tests,
  then for the root library plus Moran, scalar-mark, declared-workflow, and classical-domain tests.
  `cargo +1.96.0 check --locked --package marklab-cohort --package marklab
  --no-default-features` passed. Strict warning-denied public docs for both packages, workspace
  formatting, and `git diff --check` passed.
- No workspace-wide tests/Clippy, Nextest, feature matrix, benchmark, fuzz, memory, packaging,
  dependency audit, external-data claim, push, publication, deployment, history rewrite, or
  worktree was run or created.

## Durable typed Moran graph checkpoint 56 — 2026-08-26

- Expected red: `cargo +1.96.0 test --locked --test global_moran_project_workflow` failed to compile
  because `execute_algorithm_with_store`, `GlobalMoranAnalysisNode`, `GlobalMoranPrePostNode`, and
  `GlobalMoranPrePostResult` did not exist. The focused target then passed 1/1 after implementation.
- The passing end-to-end case builds two typed Moran sources and one exact dependent comparison,
  records miss/miss/miss, reconstructs the hierarchy, coordinate registry, provenance catalog,
  managed store, typed inputs, graph, and runtime in a fresh fixture, then records hit/hit/hit with
  the durable ledger unchanged at three. A seed-only change records miss/miss/miss and advances the
  ledger to six.
- `cargo +1.96.0 test --locked --package marklab-workflow --lib` passed 1/1. The exact existing
  `project_workflow durable_reopen_restores_dependency_outputs_without_reexecution` regression
  passed 1/1. The chained `global_moran_project_workflow` and `global_moran_typed_workflow` targets
  passed 1/1 and 2/2.
- Warning-denied Clippy passed for the affected `marklab-workflow` library and the root library plus
  both Moran integrations. No-default checks passed for `marklab-workflow` and `marklab`; strict
  warning-denied public docs generated successfully for both packages.
- Affected Rust files were formatted directly and `git diff --check` passed. No workspace-wide
  tests/Clippy, Nextest, full feature matrix, benchmark, fuzz, memory, packaging, dependency audit,
  external-data claim, push, publication, deployment, history rewrite, or worktree was run or
  created.

## Typed global Geary checkpoint 57 — 2026-08-26

- Expected red: the focused typed spatial-autocorrelation test failed to compile because
  `global_geary_permutation` and the typed Geary aliases/result did not exist.
- `cargo +1.96.0 test --locked --test global_moran_typed_workflow
  typed_frame_mark_and_compartment_design_drive_global_moran_inference -- --nocapture` passed 1/1,
  then the complete target passed 2/2. It proves `C=0.38` for six directed binary-symmetric chain
  edges, `C=0.2925` after row standardization, null expectation one, bounded compartment
  randomization, finite p-value, and exact shared weight identity with Moran.
- `cargo +1.96.0 test --locked --test global_moran_project_workflow` passed 1/1 after the shared
  error/design changes. Warning-denied Clippy passed for the root library and both affected Moran
  integrations; root no-default compilation and strict warning-denied public docs passed.
- `cargo +1.96.0 fmt --all --check` and `git diff --check` passed. No workspace-wide tests/Clippy,
  Nextest, feature matrix, benchmark, fuzz, memory, packaging, dependency audit, external-data
  claim, push, publication, deployment, history rewrite, or worktree was run or created.

## Observed scalar semivariogram checkpoint 58 — 2026-08-26

- Expected red: the focused typed spatial-autocorrelation test failed to compile because
  `scalar_semivariogram`, `ScalarVariogramBin`, and `ScalarVariogramLimits` did not exist.
- The exact focused behavior passed after implementation, then the complete
  `global_moran_typed_workflow` target passed 2/2. The four-point line proves bin counts `3,2,1`,
  hand semivariances `19/3`, `24.5`, and `32`, exact six-pair visitation, and pre-traversal rejection
  when the declared ceiling is five.
- Warning-denied Clippy passed for the root library and affected integration. Root no-default
  compilation and strict warning-denied public docs passed. After direct-review validation changes,
  the exact behavior test passed again 1/1.
- Affected files were formatted directly. No workspace-wide tests/Clippy, Nextest, feature matrix,
  benchmark, fuzz, memory, packaging, dependency audit, external-data claim, push, publication,
  deployment, history rewrite, or worktree was run or created.

## Three-workflow stabilization checkpoint 59 — 2026-08-26

- `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings` passed in
  11m59s with no diagnostics.
- `cargo +1.96.0 check --locked --workspace --no-default-features` passed.
- `cargo +1.96.0 test --locked --workspace --doc --all-features` passed all workspace doc-test
  targets. `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace --all-features
  --no-deps` passed and generated documentation for the root plus 16 other packages.
- Formatting and diff-whitespace checks pass. The full workspace integration suite and Nextest were
  not rerun because checkpoints 51/52 retain the latest complete/partial evidence and document the
  reproducible macOS loader-verification problem; the active instruction says not to retry it.

## Scalar-semivariogram ERL inference checkpoint 60 — 2026-08-26

- Expected red: `cargo +1.96.0 test --locked --test global_moran_typed_workflow
  typed_frame_mark_and_compartment_design_drive_global_moran_inference -- --nocapture` failed to
  compile because the scalar-semivariogram permutation function, design, limits, and work-bound
  error did not exist. After correcting the test insertion location, the red contained only those
  missing production symbols.
- The exact behavior passed 1/1 after implementation; the complete affected target passed 2/2. It
  proves three eligible bins, finite simultaneous bounds, finite inclusive global p-value, exact
  typed conditioning identity/status, 31 completed permutations, deterministic replay, invalid
  alpha-resolution rejection, and pre-execution rejection of `186 > 185` pair evaluations.
- Warning-denied Clippy passed for the root library and affected integration; root no-default
  compilation and strict warning-denied public docs passed. `cargo +1.96.0 fmt --all --check` and
  `git diff --check` pass.
- No workspace-wide gate was repeated after checkpoint 59 because this is one ordinary workflow and
  the production change does not invalidate unrelated packages. No Nextest, full integration loop,
  feature matrix, benchmark, fuzz, memory, packaging, dependency audit, external-data claim, push,
  publication, deployment, history rewrite, or worktree was run or created.

## Real CellViT result checkpoint 61 — 2026-08-26

- Admission ran on `mini` with the pinned CellViT Python 3.9/PyTorch 2.8 environment and the exact
  `marklab_cellvit_cptac_results_adapter.py` invocation over the frozen inference, projected-feature,
  spatial-result, case-map, clinical-label, inference-verification, spatial-verification, and
  transform paths. It rehashed 1,098 slide outputs and safely allowlisted the CellViT graph class.
  All 366 slides, 178 patients, 1,542,389 raw rows, 1,280 vector columns, coordinates,
  annotations/probabilities, projected-source rows, and cell-patch links passed; zero output,
  finiteness, shape, physical-scale, identity, or correspondence mismatches occurred.
- The real executions that passed were `marklab project classical` with 2,000 cells, 20 radii, 99
  CSR simulations, seed 20260826, 100,000,000 pair and 5,000,000 draw ceilings; `marklab analyze`
  with the pooled probabilistic scalar config; `marklab bayes vector-semivariogram` with 512 raw
  vectors and 130,816 pair visits; pinned-SciPy `projected-embedding-variograms` with 3,000 rows,
  four components, 20 permutations, and 2,000,000 pair visits; `multiscale-embedding-kernel`;
  pinned-SciPy `test-cell-patch-complementarity` with 96 patients, 5x4 nested folds, five ridge
  values, and 99 permutations; and `marklab cohort max-t` with 98 patients, four endpoints, and 999
  patient-label permutations.
- `marklab project normal-mean` passed on 170 patient observations with PyMC 6.3.0, two chains,
  500 tune plus 1,000 retained draws per chain, target acceptance 0.9, seed 20260826, and a
  180-second bound. A second process with `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1` returned
  `cache_status=hit`, byte-identical output, and one unchanged ledger record. The coordinate project
  separately returned miss then hit, identical canonical analysis content, and one ledger record.
- Expected red: the first real durable classical run failed private result decoding on redundant
  exact floating formula identity. The new focused one-ULP test failed with the same schema error,
  then passed after the bounded calculation tolerance. A first scalar preparation returned
  `insufficient_data` because separate component IDs were absent; that rejected output is retained
  under `diagnostic_failures`, and the corrected pooled run returned an available endpoint.
- `cargo +1.96.0 test --locked --test classical_spatial_workflow -- --exact
  classical_document_accepts_one_ulp_formula_roundoff_after_serialization` passed 1/1. Then
  `cargo +1.96.0 test --locked --features cli --test classical_spatial_workflow --test
  cellvit_results_bundle` passed 4/4 and 1/1. Direct Python compile and deterministic-selection
  checks passed.
- `cargo +1.96.0 clippy --locked --package marklab --all-targets --features cli -- -D warnings`
  passed in 10m44s. `cargo +1.96.0 check --locked --package marklab --no-default-features` passed.
  `cargo +1.96.0 fmt --all --check` passed before documentation closure.
- The hardened adapter was rerun over the full real corpus into an isolated validation directory;
  all 11 final prepared input files were byte-identical after the clinically complete fold/group
  rebuild. Two earlier nonidentical complementarity/multiscale results are retained as rejected
  diagnostics and are not used as evidence.
- `marklab_cellvit_results_bundle.py seal` and `verify` passed over 54 files. Verification also
  passed at `/Volumes/1TB/marklab/runs/results-cellvit-2day-01`; a copied bundle with one appended
  byte failed on the expected result digest. No workspace-wide test/Clippy, Nextest, feature matrix,
  strict docs, benchmark, fuzz, memory, packaging, dependency audit, push, publication, deployment,
  history rewrite, or worktree was run or created.

## Durable real Bayesian checkpoint 62 — 2026-08-26

- Expected behavior reds were observed first. The new durable hierarchy and gridded-LGCP tests each
  failed because their `marklab project` subcommand was absent. The first hierarchy implementation
  correctly returned `nonconverged` for a divergent seed, so the durable test adopted the existing
  established hierarchy oracle seed without weakening its complete-fit assertion. The cross-backend
  test then failed for the missing agreement command, isolated-worker module import, and installed
  ArviZ 1.3 `from_dict` API before passing with explicit file loading and the version-matched API.
- `cargo +1.96.0 test --locked --features cli --test durable_pymc_hierarchical_project --
  --exact hierarchical_normal_runs_once_then_replays_without_starting_pymc --nocapture` passed
  1/1, as did the corresponding exact `durable_pymc_gridded_lgcp_project` behavior. After the Python
  lock changed to declare its already pinned NumPyro package directly, the combined two-target
  durable command passed both tests again. Each test proves miss, backend-disabled second-process
  hit, byte-identical typed output, changed seed/input misses, and one unchanged ledger record.
- The existing `bayes_hierarchical_normal_cli` and `bayes_gridded_lgcp_fit_cli` exact oracle tests
  each passed 1/1 after their prepare/execute refactors. `cargo +1.96.0 test --locked --features cli
  --test bayes_hierarchical_agreement_cli -- --exact
  pymc_and_numpyro_agree_on_the_same_typed_patient_hierarchy --nocapture` passed 1/1. The static
  backend identity unit test passed 1/1; an earlier invocation with `--exact` named only the
  unqualified suffix and ran zero tests, so it is not evidence.
- The real hierarchy and LGCP each ran under fresh current-lock durable projects as miss then hit
  with `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1`; `cmp` passed and each ledger contains one
  record. The real PyMC/NumPyro comparison passed both parameter gates with zero divergences. The
  final CellViT LGCP adapter reproduced its three prepared files byte-for-byte in a fresh remote
  temporary directory. Current artifacts were copied without deletion to
  `/Volumes/1TB/marklab/runs/results-cellvit-bayesian-v1`.
- `cargo +1.96.0 test --locked --package marklab-bayes` passed 41 unit tests and zero doc tests.
  Warning-denied Clippy passed for all `marklab-bayes` targets and for the root binary plus the three
  affected integrations. Root and Bayesian no-default compilation passed. Strict warning-denied
  public docs for both packages, `cargo +1.96.0 fmt --all --check`, Python syntax compilation,
  the exact adapter unit test, and `git diff --check` passed.
- No workspace-wide test/Clippy, Nextest, feature matrix, benchmark, fuzz, memory, packaging,
  dependency audit, push, publication, deployment, history rewrite, or worktree was run. The
  broader Phase 4 exit is not claimed; hierarchy SBC/sensitivity and field/point-process
  cross-backend calibration remain active.

## Hierarchical calibration and five-workflow stabilization checkpoint 63 — 2026-08-26

- Expected reds: the exact sensitivity and SBC integrations first failed because their CLI
  subcommands were absent. The first five-scenario sensitivity green attempt returned an honest
  nonconverged aggregate; with its assertion unchanged, higher target acceptance and warmup made
  all five scenarios complete. `cargo +1.96.0 test --locked --features cli --test
  bayes_hierarchical_prior_sensitivity_cli -- --exact
  patient_hierarchy_reports_one_at_a_time_prior_sensitivity --nocapture` then passed 1/1.
- `cargo +1.96.0 test --locked --features cli --test bayes_hierarchical_sbc_cli -- --exact
  numpyro_hierarchy_sbc_has_bounded_ranks_coverage_and_failures --nocapture` passed 1/1 with 20
  complete simulated fits, bounded ranks, empty failures, both rank-histogram totals, uniformity
  gates, and 90% coverage gates. Python source compilation passed for the SBC worker.
- The real prior-sensitivity command ran five 121-patient/284-ROI fits at baseline and 0.5x/2x
  one-at-a-time global/between-prior scales, using two chains, 2,000 warmup, 2,000 retained draws,
  target acceptance 0.99, and seed 20260826. All fits are complete with zero divergences; the
  maximum standardized posterior shift is `0.128366`, below the prespecified `0.5` threshold.
- The first real-prior SBC schedule (40 replicates, 32 patients, two observations, 500 warmup,
  1,000 draws) correctly returned diagnostic-only with 21 failed per-fit ESS/R-hat gates. The
  second (2,000 warmup, 4,000 draws) retained two failed fits. The maximum admitted schedule
  (4,000 warmup, 8,000 draws, two chains, 40 replicates, 960,000 bounded total iterations) passed
  all 40 without changing thresholds: zero divergences, maximum R-hat `1.00435`, minimum bulk/tail
  ESS `570/1019`, rank p-values `0.163/0.312`, and both 90% coverages `0.875`.
- Focused warning-denied Clippy passed for the Bayesian library and each root binary/integration
  target after sensitivity and SBC. Root/Bayesian no-default checks and strict docs passed. At the
  five-workflow boundary, `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features
  -- -D warnings` passed in 12m58s; workspace no-default passed; workspace all-feature doc tests and
  strict all-feature docs passed; formatting and `git diff --check` pass.
- The full workspace integration suite and Nextest were not rerun because checkpoints 51/52 record
  the reproducible macOS binary-verification loop and the active instruction forbids retrying it.
  No feature matrix, benchmark, fuzz, memory, packaging, dependency audit, push, publication,
  deployment, history rewrite, or worktree was run.

## Gridded-LGCP calibration stabilization checkpoint 64 — 2026-08-27

- Expected reds: each of `gridded-lgcp-agreement`, `gridded-lgcp-sbc`,
  `gridded-lgcp-spatial-ppc`, and `gridded-lgcp-sensitivity` first failed as an absent subcommand.
  Each exact integration then passed after its smallest production path was implemented.
- `cargo +1.96.0 test --locked --package marklab-bayes` passed 41/41 package tests after each
  affected boundary. The exact original PyMC gridded-LGCP fit test and all four new CLI integrations
  passed. Both NumPyro worker sources passed isolated Python compilation.
- The real CellViT cross-backend result passes all global/cell comparison gates with zero
  divergences. The final 20-replicate SBC passes all dispositions and aggregate gates after the
  preserved 17/20 diagnostic run. The final nine-fit sensitivity grid passes after the preserved
  one-fit ESS failure, and the 32-pattern spatial PPC retains all exact draw/cell/point provenance.
- Six new result JSON files were copied without deletion to the authorized Mac mini directory
  `/Volumes/1TB/marklab/runs/results-cellvit-bayesian-v1/results`; local and remote SHA-256 checks
  match for agreement, both SBC schedules, spatial PPC, and both sensitivity schedules.
- `cargo +1.96.0 fmt --all --check` passed. `cargo +1.96.0 clippy --locked --workspace
  --all-targets --all-features -- -D warnings` passed in 13m07s. `cargo +1.96.0 check --locked
  --workspace --no-default-features`, `cargo +1.96.0 test --locked --workspace --doc
  --all-features`, and `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace
  --all-features --no-deps` passed. `git diff --check` passed.
- The full workspace integration suite and Nextest were not rerun because checkpoints 51/52 record
  the reproducible macOS binary-verification loop and the active instruction forbids retrying it.
  No feature matrix, benchmark, fuzz, memory, packaging, dependency audit, push, publication,
  deployment, history rewrite, or worktree was run.

## Robust non-Gaussian hierarchy stabilization checkpoint 65 — 2026-08-27

- Expected reds: the Student-t hierarchy, durable project, cross-backend agreement, sensitivity,
  and SBC integrations each first failed because their exact CLI/project subcommand was absent.
  The SBC green loop retained strict `R-hat`, ESS, E-BFMI, divergence, depth-hit, rank-uniformity,
  and coverage gates. Small six-patient schedules exposed one R-hat/ESS failure; a 12-patient
  schedule exposed one divergence; four-chain target-accept `0.99` exposed two depth-10 hits for a
  near-zero scale draw. The final fixed depth-12 execution passed without relaxing any gate.
- `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_student_t_hierarchy_sbc_cli -- --nocapture` passed 1/1 in `216.97s` with 20 complete
  dispositions and four passing rank/coverage families. `workers/python/.venv/bin/python -m
  py_compile workers/python/marklab_numpyro_student_t_hierarchy_sbc_worker.py` passed.
  `cargo +1.96.0 test --locked --package marklab-bayes --lib` passed 44/44.
- The combined affected command over `bayes_beta_binomial_hierarchy_cli`,
  `bayes_student_t_hierarchy_cli`, `durable_pymc_student_t_hierarchy_project`,
  `bayes_student_t_hierarchy_agreement_cli`, and
  `bayes_student_t_hierarchy_sensitivity_cli` passed 5/5. This proves the beta-binomial oracle,
  robust one-shot fit, independent PyMC/NumPyro agreement, nine-fit prior/tail grid, and
  cross-process miss then backend-disabled durable hit.
- The real-shape four-chain Student-t SBC command exceeded its declared `1200`-second backend limit
  and published no output. The sole bounded retry used two chains, 2,000 warmup, 4,000 draws,
  target acceptance `0.99`, and seed `20260827`; it passed 20/20 on the exact 121-patient/284-
  observation shape with zero failures, divergences, or depth hits. Rank p-values are
  `0.5341/0.6371/0.9114/0.4373` and 90% coverages are `0.95/0.80/0.90/0.90` for population mean,
  between-patient SD, observation SD, and degrees of freedom. Maximum R-hat is `1.00277`; minimum
  bulk/tail ESS and E-BFMI are `1065/2121/0.441`.
- Local and Mac mini SHA-256 checks both returned
  `b2b192a45c19264d9eb44c8c6341725078cba59a46c14e4f925739235db239c4` for
  `student_t_hierarchy_roi_cosine_excess_sbc.json` at the authorized local result directory and
  `/Volumes/1TB/marklab/runs/results-cellvit-bayesian-v1/results`.
- `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings` passed in
  `13m11s`. `cargo +1.96.0 check --locked --workspace --no-default-features`, `cargo +1.96.0 test
  --locked --workspace --doc --all-features`, `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc
  --locked --workspace --all-features --no-deps`, `cargo +1.96.0 fmt --all --check`, and
  `git diff --check` passed.
- The full workspace integration suite and Nextest were not rerun because checkpoints 51/52 record
  the reproducible macOS loader-verification loop and the active instruction forbids retrying it.
  No feature matrix, benchmark, fuzz, memory, packaging, dependency audit, push, publication,
  deployment, history rewrite, or worktree was run.

## Real CellViT count-hierarchy stabilization checkpoint 66 — 2026-08-27

- Expected reds: `cellvit_beta_binomial_input`, durable beta-binomial, agreement, sensitivity, and
  SBC integrations first failed at their exact missing helper/subcommand. The adapter helper test
  then passed 1/1 and direct Python compilation passed. The pinned full-corpus remote admission
  verified 366 slides, 178 patients, and 1,542,389 cells; `cmp` proved all 11 prior prepared inputs
  unchanged. The initial invocation failed before data processing because the recorded CellViT
  source root was absent from `PYTHONPATH`; its staging directory is preserved, and the corrected
  command passed.
- The real count table has SHA-256
  `d9cbd1afe3a8f86f25d5dec825de4af125f62cd2eeaacecb6a97fc37ee2e4dbb` locally and remotely.
  `marklab bayes beta-binomial-hierarchy` completed on 178 patients with two chains, 2,000 warmup,
  4,000 draws, target acceptance `0.99`, and seed `20260827`. Its result SHA is
  `2d7e1e9f6b9cdf8fa464bccce18315e4b5e7d33c5c6fc4c8eeab37b37576a88f`.
- `durable_pymc_beta_binomial_project` passed 1/1, proving miss, separate-process backend-disabled
  byte-identical hit, seed invalidation, and one ledger row. The real project repeated that behavior
  and both durable outputs match the one-shot SHA locally and on the Mac mini.
- `bayes_beta_binomial_hierarchy_agreement_cli` passed 1/1. The real PyMC/NumPyro result passed
  population, concentration, and all 178 patient gates with zero divergences/depth hits; its local
  and remote SHA is `b8adb0f31e9398259d2532ce0cccdfdd1ac90c3df1714428dd658801c902047a`.
  `bayes_beta_binomial_hierarchy_sensitivity_cli` passed 1/1; all seven real fits passed unchanged
  diagnostics and the result SHA is
  `fd07bd2906d5eea461a064bf7fc950205d711d641f1d584d9693c768e2133737` locally/remotely.
- `bayes_beta_binomial_hierarchy_sbc_cli` passed 1/1 both before and after the exact collapsed
  parameterization, finally in `36.39s`. The first real latent fit retained one of 20 failures with
  6,000/6,000 depth-12 hits and SHA
  `930f60efec95dbeb99350ad2d9ef13a9e0bce4fdec060315ad785f10415057c8`. The exact collapsed result
  passed 20/20 with unchanged gates and SHA
  `267d94f176f0c4ce6ad68d0343e83142c70dc4f257aa6fa0af55be70af199065`; both artifacts are preserved
  locally and remotely. `cargo +1.96.0 test --locked --package marklab-bayes --lib` passed 44/44
  after the agreement and SBC boundaries.
- `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings` passed in
  `14m34s`. `cargo +1.96.0 check --locked --workspace --no-default-features`, `cargo +1.96.0 test
  --locked --workspace --doc --all-features`, `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc
  --locked --workspace --all-features --no-deps`, `cargo +1.96.0 fmt --all --check`, and
  `git diff --check` passed.
- The full workspace integration suite and Nextest were not rerun because checkpoints 51/52 record
  the reproducible macOS loader-verification loop and the active instruction forbids retrying it.
  No feature matrix, benchmark, fuzz, memory, packaging, dependency audit, push, publication,
  deployment, history rewrite, or worktree was run.

## Patient molecular-group hierarchy stabilization checkpoint 67 — 2026-08-27

- Expected reds were observed before production changes: the CellViT adapter test failed because
  `beta_binomial_group_rows` was absent; the one-shot PPC assertion failed because aggregate group
  tail probabilities were absent; and the durable, agreement, sensitivity, and SBC integrations
  each failed because their exact subcommand was absent. The durable test also exposed the root
  CLI routing allowlist before passing; no assertion or diagnostic gate was weakened.
- `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_beta_binomial_group_regression_cli -- --nocapture` passed 1/1. `cargo +1.96.0 test --locked
  --package marklab --features cli --test durable_pymc_beta_binomial_group_project -- --nocapture`
  passed 1/1 and proves miss, backend-disabled separate-process hit, byte-identical typed output,
  seed invalidation, and one ledger row. The agreement, sensitivity, and SBC test targets each
  passed 1/1; the SBC oracle completed 20/20 dispositions.
- The full remote admission reran the pinned CellViT adapter over all 366 slides and 1,542,389 cells.
  Its 105-patient group table SHA is
  `e9a875daf3456895d31ecb8175de9e44637ebf3633fb6e8fb4ed16c1b2b6d6a1`; the group-provenance SHA is
  `18e716e6824b5b5a658bdeaa89944b6cf13969635d5fd0566f443e85130cfb03`. All prior admission inputs
  compared byte-identically. The real one-shot and durable miss/hit result SHA is
  `f51dfff193af353173c33a3bb159a1bb40b1bbf9d6911ed31b206f42a046c603`; `cmp` passed and
  `wc -l .../executions.jsonl` returned one.
- The real PyMC/NumPyro agreement result passed every declared parameter and all 105 patient gates
  with SHA `d46fecc300d058ef43ec42a006719fc19868f2984939dfa5255ea2b77cfa0cea`. The complete seven-fit
  prior grid has SHA `91aea9178d685d7ef74a419edd9d0b3be19739bfc4c32a1aa47df8434075848a`.
  The complete 20-replicate real-shape SBC has SHA
  `d77122a677699b536932810bbe884f1e5d008b72c644b1ec845ee5e1352a6736`. Each file was copied without
  deletion to `/Volumes/1TB/marklab/runs/results-cellvit-bayesian-v1/results`.
- `cargo +1.96.0 test --locked --package marklab-bayes --lib` passed 45/45 after the affected
  boundaries. The final combined command over `cellvit_beta_binomial_input`,
  `bayes_beta_binomial_group_regression_cli`, `durable_pymc_beta_binomial_group_project`,
  `bayes_beta_binomial_group_regression_agreement_cli`,
  `bayes_beta_binomial_group_regression_sensitivity_cli`, and
  `bayes_beta_binomial_group_regression_sbc_cli` passed all six serial integrations.
- `cargo +1.96.0 fmt --all --check` passed. `cargo +1.96.0 clippy --locked --workspace
  --all-targets --all-features -- -D warnings` passed in `14m54s`. `cargo +1.96.0 check --locked
  --workspace --no-default-features`, `cargo +1.96.0 test --locked --workspace --doc
  --all-features`, and `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace
  --all-features --no-deps` passed. The four affected Python sources passed pinned-environment
  syntax compilation, and `git diff --check` passed.
- The full workspace integration suite and Nextest were not rerun because checkpoints 51/52 record
  the reproducible macOS loader-verification loop and the active instruction forbids retrying it.
  No feature matrix, benchmark, fuzz, memory, packaging, dependency audit, push, publication,
  deployment, history rewrite, or worktree was run.

## Gender-adjusted molecular-group hierarchy stabilization checkpoint 68 — 2026-08-27

- Expected reds were observed before production changes: the exact fit, durable, agreement, and
  sensitivity integrations failed for their missing adjusted behavior/subcommands; the SBC test
  failed with the absent `beta-binomial-group-gender-regression-sbc` subcommand. No assertion,
  diagnostic threshold, coverage interval, or failure disposition was weakened.
- The full adapter admission revalidated 366 slides and 1,542,389 row-aligned cells and admitted all
  105 MSI/MSS patients into four nondegenerate group/gender cells. The raw table SHA is
  `8f6e6c433d34793b31ad59255e73194732db448390c8196715c96e247e84e4e0`; the admission SHA is
  `3c8fa133737880561dd127e15d7b9996c2969cdcfc2688a4fe3e20ca09951fda`. Prior prepared inputs
  compared byte-identically.
- The real PyMC and durable miss/hit outputs share SHA
  `f88ca744929d3ea492fffb33bdd9fb2083dcc77eb7cabe9abd7ffafd2de90499`; the hit was produced by a
  backend-disabled separate process and one ledger row remained. The complete PyMC/NumPyro
  agreement result SHA is `8023c116ba1d2fd787b0e207332f6e226ae3e47b2d29b9afedcf16766298caea`;
  every one of 15 scalar and 105 patient-probability gates passed. The complete nine-fit sensitivity
  result SHA is `be23541b9a36c785a69ace3193493b96684641430798f8cfcf18c56576ddef2c`.
- The initial focused SBC command `cargo +1.96.0 test --locked --package marklab --features cli
  --test bayes_beta_binomial_group_gender_sbc_cli -- --nocapture` passed 1/1 after the expected red.
  The real command used the exact 105-patient shape with 20 replicates, two chains, 2,000 warmup,
  4,000 draws, target acceptance `0.99`, seed `20260827`, and a 1,200-second bound. It completed
  20/20 dispositions with no failure and produced SHA
  `4d148ba728fb3bea6d22f076be3c6c754c373da8b1fc9a86b8171711477cb60c` locally and on the Mac mini.
- `cargo +1.96.0 test --locked --package marklab-bayes --lib` passed 46/46. One serial combined
  command over `cellvit_beta_binomial_input`, `bayes_beta_binomial_group_gender_regression_cli`,
  `durable_pymc_beta_binomial_group_gender_project`,
  `bayes_beta_binomial_group_gender_agreement_cli`,
  `bayes_beta_binomial_group_gender_sensitivity_cli`, and
  `bayes_beta_binomial_group_gender_sbc_cli` passed all six integrations.
- `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings` passed in
  `15m20s`. `cargo +1.96.0 check --locked --workspace --no-default-features`, `cargo +1.96.0 test
  --locked --workspace --doc --all-features`, `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc
  --locked --workspace --all-features --no-deps`, `cargo +1.96.0 fmt --all --check`, four-source
  pinned-Python in-memory syntax compilation, and `git diff --check` passed.
- The full workspace integration suite and Nextest were not rerun because checkpoints 51/52 record
  the reproducible macOS loader-verification loop and the active instruction forbids retrying it.
  No feature matrix, benchmark, fuzz, memory, packaging, dependency audit, push, publication,
  deployment, history rewrite, or worktree was run.

## Canonical CRC spatial-fingerprint result checkpoint 69 — 2026-08-27

- Expected behavior reds were observed before production changes: the coordinate, M2 fingerprint,
  and stability adapters were absent; each new M3/M4/M6/CODEX/bundle unit test first failed on its
  missing module or behavior; and the first CODEX durable run failed exact decoding because numeric
  patient order differed from canonical lexical order by two ULPs. The ordering fix made the exact
  durable path pass without relaxing the codec or adding a tolerance. M4 review also found pooled
  slide coordinate frames before use; a red distance-preservation/cross-field-exclusion test drove
  the fixed 1,000-micrometre field framing, and the superseded outputs remain quarantined.
- Six fixed M2 shards completed `668/668` durable retrieval folds. Six bounded field shards produced
  every selected full and 80% subsampled result for the first four provenance-sorted fields per
  patient. M3, corrected M4, and M6 completed `676/676`, `652/652`, and `664/664` durable folds;
  four Schürch CODEX shards completed `34/34`. Fresh separate processes reported
  `cache_status=hit` for M2, M3, M4, M6, and CODEX, reproduced the original SHA-256 byte-for-byte,
  and left each owning shard ledger row count unchanged.
- Corrected M4 preprocessing ran `marklab bayes vector-semivariogram` on 169 patient inputs, 169
  deterministic 80% cell subsamples, and 661 eligible provenance-sorted field inputs using the
  existing 0–25, 25–50, and 50–100 micrometre bins and a 1,000-pair-visit ceiling. M2 stability,
  M3 bounded raw-summary stability, and M4 cell/field/nearby-scale summaries were sealed with exact
  source and output hashes. No cell, field, core, slide, or edge was counted as an independent
  patient replicate.
- `marklab cohort energy` and `marklab cohort mmd --kernel linear --estimator unbiased` each ran
  1,999 patient-label permutations for M2, M3, M4, M6, and Schürch CODEX using seed `20260827`.
  M2, M3, M4, M6, and CODEX energy p-values were `0.2175`, `0.125`, `0.2035`, `0.33`, and `0.914`;
  their MMD p-values were `0.295`, `0.1935`, `0.126`, `0.2335`, and `0.659`.
- The existing pinned M7 result was rehashed and admitted without a new backend run. Its PyMC 6.3.0
  input covers 105 patients/217 slides/81 repeated patients; diagnostics are complete with zero
  divergences and `R-hat=1.00232`. The existing durable miss/hit files both have SHA-256
  `a0d211414e4a8c0b16c2f682d5bb7f85aa9b2b549996a347b9b36f0c595f847a`, and the project ledger
  contains one successful execution.
- Every `tests/python/test_*.py` script passed (11 files, 13 tests), and all 15 affected worker
  sources passed `python3 -m py_compile`. The focused Cargo command over
  `cellvit_crc_coordinate_input`, `cellvit_crc_coordinate_stability_input`,
  `cellvit_crc_m2_fingerprint_input`, and `cellvit_crc_retrieval_summary` passed 4/4 after the
  expected one-time macOS verification delay. Targeted warning-denied Clippy for those four
  integrations passed; `cargo +1.96.0 check --locked --package marklab --no-default-features` and
  `cargo +1.96.0 fmt --all --check` passed.
- The sealed bundle has 183 artifact hashes, nine stability hashes, 29,087 patient-feature rows,
  and 4,969 specimen-feature rows. Local and Mac mini SHA-256 values match for all five bundle
  files. The full workspace/Nextest loop, workspace-wide Clippy/tests/docs, feature matrix,
  benchmarks, fuzzing, memory tools, packaging, dependency audits, push, publication, deployment,
  and history rewriting were not run.

## Exact nearest/empty-space checkpoint 70 — 2026-08-27

- The first domain test failed on unresolved F/G/J API symbols, and the first CLI integration
  failed because `nearest-space` was not a command. Production behavior was added only after those
  expected reds. The final focused domain suite passes 5/5: hand F/G/J identities, independent
  brute-force comparison, polygon-hole probe exclusion, query-limit failure, empty/singleton strict
  document round trips, finite J suppression, and the pinned external fixture.
- `workers/python/.venv/bin/python tests/fixtures/nearest_space/generate_scipy_oracle.py | diff -u
  tests/fixtures/nearest_space/scipy_rectangle_oracle.json -` passes byte-for-byte under the locked
  SciPy 1.18.1 environment. The fixture independently uses `scipy.spatial.cKDTree` for event and
  probe nearest distances and agrees with every Rust denominator, numerator, F, G, and available J.
- `cargo +1.96.0 test --locked --package marklab --test nearest_space_calibration` passes 2/2.
  Forty deterministic conditional-CSR patterns exercise 80 componentwise F/G global tests without
  gross anti-conservatism; prespecified clustered/inhibited controls put J below/above one. The
  affected 13-test classical spatial domain suite also passes unchanged.
- `cargo +1.96.0 test --locked --package marklab --features cli --test nearest_space_cli` passes.
  Independent processes produce miss then hit with identical typed analysis and one ledger row;
  changing only the scientific seed produces a distinct miss/cache key and a second ledger row.
- Targeted warning-denied Clippy for the three nearest-space integrations passes. Package
  no-default compilation, full formatting check, Python oracle syntax compilation, and diff
  whitespace checks pass. Workspace-wide tests/Clippy/docs, Nextest, feature matrices, benchmarks,
  fuzzing, memory tools, packaging, dependency audits, push, publication, deployment, and history
  rewriting were not run.

## Typed categorical pair checkpoint 71 — 2026-08-27

- `cargo +1.96.0 test --locked --package marklab --test categorical_pair_typed_workflow` first
  failed on unresolved categorical-pair production symbols. The final 3/3 tests pass: the exact
  line hand oracle, deterministic replay, same-level/work-bound failures, and the prespecified
  alternating-versus-segregated connection/cross-K direction control.
- `cargo +1.96.0 test --locked --package marklab --test categorical_pair_project_workflow` passes.
  A fresh durable project misses once, a separately
  reconstructed project/input/store reopens as an exact hit with one ledger execution, and a seed
  change creates a second miss/execution. Semantic MarkTable provenance is verified through the
  existing artifact store before miss or hit.
- Targeted warning-denied Clippy over both integrations passes after replacing one manual saturating
  resource calculation. `cargo +1.96.0 check --locked --package marklab --no-default-features`,
  `cargo +1.96.0 fmt --all --check`, and `git diff --check` pass. No full-workspace/Nextest loop,
  workspace-wide Clippy/docs, feature matrix, benchmark, fuzz, memory, packaging, dependency,
  push, publication, deployment, or history-rewrite action was run.

## Expected probability-pair checkpoint 72 — 2026-08-27

- `cargo +1.96.0 test --locked --package marklab --test probability_pair_typed_workflow` first
  failed on the expected unresolved probability-pair production symbols. The final 3/3 tests pass:
  exact expected-contribution and without-replacement hand identities, deterministic replay,
  missing/rare/all-positive/work failures, and clustered-versus-alternating direction.
- `cargo +1.96.0 test --locked --package marklab --test probability_pair_project_workflow` first
  failed on the expected unresolved durable node. It now passes 1/1: fresh miss, separately
  reconstructed hit with one ledger execution, and seed-invalidated second miss/execution.
- The affected categorical extraction regressions pass 3/3 typed and 1/1 durable. Targeted
  warning-denied Clippy over all four integrations and `cargo +1.96.0 check --locked --package
  marklab --no-default-features` pass. `cargo +1.96.0 test --locked --package marklab --doc`
  passes with zero doctests. Formatting and whitespace checks pass after the final documentation
  update. Workspace-wide tests/Clippy/docs, Nextest, feature matrices,
  benchmarks, fuzzing, memory tools, packaging, dependency audits, push, publication, deployment,
  and history rewriting were not run.

## Normalized continuous mark-correlation checkpoint 73 — 2026-08-27

- `cargo +1.96.0 test --locked --package marklab --test continuous_mark_correlation_typed_workflow`
  first failed on the expected unresolved production
  symbols. The final 5/5 tests pass: hand identities, independent Python direct-pair agreement,
  deterministic replay, constant/missing/pair/memory failures, dominant-mark cancellation
  resistance, and similar-versus-alternating direction.
- `workers/python/.venv/bin/python tests/fixtures/continuous_mark_correlation/generate_python_oracle.py
  | diff -u tests/fixtures/continuous_mark_correlation/python_line_oracle.json -` passes
  byte-for-byte. The
  available R installation lacks `spatstat.explore`; no package was installed and no spatstat claim
  is made.
- `cargo +1.96.0 test --locked --package marklab --test
  continuous_mark_correlation_project_workflow` first failed on the expected unresolved durable
  node and now passes 1/1: fresh miss, separately reconstructed exact hit with one execution, and
  seed-invalidated second miss/execution.
- The ERL-workspace accounting correction and one-byte-short bounds pass the affected categorical
  3/3 typed plus 1/1 durable tests and probability 3/3 typed plus 1/1 durable tests.
  Warning-denied Clippy over all six integrations, package no-default compilation, affected package
  docs with zero doctests, final formatting, and diff whitespace checks pass. Workspace-wide
  tests/Clippy/docs, Nextest, feature matrices, benchmarks, fuzzing, memory tools, packaging,
  dependency audits, push, publication, deployment, and history rewriting were not run.

## Cumulative mark-weighted K checkpoint 74 — 2026-08-27

- `cargo +1.96.0 test --locked --package marklab --test mark_weighted_k_typed_workflow` first
  failed on the expected unresolved production symbols and now passes 4/4: weighted/unweighted hand
  identities, independent Python cumulative-pair agreement, deterministic replay, constant/missing/
  memory failures, and similar-versus-alternating separation with invariant unweighted K.
- `workers/python/.venv/bin/python tests/fixtures/mark_weighted_k/generate_python_oracle.py | diff
  -u tests/fixtures/mark_weighted_k/python_line_oracle.json -` passes byte-for-byte under the existing
  pinned environment. The unavailable `spatstat.explore` package recorded at checkpoint 73 was not
  installed or retried.
- `cargo +1.96.0 test --locked --package marklab --test mark_weighted_k_project_workflow` first
  failed on the expected unresolved durable node and now passes 1/1: fresh miss, separately
  reconstructed exact hit with one execution, and seed-invalidated second miss/execution.
- Targeted warning-denied Clippy over both integrations, package no-default compilation, affected
  package docs with zero doctests, final formatting, and diff whitespace checks pass. Workspace-wide
  tests/Clippy/docs, Nextest, feature matrices, benchmarks, fuzzing, memory tools, packaging,
  dependency audits, push, publication, deployment, and history rewriting were not run.
- The finite-`pi r^2` guard passes the affected 13/13 classical spatial domain tests, 3/3
  categorical-pair tests, and 4/4 weighted-K tests, including explicit `f64::MAX` radius rejection.

## Typed spatial-workflow stabilization checkpoint 75 — 2026-08-27

- `cargo +1.96.0 fmt --all --check` passes.
- The first `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`
  failed on `clippy::type_complexity` for the pre-existing durable region-retrieval
  `standardize` tuple return. The smallest cleanup renamed the calculation `standardization`,
  returned only the consumed mean and scale, and removed one redundant standardized-matrix build.
- `cargo +1.96.0 test --locked --package marklab --features cli --test
  durable_region_retrieval_project --test bayes_region_retrieval_cli` passes 1/1 plus 1/1 after
  that cleanup. The repeated exact workspace Clippy command then passes in 18m13s.
- `cargo +1.96.0 check --locked --workspace --no-default-features` passes, and `cargo +1.96.0 test
  --locked --workspace --doc --all-features` passes for every workspace package with zero doctest
  failures.
- `cargo +1.96.0 test --locked --workspace --all-features -- --test-threads=1` compiled the test
  profile in 16m50s; the main library passed 294 tests with 21 ignored, the CLI unit target passed
  3/3, and `api_contract` passed 6/6. The run was interrupted with exit 130 when the following
  integration reproduced the documented tens-of-seconds macOS loader-verification delay. It was
  not retried; Nextest, feature matrices, benchmarks, fuzzing, memory tools, packaging, dependency
  audits, push, publication, deployment, and history rewriting were not run.

## Homogeneous pair-correlation checkpoint 76 — 2026-08-27

- `cargo +1.96.0 test --locked --package marklab --test pair_correlation_typed_workflow` first
  failed on unresolved production symbols and passes 5/5 after implementation: hand formula,
  independent Python agreement, on/off-radius direction, invalid support, finite-output rejection,
  deterministic replay, typed empty support including zero-weight kernel endpoints, and
  one-byte-short memory. The endpoint regression first reproduced an assertion panic and passes
  after both compact-support partitions exclude the zero-weight boundary.
- `cargo +1.96.0 test --locked --package marklab --test pair_correlation_project_workflow` first
  failed on the absent durable node and passes 1/1: fresh miss, reconstructed exact hit with one
  execution, and seed-invalidated second miss.
- `cargo +1.96.0 test --locked --package marklab --test
  categorical_cross_pair_correlation_typed_workflow` first failed on unresolved native and durable
  symbols and now passes 4/4: exact hand/Python identities, alternating-versus-segregated direction,
  invalid support, unknown levels, one-byte-short memory, deterministic replay, store-verified
  cross-process hit, and seed invalidation.
- `workers/python/.venv/bin/python tests/fixtures/pair_correlation/generate_python_oracle.py |
  diff -u tests/fixtures/pair_correlation/python_line_oracle.json -` and
  `workers/python/.venv/bin/python
  tests/fixtures/categorical_cross_pair_correlation/generate_python_oracle.py | diff -u
  tests/fixtures/categorical_cross_pair_correlation/python_line_oracle.json -` pass byte-for-byte.
  The pinned environment has no `spatstat.explore`; it was not installed and no spatstat agreement
  claim is made.
- The affected regression command passes categorical cross-g 4/4, categorical cross-K typed 3/3
  plus durable 1/1, and homogeneous g typed 5/5 plus durable 1/1. Seed namespace tests pass 2/2.
  Targeted warning-denied Clippy over those five exact integrations passes. An earlier command that
  accidentally included Cargo `--tests` was interrupted with exit 130 after 23 minutes because it
  expanded to unrelated integration targets; it is not claimed as a package-wide Clippy pass.
- `cargo +1.96.0 check --locked --package marklab --no-default-features`, `cargo +1.96.0 test
  --locked --package marklab --doc`, `cargo +1.96.0 fmt --all --check`, and `git diff --check` pass.
  Workspace/Nextest loops, feature matrices, benchmarks, fuzzing, memory tools, packaging,
  dependency audits, push, publication, deployment, and history rewriting were not run.

## Leave-one-out inhomogeneous K/L checkpoint 77 — 2026-08-27

- `cargo +1.96.0 test --locked --package marklab --test inhomogeneous_spatial_typed_workflow`
  first failed on unresolved production symbols and now passes 3/3: the exact intensity-to-K/L
  oracle, deterministic replay, persisted fixed grid, polygon-hole probe exclusion, singleton,
  invalid radius, near-zero intensity, and one-short memory/intensity/pair/null-draw ceilings.
- `workers/python/.venv/bin/python
  tests/fixtures/inhomogeneous_spatial/generate_python_oracle.py | diff -u
  tests/fixtures/inhomogeneous_spatial/python_rectangle_oracle.json -` passes byte-for-byte. The
  independent loop agrees on all four boundary masses/intensities and exact pair/center sums, K,
  and L.
- `cargo +1.96.0 test --locked --package marklab --test inhomogeneous_spatial_calibration` passes
  2/2. Twenty prespecified gradient inhomogeneous-Poisson controls reject at most the declared gross
  anti-conservatism ceiling; the separate tight-cluster control has excess short-range K after
  reweighting.
- `cargo +1.96.0 test --locked --package marklab --test
  inhomogeneous_spatial_project_workflow` first failed on the absent durable node, then exposed
  ordinary JSON-float digest drift during strict replay. The private codec now preserves exact f64
  bits and the final 1/1 test proves miss, reconstructed hit with one execution, and seed-invalidated
  second miss.
- Targeted warning-denied Clippy over the three integrations passes after fixing one needless range
  loop. `cargo +1.96.0 test --locked --package marklab --lib common::seeds::tests` passes 2/2 for
  stable namespace derivation and uniqueness. `cargo +1.96.0 check --locked --package marklab
  --no-default-features`, `cargo +1.96.0 test
  --locked --package marklab --doc`, `cargo +1.96.0 fmt --all --check`, and `git diff --check` pass.
  The responsibility split leaves orchestration, intensity/grid/null sampling, pair accumulation,
  identity/resource accounting, and durable codec in separate modules. Workspace/Nextest loops,
  feature matrices, benchmarks, fuzzing, memory tools, packaging, dependency audits, push,
  publication, deployment, and history rewriting were not run.

## Durable inhomogeneous pair-correlation checkpoint 78 — 2026-08-27

- `cargo +1.96.0 test --locked --package marklab --test
  inhomogeneous_pair_correlation_project_workflow` first failed on the unresolved durable node and
  now passes 1/1: fresh miss, reconstructed byte-identical hit with one execution, and seed-only
  invalidation to a second miss.
- The focused affected command passes inhomogeneous g typed 2/2 and durable 1/1, K/L typed 3/3 and
  durable 1/1, and calibration 3/3. It covers the shared persisted pilot, Python value, deterministic
  replay, zero-weight kernel endpoints, one-short memory/pair limits, twenty gradient-null controls,
  and a matched short-range cluster direction.
- Both `workers/python/.venv/bin/python
  tests/fixtures/inhomogeneous_pair_correlation/generate_python_oracle.py | diff -u
  tests/fixtures/inhomogeneous_pair_correlation/python_rectangle_oracle.json -` and the unchanged
  K/L Python-oracle regeneration pass byte-for-byte.
- Targeted warning-denied Clippy over the five affected integrations passes. `cargo +1.96.0 check
  --locked --package marklab --no-default-features`, `cargo +1.96.0 test --locked --package marklab
  --doc`, affected-file Rustfmt, and diff whitespace checks pass. Workspace/Nextest loops, broad
  feature matrices, benchmarks, fuzzing, memory tools, packaging, dependency audits, push,
  publication, deployment, and history rewriting were not run.

## Exact binary compartment-interface checkpoint 79 — 2026-08-27

- `cargo +1.96.0 test --locked --package marklab --test compartment_partition_domain` first failed
  on unresolved partition symbols and now passes 3/3: exact oriented distance, tissue-edge
  distinction, frame binding, gap/overlap/unaligned-segment rejection, absent shared interface,
  non-finite/outside queries, and one-short segment work.
- `cargo +1.96.0 test --locked --package marklab --test
  compartment_interface_profile_typed_workflow` first failed on unresolved typed-profile symbols;
  its durable test separately failed on the absent node. The final 3/3 pass covers typed cell rows,
  deterministic summaries, spatial-label mismatch, one-short point/query/memory limits, fresh miss,
  reconstructed identical hit with one ledger execution, and limit-only cache invalidation.
- `python3 tests/fixtures/compartment_partition/generate_geos_oracle.py | diff -u
  tests/fixtures/compartment_partition/geos_rectangle_oracle.json -` passes byte-for-byte against
  installed GEOS/geosop 3.14.1. The independent engine confirms both 50-square-micrometre
  compartments, 100-square-micrometre union/domain, exact shared 10-micrometre line, and query
  distances 3, 2, and 5 micrometres.
- Targeted warning-denied Clippy over both integrations passes after one iterator cleanup. `cargo
  +1.96.0 check --locked --package marklab --no-default-features`, `cargo +1.96.0 test --locked
  --package marklab --doc`, affected-file Rustfmt, and diff whitespace checks pass. Workspace/
  Nextest loops, broad feature matrices, benchmarks, fuzzing, memory tools, packaging, dependency
  audits, push, publication, deployment, and history rewriting were not run.

## Exact compartment-contact checkpoint 80 — 2026-08-27

- `cargo +1.96.0 test --locked --package marklab --test
  compartment_contact_fraction_workflow` first failed on unresolved production and durable symbols
  and now passes 2/2: exact shared/outer/complete denominator values, GEOS-backed fraction `1/3`,
  fresh miss, reconstructed identical hit with one execution, and role-swap cache invalidation.
- The affected three-integration command passes contact 2/2, typed interface profile 3/3, and exact
  partition 3/3 after deterministic sorted compensated segment-length accumulation. `python3
  tests/fixtures/compartment_partition/generate_geos_oracle.py | diff -u
  tests/fixtures/compartment_partition/geos_rectangle_oracle.json -` passes byte-for-byte against
  GEOS/geosop 3.14.1 including the new 30/20 micrometre denominator components.
- Targeted warning-denied Clippy over all three integrations passes. `cargo +1.96.0 check --locked
  --package marklab --no-default-features`, `cargo +1.96.0 test --locked --package marklab --doc`,
  affected-file Rustfmt, and diff whitespace checks pass. Workspace/Nextest loops, broad feature
  matrices, benchmarks, fuzzing, memory tools, packaging, dependency audits, push, publication,
  deployment, and history rewriting were not run.

## Exact compartment fragmentation and cell-mixing checkpoint 81 — 2026-08-27

- `cargo +1.96.0 test --locked --package marklab --test
  compartment_fragmentation_workflow` first failed on missing fixture/production symbols and now
  passes 2/2: exact two-component concentration/entropy/shape burden, explicit pre-graph mixing
  unavailability, fresh miss, reconstructed hit, and orientation invalidation.
- `python3 tests/fixtures/compartment_fragmentation/generate_geos_oracle.py | diff -u
  tests/fixtures/compartment_fragmentation/geos_fragmentation_oracle.json -` passes byte-for-byte
  against GEOS/geosop 3.14.1 plus standard-library entropy. It confirms areas 1/4/5/95/100,
  perimeters 12/52, normalized entropy `0.7219280948873623`, and exact domain union.
- `cargo +1.96.0 test --locked --package marklab --test compartment_cell_mixing_workflow` first
  failed on unresolved typed/durable symbols and now passes 3/3: exact graph/edge/incidence/entropy
  values, empty graph, one-short pair/memory limits, fresh miss, reconstructed hit, and radius
  invalidation. Its independent standard-library Python direct-loop fixture regenerates byte-for-
  byte with three edges, one cross edge, observed/expected `1/3`/`2/3`, and normalized entropy
  `0.9182958340544894`.
- The complete affected geometry command passes cell mixing 3/3, fragmentation 2/2, contact 2/2,
  typed interface 3/3, and partition 3/3; all three independent fixture regenerations pass.
  Targeted warning-denied Clippy over those five integrations passes. The major-checkpoint broad
  compile/lint/doc evidence is recorded below; the documented macOS full-integration/Nextest loop
  remains intentionally unrerun.

## Compartment-geometry stabilization checkpoint 82 — 2026-08-27

- `cargo +1.96.0 fmt --all --check` passes. `cargo +1.96.0 clippy --locked --workspace
  --all-targets --all-features -- -D warnings` passes in `21m22s` with no diagnostics.
- `cargo +1.96.0 check --locked --workspace --no-default-features` passes in `10.20s`.
  `cargo +1.96.0 test --locked --workspace --doc --all-features` passes for every workspace package
  with zero doctest failures. `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace
  --all-features --no-deps` passes in `23.91s`.
- The focused five-integration command passes 13/13, and the GEOS partition/fragmentation plus
  Python mixing fixtures regenerate byte-for-byte. `git diff --check` passes after the final ledger
  update.
- The full workspace integration/Nextest loop was not run because checkpoints 51/52 and the active
  user instruction document and forbid retrying the macOS binary-verification stall. No phase-only
  feature matrix, benchmark, fuzz, memory tool, packaging, dependency audit, push, publication,
  deployment, or history rewrite was run.

## Blocked fingerprint population-inference checkpoint 91 — 2026-08-28

- `cargo +1.96.0 test --locked --features cli --test cohort_mmd_cli
  linear_mmd_cli_accepts_complete_patient_blocks_and_reports_the_design -- --exact` first failed
  because the fingerprint CSV boundary accepted only the legacy four-column header. After the
  blocked path was implemented, the complete MMD CLI passes; the analogous exact energy CLI passes.
- `cargo +1.96.0 test --locked --package marklab-cohort --test mmd_reference --test
  energy_reference` passes 5/5. Independent slow implementations match blocked MMD and energy
  observed statistics and exact p-values while reverse-order assignments prove patient-ID binding;
  incomplete, duplicate, foreign, and group-confounded designs fail. `cargo +1.96.0 test --locked
  --package marklab-cohort --lib` passes 21/21.
- `cargo +1.96.0 test --locked --features cli --test cohort_mmd_cli` passes 3/3 and `cargo +1.96.0
  test --locked --features cli --test cohort_energy_cli` passes 2/2. They cover complete blocked
  inputs, within-patient block conflict rejection, and unchanged unblocked design fields with no new
  optional keys.
- Warning-denied Clippy passes for all `marklab-cohort` targets and for the affected `marklab` binary
  plus both CLI integrations. Package no-default compilation, package doctests, affected-file
  Rustfmt, and `git diff --check` pass. Workspace-wide gates were not repeated after checkpoint 90;
  no full integration/Nextest loop, feature matrix, benchmark, fuzz, memory tool, packaging,
  dependency audit, push, publication, deployment, or history rewrite ran.

## Blocked functional-curve inference checkpoint 92 — 2026-08-28

- `cargo +1.96.0 test --locked --features cli --test cohort_functional_permutation_cli
  functional_l2_cli_accepts_complete_patient_blocks_and_reports_the_design -- --exact` first failed
  on the legacy four-column-only CSV boundary and then passed after production admission.
- `cargo +1.96.0 test --locked --package marklab-cohort --test
  functional_permutation_reference` passes 2/2. The existing unblocked and new blocked L2 p-values
  match independent slow implementations exactly; reverse-order assignments prove patient-ID rather
  than row-position binding. The focused functional library unit tests pass 2/2.
- `cargo +1.96.0 test --locked --features cli --test cohort_functional_permutation_cli` passes 3/3
  for legacy unblocked output, complete blocked output, and conflicting within-curve block rejection.
  Warning-denied Clippy passes for the affected cohort library/reference and CLI binary/integration.
  Package no-default compilation, package doctests, affected-file Rustfmt, and diff whitespace pass.
- Workspace-wide gates were not repeated after checkpoint 90. The prohibited full integration/
  Nextest loader loop, feature matrix, benchmark, fuzz, memory tool, packaging, dependency audit,
  push, publication, deployment, and history rewrite did not run.

## Typed repeated-subject residual design checkpoint 93 — 2026-08-28

- `cargo +1.96.0 test --locked --features cli --test cohort_repeated_freedman_lane_cli
  repeated_freedman_lane_recovers_the_common_within_subject_slope -- --exact` first failed because
  the CLI design omitted the typed null family and permutation unit. The final full CLI integration
  passes 1/1 with byte-identical repeated executions, four subjects/12 visits, common within-subject
  slope `2.1`, and the explicit complete-subject residual-vector unit.
- `cargo +1.96.0 test --locked --package marklab-cohort --lib
  inference_design::tests::subject_residual_signs_match_the_former_whole_subject_stream -- --exact`
  passes 1/1. Its independent direct SplitMix/Rademacher loop matches every old sign at every
  replicate; wrong-operation and replicate-range failures are explicit. The exact duplicate
  subject/visit regression also passes 1/1.
- Warning-denied Clippy passes for the affected cohort library/reference and repeated CLI binary/
  integration. Package no-default compilation, package doctests, affected-file Rustfmt, and diff
  whitespace pass. Workspace-wide gates are deferred until the current three-to-five-workflow
  FND-06 family closes; the prohibited full integration/Nextest loop and specialized gates did not
  run.

## Typed paired-patient sign-flip checkpoint 94 — 2026-08-28

- `cargo +1.96.0 test --locked --features cli --test cohort_paired_permutation_cli
  paired_cli_matches_the_hand_oracle_and_is_byte_deterministic -- --exact` first failed because the
  CLI design omitted the typed paired null and complete-difference unit; the final integration passes
  1/1 with exact hand values and byte-identical repeated outputs.
- `cargo +1.96.0 test --locked --package marklab-cohort --test paired_permutation_reference` passes
  1/1. Its independent slow SplitMix/Rademacher implementation matches the exact p-value, while the
  result exposes `PairedSignFlip`, `CompletePatientPairDifference`, and the prespecified alternative.
- Warning-denied Clippy passes for the affected cohort library/reference and paired CLI binary/test.
  Package no-default compilation, package doctests, affected-file Rustfmt, and diff whitespace pass.
  Broad gates are recorded once in the following stabilization checkpoint.

## Patient inference-design stabilization checkpoint 95 — 2026-08-28

- `cargo +1.96.0 fmt --all --check` passes. `cargo +1.96.0 clippy --locked --workspace
  --all-targets --all-features -- -D warnings` passes in `21m35s` with no diagnostics.
- `cargo +1.96.0 check --locked --workspace --no-default-features` passes in `17.47s`. `cargo
  +1.96.0 test --locked --workspace --doc --all-features` completes for every package with zero
  doctest failures. `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace
  --all-features --no-deps` passes in `21.35s`.
- `git diff --check` passes. The full workspace integration/Nextest loop was not run because the
  active instruction and checkpoints 51/52 prohibit retrying the macOS binary-verification stall.
  No feature matrix, benchmark, fuzz, memory tool, packaging, dependency audit, push, publication,
  deployment, or history rewrite ran.

## Multisite patient-count overflow checkpoint 96 — 2026-08-28

- The focused regression first panicked on `usize` accumulation and now passes 1/1 with an explicit
  input error. Both multisite unit tests and the fixed-effect CLI oracle pass. Focused warning-denied
  Clippy and package no-default compilation pass; broad checkpoint-95 gates were not repeated.

## Patient-level multisite contrast checkpoint 97 — 2026-08-28

- The exact CLI behavior test first failed because `multisite-patient-contrast` was absent and now
  passes with three site effects/SEs and the nested fixed-effect result. The full multisite CLI suite
  passes 2/2, preserving the legacy precomputed-site summary path.
- All three multisite unit tests pass, covering the patient-row hand oracle, duplicate patients,
  site/group confounding, REML variation, and aggregate-count overflow. Warning-denied Clippy passes
  for the affected cohort library and CLI test; package no-default compilation, doctests,
  affected-file Rustfmt, and diff whitespace pass. Broad checkpoint-95 gates were not repeated.

## Canonical hierarchical-bootstrap ordering checkpoint 98 — 2026-08-28

- The focused row-order regression first failed with different replicate vectors and percentile
  intervals, then passed after exact specimen-ID canonicalization. The independent slow bootstrap
  reference and patient-first CLI integration each pass 1/1.
- Focused warning-denied cohort Clippy, package no-default compilation, affected-file Rustfmt, and
  diff whitespace pass. Broad checkpoint-95 gates were not repeated.

## Typed hierarchical-bootstrap design checkpoint 99 — 2026-08-28

- The hierarchical-bootstrap CLI test first failed because its design omitted the typed null/unit
  and now passes 1/1. Bootstrap-equivalence passes 1/1 with the same shared design output.
- The independent slow reference passes unchanged. The focused design test passes exact patient/
  specimen block counts, bounded draw indices, singleton support, empty-block rejection, and flat-
  permutation rejection.
- Warning-denied Clippy passes for the affected cohort library/reference and both CLI integrations.
  Package no-default compilation, doctests, affected-file Rustfmt, and diff whitespace pass; broad
  checkpoint-95 gates were not repeated.

## Whole-cluster patient inference checkpoint 100 — 2026-08-28

- The exact CLI behavior test first failed because `cluster-permutation` was absent and now passes
  1/1 with 12 patients, six clusters, equal cluster weighting, and hand effect `7/3`.
- The independent slow reference passes 1/1 with exact p-value and typed cluster design; it also
  covers duplicate patient IDs, conflicting groups within one cluster, and insufficient group
  clusters. Warning-denied Clippy passes for the affected cohort library/reference and CLI test;
  package no-default compilation, doctests, affected-file Rustfmt, and diff whitespace pass.

## Cohort hierarchy and cluster stabilization checkpoint 101 — 2026-08-28

- `cargo +1.96.0 fmt --all --check` passes. `cargo +1.96.0 clippy --locked --workspace
  --all-targets --all-features -- -D warnings` passes in `22m39s` with no diagnostics.
- `cargo +1.96.0 check --locked --workspace --no-default-features` passes in `18.28s`. All-feature
  workspace doctests complete with zero failures. `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc
  --locked --workspace --all-features --no-deps` passes in `21.46s`.
- `git diff --check` passes. The full workspace integration/Nextest loop was not run because the
  active instruction and checkpoints 51/52 prohibit retrying the macOS binary-verification stall.
  No feature matrix, benchmark, fuzz, memory tool, packaging, dependency audit, push, publication,
  deployment, or history rewrite ran.

## Explicit randomized-interference design checkpoint 102 — 2026-08-28

- The exact causal CLI test first failed because analysis/null/randomization-unit fields were absent
  and now passes 1/1 with byte-identical replay, six assignment states, exact exposure probabilities,
  199 null values, four units, and one cluster.
- All three causal library tests pass. Warning-denied Clippy passes for the causal library and
  affected CLI test; package no-default compilation, doctests, affected-file Rustfmt, and diff
  whitespace pass. Broad checkpoint-101 gates were not repeated.

## Blocked single-step Max-T checkpoint 103 — 2026-08-28

- The exact blocked CLI test first failed on the legacy four-column-only CSV boundary and now passes;
  the complete Max-T CLI suite passes 2/2 with legacy JSON compatibility and exact block evidence.
- Both Max-T independent slow references pass with exact adjusted p-values/critical values and the
  blocked design reports two-sided population independence. Seven affected MMD/energy/functional
  reference tests pass after the shared compiler gained an explicit alternative.
- Warning-denied Clippy passes for all cohort targets and the affected CLI test. Package no-default
  compilation, doctests, affected-file Rustfmt, and diff whitespace pass; broad checkpoint-101 gates
  were not repeated.

## Patient-family step-down Max-T checkpoint 104 — 2026-08-28

- `cargo +1.96.0 test --locked --package marklab --features cli --test cohort_max_t_cli
  max_t_cli_exposes_step_down_adjustment_for_the_complete_endpoint_family -- --exact --nocapture`
  first failed because `--step-down` was absent and now passes. The complete named CLI target passes
  3/3 with legacy single-step, blocked single-step, and step-down behavior.
- `cargo +1.96.0 test --locked --package marklab-cohort --lib max_t -- --nocapture` passes 1/1.
  `cargo +1.96.0 test --locked --package marklab-cohort --test max_t_reference -- --nocapture`
  passes 3/3; the new independent slow oracle agrees for unrestricted and exact-block step-down
  streams. One broader package-filter command was interrupted after its Max-T unit passed because
  Cargo began launching unrelated integration binaries through the prohibited macOS loader loop.
- Warning-denied Clippy passes for all `marklab-cohort` targets and the affected `marklab` CLI
  binary. Package no-default compilation, strict package docs, affected-file Rustfmt, and `git diff
  --check` pass. Broad checkpoint-101 gates were not repeated.

## Paired endpoint-family Max-T checkpoint 105 — 2026-08-28

- `cargo +1.96.0 test --locked --package marklab --features cli --test
  cohort_paired_max_t_cli paired_max_t_cli_moves_each_complete_patient_difference_vector_as_one_unit
  -- --exact --nocapture` first failed because `paired-max-t` was absent and now passes. The final
  named CLI target passes 1/1 with exact condition, permutation-unit, multiplicity, correction,
  endpoint-effect, and replicate fields.
- `cargo +1.96.0 test --locked --package marklab-cohort --test paired_max_t_reference --
  --nocapture` passes 2/2 for the independent slow single-step/step-down oracle and incomplete-family
  rejection. The legacy `max_t_reference` passes 3/3, `paired_permutation_reference` passes 1/1,
  `cohort_paired_permutation_cli` passes 1/1, and the focused inference-design library tests pass
  3/3.
- Warning-denied Clippy passes for all `marklab-cohort` targets and the affected `marklab` CLI
  binary. Package no-default compilation, strict package docs, affected-file Rustfmt, and `git diff
  --check` pass. Broad checkpoint-101 gates were not repeated.

## One-covariate patient Freedman-Lane checkpoint 106 — 2026-08-28

- `cargo +1.96.0 test --locked --package marklab --features cli --test
  cohort_covariate_permutation_cli covariate_permutation_cli_adjusts_one_prespecified_patient_covariate
  -- --exact --nocapture` first failed because `covariate-permutation` was absent and now passes.
  The final named CLI target passes 1/1 with exact design/model/transform/group/effect fields.
- `cargo +1.96.0 test --locked --package marklab-cohort --test covariate_reference --
  --nocapture` passes 2/2. The independent FWL oracle agrees on coefficient, standard error,
  statistic, and p-value; row reversal and `1e100` scaling are invariant; exact group-covariate
  collinearity fails. The affected repeated Freedman-Lane CLI remains green 1/1.
- Warning-denied Clippy passes for all `marklab-cohort` targets and the affected `marklab` CLI
  binary. Package no-default compilation, strict package docs, affected-file Rustfmt, and `git diff
  --check` pass. Broad checkpoint-101 gates were not repeated.

## Blocked one-covariate residual permutation checkpoint 107 — 2026-08-28

- The blocked CLI test first failed because the five-column input was rejected and now passes. The
  complete `cohort_covariate_permutation_cli` target passes 2/2 with blocked fields and legacy
  unblocked-field absence.
- `covariate_reference` passes 3/3, including the independent restricted two-block p-value and
  reverse patient-ID assignment order. Shared alignment regressions pass: MMD 3/3, energy 2/2,
  functional 2/2, and Max-T 3/3. The first chained command used nonexistent test target
  `functional_reference` and exited after the preceding MMD/energy targets passed; the corrected
  `functional_permutation_reference` command passes 2/2.
- Warning-denied Clippy passes for all `marklab-cohort` targets and the affected `marklab` CLI
  binary. Package no-default compilation, strict package docs, affected-file Rustfmt, and `git diff
  --check` pass. Broad checkpoint-101 gates were not repeated.

## Inference-family stabilization checkpoint 108 — 2026-08-28

- `cargo +1.96.0 fmt --all --check` passes. `cargo +1.96.0 clippy --locked --workspace
  --all-targets --all-features -- -D warnings` passes in `27m17s` with no diagnostics.
- `cargo +1.96.0 check --locked --workspace --no-default-features` passes in `19.60s`.
  `cargo +1.96.0 test --locked --workspace --doc --all-features` completes every package doctest
  with zero failures. `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace
  --all-features --no-deps` passes in `25.46s`.
- `git diff --check` passes. The full workspace integration/Nextest loop was not run because the
  active instruction and checkpoints 51/52 prohibit retrying the macOS binary-verification stall.
  No feature matrix, benchmark, fuzz, memory tool, packaging, dependency audit, push, publication,
  deployment, or history rewrite ran.

## Named nuisance-matrix Freedman-Lane checkpoint 109 — 2026-08-28

- The exact matrix CLI test first failed because `covariate-matrix-permutation` was absent and now
  passes 1/1 with ordered names/transforms, dimensions, adjusted hand effect, and exact counts.
- `covariate_matrix_reference` passes 2/2. Its independent modified-Gram-Schmidt/FWL oracle agrees
  on coefficient, standard error, statistic, and blocked/unblocked p-values; reverse patient order
  and `1e100`/`1e-100` column rescaling are invariant; missing and collinear columns fail. Legacy
  `covariate_reference` passes 3/3 and `cohort_covariate_permutation_cli` passes 2/2.
- Warning-denied Clippy passes for all `marklab-cohort` targets and the affected `marklab` CLI
  binary. Package no-default compilation, strict package docs, affected-file Rustfmt, and `git diff
  --check` pass. Broad checkpoint-108 gates were not repeated.

## Adjusted multisite patient contrast checkpoint 110 — 2026-08-28

- The exact adjusted-multisite CLI test first failed because `multisite-covariate-contrast` was
  absent and now passes 1/1 over three sites/24 patients/two nuisance columns.
- `multisite_covariate_reference` first found exact row-order drift in site SEs/pooling and now
  passes 2/2 after canonical patient ordering. Its independent QR/FWL oracle agrees on every site
  coefficient/SE and fixed pool; incomplete and site-collinear designs fail. The legacy
  `cohort_multisite_inference_cli` passes 2/2.
- The first warning-denied cohort Clippy run failed on `needless_range_loop`; the iterator fix made
  final-state warning-denied Clippy pass for all cohort targets and the affected root CLI. Package
  no-default compilation, strict package docs, affected-file Rustfmt, and `git diff --check` pass.
  Broad checkpoint-108 gates were not repeated.

## Covariate-adjusted whole-cluster inference checkpoint 111 — 2026-08-28

- `cargo +1.96.0 test --locked --features cli --test cohort_cluster_covariate_cli
  cluster_covariate_cli_adjusts_equal_weight_cluster_summaries` first failed with the expected
  unrecognized `cluster-covariate-permutation` subcommand, then passed 1/1 after production wiring.
- `cargo +1.96.0 test --locked --package marklab-cohort --test cluster_covariate_reference`
  passes 2/2. Its independent modified-Gram-Schmidt QR/FWL implementation matches the adjusted
  coefficient, standard error, statistic, and every deterministic complete-cluster residual shuffle;
  it also checks exact row-order invariance, extreme column rescaling, duplicate patients,
  mixed-group clusters, incomplete/collinear matrices, and insufficient independent clusters.
- The affected reference command over `cluster_covariate_reference`, `cluster_permutation_reference`,
  and `covariate_matrix_reference` passes 5/5. The affected CLI command over
  `cohort_cluster_covariate_cli`, `cohort_cluster_permutation_cli`, and
  `cohort_covariate_matrix_permutation_cli` passes 3/3.
- `cargo +1.96.0 clippy --locked --package marklab-cohort --all-targets --all-features -- -D
  warnings` passes in 1m23s. `cargo +1.96.0 clippy --locked --package marklab --all-targets
  --features cli -- -D warnings` passes in 24m02s on the documented slow macOS verification path.
  `cargo +1.96.0 check --locked --package marklab-cohort --no-default-features` passes in 8.39s;
  strict warning-denied package docs pass in 10.55s; affected-file Rustfmt and `git diff --check`
  pass.
- No admitted CRC artifact declares randomized treatment assignment at a cluster unit, so no real
  cluster treatment effect is fabricated. Workspace integration/Nextest, workspace-wide gates,
  feature matrices, benchmarks, fuzzing, memory tools, packaging, dependency audits, push,
  publication, deployment, and history rewriting were not run.

## Ordered endpoint-family gatekeeping checkpoint 112 — 2026-08-28

- `cargo +1.96.0 test --locked --features cli --test cohort_hierarchical_max_t_cli
  hierarchical_max_t_cli_opens_only_ordered_complete_families` first failed with the expected
  unrecognized `hierarchical-max-t` subcommand, then passed 1/1 with two opened families and the
  exact typed design.
- `cargo +1.96.0 test --locked --package marklab-cohort --test hierarchical_max_t_reference`
  passes 2/2. Its first compile attempt exposed a test-only helper shadowed by a local fixture;
  renaming that fixture made the unchanged production code pass. Independent whole-patient shuffle
  oracles match single-step and step-down local critical values, adjusted p-values, opened-family
  counts, and gated decisions. Primary-family failure closes descendants; overlapping/incomplete
  partitions and one-family input fail.
- The affected reference command over `hierarchical_max_t_reference`, `max_t_reference`, and
  `paired_max_t_reference` passes 7/7. The affected CLI command over
  `cohort_hierarchical_max_t_cli`, `cohort_max_t_cli`, and `cohort_paired_max_t_cli` passes 5/5.
- Warning-denied all-target/all-feature `marklab-cohort` Clippy passes in 59.82s; warning-denied
  `marklab` CLI-binary Clippy passes in 2m58s. Package no-default compilation passes in 32.32s,
  strict warning-denied package docs pass in 11.03s, and affected-file Rustfmt plus `git diff
  --check` pass.
- No admitted CRC artifact contains a prospectively fixed ordered endpoint-family hierarchy, so no
  post-outcome hierarchy or real rejection claim is fabricated. Workspace integration/Nextest,
  workspace-wide gates, feature matrices, benchmarks, fuzzing, memory tools, packaging, dependency
  audits, push, publication, deployment, and history rewriting were not run.

## Adjusted and hierarchical inference stabilization checkpoint 113 — 2026-08-28

- `cargo +1.96.0 fmt --all --check` passes.
- `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings` passes in
  15m11s with no diagnostics.
- `cargo +1.96.0 check --locked --workspace --no-default-features` passes in 22.29s.
  `cargo +1.96.0 test --locked --workspace --doc --all-features` compiles in 36.20s and completes
  every package doctest with zero failures.
- `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace --all-features --no-deps`
  passes in 25.97s. `git diff --check` passes.
- The full workspace integration/Nextest loop was not run because the active instruction and
  checkpoints 51/52 prohibit retrying the macOS binary-verification loop. No feature matrix,
  benchmark, fuzz, memory tool, packaging, dependency audit, push, publication, deployment, or
  history rewrite ran.

## Multiclass CellViT categorical mixing checkpoint 114 — 2026-08-28

- `cargo +1.96.0 test --locked --package marklab --test
  categorical_neighborhood_mixing_workflow
  three_class_cellvit_codes_produce_the_exact_mixing_matrix` first failed on the absent production
  function/node/config/result/error exports. The first direct implementation reached the oracle but
  exposed a test-only exact-versus-algebraic one-ULP assertion; the corrected tolerance retained
  production. The first durable run then failed decode because ordinary JSON shifted entropy by one
  ULP; routing this node through the existing exact-f64 codec made all 4/4 final workflow cases pass.
- `python3 tests/fixtures/categorical_neighborhood_mixing/generate_python_oracle.py | diff -u
  tests/fixtures/categorical_neighborhood_mixing/python_oracle.json -` passes byte-for-byte. The
  independent standard-library pair loop agrees on the complete 3x3 directed count matrix,
  observed/random-label/excess fractions, cross-edge result, and per-class entropy.
- `categorical_codebook_interchange` first failed on the absent
  `Pattern::categorical_stratum_levels` field and now passes 1/1 across equivalent CSV and Parquet
  string labels/codes. The new workflow additionally rejects a Pattern-codebook/MarkTable-
  declaration mismatch. Seven Parquet unit boundaries, 11 scalar MarkTable cases, four legacy
  categorical workflows, and three soft-simplex neighborhood cases pass.
- A read-only mini audit found SHA-256
  `86afdc0c343dc804258c57c1ecd99c7f9b544aa8363756c571570e6952e8858c` for the 2,000-row admitted
  coordinate CSV and `9ba8102b98f5e4f42dc4c19d9fd341b0acb6d6ecf671997ce709187312c61fb4`
  for its exact window. The explicitly ignored real-data test was run once with those local
  temporary copies and passed 1/1: Connective 118, Dead 67, Inflammatory 365, Neoplastic 1,450.
  The temporary copies were moved to Trash after the test. The CSV has no `cell_id`; its scalar
  `mark_probability` is winning-class confidence rather than a full simplex, so durable hard-class
  mixing and uncertainty-bearing soft mixing remain respectively blocked without fabrication.
- The first targeted warning-denied Clippy run found four mechanical `manual_is_multiple_of` lints
  and one local type-complexity lint. The corrected final-state targeted all-feature library/new-test
  Clippy passes in 22.30s; package no-default compilation passes in 15.01s; strict all-feature
  package docs pass in 12.67s; affected-file Rustfmt and `git diff --check` pass.
- Workspace-wide gates were not repeated immediately after checkpoint 113. No full integration/
  Nextest loop, feature matrix, benchmark, fuzz, memory tool, packaging, dependency audit, push,
  publication, deployment, or history rewrite ran.

## Stable CellViT source identity and real durable mixing checkpoint 115 — 2026-08-28

- Expected red: the CSV/Parquet interchange test failed because `Pattern` had no `cell_ids` field.
  After the typed import path passed, the first real durable child failed with
  `ThresholdBindingMismatch { row: 81 }`; this exposed pre-serialization thresholding in the Python
  adapter. The focused adapter oracle then passed after thresholding the exact exported-as-`f32`
  value.
- On `mini`, `/Volumes/500GB/marklab/env/cellvit-mps-py39/bin/python3` plus the frozen CellViT source
  reran `marklab_cellvit_cptac_results_adapter.py` over the recorded inference, projected, spatial,
  case-map, molecular-label, clinical, verification, and transform inputs. The accepted v3 run
  revalidated 366 slides, 178 patients, and 1,542,389 cells. Its adapter digest is
  `1e2eb28d1e0481534c6102cf5338024a287b8859bc549aece1a345891a72b668`; the 2,000-row coordinate and
  window digests are `95b933c04a60abfef5cbf02784e7fd582dfdc6e3b129c0f6c84801e91c786709`
  and `9ba8102b98f5e4f42dc4c19d9fd341b0acb6d6ecf671997ce709187312c61fb4`.
  A direct audit found 2,000 unique strictly increasing IDs, all 512 vector IDs in the coordinate
  identity set, zero threshold mismatches, and only row 81 differing from the rejected v1 export.
- `MARKLAB_REAL_CELLVIT_CATEGORICAL_CSV=... MARKLAB_REAL_CELLVIT_CATEGORICAL_WINDOW=... cargo
  +1.96.0 test --locked --package marklab --all-features --test
  categorical_neighborhood_mixing_real_cellvit
  admitted_cellvit_categorical_mixing_replays_across_fresh_processes -- --ignored --exact
  --nocapture` passes. Its two child processes report miss then hit, byte-identical result JSON, and
  execution count one. The corresponding ignored real interchange test passes.
- The three affected integration targets pass 2/0/1, 1/0/1, and 4/0/0 (passed/failed/ignored).
  `scalar_mark_input` passes 11/11 and `cargo +1.96.0 test --locked --package marklab --all-features
  --lib io::` passes 8/8. The two-test Python adapter oracle, Python syntax compilation,
  affected-file Rustfmt, both LSP outlines, and `git diff --check` pass.
- No workspace-wide Clippy/test, Nextest, no-default matrix, strict docs, benchmark, fuzz, memory
  tool, packaging, dependency audit, push, publication, deployment, or history rewrite ran at this
  ordinary milestone.

## Real CellViT pair curves and multiclass stabilization checkpoint 116 — 2026-08-28

- `MARKLAB_REAL_CELLVIT_CATEGORICAL_CSV=... MARKLAB_REAL_CELLVIT_CATEGORICAL_WINDOW=... cargo
  +1.96.0 test --locked --package marklab --all-features --test categorical_pair_real_cellvit
  admitted_cellvit_pair_curves_replay_across_fresh_processes -- --ignored --exact --nocapture`
  passes. The first child completes both real pair workflows in 4.11s; the second completes in
  0.31s. Both report miss→hit, exact result-digest replay, and one unchanged execution per durable
  project for 1,450 Neoplastic source cells, 365 Inflammatory targets, four radii, and 19 null draws.
- The shared real fixture refactor was rechecked by the existing categorical-mixing fresh-process
  test, which passes again in 0.82s. The affected pair command passes cross-g 4/4, project replay
  1/1, typed pair 3/3, and the nonignored real child 1/1; its ignored parent was run separately as
  real evidence.
- `cargo +1.96.0 fmt --all --check` and `git diff --check` pass. `cargo +1.96.0 clippy --locked
  --workspace --all-targets --all-features -- -D warnings` passes in 25m26s. `cargo +1.96.0 check
  --locked --workspace --no-default-features` passes in 10.15s. `cargo +1.96.0 test --locked
  --workspace --doc --all-features` passes every package with zero failures. `RUSTDOCFLAGS='-D
  warnings' cargo +1.96.0 doc --locked --workspace --all-features --no-deps` passes in 13.12s.
- The full workspace integration/Nextest loop was not run because checkpoints 51/52 and the active
  instruction prohibit retrying the macOS loader-verification loop. No feature matrix, benchmark,
  fuzz, memory tool, packaging, dependency audit, push, publication, deployment, or history rewrite
  ran.

## Bounded parallel marked project DAG checkpoint 117 — 2026-08-28

- Expected red: the new integration failed on unresolved `execute_marked_prepost_dag`,
  `plan_marked_prepost_dag`, `MarkedPrePostDagLimits`, and `MarkedPrePostDagTarget` imports. The
  exact behavior test then passed after the fixed typed executor was connected.
- `cargo +1.96.0 test --locked --package marklab --all-features --test
  marked_prepost_dag_workflow` passes 4/4. Its fresh root-only, resume, and replay child executions
  take 0.22s, 0.10s, and 0.01s on the final run and prove cache states
  `[miss,miss,null]`, `[hit,hit,miss]`, and `[hit,hit,hit]` with durable counts `[1,1,0]`,
  `[1,1,1]`, and `[1,1,1]`. The comparison matches the direct compatibility owner exactly.
- `cargo +1.96.0 test --locked --package marklab --all-features --test project_workflow` passes
  12/12, including dependency identity, durable dependency restore, failure atomicity, semantic
  store verification, and direct marked/pre/post parity.
- `cargo +1.96.0 clippy --locked --package marklab --lib --test marked_prepost_dag_workflow
  --all-features -- -D warnings` passes in 1m09s. `cargo +1.96.0 check --locked --package marklab
  --no-default-features` passes in 12.22s. Strict all-feature root-package docs pass in 12.71s;
  affected Rustfmt, LSP outline, and `git diff --check` pass.
- Real CRC pre/post evidence was not run because the admitted cohorts have no paired repeated-
  timepoint CellViT Pattern inputs. No workspace-wide gate was repeated after checkpoint 116, and no
  Nextest/full integration, feature matrix, benchmark, fuzz, memory tool, packaging, dependency
  audit, push, publication, deployment, or history rewrite ran.

## Durable patient multiclass CellViT Bayesian checkpoint 118 — 2026-08-28

- Expected red: `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_dirichlet_multinomial_group_cli
  dirichlet_multinomial_recovers_patient_unit_multiclass_group_shift -- --exact --nocapture`
  first failed on the absent `dirichlet-multinomial-group` command. Its first wired run exposed a
  CLI parser stack overflow; moving this command to the established boxed large-command parser
  path removed that failure. The final command passes 1/1 and independently checks exact observed
  patient-proportion arithmetic plus the known opposite multiclass shifts.
- Expected durable red: `cargo +1.96.0 test --locked --package marklab --features cli --test
  durable_pymc_dirichlet_multinomial_group_project
  dirichlet_multinomial_group_runs_once_then_replays_without_starting_pymc -- --exact --nocapture`
  failed on the absent project subcommand and then passes 1/1. It proves miss, backend-disabled
  cross-process hit, byte-identical typed result, seed invalidation, exact node/schema identity,
  and one unchanged execution record.
- `python3 tests/python/test_marklab_cellvit_cptac_results_adapter.py` passes 3/3, including complete
  patient-by-class rows and retained zeros. `cargo +1.96.0 test --locked --package marklab-bayes
  dirichlet_multinomial_group::tests:: -- --nocapture` passes 1/1 for incomplete-vector rejection.
  Python syntax compilation and affected-file Rustfmt also pass.
- On `mini`, the pinned CellViT Python 3.9 environment plus frozen source snapshot reran
  `marklab_cellvit_cptac_results_adapter.py` over the exact recorded inference, projected, spatial,
  case-map, molecular-label, clinical, verification, and transform inputs. It revalidated 366
  slides, 178 patients, and 1,542,389 cells into
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v4-inputs`. The 525-row, 105-patient,
  five-class molecular input includes 31 zero counts and has SHA-256
  `00f91c5a8084d3a1e9140beb9769dca2b74ecd4e08d9eef17c258f7504c5167e`.
- Two direct `target/debug/marklab project dirichlet-multinomial-group` processes over that exact
  input report miss then hit; the hit ran with `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1`,
  `cmp` passed, and `executions.jsonl` remained one line. The complete real fit has 4,000 draws,
  R-hat 1.00199, bulk/tail ESS 1278.06/1708.53, E-BFMI 1.00882, zero divergences/tree-depth hits,
  and result SHA-256 `8a00e2119d2e454f21be53fd666bf4200b60a78f38e57130ccf59ad0f931362c`.
  The result is stored in the v4 bundle. No workspace-wide gate, full-integration/Nextest loop,
  feature matrix, benchmark, fuzz, memory tool, packaging, dependency audit, push, publication,
  deployment, or history rewrite ran.

## Multiclass Bayesian promotion and stabilization checkpoint 119 — 2026-08-28

- Three behavior tests first failed on absent `dirichlet-multinomial-group-agreement`,
  `dirichlet-multinomial-group-sensitivity`, and `dirichlet-multinomial-group-sbc` commands. Their
  final exact runs pass 1/1 each. Agreement checks all three synthetic classes and concentration
  between independently executed PyMC/NumPyro fits. Sensitivity completes the fixed seven-scenario
  grid. SBC completes 20/20 replicates and checks ten-bin ranks and 90% coverage for two baseline
  logits, two group effects, concentration, and three derived class differences.
- On the real 525-row CPTAC input, direct agreement passes every one of five reference
  probabilities, five comparison probabilities, five group differences, and concentration. PyMC
  reports R-hat 1.00402, bulk/tail ESS 1396.67/1688.75, minimum E-BFMI 0.99907, and zero
  divergences/depth hits; NumPyro reports 1.00216, 4245.79/2628.51, 1.02703, and zero/zero. The
  result SHA-256 is `288bbeaf817e440caa492032af853222d7242cf66fbb7b9e0ee181a731ebaab4`.
- The real seven-scenario sensitivity run is complete and reports only `logit_sd_lower` as material
  (0.98350 standardized shift at threshold 0.75); every other nonbaseline maximum is at most
  0.41416. Its SHA-256 is `b4a2d308fa98ba1dc0a3278b0dbd705434e512bb4079ec5751f0f4f3da67f1ce`.
  Both typed outputs are stored in the Mac-mini v4 results directory.
- `cargo +1.96.0 test --locked --package marklab-bayes` passes 47/47 plus doctests. The final-state
  direct and durable Dirichlet-multinomial tests each pass 1/1, including backend-disabled replay.
  `cargo +1.96.0 fmt --all --check` passes. `cargo +1.96.0 clippy --locked --workspace
  --all-targets --all-features -- -D warnings` passes in 26m22s. `cargo +1.96.0 check --locked
  --workspace --no-default-features` passes in 21.06s. `cargo +1.96.0 test --locked --workspace
  --doc --all-features` passes every package with zero failures. `RUSTDOCFLAGS='-D warnings' cargo
  +1.96.0 doc --locked --workspace --all-features --no-deps` passes in 25.95s.
- The full workspace integration/Nextest loop was not run because checkpoints 51/52 and the active
  instruction prohibit retrying the macOS loader-verification loop. No feature matrix, benchmark,
  fuzz, memory tool, packaging, dependency audit, push, publication, deployment, or history rewrite
  ran.

## Representative sparse CellViT graph heat checkpoint 120 — 2026-08-28

- Expected red: `cargo +1.96.0 test --locked --package marklab --features cli --test
  graph_sparse_radius_heat_cli
  sparse_radius_heat_matches_the_exact_small_graph_and_enforces_work_limits -- --exact --nocapture`
  failed on the absent `sparse-radius-heat` graph command. The final two-test target passes the path
  and negative-grid dense-eigensolver differentials plus candidate/edge/working-byte boundaries.
- Expected durable red: `cargo +1.96.0 test --locked --package marklab --features cli --test
  durable_sparse_radius_heat_project
  sparse_radius_heat_replays_across_processes_and_invalidates_on_input_change -- --exact
  --nocapture` failed on the absent project subcommand. Its first changed-input miss exposed decimal
  JSON normalization; connecting the node to the existing exact-f64 codec produced the final 1/1
  pass with miss/hit byte identity, one ledger row, and changed-input miss.
- `python3 tests/python/test_marklab_cellvit_cptac_results_adapter.py` passes 4/4, including exact
  CellId/coordinate retention and the hard Neoplastic signal request. `cargo +1.96.0 test --locked
  --package marklab-graph` passes 1/1 plus doctests after the documented serial binary-verification
  delay. Affected-file Rustfmt and diff whitespace checks pass.
- On `mini`, the pinned/frozen adapter revalidated 366 slides, 178 patients, and 1,542,389 cells
  into `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v5-inputs`. The 2,000-node graph input
  SHA-256 is `49d5509ff4996b8e23baa8d0c8118fa579f43cd07c95ed9b301124ebc7e35dcf`.
  `/usr/bin/time -l target/debug/marklab graph sparse-radius-heat ...` completed in 0.07s with
  14,958,592-byte maximum RSS. It retained 24,755 edges and nine isolates after 56,086 candidates,
  selected order 13, performed 669,630 declared matvec work, and accounted 2,268,208 working bytes.
- Two real `marklab project sparse-radius-heat` processes report miss then hit; `cmp` passes,
  `executions.jsonl` remains one line, and the direct/durable result SHA-256 is
  `769ece4d0b37345e053abff0c387da42feabaec47431db870dbee9c8fdb3f593`. The result is stored in the
  v5 Mac-mini bundle. No workspace-wide gate was repeated after checkpoint 119; no full
  integration/Nextest loop, feature matrix, benchmark suite, fuzz, memory tool, packaging,
  dependency audit, push, publication, deployment, or history rewrite ran.

## Representative CellViT witness persistence checkpoint 121 — 2026-08-28

- Expected adapter red: `python3 tests/python/test_marklab_cellvit_cptac_results_adapter.py
  CellvitCptacResultsAdapterTest.test_witness_persistence_input_retains_exact_cell_identity_and_coordinates`
  failed because `witness_persistence_input` was absent. The final exact test and complete adapter
  suite pass 1/1 and 5/5 with exact CellId/coordinate retention and declared topology bounds.
- Expected durable red: `cargo +1.96.0 test --locked --package marklab --features cli --test
  durable_gudhi_witness_project witness_persistence_replays_without_a_second_gudhi_execution --
  --exact --nocapture` failed on the absent project subcommand. The final 1/1 pass proves miss then
  fresh-process hit with `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1`, byte identity, one ledger
  record, exact source digest/schema identity, and changed-request invalidation.
- `cargo +1.96.0 test --locked --package marklab-topology` passes with zero unit/doc tests, and the
  existing `topology_witness_persistence_cli` exact five-point test passes 1/1 against the
  farthest-point/tree oracle. Affected-file Rustfmt and `git diff --check` pass.
- On `mini`, the pinned/frozen adapter revalidated 366 slides, 178 patients, and 1,542,389 cells
  into `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v6-inputs`. The 2,000-point witness
  input SHA-256 is `deea7b94b38f2430e8004e9f26d607572669b93cc904dc6e9f4e844c31c40755`.
  `/usr/bin/time -l target/debug/marklab topology witness-persistence ...` completed in 2.04 seconds
  with 40,288,256-byte maximum RSS and a 428,629-byte result. It selected 64 landmarks, recorded
  118.623619-micrometre coverage, and retained 388 simplices (64/150/174 by dimension).
- Real direct and durable outputs compare byte-for-byte; the second process succeeds with backend
  execution disabled, the ledger stays at one line, and result SHA-256 is
  `81eba331498cdebeb0cd460b6368db694e0260ada53a14df4a6a7c3acd346f40`. The result is sealed in the
  v6 Mac-mini bundle. No workspace-wide gate was repeated after checkpoint 119; no full
  integration/Nextest loop, feature matrix, benchmark suite, fuzz, memory tool, packaging,
  dependency audit, push, publication, deployment, or history rewrite ran.

## Exact-window CellViT Poisson likelihood checkpoint 122 — 2026-08-28

- Expected direct red: `cargo +1.96.0 test --locked --package marklab --features cli --test
  arbitrary_window_ipp_cli weighted_arbitrary_window_likelihood_matches_constant_intensity_oracle_and_bounds_work
  -- --exact --nocapture` failed on the absent command. Its first production attempt exposed a
  main-thread stack overflow in the legacy monolithic Bayes parser; the dedicated-parser path leaves
  `target/debug/marklab bayes --help` passing and the final exact test passes 1/1.
- The direct oracle covers one exact MultiPolygon with a hole and disconnected component, exact
  constant-intensity event/integral/likelihood arithmetic, finite/status fields, and one-short work.
  The adapter test was red on the absent boundary and the complete suite now passes 6/6.
- Expected durable red: `cargo +1.96.0 test --locked --package marklab --features cli --test
  durable_arbitrary_window_ipp_project arbitrary_window_ipp_replays_across_processes_and_invalidates_on_source_change
  -- --exact --nocapture` failed on the absent project command. The final 1/1 pass proves miss/hit
  byte identity, one ledger record, and exact source-byte invalidation. Affected-file Rustfmt and
  `git diff --check` pass.
- On `mini`, the frozen adapter revalidated 366 slides, 178 patients, and 1,542,389 cells into
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v7-inputs`. It emitted 2,000 exact events
  and 552 positive clipped quadrature cells for the exact 12-component, 787,061.271429-square-
  micrometre patch union. Event/quadrature/window digests are `965239e4c82c346e19fec097e26453812e7c8fe885cd495ecaf8ec8f3aef476c`,
  `42e759b5780340d38d1bcdf92cd7830e717df5a0c33cb62202233391666af146`, and
  `9ba8102b98f5e4f42dc4c19d9fd341b0acb6d6ecf671997ce709187312c61fb4`.
- `/usr/bin/time -l target/debug/marklab bayes arbitrary-window-ipp-likelihood ...` completed in
  0.05 seconds at 16,695,296-byte maximum RSS. At intercept -5.975159 and coefficient zero, the
  event term is -11,950.318, integral 2,000, likelihood -13,950.318, work 2,552, and retained bytes
  232,296. Direct and durable outputs compare byte-for-byte, the ledger remains one line, and result
  SHA-256 is `756c7b110fe6971e07e8bc94489a6d961c04f9667863af54f4cc905f4ff55c4a`, sealed in the v7 bundle.
  No broad workspace gate was repeated after checkpoint 119; no full integration/Nextest loop,
  feature matrix, benchmark suite, fuzz, memory tool, packaging, dependency audit, push,
  publication, deployment, or history rewrite ran.

## Durable fitted exact-window CellViT IPP checkpoint 123 — 2026-08-28

- Expected direct red: `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_arbitrary_window_ipp_fit_cli pymc_weighted_exact_window_fit_recovers_positive_covariate_effect
  -- --exact --nocapture` failed on the absent command. The final 1/1 pass recovers the known
  positive two-component covariate effect, total count, complete diagnostics, and exact backend
  identity through the unchanged rectangular worker.
- Expected durable red: `cargo +1.96.0 test --locked --package marklab --features cli --test
  durable_pymc_arbitrary_window_ipp_project arbitrary_window_pymc_fit_replays_without_second_backend_execution
  -- --exact --nocapture` failed on the absent project command. The final 1/1 pass proves miss,
  backend-disabled hit, byte identity, one ledger row, and prior-change invalidation. The existing
  `bayes_inhomogeneous_poisson_fit_cli` exact test also passes 1/1 after the new adapter connection.
- The real v7 2,000-event/552-node fit completed 2,000 posterior draws in 2.26 seconds with
  335,675,392-byte maximum RSS. R-hat is 1.00293, bulk/tail ESS 1630.45/1272.84, minimum E-BFMI
  1.21765, divergences/tree-depth hits zero, posterior coefficient -0.4566 (95% interval -0.5210 to
  -0.3888), and total expected count 2000.97. Two independent executions agree across 7,216 numeric
  fields within maximum absolute difference `1.93e-11`; durable miss/hit bytes are exact with one
  execution. Canonical result SHA-256 is `a65547c01d93e9ef51ee8821f9f42e0f61f8dd078a2b80f0c296f3a57ae89b2d`
  and is sealed in the v7 Mac-mini bundle.

## Graph/topology/exact-window stabilization checkpoint 124 — 2026-08-28

- `cargo +1.96.0 fmt --all --check` passes. The first workspace warning-denied Clippy run found one
  `needless_range_loop` in the new three-source verifier. Both affected durable tests passed after
  the zipped-iterator fix; the required final-state `cargo +1.96.0 clippy --locked --workspace
  --all-targets --all-features -- -D warnings` passes in 27m56s.
- `cargo +1.96.0 check --locked --workspace --no-default-features` passes in 22.23s.
  `cargo +1.96.0 test --locked --workspace --doc --all-features` passes every package with zero
  doctest failures. `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace
  --all-features --no-deps` passes in 25.30s, and `git diff --check` passes.
- The prohibited full workspace integration/Nextest loop was not run. No phase-only feature
  matrix, benchmark suite, fuzzing, memory tool, packaging, dependency audit, push, publication,
  deployment, or history rewrite ran.

## Exact-window IPP agreement and prior sensitivity checkpoint 125 — 2026-08-28

- Expected agreement red: `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_arbitrary_window_ipp_agreement_cli pymc_and_numpyro_agree_on_weighted_exact_window_ipp --
  --exact --nocapture` failed on the absent command. The final 1/1 pass covers a known positive
  coefficient, both exact backend identities/diagnostics, interval overlap, parameter/count gates,
  and truthful agreement status. Python syntax validation passes for the new narrow worker.
- Expected sensitivity red: `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_arbitrary_window_ipp_sensitivity_cli weighted_exact_window_ipp_runs_fixed_prior_scale_grid
  -- --exact --nocapture` failed on the absent command. The final 1/1 pass covers all five fixed
  scenarios, complete diagnostics, baseline direction, standardized shifts, and total-work bounds.
- The real agreement completed in 4.52 seconds at 566,575,104-byte maximum RSS. PyMC/NumPyro
  differences are 0.000352 intercept, 0.000793 coefficient, and 0.631 total expected count; all pass
  the declared 0.05/0.05/5.0 gates with zero divergences/depth hits. Result SHA-256 is
  `dda4b0d95470b25af8f217839df55920a7ed53de6c1c84b619aa765d608b2b9e`.
- The real five-fit sensitivity completed in 8.75 seconds at 327,598,080-byte maximum RSS. Every fit
  is complete, total declared draw-node work is 5,520,000, no scenario reaches 0.75 standardized SD,
  and the maximum is 0.0603. Result SHA-256 is
  `64e644a073123dc43ad36c0ae47a01a52841763718ac4570c450f53b1df7f2a7`. Both results are sealed in
  the v7 Mac-mini bundle. No broad gate was repeated after checkpoint 124; no full
  integration/Nextest loop, feature matrix, benchmark suite, fuzz, memory tool, packaging,
  dependency audit, push, publication, deployment, or history rewrite ran.

## Exact-window IPP diagnostic promotion checkpoint 126 — 2026-08-28

- Expected adapter red: `python3 tests/python/test_marklab_cellvit_cptac_results_adapter.py
  CellvitCptacResultsAdapterTest.test_arbitrary_window_ipp_events_retain_quadrature_membership_when_requested`
  failed because the exact event-to-node mapping was absent. The final full adapter suite passes
  7/7, and Python syntax compilation passes for the adapter, shared PyMC worker, spatial-PPC
  worker, and NumPyro SBC worker.
- Expected SBC red: the exact CLI test first failed on the absent command. The all-draws-rejected
  regression then failed with non-JSON `nan` calibration summaries before the worker emitted a
  finite zero-completion diagnostic. `cargo +1.96.0 test --locked --package marklab --features cli
  --test bayes_arbitrary_window_ipp_sbc_cli -- --nocapture` passes 2/2, covering accepted
  prior-generative rank/coverage and explicit generated-count-ceiling failures.
- Expected spatial-PPC red: `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_arbitrary_window_ipp_spatial_ppc_cli
  weighted_exact_window_ipp_spatial_ppc_uses_event_node_membership_and_physical_neighbors --
  --exact --nocapture` failed on the absent command. The final 1/1 pass checks exact membership,
  two physical neighbor pairs, analytic node-density variance 1,075 and neighbor contrast 50,
  2,000 bounded replicas, finite tails, and complete diagnostics.
- Identity regressions first failed because prior- and quadrature-sensitivity scenarios lacked
  backend/request/input fields. The combined final command over
  `bayes_arbitrary_window_ipp_sensitivity_cli` and
  `bayes_arbitrary_window_ipp_quadrature_sensitivity_cli` passes 2/2 with exact PyMC/worker,
  request, logical input, sampling, work, and shift evidence. The final agreement control passes
  1/1; direct fit passes 1/1; durable fit passes 1/1 with miss, backend-disabled hit, byte identity,
  one ledger execution, and changed-prior invalidation.
- The Mac-mini adapter command with the pinned CellViT environment and explicit CellViT checkout
  `PYTHONPATH` re-audited 366 slides and 1,542,389 cells in 110.49 seconds at 689,995,776-byte
  maximum RSS. The v10 bundle contains 2,000 events, 2,000 identity-aligned membership rows, 381
  occupied nodes, and 552 baseline nodes; every referenced node exists. Membership SHA-256 is
  `99f95d5850db48daa97441baae93c37dfd6bef7055f2b436b43898d35c624da6`.
- The final real durable fit miss took 7.11 seconds at 316,522,496-byte maximum RSS; a fresh process
  with `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1` returned the hit, `cmp` passed, and the ledger
  contains one line. Result SHA-256 is
  `bc13a44e7ac7a17bc7ad3931712a16b9d19eabba4f318e7d0d8e58fae18b309b`.
- Final real backend agreement took 5.14 seconds at 563,822,592-byte maximum RSS and retains the
  prior 0.000352/0.000793/0.631 intercept/coefficient/count differences with all gates passing;
  SHA-256 is `0fd65d12afd1eb6d8649f0f10cf1d3378b4e65e94b62f8e95cc6fdd2a7e4262c`.
  Final prior sensitivity took 8.23 seconds at 313,507,840-byte maximum RSS; all five fits complete,
  maximum shift remains 0.0603, no scenario is material, and SHA-256 is
  `e9ab431fd1f204e4e90e022730983fd4101302b2c528f4327cef125a480a71f8`.
- Final 372/552/845-node quadrature sensitivity took 5.41 seconds at 340,246,528-byte maximum RSS,
  declared 3,538,000 draw-node operations, maximum standardized shift 0.00193, and no material
  change. SHA-256 is `a5df627be5f735004d1130181eb73edf3e3866391a6dbf2a17c1ee542e4fa364`.
- The deployed-prior SBC took 13.10 seconds at 1,196,376,064-byte maximum RSS. It completes 16/20
  replicas with all three completed-replicate calibration summaries accepted, zero divergences,
  and four exact generated-count-ceiling failures; the overall result is correctly `not_accepted`.
  SHA-256 is `e1bb2af3d46be486ebcc28e1b1cbdb263e71c6ced9ab7eb1ee051ab1d11c264f`.
  The separately named physical-prior SBC took 15.01 seconds at 1,323,335,680-byte maximum RSS and
  accepts 20/20 with maximum R-hat 1.01666 and zero divergences; SHA-256 is
  `74f06d3a5472f0b5aa95b197d9a2c3f66a85ea8eb21a8a28c92a3450fa67725a`.
- The final real spatial PPC took 1.83 seconds at 428,130,304-byte maximum RSS over 3,233 fixed
  100-micrometre neighbor pairs and 7,570,000 predictive work units. The complete fit has tail
  probabilities 0.0405 for node-density variance and 0.0200 for neighbor density contrast;
  SHA-256 is `ead46ba7f9f05aa7b0aa654e41e774853a5e14adbce3f023380ad0e73e963d42`.
  `shasum -a 256 -c results_final_sha256.txt` passes for all seven final v10 results.
- `cargo +1.96.0 clippy --locked --package marklab --features cli --bin marklab` with the seven
  affected integration targets and `-D warnings` passes on the final state in 2m06s. `cargo +1.96.0 check --locked
  --package marklab --no-default-features` passes in 9.73s. Affected-file Rustfmt,
  `cargo +1.96.0 fmt --all --check`, and `git diff --check` pass. The prohibited workspace
  integration/Nextest loader loop, full feature matrix, broad docs, benchmarks, fuzzing, memory
  tools, packaging, dependency audits, push, publication, deployment, and history rewrite did not
  run.

## Durable exact-window CellViT latent-field checkpoint 127 — 2026-08-28

- Expected direct-fit red: `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_arbitrary_window_lgcp_fit_cli` failed on the absent `fit-arbitrary-window-lgcp` command.
  The final test passes and differentially agrees within `1e-10` with the existing rectangular
  PyMC LGCP for the intercept, coefficient, and every four-node latent mean; independent observed
  spatial summaries are exactly 28.75 density variance and 7.0 mean four-pair contrast.
- Expected durable red: `cargo +1.96.0 test --locked --package marklab --features cli --test
  durable_pymc_arbitrary_window_lgcp_project` failed on the absent project command. The final test
  passes with a miss, backend-disabled fresh-process hit, byte identity, one execution record, and
  typed exact-f64 hit validation. Expected sensitivity red likewise failed on the absent
  `arbitrary-window-lgcp-sensitivity` command before the final five-scenario test passed.
- The combined affected Rust command over direct LGCP, sensitivity, durable LGCP, shared IPP spatial
  membership, and durable IPP passes 5/5. The adapter direct-file suite passes 7/7. The initial
  `python3 -m unittest tests/python/test_marklab_cellvit_cptac_results_adapter.py` attempt failed
  before discovery because `tests.python` is not an importable package; the repository's direct
  `python3 tests/python/test_marklab_cellvit_cptac_results_adapter.py` command then passes. A first
  syntax command named a nonexistent historical IPP worker path after compiling the preceding
  files; the corrected command over the adapter, exact-window LGCP wrapper, IPP spatial wrapper,
  and source gridded-LGCP worker passes.
- The Mac-mini adapter re-audited 366 slides, 178 patients, and 1,542,389 cells in 106.64 seconds at
  730,546,176-byte maximum RSS. The v11 input contains 2,000 memberships over 32 positive exact
  clipped nodes, 30 occupied nodes, and area 787,061.271429 square micrometres. Raw input SHA-256
  values are `965239e4c82c346e19fec097e26453812e7c8fe885cd495ecaf8ec8f3aef476c` events,
  `22c580b4429b186ebd16b38aeb01cf30f755218d95943ef10fa0d17108e2a20e` membership,
  `afa0c1e4e4d1cdc73a12e7617d18bcd51a449867ae0752dd17161e879618f22a` quadrature, and
  `9ba8102b98f5e4f42dc4c19d9fd341b0acb6d6ecf671997ce709187312c61fb4` window.
- The identity-final real durable miss took 15.09 seconds at 302,612,480-byte maximum RSS; a second
  process with external backend execution disabled returned the byte-identical hit with one ledger
  execution. The 4,000 retained-draw fit has R-hat 1.00492, bulk/tail ESS 1783.98/2034.15, minimum
  E-BFMI 0.94255, zero divergences/depth hits, and SHA-256
  `8ff67152ce931e51b11399b73344c178d56b1c61c0dd58772fc414644514142e`.
  Its 32-replicate/77-pair physical PPC has zero upper-tail probability for both observed summaries.
- The real five-fit fixed-kernel sensitivity command completed in 57.44 seconds at
  349,552,640-byte maximum RSS and declared 640,000 draw-node operations. All fits are complete with
  zero divergences; the maximum standardized change is 0.69398 at the doubled 1,024-micrometre
  length scale, below the prespecified 0.75 threshold. The sealed v11 result SHA-256 is
  `da284929a3a1e6cd4b0f83b27a570754a257f621b5fafbd40e600937b92ad2fc`.
- Targeted warning-denied Clippy over the affected binary and five integrations passes. `cargo
  +1.96.0 check --locked --package marklab --no-default-features`, Python syntax checks,
  affected-file Rustfmt, and diff whitespace checks pass. No workspace-wide integration/Nextest
  loop, full feature matrix, broad docs, benchmarks, fuzzing, memory tool, packaging, dependency
  audit, push, publication, deployment, or history rewrite ran.

## Exact-window latent-field promotion checkpoint 128 — 2026-08-28

- Expected agreement red: `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_arbitrary_window_lgcp_agreement_cli -- --nocapture` failed on the absent command. The final
  control passes with both exact adapter/source NumPyro identities, complete diagnostics, and
  Monte Carlo-aware agreement for global parameters and every four-node latent/count summary.
- Expected SBC red likewise failed on the absent `arbitrary-window-lgcp-sbc` command. The final
  four-node control passes 20/20 complete dispositions, physical-node identity, rank histograms,
  coverage, and outer/source worker validation while calling the existing gridded SBC owner.
  Expected quadrature red failed on the absent sensitivity command; its final 4/6/8-node control
  passes exact area/membership, complete fits, analytic near-zero-field stability, and 36,000 exact
  draw-node operations.
- The first real v12 adapter attempt audited all 366 slides but failed before publication because
  its existing internal membership/quadrature guards required grid size at least eight. A focused
  four-grid regression then failed under the same guard. After admitting the immediate 4/6/8 LGCP
  caller under the still-bounded 4–128 internal range, the regression and full adapter suite pass.
  The failed `.part` directory was moved to trash. The final adapter completed in 93.91 seconds at
  991,035,392-byte maximum RSS and emitted 11/22/32 positive exact-clipped nodes with 2,000 complete
  identity-aligned memberships at every resolution.
- The real agreement completed in 15.66 seconds at 591,396,864-byte maximum RSS. Both fits are
  complete with zero divergences/depth hits. Intercept/coefficient differences are 0.00215/0.00492,
  maximum standardized latent/count differences are 0.744/1.580, and every node interval overlaps.
  SHA-256 is `8f80eecad54f21b964317029bd846d4a597f658854657f3521a2ec25d324f635`.
- The deployed-prior physical SBC completed in 44.93 seconds at 1,498,644,480-byte maximum RSS and
  accepts 20/20 with rank-uniformity p-values 0.163/0.534/0.834 and 90% coverage 0.90/0.80/0.90 for
  intercept/coefficient/physical latent node. SHA-256 is
  `9670d00a8e003de46d5ceeafda09cca6dc7b4fd219f652a80b8d1ad15274fa62`.
- The real 11/22/32-node quadrature sensitivity completed in 28.58 seconds at 334,839,808-byte
  maximum RSS. All 260,000 draw-node operations complete; maximum global shift is 0.512 posterior
  SD, total-count relative shift is below 0.00075, and no resolution is material at 0.75. Its
  aggregation-dependent spatial PPC is retained rather than compared as an invariant. SHA-256 is
  `c32b4903302c944a674dc4b1a2bbbf60a64a8c618bd2a7f11249b105c9b8e487`.
- The combined affected command over seven integration binaries passes 7/7; the adapter passes 7/7
  and all four changed Python workers compile. `cargo +1.96.0 fmt --all --check` passes. `cargo
  +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings` passes in 28.35
  seconds. Workspace no-default compilation passes in 1.10 seconds, all-feature doctests pass with
  zero failures after 19.96 seconds, and strict all-feature docs pass in 8.44 seconds. Diff
  whitespace checks pass. The v12 `shasum -a 256 -c results_sha256.txt` manifest verifies all five
  exact-window LGCP results. The prohibited full-workspace integration/Nextest loader loop was not
  run; no feature matrix, benchmark, fuzz, memory tool, packaging, dependency audit, push,
  publication, deployment, or history rewrite ran.

## Durable replicated-patient CellViT field checkpoint 129 — 2026-08-28

- Expected direct red: `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_replicated_arbitrary_window_lgcp_fit_cli -- --nocapture` failed on the absent command. The
  final known-shift control passes with eight patients, two patterns each, exact 64-node identity,
  positive group recovery, patient/pattern uncertainty, complete diagnostics, and patient unit.
- Expected durable red: the focused durable integration failed on the absent project command. Its
  final run passes with miss, fresh backend-disabled hit, byte identity, one ledger execution,
  exact-f64 codec, and typed hit validation. The existing replicated-model builder remains green.
- The adapter selection test first failed on its absent owner, then passes four patients per MSI/MSS
  group and two provenance-sorted slides each. The first real adapter run stopped after 7.51 seconds
  because the existing 2,000-row membership guard rejected an all-cell use. The caller now streams
  exact all-cell counts/digests without materializing that table; the guard remains unchanged. The
  v14 audit completed in 94.95 seconds at 904,511,488-byte RSS and emitted eight patients, 16
  patterns, 222 nodes, and 69,377 cells. Input SHA-256 is
  `4c19d880519901524db244c3891eb165222df3c169a4278090caa983d5bc0ee7`.
- The initial unconstrained sampler had four divergences and 3,731 depth hits. Group-wise patient
  centering then converged but was rejected because it removed between-patient group uncertainty.
  The retained model leaves patient effects unconstrained while centering slide effects within
  patient and fields within slide. Neither rejected artifact was sealed as final evidence.
- The final durable miss completed in 171.81 seconds at 453,492,736-byte RSS. Diagnostics are R-hat
  1.00772, bulk/tail ESS 999.07/1401.69, E-BFMI 0.95955, zero divergences, and zero depth hits. The
  MSI-minus-MSS log-intensity effect is 0.18694 with interval [-0.51781, 0.88489], patient SD
  0.55032, and slide SD 0.76332. A fresh backend-disabled process returned the byte-identical hit;
  the project has one ledger row. SHA-256 is
  `e088d056e48cd28aade956068000f74d9cddd761386c7efb28b7677082d15161`, and the v14 final manifest
  passes `shasum -a 256 -c`.
- The final affected command passes direct, durable, and builder integrations 3/3; the adapter
  passes 8/8 and Python syntax passes. Targeted warning-denied Clippy and package no-default
  compilation pass after fixing one reported unnecessary cast. Affected Rustfmt and whitespace
  checks pass. Broad gates were not repeated after checkpoint 128; the prohibited integration/
  Nextest loop, feature matrix, benchmarks, fuzzing, memory tools, packaging, dependency audit,
  push, publication, deployment, and history rewrite did not run.

## Replicated-patient CellViT field promotion checkpoint 130 — 2026-08-29

- The direct PPC assertion first failed because `pattern_posterior_predictive` was absent, then the
  exact direct and durable tests passed with 16 typed rows, 3,000 deterministic replicates per
  synthetic pattern, version-2 cache identity, fresh-process backend-disabled byte replay, and one
  ledger row. The sensitivity, agreement, and SBC integrations each first failed on an absent CLI
  command. Their final combined command passes 5/5; the existing replicated builder passes 1/1.
- The nine-fit synthetic sensitivity oracle passes with an exact 1,728,000 draw-node total and zero
  baseline shifts. PyMC/NumPyro agree on all five globals, eight patient effects, 16 slide effects,
  and 64 latent/expected nodes. The four-chain SBC accepts 20/20 replicates for group effect,
  patient SD, slide SD, and one prespecified physical latent node without hidden failures.
- The real version-2 durable miss/hit is byte-identical with one ledger row. The miss took 168.43
  seconds at 458,604,544-byte RSS; SHA-256 is
  `f9bc6c51ed0cfd5b18558855dc5115853448580ab14cd6347f68949fdbcbe08f`.
  All 16 total-count and node-variance PPC rows are retained; minimum tails are 0.9365/0.0555.
- Real agreement at NumPyro depth 10 and 12 is retained but rejected for 1,211 and 3 depth hits.
  Depth 13 completed in 279.10 seconds with zero divergences/depth hits and all 473 compared
  quantities interval-overlapping. SHA-256 is
  `2f5051523b5b21a06cefd1c978499fc8e61821e27190864b8542b06277fcfbe9`.
- The real nine-scenario sensitivity run took 1,841.59 seconds under 7,992,000 draw-node work.
  Patient/slide scale scenarios stay below 0.366 SD, while field scenarios reach 2.079 SD and three
  fits have 4/250/6 depth hits. SHA-256 is
  `b9bb082f9b8c1d7fc2e6ae9505e70a1fc2bcf87158c719054aae40563af4b8ce`.
  This negative stability result is final evidence, not an optimization target.
- Deployed-prior SBC retained 16 failures: 14 event-ceiling excesses and two depth-saturated fits.
  The separately named physical count-scale prior completed 20/20 on the same 222-node geometry in
  252.49 seconds; SHA-256 is
  `481f9969bf5d684b3e199cc58b6161f7162ef5241d05a134ac3c69b00b53c16a`.
  `ssh mini 'cd ...v15-lgcp-promotion && shasum -a 256 -c results_sha256.txt'` passes all eight
  canonical/diagnostic files and the sealed durable ledger has one row.
- Final focused Python syntax, affected Rustfmt, `git diff --check`, warning-denied affected Clippy,
  package no-default compilation, and all six focused integrations pass. The scheduled stabilization
  commands `cargo +1.96.0 fmt --all --check`, workspace all-target/all-feature warning-denied Clippy,
  workspace no-default compilation, all-feature doctests, and strict workspace docs all pass. The
  documented full-integration/Nextest loader loop was not retried; no feature matrix, benchmark,
  fuzz, memory tool, packaging, dependency audit, push, publication, deployment, or history rewrite
  ran.

## Inferred shared-kernel replicated CellViT field checkpoint 131 — 2026-08-29

- The direct behavior test first failed on the absent
  `fit-replicated-arbitrary-window-lgcp-inferred-kernel` command. It then passed with eight patients,
  16 patterns, 64 nodes, recovered positive group shift, finite inferred amplitude/length, complete
  diagnostics, node posteriors, and pattern PPC. The durable test first failed on the absent project
  command and then passed miss/fresh-process backend-disabled hit/byte identity/one ledger row.
- The real depth-10 durable miss completed in 527.65 seconds but retained 377 tree-depth hits and
  `fit_state=nonconverged`. The explicit depth-13 regression first failed on its absent CLI control;
  final direct/durable tests pass with that exact control and the zero-hit policy unchanged.
- The real depth-13 miss completed in 582.67 seconds at 475,627,520-byte RSS. Diagnostics are R-hat
  1.00453, bulk/tail ESS 514.84/830.86, E-BFMI 0.65048, zero divergences, and zero depth hits. A
  fresh backend-disabled process returned the byte-identical hit and the ledger has one row.
  SHA-256 is `4ddb116a550c61dabc76c6eef10a25c646b9445748692d076b83921ff39a0b8f`.
- `ssh mini 'cd ...v16-lgcp-inferred-kernel && shasum -a 256 -c
  inferred_results_sha256.txt'` passes the unchanged input, canonical fit, and rejected depth-10
  diagnostic; the sealed durable ledger has one row. The scientific group interval crosses zero,
  while the inferred kernel and improved PPC are reported without attraction or significance claims.
- Final Python syntax, affected Rustfmt, warning-denied affected Clippy, package no-default
  compilation, and `git diff --check` pass. The two focused integrations pass. Broad workspace
  gates, the documented full-integration/Nextest loop, feature matrix, benchmarks, fuzzing, memory
  tools, packaging, dependency audit, push, publication, deployment, and history rewrite did not run.

## Inferred shared-kernel agreement/calibration checkpoint 132 — 2026-08-29

- `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_replicated_arbitrary_window_lgcp_inferred_kernel_agreement_cli -- --nocapture` first failed
  on the absent command and then passed. The final-state rerun passes in 23.95 seconds.
- `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_replicated_arbitrary_window_lgcp_inferred_kernel_sbc_cli -- --nocapture` first failed on the
  absent command and then passed. The final-state 20-replicate rerun passes in 238.55 seconds.
- The real agreement ran with two chains, 2,000 tune/draws, and explicit PyMC/NumPyro depth 13 in
  929.57 seconds at 970,342,400-byte maximum RSS. Both backends are complete, and all global,
  patient, slide, latent, and expected-count comparisons pass. SHA-256 is
  `7e1336748f4cf32b629429865e0869d3213c5fd4a8cc0634ae8f2799150050e0`.
- The first real-geometry SBC ran 20 replicates at four chains, 750 tune, and 1,000 draws in 814.20
  seconds at 6,024,445,952-byte maximum RSS. It retains one R-hat failure and is nonconverged. The
  only sampling-capacity retry ran four chains, 1,000 tune, and 1,500 draws in 1,063.25 seconds at
  6,345,883,648-byte maximum RSS; it retains one divergence and is nonconverged. Their SHA-256 values
  are `ee77fe54cdce6a935471cc317b925fbd0d8b8599d1c4cd619ddb634b2440bbf0` and
  `c366c0b8a09053383296b4f5ce86bcb07db51cfd40a02cd9aa2a68b509ea8b28`.
- Python `py_compile` passes for both new workers. Affected Rustfmt passes. `cargo +1.96.0 clippy
  --locked --package marklab --features cli --bin marklab --test
  bayes_replicated_arbitrary_window_lgcp_inferred_kernel_agreement_cli --test
  bayes_replicated_arbitrary_window_lgcp_inferred_kernel_sbc_cli -- -D warnings` passes.
  `cargo +1.96.0 check --locked --package marklab --no-default-features` passes.
- `ssh mini 'cd ...v17-lgcp-inferred-promotion && shasum -a 256 -c
  promotion_results_sha256.txt'` passes all seven sealed files. Broad workspace gates, the documented
  full-integration/Nextest loop, feature matrix, benchmarks, fuzzing, packaging, dependency audit,
  push, publication, deployment, and history rewrite did not run.

## Conditional hard-multitype CellViT mark checkpoint 133 — 2026-08-29

- `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_conditional_multitype_mark_cli -- --nocapture` first failed on the absent command and passes
  in the final state. Its independent three-class spatial-segregation oracle requires all three
  cross-affinity upper intervals below zero and conditional score improvement above 20.
- `cargo +1.96.0 test --locked --package marklab --features cli --test
  durable_pymc_conditional_multitype_mark_project -- --nocapture` first failed on the absent project
  command and passes in the final state. Two fresh processes prove miss/backend-disabled hit, byte
  identity, and one execution-ledger row.
- The real durable miss over the exact admitted 2,000-cell table and 24,755-edge radius graph runs in
  30.00 seconds at 879,837,184-byte maximum RSS. Diagnostics are R-hat 1.00110, bulk/tail ESS
  1,601.24/1,896.05, E-BFMI 0.96426, and zero divergences/depth hits. A fresh process with
  `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1` returns a byte-identical hit; the ledger has one
  row. SHA-256 is `03ddbacba4b729ac0814079298067cf3499d9f9427028a27e1197d68a0c5a371`.
- Python `py_compile`, affected Rustfmt, `git diff --check`, warning-denied affected Clippy, and
  `cargo +1.96.0 check --locked --package marklab --no-default-features` pass. `ssh mini 'cd
  ...v19-conditional-multitype && shasum -a 256 -c conditional_multitype_sha256.txt'` passes all
  five sealed files. Broad workspace gates, the documented full-integration/Nextest loop, feature
  matrix, benchmarks, fuzzing, packaging, dependency audit, push, publication, deployment, and
  history rewrite did not run.

## Conditional hard-multitype promotion checkpoint 134 — 2026-08-29

- `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_conditional_multitype_mark_agreement_cli -- --nocapture` first failed on the absent command
  and passes in the final state. It independently compares intercepts, potentials, invariant
  affinities, conditional score, expected same-edge count, and expected type counts.
- `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_conditional_multitype_mark_sensitivity_cli -- --nocapture` first failed on the absent
  command. Its first fitted run retained one marginal R-hat failure and material interaction-prior
  shifts; the final capacity run passes and requires `material_prior_sensitivity` on that synthetic
  control. Final direct, durable, agreement, and sensitivity integrations pass in 4.57, 14.97, 8.56,
  and 15.93 seconds.
- The real agreement completes in 37.30 seconds at 1,308,311,552-byte maximum RSS. PyMC/NumPyro
  diagnostics are R-hat 1.00110/1.00124, bulk ESS 1,601.24/1,250.12, tail ESS
  1,896.05/1,954.47, E-BFMI 0.96426/1.02740, and zero divergences/depth hits. SHA-256 is
  `50299b063e86bfba900f4587100544a7adf550855e0c5a6372b446701f41bf8f`.
- The real five-scenario grid completes in 106.77 seconds at 1,139,654,656-byte maximum RSS. Every
  scenario is complete; maximum affinity shifts are 0.111, 0.050, 0.647, and 0.190 baseline SD for
  half/double intercept and half/double interaction priors. SHA-256 is
  `f923906bade54e9ac62b309bbecb1fbf341f07c8d3d5cccb0368e9bc3e31fe6c`.
- Both new worker `py_compile` checks, affected Rustfmt, `git diff --check`, warning-denied affected
  Clippy, and package no-default compilation pass. `ssh mini 'cd ...v20-conditional-multitype-
  promotion && shasum -a 256 -c conditional_multitype_promotion_sha256.txt'` passes all five files.
  Broad workspace gates and the documented full-integration/Nextest loop were not run before exact
  finite-state calibration; no feature matrix, benchmark, fuzzing, packaging, dependency audit,
  push, publication, deployment, or history rewrite ran.

## Exact finite-state conditional multitype calibration checkpoint 135 — 2026-08-29

- `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_conditional_multitype_mark_sbc_cli -- --nocapture` first failed on the absent command and
  passes in the final state. It checks exactly 19,683 states, the analytic `9*ln(3)` zero-parameter
  log normalizer, 20 exact dispositions, and all ten parameter diagnostics.
- The final standalone run completes in 19.94 seconds at 1,473,626,112-byte maximum RSS with 20/20
  complete replicates, no failures, minimum rank-uniformity p-value 0.04872, and 90% coverage
  0.75–1.00. SHA-256 is `64e933ef674901e2b1ac2cda73bb0ad322be14a93c75fddfe54cdc41ac5edeea`.
- Python `py_compile` passes for the SBC worker. `ssh mini 'cd ...v21-conditional-multitype-
  calibration && shasum -a 256 -c calibration_sha256.txt'` passes its three sealed files. Major
  stabilization commands are recorded below after they run; the documented Nextest/full-integration
  loader loop remains prohibited. No benchmark, fuzzing, packaging, dependency audit, push,
  publication, deployment, or history rewrite ran.
- Major stabilization passes: `cargo +1.96.0 fmt --all --check`; `cargo +1.96.0 clippy --locked
  --workspace --all-targets --all-features -- -D warnings` in 24.85 seconds; `cargo +1.96.0 check
  --locked --workspace --no-default-features` in 1.24 seconds; `cargo +1.96.0 test --locked
  --workspace --doc --all-features` with zero failures; and `RUSTDOCFLAGS='-D warnings' cargo
  +1.96.0 doc --locked --workspace --all-features --no-deps`. The documented macOS Nextest/full-
  integration loader loop was not retried, so no full integration result is claimed.

## Typed probability-simplex composition checkpoint 83 — 2026-08-27

- `cargo +1.96.0 test --locked --package marklab --test
  probability_simplex_composition_workflow` first failed on absent fixture/type/result symbols and
  its durable test separately failed on the absent node. The final 3/3 pass covers exact soft means
  and entropies, deterministic replay, malformed sum/shape, point/class/value/memory ceilings,
  fresh miss, reconstructed hit with one execution, and limit-only cache invalidation.
- `python3 tests/fixtures/probability_simplex/generate_python_oracle.py | diff -u
  tests/fixtures/probability_simplex/python_composition_oracle.json -` passes byte-for-byte. The
  independent standard-library oracle agrees on all three means, row/aggregate entropy, effective
  class count, and exact zero row-sum error.
- The affected command passes simplex 3/3, scalar input 11/11, declared marked workflow 9/9, and
  typed Moran 2/2. Targeted warning-denied Clippy over those four integrations passes. `cargo
  +1.96.0 check --locked --package marklab --no-default-features`, package doctests, affected-file
  Rustfmt, and diff whitespace checks pass. Workspace-wide gates were not repeated after the
  immediately preceding checkpoint-82 stabilization; no Nextest/full integration loop, feature
  matrix, benchmark, fuzz, memory tool, packaging, dependency audit, push, publication, deployment,
  or history rewrite was run.

## Fixed-radius soft neighborhood checkpoint 84 — 2026-08-27

- `cargo +1.96.0 test --locked --package marklab --test
  soft_neighborhood_composition_workflow` first failed on unresolved typed/config symbols and its
  durable test separately failed on the absent node. The final 3/3 pass covers exact per-cell/
  aggregate probability mass, typed all-isolated and mixed zero-neighbor states, invalid radius,
  one-short pair/memory work, fresh miss, reconstructed hit with one execution, and radius-only
  cache invalidation.
- `python3 tests/fixtures/soft_neighborhood/generate_python_oracle.py | diff -u
  tests/fixtures/soft_neighborhood/python_radius_oracle.json -` passes byte-for-byte. The independent
  direct pair loop agrees on two visits, every row state/vector, two unavailable rows, and aggregate
  `[0.5,0.5]`.
- The affected command passes soft neighborhood 3/3 and simplex composition 3/3. Targeted
  warning-denied Clippy including scalar input passes. Package no-default compilation, package
  doctests, affected-file Rustfmt, and diff whitespace checks pass. Workspace-wide gates were not
  repeated after checkpoint 82; no full integration/Nextest loop, feature matrix, benchmark, fuzz,
  memory tool, packaging, dependency audit, push, publication, deployment, or history rewrite ran.

## Prespecified multiscale soft-neighborhood checkpoint 85 — 2026-08-28

- `cargo +1.96.0 test --locked --package marklab --test
  soft_neighborhood_composition_workflow complete_simplex_rows_flow_across_one_fixed_physical_radius
  -- --exact` failed first because the new multiscale codec accepted a corrupted row neighbor count;
  after structural row/scale/adjacent-distance validation it passes 1/1.
- `cargo +1.96.0 test --locked --package marklab --test
  soft_neighborhood_composition_workflow --test probability_simplex_composition_workflow --test
  scalar_mark_input` passes 17/17. It covers one shared geometry build, exact two-scale outputs,
  typed all-isolated scales, invalid/duplicate radii, one-short pair/memory ceilings, inconsistent
  codec rows, fresh misses, reopened fixed/multiscale hits, and radius/radius-list invalidation.
- `python3 tests/fixtures/soft_neighborhood/generate_python_oracle.py | diff -u
  tests/fixtures/soft_neighborhood/python_radius_oracle.json -` passes byte-for-byte. The independent
  direct-pair loop agrees on six total visits, every scale row/vector, second-scale aggregate
  `[0.375,0.625]`, and adjacent total variation `0.125`.
- Targeted warning-denied Clippy over the three affected integrations passes. `cargo +1.96.0 check
  --locked --package marklab --no-default-features`, `cargo +1.96.0 test --locked --package marklab
  --doc`, affected-file Rustfmt, and diff whitespace checks pass. Workspace-wide gates were not
  repeated after checkpoint 82; no full integration/Nextest loop, feature matrix, benchmark, fuzz,
  memory tool, packaging, dependency audit, push, publication, deployment, or history rewrite ran.

## Row-bound CellViT vector-artifact checkpoint 86 — 2026-08-28

- `cargo +1.96.0 test --locked --package marklab --test cellvit_embedding_artifact_graph
  verified_cellvit_artifact_is_a_row_bound_typed_vector_mark_for_the_centroid_caller -- --exact`
  first failed on the absent vector declaration/unit/column/accessor. The focused typed test then
  passed after implementing the no-copy row-bound artifact reference.
- The first typed workflow run failed with `Input { source: SemanticRoleAlias }` because the new
  MarkTable and the existing centroid boundary correctly named the same embedding roles. After the
  caller verified artifact agreement and formed one exact semantic-input union, the focused
  workflow test passes with a store miss, identical hit, one successful execution, and the unchanged
  exact private codec.
- `cargo +1.96.0 test --locked --package marklab --test cellvit_embedding_artifact_graph
  declared_binary_centroid` passes 14/14. It covers the new typed/no-copy path, artifact/table/QC/
  CellId mismatch, nullable and non-nullable status policies, caller-reference mismatch, semantic
  store verification, cache identity, exact limits, availability states, and codec rejection.
  The separate affected MarkTable/simplex/neighborhood command passes 17/17.
- Targeted warning-denied Clippy over the four affected integrations passes after boxing the compact
  artifact metadata inside the existing column enum. `cargo +1.96.0 check --locked --package
  marklab --no-default-features`, package doctests, affected-file Rustfmt, and diff whitespace checks
  pass. Workspace-wide gates were not repeated; no full integration/Nextest loop, feature matrix,
  benchmark, fuzz, memory tool, packaging, dependency audit, push, publication, deployment, or
  history rewrite ran.

## Patient population-independence design checkpoint 87 — 2026-08-28

- The focused `marklab-cohort` unit test first failed on the absent
  `InferenceDesign::population_independence` constructor and `PopulationIndependence` null-family
  variant. It now passes 1/1 and proves every shared index permutation reproduces the legacy
  method-namespaced whole-label Fisher–Yates stream with exact group-count preservation.
- `cargo +1.96.0 test --locked --package marklab-cohort --test mmd_reference --test
  energy_reference --test inference_design_reference` passes 4/4. The unchanged independent slow
  references agree exactly on linear/RBF biased/unbiased MMD statistics and p-values, Euclidean
  energy distance and p-value, deterministic blocked schedules, and design rejection boundaries.
- `cargo +1.96.0 clippy --locked --package marklab-cohort --all-targets -- -D warnings`, `cargo
  +1.96.0 check --locked --package marklab-cohort --no-default-features`, package doctests,
  affected-file Rustfmt, and diff whitespace checks pass. No workspace-wide/Nextest loop, feature
  matrix, benchmark, fuzz, memory tool, packaging, dependency audit, push, publication, deployment,
  or history rewrite ran.

## Complete formal vector-mark admission checkpoint 88 — 2026-08-28

- The focused probability cross-covariance test first failed on the absent typed-reference mismatch
  error and then passed after exact artifact agreement became part of the caller. The nucleus-area
  test independently failed on the same missing production behavior and then passed after its own
  typed boundary was connected.
- `cargo +1.96.0 test --locked --package marklab --test cellvit_embedding_artifact_graph` passes
  64/64. It covers all three typed vector callers, old no-reference compatibility, exact 1,280-D
  numeric/status oracles, artifact/table/QC/CellId/provenance drift, unavailable embedding states,
  Arrow/Parquet equivalence and hostile inputs, hard resource bounds, centroid semantic-store
  verification, miss/hit replay, and exact codec rejection.
- `cargo +1.96.0 clippy --locked --package marklab --test cellvit_embedding_artifact_graph -- -D
  warnings`, `cargo +1.96.0 check --locked --package marklab --no-default-features`, package
  doctests, affected-file Rustfmt, and diff whitespace checks pass. Workspace-wide/Nextest loops,
  broad feature matrices, benchmarks, fuzzing, memory tools, packaging, dependency audits, push,
  publication, deployment, and history rewriting were not run.

## Durable ordinal IHC composition checkpoint 89 — 2026-08-28

- The focused ordinal integration first failed on unresolved production symbols. Its durable test
  then failed because ordinary JSON decimal replay changed entropy from
  `1.0397207708399179` to `1.039720770839918`. After moving the already proven bit-tag finite-f64
  codec into one crate-private owner, the final ordinal integration passes 2/2 with exact miss/hit/
  limit invalidation.
- `python3 tests/fixtures/ordinal_composition/generate_python_oracle.py | diff -u
  tests/fixtures/ordinal_composition/python_oracle.json -` passes byte-for-byte. The independent
  standard-library oracle agrees on every count/proportion/CDF, both median levels, entropy,
  normalized entropy, and effective level count.
- The affected five-integration command passes 18/18: ordinal 2, scalar MarkTable 11,
  probability-simplex composition 3, and both legacy inhomogeneous exact-codec durable workflows.
  Targeted warning-denied Clippy, package no-default compilation, package doctests, affected-file
  Rustfmt, and diff whitespace checks pass.

## Typed-mark and inference stabilization checkpoint 90 — 2026-08-28

- `cargo +1.96.0 fmt --all --check` passes. `cargo +1.96.0 clippy --locked --workspace
  --all-targets --all-features -- -D warnings` first passed in `25m08s`; after final retained-byte
  accounting changed production, the required final-state rerun passes in `21m01s` with no
  diagnostics.
- The final-state `cargo +1.96.0 check --locked --workspace --no-default-features` passes in `8.93s`.
  `cargo +1.96.0 test --locked --workspace --doc --all-features` passes for every workspace package
  with zero doctest failures after an `11.53s` final-state compile.
- The first strict-doc run found one existing `[0,1]` comment parsed as a broken intra-doc link.
  After formatting the range as code, `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked
  --workspace --all-features --no-deps` passes on the final state in `12.26s`. `git diff --check`
  passes.
- The full workspace integration/Nextest loop was not run because checkpoints 51/52 and the active
  instruction document and forbid retrying the macOS binary-verification stall. No phase-only
  feature matrix, benchmark, fuzz, memory tool, packaging, dependency audit, push, publication,
  deployment, or history rewrite was run.

## Replicated-patient conditional CellViT marks checkpoint 136 — 2026-08-29

- `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_replicated_conditional_multitype_mark_cli -- --nocapture` first failed on the absent command
  and passes in the final state. Its exact 5-by-3/15-by-3 grid oracle checks 1,936 total edges,
  114/30 same-type edges per separated/mixed pattern, and positive intervals for all three known
  group affinity shifts.
- `cargo +1.96.0 test --locked --package marklab --features cli --test
  durable_pymc_replicated_conditional_multitype_mark_project -- --nocapture` first failed on the
  absent project command and passes in the final state. Two fresh processes prove miss, backend-
  disabled hit, byte identity, and one durable execution. The combined final direct/durable command
  passes both targets in 17.33 and 26.05 seconds.
- `target/pymc-venv/bin/python -m unittest
  tests.python.test_marklab_cellvit_cptac_results_adapter` passes 9/9. Both changed Python files pass
  `py_compile`. The pinned Mac-mini CellViT environment re-audited 366 slides, 178 patients, and
  1,542,389 cells in 115.03 seconds at 665,829,376-byte maximum RSS. The final 8-patient/16-slide,
  8,192-cell input digest is
  `ebecd5a5bf47261bdaeb965255b5b5bfd3f74f6bebf01ca5c5e8447856a7e9c6`.
- The first 30,130-cell real attempt exceeded the explicit 1,200-second worker ceiling; total elapsed
  and maximum RSS were 1,208.22 seconds and 898,449,408 bytes. The bounded 8,192-cell target-0.95
  fit completed in 548.66 seconds but retained 76 divergences. Its backend-disabled replay is exact
  with one ledger row. The sole target-0.99/depth-13 capacity retry completed in 1,201.20 seconds at
  731,693,056-byte maximum RSS with R-hat 1.00729, bulk/tail ESS 630.90/858.52, E-BFMI 0.69379,
  and zero divergences/depth hits. A formatted-source runtime-identity rerun completed in 1,197.25
  seconds at 677,183,488-byte maximum RSS and reproduced that result byte-for-byte. Its fresh
  backend-disabled hit is byte-identical with one final v2 ledger row; result SHA-256 is
  `15a3b52b498280f0baf80e0f53b2fa9b33b059936ceffec50ceca77237bcc8e4`.
- `cargo +1.96.0 clippy --locked --package marklab --features cli --bin marklab --test
  bayes_replicated_conditional_multitype_mark_cli --test
  durable_pymc_replicated_conditional_multitype_mark_project -- -D warnings`, package no-default
  compilation, affected-file Rustfmt checks, and `git diff --check` pass. `ssh mini 'cd
  ...v25-replicated-conditional-multitype && shasum -a 256 -c bundle_sha256.txt'` verifies every
  sealed file. Broad workspace gates and the documented full-integration/Nextest loop were not
  rerun after checkpoint 135; no feature matrix, benchmark, fuzzing, packaging, dependency audit,
  push, publication, deployment, or history rewrite ran.

## Replicated conditional-mark promotion checkpoint 137 — 2026-08-29

- `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_replicated_conditional_multitype_mark_sbc_cli -- --nocapture` first failed on the absent
  command. The initial implemented 2x1,000-draw capacity produced 15 complete dispositions and five
  exact R-hat-only failures (1.01025–1.01312). With the sole capacity increase to 1,000 warmup and
  1,500 retained draws, the final formatted-state run passes 1/1 in 74.73 seconds with 20/20 exact
  hierarchy refits, ten rank/coverage families, 729 states per six-site/three-type pattern, and the
  independent `6*ln(3)` zero-potential partition oracle.
- `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_replicated_conditional_multitype_mark_cli --test
  durable_pymc_replicated_conditional_multitype_mark_project --test
  bayes_replicated_conditional_multitype_mark_agreement_cli --test
  bayes_replicated_conditional_multitype_mark_sensitivity_cli -- --nocapture` passes all four
  integrations. Individual test times are 15.49, 26.07, 45.27, and 45.00 seconds. The durable test
  retains fresh-process miss/backend-disabled hit/one-ledger proof; agreement covers all 101
  quantities; sensitivity retains all four fixed scenarios.
- `cargo +1.96.0 clippy --locked --package marklab --features cli --bin marklab --test
  bayes_replicated_conditional_multitype_mark_sbc_cli -- -D warnings`, `cargo +1.96.0 check
  --locked --package marklab --no-default-features`, package doctests, worker `py_compile`,
  affected-file Rustfmt, and `git diff --check` pass.
- At the four-workflow major checkpoint, `cargo +1.96.0 fmt --all --check`, `cargo +1.96.0 clippy
  --locked --workspace --all-targets --all-features -- -D warnings`, `cargo +1.96.0 check --locked
  --workspace --no-default-features`, `cargo +1.96.0 test --locked --workspace --doc --all-features`,
  and `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace --all-features --no-deps`
  all pass. The forbidden macOS Nextest/full-integration loader loop was not retried; no feature
  matrix, benchmark, fuzzing, memory tool, packaging, dependency audit, push, publication,
  deployment, or history rewrite ran.

## Replicated exact-window multitype LGCP checkpoint 138 — 2026-08-29

- `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_replicated_arbitrary_window_multitype_lgcp_fit_cli -- --nocapture` first failed on the absent
  command. The final 2/2 run passes in 13.28 seconds: one exact known type-specific shift recovery,
  one incomplete-type rejection, and one one-short 192-row ceiling rejection.
- `cargo +1.96.0 test --locked --package marklab --features cli --test
  durable_pymc_replicated_arbitrary_window_multitype_lgcp_project -- --nocapture` first failed on the
  absent project subcommand and passes in 22.31 seconds with miss, fresh backend-disabled hit, byte
  identity, one execution row, and exact replay validation of every identity, prior/resource field,
  type pair, hierarchy effect, node posterior, and predictive row.
- `target/pymc-venv/bin/python -m unittest
  tests.python.test_marklab_cellvit_cptac_results_adapter` passes 10/10. The new oracle proves 48
  complete node-type rows, four retained common-type cells, excluded foreign hard types, one typed
  event digest, and exact area conservation. Both changed workers pass `py_compile`.
- The first remote adapter command used the verification run as `--inference-root` and failed before
  output because manifests were absent; the second found the exact inference source but lacked the
  frozen CellViT source on `PYTHONPATH`. Each failed `.part` directory was moved to Trash. The final
  pinned Python-3.9/frozen-source command re-audited 366 slides, 178 patients, and 1,542,389 cells in
  103.36 seconds at 643,956,736-byte maximum RSS and produced the 666-row input SHA-256
  `c862ce9790274f1f099f040716de864b5185cc3084004ccef7693376a95f791f`.
- `/usr/bin/time -l target/debug/marklab project
  replicated-arbitrary-window-multitype-lgcp ...` completes the identity-final miss in 256.49
  seconds at 641,417,216-byte maximum RSS. The typed result is complete at R-hat 1.00530,
  bulk/tail ESS 1172.17/1634.16, E-BFMI 0.95063, and zero divergences/depth hits. A second command
  with `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1` reports a hit; `cmp` passes and the ledger has
  one row. `/usr/bin/shasum -a 256 -c bundle_sha256.txt` verifies every v28 bundle file.
- Targeted warning-denied Clippy over the binary and both integrations, package no-default
  compilation, final debug build, affected-file Rustfmt, and `git diff --check` pass. Broad workspace
  gates were not repeated after checkpoint 137; no full integration/Nextest loop, feature matrix,
  benchmark, fuzzing, packaging, dependency audit, push, publication, deployment, or history rewrite
  ran.

## Replicated multitype LGCP promotion checkpoint 139 — 2026-08-29

- `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_replicated_arbitrary_window_multitype_lgcp_sensitivity_cli -- --nocapture` first exercised
  2x1,500 draws and retained one divergence in each of `patient_scale_double` and
  `field_amplitude_double`. The single declared 2x2,000-draw capacity increase retained one exact
  divergence only in `field_length_double`; the final test passes by requiring that truthful
  aggregate `nonconverged` disposition, all eight finite fits, and the planted type-A shift.
- `/usr/bin/time -l target/debug/marklab bayes
  replicated-arbitrary-window-multitype-lgcp-sensitivity ...` completes the real 8-patient,
  16-slide, 666-row grid in 407.87 seconds at 1,204,912,128-byte maximum RSS. All eight scenarios
  are complete with zero divergences/depth hits. The maximum type-group/pair shifts are
  0.1231/0.1264 baseline SD, while slide/local-expected shifts reach 3.4168/8.5574 SD. The 3,563,381-
  byte output SHA-256 is `38586b490710c2f7529093f51fe9fef7928cc231a6c7820e05870588b455ccdc`.
- `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_replicated_arbitrary_window_multitype_lgcp_prior_calibration_cli -- --nocapture` first failed
  on the absent command. Its first implemented run found an empirical-covariance type/node axis
  interleave (0.214 amplitude-squared RMSE); after correcting that production boundary, the exact
  deterministic test passes all five moment, centered-covariance, and Poisson checks.
- `/usr/bin/time -l target/debug/marklab bayes
  replicated-arbitrary-window-multitype-lgcp-prior-calibration ...` completes 4,096 real-geometry
  draws in the identity-final NumPy run in 1.47 seconds at 393,920,512-byte maximum RSS. The declared estimate/ceiling are
  673,972,224/1,073,741,824 bytes under exact NumPy 2.4.6 identity. All moment checks pass, covariance RMSE is 0.00752 amplitude
  squared, and 2,727,936 Poisson residuals have mean -0.00119 and second moment 0.99993. Output
  SHA-256 is `956211a290cf4a8d781ef2be4ac61dc59d0cba4085c503e885f90e0dc14b3e6c`.
- The combined focused command passes agreement and sensitivity in 22.29 and 51.42 seconds; after
  correcting the calibration backend identity from NumPyro to the NumPy generator actually used,
  its identity-final focused test passes in 3.37 seconds. Focused warning-denied Clippy, package no-default CLI
  compilation, worker `py_compile`, affected-file Rustfmt, and `git diff --check` pass. `ssh mini
  'cd ...v30-replicated-multitype-lgcp-promotion && shasum -a 256 -c bundle_sha256.txt'` verifies
  all 14 sealed files, including the durable fit and three promotion results.
- At this three-workflow stabilization boundary, `cargo +1.96.0 fmt --all --check`, `cargo +1.96.0
  clippy --locked --workspace --all-targets --all-features -- -D warnings`, `cargo +1.96.0 check
  --locked --workspace --no-default-features`, `cargo +1.96.0 test --locked --workspace --doc
  --all-features`, and `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace
  --all-features --no-deps` pass. The documented macOS Nextest/full-integration loader loop was not
  run. No feature matrix, benchmark, fuzzing, memory tool, packaging, dependency audit, push,
  publication, deployment, or history rewrite ran.

## Replicated multitype inferred-kernel checkpoint 140 — 2026-08-29

- `cargo +1.96.0 test --locked --package marklab --features cli --test
  bayes_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_cli` first failed on the absent
  command and passes in the final state in 15.88 seconds. The exact planted-shift oracle retains
  eight patients, 16 slides, three types, 64 nodes, 192 node/type rows, positive finite inferred
  scales, all node/type posteriors, and zero divergences/depth hits.
- `cargo +1.96.0 test --locked --package marklab --features cli --test
  durable_pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_project` first failed on
  the absent project command and passes in 24.89 seconds with miss, fresh backend-disabled hit, byte
  identity, one ledger row, exact source/backend/worker/request/kernel/resource identity, and replay
  validation of every type, hierarchy, node, and predictive row.
- `/usr/bin/time -l target/debug/marklab project
  replicated-arbitrary-window-multitype-lgcp-inferred-kernel ...` completes the real durable miss in
  625.25 seconds at 1,284,538,368-byte maximum RSS under 2,664,000 draw-node-type and 45,750 kernel-
  cube work. R-hat is 1.00782, bulk/tail ESS 479.79/727.01, E-BFMI 0.67294, with zero divergences and
  depth hits. Shared amplitude/length are 1.03498 [0.95688, 1.11971] and 286.13 [236.41, 337.82]
  micrometres; all absolute and pairwise group intervals span zero.
- A fresh `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1` command reports `cache_status=hit`; `cmp`
  passes and `wc -l .../executions.jsonl` is one. Result SHA-256 is
  `4e437daefdc20133f674877515d911b918f6c7d27380cb3c69d51835771c7e88`. `ssh mini 'cd
  ...v31-replicated-multitype-lgcp-inferred-kernel && shasum -a 256 -c bundle_sha256.txt'` verifies
  all 20 files.
- Focused warning-denied Clippy over the binary and both integrations, package no-default CLI
  compilation, worker `py_compile`, affected-file Rustfmt, and `git diff --check` pass. Broad
  workspace gates were not repeated one milestone after checkpoint 139; no Nextest/full-integration
  loop, feature matrix, benchmark, fuzzing, packaging, dependency audit, push, publication,
  deployment, or history rewrite ran.

## Replicated multitype inferred-kernel agreement checkpoint 141 — 2026-08-29

- `cargo +1.96.0 test --locked --package marklab --features cli --test bayes_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_agreement_cli` first failed on the absent command and passes in 32.17 seconds. It creates one typed PyMC baseline, then validates that exact result and starts only NumPyro; all seven comparison categories pass.
- `/usr/bin/time -l target/debug/marklab bayes replicated-arbitrary-window-multitype-lgcp-inferred-kernel-agreement ...` starts only the real NumPyro fit and completes in 438.12 seconds at 1,179,484,160-byte maximum RSS. R-hat is 1.00921, bulk/tail ESS 450.57/719.58, E-BFMI 0.67400, with zero divergences and depth hits.
- All 1,424 real quantities pass the fixed maximum-standardized-difference 5.0 and minimum parameter/field tolerance 0.12/0.2 rules with interval overlap. Kernel and expected-count maxima are 0.8713 and 0.9324. Output SHA-256 is `2295cd21b1a836627f1fa39c9082195686ce304865f484f429c533b913ead589`.
- Focused warning-denied Clippy, package no-default CLI compilation, worker `py_compile`, affected-file Rustfmt, and `git diff --check` pass. `ssh mini 'cd ...v32-replicated-multitype-lgcp-inferred-agreement && shasum -a 256 -c bundle_sha256.txt'` verifies all 21 files. Broad workspace gates were not repeated two milestones after checkpoint 139; no Nextest/full-integration loop, feature matrix, benchmark, fuzzing, packaging, dependency audit, push, publication, deployment, or history rewrite ran.

## Replicated multitype inferred-kernel calibration checkpoint 142 — 2026-08-29

- `cargo +1.96.0 test --locked --package marklab --features cli --test bayes_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_prior_calibration_cli` first fails on the absent command and passes on the final deterministic small oracle.
- `/usr/bin/time -l target/debug/marklab bayes replicated-arbitrary-window-multitype-lgcp-inferred-kernel-prior-calibration ...` completes 4,096 exact real-geometry draws in 1.75 seconds at 565,854,208-byte maximum RSS. Exact resource identities are 2,727,936 node-type and 187,392,000 kernel-cube work with a 717,619,200-byte conservative estimate.
- Seven prior-moment checks, recovered whitened moments (-0.00111/1.00001), slide centering (8.44e-15 maximum), and 2,727,936 Poisson residuals (-0.000256/0.99918 first/second moments) pass. Output SHA-256 is `71445bb843aa9ab15ebc0e7a234ad1ebf9861bbbbeae7239e799b9379b23e0fe`.
- The final combined direct/durable/agreement/calibration command passes in 13.80/24.88/32.87/1.30 seconds. Focused warning-denied Clippy, package no-default CLI compilation, worker `py_compile`, affected-file Rustfmt, and `git diff --check` pass. `ssh mini 'cd ...v33-replicated-multitype-lgcp-inferred-promotion && shasum -a 256 -c bundle_sha256.txt'` verifies all 22 files.
- At this three-workflow stabilization boundary, `cargo +1.96.0 fmt --all --check`, `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`, `cargo +1.96.0 check --locked --workspace --no-default-features`, `cargo +1.96.0 test --locked --workspace --doc --all-features`, and `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace --all-features --no-deps` pass. The documented macOS Nextest/full-integration loader loop was not run; no feature matrix, benchmark, fuzzing, packaging, dependency audit, push, publication, deployment, or history rewrite ran.

## Sparse radius-heat coordinate stability checkpoint 143 — 2026-08-29

- `cargo +1.96 test --locked --package marklab --features cli --test
  graph_sparse_radius_heat_stability_cli` first fails on the absent direct subcommand and passes in
  the final state. Its five-node zero-jitter oracle requires four exact graph-digest/signal
  identities and rejects a one-short aggregate candidate ceiling before execution.
- `cargo +1.96 test --locked --package marklab --features cli --test
  durable_sparse_radius_heat_stability_project` first fails on the absent project subcommand and
  passes in the final state with miss, fresh-process hit, byte identity, and one ledger row.
  `cargo +1.96 test --locked --package marklab-graph` passes. The final affected integration command
  over legacy/new direct and durable sparse-heat targets passes 5/5 assertions across four targets.
- The remote durable miss over the exact admitted 2,000-cell input completes in 8.94 seconds at
  15,482,880-byte maximum RSS. Sixteen fixed 1-micrometre perturbations execute 955,108 candidate
  and 11,378,068 matrix-vector work; maximum relative L2 change is 0.009872 and edge counts span
  24,705–24,775. A second remote process reports `cache_status=hit`; `cmp` passes and
  `executions.jsonl` remains one row. `ssh mini 'cd
  /Volumes/1TB/marklab/runs/results-cellvit-categorical-v34-sparse-heat-stability && shasum -a 256
  -c bundle_sha256.txt'` verifies every sealed file.
- Affected-file Rustfmt passes. Broad workspace gates were not repeated one workflow after
  checkpoint 142, and the documented Nextest/full-integration loader loop was not run.

## Sparse radius diffusion wavelet checkpoint 144 — 2026-08-29

- `cargo +1.96 test --locked --package marklab --features cli --test
  graph_sparse_radius_diffusion_wavelet_cli` first fails on the absent direct command. After the
  initial implementation passes, the real durable artifact exceeds 1 MiB; the strengthened test
  then fails while redundant `filtered_signal` fields remain. The final sufficient detail/coarse
  representation passes dense spectral agreement at every scale, exact telescoping reconstruction,
  absent redundant fields, and one-short aggregate work rejection.
- `cargo +1.96 test --locked --package marklab --features cli --test
  durable_sparse_radius_diffusion_wavelet_project` first fails on the absent project command and
  passes after wiring; it passes again after the sufficient-result schema change with miss,
  fresh-process hit, byte identity, and one ledger row. `cargo +1.96 test --locked --package
  marklab-graph` passes. The final focused command over sparse heat, exact diffusion wavelet, and
  direct/durable sparse diffusion wavelet passes 5/5 tests across four integration targets.
- The retained first real attempt fails exactly with `encoded 1409419 inline bytes, exceeding limit
  1048576` and leaves zero ledger rows. The final remote miss completes in 10.42 seconds at
  29,278,208-byte maximum RSS; its artifact is 710,433 bytes, total candidate/matrix-vector work is
  224,344/2,523,990, and reconstruction error is 3.33e-16. A second process reports a hit, `cmp`
  passes, and the ledger remains one row. `ssh mini 'cd /Volumes/1TB/marklab/runs/results-cellvit-
  categorical-v35-sparse-diffusion-wavelet && shasum -a 256 -c bundle_sha256.txt'` verifies every
  sealed file, including the rejected-boundary stderr.
- Affected-file Rustfmt and `git diff --check` pass. Broad workspace gates were not repeated two
  workflows after checkpoint 142; the documented Nextest/full-integration loop was not run.

## Sparse radius scattering checkpoint 145 — 2026-08-29

- `cargo +1.96 test --locked --package marklab --features cli --test
  graph_sparse_radius_scattering_cli` first fails on the absent direct command and passes in the
  final state. Its independent dense spectral oracle checks every first/second-order coefficient,
  exact eight-application plan, and one-short aggregate candidate rejection.
- `cargo +1.96 test --locked --package marklab --features cli --test
  durable_sparse_radius_scattering_project` first fails on the absent project command and passes in
  the final state with miss, fresh-process hit, byte identity, and one ledger row. `cargo +1.96 test
  --locked --package marklab-graph` passes. The focused command over exact scattering, sparse
  wavelet, and direct/durable sparse scattering passes 4/4 integrations; after the Clippy cleanup,
  the final direct/durable pair passes again.
- The first workspace Clippy attempt reports only `clippy::needless-range-loop` in the new
  first-modulus propagation. After converting it to the equivalent enumerated iterator and
  revalidating focused behavior, `cargo +1.96 clippy --locked --workspace --all-targets
  --all-features -- -D warnings` passes in 48.77 seconds. `cargo +1.96 fmt --all --check`, `cargo
  +1.96 check --locked --workspace --no-default-features` (4.66 seconds), `cargo +1.96 test --locked
  --workspace --doc --all-features` (11.57 seconds), and `RUSTDOCFLAGS='-D warnings' cargo +1.96 doc
  --locked --workspace --all-features --no-deps` (4.31 seconds) pass.
- The pre-final-runtime remote miss completes in 7.60 seconds at 18,448,384-byte maximum RSS. The
  identity-final v37 miss completes in 10.64 seconds at 20,348,928-byte maximum RSS and is
  scientifically byte-identical. Its fresh-process hit passes `cmp` with one ledger row. Result
  SHA-256 is `d1c2a01c709a20528665d21a7d8d45d450f8dc403ae1a06924990514dfab71e0`.
  `ssh mini 'cd /Volumes/1TB/marklab/runs/results-cellvit-categorical-v37-sparse-scattering-final
  && shasum -a 256 -c bundle_sha256.txt'` verifies every sealed file.
- Final affected-file Rustfmt and `git diff --check` pass. The documented macOS Nextest/full-
  integration loop was not run; no feature matrix, benchmark, fuzzing, packaging, dependency audit,
  push, publication, deployment, or history rewrite ran.

## Witness-persistence coordinate stability checkpoint 146 — 2026-08-29

- `cargo +1.96 test --locked --package marklab --features cli --test
  topology_witness_persistence_stability_cli` first fails on the absent command, then its first
  compile rejects an unowned direct `sha2` import. Reusing `marklab-topology`'s canonical SHA-256
  owner preserves the dependency boundary; the final test passes exact two-replicate zero-jitter
  identity and one-short aggregate point-work rejection.
- `cargo +1.96 test --locked --package marklab --features cli --test
  durable_gudhi_witness_stability_project` first fails on the absent project command and passes in
  the final state with miss, fresh-process `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1` hit, byte
  identity, and one ledger row. `cargo +1.96 test --locked --package marklab-topology` passes. The
  final focused command over legacy/new direct and durable witness targets passes 4/4 integrations.
- The first Mac-mini standalone execution fails before project creation with missing exact pinned
  path `/Users/user/Bench/gsc-marklab/workers/python/uv.lock`; its stderr is retained. The unchanged
  SHA-256-identified input executes through the existing local pinned GUDHI environment: all 17
  backend runs complete in 31.34 seconds at 48,824,320-byte maximum RSS, 34,000 point-work, and an
  8,500,000-simplex aggregate ceiling. The fixed diagnostic is unstable at 0.890625 minimum landmark
  overlap and 43,950.386-square-micrometre maximum total-persistence change.
- A fresh backend-disabled process reports `cache_status=hit`; `cmp` passes and the ledger remains
  one row. Result SHA-256 is `883d07beed9916a250b41ad57899ecbba31940ae5c0b6096646677fa7cea0326`.
  `ssh mini 'cd /Volumes/1TB/marklab/runs/results-cellvit-categorical-v39-witness-stability-final
  && shasum -a 256 -c bundle_sha256.txt'` verifies every sealed file. Affected-file Rustfmt and
  `git diff --check` pass; broad workspace gates were not repeated one workflow after checkpoint 145.

## Final CRC scientific analysis checkpoint 147 — 2026-08-29

- Red-first behavior evidence:
  `target/pymc-venv/bin/python -m unittest tests.python.test_crc_graph_topology_final`
  first failed because `marklab_crc_graph_topology_final.py` was absent; focused additions then
  failed on the absent CLI, nonspatial summary, held-out model, and patient-rank stability owners.
  `target/pymc-venv/bin/python -m unittest tests.python.test_crc_graph_topology_summary` first
  failed because its summary owner was absent. `target/pymc-venv/bin/python -m unittest
  tests.python.test_crc_final_science_bundle` first failed because the final sealer was absent.
- The final focused command
  `target/pymc-venv/bin/python -m unittest tests.python.test_crc_final_science_bundle
  tests.python.test_crc_graph_topology_final tests.python.test_crc_graph_topology_summary` passes
  9/9. `target/pymc-venv/bin/python -m py_compile
  workers/python/marklab_crc_graph_topology_final.py
  workers/python/marklab_crc_graph_topology_summary.py
  workers/python/marklab_crc_final_science_bundle.py` passes.
- `cargo +1.96.0 build --locked --features cli --bin marklab` passes in 33.82 seconds before the
  real project executions. No Rust production source changed after checkpoint 146, so there is no
  affected Rust package for warning-denied Clippy, no-default compilation, or docs.
- The exact admitted marks were copied read-only from
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v25-replicated-conditional-multitype/inputs/replicated_conditional_multitype_marks.csv`;
  local SHA-256 `ebecd5a5bf47261bdaeb965255b5b5bfd3f74f6bebf01ca5c5e8447856a7e9c6`
  matches the source. `prepare --marks ... --out target/science-crc-final/prepared-v1` admits eight
  patients, 16 slide patterns, and 512 cells per pattern without label-based selection.
- Six bounded parallel processes ran the existing commands `marklab project
  sparse-radius-scattering`, `marklab project witness-persistence`, and `marklab project
  witness-persistence-stability` over the declared requests. Fresh processes with
  `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1` return 80/80 graph and 64/64 topology hits; every
  result is byte-identical to its miss output and ledgers remain at 80 and 64 total rows.
- The allowlisted CellViT extraction ran on the Mac mini through
  `PYTHONPATH=/Users/user/Bench/CellViT-plus-plus
  /Users/user/Bench/CellViT-plus-plus/.venv-cellvit/bin/python ... extract-nonspatial` and verifies
  exact source rows/coordinates for all 16 1,280-dimensional graphs. Provenance SHA-256 is
  `b86420b55dfba1e72797e654d61fa49d6fa0570e4d65613ba09c058e7e163ce9`.
- Existing `marklab cohort energy` and `marklab cohort mmd --kernel linear --estimator unbiased`
  commands complete 999 patient-label permutations for the baseline, graph, topology, augmented,
  and final admitted blocks. Thirty-two fold-specific `marklab project region-retrieval` misses
  then replay as 32/32 backend-disabled hits with byte identity and one row per fold ledger. The
  first declared 64-visit retrieval bound fails before execution as expected because exact work is
  `(n(n-1)+n)*d`; the corrected explicit 10,000 ceiling is retained and remains far below 250M.
- `python3 marklab_crc_final_science_bundle.py --canonical
  /Volumes/1TB/marklab/runs/results-crc-spatial-fingerprint-v1 --graph
  /Volumes/1TB/marklab/runs/science-crc-final-01-work-v1 --outcome
  /Volumes/1TB/marklab/runs/crc-spatial-phenotype-outcome-01-v3 --out
  /Volumes/1TB/marklab/runs/science-crc-final-01` passes. The independent read-only verifier hashes
  all 517 manifest artifacts with zero errors. Manifest SHA-256 is
  `3bd77f27a3f06f2a59d703a6b0335a51d2ba1b55fda1b76aac10e36e29dd5bf9`.
- Checkpoint 145 remains the latest broad workspace stabilization. No Nextest/full-integration
  loop, workspace-wide test/Clippy/docs, feature matrix, benchmark, fuzzing, memory tool, packaging,
  dependency audit, push, publication, deployment, or history rewrite ran.

## Sparse component-basis checkpoint 148 — 2026-08-29

- Red-first evidence: `cargo +1.96.0 test --locked --package marklab --features cli --test
  graph_sparse_radius_basis_cli -- --nocapture` first failed because `sparse-radius-basis` was not a
  graph subcommand. `cargo +1.96.0 test --locked --package marklab --features cli --test
  durable_sparse_radius_basis_project -- --nocapture` then failed because the matching project
  command was absent. Both pass after the direct and durable paths were implemented. The focused
  typed-corruption unit test first failed because replay validation trusted the stored maximum
  residual; a follow-up red also exposed a pre-validation oversized-component allocation. Both pass
  after bounded derived residual, orthogonality, nullspace, and ordering validation.
- `cargo +1.96.0 test --locked --package marklab-graph` passes 5/5 unit tests and doctests. The
  affected integration command covering direct/durable basis, heat, diffusion-wavelet, and
  scattering passes all eight targets and nine tests, including the exact five-node path spectrum
  and fresh-process one-ledger basis replay.
- `cargo +1.96.0 clippy --locked --package marklab-graph --all-targets -- -D warnings` passes.
  `cargo +1.96.0 clippy --locked --package marklab --features cli --test
  graph_sparse_radius_basis_cli --test durable_sparse_radius_basis_project -- -D warnings` passes.
  `cargo +1.96.0 check --locked --package marklab-graph --no-default-features` passes.
  `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --package marklab-graph --no-deps` passes.
- The final runtime binary SHA-256 is
  `71f73265e6b299ab0b9c9a062c9fbe6f5562437f7723c8c890963f15643579e2`; the exact request SHA-256 is
  `d3a0b9a8d2b5663e87a1fc2cc284163b3264c0c4eecdbce3017d4a061606c2ad`. `/usr/bin/time -l
  target/debug/marklab project sparse-radius-basis ...` completes the identity-final real 2,000-node
  miss in 8.07 seconds at 28,524,544-byte maximum RSS. The result has 24,755 edges, 24 components,
  32 modes, maximum residual `3.3187388269244314e-8`, and orthogonality error
  `6.5503158452884236e-15`.
- A fresh `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1 target/debug/marklab project
  sparse-radius-basis ...` process reports `cache_status=hit`; `cmp` passes and the execution ledger
  remains one row. Miss and hit both hash to
  `31ab52bb440176ec6989380e28c5a143ec42e15d656ddd678d5c705acf4468a5`.
- Local and remote `shasum -a 256 -c SHA256SUMS` verify every file in
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v43-sparse-radius-basis-final`; its manifest
  hash is `63ba1b1279eb09476e50872e74d3c15e9d2affc38a0b82bee0c843ee0a931338`.
- Affected-file Rustfmt passes. `git diff --check` passes. Checkpoint 145 remains the latest broad
  workspace stabilization; no Nextest/full-workspace loop, feature matrix, benchmark, fuzzing,
  memory tool, packaging, dependency audit, push, publication, deployment, or history rewrite ran.

## Sparse Fourier signal-energy checkpoint 149 — 2026-08-29

- Red-first evidence: `cargo +1.96.0 test --locked --package marklab --features cli --test
  graph_sparse_radius_fourier_energy_cli -- --nocapture` first fails after one corrected test-only
  moved-path compile error because the graph subcommand is absent. `cargo +1.96.0 test --locked
  --package marklab --features cli --test durable_sparse_radius_fourier_energy_project --
  --nocapture` independently fails because the project subcommand is absent. Both pass after the
  direct and durable production paths are implemented.
- The final focused command covering direct/durable sparse basis and Fourier targets passes four
  integrations. The analytic path result agrees with constant/nonzero eigenmode energies 4/9 and
  total 13 within `1e-8`, and rejects planned projection work 14 against required 15 before basis
  execution. `cargo +1.96.0 test --locked --package marklab-graph` passes 6/6 unit tests and
  doctests, including typed all-zero and component-constant fraction states.
- `cargo +1.96.0 clippy --locked --package marklab-graph --all-targets -- -D warnings` passes.
  `cargo +1.96.0 clippy --locked --package marklab --features cli --test
  graph_sparse_radius_fourier_energy_cli --test durable_sparse_radius_fourier_energy_project -- -D
  warnings` passes. `cargo +1.96.0 check --locked --package marklab-graph --no-default-features`
  passes. `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --package marklab-graph --no-deps`
  passes.
- The exact request SHA-256 is
  `a1a04f155818981d1ec514970316c85f2ef061e7ecabc117c6b4b3b266cd8a61`; final runtime binary
  SHA-256 is `27f8ec740b9e92ca9ab45f755624c5749c19d4420252debe9bf5dccf47c1419f`.
  `/usr/bin/time -l target/debug/marklab project sparse-radius-fourier-energy ...` completes the
  real 2,000-node miss in 7.97 seconds at 25,739,264-byte maximum RSS. Its 3,063 actual projection
  visits are below the conservative 64,000 ceiling.
- A fresh `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1 target/debug/marklab project
  sparse-radius-fourier-energy ...` process reports `cache_status=hit`; `cmp` passes and the ledger
  remains one row. Both results hash to
  `e38bec64677b5fbc2ea82eda47dcec15b0ae4d3c7226ec55ef6b5e7076fc192d`.
- Local and remote `shasum -a 256 -c SHA256SUMS` verify every file in
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v44-sparse-fourier-energy-final`; its
  manifest hash is `5784d40fd46568ea0a92495bcba417efb8c3ba98cb77d0f66ef99be84d702b38`.
- Affected-file Rustfmt and `git diff --check` pass. Checkpoint 145 remains the latest broad
  stabilization; no Nextest/full-workspace loop, feature matrix, benchmark, fuzzing, memory tool,
  packaging, dependency audit, push, publication, deployment, or history rewrite ran.

## Patient-replicated sparse Fourier checkpoint 150 — 2026-08-29

- Red-first Rust evidence: the focused
  `pathology_component_count_can_retain_eight_nonzero_modes_above_sixty_four_total` test first fails
  with invalid controls under the 64-mode ceiling. It passes after the bounded ceiling becomes 128
  and returns 65 component-zero plus eight nonzero modes on 130 nodes; mode count is also bounded by
  node count.
- Red-first patient behavior evidence: `target/pymc-venv/bin/python -m unittest
  tests.python.test_crc_sparse_fourier_patient` first fails because
  `marklab_crc_sparse_fourier_patient.py` is absent. Preparation, fixed-mode feature extraction, and
  patient summary pass after implementation. A fourth bounded-execution test first fails because
  `execute_requests` is absent, then passes 40 fake misses and 40 byte-identical hits with two
  processes. The final focused Python suite passes 4/4; `python -m py_compile` passes.
- `cargo +1.96.0 test --locked --package marklab-graph` passes 7/7 unit tests and doctests. The
  final focused Rust command over direct/durable sparse basis and Fourier integrations passes all
  four targets. Warning-denied affected Clippy, graph no-default compilation, and affected docs pass.
- The real `prepare` command admits eight patients, 16 nested slides, five variants, and 80 exact
  requests. Independent component preflight performs 9,707,200 pair checks below 20,000,000;
  components span 28–70 and total modes 36–78, always retaining eight nonzero modes under 128.
- After the scheduled stabilization gates, runtime binary SHA-256 is
  `16c6e9e1ad435cdea2fe0c0f2e0c2701a34798451aaae0f65c1d76681bb2205b`.
  `/usr/bin/time -l ... execute --maximum-processes 6` completes 80 identity-final misses in 88.00
  seconds. The single backend-disabled replay command completes 80/80 hits in 78.74 seconds; every
  result is byte-identical and all 80 ledgers remain one row. No replay retry ran.
- The deterministic `summarize` command reports patient-unit stability and held-out inference.
  Fourier-only balanced accuracy is 0.50 with exact 70-assignment p=0.6286; adding Fourier to M0–M3
  changes balanced accuracy and retrieval by zero, with bootstrap increment interval
  `[-0.375, 0.375]`. Only coordinate stability passes; the frozen fusion gate rejects the block.
  Summary SHA-256 is `9dc6ad63f7aeb69a7954aab7b81bc2d0b820113bbdb20bd0701f2d229fda5857`.
- Local and remote verification rehash all 656 files in
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v46-patient-sparse-fourier-final`; manifest
  SHA-256 is `fb2b819f4936f6c5e848601618260303645f4139adff1e86df54543fee1cef59`.
  The sealed workflow hashes to `dc3c60323f3fa87ca59ce42bbe662c2f4f074383c4d93a95f11ac9cdb4e988bc`.
  Remote verification also proves all 80 ledger files have exactly one row.
- Major-checkpoint commands pass once:
  `cargo +1.96.0 fmt --all --check`;
  `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings`;
  `cargo +1.96.0 test --locked --workspace --doc --all-features`;
  `cargo +1.96.0 check --locked --workspace --no-default-features`; and
  `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace --all-features --no-deps`.
  The prohibited Nextest/full-integration loop was not run. No feature matrix, benchmark, fuzzing,
  memory tool, packaging, dependency audit, push, publication, deployment, or history rewrite ran.

## Witness bottleneck-stability checkpoint 151 — 2026-08-29

- Red-first direct evidence: `cargo +1.96.0 test --locked --package marklab --features cli --test
  topology_witness_bottleneck_stability_cli -- --nocapture` first fails because
  `witness-persistence-bottleneck-stability` is not a topology command. The durable target first
  fails for the corresponding absent project command. Both pass after the direct and durable paths
  are implemented.
- `target/gudhi-venv/bin/python -m unittest
  tests.python.test_gudhi_witness_bottleneck_worker` passes the analytic interval-distance and typed
  essential-mismatch oracles. `cargo +1.96.0 test --locked --package marklab-topology` passes. The
  final focused command over the old/new direct and durable witness integrations passes all six
  targets. Python compilation and the affected CLI build pass.
- Warning-denied topology all-target Clippy and root Clippy for the new integrations pass.
  Topology no-default compilation and warning-denied topology docs pass. Affected formatting passes.
  Checkpoint 150 already ran the latest broad workspace gates, so they are not repeated.
- The exact source/request/runtime/worker SHA-256 values are respectively
  `85935f75e6afeb5c8004925fd69e6e07e0d6057996f68b14e28cbc02decdc91a`,
  `ea23f548f64a12043a08911744a17805b9c067542ee79f8d120762b4dffd8556`,
  `af3722def3320834487f5375c2ddd220af19e3266e44d623b0c9f6b7b3776382`, and
  `15e1dae22362a25ed79f56b078d149ad620d69f9cc72cb0b2a3c774019bbb912`.
  The real miss completes in 31.35 seconds at 52,920,320-byte maximum RSS. Its 48 comparisons and
  7,660 intervals remain below 48 and 48,000,000, and its 18 total backends equal the declared cap.
- A fresh `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1 target/debug/marklab project
  witness-persistence-bottleneck-stability ...` process returns a hit. `cmp` passes; miss/hit both
  hash to `11818914ec96521516a94e03c08710d3a75ff2eba12df16f6b7568003f87d2ef` and the
  ledger remains one row. The result truthfully fails the 600-square-micrometre threshold at
  `7131.385451975762` and retains typed infinite essential mismatches.
- Local and remote `shasum -a 256 -c SHA256SUMS` verify every listed file in
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v48-witness-bottleneck-stability-final`;
  its manifest hash is `77018e4fd3c7f95ec68577cb6b689d9801bf12997aa5b71bd0007788625faa6c`.
  The prohibited macOS Nextest/full-integration loop and other broad or specialized gates were not
  run.

## SCIENCE-CRC-FINAL-01 canonical bundle checkpoint 152 — 2026-08-29

- Red-first evidence: `target/pymc-venv/bin/python -m unittest
  tests.python.test_crc_final_science_bundle.CrcFinalScienceBundleTest.test_witness_bottleneck_addendum_requires_one_execution_and_equal_replay`
  first errors because `witness_bottleneck_addendum` is absent. After the narrow sealer connection,
  `target/pymc-venv/bin/python -m unittest tests.python.test_crc_final_science_bundle` passes 2/2,
  including rejection of a second ledger row. Python compilation passes.
- The real sealer runs once on the Mac mini with the previously sealed canonical, graph/topology,
  outcome, and v48 bottleneck sources and writes only the new
  `/Volumes/1TB/marklab/runs/science-crc-final-01-v2` output. No M0--M7, external backend, graph,
  topology, retrieval, cohort, Bayesian, external-validation, or outcome analysis is rerun.
- An independent `jq` manifest expansion piped to `shasum -a 256 -c -` verifies all 529 listed
  artifacts. The directory has 530 files including its manifest. Manifest and interpretation hashes
  are `8c03b980b72c2423253e074fc874ce67fe37057707860998f4a27fde180e4e18` and
  `d9e8348c9834f6004f5d8656aaa5aeb316ff24d61eb945282cdc41a1555e9c39`.
- The copied bottleneck miss/hit compare byte-identically, hash to
  `11818914ec96521516a94e03c08710d3a75ff2eba12df16f6b7568003f87d2ef`, and retain one ledger row.
  The final interpretation records the 7,131.39-versus-600-square-micrometre failure, essential-
  count mismatch, one-specimen claim limit, exclusion from fusion, and unchanged patient conclusion.
- Focused formatting, whitespace, direct diff, and status checks pass. Existing broad stabilization
  evidence is not rerun; no Nextest/full integration, workspace-wide test/Clippy/docs, feature
  matrix, benchmark, fuzzing, memory, packaging, dependency, push, publication, deployment, or
  history-rewrite command runs.

## Patient witness-bottleneck checkpoint 153 — 2026-08-29

- Red-first behavior evidence: `target/pymc-venv/bin/python -m unittest
  tests.python.test_crc_witness_bottleneck_patient` first fails because the patient workflow is
  absent. Its end-to-end fake process oracle passes after implementation, proving prepare,
  two-process misses, backend-disabled hits, byte equality, one-row ledgers, patient nesting, exact
  group comparison, and unstable nonpromotion.
- The first real execution command fails before GUDHI with `bottleneck interval budget exceeds
  caller maximum`: the prepared 4,000,000 ceiling omitted the three homology dimensions from the
  conservative owner bound. A focused assertion first fails 4,000,000 versus 12,000,000; after the
  exact bound is fixed, the test passes. The failed partial output is retained.
- `target/pymc-venv/bin/python -m unittest tests.python.test_crc_witness_bottleneck_patient
  tests.python.test_crc_graph_topology_final` passes 7/7. Python compilation and
  `cargo +1.96.0 fmt --all --check` pass. No Rust package changed, so affected Clippy/no-default/docs
  are not applicable.
- Real preparation admits eight patients, 16 slides, 8,192 cells, four perturbations, 12 comparisons
  and 12,000,000 conservative intervals per slide. `/usr/bin/time -l ... execute ...
  --maximum-processes 6` completes in 42.48 seconds with 38,191,104-byte parent maximum RSS. The
  execution manifest reports 16 misses, 16 backend-disabled hits, byte identity, one row in every
  ledger, and exactly 96 miss-side backend executions under the declared cap.
- The deterministic patient summary reports 0/8 stable, 8/8 with an essential-count mismatch,
  maximum finite distance `18807.03660672011`, MSI-minus-MSS mean difference
  `2972.21834719212`, bootstrap interval `[-3738.5436356459386, 10085.458614861542]`, and exact
  70-assignment p=0.60. It retains `unstable_not_promoted` and no fusion.
- Local and remote `shasum -a 256 -c SHA256SUMS` verify all 136 listed files in
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v49-patient-witness-bottleneck-final`;
  137 total files include the manifest. `SHA256SUMS` hashes to
  `bf6cc998ce8709f766a5ef9c984a28defe8d09265922f16b218f14478ec46e14`.
  Whitespace and direct diff checks pass. No broad or specialized workspace gate runs.

## Multiclass soft pair-mixing checkpoint 154 — 2026-08-29

- Red-first evidence: `cargo +1.96.0 test --locked --package marklab --test
  soft_pair_mixing_workflow --no-default-features -- --nocapture` first fails solely because
  `soft_pair_mixing` and its typed node/config/limits do not exist after two test-fixture type errors
  are corrected. The exact target passes after implementation.
- The final focused command covering `soft_pair_mixing_workflow`,
  `soft_neighborhood_composition_workflow`, and `probability_simplex_composition_workflow` passes
  7/7 under no-default features. The new analytic oracle recovers 0.5 observed A↔B probability,
  0.125 null probability, enrichment 4, and 1/12 A→A null; a maximum of one pair visit rejects when
  two are required. Durable miss/hit equality and one execution row pass.
- `cargo +1.96.0 clippy --locked --package marklab --no-default-features --lib --test
  soft_pair_mixing_workflow -- -D warnings` passes. `cargo +1.96.0 check --locked --package marklab
  --no-default-features` and `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --package marklab
  --no-default-features --no-deps` pass. Affected formatting and whitespace checks pass.
- Read-only Mac mini inspection of the admitted representative 3,247-cell JSON confirms keys
  `bbox`, `cell_status`, `centroid`, `contour`, `edge_information`, `edge_position`, `offset_global`,
  `patch_coordinates`, `type`, and `type_prob`; only `type_prob` is probabilistic. The paired graph
  has tensors `x`, `positions`, and metadata keys `nuclei_types`/`wsi_metadata`. A complete ordered
  class probability vector is absent, so no real soft-multiclass run is claimed.
- No workspace-wide test/Clippy/docs, Nextest/full integration, feature matrix, benchmark, fuzzing,
  memory, packaging, dependency, push, publication, deployment, or history-rewrite command runs.

## SCIENCE-CRC-FINAL-01 patient-evidence seal checkpoint 155 — 2026-08-29

- Red-first evidence: `target/pymc-venv/bin/python -m unittest
  tests.python.test_crc_final_science_bundle.CrcFinalScienceBundleTest.test_patient_witness_addendum_revalidates_patient_replay_and_unstable_result`
  first errors because `patient_witness_bottleneck_addendum` is absent. After the narrow sealer
  connection, `target/pymc-venv/bin/python -m unittest tests.python.test_crc_final_science_bundle`
  passes 3/3, including deliberate hit-byte corruption rejection. Python compilation passes.
- The new validator passes against local
  `target/patient-witness-bottleneck-real-v1/bundle-v49`: eight patients, 16 nested slides, 16
  byte-identical backend-disabled hits, 16 one-row ledgers, and 96 miss-side backend executions.
- The real sealer runs once on the Mac mini with the previously sealed canonical, graph/topology,
  outcome, v48 one-specimen bottleneck, and v49 patient bottleneck sources. It writes only
  `/Volumes/1TB/marklab/runs/science-crc-final-01-v3`; no scientific analysis or external backend
  runs.
- An independent manifest expansion piped to `shasum -a 256 -c -` verifies all 666 artifacts and
  confirms 667 total files including the manifest. Manifest and interpretation SHA-256 values are
  `0948a7c49af75de447132dfbabcd3a8b4e702c14e0e997e42b6aef076c1117b4` and
  `058c4492f4f9225324d79d7519e3cd5abc6f5ff3637b996401b944b88ace2a4a`. Sixteen copied project
  ledgers are present, and the final interpretation records 16 verified backend-disabled hits,
  byte equality, one-row ledgers, 0/8 stability, 8/8 essential mismatch, and the null-compatible
  whole-patient effect and interval.
- Checkpoint 145 remains the requested broad workspace baseline. No workspace-wide test/Clippy/docs,
  Nextest/full integration, feature matrix, benchmark, fuzzing, memory, packaging, dependency,
  push, publication, deployment, or history-rewrite command runs.

## Patient hard-categorical pair checkpoint 156 — 2026-08-29

- Red-first CLI evidence: `cargo +1.96.0 test --locked --package marklab --features cli --test
  categorical_pair_project_cli -- --nocapture` first fails because `project categorical-pair` is
  absent, then passes cross-process miss/hit, byte equality, typed format, and one-ledger assertions.
  The hit process sets `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1`.
- Red-first patient evidence: `target/pymc-venv/bin/python -m unittest
  tests.python.test_crc_categorical_pair_patient` first fails because the patient workflow is
  absent. Its final eight-patient fake-process oracle passes preparation, 64 misses, 64 hits,
  deliberate structural endpoint unavailability, one-row ledgers, held-out increment, and patient
  Max-T. A real summary first exposes unequal specimen endpoint sets; the oracle is extended to
  reproduce that boundary, fails, and passes after complete endpoint intersection precedes patient
  reduction.
- The first real execution retains 52 result files and 12 failures across three windows. A fresh
  diagnostic reports `decoded categorical pair window perimeter does not match its cache-bound
  request`. The full high-precision 11-component window is added to the CLI regression and fails.
  After retaining exact canonical geometry/config digests and applying the existing finite
  comparison to redundant area/perimeter summaries, the exact regression passes.
- `/usr/bin/time -l ... execute --maximum-processes 6` completes the identity-final 64 misses in
  70.28 seconds at 33,456,128-byte maximum RSS. The single fresh backend-disabled replay completes
  64/64 hits in 64.47 seconds; all outputs are byte-identical and all ledgers remain one row. Binary
  SHA-256 is `f046d5aea6f0b09c7281ea3d91de9464cf501b1045af94e53cb61779be7087ce`.
- The deterministic summary retains 21/32 complete endpoints and 11 structural blockers. Pair-only
  balanced accuracy is 0.25 with exact 70-assignment p=0.8857. The M0–M3-plus-pair increment is
  -0.25 with interval [-0.625, 0.25]. Nested-slide median/q10 stability is 0.7619/0.4333, and minimum
  step-down Max-T adjusted p is 0.44. No endpoint, radius, subset, or threshold is tuned.
- `target/pymc-venv/bin/python -m unittest tests.python.test_crc_categorical_pair_patient
  tests.python.test_marklab_cellvit_cptac_results_adapter
  tests.python.test_crc_graph_topology_summary` passes 13/13; Python compilation passes. The focused
  Rust command over `categorical_pair_project_cli` and `categorical_pair_project_workflow` passes
  3/3. Warning-denied affected Clippy and root no-default compilation pass.
- Remote `shasum -a 256 -c SHA256SUMS` verifies all 1,238 listed files in the 1,239-file v50 bundle;
  `SHA256SUMS` hashes to
  `c71a28bff748f8d618c2672bfdbd913ffdcd7047d5045e9fab165611a4653e53`. Affected formatting and
  whitespace checks pass. No workspace-wide test/Clippy/docs, Nextest/full integration, feature
  matrix, benchmark, fuzzing, packaging, dependency, push, publication, deployment, or history
  rewrite runs.

## Real exact-window inhomogeneous K/L checkpoint 157 — 2026-08-29

- Red-first evidence: `cargo +1.96.0 test --locked --package marklab --features cli --test
  inhomogeneous_spatial_project_cli -- --nocapture` first fails because `project
  inhomogeneous-spatial` is absent. It passes miss/fresh backend-disabled hit, byte equality, typed
  result, and one-ledger assertions after the narrow CLI is implemented.
- The first real command fails after computation with `output encoding failed: fixed intensity grid
  row is inconsistent`. The CLI oracle is changed to the real high-coordinate rectangle and 16x16
  grid; it reproduces the failure. `build_grid` and `validate_fixed_grid` used different floating-
  operation order for cell centres. After the validator uses the builder's exact operations, the
  same oracle passes without tolerance or schema changes.
- `/usr/bin/time -l target/debug/marklab project inhomogeneous-spatial ...` completes the identity-
  final real miss in 6.26 seconds at 22,118,400-byte maximum RSS. The result performs 5,641,561
  intensity evaluations, 155,510 total pair visits, and 14,312 null draws under 100,000,000,
  10,000,000, and 1,000,000 ceilings. A fresh
  `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1` process reports a hit; `cmp` passes, the ledger
  remains one row, and both results hash to
  `26861081dda3ee587d20aa0f9fe79056cc1676b46cbfeaebdb981fc2918654d5`.
- The real result retains L-minus-r -18.686/+16.992/+22.135 micrometres, 37/256 grid probes,
  fixed-grid mass 205.30, intensity range 4.36e-9 to 4.22e12 per square micrometre, and the minimum
  attainable 19-draw p=0.05. It is recorded as unreliable scale/capacity evidence, not interaction.
- The focused command over `inhomogeneous_spatial_project_cli`,
  `inhomogeneous_spatial_project_workflow`, and `inhomogeneous_spatial_typed_workflow` passes 6/6.
  Warning-denied affected Clippy and clean root no-default compilation pass. Remote
  `shasum -a 256 -c SHA256SUMS` verifies all 16 listed files in the 17-file v51 bundle;
  `SHA256SUMS` hashes to
  `5e2af34ed49a52d1a044ae8f75442ad622f9477c0e0b311c417bbbdfad00cd74`. Affected formatting and
  whitespace checks pass. No workspace-wide test/Clippy/docs, Nextest/full integration, feature
  matrix, benchmark, fuzzing, packaging, dependency, push, publication, deployment, or history
  rewrite runs.

## SCIENCE-CRC-FINAL-01 final seal checkpoint 158 — 2026-08-29

- Red-first: `target/pymc-venv/bin/python -m unittest
  tests.python.test_crc_final_science_bundle.CrcFinalScienceBundleTest.test_categorical_pair_addendum_revalidates_patient_replay_and_nonpromotion`
  first errors with `AttributeError` because `categorical_pair_addendum` is absent. After the narrow
  validator is implemented, the test passes and deliberate hit corruption is rejected.
- `target/pymc-venv/bin/python -m unittest tests.python.test_crc_final_science_bundle
  tests.python.test_crc_categorical_pair_patient` passes 5/5. `target/pymc-venv/bin/python -m
  py_compile workers/python/marklab_crc_final_science_bundle.py
  workers/python/marklab_crc_categorical_pair_patient.py` passes.
- The sealer runs once on `ssh mini` with the existing canonical, graph/topology, outcome, v48
  bottleneck, v49 patient-bottleneck, and v50 patient hard-pair sources and writes only
  `/Volumes/1TB/marklab/runs/science-crc-final-01-v4`. No scientific workflow or external backend
  reruns. The final validator code then passes read-only against the remote v50 source and reports
  eight patients, 16 patterns, 64 verified hits, and `unstable_nonincremental_not_added`.
- Independent `jq` manifest expansion piped to `shasum -a 256 -c -` verifies all 1,905 artifacts;
  1,906 files include the manifest. Manifest and interpretation SHA-256 values are
  `1fe3dbeec3de956f2cdd6a9e67f8ccd983ae26301bfe44383fbb42a525b4f2b2` and
  `34f29d358deeac520facfd0cd644fc800461c2af049e89424d0d053ed35023ed`.
  All 64 copied pair ledgers contain one row; the interpretation retains 64 verified misses and 64
  backend-disabled byte-identical hits, the -0.25 held-out increment with interval [-0.625, 0.25],
  and exclusion from fusion.
- Affected formatting, whitespace, direct diff, and status checks pass. Checkpoint 145 remains the
  broad workspace baseline; no workspace-wide/Nextest loop, Bayesian backend, feature matrix,
  benchmark, fuzzing, packaging, dependency, push, publication, deployment, or history rewrite
  runs.

## Real exact-window inhomogeneous pair-correlation checkpoint 159 — 2026-08-29

- Red-first: `cargo +1.96.0 test --locked --package marklab --features cli --test
  inhomogeneous_pair_correlation_project_cli -- --nocapture` fails with `unrecognized subcommand
  'inhomogeneous-pair-correlation'`. It passes after the narrow project command is implemented.
- The final focused command over `inhomogeneous_pair_correlation_project_cli`,
  `inhomogeneous_pair_correlation_project_workflow`, and
  `inhomogeneous_pair_correlation_typed_workflow` passes 4/4, including the independent Python
  value oracle, zero-support state, one-short work limits, cache invalidation, fresh-process
  backend-disabled replay, byte equality, and one ledger row.
- `/usr/bin/time -l target/debug/marklab project inhomogeneous-pair-correlation ...` completes the
  fixed real miss in 8.07 seconds at 22,790,144-byte maximum RSS. A fresh
  `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1` process reports a hit in 5.66 seconds; `cmp`
  passes, the ledger remains one row, and both outputs hash to
  `46539d2263b16abf0dd88be2b8e54961788a03be51b9f0bf1a1a261ee0b98d3a`.
- Exact-float decoding retains g 0.002930/1.215872/0.898092, 37/256 probes, grid mass 205.30,
  intensity range 4.36e-9 to 4.22e12 per square micrometre, 5,641,561 intensity evaluations,
  155,827 total pair visits, 14,312 null draws, and minimum-resolution p=0.05. No scale, grid,
  bandwidth, subset, or threshold is tuned.
- `cargo +1.96.0 clippy --locked --package marklab --features cli --lib --bin marklab --test
  inhomogeneous_pair_correlation_project_cli --test
  inhomogeneous_pair_correlation_project_workflow --test
  inhomogeneous_pair_correlation_typed_workflow -- -D warnings` passes. `cargo +1.96.0 check
  --locked --package marklab --no-default-features` passes.
- The first two remote hash commands fail after copying because the non-login SSH PATH cannot find
  `shasum`, then cannot find `find`, `wc`, or `tr`. The corrected absolute-path command uses
  `/usr/bin/find`, `/usr/bin/sort`, `/usr/bin/shasum`, `/usr/bin/wc`, `/usr/bin/tr`,
  `/usr/bin/awk`, and `/usr/bin/cmp`; it verifies all 11 listed artifacts plus `SHA256SUMS`, one
  ledger row, and byte-identical replay. Manifest SHA-256 is
  `1a3a9b13110d3d19df91f8a2960410fec3d0d05de1d05cb3a40e495e84b6bc63`.
- Affected formatting, whitespace, direct diff, and status checks pass. No workspace-wide/Nextest
  loop, full feature matrix, benchmark, fuzzing, packaging, dependency, push, publication,
  deployment, or history rewrite runs.

## Durable real categorical cross-g checkpoint 160 — 2026-08-29

- Red-first: `cargo +1.96.0 test --locked --package marklab --features cli --test
  categorical_cross_pair_correlation_project_cli -- --nocapture` fails with `unrecognized
  subcommand 'categorical-cross-pair-correlation'`. It passes after the narrow command and concrete
  shared adapter extraction.
- The final focused command over `categorical_cross_pair_correlation_project_cli`,
  `categorical_cross_pair_correlation_typed_workflow`, and `categorical_pair_project_cli` passes
  7/7. It covers the independent Python direct-pair oracle, directional control, unknown/support/
  one-short boundaries, durable cache invalidation, both fresh-process CLIs, high-precision window
  replay, and the existing categorical-pair regression.
- `/usr/bin/time -l target/debug/marklab project categorical-cross-pair-correlation ...` completes
  the real miss in 5.96 seconds at 28,868,608-byte maximum RSS. A fresh
  `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1` process reports a hit in 5.67 seconds. `cmp`
  passes, the ledger remains one row, and both outputs hash to
  `f36687f80cd3570300f1fc7416a4bf00807e8c74308fe96c765b4874c9fa2634`.
- A new `categorical-pair` run with the exact v50 request produces byte-identical output to the
  sealed v50 artifact; both hash to
  `4045fdbfffc0579e74ef009842eccbabf5a20a2bc13f7356cd31d9ea77ad46aa`.
- Warning-denied affected Clippy passes for the library, binary, new CLI/oracle tests, and existing
  categorical-pair CLI. `cargo +1.96.0 check --locked --package marklab --no-default-features`
  passes. The initial combined command crosses the 30-second yield after its tests finish; process
  inspection confirms it has exited, and the exact Clippy and no-default commands are then captured
  separately as successful incremental invocations.
- Absolute-path remote hashing verifies all 23 listed artifacts plus `SHA256SUMS`, both one-row
  ledgers, byte-identical cross-g replay, and byte-identical categorical-pair parity. Manifest
  SHA-256 is `1e4a8d0186ce76f5daa853f3fcfef6d0e4129f2b3fb3b384804aa86f68bd9797`.
- Affected formatting, whitespace, direct diff, and status checks pass. No workspace-wide/Nextest
  loop, full feature matrix, benchmark, fuzzing, packaging, dependency, push, publication,
  deployment, or history rewrite runs.

## Patient categorical cross-g checkpoint 161 — 2026-08-29

- Red-first extension: the expanded fake-process behavior test first errors because `execute` does
  not accept the cross-g `analysis`; after statistic-specific execution/extraction is added it
  passes. A second red requires explicit `promotion_status` and `fusion_status`; the completed test
  passes with `nonincremental_not_promoted` and
  `not_added_without_positive_incremental_information`.
- `target/pymc-venv/bin/python -m unittest tests.python.test_crc_categorical_pair_patient
  tests.python.test_crc_graph_topology_summary
  tests.python.test_marklab_cellvit_cptac_results_adapter` passes 13/13. `target/pymc-venv/bin/python
  -m py_compile workers/python/marklab_crc_categorical_pair_patient.py` passes.
- `/usr/bin/time -l ... execute --analysis categorical-cross-pair-correlation --maximum-processes
  6` completes 64 real misses in 69.93 seconds at 31,244,288-byte parent maximum RSS. The single
  `--replay` pass completes 64 backend-disabled hits in 64.17 seconds at 33,554,432-byte maximum
  RSS. Execution/replay manifest SHA-256 values are
  `bd97b6d1c798ccb1c7a5a9f5c6c2e700a338cb1b5db219ed9a610c72dd437c96` and
  `93529b700120623dfe3603131217695dfe2e73a216a81ed362099b12cd95b9e2`.
- The identity-final summary admits eight endpoints, reports stability median/q10 0.762/0.512,
  cross-g-only balanced accuracy 0.125 with exact p=0.9714, increment -0.25 with interval
  [-0.625, 0], and minimum adjusted p=0.921. Summary and patient-fingerprint SHA-256 values are
  `742ac465b3b457d37a6fc89418ba36b136782a62c3563d61bcb9b1560440b288` and
  `142ed82c25b0ef0bff377cb1e0a20bcbbbdc393eecec8e79c76ff730dfba4f94`.
- Absolute-path remote hashing verifies all 687 listed artifacts plus `SHA256SUMS`. An independent
  remote Python expansion compares all 64 miss/hit pairs, verifies all 64 ledgers have one row, and
  confirms the explicit nonpromotion/fusion state. Manifest SHA-256 is
  `d094671df0d0dd01e912322ba2c99afe3a4c90c73032d529530bde7e67f8b4b6`.
- Affected formatting, whitespace, direct diff, and status checks pass. No Rust package changes, so
  affected Clippy/no-default checks are not applicable. No broad workspace gate runs at this
  ordinary milestone; the scheduled stabilization follows.

## Point-process/mark stabilization checkpoint 162 — 2026-08-29

- `cargo +1.96.0 fmt --all --check` passes.
- `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings` passes
  without findings in 49.66 seconds.
- `cargo +1.96.0 check --locked --workspace --no-default-features` passes in 1.44 seconds.
- `cargo +1.96.0 test --locked --workspace --doc --all-features` passes all 17 package doctest
  binaries in 14.20 seconds.
- `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace --all-features --no-deps`
  passes in 4.27 seconds.
- The documented macOS Nextest/full-integration loader loop is not run. The full compile matrix is
  reserved for phase/release boundaries by `WORKSPACE_POLICY.md`, so it is not applicable here.
  Checkpoints 154/156/157/159/160/161 retain their focused, real-data, replay, and remote-bundle
  evidence. No benchmark, fuzzing, DHAT, RSS, packaging, dependency, push, publication, deployment,
  or history rewrite command runs.
- `git diff --check`, direct diff review, and `git status --short` pass with only the user's
  pre-existing unstaged/untracked CRC outcome changes remaining after the stabilization commit.

## Piecewise binary-compartment intensity/K-L checkpoint 163 — 2026-08-29

- Red-first direct evidence: `cargo +1.96.0 test --locked --package marklab --test
  piecewise_compartment_spatial_typed_workflow -- --nocapture` fails on the absent public estimator,
  role, config, and limits, then passes 3/3 after implementation. The direct-loop unequal-count
  oracle and interface/sparse/query/draw boundaries are included.
- Red-first durable evidence: `cargo +1.96.0 test --locked --package marklab --test
  piecewise_compartment_spatial_project_workflow -- --nocapture` fails on the absent node, then
  passes miss/hit, byte-equivalent typed output, one ledger row, seed invalidation, and oriented-role
  invalidation.
- Red-first CLI evidence: `cargo +1.96.0 test --locked --package marklab --features cli --test
  piecewise_compartment_spatial_project_cli -- --nocapture` fails with `unrecognized subcommand
  'piecewise-compartment-spatial'`, then passes a fresh miss and fresh backend-disabled hit with
  byte-identical output and one ledger row.
- The final affected command over the three new tests plus existing
  `inhomogeneous_spatial_{typed,project}_workflow` and
  `inhomogeneous_pair_correlation_{typed,project}_workflow` passes 12/12. This revalidates the
  Gaussian K/L and g paths after the shared pair accumulator is narrowed to radii plus pair ceiling.
- `cargo +1.96.0 clippy --locked --package marklab --features cli --lib --bin marklab` with the same
  seven integration targets and `-- -D warnings` first reports one complex envelope tuple. The
  focused piecewise tests pass again after replacing it with a named internal summary, and the exact
  warning-denied Clippy command then passes. `cargo +1.96.0 check --locked --package marklab
  --no-default-features` passes. `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --package
  marklab --features cli --no-deps` passes.
- `ssh mini` read-only `jq` filtering of the final-v4 manifest lists only whole observation
  `window.geojson` artifacts. A bounded `find` under the admitted v25 multitype input directory
  returns no mask, compartment, or GeoJSON file. Real exact-partition evidence is recorded
  unavailable rather than synthesized. Affected formatting and final whitespace/diff/status checks
  pass; no broad gate runs.

## Piecewise binary-compartment pair-correlation checkpoint 164 — 2026-08-29

- The direct test first fails to compile because
  `analyze_piecewise_compartment_pair_correlation` and its config are absent. Its final independent
  loop passes exact eligible-center, kernel-support-pair, inverse-intensity sum, normalized-g,
  kernel, bandwidth, and fixed-count-null assertions.
- The durable test first fails on the absent
  `PiecewiseCompartmentPairCorrelationAnalysisNode`, then passes miss/hit equality, one ledger row,
  and pair-bandwidth invalidation. The CLI test first fails with `unrecognized subcommand
  'piecewise-compartment-pair-correlation'`, then passes fresh-process miss/hit byte equality and
  one ledger row with `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1` on the hit.
- The final command covering the piecewise direct/durable/CLI targets plus the existing Gaussian
  inhomogeneous K/L and g direct/durable/CLI targets passes 16/16. This includes both checkpoint-163
  CLI processes after the shared source-preparation extraction.
- Warning-denied affected Clippy first reports `clippy::type-complexity` for the exact three-vector
  null sample return. After a named `SampledPiecewisePattern` replaces the tuple, the three focused
  piecewise targets pass 8/8 and the exact affected Clippy command passes. `cargo +1.96.0 check
  --locked --package marklab --no-default-features` and `RUSTDOCFLAGS='-D warnings' cargo +1.96.0
  doc --locked --package marklab --features cli --no-deps` pass.
- The successful bounded real-partition admission check from checkpoint 163 is not repeated because
  this workflow requires the identical partition artifact. Affected formatting, whitespace, direct
  diff, and status checks pass; no broad gate runs.

## Prespecified Gaussian bandwidth-selection checkpoint 165 — 2026-08-29

- Red-first direct evidence: `cargo +1.96.0 test --locked --package marklab --test
  inhomogeneous_spatial_bandwidth_selection -- --nocapture` fails on the absent selector/config/
  limits, then passes 2/2 independent score/selection and one-short aggregate-work tests.
- Red-first durable evidence: the matching project-workflow test fails on the absent node, then
  passes miss/hit equality, one ledger row, and candidate-list invalidation. Red-first CLI evidence
  fails with `unrecognized subcommand 'gaussian-bandwidth-selected-spatial'`, then passes fresh
  miss/backend-disabled hit byte equality and one ledger row.
- The final affected command over selected K/L plus existing fixed Gaussian K/L and g direct,
  durable, and fresh-process CLI targets passes 13/13. This explicitly revalidates both prior CLIs
  after their exact point-table/window/project preparation is shared. The three selected targets
  pass 4/4 again after formatting.
- Warning-denied affected Clippy first reports a needless borrow introduced by event-fit extraction,
  then reports a boolean literal assertion after that fix. Focused selector/fixed-K/L tests pass
  after the production fix, the direct selector passes after the test fix, and the exact affected
  Clippy command then passes. `cargo +1.96.0 check --locked --package marklab
  --no-default-features` and strict package docs with CLI/no-deps pass.
- `command -v Rscript` returns `/opt/homebrew/bin/Rscript`; `Rscript -e
  'packageVersion("spatstat.explore")'` fails exactly because that package is absent. `rg --files`
  finds no `renv.lock`, R package metadata, R profile, or R source. No package is installed. The
  checkpoint-157 real grid failure is reused as the exact reason not to run a selected real result.
  Affected formatting, whitespace, direct diff, and status checks pass; no broad gate runs.

## PP-05 estimator-family stabilization checkpoint 166 — 2026-08-29

- `cargo +1.96.0 fmt --all --check` passes.
- `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings` passes
  without findings in 56.09 seconds.
- `cargo +1.96.0 check --locked --workspace --no-default-features` passes.
- `cargo +1.96.0 test --locked --workspace --doc --all-features` passes all 17 package doctest
  binaries.
- `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace --all-features --no-deps`
  passes and generates the root plus 16 other package documentation trees.
- The prohibited macOS Nextest/full-integration loop and phase/release-only full feature matrix are
  not run. Checkpoints 163--165 retain their exact focused/direct/durable/CLI evidence. Final
  `git diff --check`, direct diff review, and `git status --short` pass with only the user's
  pre-existing unstaged/untracked changes after commit.

## Translation-corrected polygon-window K/L checkpoint 167 — 2026-08-29

- Red-first direct: `cargo +1.96.0 test --locked --package marklab --test
  translation_spatial_typed_workflow -- --nocapture` fails only on the absent estimator/config/
  limits/status exports, then passes 3/3. The final cases cover exact rectangle and concave hand
  values, a static GEOS 3.14.1 holed-multipolygon oracle, exact/one-short pair, overlap, candidate-
  work and memory limits, preflight output complexity, and zero-measure overlap.
- Red-first durable: `cargo +1.96.0 test --locked --package marklab --test
  translation_spatial_project_workflow -- --nocapture` first exposes one test-local name shadowing;
  after that correction it fails only on the absent node/document and then passes. Final evidence
  covers canonical round trip, unknown/value/work corruption, miss/hit equality, one successful
  run, and overlap-limit invalidation.
- Red-first CLI: `cargo +1.96.0 test --locked --package marklab --features cli --test
  translation_spatial_project_cli -- --nocapture` fails with `unrecognized subcommand
  'translation-spatial'`, then passes a fresh miss and backend-disabled byte-identical hit with one
  ledger row. The final three-target command passes 5/5 after raw-source/runtime identity and null-
  geometry validation are added.
- The affected command over the three translation targets plus `classical_spatial_domain`,
  `classical_spatial_workflow`, `classical_spatial_cli`, and `durable_classical_project` passes
  39/39. `cargo +1.96.0 clippy --locked --package marklab --features cli --lib --bin marklab` with
  all three new targets and `-- -D warnings` passes. `cargo +1.96.0 check --locked --package marklab
  --no-default-features` and strict CLI package docs pass.
- `PATH=/opt/homebrew/bin:/usr/bin:/bin target/pymc-venv/bin/python
  tests/fixtures/translation_overlap/generate_geos_oracle.py | diff -u ... -` passes exactly.
  The bounded real miss uses the frozen checkpoint-157 512-cell/12-component input, radius 20,
  19 simulations, seed 20260829, exact 2,616,320 pair visits, and explicit overlap/draw/memory
  limits. It completes in 6.68 seconds at 22,528,000-byte maximum RSS. A fresh
  `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1` hit completes in 5.76 seconds; `cmp` passes, the
  ledger has one row, and both outputs hash to
  `336b9ce32d50dd9b3d782506ff574bdb52fc0153f88afde4b0be83ccb5e6552f`.
- Affected formatting, final whitespace, direct diff, dependency-lock, and status checks pass. No
  workspace-wide gate, loader loop, full feature matrix, publication, deployment, or push runs.

## Isotropic visible-arc K/L checkpoint 168 — 2026-08-29

- Red-first direct: `cargo +1.96.0 test --locked --package marklab --test
  isotropic_spatial_typed_workflow -- --nocapture` fails only on absent estimator/config/limits/
  status exports. The first implemented run reaches the hole K assertion and exposes a test-only
  84-versus-96 area error; correcting the independent normalization produces green. The final four
  cases cover exact rectangle, hole, boundary-centered, million-angle concave differential,
  exact/one-short work and memory, and tangent-only zero-visible-measure behavior.
- Red-first durable: the matching project-workflow target fails only on the absent node/document.
  Its first green attempt exposes one-ULP direct-versus-canonical formula normalization; an explicit
  one-ULP direct assertion then exposes non-idempotent first-round-trip JSON. The document owner now
  emits a stable fixed point within three bounded passes. Isotropic and translation workflow tests
  pass with exact canonical miss/hit bytes, round trips, corruption rejection, and limit identity.
- Red-first CLI: `cargo +1.96.0 test --locked --package marklab --features cli --test
  isotropic_spatial_project_cli -- --nocapture` fails with `unrecognized subcommand
  'isotropic-spatial'`, then passes a fresh miss and backend-disabled byte-identical hit with one
  ledger row.
- The final affected command over three isotropic, three translation, and
  `classical_spatial_domain` targets passes 24/24. Warning-denied affected Clippy passes for the
  library, CLI binary, three isotropic targets, and translation document regression.
  `cargo +1.96.0 check --locked --package marklab --no-default-features` passes.
- Strict package docs first fail because `[0,1]` in a public error comment is interpreted as an
  intra-doc link. The literal is backticked and the same warning-denied docs command then passes;
  no lint allowance is added.
- The identity-final real run uses the frozen 512-cell/12-component checkpoint-157 input, radius
  20, 19 simulations, seed 20260829, exact 2,616,320 pair visits, and explicit arc/draw/memory
  limits. The miss takes 6.28 seconds at 22,478,848-byte maximum RSS; a fresh
  `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1` hit takes 5.78 seconds at 21,659,648 bytes. `cmp`
  passes, the ledger has one row, and both outputs hash to
  `98961c7825ae78f3805b3f643494607a83f0600edaf0582266b4cdc8cf6c0ceb`.
- Affected formatting, whitespace, direct diff, and status checks pass. No workspace-wide gate,
  loader loop, full feature matrix, publication, deployment, or push runs.

## Translation-corrected homogeneous pair correlation checkpoint 169 — 2026-08-29

- Red-first direct: `cargo +1.96.0 test --locked --package marklab --features cli --test
  translation_pair_correlation_typed_workflow` fails only on absent estimator/config exports. The
  implemented rectangle oracle passes at overlap 90, weighted kernel sum `10/3`, and g
  `250/(3*pi)`. The static GEOS 3.14.1 holed/disconnected overlap fixture agrees at 61.8125 after
  its test ceiling is set to the independently required Boolean-output bound. Compact-support
  emptiness and exact/one-short pair, overlap, candidate-work, output, and memory boundaries pass.
- Red-first durable: `cargo +1.96.0 test --locked --package marklab --features cli --test
  translation_pair_correlation_project_workflow` fails only on absent node/document exports. Its
  first implementation exposes a one-ULP direct-to-canonical JSON difference already covered by
  the stable document policy; the test retains exact canonical round-trip/strict corruption and a
  bounded direct-value tolerance. Overlap-limit changes produce a distinct miss.
- Red-first CLI: `cargo +1.96.0 test --locked --package marklab --features cli --test
  translation_pair_correlation_project_cli` fails with unrecognized subcommand, then passes a
  fresh miss and backend-disabled byte-identical hit with one ledger row. The final non-CLI affected
  command over translation g, translation K/L, and standard-border g passes 14/14; the final CLI
  target passes 1/1, and `cargo +1.96.0 test --locked --package marklab --lib
  common::seeds::tests` passes 2/2.
- The frozen checkpoint-157 input hashes again match
  `f2f1cc27d4ab2bfac2ecbc86bcc44ef3aac33180eb3051dc9586a8b2c581ec99` and
  `9f56f9ddeecbc9adb6c81d624964b5b578bfa7116da2a34777b6b42769fa87ac`.
  `/usr/bin/time -l target/debug/marklab project translation-pair-correlation ...` completes the
  radius-20/bandwidth-10 miss in 7.16 seconds at 23,379,968-byte RSS. The fresh
  `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1` hit takes 5.80 seconds at 21,626,880 bytes; `cmp`
  passes, the ledger remains one row, and both outputs hash to
  `8bc1bfbf211e6b474fe723534fd1f6d4b1eaffbf2d81984e4af691114cce7126`.
- The real result records observed/total pair visits 130,816/2,616,320, observed/total overlap calls
  812/7,717, 17,779,968 conservative segment-pair operations, maximum 60 output positions, 170,384
  estimated retained bytes, and 64,298 CSR draws. Its g 2.109894 and p=0.10 are retained only as
  one-specimen capacity evidence.
- Warning-denied affected Clippy passes for the library, CLI binary, three new targets, and affected
  translation K/L targets. Root no-default compilation and strict CLI package docs pass. Affected
  rustfmt and whitespace/diff checks pass. No workspace-wide/Nextest loop, full feature matrix,
  benchmark, fuzzing, packaging, dependency, publication, deployment, or push runs.

## Isotropic homogeneous pair correlation checkpoint 170 — 2026-08-29

- Red-first direct fails only on absent isotropic-g estimator/config exports, then passes rectangle
  and square-hole analytic visible-fraction/normalization oracles plus strict compact-support and
  exact/one-short work boundaries. Red-first durable fails only on absent node/document exports and
  then passes miss/hit, stable strict round-trip, corruption rejection, and arc-limit invalidation.
  Red-first CLI fails with an unrecognized subcommand and then passes fresh-process miss plus
  backend-disabled byte-identical hit with one ledger row.
- The final affected non-CLI command passes 9/9 across isotropic K/L and g direct/durable targets;
  the isotropic-g CLI target passes 1/1. Warning-denied affected Clippy, root no-default compilation,
  strict CLI package docs, affected rustfmt, and whitespace checks pass.
- Frozen input hashes match checkpoints 157/169. The radius-20/bandwidth-10 miss takes 8.32 seconds
  at 22,528,000-byte RSS and the disabled hit 5.79 seconds at 21,708,800 bytes. `cmp` passes, the
  ledger remains one row, and both outputs hash to
  `dc082a26b2b144f563151acd42a3423c296f57cdf85c25cbfe039abbd211bd8b`.
- The real capacity result records 130,816/2,616,320 observed/total pair visits, 1,624/15,950
  observed/total directed arc evaluations, 765,600 segment tests, 19,570 membership queries,
  maximum four intersection angles, 72,336 estimated bytes, and 65,365 CSR draws. Its g and p-value
  are retained without biological interpretation. No broad workspace/loader, feature matrix,
  benchmark, fuzzing, packaging, dependency, publication, deployment, or push command runs.

## Corrected point-process stabilization checkpoint 171 — 2026-08-29

- `cargo +1.96.0 fmt --all --check` passes.
- `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings` passes in
  1 minute 18 seconds without a finding.
- `cargo +1.96.0 check --locked --workspace --no-default-features` passes.
- `cargo +1.96.0 test --locked --workspace --doc --all-features` passes for all 17 workspace
  packages; the workspace currently declares zero doctest cases.
- `RUSTDOCFLAGS=-Dwarnings cargo +1.96.0 doc --locked --workspace --all-features --no-deps` passes
  for all 17 workspace packages.
- The known-stalling macOS Nextest/full-integration loader loop is excluded rather than retried.
  Checkpoints 167--170 retain the exact focused behavior, CLI, durable replay, and real-capacity
  evidence invalidated by none of these read-only gates.

## Translation-corrected directed categorical cross-g checkpoint 172 — 2026-08-29

- Red-first direct, durable, and CLI tests fail respectively on absent function/config, node, and
  subcommand, then pass. The full categorical typed target passes 6/6 and both standard/translation
  CLI targets pass. Warning-denied affected Clippy, root no-default compilation, strict docs,
  rustfmt, and whitespace checks pass.
- The frozen 512-cell miss takes 7.96 seconds at 25,673,728-byte RSS and the disabled hit 5.83
  seconds at 22,872,064 bytes. `cmp` passes, one ledger row remains, and both hash to
  `a6f18a64141d8f93c460f67e753591c37836c5208abb29a195c4651e08a625e8`.
- The result retains 1,872 directed pairs, 936 unordered visits, 812 overlaps, 1,870,848 segment-pair
  operations, maximum 60 output positions, 170,536 estimated bytes, and 35,568 null pair
  evaluations. Cross-g 0.655655 and p=0.10 are not patient-level claims.

## Isotropic directed categorical cross-g checkpoint 173 — 2026-08-29

- Red-first direct, durable, and CLI tests fail on absent function/config, node, and subcommand,
  then pass. The typed categorical target passes 8/8; standard, translation, and isotropic CLI
  targets pass 3/3. Warning-denied affected Clippy, root no-default compilation, strict docs,
  rustfmt, and whitespace checks pass.
- The frozen 512-cell miss/hit take 7.90/5.83 seconds at 25,165,824/23,494,656-byte RSS. `cmp`
  passes, one ledger row remains, and both hash to
  `73036c3fb94b2194a726e7f03c2a59429e82d1790004aaef5d918d06544ca395`.
- The result retains 1,872 directed pairs, 936 unordered visits, 1,624 arc evaluations, 77,952
  segment tests, 2,017 membership queries, maximum four intersection angles, 229,000 estimated
  bytes, and 35,568 null pair evaluations. Cross-g 0.604679 and p=0.10 are not patient claims.

## Corrected categorical patient sensitivity checkpoint 174 — 2026-08-29

- Red-first extension of `tests.python.test_crc_categorical_pair_patient` fails with
  `unsupported categorical analysis: translation-categorical-cross-pair-correlation`, then passes
  after the patient executor receives only the two existing corrected selections and their exact
  geometry ceilings. Final `env PYTHONDONTWRITEBYTECODE=1 target/pymc-venv/bin/python -m unittest
  tests.python.test_crc_categorical_pair_patient` passes 1/1 in 7.26 seconds.
- Authorized translation execution at
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v55-patient-translation-cross-g-work`
  completes 64 misses in 106.71 seconds at 31,129,600-byte RSS and 64 backend-disabled hits in
  89.33 seconds at 23,609,344-byte RSS. Execution, replay, manifest, and summary hashes are
  `9ef63dd3fd353983291978b5cac0e6525519236f0dfdbafda0cb5a20699c250a`,
  `565157e2ad9b311a7aebe55a709bf80ac2dcb342e25f400b93bbccae9180d2e9`,
  `22fecf22584a492cb5862bc9dc0de002380abc4e8ce65b5ae2397d605fb77ef1`, and
  `f57bf257ddd4a62bcc644ce622af0f64abae142923613a0939e8684cea6b83f5`.
- Authorized isotropic execution at
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v56-patient-isotropic-cross-g-work`
  completes 64 misses in 91.93 seconds at 32,784,384-byte RSS and 64 backend-disabled hits in
  87.94 seconds at 23,625,728-byte RSS. The corresponding four hashes are
  `46e85b6c3b6b1e81acc1975ae33674ea42a24cea2cab5a262a254e5627763103`,
  `cafc373d11d96f6e6adcfa42cd548b2ee26d19d3d7348830729bd31fc7072e87`,
  `7742cfc27941a57ad462a6324194d5a7458a6e9c7479dc63d94152f585f5666f`, and
  `5057ef35d90257db386e3687eb16c82bea8aec9dbb2c2aa90ff298dc2b911bd1`.
- Both summaries verify byte equality, one ledger execution, whole-patient held-out preprocessing,
  exact 70-assignment label permutation, 999-step-down-Max-T draws, and the same unavailable
  acquisition-site blocker. Both show zero balanced-accuracy increment, -0.125 retrieval
  increment, bootstrap interval [-0.375, 0.375], and no fusion. Translation minimum adjusted
  p=0.454 and isotropic minimum adjusted p=0.332 are retained without tuning or significance claim.
- Direct diff and whitespace checks follow this ledger update. No workspace-wide/Nextest loader,
  packaging, benchmark, fuzzing, publication, deployment, or push command runs.

## SCIENCE-CRC-FINAL-01 read-only completion audit checkpoint 175 — 2026-08-29

- On `ssh mini`, a standard-library SHA-256 audit of
  `/Volumes/1TB/marklab/runs/science-crc-final-01-v4/manifest.json` verifies all 1,905 declared
  artifacts, 1,906 total files including the manifest, zero missing paths, and zero digest
  mismatches. Manifest and `scientific_interpretation.json` hashes exactly match checkpoint 158.
- Direct manifest/ledger inspection verifies 64 categorical-pair replay hits with byte equality and
  64 one-row ledgers; 16 patient witness-bottleneck misses plus 16 backend-disabled byte-identical
  hits with 16 one-row ledgers; and the intended five graph/four topology variant executions per
  pattern. No external backend or completed analysis is started.
- `env PYTHONDONTWRITEBYTECODE=1 target/pymc-venv/bin/python -m unittest
  tests.python.test_crc_final_science_bundle tests.python.test_crc_graph_topology_final
  tests.python.test_crc_graph_topology_summary tests.python.test_crc_witness_bottleneck_patient`
  passes 13/13 in 0.84 seconds.
- Checkpoint 145 remains the requested broad baseline. Final whitespace, diff, and status checks
  run after this entry; no workspace-wide/Nextest loader, Bayesian backend, feature matrix,
  benchmark, fuzzing, packaging, dependency, push, publication, deployment, or history rewrite
  command runs.

## Type-specific inhomogeneous categorical cross-g checkpoint 176 — 2026-08-29

- Red-first `cargo +1.96.0 test --locked --package marklab --test
  inhomogeneous_categorical_cross_pair_correlation_typed_workflow` fails only on the absent public
  function/config, then passes 3/3 after the analytic caller, sparse-type guard, exact one-short
  memory boundary, durable miss/hit, and seed invalidation are implemented. The first analytic
  assertion exposes a test-only false symmetry assumption for boundary-corrected intensities; the
  independent oracle is corrected to evaluate all four role/event boundary masses and then agrees.
- `cargo +1.96.0 test --locked --package marklab --features cli` over the new typed/CLI targets and
  the affected standard categorical cross-g plus unmarked inhomogeneous g direct/durable/CLI
  targets passes 17/17. `cargo +1.96.0 test --locked --package marklab --lib
  common::seeds::tests` passes 2/2.
- Warning-denied affected Clippy passes for the library, CLI binary, and both new targets. Package
  no-default compilation first fails because the durable node's encoder is accidentally gated on
  `cli`, then passes after removing only that gate. Strict CLI package docs and
  `cargo +1.96.0 fmt --all --check` pass.
- The final binary hash matches locally and on the Mac mini at
  `8f5640960a44a29f07b3589e2b12ddef7209a1d90706848cad805b987dd8b5c5`. The fixed real command at
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v57-inhomogeneous-cross-g-work` exits 1 in
  9.68 seconds at 22,986,752-byte RSS with exact observed intensity
  `1.1027597985337386e-32 < 1e-12`. `miss.json` is absent and `executions.jsonl` has zero rows.
  The earlier pre-allocation-hardening binary reaches the identical blocker and is superseded.
- Final direct diff, whitespace, and status checks follow. No bandwidth/floor/pair/scale/subset
  search, broad workspace/Nextest loader loop, benchmark, fuzzing, packaging, dependency, push,
  publication, deployment, or history rewrite command runs.

## Durable typed scalar-semivariogram checkpoint 177 — 2026-08-29

- Red-first `cargo +1.96.0 test --locked --package marklab --test global_moran_typed_workflow
  typed_frame_mark_and_compartment_design_drive_global_moran_inference` fails only because
  `ScalarVariogramAnalysisNode` is absent, then passes after the cache-bound node, exact-float wire
  document, structural decoder, memory preflight, reopened hit, and seed invalidation are added.
- The first CLI assertion expects ordinary JSON numbers and fails after both miss/hit processes
  succeed because the repository exact-float codec intentionally stores IEEE-754 bit objects. The
  test is corrected to decode those exact bits; production is unchanged. Final `cargo +1.96.0 test
  --locked --package marklab --features cli` over `global_moran_typed_workflow`, the new scalar CLI,
  and affected categorical/inhomogeneous categorical CLI targets passes 5/5.
- `cargo +1.96.0 clippy --locked --package marklab --features cli --lib --bin marklab --test
  global_moran_typed_workflow --test scalar_variogram_project_cli -- -D warnings`, package
  no-default compilation, strict CLI package docs, and `cargo +1.96.0 fmt --all --check` pass.
- Read-only inspection confirms the v54 frozen header is exactly
  `cell_id,x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,slide_id,histologic_compartment`.
  Checkpoint 154 established that upstream CellViT JSON exposes `contour`, but no admitted
  contour-unit/scale-to-`nucleus_area_um2` provenance artifact exists. No real result is run or
  fabricated.
- Final direct diff, whitespace, and status checks follow. No broad workspace/Nextest loader,
  external backend, benchmark, fuzzing, packaging, dependency, push, publication, deployment, or
  history rewrite command runs.
