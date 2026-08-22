# Phase 0 Regression Reproductions

These tests encode the corrected contract but remain ignored until their owning remediation phase. Each was run explicitly with `--ignored --test-threads=1` and confirmed to fail for the named defect. Normal test runs remain green and report the reproductions as ignored.

| Requirement | Test | Observed failing evidence |
| --- | --- | --- |
| COR-01 | `validation::tests::remediation_multimodal_validation_calls_the_public_engine` | Six scenario replicates produced zero `MultimodalEngine::analyze` calls. |
| COR-02 | `registration::tests::rigid_rotation` (formerly ignored as `remediation_rigid_registration_recovers_known_rotation`) | Baseline produced `scale_translation` with zero scale and mapped the known rotated point to x=8.5 instead of x=8.0. The enabled test now passes through the true rigid fit in `53e2348`. |
| COR-03 | `distinct_nulls_are_actually_executed` (formerly ignored as `remediation_confounding_compares_unstratified_and_stratified_results`) | Baseline separately proved unstratified significance and stratified nonsignificance but lacked `ConfoundedBySpatialStrata` because it compared the stratified path with itself. The enabled test passes in `aecc554`; versioned dual-result serialization remains Phase 5 scope. |
| COR-04 | Enabled `neighborhood::tests::remediation_sparse_enrichment_statistics_are_finite_or_typed_undefined` | Baseline positive observed edges with zero expectation produced infinity and a fabricated zero z-score. In `4bf20e8`, both values are absent with typed reasons while the p-value remains available. |
| COR-04 | Enabled `neighborhood::tests::remediation_sparse_enrichment_roundtrips_through_json` | Baseline serde emitted infinity as JSON `null` and could not deserialize it into `f64`. The 0.3 nullable state now round-trips and CSV/Parquet/report projections preserve its reason. |
| COR-05 | Enabled `spectra::tests::remediation_pair_correlation_does_not_report_empty_bins_as_observed_zero` | Baseline returned the first empty bin as `count: 0, value: 0.0`. In `e7447c0`, it is `count: 0, value: None`, is excluded from inference, and unavailable curve comparisons serialize a typed state with a null statistic. |
| COR-06 | Enabled `prepost::tests::remediation_prepost_axes_accept_harmless_float_reconstruction` | Baseline rejected axes `0.1 + 0.2` and `0.3`. In `e7f91ca`, spectrum, pair-correlation, and cross-interaction axes share a documented absolute-plus-relative tolerance; a separate material-difference test proves real mismatches remain unavailable. |
| COR-07 | Enabled `validation::tests::remediation_internal_control_fraction_is_not_final_retained_fraction` | Baseline reported 0.5 (final retained fraction) instead of the independent 0.75 control-valid fraction. In `6000cc8`, shared adapter counters produce distinct fractions, with CSV/Parquet parity and combination/zero-denominator coverage. |
| MODEL-04 | Enabled `remediation_separate_component_mode_does_not_behave_like_both` | Baseline returned component analyses plus the active pooled spectrum. In `b56cc60`, Separate returns components while every pooled endpoint is NotApplicable; Pooled, Both, and both Auto resolutions have explicit coverage and a recorded reason. |
| OUT-01 | Enabled `output::tests::result_and_timings_use_same_telemetry` | Baseline added `write_outputs` only to `timings.json`. Commits `756ecbc` and `3d8ad46` serialize one analysis-stage vector into result, internal timing, external timing, and trace projections without rereading a sidecar. |
| OUT-04 / OUT-05 | Enabled `io::parquet_tests::optional_absence_preserved` plus `csv_parquet_equivalent_rows_produce_equal_pattern` | Baseline reloaded an absent internal control and zero QC/component IDs as measured values. Commit `f4243cd` routes both formats through one logical row/builder, omits unavailable states from the explicitly filtered export, records provenance, and proves full logical parity for equivalent rows. |
| OUT-06 | Enabled `tests/cli.rs::batch_id_cannot_escape_output_root` plus shared resolver unit tests | Baseline accepted `../escaped`, exited successfully, and wrote outside the configured root. Commit `a8d38c5` rejects unsafe components and symlink targets through one marked/multimodal resolver while retaining valid sequential/parallel behavior. |

## Commands and results

- `cargo +1.96.0 test --locked --all-features remediation_ -- --ignored --test-threads=1` exited 101. All nine library reproductions failed; after correcting one telemetry fixture guard, each failure reached the intended defect.
- `cargo +1.96.0 test --locked --all-features --test engine_spectrum remediation_ -- --ignored --test-threads=1` exited 101 with both COR-03 and MODEL-04 failures shown above.
- `cargo +1.96.0 test --locked --all-features --test cli remediation_batch_id_cannot_escape_output_root -- --ignored --test-threads=1` exited 101 because the unsafe manifest ID unexpectedly succeeded.
- `cargo +1.96.0 fmt --check` and warnings-denied all-target/all-feature Clippy both passed after adding the reproductions.
- `cargo +1.96.0 test --locked --all-features --lib` passed 167 tests with 9 ignored reproductions.
- `cargo +1.96.0 test --locked --all-features --test engine_spectrum` passed 17 tests with 2 ignored reproductions.
- `cargo +1.96.0 test --locked --all-features --test cli` passed 15 tests with 1 ignored reproduction.
