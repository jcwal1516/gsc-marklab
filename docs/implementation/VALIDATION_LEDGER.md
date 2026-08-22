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
