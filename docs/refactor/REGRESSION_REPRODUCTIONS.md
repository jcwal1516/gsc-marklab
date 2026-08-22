# Phase 0 Regression Reproductions

These tests encode the corrected contract but remain ignored until their owning remediation phase. Each was run explicitly with `--ignored --test-threads=1` and confirmed to fail for the named defect. Normal test runs remain green and report the reproductions as ignored.

| Requirement | Test | Observed failing evidence |
| --- | --- | --- |
| COR-01 | `validation::tests::remediation_multimodal_validation_calls_the_public_engine` | Six scenario replicates produced zero `MultimodalEngine::analyze` calls. |
| COR-02 | `registration::tests::rigid_rotation` (formerly ignored as `remediation_rigid_registration_recovers_known_rotation`) | Baseline produced `scale_translation` with zero scale and mapped the known rotated point to x=8.5 instead of x=8.0. The enabled test now passes through the true rigid fit in `53e2348`. |
| COR-03 | `distinct_nulls_are_actually_executed` (formerly ignored as `remediation_confounding_compares_unstratified_and_stratified_results`) | Baseline separately proved unstratified significance and stratified nonsignificance but lacked `ConfoundedBySpatialStrata` because it compared the stratified path with itself. The enabled test passes in `aecc554`; versioned dual-result serialization remains Phase 5 scope. |
| COR-04 | `neighborhood::tests::remediation_sparse_enrichment_statistics_are_finite_or_typed_undefined` | A positive observed edge count with zero expected edges produced `enrichment_ratio: inf` and a fabricated zero z-score. |
| COR-04 | `neighborhood::tests::remediation_sparse_enrichment_roundtrips_through_json` | Serde emitted the infinite ratio as JSON `null`; deserialization then failed with “invalid type: null, expected f64”. |
| COR-05 | `spectra::tests::remediation_pair_correlation_does_not_report_empty_bins_as_observed_zero` | The first empty bin was returned as `count: 0, value: 0.0`. |
| COR-06 | `prepost::tests::remediation_prepost_axes_accept_harmless_float_reconstruction` | Axes `0.1 + 0.2` and `0.3` were rejected as misaligned instead of running the two comparison diagnostics. |
| COR-07 | `validation::tests::remediation_internal_control_fraction_is_not_final_retained_fraction` | The loader reported 0.5 (final retained fraction) instead of the independently observed 0.75 internal-control-valid fraction. |
| MODEL-04 | `remediation_separate_component_mode_does_not_behave_like_both` | `Separate` returned both two component analyses and an active pooled spectrum, making it behaviorally equivalent to `Both`. |
| OUT-01 | `output::tests::remediation_result_and_timings_sidecar_use_the_same_telemetry` | `timings.json` contained an extra `write_outputs` stage absent from the result document's timing history. |
| OUT-04 / OUT-05 | `io::parquet_tests::remediation_parquet_roundtrip_preserves_optional_absence` | A Pattern with absent optional control/component fields reloaded with fabricated presence; the first failure was `internal_control_valid_fraction.is_none()`. |
| OUT-06 | `remediation_batch_id_cannot_escape_output_root` | A manifest ID of `../escaped` completed successfully instead of being rejected. |

## Commands and results

- `cargo +1.96.0 test --locked --all-features remediation_ -- --ignored --test-threads=1` exited 101. All nine library reproductions failed; after correcting one telemetry fixture guard, each failure reached the intended defect.
- `cargo +1.96.0 test --locked --all-features --test engine_spectrum remediation_ -- --ignored --test-threads=1` exited 101 with both COR-03 and MODEL-04 failures shown above.
- `cargo +1.96.0 test --locked --all-features --test cli remediation_batch_id_cannot_escape_output_root -- --ignored --test-threads=1` exited 101 because the unsafe manifest ID unexpectedly succeeded.
- `cargo +1.96.0 fmt --check` and warnings-denied all-target/all-feature Clippy both passed after adding the reproductions.
- `cargo +1.96.0 test --locked --all-features --lib` passed 167 tests with 9 ignored reproductions.
- `cargo +1.96.0 test --locked --all-features --test engine_spectrum` passed 17 tests with 2 ignored reproductions.
- `cargo +1.96.0 test --locked --all-features --test cli` passed 15 tests with 1 ignored reproduction.
