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
- Cache identity: a deterministic content key over node version/specification, input/config content, execution policy, and implementation identity. Cache support is added only for the demonstrated slice.
- Failure: invalid/cyclic graph, missing/digest-mismatched input, resource failure, or analysis error remains an error; no successful node/result entry is committed.
- Non-goals: broad plugin system, remote scheduler, general schema registry, scientific migration, or arbitrary task runner.
