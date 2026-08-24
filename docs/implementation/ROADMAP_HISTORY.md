# Marklab roadmap history

Last updated: 2026-08-24

Status: preserved superseded roadmap state from before the 2026-08-24 classical-workflow checkpoint. This file is historical evidence only. `ACTIVE_ROADMAP.md` is the current execution control; `MASTER_PLAN.md` remains authoritative total scope.

The sole active outcome is the complete end-to-end classical spatial-pathology workflow below. Work remains single-agent. Behavior changes follow red–green–refactor, and broad gates run once at this outcome's major stabilization checkpoint. The preserved candidate sections after the active outcome are inactive planning records only: they retain prior uncommitted work but do not authorize implementation or alter dependency order.

## Outcome 1 — Complete end-to-end classical spatial-pathology workflow

Tracker state: active.

Master-plan IDs advanced: DATA-01, FND-02, FND-03, FND-06, FND-07, GEO-01, PP-01, PP-06A, NUL-01A, INF-01B, PLAT-01, WF-01, WS-11, WS-12, WS-22, WS-30, and WS-31.

Concrete workflow delivered: a user runs one bounded CLI command over the existing supported cell input and mask GeoJSON. Marklab loads and filters the cells, constructs an explicit validated 2-D observation window, builds one immediately consumed reusable geometry/pair plan, declares conditional homogeneous CSR over the whole location pattern, computes border-corrected homogeneous Ripley K and L with deterministic CSR global inference, executes through the project scheduler, and atomically writes a typed result, run manifest, and claim-bounded report.

Existing prerequisites:

- the existing CSV/Parquet PatternLoader and MultiPolygon mask input used by the analyze command;
- C-02 physical micrometre coordinate semantics and current finite input validation;
- the exact R-tree, deterministic seed derivation, finite-result policy, ERL global-envelope implementation, and memory accounting substrate;
- the B-04 project, typed node, local scheduler, cache, and failure-atomic output transaction;
- existing CLI, strict JSON, run-manifest, and Markdown report patterns;
- result-format 0.3 remains unchanged; the classical workflow uses its own strict versioned result family.

Accepted inputs:

- CLI: marklab classical with required cells, mask, out, r-max-um, r-steps, simulations, seed, alpha, memory-budget-mib, max-pair-visits, and max-csr-draws arguments;
- cells: any CSV or Parquet input already accepted by PatternLoader; retained finite x_um/y_um coordinates are interpreted in the existing physical micrometre frame and the mark columns are not part of this unmarked point-process estimand;
- mask: one GeoJSON Geometry, Feature, or one-feature FeatureCollection containing a MultiPolygon, at most 16 MiB, with finite closed rings, nonzero area, no zero-length edge, self-intersection, ring crossing/touching, overlapping components, or hole outside its exterior;
- window bounds: at most 4,096 components, 65,536 rings, and the positive caller maximum of no more than 1,000,000 vertices; valid ring orientation may be canonicalized while invalid topology is never silently repaired;
- point pattern: at least zero retained points, all inside or on the closed window, with duplicate coordinates rejected for this simple-point-process workflow;
- radii: r-steps in 1..=4,096 values r_i = i * r-max-um / r-steps, where r-max-um is finite and positive;
- null design: exactly conditional homogeneous CSR, holding the validated window and observed point count fixed and resampling the entire location pattern; simulations must be positive and satisfy (simulations + 1) * alpha >= 1, seed is an explicit u64, and alpha is finite in (0, 1);
- resource limits: positive memory, pair-visit, and CSR candidate-draw limits admitted before successful output commit.

Observable outputs:

- a deterministic strict marklab.classical_spatial result family containing input/window/geometry/config/cache identities, exact resource telemetry, the explicit null and whole-pattern randomization unit, seed/simulation/alpha metadata, and typed workflow status;
- a window descriptor with physical unit/frame, area, perimeter, bounding box, component/hole/ring/vertex counts, canonical digest, and exact topology policy;
- one reusable geometry-plan descriptor with point count, boundary-distance and pair-traversal ownership, duplicate policy, maximum radius, pair visits, storage/work bounds, exact mode, and digest;
- for each radius, eligible-center count m(r), ordered-pair count q(r), theoretical K = pi r^2 and L = r, and, when m(r) is nonzero, K_border(r) = A q(r) / (n m(r)) and L_border(r) = sqrt(K_border(r) / pi);
- deterministic CSR curves and an existing ERL global envelope/p-value over radii where observed and all simulated L values are available; ineligible radii retain typed unavailable status and no floating filler is serialized as a result value;
- top-level Available, InsufficientPoints, or InsufficientInferenceSupport status; typed errors for malformed/ambiguous geometry, outside or duplicate points, invalid radii/null settings, exhausted CSR draws, and exceeded memory/pair limits;
- a project-scheduler miss/hit contract whose cache identity changes with any cells, window, radii, null, seed, alpha, simulation, implementation, or resource-policy change;
- one atomic output directory containing result.json, run_manifest.json, and report.md. The report names the estimand, window, border correction, CSR conditioning, whole-pattern randomization unit, exact/typed availability, and prohibited interpretations.

Explicit non-goals for this increment:

- no inhomogeneous K/L, pair-correlation, cross-K/cross-g, F/G/J, translation/isotropic/toroidal correction, intensity estimator, mark function, compartment/interface statistic, cohort inference, or Bayesian model;
- no raster-mask inference without a physical transform, silent geometry repair, approximate neighbor search, retained all-pairs matrix, GPU path, 3-D window, or external geometry/backend dependency;
- no result-format 0.3 mutation and no changes to existing analyze/batch/prepost/multimodal numerical or serialized behavior;
- no claim that departure from conditional CSR proves a biological mechanism, patient/population effect, causality, treatment effect, or clinical validity.

Focused acceptance tests:

1. Hand-computed rectangle, donut, concave, and disconnected-window fixtures prove area, perimeter, containment, holes, boundary distance, canonical digest, and topology rejection; malformed JSON/geometry, non-finite/zero-length/self-intersecting/touching/overlapping rings, empty windows, outside points, and exact-boundary points have frozen outcomes.
2. A brute-force reference agrees with the reusable indexed plan for unique/duplicate points, exact boundary distances, ordered pair counts, eligible centers, every radius, ties, empty/singleton inputs, and one-short point/vertex/memory/pair limits.
3. Small border-corrected K/L fixtures match the frozen formula and a trusted reference; positive zero is canonical and no NaN or infinity is emitted.
4. Conditional-CSR fixtures prove uniform window sampling, fixed observed n, whole-pattern randomization, seed/thread reproducibility, exact simulation count, ERL parity, degenerate/ineligible radii, candidate-draw exhaustion, and one-short resource limits.
5. Project-node tests prove failure atomicity, miss/hit parity, full cache invalidation, canonical codec fixed point, and no result-0.3 use.
6. CLI integration proves the supported CSV and Parquet flows load the same pattern, use the explicit window, write the three required artifacts atomically, expose typed unavailable/error states, and leave existing analyze output bytes/behavior unchanged.

Major-checkpoint exit condition: complete domain, brute-force/trusted-oracle, null, project/workflow, CLI, result, report, compatibility, and resource tests pass; retrospective cleanup leaves one window owner and one reusable pair traversal; formatting, affected packages, warning-denied workspace Clippy, no-default compilation, strict docs, applicable feature/CLI suites, and the full workspace suite run once and pass; STATUS.md and affected ledgers plus PROGRAM_TRACKER.md are updated from executed evidence; ACTIVE_ROADMAP.md promotes the next dependency-ordered outcomes; a coherent local checkpoint commit is created without push or publication.

## Preserved inactive candidate A — Durable binary/probability marks through a current project workflow

Tracker state: inactive planning record. Explicit user direction on 2026-08-24 promoted the classical workflow first and prohibited another narrow C-06 statistic.

## Preserved inactive candidate B — Patient-level randomization of a declared scalar endpoint

Tracker state: inactive planning record pending completion of the classical workflow and later roadmap promotion.

Master-plan IDs advanced: FND-06, COH-01, CMP-01, CMP-01A, INF-01A, INF-01C, NUL-01B, NUL-01D, WF-01, WS-12, WS-31, and WS-34.

Concrete workflow delivered: a user can take a prespecified finite scalar endpoint produced per independent patient, declare an independent-group or paired-patient randomization design, and obtain an exact or deterministically sampled patient-level comparison without treating cells, patches, regions, or slides as biological replicates.

Existing prerequisites:

- C-01 patient/specimen/timepoint hierarchy and biological-versus-technical roles;
- deterministic seed namespaces and the current canonical scalar permutation implementation;
- the project catalog/workflow/cache substrate;
- one provenance-complete scalar endpoint artifact from Outcome 1 or an existing bounded C-06 computation;
- an explicit decision before any stable result-schema addition.

Accepted inputs:

- a project hierarchy with stable PatientId and, when paired, TimepointId relations;
- exactly one finite scalar endpoint per eligible patient after a prespecified upstream aggregation; the workflow does not infer or silently aggregate cell/patch/region/slide rows;
- either two nonempty independent patient groups with one immutable group label per patient, or complete patient pairs with one value in each declared condition/timepoint;
- a declared null of patient-label permutation for independent groups or paired sign flip for paired data;
- exact enumeration when the number of assignments is within the configured bound, otherwise a positive replicate count, deterministic seed namespace, and resource limit;
- a prespecified two-sided alternative and one endpoint-family identifier for multiplicity ownership.

Observable outputs:

- observed difference in patient-level means, eligible patient/pair counts, group/pair identity digest, exact design summary, number of distinct and evaluated assignments, seed/replicate metadata, and a finite randomization p-value;
- a typed exact-versus-sampled execution mode and deterministic replay identity;
- typed failures for non-patient rows, duplicate patients, broken pairs, overlapping groups, non-finite endpoints, insufficient biological replicates, degenerate nulls, or a requested cell-level randomization;
- a content-addressed workflow result that cannot be reused after any endpoint, hierarchy, grouping, pairing, null, seed, or multiplicity-family change.

Explicit non-goals for this increment:

- no cell-level population claim, implicit within-patient aggregation, automatic fallback to a different randomization unit, or causal interpretation;
- no hierarchical bootstrap, functional curve comparison, MMD, energy distance, equivalence/noninferiority, interval family, Bayesian model, or multiple-endpoint correction beyond recording the family owner;
- no promise of adequate power from the existence of a computable p-value;
- no result-format 0.3 mutation.

Focused acceptance tests:

1. Hand-enumerated independent-group and paired sign-flip fixtures match an obvious reference implementation exactly, including tied/extreme statistics and the finite-sample p-value convention.
2. Patient row reordering does not change the result; changing hierarchy, labels, pairing, values, seed, or design changes the identity.
3. Cell/patch/region/slide pseudoreplication, duplicate patients, broken pairs, one-patient groups, homogeneous/degenerate assignments, non-finite values, and resource-limit exhaustion return the expected typed unavailable/error state.
4. Exact and sampled execution are deterministic across supported thread counts; sampled execution never claims exact enumeration.
5. Project scheduling is failure-atomic and cache hits reattach only after full current-input/design validation.

Major-checkpoint exit condition: both hand-oracle workflows and adversarial hierarchy tests pass; the unchanged canonical scalar permutation tests remain green; affected project/workflow and cohort integration tests pass; the shared checkpoint gates run once; the final diff demonstrates that patient is the only population unit accepted by this workflow and that no unsupported population, power, equivalence, or causal claim is emitted.

## Preserved inactive candidate C — Exact 2-D window and reusable geometry driving border-corrected K/L

Tracker state: folded into active Outcome 1; retained here only as prior detailed planning context.

Master-plan IDs advanced: FND-02, FND-03, GEO-01, PP-01, PP-06A, PLAT-01, WF-01, WS-22, and WS-30.

Concrete workflow delivered: a user can register an exact physical 2-D tissue window, validate point membership, construct one memory-budgeted reusable geometry plan, and compute descriptive homogeneous Ripley K and L curves with named border correction through the project workflow.

Existing prerequisites:

- C-01 stable CellIds and owning slide identity;
- C-02 finite physical 2-D coordinate frames and units;
- C-03 immutable artifacts and project store;
- the current exact R-tree/index, deterministic ordering, seed, and memory-accounting substrate;
- Outcome 2’s explicit design ownership, used here to declare descriptive-only output and prohibit an undeclared inferential null;
- dependency/license/MSRV review and a recorded decision before adding any geometry dependency or public schema.

Accepted inputs:

- one finite 2-D point pattern keyed by unique CellIds in a declared micrometre coordinate frame;
- one oriented polygon or multipolygon window in the same frame, including explicit holes and disconnected components;
- an explicit boundary tolerance and outside-window policy that never silently repairs or clips data;
- a strictly increasing finite radius vector, positive maximum radius, named border correction, and exact pair/storage/work limits;
- duplicate-coordinate policy chosen explicitly;
- no weights, intensity estimate, categorical type, or approximation mode for this outcome.

Observable outputs:

- an immutable window artifact and bounded descriptor containing area, perimeter, component/hole counts, frame, topology status, and digest;
- a reusable geometry-plan identity and telemetry containing point count, canonical pair ordering, radius support, boundary-distance ownership, exact mode, work/storage counts, and digest;
- the standard border/reduced-sample estimator used by [spatstat.explore Kest source revision 5.141 dated 2026-05-01](https://rdrr.io/cran/spatstat.explore/src/R/Kest.R): for window area A, n points, m(r) eligible centers whose boundary distance is at least r under the frozen tolerance, and q(r) ordered distinct-point pairs whose first point is eligible and whose distance is at most r, K_border(r) = A q(r) / (n m(r)) and L_border(r) = sqrt(K_border(r) / pi);
- finite radius-aligned K and L curves with exact numerator q(r), eligible-center count m(r), correction/estimand metadata, and typed undefined/unavailable states when n is less than two, m(r) is zero, or another window/radius/support condition fails;
- typed failures for invalid/self-intersecting/zero-area topology, frame/unit mismatch, prohibited outside points, unspecified duplicate policy, unsupported radii, and pre-allocation budget excess;
- one project workflow execution whose window, geometry, estimator, and input identities participate in cache validation.

Explicit non-goals for this increment:

- no inhomogeneous K/L, pair-correlation, cross-K/cross-g, F/G/J, translation/isotropic correction, intensity estimator, mark function, spatial p-value, or cohort claim;
- no silent geometry repair, raster-mask inference without a physical transform, toroidal behavior, approximate neighbor search, 3-D window, compartment inference, or GPU path;
- no claim that descriptive K/L alone establishes clustering, inhibition, causality, or a biological mechanism.

Focused acceptance tests:

1. Rectangle, concave polygon, donut, and disconnected-window fixtures validate area, perimeter, holes, containment, and boundary distance against an independent exact oracle; self-intersection, zero-area, non-finite, wrong-frame, and outside-window cases fail as specified.
2. The reusable plan agrees with an obvious brute-force reference on canonical pairs, ties, duplicate-coordinate policies, bins, and boundary eligibility.
3. Border-corrected K/L agrees with an independent trusted small-case oracle at every radius, preserves the frozen K-to-L transform, emits no NaN/infinity, and exposes unsupported-radius/zero-support states.
4. Window vertex, point, pair, radius, work, and storage limits reject before partial artifact/cache success; deterministic digests/results survive input ordering allowed by the contract and supported thread counts.
5. The project workflow reuses one geometry plan without endpoint-specific pair rebuilding and invalidates cache identity after any point, window, radius, correction, tolerance, or policy change.

Major-checkpoint exit condition: logical/physical window round trips, geometry differential tests, independent K/L oracle tests, and affected project/workflow integrations pass; required performance evidence demonstrates the declared resource model on a representative fixed-density workload if the implementation changes that boundary; the shared checkpoint gates run once; final review confirms one canonical geometry owner, exact-mode truthfulness, finite-result policy, and no undeclared inference or geometry repair.

## Preserved candidate checkpoint notes

If a preserved candidate is later promoted at a major checkpoint:

1. perform focused retrospective cleanup without changing behavior;
2. run formatting, affected package/integration tests, workspace-wide warning-denied Clippy, no-default compilation, strict docs, and the single all-feature workspace test suite once;
3. run only specialized geometry/performance evidence required by Outcome 3’s actual boundary;
4. review git diff, git diff --check, and git status --short;
5. update STATUS.md and affected validation/performance/claims/interface/decision ledgers from executed evidence;
6. update PROGRAM_TRACKER.md, re-evaluate every affected blocker/data gap, mark exact completed coverage, and replace this file with the next one-to-three dependency-ordered outcomes;
7. create the coherent local stabilization commit; do not push, publish, deploy, or rewrite history.
