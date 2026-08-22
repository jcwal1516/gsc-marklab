# Completion Evidence Audit

Audit date: 2026-08-22<br>
Evidence base: completion-audit commits `6d7c13d`, `97dd3e1`, and `00cad21`,
plus the focused evidence tests listed below.<br>
Scope: the mandatory regression matrix, definition-of-done invariants, public
deliverables, and the seven findings discovered after the first Phase 13
closure claim.

This document maps the master-plan names to actual test functions. A different
test name is acceptable only when the mapped assertions exercise the complete
required behavior. Newly discovered missing behavior received a focused test
rather than being treated as covered by narrative.

## Mandatory regression matrix

| Required coverage | Current executable evidence |
| --- | --- |
| `rigid_identity` | `registration::tests::rigid_identity` |
| `rigid_translation` | `registration::tests::rigid_translation` |
| `rigid_rotation` | `registration::tests::rigid_rotation` and `rigid_rotation_90_degrees` |
| `rigid_rotation_translation` | `registration::tests::rigid_rotation_and_translation` |
| `rigid_preserves_distances` | `registration::tests::rigid_preserves_distance` |
| `rigid_rejects_scaling` | `registration::tests::rigid_does_not_absorb_scale` |
| `rigid_rejects_degenerate_geometry` | `registration::tests::rigid_rejects_degenerate_landmarks` |
| `affine_recovers_known_transform` | `registration::tests::affine_transform_recovers_shear` |
| `registration_qc_known_residuals` | `registration::tests::registration_qc_reports_usable_distance_scale` |
| `registration_extrapolation_boundary` | `multimodal::registration_artifacts::tests::registration_extrapolation_boundary` |
| Four scalar permutation tail/finite contracts | Exact tests `permutation_high_tail_inclusive_ties`, `permutation_low_tail_inclusive_ties`, `permutation_two_sided_equal_tail`, and `permutation_rejects_nonfinite` in `inference::tests` |
| Four ERL contracts | Exact tests `erl_matches_checked_oracle`, `erl_pointwise_ties`, `erl_identical_curves`, and `erl_eligibility_mask` |
| `benjamini_hochberg_known_vector` | Exact test in `inference::tests`; also rejects non-finite and out-of-range p-values |
| `unstratified_significant_stratified_not_significant` | `api::qc_pipeline::tests::confounding_detected_when_unstratified_disappears_after_stratification` |
| `both_significant` | `api::qc_pipeline::tests::confounding_not_detected_when_both_remain_significant` |
| `neither_significant` | `api::qc_pipeline::tests::confounding_not_detected_when_neither_is_significant` |
| `homogeneous_strata` | `engine_spectrum::homogeneous_strata_report_degenerate_null` |
| `missing_stratum` | `engine_spectrum::missing_strata_report_validation_error` |
| `stratified_result_is_not_recomputed_primary` | `engine_spectrum::distinct_nulls_are_actually_executed` checks distinct calls, values, persisted summaries, and round trip |
| `sparse_enrichment_roundtrip` | Exact test in `neighborhood::tests` |
| `undefined_z_score_roundtrip` | Exact test in `neighborhood::tests` |
| `result_v03_roundtrip`, `result_v02_to_v03_conversion`, `prepost_result_roundtrip`, `unknown_result_version_rejected`, `unknown_fields_rejected` | Exact tests in `tests/result_v03.rs` |
| `all_result_floats_are_finite` | Exact test in `output::tests`; nested non-finite option/sequence rejections are in `common::finite::tests` |
| `csv_parquet_equivalent_rows_produce_equal_pattern` and `optional_absence_preserved` | Exact tests in `io::parquet_tests` |
| `partial_dense_column_rejected` | `csv_loader_rejects_partially_populated_dense_optional_metrics` plus `parquet_loader_rejects_partially_populated_dense_optional_metrics` |
| `internal_control_fraction_correct` | `remediation_internal_control_fraction_is_not_final_retained_fraction`, `csv_loader_uses_internal_control_local_as_validity_mask`, and Parquet parity |
| `artifact_fraction_correct` and `nonviable_fraction_correct` | `csv_loader_tracks_each_qc_fraction_against_all_in_mask_cells` and `parquet_loader_excludes_artifact_and_nonviable_rows_from_analysis_window` |
| `metadata_mismatch_rejected` | Exact CSV/PatternBuilder boundary test in `synthetic_smoke::tests` |
| `filtered_export_is_explicitly_not_full_roundtrip` | `io::parquet_tests::optional_absence_preserved` asserts `marklab.export_kind = filtered_canonical_pattern` and absence of fabricated source fields |
| Eight spatial-index contracts | Exact `nearest_neighbor_matches_bruteforce`, `radius_query_matches_bruteforce`, `knn_matches_bruteforce`, `duplicate_coordinate_ties`, `graph_matches_bruteforce`, `pair_plan_matches_bruteforce`, `territory_neighbors_match_bruteforce`, and `deterministic_query_order` tests |
| `binary_kernel_matches_dense_reference` | Exact structure-factor test, including the minority-subset path; `unmarked_subset_core_matches_dense_binary_labels` covers the complementary optimization |
| `continuous_kernel_matches_reference` | Exact manual complex-sum fixture in `spectra::structure_factor::tests` |
| `shell_aggregation_known_modes` | `shell_means_group_the_production_modes` and `tapered_periodogram_groups_all_modes_in_each_radial_shell` |
| `chunk_sizes_produce_same_result` | Binary, continuous, and stratified `chunk_sizes_produce_same_*_spectrum` tests at size 1, typical, and oversize chunks |
| `parallel_and_serial_match` | `performance_contract::permutation_spectrum_is_reproducible_across_thread_counts` |
| `permutation_order_stable` | The same cross-thread exact-result test plus the chunk-size exact-equality tests prove permutation index/seed order is scheduling-independent |
| `shell_level_storage_matches_previous_valid_output` | Exact mode-matrix-versus-shell-matrix differential test in `spectra::structure_factor::tests` |
| `probabilistic_marks_finite` | `engine_spectrum::engine_uses_probabilistic_marks_when_configured`, continuous chunk equality, and the result finite boundary |
| `application_builds_transform_once` and `application_builds_graph_once` | Exact tests in `multimodal::tests` |
| `library_and_cli_core_results_match` | Exact integration test in `tests/multimodal_cli.rs` |
| `all_configured_null_models_present` | `multimodal::tests::all_configured_null_models_are_present_in_the_application_run` |
| `profile_fields_are_computed_or_absent_from_schema` | Exact format-0.3 test asserts calculated fractions and absence of `enrichment`, `cross_curves`, and QC placeholders |
| `territory_types_are_distinct` | Exact format-0.3 test serializes both territory families and asserts disjoint algorithm-specific fields |
| `label_access_is_allocation_free_in_hot_path` | Exact borrowed-label pointer-identity test plus `application_builds_primary_label_encoding_once` for shared compact IDs |
| `multimodal_telemetry_populated` | `multimodal_telemetry_populates_every_application_stage_in_order` and budget-peak telemetry tests |
| Eight validation contracts | Exact or directly named tests: `remediation_multimodal_validation_calls_the_public_engine`, both `negative_control_calibrates` scheduled tests, `positive_control_detects_signal`, `rotation_scenario_requires_real_rigid_transform`, `registration_jitter_uses_actual_registration_output`, `prepost_equivalence_uses_actual_comparison`, `no_manual_status_flag_injection`, and `failed_replicates_are_reported_and_fail_the_smoke_scenario` |
| Five output contracts | Exact `failed_artifact_write_does_not_commit_final_directory`, `manifest_matches_written_artifacts`, `result_and_timings_use_same_telemetry`, `file_and_directory_prepost_inputs_are_consistent`, and `batch_id_cannot_escape_output_root` tests |
| `pooled_only`, `both`, `auto_pooled`, `auto_separate_or_both`, and `mode_selection_reason_reported` | Distinct assertions in `engine_reports_separate_component_summaries_when_component_mode_is_both` cover pooled, both, auto-pooled, auto-both, and persisted reasons |
| `separate_only` | `remediation_separate_component_mode_does_not_behave_like_both` asserts component results and every pooled endpoint is not applicable |

## Completion-audit finding evidence

| ID | Reproduction and closure evidence |
| --- | --- |
| AUDIT-01 | Missing BH/ERL and ambiguous mandatory-test evidence was reproduced by source search. Focused BH, ERL, continuous-kernel, shell-storage, metadata, profile, territory, and undefined-statistic tests plus this exact mapping close the gap. |
| BOUND-06 | Pre/post services were under `cfg(feature = "cli")`. They now compile unconditionally, are supported crate-root functions, pass `cargo check --no-default-features`, and are called by the public API contract. |
| MODEL-05 | `MarkedPatternResult.prepost_curve_comparisons` was always empty. It and its report/converter producer were removed; strict 0.3 parsing rejects the obsolete key; comparisons exist only in versioned pre/post payloads. |
| PERF-09 | One sorted run-level `PrimaryLabelEncoding` is built once and passed to enrichment, all sensitivity nulls, cross curves, territories, profiles, and graph smoothing. Edge/permutation loops use compact IDs; strings are reconstructed only for result rows. |
| PERF-11 | `CrossInteractionPlan` uses the shared `SpatialIndex2D`, stores each source/target/bin once, and is reused for every configured pair and permutation. Brute-force, one-plan, one-index, empty-bin, ERL, serialization, budget, and scaling evidence pass. |
| PERF-12 | `RasterAssignmentPlan` retains each cell's raster bin once and refills reusable buffers for observed and null labels. Differential, one-build, memory-accounting, and DHAT no-allocation-after-setup tests pass. |
| PERF-13 | `MultimodalMemoryBudget` reserves retained input/results and checks sequential scratch. Index, graph, cross-pair, and territory-neighborhood builders receive remaining bytes and reject before an over-budget entry. Low-budget, dense-plan, graph/plan cap, and telemetry-within-budget tests pass. |
| SCI-10 | Cross curves use the checked ERL global envelope with explicit geometric eligibility. Empty geometry is null; observed zero remains numeric zero. The old minima/maxima and separate max-bin p-value semantics are gone. |

## Source and deliverable audit

- Every required deliverable named in §25 exists and is non-empty. The closure
  report remains intentionally reopened until final verification is recorded.
- Production `use super::*` search finds only imports inside `#[cfg(test)]`
  modules. Production task/MVP/TODO/FIXME/compatibility-scaffolding search is
  empty.
- Numeric-helper search finds one canonical common statistics implementation;
  territory-specific mean/median functions delegate to it.
- Non-finite constants in production are internal reducer/envelope
  initializers. The finite result boundary and sparse/empty-bin tests prevent
  them from reaching persisted results.
- Remaining `cfg(feature = "cli")` sites gate CLI/input/output adapters, not
  scientific comparison or enrichment semantics. The no-default build is the
  executable boundary check.
- Production contains no complete-result or fused-table output clone. The one
  matching `result.clone()` is test-only fixture setup.
- `timings: Vec::new()` in multimodal assembly is a temporary field value in
  the same function and is immediately replaced by the one authoritative
  in-memory telemetry vector before the result is returned.
