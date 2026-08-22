# Phase 11 Code-Shape Audit

Audit SHA: `e5cf7e01443578d50293fb02c903dfa3a17ea21c`

This audit treats 500–800 lines per production file and 80–100 lines per
function as review triggers, not mechanical limits. Test-only modules are
listed separately from production ownership. A retained long function must
own one algorithm, state machine, or ordered application stage; otherwise it
was split.

## Named god-file review

| Original path | Reproduced responsibilities | Current owner and dependencies | Moved or deleted | Disposition |
| --- | --- | --- | --- | --- |
| `src/spectra/structure_factor.rs` | Fourier kernels, mode planning, shell aggregation, permutation execution, summaries, result assembly, tests | The 498-line facade has about 60 production lines; `kernel`, `modes`, `shells`, `permutation`, and `summaries` own the mathematical and execution boundaries explicitly | Phase 4/7 moved every production responsibility; the remaining length is numerical regression tests | Fixed under ARCH-03; no Phase 11 move |
| `src/api.rs` | Validation, planning, all endpoint computation, interpretation, timing, and assembly | The marked application coordinator delegates to validation/QC, component planning, spectrum, mark-pair, multiscale/spatial, diagnostics, interpretation, and assembly modules; dependencies are explicit | `stages.rs` was deleted; mark-pair and multiscale logic moved to dedicated stages; interpretation moved out of assembly | Fixed under ARCH-02/ARCH-09 |
| `src/api/stages.rs` | Mark-pair covariance, scale-energy envelope/nulls, residual-territory inference, raster sizing, and periodogram policy | No longer exists. `mark_pair_stage`, `multiscale_stage`, and `spatial_stage` own those subjects | Deleted the 570-line catch-all and moved its two geometry-reuse tests to the spatial coordinator | Fixed under ARCH-09 |
| `src/api/assembly.rs` | Main result DTO, spectrum conversion, null-sensitivity conversion, component-mode suppression, interpretation | The 392-line module now owns result assembly only. Top-level assembly is 92 lines; spectrum section construction and component-mode suppression are focused helpers | Interpretation policy moved to a 46-line module; the prior 227-line assembler was reduced | Fixed under ARCH-09 |
| `src/cli/multimodal/analyze.rs` | Transform, graph, nulls, hulls, residuals, scientific sidecars | The 104-line CLI adapter loads inputs/config, invokes `MultimodalEngine`, and hands the run to output | Scientific work moved to the application run in Phase 4 | Fixed under ARCH-04; preserve thin adapter |
| `src/config.rs` | Model, defaults, manual serde, TOML decoding/merging, and cross-field validation | A 12-line facade fronts `model` (225), `defaults` (108), `deserialize` (65), and `validate` (179) | No public path or serialized behavior changed | Fixed under ARCH-05 |
| `src/validation.rs` / `src/synthetic_smoke.rs` | Marked and multimodal generators, execution, outcome interpretation, calibration statistics, DTOs, thresholds, and notes | `synthetic_smoke.rs` is a 24-line facade. Model, interval statistics, acceptance policy, marked workflow, multimodal runner, production-result observation, and marked/multimodal generators have separate owners; all call production engines | The obsolete validation path no longer exists; the 1,232-line smoke workflow and 563-line mixed generator were decomposed | Fixed under COR-01/ARCH-09 |
| `src/prepost/deltas.rs` | Marked/multimodal orchestration, axes, curve tests, territory matching, and prose | No longer exists. The 29-line facade fronts marked, multimodal, axes, curves, context, and territories | Phase 4 deleted the mixed workflow | Fixed under ARCH-06 |
| `src/output/writer.rs` | Document parsing, marked/multimodal projections, transaction, manifest/status, JSON helpers | The 230-line writer owns artifact planning, same-filesystem transaction commit, manifest validation/status, and path rebasing | Document semantics, family projections, and finite JSON/timing writes moved to their owners | Fixed under ARCH-07 |
| `src/output/result_types.rs` | All common, marked, multimodal, pre/post, diagnostic, and artifact DTOs | A 33-line facade fronts six schema-family modules | Phase 10 separated every result family and narrowed the public API | Fixed under ARCH-08 |
| `src/io/parquet.rs` | Loader routing plus four unrelated physical output schemas and ArrowWriter commit | A 13-line facade fronts loader/row/schema, multimodal writer (240), filtered Pattern writer (171), and record-batch commit (23) | Split output schema families without changing public function paths | Fixed under ARCH-09 |

## Production file-size triggers

`src/spectra/structure_factor/permutation.rs` is the only remaining production
file over 500 lines (520). It owns one bounded permutation executor: mark-field
scratch, fixed/stratified label generation, mode chunks, shell accumulation,
parallel dispatch, and the public binary/continuous wrappers. Phase 7
differential, chunk-size, determinism, memory, and benchmark evidence depends
on keeping that execution order visible. No split was made.

Files over 500 lines in the inventory otherwise contain tests (`synthetic_smoke/tests.rs`,
`perf/baseline_tests.rs`, `neighborhood/tests.rs`, `output/tests.rs`,
`prepost/tests.rs`, and `multimodal/tests.rs`). They are not production god
files; test organization can be revisited independently if navigation becomes
materially difficult.

## Production function-size triggers

| Function | Lines | Review judgment |
| --- | ---: | --- |
| `MultimodalEngine::analyze_run` | 209 | Retain: ordered application service with one transform/index/graph and typed stage timings; scientific calculations are delegated. |
| `summarize_permutation_whitening_from_shells` | 174 | Retain: one numerical shell-whitening/envelope assembly whose ordering is covered by oracle and differential tests. |
| `permutation_whitened_anisotropy` | 150 | Retain: one chunked permutation kernel with bounded scratch and dense-reference tests. |
| `run_multimodal_replicate` | 144 | Retain: one auditable dispatch from 22 declared scenarios to observed production result fields/errors. Scenario input construction is separate. |
| `multimodal_replicate_scenario` | 134 | Retain: one declared input-scenario dispatch; outcome decisions are prohibited from this module. |
| `AnalysisConfig::validate` | 134 | Retain: one cross-field invariant boundary. Splitting by section would hide constraints spanning permutation, inference, spectrum, registration, and memory settings. |
| `spatial_stage::run` | 120 | Retain: one ordered application stage that builds/reuses geometry and records timings; endpoint algorithms are separate. |
| `AnalysisEngine::analyze_pattern_inner` | 116 | Retain: high-level marked application coordinator; planning, computation, interpretation, and assembly have explicit owners. |
| `PatternBuilder::push` | 114 | Retain: one decoded-row validation/filter/QC state transition with CSV/Parquet parity tests. |
| `mark_pair_covariance_with_envelope` | 113 | Retain: one observed/null/envelope endpoint over a reusable pair plan. |
| `ResidualTerritoryPlan::build` | 112 | Retain: one checked contiguous neighborhood-plan builder with brute-force and budget tests. |
| `write_filtered_pattern_export_parquet` | 112 | Retain: one authoritative physical schema projection; optional absence and provenance are tested together. |
| `GlobalEnvelope::from_matrix_with_eligibility` | 110 | Retain: one ERL ranking/envelope algorithm validated against an independent oracle. |
| `summarize_multimodal_outcomes` | 108 | Retain: one denominator/interval aggregation boundary; production observation is separate. |
| `component_summary_for` | 108 | Retain: one component-specific analysis/result builder with shared spectrum machinery. |
| `run_cli` | 107 | Retain: one command dispatcher; command implementations live in separate modules. |
| `multiscale_residual_scalar_p_values` | 106 | Retain: one paired permutation loop that deliberately reuses the same labels for its two multiscale scalar endpoints. |

The assembler was the one trigger that still mixed responsibilities: its
top-level function fell from 227 to 92 lines, spectrum construction became a
focused helper, and interpretation moved to its own module.

## Tiny-module review

Deleted as ceremonial:

- `multiscale_residual/residual_field.rs`: the seven-line statistic now lives
  beside its only production consumer in `territories.rs`.
- `multiscale_residual/scale_radius.rs`: the three-line radius mapping now
  lives with territory geometry.
- `periodogram/taper.rs`: the six-line Hann weight now lives with the tapered
  periodogram that applies it.
- `permutation/rng.rs`: the one-line compatibility re-export was removed;
  callers use the canonical common seed primitive directly.

Retained small files are module facades, feature adapters, or narrow domain
owners with more than line-count value—for example result/config/Parquet
facades, the binary entry point, landmark DTOs, label policy, and output
artifact I/O. No one-function production file remains solely to disguise a
parent god workflow.

## Import and scaffolding audit

There are no production `use super::*` imports. The eight remaining matches
are inside local test modules. Searches found no production `Task N`, `MVP`,
`TODO`, `FIXME`, or `compatibility alias` scaffolding comments.
