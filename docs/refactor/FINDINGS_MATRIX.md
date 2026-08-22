# Findings Matrix

Every registered finding remains present until it is fixed, disproved with specific evidence, or explicitly deferred with a documented reason and remaining risk.

| ID | Finding | Reproduced | Resolution | Tests | Benchmark | Commit | Status |
| --- | --- | --- | --- | --- | --- | --- | --- |
| COR-01 | Multimodal validation bypasses the multimodal engine | Yes — `validation/generators.rs` synthesizes outcome booleans and `run_multimodal_generator` only counts them | Pending remediation | Ignored `remediation_multimodal_validation_calls_the_public_engine` fails with zero calls | N/A | — | Confirmed |
| COR-02 | Configured rigid registration is not rigid | Yes — `Rigid` dispatches to `fit_similarity`, which returns diagonal scale plus translation and explicitly omits rotation | Pending remediation | Ignored known-rotation regression fails | N/A | — | Confirmed |
| COR-03 | Stratified confounding comparison recomputes the same result | Yes — configured strata produce the primary spectrum, then `stratified_confounds` reruns the same stratified spectrum | Pending remediation | Ignored distinct-null regression fails after proving unstratified significant / stratified nonsignificant | Pending reuse benchmark | — | Confirmed |
| COR-04 | Non-finite enrichment results can break serialization | Yes — zero expected edges with positive observed edges returns `f64::INFINITY`; zero null variance is forced to z-score zero | Pending remediation | Ignored finite-state and JSON round-trip regressions both fail | N/A | — | Confirmed |
| COR-05 | Unavailable or invalid states are represented by numeric zero | Yes — profile/pre-post errors use `statistic: 0.0`; empty pair bins use `value: 0.0` | Pending remediation | Ignored empty-bin regression fails; curve-test availability coverage still pending | N/A | — | Confirmed |
| COR-06 | Exact floating-point axis equality is used in pre/post comparison | Yes — spectrum, pair-correlation, and cross-curve axes use direct `f64 != f64` checks | Pending remediation | Ignored harmless-reconstruction regression fails | N/A | — | Confirmed |
| COR-07 | Internal-control validity semantics may be conflated with overall retained fraction | Yes — both CSV and Parquet assign `internal_control_valid_fraction = valid_mask_fraction`, whose numerator is final retained rows | Pending remediation | Ignored independent-denominator regression fails; Parquet parity remains pending | N/A | — | Confirmed |
| SCI-01 | The MODWT implementation is not an MODWT | Pending | Pending investigation | Pending | Pending | — | Open |
| SCI-02 | The DoG module does not implement a difference of Gaussians | Pending | Pending investigation | Pending | Pending | — | Open |
| SCI-03 | Wavelet territories are neighborhood residual heuristics | Pending | Pending investigation | Pending | Pending | — | Open |
| SCI-04 | Audit the “Bartlett periodogram” name | Pending | Pending investigation | Pending | Pending | — | Open |
| SCI-05 | Generic marked-pattern analysis emits MMR-specific interpretation text | Pending | Pending investigation | Pending | Pending | — | Open |
| ARCH-01 | Library and CLI perform different multimodal analyses | Pending | Pending investigation | Pending | Pending | — | Open |
| ARCH-02 | AnalysisEngine is a distributed god workflow | Pending | Pending investigation | Pending | Pending | — | Open |
| ARCH-03 | structure_factor.rs is a god file | Pending | Pending investigation | Pending | Pending | — | Open |
| ARCH-04 | CLI multimodal analysis contains domain logic | Pending | Pending investigation | Pending | Pending | — | Open |
| ARCH-05 | Configuration is a god file | Pending | Pending investigation | Pending | Pending | — | Open |
| ARCH-06 | Pre/post comparison is a god file | Pending | Pending investigation | Pending | Pending | — | Open |
| ARCH-07 | Output writer is a god file | Pending | Pending investigation | Pending | Pending | — | Open |
| ARCH-08 | Result types are a schema landfill | Pending | Pending investigation | Pending | Pending | — | Open |
| ARCH-09 | Cosmetic modularity | Pending | Pending investigation | Pending | Pending | — | Open |
| BOUND-01 | Pattern opens files | Pending | Pending investigation | Pending | Pending | — | Open |
| BOUND-02 | Cell DTOs, CSV loading, CellViT adaptation, validation, and label interpretation share one module | Pending | Pending investigation | Pending | Pending | — | Open |
| BOUND-03 | Domain enrichment is compiled only under the CLI feature | Pending | Pending investigation | Pending | Pending | — | Open |
| BOUND-04 | Output code calculates domain policy | Pending | Pending investigation | Pending | Pending | — | Open |
| BOUND-05 | WSI adapter is comparatively clean | Pending | Pending investigation | Pending | Pending | — | Open |
| DUP-01 | Multiple median implementations | Pending | Pending investigation | Pending | Pending | — | Open |
| DUP-02 | Multiple mean, finite-mean, variance, min/max, and effective-length helpers | Pending | Pending investigation | Pending | Pending | — | Open |
| DUP-03 | Permutation p-value formula is duplicated | Pending | Pending investigation | Pending | Pending | — | Open |
| DUP-04 | Stratified and unstratified enrichment duplicate the full algorithm | Pending | Pending investigation | Pending | Pending | — | Open |
| DUP-05 | CSV and Parquet loading duplicate the same state machine | Pending | Pending investigation | Pending | Pending | — | Open |
| DUP-06 | Registration and graph construction are repeated by CLI and engine | Pending | Pending investigation | Pending | Pending | — | Open |
| DUP-07 | Timing and manifest construction are duplicated | Pending | Pending investigation | Pending | Pending | — | Open |
| DUP-08 | Curve-test DTO construction is duplicated | Pending | Pending investigation | Pending | Pending | — | Open |
| DUP-09 | Effective geometry is calculated using inconsistent definitions | Pending | Pending investigation | Pending | Pending | — | Open |
| PERF-01 | spatial_index.rs is quadratic and is not a spatial index | Pending | Pending investigation | Pending | Pending | — | Open |
| PERF-02 | Radius graph construction is quadratic | Pending | Pending investigation | Pending | Pending | — | Open |
| PERF-03 | kNN graph construction sorts all other cells for every cell | Pending | Pending investigation | Pending | Pending | — | Open |
| PERF-04 | Pair correlation recalculates all pair distances | Pending | Pending investigation | Pending | Pending | — | Open |
| PERF-05 | Territory detection repeats neighborhood scans | Pending | Pending investigation | Pending | Pending | — | Open |
| PERF-06 | Profile membership repeatedly scans every cell | Pending | Pending investigation | Pending | Pending | — | Open |
| PERF-07 | Million-cell benchmark is not credible with quadratic nearest-neighbor calculation | Pending | Pending investigation | Pending | Pending | — | Open |
| PERF-08 | Spectrum stores large nested mode-power matrices | Pending | Pending investigation | Pending | Pending | — | Open |
| PERF-09 | Repeated metadata and label allocations | Pending | Pending investigation | Pending | Pending | — | Open |
| PERF-10 | Complete results and cell tables are cloned for output | Pending | Pending investigation | Pending | Pending | — | Open |
| MODEL-01 | TerritoryFeature overloads unrelated algorithms | Pending | Pending investigation | Pending | Pending | — | Open |
| MODEL-02 | Public fields are present but not implemented | Pending | Pending investigation | Pending | Pending | — | Open |
| MODEL-03 | String statuses and interpretation classes | Pending | Pending investigation | Pending | Pending | — | Open |
| MODEL-04 | Component modes are not behaviorally distinct | Yes — `Separate` and `Both` share one branch and pooled analysis always runs | Pending remediation | Ignored `Separate`-versus-`Both` regression fails | N/A | — | Confirmed |
| OUT-01 | Result and timings artifacts describe different timing histories | Yes — writer clones result timings and appends `write_outputs` only to the sidecar | Pending remediation | Ignored result/sidecar equality regression fails | N/A | — | Confirmed |
| OUT-02 | Pre/post results are unversioned | Pending | Pending investigation | Pending | Pending | — | Open |
| OUT-03 | Output writing is non-atomic | Pending | Pending investigation | Pending | Pending | — | Open |
| OUT-04 | Parquet writer fabricates absent fields | Yes — writer emits “valid”, true/false flags, and zero component/QC IDs for absent Pattern fields | Pending remediation | Ignored optional-absence round-trip regression fails | N/A | — | Confirmed |
| OUT-05 | CSV and Parquet schema definitions can drift | Yes — independently defined CSV DTO and Parquet schema; current writer/loader changes logical absence | Pending shared logical schema | Optional-absence regression fails; full parity cases pending | N/A | — | Confirmed |
| OUT-06 | Batch output IDs may escape the output directory | Yes — both batch paths join unvalidated manifest IDs directly to the output root | Pending remediation | Ignored `../escaped` CLI regression unexpectedly succeeds | N/A | — | Confirmed |
