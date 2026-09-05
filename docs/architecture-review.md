# Marklab architecture inspection — 4 September 2026

Reviewed checkout: `/Users/user/Bench/gsc-marklab`, commit `f53f20746ec1455f8b9e76cbc983af08e7d101e9`.

This is the historical inspection at that commit. Subsequent implementation and executed evidence
are recorded in [ARCH-INTEGRATION-01](implementation/task-contracts/ARCH-INTEGRATION-01.md);
the [multiplex guide](multiplex-study.md) describes the newly implemented workflow.

**Assessment**

Marklab has substantial scientific breadth and useful foundations for trustworthy execution. Its principal architectural weakness is that individually admitted methods have expanded faster than the common data, application, and delivery layers. A researcher can run many sophisticated analyses, but assembling an ordinary multiplex, multi-slide, patient-level study still requires substantial method-specific preparation and orchestration.

For the intended intensive pathology spatial-statistics suite, prioritize multi-marker data, reusable application services, composable studies, deployable backends, and reproducible validation. Additional method names alone will not resolve these constraints.

This was an architectural inspection with targeted execution, not an exhaustive numerical audit or a release certification. Findings distinguish observed behavior, source-confirmed restrictions, and remaining validation needs. No production code, dependency, public API, scientific contract, or implementation ledger was changed.

**Scope and evidence**

Read the repository instructions, relevant portions of the current status/roadmap/tracker and canonical ownership/interface records, the master plan's architecture and workflow expectations, manifests, CI/release configuration, and representative implementation and tests. Traced data/marks, geometry, workflow/cache/durability, CLI routing, worker execution, cohort inference, maturity policy, and result boundaries. Inspected the recent patient-outcome statistics separately.

A census of tracked files found 17 workspace packages including the root; 365 Rust files under the root library, 250 under the binary, 460 under member-crate source directories, and 153 Python files under workers. These are physical source counts, including inline tests and adapters, not measures of implementation quality or coverage. The binary source totals 86,678 lines; its project module alone is 10,122 lines. Textual counting found 75 `impl WorkflowNode for` declarations in binary sources and 39 elsewhere in root/member library sources, including any inline test implementations.

**Prioritized findings**

P1 means a major obstacle to the stated suite or a delivery/verification defect. P2 means an important integration, scale, or usability limitation. These priorities do not assert that the underlying statistical formulas are incorrect.

| Priority | Finding | Evidence category |
|---|---|---|
| P1 | Shared marks cannot represent a general multiplex panel | Source-confirmed contract |
| P1 | Too much reusable execution behavior belongs to the binary | Source-confirmed ownership |
| P1 | Backend installation depends on the original build checkout | Source-confirmed deployment defect |
| P1 | CI does not provision the environment required by backend tests | Source-confirmed configuration mismatch; hosted run not executed |
| P1 | General study composition still requires custom orchestration | Source-confirmed limitation; fixed DAG tested |
| P2 | CLI routing hides supported capabilities from help | Reproduced |
| P2 | Maturity policy is detached from ordinary result publication | Source-confirmed integration gap |
| P2 | Intensive scale is method-specific and often bounded by dense storage | Source-confirmed constraints; no new performance measurements |
| P2 | Ecosystem interchange and notebook access are unfinished | Source/manifest inspection and explicit tracker state |
| P2 | Scientific readiness is difficult to discover across the catalog | Documentation, policy, and validation-surface inspection |

**1. P1 — Generalize the data boundary around a real multiplex caller.**

[`MarkTable::new`](/Users/user/Bench/gsc-marklab/src/scalar_mark/table/mark_table.rs:73) requires exactly one binary column and permits at most one probability, continuous, categorical, simplex, ordinal, and vector-reference column of each kind. The [continuous value variant](/Users/user/Bench/gsc-marklab/src/scalar_mark/table/column.rs:94) specifically carries `NucleusAreaUm2MarkDeclaration`; categorical values carry a histologic-compartment declaration. The [Pattern adapter](/Users/user/Bench/gsc-marklab/src/scalar_mark/table/validation.rs:14) rejects missing materialized scalar observations.

Consequently, a panel containing many protein intensities, several binary phenotypes, multiple cell classifications, and partial assay failures cannot be represented directly by this common table. Other model-specific matrix inputs exist, but that does not supply a shared panel contract. The broader mark vocabulary in documentation should not be mistaken for arbitrary numbers of independently named observations.

Use an actual multiplex dataset to introduce named columns with explicit value type, units, provenance, measurement status, and per-observation availability. Methods should select the columns they require. Keep the existing binary Pattern adapter as a compatibility projection. Do not remove its validation rules globally to admit new data, and do not invent a universal type system before a caller needs it.

Acceptance: one admitted panel containing several continuous and binary markers, categorical labels, and missing observations feeds two spatial methods and one patient reduction without manufacturing a nucleus-area field, dummy binary endpoint, or separate bespoke source table for each method.

**2. P1 — Move application behavior out of the CLI.**

The [project binary module](/Users/user/Bench/gsc-marklab/src/bin/marklab/project.rs:1) combines command declarations, dispatch, workflow-node implementations, cache identities, codecs, backend preparation, and publication. For example, [`HierarchicalNormalProjectNode`](/Users/user/Bench/gsc-marklab/src/bin/marklab/project.rs:8771) is private to the executable, while [cohort-energy execution](/Users/user/Bench/gsc-marklab/src/bin/marklab/project/cohort_energy.rs:34) prepares inputs and constructs the durable project, graph, scheduler, and result publication inside another binary module.

Library consumers can access many mathematical contracts but cannot reuse these complete durable services directly. A future notebook or server would have to invoke the CLI or reconstruct its orchestration. This violates the direction intended by the master plan: application surfaces should call reusable execution owners.

Extract coherent application services for existing workflows, beginning with two callers that genuinely share the lifecycle. Leave argument parsing and presentation in the binary. Preserve the current numerical engines and durable owners. Splitting the 10,122-line file into equally coupled files would not fix the ownership problem; a new crate is justified only if it establishes a real dependency boundary.

Acceptance: the CLI and a Rust integration client execute the same analysis service, producing the same scientific payload and honoring the same cache, provenance, resource, and error contracts.

**3. P1 — Make backend assets and environments relocatable.**

The hierarchy adapter resolves worker files from compile-time `CARGO_MANIFEST_DIR` in [hierarchical preparation](/Users/user/Bench/gsc-marklab/src/bin/marklab/bayes/hierarchical.rs:65). The [worker runner](/Users/user/Bench/gsc-marklab/src/bin/marklab/bayes/worker_process.rs:31) then requires `target/pymc-venv/bin/python` and `workers/python` under that root. The [release workflow](/Users/user/Bench/gsc-marklab/.github/workflows/release.yml:44) archives the executable, README, and licenses, without the workers or an environment/asset installation mechanism.

A distributed binary therefore cannot execute those methods on a clean recipient machine using the current asset resolution. The Unix interpreter path also does not describe a Windows virtual environment. Existing checksums and isolated process execution are valuable; they do not solve deployment.

Introduce an explicit installed backend location and asset manifest resolved at runtime. Reuse the pinned lock and digest checks. Provide a focused availability/doctor command and a documented installation path, with platform support stated per backend. The repository already has real Python callers, so this is an immediate production need.

Acceptance: unpack the distribution in a location unrelated to its build checkout; install an admitted backend through the documented procedure; execute one native and one Python analysis, then verify durable replay. This relocation test was not performed during the inspection; the defect follows from the referenced path and archive construction.

**4. P1 — Make backend verification reproducible on a clean CI runner.**

The [Rust CI job](/Users/user/Bench/gsc-marklab/.github/workflows/ci.yml:19) installs Rust and nextest, then runs all-feature workspace tests. It does not install Python 3.12, synchronize the worker lock, or create the environment expected by the runner. [The hierarchical durable integration test](/Users/user/Bench/gsc-marklab/tests/durable_pymc_hierarchical_project.rs:48) is an ordinary CLI-feature test that requires a real successful PyMC execution; similar tests exist for other backends. The local checkout has the pinned Python environment, which a clean hosted checkout does not acquire from this workflow.

This is a configuration mismatch that is expected to fail on a clean runner, not an observed GitHub job failure. Standalone scientific tests under `tests/python` also have no invocation in the inspected workflows. Rust integration tests invoking workers do provide meaningful coverage, but do not replace every standalone Python test.

Create explicit native and admitted-backend CI lanes; provision and verify the locked environment in the backend lane. Execute relevant Python unit tests there, and control concurrency for expensive fits. Keep required backend tests required in their lane instead of hiding them behind unconditional skips.

Acceptance: both lanes pass on clean environments, with a declared backend/version matrix and explicit reporting when a required environment cannot be admitted.

**5. P1 — Promote study composition beyond individually replayable commands.**

[`LocalScheduler`](/Users/user/Bench/gsc-marklab/crates/marklab-workflow/src/scheduler.rs:18) executes one typed node at a time. [`execute_algorithm`](/Users/user/Bench/gsc-marklab/crates/marklab-workflow/src/execution.rs:9) explicitly requires dependency outputs to have been executed or restored by the caller. A useful [fixed pre/post DAG](/Users/user/Bench/gsc-marklab/src/marked_prepost_dag.rs:20) already plans two roots and a comparison, including resource limits and cross-process resume; its tests passed in this inspection.

The limitation is therefore arbitrary study composition, not absence of graphs or durable replay. Import → common geometry → multiple markers/statistics → slide reduction → patient inference remains custom application work. There is no general recipe-driven execution surface connecting those stages.

Build the next increment around one complete cohort workflow. Extend the proven scheduler/durable boundaries only for that dependency graph, including partial failure, resume, typed artifact handoff, and host resource accounting. Avoid a plugin framework or distributed scheduler until an immediate study requires one.

Acceptance: interrupt the workflow after some slide analyses, resume from another process, and obtain identical patient-level results with no completed expensive node rerun and no silent omission of failed slides.

**6. P2 — Consolidate CLI routing and capability discovery.**

The [binary entry point](/Users/user/Bench/gsc-marklab/src/bin/marklab.rs:48) manually intercepts command strings before falling through to the [legacy Clap schema](/Users/user/Bench/gsc-marklab/src/cli/schema.rs:10). This produces observable disagreement between executable behavior and discovery:

- `target/debug/marklab --help` omits working families including `cohort` and `bayes`.
- `target/debug/marklab cohort --help` successfully lists the cohort commands.
- `target/debug/marklab project --help` omits `hierarchical-normal`.
- `target/debug/marklab project hierarchical-normal --help` succeeds and documents its arguments.

Use one composed command tree as the source of parsing, help, and completion. Preserve existing spellings and aliases. Add a behavioral discovery test covering every supported family and representative nested commands.

**7. P2 — Bind maturity and claim status to actual outputs.**

The only production call to `determine_result_maturity` found in source is the [standalone policy CLI](/Users/user/Bench/gsc-marklab/src/bin/marklab/policy.rs:174), which accepts a separately supplied `ResultMaturitySpec`. The [workflow node specification](/Users/user/Bench/gsc-marklab/crates/marklab-workflow/src/graph.rs:32) contains ID, kind, version, and dependencies; it does not carry the common scientific/maturity contract described by the master plan.

Individual methods do retain diagnostic and claim restrictions, including nonconvergence. The gap is their uniform propagation into publication, comparison, and downstream composition. A separately supplied policy evaluation is not automatically evidence about an analysis artifact.

At an appropriate recorded schema decision, add a common result descriptor that references the existing typed payload and binds method identity, estimand, replication unit, null/likelihood, measurement status, diagnostics, approximation status, validation evidence, and claim ceiling. Derive policy inputs from verified results wherever possible. Preserve result-format 0.3 and existing specialized schemas through explicit adapters.

Acceptance: a nonconverged fit can remain a diagnostic artifact, but a downstream report or comparative workflow cannot silently present it as validated evidence. Replay must retain the same decision.

**8. P2 — Define intensive scale per method and complete the necessary execution paths.**

The [mark-pair plan](/Users/user/Bench/gsc-marklab/src/mark_pair_plan.rs:15) retains contributing directed pairs. This is output-sensitive indexed work with explicit budgets, not necessarily an all-pairs scan, but retained edges can still dominate a dense or wide-radius study. The [adaptive SPDE worker](/Users/user/Bench/gsc-marklab/workers/python/marklab_scipy_advanced_bayes_worker.py:415) explicitly allocates dense finite-element matrices; its admitted mesh is capped at 1,024 vertices. The [exact GP](/Users/user/Bench/gsc-marklab/crates/marklab-bayes/src/gp.rs:96) admits 5–128 observations. Other sparse and approximate methods exist, so these examples should not be generalized to every algorithm.

Hard bounds are correct safety behavior. They nevertheless define limited computational domains. Million-row embedding-storage evidence does not establish that every inference method can fit a whole-slide dataset at that size. Likewise, scheduler output-size limits are not peak working-memory limits.

Specify representative ROI, whole-slide, and cohort workloads, with cells, channels, radius/edge density, null replicates, and time/memory ceilings. Profile the complete workflow. Where justified, reuse geometry across endpoints, select retained versus streamed pair execution deliberately, use sparse solvers, and move large intermediate/posterior payloads through artifact references. Preserve a tested small exact reference when adding approximation.

No throughput, speedup, RSS, or broad whole-slide fitting claim was measured in this inspection.

**9. P2 — Bring ecosystem access into the first useful study workflow.**

The [common cell loader](/Users/user/Bench/gsc-marklab/src/io/mod.rs:51) accepts CSV and Parquet; specialized embedding paths add useful other profiles. Inspection of production source/manifests found no SpatialData, AnnData, or OME-NGFF interchange implementation. The [program tracker](/Users/user/Bench/gsc-marklab/docs/implementation/PROGRAM_TRACKER.md:261) explicitly leaves Python/R/notebook clients and recipes planned.

For pathology users already operating in Python/R, repeated manual conversion to method-specific CSV/JSON is a major access and provenance burden. Prioritize one thin client and one real interchange profile, preserving row IDs, units, coordinate transforms, missingness, and measured-versus-predicted status. Do not duplicate statistical engines in a wrapper.

This recommendation aligns with established ecosystem boundaries: [SpatialData](https://spatialdata.scverse.org/en/stable/api/SpatialData.html) organizes images, labels, points, shapes, and annotation tables; [Squidpy's stable API](https://squidpy.readthedocs.io/en/stable/api.html) connects spatial graphs, statistics, and visualization; [spatstat](https://spatstat.org/) supplies an established point-pattern ecosystem and useful independent reference methods. These are integration/reference opportunities, not evidence that Marklab currently supports their formats or achieves estimator agreement. No new dependency version is selected by this review.

**10. P2 — Make scientific readiness visible at the point of use.**

The [tracker's point-process rows](/Users/user/Bench/gsc-marklab/docs/implementation/PROGRAM_TRACKER.md:174) distinguish implemented estimators from absent pinned-spatstat agreement, calibration work, and unsuccessful real intensity pilots. The [status record](/Users/user/Bench/gsc-marklab/docs/implementation/STATUS.md:4574) retains a nonconverged real joint location–embedding fit and failed embedding posterior prediction. These are valuable truthful limitations; they are not evidence that the mathematical implementations are intrinsically wrong.

However, a user must reconcile an extensive README/command catalog, method-specific results, thousands of status/roadmap lines, and separate policy commands to determine readiness for a particular question. The scheduled calibration workflow still targets the legacy marked/multimodal negative controls, while additional method-family validation lives elsewhere.

Expose a concise evidence-backed capability table: method and estimator, supported domain/units/marks/design, validated scale, exact/approximate status, external agreement, calibration, real-data scope, and claim ceiling. Keep scientific failure distinct from software failure and distinguish a synthetic oracle from biological validation. Archive superseded operational notes at normal checkpoints so the active roadmap identifies current work and blockers clearly.

Acceptance: a researcher can determine whether an analysis is supported for their dataset and inference target before launching it, and can inspect the same evidence through its result afterward.

**Coverage against common pathology workflows**

| Workflow | Useful current foundation | Principal gap for a general suite |
|---|---|---|
| Classical spatial organization in an exact ROI | K/L, g, nearest/empty space, several edge corrections, typed nulls, durable execution | Broader independent agreement/calibration and declared validated scales |
| Multiplex protein/cell-state analysis | Binary/categorical/simplex methods, neighborhood mixing, spatial autocorrelation, covariance/variograms | General multi-column marks, missingness, and assay interchange |
| Patient/cohort comparisons | Patient/block/paired/cluster inference, hierarchical bootstrap, Max-T, equivalence/noninferiority | Reusable provenance-bearing slide/ROI-to-patient study composition |
| Bayesian spatial pathology | Many pinned worker contracts, diagnostics, selected independent agreement and SBC | Portable backend delivery, reusable execution services, family-specific scale/validation |
| Interfaces, infiltration, glands/vessels/necrosis | Exact binary compartment geometry, signed interfaces, contact/fragmentation | Multiclass/object geometry, uncertainty, admitted annotations and pathology validation |
| Embedding studies | Verified cell/patch/region/slide artifacts, spatial/vector statistics, selected real callers | Flexible modality integration, reproducible end-to-end patient studies, external validation |
| 3-D, longitudinal, causal, generative studies | Bounded mathematical workflows and explicit admission restrictions | Suitable real designs/data, validated computational scale, and usable integrated workflows |

The appropriate interpretation of “everyone's spatial stats needs” is broad coverage through a coherent study model, a reliable established core, and clearly admitted extensions. Readiness should be declared by workflow and data/design assumptions, rather than by the number of available commands.

**Recommended implementation sequence**

1. Repair delivery and discovery: one command tree, relocatable backend assets, clean native/backend CI lanes. These changes make existing capabilities usable and verifiable.
2. Deliver one complete multiplex cohort study: real multi-column observations → exact window/geometry → two existing spatial methods → declared patient reduction → adjusted inference → report. Extract application services as this workflow needs them.
3. Run the same study through durable recipe execution and a thin Python client, with real artifact interchange and interruption/resume coverage.
4. Establish workflow-scale and scientific promotion evidence, then expand methods and modalities according to demonstrated unmet needs.

Retain deterministic seeds, finite/typed-unavailable results, exact spatial and biological identity, measured/predicted separation, transaction semantics, and compatibility. Those are strengths to build on. This sequence proposes future changes; it does not amend the immutable master plan or authorize a new dependency/schema/API by itself.

**Commands executed and limits**

| Command | Result |
|---|---|
| `cargo +1.96.0 test --locked --package marklab-workflow` | Passed: 1 unit test; 0 doc tests. Initially waited for a build lock, then completed. |
| `cargo +1.96.0 test --locked --package marklab --test workspace_contract --test project_workflow --test marked_prepost_dag_workflow` | Passed: 2 workspace, 12 project-workflow, and 4 fixed-DAG tests. Child-process checks are included in the DAG tests. |
| `cargo +1.96.0 test --locked --package marklab --test scalar_mark_input typed_mark_table_` | Passed: 2 focused tests; 9 filtered out. |
| `env PYTHONDONTWRITEBYTECODE=1 target/pymc-venv/bin/python -m unittest discover -s tests/python -p test_crc_spatial_outcome_statistics.py` | Passed: 4 tests covering patient resampling/permutation, a Cox example, and BH adjustment. This is not independent numerical certification of Cox regression. |
| `target/debug/marklab --help` and the three family/nested help commands listed in finding 6 | All exited successfully; reproduced inconsistent discovery. |
| `git diff --check`; `git diff --no-index --check /dev/null docs/architecture-review.md` | Passed, including whitespace validation of the new untracked report. |
| Local report-reference validation | All 25 file/line references resolve to existing files and valid line numbers. |

Total selected top-level test cases: 21 Rust and 4 Python, all passed. No test suite failed in this review. These existing tests substantiate their bounded behaviors; they do not establish that the architectural gaps are resolved.

No full-workspace nextest run, Clippy, formatter, feature matrix, packaging, dependency audit, fuzzing, benchmark, fresh backend fit, hosted CI job, release relocation test, or new real-data analysis was performed. No remote scientific data or unrelated repository content was accessed. Published/internal historical validation was treated as recorded evidence, not rerun evidence.

LSP was attempted with the explicit workspace root. It returned the scheduler outline but no symbols for the large binary project module. The task's workspace server was stopped and removal verified; other roots appeared in the manager inventory with ambiguous ownership and were left untouched. Targeted source reads supplied the remaining navigation.

Only this review document was added. The starting checkout was clean; final status shows only this new report. No local commit, push, or deployment was performed.
