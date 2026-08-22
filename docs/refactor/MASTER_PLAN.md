# Marklab Remediation and Architecture Refactor Master Plan

Repository: https://github.com/jcwal1516/marklab
Base branch: main
Plan version: 1.0
Primary objective: Convert the current repository from an overextended, partly misleading scientific prototype into a coherent, testable, performant, and scientifically honest Rust library and CLI.

⸻

1. Mission

Perform a comprehensive remediation of marklab covering:

1. Source-level correctness defects.
2. Misnamed or overstated scientific algorithms.
3. Fake or circular validation.
4. God files and distributed god workflows.
5. Duplicate implementations and missed reuse.
6. Dirty library/CLI/I/O/output boundaries.
7. Redundant and unimplemented public schema.
8. Quadratic spatial algorithms.
9. Excessive allocation, cloning, and repeated computation.
10. Output consistency, serialization safety, and atomicity.
11. API and result-format cleanup.
12. Test, benchmark, and validation credibility.
13. Documentation that accurately states what the system does and does not do.

Do not treat this plan as an instruction to mechanically rename files or create abstraction layers. Every refactor must reduce conceptual duplication, improve correctness, or produce a measurable performance or maintainability benefit.

The current audit was static. Every finding below must be reproduced or falsified with code inspection, tests, or benchmarks before closing it. Do not preserve a defect merely because a test currently encodes the defective behavior.

⸻

2. Persistent execution protocol

This project will span multiple context windows and compactions. Do not rely on conversational memory.

2.1 Create permanent planning files

At the beginning of the work, create:

docs/refactor/MASTER_PLAN.md
docs/refactor/STATUS.md
docs/refactor/DECISIONS.md
docs/refactor/FINDINGS_MATRIX.md
docs/refactor/PERFORMANCE_BASELINE.md

Copy this complete plan into MASTER_PLAN.md.

STATUS.md must always contain

# Current Refactor Status
Plan version:
Current repository SHA:
Current branch:
Current phase:
Current workstream:
Last completed requirement IDs:
Requirements currently in progress:
Known failing commands:
Known failing tests:
Dirty files:
Recent decisions:
Unresolved technical questions:
Next three concrete actions:
Next verification command:
Performance baseline location:
Last updated:

FINDINGS_MATRIX.md must contain

ID	Finding	Reproduced	Resolution	Tests	Benchmark	Commit	Status

Every issue ID in this plan must remain in that matrix until it is either:

* fixed;
* disproved with evidence;
* explicitly deferred with a documented reason and remaining risk.

“Could not reproduce” is not enough. Record exactly what was inspected or tested.

2.2 Compaction protocol

Immediately before any context compaction:

1. Update STATUS.md.
2. Record the exact current Git SHA.
3. Record git status --short.
4. Record all failing tests and commands.
5. Record every uncommitted file and why it is dirty.
6. Record the current phase and requirement IDs.
7. Record the next exact command to run.
8. Update the findings matrix.
9. Commit completed, verified work where appropriate.
10. Do not leave undocumented architectural decisions only in chat.

Immediately after a context compaction:

1. Read MASTER_PLAN.md.
2. Read STATUS.md.
3. Read DECISIONS.md.
4. Read FINDINGS_MATRIX.md.
5. Run:

git status --short
git log --oneline -10
git diff --stat
git diff

6. Re-run the smallest relevant verification command.
7. State internally:
    * current phase;
    * current requirement IDs;
    * next three actions;
    * existing failures that predate the current action.

Do not resume from memory alone.

2.3 Phase transition protocol

At the beginning and end of every phase:

1. Re-read the phase section in this plan.
2. Update STATUS.md.
3. Update the findings matrix.
4. Run the phase exit commands.
5. Write a brief phase closure entry in DECISIONS.md.
6. Confirm that no completed requirement has silently regressed.

⸻

3. Git and change-management rules

1. Start from a clean worktree.
2. If currently on main, create a branch such as:

git switch -c refactor/audit-remediation

3. Do not combine unrelated changes into one commit.
4. Use requirement IDs in commit messages, for example:

COR-02 implement true 2D rigid registration
PERF-01 replace quadratic nearest-neighbor scan
IO-01 unify CSV and Parquet pattern construction

5. Every committed change must leave the relevant test subset passing.
6. Do not commit knowingly broken intermediate states.
7. Do not use broad formatting rewrites that obscure substantive diffs.
8. Do not modify generated fixtures, lockfiles, or schemas without recording why.
9. Do not silently change public formats.
10. Do not force-push or rewrite published history.
11. Do not remove tests merely because they expose a defect. Replace invalid tests with tests of the corrected contract.
12. Do not introduce a dependency without:
    * explaining why the standard library or existing dependencies are insufficient;
    * running dependency policy checks;
    * checking license compatibility;
    * benchmarking the relevant path;
    * recording the decision.

⸻

4. Non-negotiable engineering principles

4.1 Scientific honesty

* Public names must match the implemented algorithm.
* Do not call a heuristic “MODWT,” “DoG,” “Bartlett,” “rigid,” “Bayesian,” or “validation” unless it actually implements the accepted meaning of that term.
* Do not use scientific vocabulary as decorative naming.
* Do not generate interpretation claims from arbitrary thresholds without documenting the threshold, rationale, and limitations.
* Never encode unavailable data as a meaningful numeric zero.
* Never encode an undefined statistic as infinity in a JSON-facing result.
* Validation must exercise the public production pipeline.

4.2 Architecture

Dependency direction should be:

domain/common
    ↑
domain algorithms
    ↑
application workflows
    ↑
input/output adapters
    ↑
CLI

The reverse direction is prohibited.

In particular:

* domain types must not open files;
* CLI modules must not contain scientific algorithms;
* output writers must not calculate analysis results;
* validation must not bypass the production engine;
* domain functionality must not be hidden behind a cli feature;
* result DTOs must not be generic dumping grounds for unrelated analysis types.

4.3 Reuse

* One canonical implementation for each mathematical statistic.
* One canonical implementation for permutation p-values.
* One canonical ingestion pipeline after format decoding.
* One canonical registration fit per transform type.
* One canonical graph per multimodal analysis run.
* One canonical run-manifest implementation.
* One canonical timing model.
* One canonical interpretation of component modes.
* One canonical spatial-index implementation reused across spatial algorithms.

4.4 Performance

* Replace quadratic spatial searches with indexed queries.
* Build geometric plans once and reuse them.
* Do not recompute distances, raster mappings, k-grids, or transforms unnecessarily.
* Avoid Vec<Vec<T>> for large numerical matrices where contiguous storage is appropriate.
* Avoid cloning complete results or cell tables merely to write outputs.
* Use configured memory limits operationally, not ceremonially.
* Preserve deterministic behavior under parallel execution.
* Benchmark algorithmic scaling, not only one input size.

4.5 Code quality

* Remove use super::* from production modules.
* Do not split code into one-function files merely to reduce line counts.
* Do not leave large workflows distributed across files through wildcard imports.
* Prefer cohesive modules with explicit inputs and outputs.
* No “Task 3,” “Task 13,” “MVP,” or implementation-plan terminology in production comments.
* Comments should explain domain semantics, invariants, or non-obvious tradeoffs.
* Avoid traits with only one implementation unless they establish a real external boundary.
* Avoid new generic frameworks unless they eliminate confirmed duplication.

⸻

5. Findings register

All findings below are mandatory investigation items.

Critical correctness and scientific findings

COR-01 — Multimodal validation bypasses the multimodal engine

The current multimodal synthetic validation generates outcome booleans directly rather than constructing multimodal input and invoking the production engine.

Required outcome:

* all multimodal validation scenarios call the public production pipeline;
* no validation outcome is directly synthesized;
* no status flags are manually inserted to make a scenario pass;
* no scenario sets passed = true unconditionally.

COR-02 — Configured rigid registration is not rigid

The current Rigid option routes to a scale-plus-translation model without rotation.

Required outcome:

* implement a true orientation-preserving 2-D rigid transformation:
    * rotation;
    * translation;
    * no scale change;
* retain scale-plus-translation only under an accurate name if still required;
* add known-transform and noisy-transform tests;
* update result metadata and documentation.

COR-03 — Stratified confounding comparison recomputes the same result

The current logic appears to test a stratified result against a recomputation of the same stratified result.

Required outcome:

* compute distinct unstratified and stratified analyses when confounding sensitivity is requested;
* report both results;
* define the confounding rule explicitly;
* avoid duplicate computation where shared observed quantities can be reused.

COR-04 — Non-finite enrichment results can break serialization

An expected edge count of zero can produce INFINITY.

Required outcome:

* all persisted floating-point values must be finite;
* use Option<f64>, a typed undefined state, or a justified finite estimator;
* zero null variance must not be represented as a z-score of zero unless zero is mathematically justified;
* serialization tests must cover sparse and degenerate null distributions.

COR-05 — Unavailable or invalid states are represented by numeric zero

Examples include comparison errors with statistic = 0.0 and empty pair-correlation bins with value zero.

Required outcome:

* distinguish:
    * observed zero;
    * unavailable;
    * undefined;
    * disabled;
    * insufficient data;
* use typed state rather than sentinel numbers.

COR-06 — Exact floating-point axis equality is used in pre/post comparison

Required outcome:

* either reuse a canonical axis identifier/configuration so equality is structural;
* or compare with a documented tolerance;
* return a typed axis mismatch;
* add tests with harmless floating-point reconstruction differences.

COR-07 — Internal-control validity semantics may be conflated with overall retained fraction

Investigate CSV and Parquet loaders. The current internal-control fraction appears to reuse a retained-row fraction that also includes tumor validity, IHC validity, artifact exclusions, and nonviable exclusions.

Required outcome:

* maintain separate counters and denominators for:
    * in-mask cells;
    * valid tumor;
    * valid IHC;
    * valid internal control;
    * artifact exclusions;
    * nonviable exclusions;
    * final retained cells;
* document every QC fraction.

⸻

Scientific naming findings

SCI-01 — The MODWT implementation is not an MODWT

Required outcome: choose and document one compliant path.

Preferred immediate path: rename the existing heuristic subsystem to an accurate name such as multiscale_residual, bump the result format, and remove false wavelet terminology.

Alternative path: implement an actual, documented undecimated 2-D wavelet transform with:

* named filter family;
* boundary policy;
* scale definition;
* energy normalization;
* independent numerical fixtures;
* documented limitations.

The current heuristic may not retain the MODWT name.

SCI-02 — The DoG module does not implement a difference of Gaussians

Required outcome:

* rename it to an accurate scale/radius helper;
* or implement actual Gaussian smoothing and difference-of-Gaussian response fields;
* add independent reference fixtures if implementing DoG.

SCI-03 — Wavelet territories are neighborhood residual heuristics

Required outcome:

* rename them and their result fields accurately;
* or replace them with an actual wavelet/DoG territory detector;
* stop populating fields such as qc_overlap_fraction with unconditional zero.

SCI-04 — Audit the “Bartlett periodogram” name

The current implementation appears to be a Hann-tapered raster FFT rather than a Bartlett segment-averaged periodogram.

Required outcome:

* verify the actual method;
* rename to tapered_periodogram if appropriate;
* or implement a true Bartlett estimator;
* ensure low_k_shells refers to real radial shells rather than the first sorted modes.

SCI-05 — Generic marked-pattern analysis emits MMR-specific interpretation text

Required outcome:

* move MMR-specific language to an MMR presentation or policy layer;
* keep the core marked-pattern engine generic;
* or explicitly narrow the engine contract to MMR-IHC and remove generic claims.

⸻

Architecture findings

ARCH-01 — Library and CLI perform different multimodal analyses

Required outcome:

* the application service must calculate all configured multimodal analyses;
* the CLI must not refit registration, rebuild the graph, or calculate separate domain sidecars;
* library and CLI users must receive equivalent domain results.

ARCH-02 — AnalysisEngine is a distributed god workflow

Required outcome:

* separate planning, computation, interpretation, and result assembly;
* use explicit stage inputs and outputs;
* do not merely move code into files that import the entire parent scope.

ARCH-03 — structure_factor.rs is a god file

Required outcome:

* split Fourier kernels, permutation execution, shell aggregation, scalar summaries, and result construction;
* preserve numerical behavior through differential tests.

ARCH-04 — CLI multimodal analysis contains domain logic

Move out:

* transform fitting;
* graph construction;
* null-model sensitivity;
* convex-hull geometry;
* extrapolation analysis;
* registration residual calculations;
* scientific CSV projections.

ARCH-05 — Configuration is a god file

Split:

* configuration data model;
* defaults;
* deserialization;
* validation;
* migration/version handling.

ARCH-06 — Pre/post comparison is a god file

Separate:

* marked comparison;
* multimodal comparison;
* axis validation;
* territory matching;
* scalar statistics;
* interpretation policy.

ARCH-07 — Output writer is a god file

Separate:

* result-document serialization;
* artifact planning;
* marked artifact generation;
* multimodal artifact generation;
* filesystem transaction/commit;
* manifest generation;
* telemetry writing.

ARCH-08 — Result types are a schema landfill

Required outcome:

* common result types;
* marked result types;
* multimodal result types;
* pre/post result types;
* diagnostic types;
* artifact types;
* no multimodal placeholders inside marked results.

ARCH-09 — Cosmetic modularity

Required outcome:

* merge ceremonial one-function files where they do not establish a meaningful responsibility;
* split actual god workflows by dependency and data ownership;
* remove wildcard parent imports.

⸻

Boundary findings

BOUND-01 — Pattern opens files

Required outcome:

* remove or deprecate Pattern::from_paths;
* put filesystem loading behind PatternLoader or input adapters;
* keep Pattern as a validated domain value.

BOUND-02 — Cell DTOs, CSV loading, CellViT adaptation, validation, and label interpretation share one module

Required outcome:

* separate domain cell types;
* label classification;
* generic CSV input;
* CellViT-specific adapter;
* validation helpers.

BOUND-03 — Domain enrichment is compiled only under the CLI feature

Required outcome:

* domain algorithms must be available to library users independently of the CLI feature;
* feature flags should gate dependencies and adapters, not analytical semantics.

BOUND-04 — Output code calculates domain policy

Required outcome:

* output adapters receive fully formed result and artifact models;
* no scientific interpretation, transform fitting, graph construction, or statistical calculation in output code.

BOUND-05 — WSI adapter is comparatively clean

Preserve its adapter boundary. Split only if growth justifies it. Do not destabilize it during unrelated refactoring.

⸻

Duplicate and missed-reuse findings

DUP-01 — Multiple median implementations

Required outcome:

* one canonical median implementation;
* explicitly define even-sample behavior;
* update all callers;
* add unit tests.

DUP-02 — Multiple mean, finite-mean, variance, min/max, and effective-length helpers

Required outcome:

* consolidate only where semantics are genuinely identical;
* use distinct names where denominator or missing-value behavior differs.

DUP-03 — Permutation p-value formula is duplicated

Required outcome:

* one scalar permutation-test implementation;
* explicit tail;
* inclusive tie behavior;
* plus-one correction;
* finite-value policy;
* minimum permutation requirements.

DUP-04 — Stratified and unstratified enrichment duplicate the full algorithm

Required outcome:

* one enrichment implementation;
* inject or parameterize only the permutation strategy;
* do not duplicate result construction.

DUP-05 — CSV and Parquet loading duplicate the same state machine

Required outcome:

* format adapters decode into one normalized row type;
* one PatternBuilder owns filtering, metadata validation, optional-column consistency, QC counters, and finalization.

DUP-06 — Registration and graph construction are repeated by CLI and engine

Required outcome:

* calculate each once;
* return required artifacts from the application run.

DUP-07 — Timing and manifest construction are duplicated

Required outcome:

* one telemetry model;
* one manifest builder;
* no writing and rereading timings.json just to create another representation.

DUP-08 — Curve-test DTO construction is duplicated

Required outcome:

* typed comparison results;
* one conversion to output DTOs;
* no fake zero statistic for unavailable tests.

DUP-09 — Effective geometry is calculated using inconsistent definitions

Required outcome:

* define canonical:
    * analysis effective length;
    * component effective length;
    * bounding diameter;
    * maximum interpretable scale;
* use names that state the geometric definition.

⸻

Performance findings

PERF-01 — spatial_index.rs is quadratic and is not a spatial index

Required outcome:

* replace it with a real reusable 2-D spatial index;
* document backend choice;
* ensure deterministic tie handling;
* validate against brute force.

PERF-02 — Radius graph construction is quadratic

Required outcome:

* use radius queries from the shared spatial index;
* create each undirected edge once.

PERF-03 — kNN graph construction sorts all other cells for every cell

Required outcome:

* use indexed kNN;
* preserve deterministic ordering and tie-breaking.

PERF-04 — Pair correlation recalculates all pair distances

Required outcome:

* build a reusable pair/bin plan using indexed radius queries;
* reuse pair/bin assignments for observed and permutation curves.

PERF-05 — Territory detection repeats neighborhood scans

Required outcome:

* use indexed scale-specific neighbor queries or precomputed neighbor plans;
* avoid recalculating geometry for every permutation.

PERF-06 — Profile membership repeatedly scans every cell

Required outcome:

* use radius queries against the same spatial index.

PERF-07 — Million-cell benchmark is not credible with quadratic nearest-neighbor calculation

Required outcome:

* make the full benchmark operational after indexed geometry;
* stream fixture generation rather than building an enormous in-memory CSV string;
* keep million-cell workloads manual or scheduled, not ordinary pull-request CI.

PERF-08 — Spectrum stores large nested mode-power matrices

Required outcome:

* aggregate mode powers into shell powers as early as possible;
* use contiguous storage;
* make k_chunk_modes operational;
* reuse scratch buffers.

PERF-09 — Repeated metadata and label allocations

Required outcome:

* separate shared run metadata from per-cell records;
* avoid cloning case_id, timepoint, and protein into every fused cell where feasible;
* represent labels using borrowed values, enums, or interned IDs;
* make primary_label avoid allocating a new String on every call.

PERF-10 — Complete results and cell tables are cloned for output

Required outcome:

* use borrowing or ownership transfer;
* do not clone a complete analysis result merely to write it;
* return artifact metadata separately.

⸻

Public model and output findings

MODEL-01 — TerritoryFeature overloads unrelated algorithms

Required outcome:

* separate marked residual territories and multimodal neighborhood territories;
* use meaningful fields for each;
* remove z_or_power.

MODEL-02 — Public fields are present but not implemented

Investigate:

* p_equivalence;
* empty TerritoryProfile.enrichment;
* empty TerritoryProfile.cross_curves;
* empty multimodal timings;
* constant-zero QC overlap;
* compatibility aliases.

Required outcome:

* implement the field correctly;
* or remove it in a versioned schema change;
* never retain placeholder fields solely to imply future capability.

MODEL-03 — String statuses and interpretation classes

Required outcome:

* use enums internally and in serde where appropriate;
* centralize display formatting;
* prevent invalid status strings.

MODEL-04 — Component modes are not behaviorally distinct

Define and test:

* Pooled;
* Separate;
* Both;
* Auto.

Separate must not silently behave like Both.

OUT-01 — Result and timings artifacts describe different timing histories

Required outcome:

* define one authoritative telemetry model;
* result document and timing sidecar must not contradict each other.

OUT-02 — Pre/post results are unversioned

Required outcome:

* add pre/post result variants to the versioned result envelope;
* or introduce a separately versioned pre/post document;
* standardize file/directory input behavior.

OUT-03 — Output writing is non-atomic

Required outcome:

* write to a temporary run directory on the same filesystem;
* validate artifacts;
* rename or commit the completed directory;
* clean up failed temporary runs;
* prevent half-written results from appearing successful.

OUT-04 — Parquet writer fabricates absent fields

Required outcome:

* preserve optional-field absence where round-trip semantics are promised;
* or rename the operation explicitly to a filtered canonical export;
* do not claim a general Pattern round trip when values are synthesized.

OUT-05 — CSV and Parquet schema definitions can drift

Required outcome:

* one authoritative logical cell schema;
* adapter-specific physical mapping;
* parity tests.

OUT-06 — Batch output IDs may escape the output directory

Investigate use of manifest IDs in output paths.

Required outcome:

* reject absolute paths;
* reject ..;
* normalize and verify the final path remains inside the configured output root.

⸻

6. Target architecture

Do not perform a big-bang rewrite. Move toward this dependency structure incrementally.

A reasonable target is:

src/
  lib.rs
  common/
    mod.rs
    finite.rs
    stats.rs
    geometry.rs
    seeds.rs
    telemetry.rs
  domain/
    mod.rs
    marked/
      mod.rs
      pattern.rs
      validation.rs
      spectrum/
        mod.rs
        kernel.rs
        modes.rs
        shells.rs
        permutation.rs
        summaries.rs
      pair_correlation.rs
      anisotropy.rs
      multiscale_residual.rs
      components.rs
    multimodal/
      mod.rs
      cells.rs
      labels.rs
      registration/
        mod.rs
        rigid.rs
        affine.rs
        scale_translation.rs
        qc.rs
      graph.rs
      enrichment.rs
      cross_interaction.rs
      territories.rs
      profiles.rs
    inference/
      mod.rs
      scalar.rs
      erl.rs
      multiple_testing.rs
    comparison/
      mod.rs
      axes.rs
      difference.rs
      equivalence.rs
      territory_matching.rs
  application/
    mod.rs
    marked_analysis.rs
    multimodal_analysis.rs
    marked_prepost.rs
    multimodal_prepost.rs
    validation/
      mod.rs
      marked.rs
      multimodal.rs
  config/
    mod.rs
    model.rs
    defaults.rs
    deserialize.rs
    validate.rs
    migrate.rs
  adapters/
    mod.rs
    input/
      mod.rs
      row.rs
      pattern_builder.rs
      csv.rs
      parquet.rs
      geojson.rs
      cellvit.rs
    output/
      mod.rs
      document.rs
      artifact_plan.rs
      transaction.rs
      json.rs
      parquet.rs
      csv.rs
      geojson.rs
      figures.rs
      reports.rs
    wsi/
      mod.rs
      reader.rs
      metadata.rs
      validation.rs
  cli/
    mod.rs
    args.rs
    commands/
      analyze.rs
      multimodal.rs
      prepost.rs
      batch.rs
      validate.rs
      slide.rs
  bin/
    marklab.rs

Exact names may differ, but the dependency rules may not.

6.1 Core run objects

Create application-level run objects that retain expensive reusable artifacts without leaking them into serialized domain results.

Example:

pub struct MarkedAnalysisRun {
    pub result: MarkedPatternResult,
    pub telemetry: RunTelemetry,
    pub artifact_data: MarkedArtifactData,
}
pub struct MultimodalAnalysisRun {
    pub result: MultimodalResult,
    pub transform: Transform2D,
    pub graph: SpatialGraph,
    pub null_model_sensitivity: Vec<NullModelSensitivityResult>,
    pub registration_residuals: Vec<RegistrationResidual>,
    pub extrapolation: Vec<CellExtrapolationRecord>,
    pub telemetry: RunTelemetry,
    pub artifact_data: MultimodalArtifactData,
}

The CLI should consume these objects. It must not recompute their contents.

6.2 Domain model rules

* Pattern contains validated data, not filesystem operations.
* Dataset metadata is stored once.
* Per-cell structures contain only per-cell data.
* Undefined statistics are typed.
* Serialized results contain no non-finite values.
* Marked and multimodal territories are distinct types.
* Public output types reflect completed implementation only.
* Configuration values have actual behavioral effects.

⸻

7. Phase 0 — Baseline, evidence, and reproducibility

7.1 Record repository state

Run and record:

git status --short
git rev-parse HEAD
git branch --show-current
rustc --version
cargo --version

Use the repository-pinned toolchain. Do not silently change the declared Rust version merely to make local setup easier.

7.2 Run the complete existing verification suite

Attempt:

cargo +1.96.0 fmt --check
cargo +1.96.0 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.96.0 nextest run --locked --all-features
cargo +1.96.0 test --locked --all-features
cargo +1.96.0 test --locked --doc --all-features
cargo +1.96.0 check --locked --no-default-features
cargo +1.96.0 test --locked --features wsi,cli --test wsi_integration
cargo audit
cargo deny check advisories licenses bans sources
cargo machete
cargo +nightly fuzz check

If a command is unavailable:

* record that fact;
* install only the repo-declared tool where appropriate;
* do not mark the check as passed.

7.3 Inventory the codebase

Record:

* production LOC;
* test LOC;
* largest source files;
* largest functions where practical;
* module dependency graph;
* public API surface;
* feature combinations.

Useful searches:

rg -n 'use super::\*'
rg -n 'Task [0-9]+|MVP|TODO|FIXME|compatibility alias'
rg -n 'fn median|fn mean|fn finite_mean|fn min_max|fn effective_length'
rg -n 'INFINITY|NEG_INFINITY|statistic: 0\.0|qc_overlap_fraction: 0\.0'
rg -n 'timings: Vec::new\(\)|enrichment: Vec::new\(\)|cross_curves: Vec::new\(\)'
rg -n '#\[cfg\(feature = "cli"\)\]'

7.4 Add minimal reproductions before changing behavior

Create targeted regression tests for:

1. Rigid registration failing on rotation.
2. Sparse enrichment creating a non-finite ratio.
3. Result serialization with sparse enrichment.
4. Stratified confounding comparing identical analyses.
5. Separate component mode behaving like Both.
6. Result/timings mismatch.
7. CSV/Parquet optional-field drift.
8. Internal-control fraction conflation.
9. Exact float axis comparison.
10. Multimodal validation bypassing actual analysis.
11. Pair-correlation empty bins being represented as observed zero.
12. Batch output path traversal.

Tests may initially fail locally, but committed states must remain green.

7.5 Establish performance baselines

Create reproducible baseline benchmarks for:

* nearest-neighbor distance;
* radius graph;
* kNN graph;
* pair correlation;
* marked territories;
* multimodal territories;
* territory profiles;
* structure-factor observed path;
* structure-factor permutation path;
* probabilistic-mark spectrum;
* CSV load;
* Parquet load;
* complete marked analysis;
* complete multimodal analysis.

Use at least three sizes where appropriate so scaling is visible.

Record:

input size
point density
edge count
permutation count
thread count
wall time
peak memory
hardware
compiler profile
Git SHA

Do not claim improvement based on a single noisy benchmark.

Phase 0 exit criteria

* Existing suite results are recorded.
* Every critical finding has either a regression test or a documented reproduction.
* Performance baseline exists.
* Findings matrix is populated.
* No production behavior has changed yet except testability scaffolding.

⸻

8. Phase 1 — Foundational shared utilities and invariants

This phase should reduce duplicate low-level behavior before major rewrites.

8.1 Canonical statistics module

Implement and test canonical functions for:

* median with documented even-sample behavior;
* finite median;
* arithmetic mean;
* finite mean;
* population variance;
* sample variance;
* min/max over finite values;
* percentile definition;
* safe ratio;
* finite-value validation.

Do not force functions with different denominator semantics into one vaguely named helper.

Example naming:

median_sorted_average_even
mean_all_finite
mean_ignoring_nonfinite
sample_standard_deviation
safe_finite_ratio

Prefer rejecting non-finite scientific input unless ignoring it is explicitly part of the contract.

8.2 Canonical seed derivation

Create one domain-separated deterministic seed helper.

Requirements:

* distinct endpoint namespaces;
* stable across thread counts;
* stable across process runs;
* no accidental correlation through ad hoc XOR constants scattered across files.

Example concept:

derive_seed(base_seed, Endpoint::Spectrum, permutation_index)
derive_seed(base_seed, Endpoint::PairCorrelation, permutation_index)

Preserve historical deterministic outputs only where they are part of an intentional contract.

8.3 Finite result boundary

Introduce one authoritative validation path ensuring serialized domain results contain no:

* NaN;
* positive infinity;
* negative infinity.

Do not serialize them as strings.

For potentially undefined results, use:

Option<f64>

or:

AnalysisSection<f64>

or an explicitly tagged statistic state.

8.4 Explicit imports

Begin removing use super::* from modules being touched. Do not perform a repository-wide noisy import rewrite in one commit.

Phase 1 exit criteria

* Duplicate median implementations are removed.
* Shared finite-value policy is documented.
* Seed derivation is centralized for touched endpoints.
* All tests pass.
* No new abstractions exist without an immediate caller.

⸻

9. Phase 2 — Critical correctness remediation

9.1 Implement a true rigid transform — COR-02

Implement an orientation-preserving 2-D least-squares rigid fit.

A dependency-free closed-form solution is acceptable:

1. Compute source and target centroids.
2. Center landmarks.
3. Calculate:

a = Σ(sx * tx + sy * ty)
b = Σ(sx * ty - sy * tx)
theta = atan2(b, a)

4. Construct:

R = [[cos(theta), -sin(theta)],
     [sin(theta),  cos(theta)]]

5. Translation is:

t = target_centroid - R * source_centroid

6. Reject degenerate source geometry.
7. Do not estimate scale in the rigid path.
8. Preserve deterministic behavior.
9. Confirm reflection behavior is explicitly prohibited or separately supported.

Required tests:

rigid_identity
rigid_translation
rigid_rotation_90_degrees
rigid_rotation_and_translation
rigid_preserves_distance
rigid_does_not_absorb_scale
rigid_rejects_degenerate_landmarks
rigid_handles_small_noise
rigid_result_is_finite

Rename the old implementation to fit_scale_translation if retained.

Update:

* configuration enum;
* serde names;
* result transform type;
* documentation;
* CLI help;
* migration behavior.

9.2 Correct confounding sensitivity — COR-03

Define a clear contract.

Recommended behavior:

* unstratified analysis remains available as a sensitivity result;
* stratified analysis is calculated when configured;
* primary result is explicitly declared by configuration;
* ConfoundedBySpatialStrata is raised when:
    * unstratified endpoint is significant;
    * stratified endpoint is not significant;
    * both are evaluable;
* mark-homogeneous strata are reported as a degenerate null, not silently treated as an ordinary result.

Create a type such as:

pub struct SpectrumNullSensitivity {
    pub unstratified: AnalysisSection<SpectrumInference>,
    pub stratified: AnalysisSection<SpectrumInference>,
    pub conclusion: ConfoundingConclusion,
}

Do not rerun the same calculation to compare it with itself.

Reuse:

* resolved modes;
* observed power;
* shell plan;
* configuration validation.

Required tests:

confounding_detected_when_unstratified_disappears_after_stratification
confounding_not_detected_when_both_remain_significant
confounding_not_detected_when_neither_is_significant
homogeneous_strata_report_degenerate_null
missing_strata_report_validation_error
distinct_nulls_are_actually_executed

9.3 Remove non-finite enrichment output — COR-04

Change enrichment result semantics.

Recommended fields:

pub expected_edges: f64,
pub enrichment_ratio: Option<f64>,
pub z_score: Option<f64>,
pub p_value: Option<f64>,
pub q_value: Option<f64>,

Rules:

* expected_edges == 0 and observed_edges > 0:
    * ratio is None, unless a documented pseudocount estimator is explicitly selected;
* zero null variance:
    * z-score is None;
* p-value can still be calculated by a valid permutation rule;
* output must include enough state for the consumer to understand why a statistic is undefined.

Update JSON, CSV, Parquet, reports, tests, and documentation.

9.4 Replace sentinel zero states — COR-05

Remove patterns such as:

statistic: 0.0

for failed comparisons.

Use:

AnalysisSection<CurveTestResult>

or an equivalent tagged state.

For pair-correlation bins with no contributing pairs:

* represent the bin value as optional;
* or omit the bin with an explicit bin-availability summary;
* do not imply measured zero correlation.

9.5 Correct axis comparison — COR-06

Preferred solution:

* derive pre/post axes from a canonical AxisDefinition;
* compare the definition structurally;
* use the same axis object for both curves where possible.

Fallback:

* use a documented relative/absolute tolerance;
* include mismatch diagnostics.

Do not use direct f64 != f64 for independently reconstructed axes.

9.6 Correct QC fraction semantics — COR-07

Create a shared QcCounters or PatternBuildCounters.

Track separate denominators and numerators.

Add result fields only where they have defined meaning.

Required tests:

* internal-control-invalid rows;
* artifact-only exclusions;
* nonviable-only exclusions;
* valid-tumor failures;
* valid-IHC failures;
* combinations of exclusions;
* zero in-mask denominator;
* CSV and Parquet parity.

9.7 Fix component mode semantics — MODEL-04

Define behavior explicitly.

Recommended contract:

Pooled

* calculate pooled analysis only;
* component result is NotApplicable or Disabled.

Separate

* calculate component analyses;
* do not present pooled inference as the primary endpoint;
* result schema must make this distinction explicit.

Both

* calculate pooled and component analyses.

Auto

* apply a documented fragmentation rule;
* record why pooled, separate, or both were selected.

Do not leave Separate and Both equivalent.

Phase 2 exit criteria

* True rigid registration is implemented.
* Confounding uses distinct analyses.
* No serialized non-finite enrichment values.
* No sentinel-zero unavailable curve tests.
* QC fractions are semantically separated.
* Component modes are behaviorally distinct.
* Result schema changes are versioned or staged behind internal types.
* All correctness regression tests pass.

⸻

10. Phase 3 — Scientific naming and algorithm integrity

10.1 Create an algorithm naming audit

For every public analytical function, result field, CLI term, report phrase, and configuration key, record:

public name
implemented method
accepted technical meaning
match/mismatch
required action
reference test

At minimum review:

* MODWT;
* wavelet;
* scalogram;
* DoG;
* Bartlett periodogram;
* rigid;
* similarity;
* graph smoothing;
* Bayesian/beta-binomial wording;
* global envelope;
* equivalence;
* validation;
* territory;
* spatial index.

10.2 Resolve wavelet terminology

Do not preserve the present naming.

Default remediation path

Rename the current implementation:

wavelet                    → multiscale_residual
WaveletSummary             → MultiscaleResidualSummary
ScalogramPoint             → ScaleEnergyPoint
wavelet_territories        → residual_territories
coarse_variance_fraction   → appropriately named heuristic metric

State exactly how the metrics are calculated.

Bump result format.

Actual-wavelet path

Only choose this path if the product explicitly requires a true wavelet method.

Requirements:

* actual undecimated transform;
* named filter;
* explicit boundary handling;
* physical-scale mapping;
* energy definition;
* independent fixtures generated from a trusted implementation;
* tests for constant, impulse, stripe, coarse field, and random field;
* no reuse of the old heuristic under the new name.

10.3 Resolve DoG terminology

If implementing DoG:

* define Gaussian kernels in physical units;
* normalize kernels;
* define boundary behavior;
* compute two smoothed fields;
* subtract them;
* define positive/negative response interpretation;
* validate against independent fixtures.

Otherwise remove the DoG name.

10.4 Resolve periodogram terminology

Verify whether the implementation is:

* a tapered periodogram;
* a Bartlett estimator;
* another raster spectral diagnostic.

Then:

* rename or implement correctly;
* implement real radial shell aggregation;
* define low-(k) shell selection;
* add tests proving shell grouping.

10.5 Move MMR interpretation policy

The generic marked engine should return neutral analytical classes and quantities.

Example:

SpatialPatternClass::CoarseExcess
SpatialPatternClass::LowFrequencySuppression
SpatialPatternClass::RandomLike
SpatialPatternClass::InsufficientData
SpatialPatternClass::SuppressedByQc

An MMR-specific report layer may convert that into MMR-IHC prose.

No generic library result should claim MMR semantics unless its input type is explicitly MMR-specific.

Phase 3 exit criteria

* No public algorithm name materially overstates its implementation.
* Result-format changes are recorded.
* README and SPEC use accurate terminology.
* Independent reference tests exist for any retained established algorithm name.

⸻

11. Phase 4 — Application and boundary refactor

11.1 Refactor marked analysis workflow — ARCH-02

Break AnalysisEngine::analyze_pattern_inner into explicit cohesive stages.

Possible stage outputs:

ValidatedPatternContext
SpectrumPlan
SpectrumAnalysis
PairCorrelationAnalysis
AnisotropyAnalysis
MultiscaleAnalysis
ComponentAnalysis
DiagnosticsAnalysis
MarkedAnalysisRun

Do not create a trait for each stage unless multiple implementations actually exist.

Each stage should:

* accept explicit inputs;
* return a typed result;
* not mutate unrelated global state;
* report telemetry through one mechanism;
* avoid importing the entire parent module.

11.2 Refactor spectrum module — ARCH-03

Split responsibilities:

Kernel

* centered binary mark Fourier power;
* centered continuous mark Fourier power;
* total phase sums;
* selected-subset phase sums.

Modes

* resolvable band;
* mode generation;
* shell assignment.

Permutation execution

* deterministic seed;
* sequential and parallel execution;
* scratch buffers;
* stratification strategy.

Shell aggregation

* mode-to-shell aggregation;
* median permutation baseline;
* eligibility.

Scalar summaries

* low-(k) excess;
* dominant frequency;
* characteristic scale;
* stability interval;
* low-(k) slope.

Result assembly

* envelope;
* scalar p-values;
* finite validation;
* domain result.

11.3 Create a full multimodal application run — ARCH-01

The production application service should perform once:

1. Input validation.
2. Transform fitting.
3. Registration QC.
4. Cell fusion.
5. Spatial index construction.
6. Graph construction.
7. Primary enrichment.
8. All configured null-model sensitivity analyses.
9. Cross-interaction curves.
10. Territory detection.
11. Territory profiles.
12. Territory comparisons.
13. Diagnostics.
14. Registration residuals.
15. Extrapolation analysis.
16. Telemetry.
17. Result assembly.
18. Artifact projections.

Return one MultimodalAnalysisRun.

11.4 Make CLI thin — ARCH-04

CLI responsibilities:

* parse arguments;
* resolve paths;
* load configuration;
* invoke input adapter;
* invoke application service;
* invoke output transaction;
* map errors to exit codes.

CLI must not:

* fit transforms;
* build graphs;
* calculate hulls;
* run permutations;
* construct scientific summaries;
* calculate QC metrics;
* write ad hoc domain sidecars.

11.5 Remove CLI feature gates from domain logic — BOUND-03

Move stratified enrichment and any similar functions out of #[cfg(feature = "cli")].

Feature flags may continue to gate:

* Clap;
* CSV;
* Parquet;
* image encoding;
* WSI integration;
* tracing subscriber.

They must not change analytical meaning unexpectedly.

11.6 Split cell-table responsibilities — BOUND-02

Create:

* domain HeCell, IhcCell, FusedCell or compact equivalents;
* label policy;
* CSV adapter;
* CellViT adapter;
* row validation.

Make label access allocation-free.

11.7 Separate shared metadata — PERF-09

Introduce a run-level metadata object:

pub struct AnalysisMetadata {
    pub case_id: String,
    pub timepoint: String,
    pub protein: String,
}

Do not repeat those strings in every fused cell unless serialization requires a flattened export.

For output, join shared metadata into rows during writing.

11.8 Split pre/post workflows — ARCH-06

Create distinct marked and multimodal pre/post services.

Share only:

* axis definitions;
* generic scalar delta helpers;
* comparison statistics;
* typed errors;
* territory matching utilities where semantics are identical.

Interpretation prose belongs in policy/report code.

Phase 4 exit criteria

* CLI does not calculate domain results.
* Transform and graph are built once per multimodal run.
* Library and CLI produce the same core analysis.
* Domain analysis no longer depends on the CLI feature.
* use super::* has been removed from refactored production modules.
* Marked and multimodal pre/post workflows are separated.
* All tests pass.

⸻

12. Phase 5 — Input, schema, and output architecture

12.1 Create a normalized decoded row

CSV and Parquet adapters should both produce:

pub struct DecodedCellRow {
    pub x_um: f64,
    pub y_um: f64,
    pub mark: u8,
    pub mark_probability: Option<f32>,
    pub tumor_probability: Option<f32>,
    pub nucleus_area_um2: Option<f32>,
    pub case_id: String,
    pub timepoint: String,
    pub protein: String,
    pub internal_control: Option<InternalControlState>,
    pub slide_id: Option<String>,
    pub section_id: Option<String>,
    pub stain_batch: Option<String>,
    pub block_id: Option<String>,
    pub region_id: Option<String>,
    pub slide_region: Option<String>,
    pub histologic_compartment: Option<String>,
    pub valid_tumor: bool,
    pub valid_ihc: bool,
    pub artifact_flags: ArtifactFlags,
    pub nonviable_flags: NonviableFlags,
    pub qc_bin: Option<u16>,
    pub component_id: Option<u32>,
    pub local_dab_od: Option<f32>,
    pub local_hematoxylin_od: Option<f32>,
}

Use enums instead of ambiguous strings where feasible.

12.2 Create one PatternBuilder

The builder owns:

* mask filtering;
* metadata consistency;
* optional dense-column consistency;
* categorical encoding;
* QC counters;
* exclusion policy;
* retained arrays;
* nearest-neighbor calculation;
* window finalization;
* invariant validation.

CSV and Parquet must not duplicate this logic.

12.3 Define one logical schema

Create one authoritative logical schema for cell inputs.

CSV and Parquet adapters may map physical details differently, but tests must prove equivalent logical input creates equivalent Pattern.

12.4 Correct Parquet export semantics — OUT-04

Choose distinct APIs:

write_pattern_roundtrip_parquet
write_filtered_pattern_export_parquet

A true round-trip writer must preserve:

* optional-field absence;
* metadata;
* categorical strata;
* QC columns where represented;
* no fabricated zero component IDs;
* no fabricated internal-control validity.

A filtered export may intentionally mark all rows as retained, but must:

* use an explicit name;
* record provenance;
* document that excluded source rows are absent;
* not claim full input round-trip.

12.5 Version result formats

Breaking schema cleanup should produce result format 0.3.

Requirements:

* do not silently emit changed fields under 0.2;
* add explicit version constants;
* update README and SPEC;
* update fixture tests;
* reject unsupported versions clearly.

Strongly consider a narrow 0.2 → 0.3 converter for existing result documents.

At minimum provide a migration document.

Remove:

* Task-derived compatibility aliases;
* placeholder public fields;
* multimodal fields from marked results;
* ambiguous z_or_power.

12.6 Version pre/post outputs — OUT-02

Use a versioned document such as:

AnalysisResult::MarkedPrePost(...)
AnalysisResult::MultimodalPrePost(...)

or a separately versioned comparison document.

Standardize input behavior:

* both marked and multimodal pre/post commands accept a result file or result directory;
* directory resolution is centralized;
* error messages are consistent.

12.7 Create an artifact plan

Before writing:

pub struct ArtifactPlan {
    pub result_document: ResultDocument,
    pub artifacts: Vec<PlannedArtifact>,
    pub manifest: RunManifest,
    pub telemetry: RunTelemetry,
}

Artifact generation must not depend on files already written to disk.

12.8 Make output transactional — OUT-03

Required sequence:

1. Validate output path and overwrite policy.
2. Create a temporary sibling directory.
3. Write all artifacts into the temporary directory.
4. Flush and close writers.
5. Validate required artifacts.
6. Write final result and manifest.
7. Rename the temporary directory into place.
8. Clean up on failure.

Do not leave a partially written directory that appears successful.

12.9 Unify telemetry and run manifests — DUP-07

* Result telemetry and timings.json must derive from one in-memory object.
* Do not write timings and then read them back.
* Define whether output-writing time belongs:
    * in result telemetry;
    * only in the run manifest;
    * or in a separate artifact-write summary.
* Document the choice.

12.10 Secure batch output paths — OUT-06

For manifest IDs:

* trim;
* reject blank;
* reject absolute paths;
* reject . and .. components;
* reject separators if IDs are meant to be single path components;
* canonicalize or normalize;
* verify final output remains inside the root.

Phase 5 exit criteria

* CSV and Parquet share one builder.
* Logical parity tests pass.
* Parquet export semantics are explicit.
* Result format is versioned correctly.
* Pre/post results are versioned.
* Output is transactional.
* One telemetry and manifest path exists.
* Batch paths cannot escape the output root.

⸻

13. Phase 6 — Spatial indexing and geometry optimization

This phase is mandatory before retaining large-cell-count claims.

13.1 Introduce one reusable spatial index — PERF-01

Provide deterministic operations:

nearest_neighbor(index) -> Option<Neighbor>
k_nearest(index, k) -> Vec<Neighbor>
within_radius(index, radius) -> Vec<Neighbor>
points_within_radius(x, y, radius) -> Vec<Neighbor>

Requirements:

* immutable point coordinates;
* stable original point indices;
* finite-coordinate validation;
* deterministic ordering by distance then index;
* defined duplicate-coordinate behavior;
* no hidden global state;
* no rebuilding per endpoint.

Choose the backend after evaluating:

* maintained Rust dependencies;
* license;
* deterministic behavior;
* radius query performance;
* kNN performance;
* memory;
* dependency policy.

An internal uniform grid may work well for bounded-radius queries, but kNN behavior must still be efficient and exact. Do not choose an implementation only because it is easy to write.

Record the choice in DECISIONS.md.

13.2 Replace nearest-neighbor scan

Implement mean nearest-neighbor distance through the shared index.

Required differential tests:

* random points;
* grid;
* duplicate coordinates;
* collinear points;
* two points;
* large coordinate magnitudes;
* comparison with brute force.

13.3 Replace graph construction

Radius graph

* query neighbors within radius;
* add only target > source;
* calculate each distance once.

kNN graph

* indexed kNN query;
* normalize undirected pairs;
* preserve deterministic ties.

Combined graph

* union radius and kNN edges without recomputing distances unnecessarily.

Differentially test against the old brute-force implementation on small inputs.

13.4 Optimize pair correlation — PERF-04

Create a PairCorrelationPlan:

struct PairCorrelationPlan {
    bins: Vec<PairBin>,
    bin_edges: Vec<f64>,
    pair_counts: Vec<usize>,
}

Each PairBin should identify:

* source;
* target;
* bin index.

Build geometry once with radius queries.

For each observed or permuted label vector:

* calculate centered marks;
* iterate the fixed pair plan;
* accumulate by bin;
* do not recalculate distances.

Represent empty bins explicitly.

13.5 Optimize territory detection — PERF-05

For each required physical scale:

* precompute or query neighborhoods through the spatial index;
* reuse neighborhood membership across candidate labels and permutations;
* avoid scanning every point for every candidate.

For permutation-based territory-count nulls:

* reuse geometry;
* update only label-dependent counts.

13.6 Optimize territory profiles — PERF-06

Use spatial radius queries around each territory.

Do not scan all fused cells per territory.

13.7 Reuse geometry across endpoints

Create application-owned geometry plans where justified:

SpatialIndex2D
PairCorrelationPlan
RasterAssignmentPlan
TerritoryNeighborhoodPlan
Graph
KGridPlan

Do not place all plans into a monolithic cache with ambiguous lifetime. Keep them explicit.

13.8 Performance acceptance

Algorithmic requirements:

* nearest-neighbor path must no longer be (O(n^2));
* radius graph should scale approximately with (n + e);
* kNN graph should use indexed queries;
* pair correlation should scale with pairs inside the requested radius;
* profile lookup should scale with returned neighborhood size;
* geometry must not be rebuilt per permutation.

Scaling tests should compare at least n, 2n, and 4n.

For fixed density and bounded radius:

* doubling n should not approach a 4× runtime increase for indexed radius operations;
* record actual ratios;
* investigate major deviations.

Phase 6 exit criteria

* spatial_index.rs contains a real index or is removed.
* Brute-force differential tests pass.
* Nearest-neighbor, graph, pair correlation, territories, and profiles use indexed geometry.
* Performance scaling is documented.
* Memory is proportional to points plus required edges/plans, not all possible pairs.

⸻

14. Phase 7 — Spectral and permutation optimization

14.1 Use a compact mark-field abstraction

Avoid duplicated binary and probabilistic workflows.

A simple enum is preferable to a complex trait hierarchy:

pub enum MarkField<'a> {
    Binary(&'a [u8]),
    Continuous(&'a [f32]),
}

Shared logic should include:

* length validation;
* mean calculation;
* mode evaluation;
* permutation execution;
* shell aggregation;
* result construction.

Keep optimized binary subset calculations where materially faster.

14.2 Aggregate modes into shells early — PERF-08

Current large storage should be replaced with:

observed shell powers
B × shell permutation powers

rather than:

B × mode permutation powers

Compute mode contributions in chunks, accumulate directly into shell totals, and discard mode scratch when possible.

ERL envelopes require curve-level shell values, not every individual mode value.

14.3 Make k_chunk_modes real

Use performance.k_chunk_modes to cap:

* mode scratch;
* phase precomputation;
* per-thread buffers.

Test:

* chunk size 1;
* typical chunk size;
* chunk larger than mode count;
* deterministic equality across chunk sizes within numerical tolerance.

14.4 Use contiguous matrices

Replace large Vec<Vec<f64>> structures with a contiguous matrix abstraction or flat vector.

Requirements:

* checked dimensions;
* row accessors;
* no unnecessary per-row allocation;
* predictable memory estimation.

14.5 Reuse scratch buffers

For each permutation worker:

* selected indices;
* shuffled labels or values;
* shell sums;
* mode chunk scratch;
* temporary rank vectors where safe.

Do not allocate a new full vector inside every inner loop.

14.6 Avoid repeated count scans

Pattern::n_marked() currently scans marks.

Create an analysis context with cached:

* n_cells;
* n_marked;
* n_unmarked;
* prevalence;
* geometry summary.

Do not store an invalidatable cache inside a freely mutable Pattern unless mutation is removed or controlled.

14.7 Parallelism and determinism

Required tests:

* one thread versus multiple threads;
* strict reproducibility;
* repeated run equality;
* stable permutation order;
* stable seed domain separation;
* no thread-count-dependent results beyond documented floating-point tolerance.

Parallel execution should not alter which random permutation corresponds to an index.

14.8 Reuse permutation geometry, not necessarily permutation samples

Centralize permutation generation and seed derivation.

Do not automatically force every endpoint to share the same permutation sample unless the inferential design calls for it.

Document whether endpoints use:

* the same permutation index and labels;
* endpoint-specific domain-separated permutations;
* stratified versus unstratified nulls.

14.9 Optimize probabilistic-mark spectrum

Evaluate chunked precomputation of phase terms subject to the memory budget.

Potential strategy:

* process modes in chunks;
* compute or cache cosine/sine terms for the current chunk;
* perform dot products for shuffled values;
* discard chunk cache;
* aggregate into shells.

Benchmark before and after. Do not introduce a phase table larger than the configured memory budget.

14.10 Preserve independent numerical checks

For unchanged scientific behavior:

* compare old and new observed powers;
* compare shell powers;
* compare envelope values;
* compare scalar readouts;
* compare p-values.

Any intentional numerical change must have:

* a documented defect;
* a new reference;
* a migration note.

Phase 7 exit criteria

* structure_factor.rs is decomposed.
* Binary and probabilistic paths share appropriate logic.
* k_chunk_modes affects execution.
* Large permutation matrices store shell-level data.
* Scratch allocations are reduced.
* Determinism tests pass.
* Numerical differential tests pass.
* Spectrum benchmarks and memory measurements improve or have documented tradeoffs.

⸻

15. Phase 8 — Multimodal model completion

15.1 Put all configured null models in the production result

The configuration currently advertises multiple null models.

Required outcome:

* every configured null model is calculated by the application service;
* results are part of the versioned result model or a clearly linked artifact model;
* library users receive the same analysis as CLI users;
* the primary null is explicitly identified;
* sensitivity results are not hidden in CLI-only sidecars.

15.2 Complete or remove territory-profile fields — MODEL-02

For:

TerritoryProfile.enrichment
TerritoryProfile.cross_curves

Choose:

* actually calculate them;
* or remove them from result format 0.3.

Do not emit empty vectors implying successful calculation.

15.3 Complete or remove QC overlap

For each territory type:

* define the QC structure being overlapped;
* define denominator and area/cell weighting;
* calculate the fraction;
* or remove the field.

Do not use constant zero.

15.4 Add multimodal telemetry

Measure:

* registration fit;
* registration QC;
* fusion;
* spatial-index build;
* graph build;
* enrichment per null;
* cross curves;
* territory detection;
* profiles;
* comparison;
* diagnostics;
* artifact projections.

Return telemetry through the same run model used by marked analysis.

15.5 Separate territory types — MODEL-01

Example:

pub struct ResidualTerritory {
    pub center_x_um: f64,
    pub center_y_um: f64,
    pub radius_um: f64,
    pub analysis_scale_um: f64,
    pub residual_score: f64,
    pub supporting_marked_cells: usize,
    pub component_id: Option<u32>,
    pub qc_overlap_fraction: Option<f64>,
}
pub struct NeighborhoodTerritory {
    pub center_x_um: f64,
    pub center_y_um: f64,
    pub radius_um: f64,
    pub supporting_abnormal_cells: usize,
    pub cluster_id: u32,
    pub below_registration_resolution: bool,
}

15.6 Improve label representation

Use compact label IDs during graph calculations.

Convert to strings only for:

* serialized results;
* reports;
* user-facing diagnostics.

Avoid repeated String allocation in edge loops and permutations.

15.7 Registration extrapolation

Move convex hull and point-in-hull logic to a reusable geometry module.

Test:

* empty landmarks;
* one/two landmarks;
* collinear landmarks;
* boundary points;
* clockwise and counterclockwise hulls;
* duplicate points;
* numerical tolerance.

Define whether fewer than three hull points mean:

* extrapolation cannot be assessed;
* all points are considered inside;
* a separate status.

Do not silently classify all points as inside without a documented state.

Phase 8 exit criteria

* Multimodal result contains all configured analyses.
* No CLI-only scientific result remains.
* Placeholder profile and QC fields are implemented or removed.
* Multimodal telemetry exists.
* Territory result types are semantically distinct.
* Label allocation is reduced.
* Registration extrapolation has defined degenerate behavior.

⸻

16. Phase 9 — Rewrite validation so it validates production code

16.1 Delete direct outcome synthesis — COR-01

Remove the current model in which validation generates:

detected
false_positive
below_registration_resolution
equivalent

directly from random arithmetic.

A scenario generator may generate input data. It may not generate the expected analysis result.

16.2 Build real multimodal scenarios

Each replicate must construct:

* H&E cells;
* IHC cells;
* landmarks;
* known transform;
* optional registration noise;
* case metadata;
* production configuration.

Then invoke:

MultimodalEngine

or the replacement public application API.

Required scenarios

Negative controls

1. Random labels with no spatial association.
2. Two unrelated MMR-abnormal territories.
3. Immune cells independent of MMR territories.
4. Noisy registration without true association.
5. Pre/post equivalent organization.

Positive controls

1. Nearby related MMR territories.
2. Immune-enriched MMR territory.
3. Known cross-interaction enrichment.
4. Pre/post changed spatial organization.
5. Registration residuals above configured threshold.
6. Associations below registration resolution.

Edge cases

1. Too few landmarks.
2. Degenerate landmarks.
3. Empty H&E or IHC set.
4. No abnormal cells.
5. Sparse graph.
6. Zero expected edge count.
7. Multiple cell classes.
8. Multiple configured null models.
9. Rotation requiring true rigid registration.
10. Affine deformation requiring affine registration.

16.3 Validate actual result fields

Outcomes must be derived from production results, for example:

* detected territory count;
* enrichment p-value;
* q-value;
* comparison result;
* registration status;
* below-resolution status;
* equivalence conclusion.

No expected status flag may be inserted into the result.

16.4 Use statistically meaningful acceptance

For calibration scenarios:

* use enough replicates;
* calculate a confidence interval;
* compare the interval with a justified nominal target;
* distinguish smoke validation from formal calibration.

Do not allow type-I error limits such as 60% and call the system calibrated.

A smoke suite may use fewer replicates, but must be named a smoke suite and must not make calibration claims.

16.5 Rewrite marked validation hacks

Remove:

* manual insertion of expected flags;
* unconditional passing;
* tautological conditions such as non-negative counts;
* thresholds selected only because current output passes.

For each marked scenario:

* generate input;
* run production analysis;
* judge actual output;
* record reasoned thresholds.

16.6 Independent references

Retain and expand independent fixtures for:

* ERL global envelopes;
* scalar permutation p-values;
* rigid and affine registration;
* periodogram or wavelet method if those names remain;
* multiple-testing correction;
* known spatial graphs;
* Parquet/CSV parity.

Reference fixtures should be checked in with:

* generation script;
* external implementation/version;
* parameters;
* expected tolerance.

16.7 Validation output honesty

Validation result should report:

replicates attempted
replicates completed
replicates failed
failure reasons
detection rate
false-positive rate
confidence interval
threshold
pass/fail
seed
configuration
engine version

Do not hide failed replicates or drop undefined outputs.

Phase 9 exit criteria

* Every validation scenario invokes production analysis.
* No direct outcome booleans remain.
* No manually injected flags remain.
* Statistical acceptance criteria are documented.
* Smoke and calibration suites are clearly distinguished.
* Validation output records failures and denominators.
* CI no longer gives false confidence through circular tests.

⸻

17. Phase 10 — Result model and API cleanup

17.1 Split result types — ARCH-08

Create focused modules:

output/common.rs
output/marked.rs
output/multimodal.rs
output/prepost.rs
output/diagnostics.rs
output/artifacts.rs

Marked results must not include:

* registration placeholders;
* fused-cell placeholders;
* multimodal neighborhood placeholders.

Multimodal results must not include marked-spectrum placeholders.

Common data should be shared only where semantics are identical.

17.2 Use typed statuses — MODEL-03

Replace free strings such as:

"ok"
"suppressed"
"multimodal_summary"
"coarse_clustered"

with enums.

Serde may still emit snake-case strings.

17.3 Remove unimplemented fields

For every public field, require:

* producer;
* consumer;
* test;
* documentation.

If any is absent, either implement or remove the field.

17.4 Make analysis availability consistent

Continue using a tagged availability model, but define consistent rules for:

* Available;
* Disabled;
* NotApplicable;
* InsufficientData;
* computation error.

Do not return Available(Vec::new()) when empty means unavailable unless the empty set is a valid successful result.

17.5 Public API review

Review every public re-export in lib.rs.

For each item, record:

* why it is public;
* stability expectation;
* documentation;
* test coverage.

Reduce the public API to supported domain entry points and result models.

Do not expose internal orchestration merely for convenience.

Phase 10 exit criteria

* Result modules are cohesive.
* String statuses are replaced.
* No placeholder fields remain.
* Availability semantics are consistent.
* Public API is intentionally documented.
* Result format 0.3 round-trips.

⸻

18. Phase 11 — God-file and code-shape cleanup

Do this after behavior and boundaries are stable.

18.1 Review the known god files

At minimum:

src/spectra/structure_factor.rs
src/api.rs
src/api/stages.rs
src/api/assembly.rs
src/cli/multimodal/analyze.rs
src/config.rs
src/validation.rs
src/prepost/deltas.rs
src/output/writer.rs
src/output/result_types.rs
src/io/parquet.rs

For each, record:

* current responsibilities;
* target responsibilities;
* dependencies;
* moved code;
* deleted duplication.

18.2 Function-size review

Flag functions over roughly 80–100 lines for review.

Do not split a cohesive numerical loop solely to meet a line limit. Split when the function performs multiple responsibilities or hides reusable invariants.

18.3 File-size review

Flag production files over roughly 500–800 lines.

This is a review trigger, not an automatic failure. A long declarative schema file may be acceptable; a long orchestration file is not.

18.4 Remove ceremonial files

Merge tiny helpers where they form one cohesive subject.

Examples may include:

* scale/radius helpers;
* simple taper helpers;
* simple residual-statistic helpers.

Do not merge unrelated code merely to reduce file count.

18.5 Explicit dependency review

No refactored production module should use use super::*.

Imports should make dependencies visible.

Phase 11 exit criteria

* No distributed god workflow remains.
* Large files have one coherent responsibility.
* One-function files are justified or merged.
* Explicit imports are used.
* No task-scaffolding comments remain.

⸻

19. Phase 12 — Performance hardening and regression protection

19.1 Benchmark methodology

Run benchmarks on the same machine and profile.

Record:

* CPU;
* memory;
* OS;
* Rust version;
* build profile;
* thread count;
* Git SHA.

Use repeated samples and report confidence/noise.

19.2 Required performance comparisons

Before versus after:

Workload	Metrics
nearest neighbor	wall time, allocations, scaling
radius graph	wall time, edge count, memory
kNN graph	wall time, edge count, memory
pair correlation	plan build, observed evaluation, permutation evaluation
marked territories	plan build, observed, permutation
multimodal territories	wall time, memory
territory profiles	wall time versus number of territories
structure factor	observed, binary permutations, continuous permutations
complete marked run	wall time, memory
complete multimodal run	wall time, memory
CSV load	throughput, memory
Parquet load	throughput, memory
output writing	wall time, peak memory

19.3 Scaling acceptance

For indexed fixed-density radius workloads:

* runtime growth must be substantially below quadratic;
* doubling input size should not consistently produce approximately 4× runtime;
* report actual edge counts because output-sensitive complexity matters.

For spectrum:

* report scaling with:
    * cells;
    * modes;
    * shells;
    * permutations;
    * threads;
    * chunk size.

19.4 Memory regression

Use the existing memory instrumentation or DHAT.

Verify:

* shell-level storage rather than mode-level permutation matrices;
* scratch reuse;
* no full result cloning;
* no repeated metadata strings per cell where avoidable;
* no unbounded phase cache;
* configured memory budget enforced against actual strategy.

19.5 Million-cell workload

After spatial indexing:

* generate the fixture by streaming rows to disk;
* do not build the entire CSV in a String;
* separate input decoding time from nearest-neighbor computation;
* run as a manual or scheduled benchmark;
* record completion and peak memory.

Do not retain “1m cells” in a benchmark name if the full path cannot practically complete.

19.6 Regression policy

As a default:

* investigate benchmark regressions above 10%;
* block unexplained regressions above 20%;
* permit regressions only where correctness or memory safety justifies them;
* record the reason.

Do not make noisy wall-clock thresholds the only CI gate. Add algorithmic scaling and operation-count assertions where possible.

Phase 12 exit criteria

* Performance report completed.
* Spatial operations are demonstrably subquadratic for appropriate workloads.
* Spectrum memory is reduced.
* Full benchmark naming is credible.
* No unexplained major regression remains.

⸻

20. Phase 13 — CI, documentation, and release readiness

20.1 CI checks

The final CI should cover:

cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo nextest run --locked --all-features
cargo test --locked --doc --all-features
cargo check --locked --no-default-features
cargo test --locked --features wsi,cli --test wsi_integration
cargo audit
cargo deny check advisories licenses bans sources
cargo machete
cargo +nightly fuzz check

Add targeted tests for:

* result-format conversion;
* CSV/Parquet parity;
* output transaction failure;
* deterministic parallelism;
* spatial-index differential equivalence;
* real multimodal validation smoke suite.

Do not run an expensive formal calibration suite on every pull request. Use scheduled CI for large replicate counts.

20.2 Fuzzing

Retain existing fuzz targets and add candidates for:

* result-document parser;
* config parser;
* normalized cell row;
* axis definitions;
* output-path validation;
* region request validation;
* spatial-index query edge cases.

20.3 Documentation

Update:

* README.md;
* SPEC.md;
* configuration example;
* multimodal configuration example;
* Rust examples;
* result schema documentation;
* migration guide;
* validation methodology;
* performance methodology;
* dependency advisories.

Remove unsupported claims.

Explicitly document:

* what the analysis can conclude;
* what it cannot conclude;
* null models;
* scale definitions;
* registration model;
* territory definition;
* missing/undefined result semantics;
* result version;
* validation limitations.

20.4 Architecture decisions

At minimum record decisions for:

1. Spatial-index backend.
2. Wavelet rename versus real implementation.
3. Result format 0.3.
4. Rigid/affine/scale-translation transform model.
5. Primary versus sensitivity null model.
6. Shared metadata representation.
7. Output transaction model.
8. Component mode semantics.
9. Finite-statistic policy.
10. Pre/post axis identity/tolerance.

20.5 Release readiness

Before declaring completion:

cargo package --locked
cargo test --locked --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings

Test release archives and binary commands.

Verify:

* fresh output directory;
* existing output directory;
* failed run cleanup;
* marked analysis;
* multimodal analysis;
* marked pre/post;
* multimodal pre/post;
* validation smoke;
* WSI inspection and extraction.

Phase 13 exit criteria

* CI is green.
* Documentation no longer overclaims.
* Migration is documented.
* Release package succeeds.
* End-to-end smoke tests pass.

⸻

21. Mandatory regression-test matrix

Implement or retain tests covering the following.

Registration

rigid_identity
rigid_translation
rigid_rotation
rigid_rotation_translation
rigid_preserves_distances
rigid_rejects_scaling
rigid_rejects_degenerate_geometry
affine_recovers_known_transform
registration_qc_known_residuals
registration_extrapolation_boundary

Inference

permutation_high_tail_inclusive_ties
permutation_low_tail_inclusive_ties
permutation_two_sided_equal_tail
permutation_rejects_nonfinite
erl_matches_checked_oracle
erl_pointwise_ties
erl_identical_curves
erl_eligibility_mask
benjamini_hochberg_known_vector

Confounding

unstratified_significant_stratified_not_significant
both_significant
neither_significant
homogeneous_strata
missing_stratum
stratified_result_is_not_recomputed_primary

Serialization

sparse_enrichment_roundtrip
undefined_z_score_roundtrip
result_v03_roundtrip
result_v02_to_v03_conversion
prepost_result_roundtrip
all_result_floats_are_finite
unknown_result_version_rejected
unknown_fields_rejected

I/O

csv_parquet_equivalent_rows_produce_equal_pattern
optional_absence_preserved
partial_dense_column_rejected
internal_control_fraction_correct
artifact_fraction_correct
nonviable_fraction_correct
metadata_mismatch_rejected
filtered_export_is_explicitly_not_full_roundtrip

Spatial indexing

nearest_neighbor_matches_bruteforce
radius_query_matches_bruteforce
knn_matches_bruteforce
duplicate_coordinate_ties
graph_matches_bruteforce
pair_plan_matches_bruteforce
territory_neighbors_match_bruteforce
deterministic_query_order

Spectrum

binary_kernel_matches_dense_reference
continuous_kernel_matches_reference
shell_aggregation_known_modes
chunk_sizes_produce_same_result
parallel_and_serial_match
permutation_order_stable
shell_level_storage_matches_previous_valid_output
probabilistic_marks_finite

Multimodal

application_builds_transform_once
application_builds_graph_once
library_and_cli_core_results_match
all_configured_null_models_present
profile_fields_are_computed_or_absent_from_schema
territory_types_are_distinct
label_access_is_allocation_free_in_hot_path
multimodal_telemetry_populated

Validation

multimodal_validation_calls_public_engine
negative_control_calibrates
positive_control_detects_signal
rotation_scenario_requires_real_rigid_transform
registration_jitter_uses_actual_registration_output
prepost_equivalence_uses_actual_comparison
no_manual_status_flag_injection
failed_replicates_are_reported

Output

failed_artifact_write_does_not_commit_final_directory
manifest_matches_written_artifacts
result_and_timings_use_same_telemetry
file_and_directory_prepost_inputs_are_consistent
batch_id_cannot_escape_output_root

Component modes

pooled_only
separate_only
both
auto_pooled
auto_separate_or_both
mode_selection_reason_reported

⸻

22. Optimization-specific implementation notes

These are expected engineering directions, not optional suggestions.

22.1 Pair-correlation permutations

Do not rebuild pair geometry for every permutation.

Correct pattern:

build pair/bin plan once
calculate observed curve
for each permutation:
    generate labels
    evaluate labels over fixed pair/bin plan

22.2 Raster permutations

Create a fixed cell-to-raster assignment plan.

Correct pattern:

build raster dimensions and cell bin indices once
for each label vector:
    clear raster
    accumulate centered marks through cell bin indices

Do not recalculate coordinate-to-pixel mapping for every permutation.

22.3 Territory null permutations

For fixed geometry and scale:

precompute candidate neighborhoods
for each permutation:
    count marked cells in each neighborhood
    calculate residuals
    select non-overlapping candidates

Do not run all-pairs distance checks inside every null iteration.

22.4 Graph permutations

Represent labels compactly.

For each configured label pair:

* pre-resolve label IDs;
* count edge pairs using IDs;
* avoid repeated string comparisons.

22.5 Spectrum permutations

Store shell powers, not all mode powers, when mode-level null values are not required downstream.

Use checked contiguous indexing.

22.6 Result writing

Avoid:

result.clone()
fused_cells.clone()

merely to satisfy ownership during writing.

Refactor APIs to borrow or consume.

22.7 Reports and sidecars

Generate all projections from the same result/run model.

Do not recalculate scientific values in Markdown, CSV, or Parquet writers.

⸻

23. Prohibited shortcuts

Do not:

1. Rename a fake algorithm but leave old misleading report fields elsewhere.
2. Add a wrapper around duplicate functions while keeping both implementations.
3. Move code from CLI into a module still compiled only with cli.
4. Replace INFINITY with a large arbitrary finite number.
5. Replace missing statistics with zero.
6. Make multimodal validation call a mocked engine that returns predetermined outcomes.
7. Keep direct outcome synthesis as a “fast validation path.”
8. Preserve exact float equality because current fixtures happen to match.
9. add a spatial-index trait while retaining brute-force implementations in production.
10. keep the million-cell benchmark name without making the path practical.
11. introduce unsafe code for speed without an independently justified review.
12. silently alter result format 0.2.
13. keep unimplemented fields “for future compatibility.”
14. create dozens of tiny files while leaving the same parent wildcard coupling.
15. claim calibration based on a smoke suite.
16. skip benchmarks after changing spatial or permutation algorithms.
17. accept a changed numerical result without a reference test or documented defect.
18. hide failed validation replicates.
19. calculate a transform or graph twice because it is convenient.
20. leave task-number comments in production code.

⸻

24. Definition of done

The remediation is complete only when all conditions below are met.

Correctness

* True rigid registration exists and is tested.
* Stratified confounding compares distinct null analyses.
* No JSON-facing non-finite statistic can be produced.
* Missing and undefined states are typed.
* Component modes have distinct semantics.
* Internal-control and exclusion fractions use correct denominators.
* Pre/post axes use structural identity or tolerance.
* Batch paths cannot escape output root.

Scientific integrity

* No false MODWT, DoG, Bartlett, rigid, or validation naming remains.
* Generic analysis no longer emits unjustified MMR-specific claims.
* Every retained established algorithm name has a documented implementation and reference test.
* Validation exercises production code.
* No validation flag or pass state is manually injected.

Architecture

* CLI is a thin adapter.
* Domain types do not open files.
* Domain behavior is not gated by the CLI feature.
* Transform and graph are built once.
* CSV and Parquet use one PatternBuilder.
* Output writing performs no scientific calculations.
* Marked and multimodal result types are separated.
* Pre/post workflows are separated.
* No distributed god workflow remains.
* No use super::* remains in refactored production modules.

Reuse

* One median implementation.
* One scalar permutation p-value implementation.
* One enrichment core.
* One telemetry model.
* One run-manifest builder.
* One logical cell schema.
* One spatial index.
* One geometry definition per named quantity.

Performance

* Nearest-neighbor search is indexed.
* Radius graph is indexed.
* kNN graph is indexed.
* Pair-correlation geometry is reused.
* Territory geometry is reused.
* Profile membership is indexed.
* Spectrum permutations store shell-level data where possible.
* k_chunk_modes is operational.
* Large scratch buffers are reused.
* Shared metadata is not redundantly cloned per cell where avoidable.
* Performance report demonstrates nonquadratic spatial scaling.
* Million-cell benchmark claim is credible or removed.

Output and API

* Result format is versioned correctly.
* Pre/post results are versioned.
* Output is transactional.
* Result and timing artifacts do not contradict each other.
* Parquet export semantics are explicit.
* Placeholder public fields are implemented or removed.
* Statuses are typed.
* Public API is documented and intentional.

Verification

* Formatting passes.
* Clippy passes with warnings denied.
* All-feature tests pass.
* No-default-feature check passes.
* Documentation tests pass.
* WSI integration tests pass.
* Dependency checks pass.
* Fuzz targets build.
* Benchmarks run.
* Release package builds.
* Findings matrix has no undocumented open critical issue.

⸻

25. Final deliverables

Produce all of the following.

25.1 Code

* corrected algorithms;
* reorganized architecture;
* optimized spatial and permutation paths;
* versioned results;
* real validation;
* tests and benchmarks.

25.2 Documentation

docs/refactor/MASTER_PLAN.md
docs/refactor/STATUS.md
docs/refactor/DECISIONS.md
docs/refactor/FINDINGS_MATRIX.md
docs/refactor/PERFORMANCE_BASELINE.md
docs/refactor/PERFORMANCE_FINAL.md
docs/result-format-0.3.md
docs/migration-0.2-to-0.3.md
docs/validation-methodology.md

25.3 Closure report

The final report must include:

Executive summary

* what was repaired;
* what was deleted;
* what was renamed;
* remaining risk.

Findings closure matrix

ID	Resolution	Files	Tests	Benchmark	Commit

Architecture before and after

* dependency boundaries;
* removed duplicate workflows;
* god-file reduction;
* public API changes.

Scientific changes

* renamed algorithms;
* corrected registration;
* corrected null-model interpretation;
* validation redesign;
* any changed numerical semantics.

Performance

* baseline and final results;
* scaling ratios;
* memory changes;
* remaining bottlenecks.

Result-format migration

* removed fields;
* renamed fields;
* converter behavior;
* compatibility limitations.

Verification commands

List every final command and its actual result. Do not write “all tests pass” without the command output or recorded exit status.

Unresolved issues

State all unresolved defects directly. Do not downgrade them through vague wording.

⸻

26. Final execution order

Use this order unless a verified dependency requires a small adjustment:

Phase 0  Baseline and reproduce defects
Phase 1  Shared finite/statistical/seed foundations
Phase 2  Critical correctness fixes
Phase 3  Scientific naming and algorithm integrity
Phase 4  Application and CLI boundary refactor
Phase 5  Input/output/schema architecture
Phase 6  Spatial indexing and geometry optimization
Phase 7  Spectrum and permutation optimization
Phase 8  Complete multimodal model
Phase 9  Rewrite real validation
Phase 10 Result model and public API cleanup
Phase 11 God-file and code-shape cleanup
Phase 12 Performance hardening
Phase 13 CI, documentation, and release readiness

Do not start with a cosmetic directory rewrite. Start by reproducing defects and establishing tests. Do not claim completion until the findings matrix, performance report, migration documentation, and full verification suite are complete.

