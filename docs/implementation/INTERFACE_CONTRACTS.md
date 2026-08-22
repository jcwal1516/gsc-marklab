# Interface contracts

Status: characterization freeze for WS-A. Exact field/symbol inventory is active under A-03.

## IC-0001 — Current marked analysis compatibility contract

- Input: strict current `Config` plus a finite `Pattern` loaded through supported CSV/Parquet paths.
- Observable output: result-format 0.3 marked result and referenced artifacts; current CLI/library parity.
- Scientific meaning: structure-factor, centered mark-pair covariance, anisotropy, and named multiscale residual diagnostics under the configured fixed-position label-null policies.
- Preserved invariants: deterministic seeds, finite serialization, typed availability, memory budgets, transactional final output.
- Prohibited reinterpretation: these outputs are not Ripley K/L, classical g, wavelets, tissue domains, patient-level inference, or equivalence.
- WS-B rule: compatibility output must be characterized before any move and remain identical unless a versioned breaking migration is explicitly approved.

## IC-0002 — Current multimodal compatibility contract

- Input: current multimodal config plus H&E/IHC cells, labels, and optional landmarks/transform settings.
- Observable output: result-format 0.3 multimodal summaries/artifacts and CLI behavior.
- Scientific meaning: registered coordinate-frame fusion, graph-edge enrichment, raw cross-label pair-count curves, MMR-abnormal territories, and nearby cell-type profiles.
- Preserved invariants: one reusable index/graph plan, transform QC, deterministic nulls, typed unavailable states.
- Prohibited reinterpretation: no same-cell identity, biological correspondence, cross-K/cross-g, general niche/domain, or causal signaling.

## IC-0003 — Current pre/post and margin contract

- Input: compatible already-aggregated marked or multimodal results.
- Observable output: aligned descriptive comparisons, explicitly approximate pooled-bin diagnostic, and descriptive margin assessment.
- Biological unit: not inferred by this layer.
- Prohibited reinterpretation: no patient-level population difference, noninferiority, or equivalence claim.

## IC-0004 — Result-format 0.3 and artifact boundary

- Stable envelope: strict versioned tagged results with unknown-field rejection and finite persisted values.
- Unavailability: typed state/reason, never NaN, infinity, empty-success, or numeric zero sentinel.
- Artifacts: large/local tables and curves remain referenced artifacts; final writes are transactional.
- Migration: semantics that are absent in 0.3 cannot be invented during conversion.

## IC-0005 — WS-A no-production-change boundary

- Allowed tracked writes: root `AGENTS.md` and `docs/implementation/**`.
- Baseline commands may create ignored build, benchmark, fuzz, package, and smoke outputs.
- Any source, test, manifest, dependency, schema, or workflow change stops WS-A scope and requires a recorded task/decision.

## IC-0006 — WS-B compatibility facade

- The existing crate-root API listed in `src/lib.rs` remains callable from package `marklab`.
- Existing `marklab` CLI command spellings and arguments remain accepted.
- Configuration 0.2 remains strict and is not repurposed as project/workflow configuration.
- Result-format 0.3 DTOs and artifact projections remain closed and unchanged.
- A compatibility path calls each existing canonical scientific implementation; it may not copy a formula or construct a second output projection.
- Mechanical source movement, if selected, is isolated from semantic changes and must retain Git history.

## IC-0007 — First project/workflow vertical slice

- Input: an immutable reference to the current marked-analysis inputs and configuration, plus one typed workflow node that owns the invocation.
- Output: the same current `MarkedPatternResult`/0.3 result and artifacts as direct `AnalysisEngine` execution.
- Cache identity: a deterministic content key over node version/specification, input/config content, declared execution policy, scheduler retained-artifact limit, and implementation identity. Cache support is added only for the demonstrated slice.
- Canonical output: a miss reaches a decode/re-encode fixed point before commit; a hit verifies digest/length and decodes the exact retained bytes. The inline cap bounds retained artifact size, not codec peak allocation.
- Failure: invalid/cyclic graph, missing/digest-mismatched input, resource failure, unstable codec, or analysis error remains an error; no successful node/result entry is committed.
- Non-goals: broad plugin system, remote scheduler, general schema registry, scientific migration, or arbitrary task runner.

## IC-0008 — Typed in-memory cohort hierarchy

- Identity: opaque, exact, 1–255-byte typed IDs for site, patient, timepoint, specimen, block, slide, section, core, region, cell, and patch; text never supplies parentage or replication.
- Relations: one enumerated containment parent plus an independent biological source for biological subsamples and technical replicates. A donor TMA core may therefore belong to a donor unit while being contained by a recipient slide.
- Replication safety: only patient/specimen objects may be declared biological units; cell/patch/site/timepoint objects are structural, and lower physical objects cannot become independent units from row count.
- Nested levels: nearest resolution remains specific, while explicit lineage membership permits downstream selection of an enclosing declared patient above biological specimens. C-01 itself performs no randomization or population inference.
- Repeated designs: declarations contain at least two distinct same-kind observations, one set per biological-unit/observation-kind pair, and globally unique observation membership. Retained order is deterministic but not temporal.
- Persistence: this is an immutable in-memory 0.1 boundary installed once into `MarklabProject`. Serialization, schema evolution, cross-artifact drift, and result-format 0.4 are deferred to C-03.
