#[cfg(test)]
use crate::{data::Pattern, spectra::kgrid::KMode};

pub(crate) mod kernel;
mod modes;
mod permutation;
mod shells;
mod summaries;

pub use kernel::{
    centered_structure_factor, centered_structure_factor_for_marks, observed_power_for_modes,
    observed_value_power_for_modes,
};
#[cfg(test)]
use kernel::{centered_structure_factor_for_index_subset, total_phase_sum};
pub use modes::resolvable_modes_for_pattern;
pub use permutation::{
    permutation_whitened_spectrum, permutation_whitened_spectrum_from_observed_modes,
    permutation_whitened_value_spectrum, permutation_whitened_value_spectrum_from_observed_modes,
    stratified_permutation_whitened_spectrum_from_observed_modes,
};
#[cfg(test)]
use summaries::{leave_one_out_medians, spectrum_scalar_readouts, summarize_permutation_whitening};

#[derive(Clone, Debug, PartialEq)]
pub struct PermutationWhitenedSpectrum {
    pub k_values: Vec<f64>,
    pub observed_power: Vec<f64>,
    pub median_permutation_power: Vec<f64>,
    pub whitened_power: Vec<f64>,
    pub inference_eligible: Vec<bool>,
    pub lower_global_envelope: Vec<f64>,
    pub upper_global_envelope: Vec<f64>,
    pub erl_depth: f64,
    pub n_modes: usize,
    pub n_permutations: usize,
    pub low_k_excess: f64,
    pub low_k_excess_p_value: Option<f64>,
    pub p_global: f64,
    pub dominant_k: Option<f64>,
    pub xi_um: Option<f64>,
    pub xi_stability_interval_um: Option<[f64; 2]>,
    pub xi_um_p_value: Option<f64>,
    pub alpha: Option<f64>,
    pub alpha_p_value: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpectrumPermutationOptions {
    pub n_shells: usize,
    pub low_k_modes: usize,
    pub n_permutations: usize,
    pub seed: u64,
    pub family_wise_alpha: f64,
    pub max_scale_um: f64,
    pub k_shell_min: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::stats::median_average_even;
    use crate::data::PatternMeta;
    use crate::inference::scalar_pvalues::{permutation_p_value, Tail};
    use crate::spectra::structure_factor::shells::shell_mean_powers;
    use approx::assert_abs_diff_eq;
    use proptest::prelude::*;

    fn pattern(marks: Vec<u8>) -> Pattern {
        Pattern::from_arrays(
            vec![0.0, 1.0, 2.0, 3.0],
            vec![0.0, 1.0, 0.0, 1.0],
            marks,
            PatternMeta {
                case_id: "case_001".into(),
                timepoint: "post".into(),
                protein: "MSH6".into(),
                slide_id: None,
                section_id: None,
                stain_batch: None,
                block_id: None,
                region_id: None,
            },
        )
        .expect("pattern")
    }

    #[test]
    fn marked_subset_core_matches_dense_binary_labels() {
        let pattern = pattern(vec![1, 0, 1, 0]);
        let total = total_phase_sum(&pattern, 0.7, 1.3).expect("total phase sum");
        let by_labels = centered_structure_factor_for_marks(&pattern, &pattern.mark, 0.7, 1.3)
            .expect("label structure factor");
        let by_indices = centered_structure_factor_for_index_subset(
            &pattern,
            &[0, 2],
            2,
            false,
            total,
            0.7,
            1.3,
        )
        .expect("subset structure factor");

        assert_abs_diff_eq!(by_indices, by_labels, epsilon = 1e-12);
    }

    #[test]
    fn unmarked_subset_core_matches_dense_binary_labels() {
        let pattern = pattern(vec![1, 1, 1, 0]);
        let total = total_phase_sum(&pattern, 0.7, 1.3).expect("total phase sum");
        let by_labels = centered_structure_factor_for_marks(&pattern, &pattern.mark, 0.7, 1.3)
            .expect("label structure factor");
        let by_indices =
            centered_structure_factor_for_index_subset(&pattern, &[3], 3, true, total, 0.7, 1.3)
                .expect("subset structure factor");

        assert_abs_diff_eq!(by_indices, by_labels, epsilon = 1e-12);
    }

    #[test]
    fn shell_means_group_the_production_modes() {
        let modes = vec![
            KMode {
                kx: 0.1,
                ky: 0.0,
                k: 0.1,
                shell_index: 0,
            },
            KMode {
                kx: 0.2,
                ky: 0.0,
                k: 0.2,
                shell_index: 0,
            },
            KMode {
                kx: 0.9,
                ky: 0.0,
                k: 0.9,
                shell_index: 1,
            },
            KMode {
                kx: 1.0,
                ky: 0.0,
                k: 1.0,
                shell_index: 1,
            },
        ];
        let means =
            shell_mean_powers(&modes, &[2.0, 4.0, 10.0, 14.0], &[0, 1]).expect("shell means");

        assert_abs_diff_eq!(means[0], 3.0, epsilon = 1e-12);
        assert_abs_diff_eq!(means[1], 12.0, epsilon = 1e-12);
    }

    #[test]
    fn low_k_readout_averages_only_requested_shells() {
        let readout = spectrum_scalar_readouts(&[0.1, 0.2, 0.3], &[2.0, 4.0, 100.0], 2);

        assert_abs_diff_eq!(readout.low_k_excess, 3.0, epsilon = 1e-12);
    }

    #[test]
    fn leave_one_out_medians_match_direct_exclusion_with_ties() {
        for values in [
            vec![1.0, 1.0],
            vec![3.0, 1.0, 2.0],
            vec![4.0, 1.0, 4.0, 2.0],
            vec![5.0, 1.0, 5.0, 2.0, 3.0],
            vec![8.0, 8.0, 18.0, 18.0, 8.0, 18.0],
        ] {
            let baselines = leave_one_out_medians(&values).expect("leave-one-out medians");
            for (excluded, baseline) in baselines.iter().copied().enumerate() {
                let mut remaining = values
                    .iter()
                    .copied()
                    .enumerate()
                    .filter_map(|(index, value)| (index != excluded).then_some(value))
                    .collect::<Vec<_>>();
                assert_eq!(
                    baseline,
                    median_average_even(&mut remaining).expect("median")
                );
            }
        }
    }

    proptest! {
        #[test]
        fn leave_one_out_medians_match_direct_exclusion_for_generated_values(
            raw_values in prop::collection::vec(0_u16..=10_000, 2..65),
        ) {
            let values = raw_values
                .into_iter()
                .map(f64::from)
                .collect::<Vec<_>>();
            let baselines = leave_one_out_medians(&values).expect("leave-one-out medians");

            for (excluded, baseline) in baselines.iter().copied().enumerate() {
                let mut remaining = values
                    .iter()
                    .copied()
                    .enumerate()
                    .filter_map(|(index, value)| (index != excluded).then_some(value))
                    .collect::<Vec<_>>();
                prop_assert_eq!(
                    baseline,
                    median_average_even(&mut remaining).expect("median")
                );
            }
        }

        #[test]
        fn leave_one_out_scores_are_equivariant_under_run_reversal(
            raw_curves in prop::collection::vec(any::<[u16; 3]>(), 2..33),
        ) {
            let curves = raw_curves
                .into_iter()
                .map(|curve| curve.map(f64::from))
                .collect::<Vec<_>>();
            let scores = leave_one_out_low_k_scores(&curves);
            let reversed_curves = curves.iter().copied().rev().collect::<Vec<_>>();
            let reversed_scores = leave_one_out_low_k_scores(&reversed_curves);

            prop_assert_eq!(
                reversed_scores,
                scores.into_iter().rev().collect::<Vec<_>>()
            );
        }
    }

    fn leave_one_out_low_k_scores(curves: &[[f64; 3]]) -> Vec<f64> {
        let mut baselines = vec![[0.0; 3]; curves.len()];
        for shell in 0..3 {
            let values = curves.iter().map(|curve| curve[shell]).collect::<Vec<_>>();
            for (run, baseline) in leave_one_out_medians(&values)
                .expect("leave-one-out medians")
                .into_iter()
                .enumerate()
            {
                baselines[run][shell] = baseline;
            }
        }

        curves
            .iter()
            .zip(baselines)
            .map(|(curve, baseline)| {
                curve
                    .iter()
                    .zip(baseline)
                    .map(|(power, reference)| power / reference.max(f64::EPSILON))
                    .sum::<f64>()
                    / 3.0
            })
            .collect()
    }

    #[test]
    fn leave_one_out_whitening_preserves_observed_low_k_score() {
        let modes = vec![
            KMode {
                kx: 1.0,
                ky: 0.0,
                k: 1.0,
                shell_index: 0,
            },
            KMode {
                kx: 0.0,
                ky: 2.0,
                k: 2.0,
                shell_index: 1,
            },
        ];
        let result = summarize_permutation_whitening(
            &modes,
            vec![9.0, 6.0],
            vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 8.0]],
            SpectrumPermutationOptions {
                n_shells: 2,
                low_k_modes: 2,
                n_permutations: 3,
                seed: 0,
                family_wise_alpha: 0.5,
                max_scale_um: 10.0,
                k_shell_min: 2,
            },
        )
        .expect("valid spectrum summary")
        .expect("eligible spectrum summary");

        assert_eq!(result.median_permutation_power, vec![3.0, 4.0]);
        assert_eq!(result.whitened_power, vec![3.0, 1.5]);
        assert_eq!(result.low_k_excess, 2.25);
    }

    #[test]
    fn exact_five_curve_table_reproduces_shared_baseline_defect_and_validates_loo_repair() {
        let modes = vec![
            KMode {
                kx: 1.0,
                ky: 0.0,
                k: 1.0,
                shell_index: 0,
            },
            KMode {
                kx: 0.0,
                ky: 2.0,
                k: 2.0,
                shell_index: 1,
            },
        ];
        let curves = [
            [8.0, 20.0],
            [8.0, 10.0],
            [8.0, 40.0],
            [18.0, 20.0],
            [18.0, 10.0],
        ];
        let options = SpectrumPermutationOptions {
            n_shells: 2,
            low_k_modes: 2,
            n_permutations: 2,
            seed: 0,
            // The scalar p-value does not depend on alpha. Use a resolvable
            // level for the unrelated two-sided scalar readouts.
            family_wise_alpha: 2.0 / 3.0,
            max_scale_um: 10.0,
            k_shell_min: 2,
        };

        let mut shared_baseline_rejection_count = 0;
        let mut leave_one_out_rejection_count = 0;
        for observed in &curves {
            for first_null in &curves {
                for second_null in &curves {
                    let shared_baseline_p_value =
                        shared_baseline_low_k_p_value(observed, [first_null, second_null]);
                    if shared_baseline_p_value <= 1.0 / 3.0 {
                        shared_baseline_rejection_count += 1;
                    }

                    let result = summarize_permutation_whitening(
                        &modes,
                        observed.to_vec(),
                        vec![first_null.to_vec(), second_null.to_vec()],
                        options,
                    )
                    .expect("valid spectrum summary")
                    .expect("eligible spectrum summary");
                    if result
                        .low_k_excess_p_value
                        .is_some_and(|p_value| p_value <= 1.0 / 3.0)
                    {
                        leave_one_out_rejection_count += 1;
                    }
                }
            }
        }

        assert_eq!(shared_baseline_rejection_count, 43);
        assert!(
            leave_one_out_rejection_count <= 125 / 3,
            "leave-one-out test rejected {leave_one_out_rejection_count}/125 exact uniform tables"
        );
    }

    fn shared_baseline_low_k_p_value(observed: &[f64; 2], permutations: [&[f64; 2]; 2]) -> f64 {
        let baseline = [0, 1].map(|shell| {
            let mut values = permutations
                .iter()
                .map(|curve| curve[shell])
                .collect::<Vec<_>>();
            median_average_even(&mut values).expect("shared permutation baseline")
        });
        let score = |curve: &[f64; 2]| {
            curve
                .iter()
                .zip(baseline)
                .map(|(power, reference)| power / reference.max(f64::EPSILON))
                .sum::<f64>()
                / 2.0
        };
        let null_scores = permutations
            .iter()
            .map(|curve| score(curve))
            .collect::<Vec<_>>();

        permutation_p_value(score(observed), &null_scores, Tail::OneSidedHigh, 2.0 / 3.0)
            .expect("shared-baseline p-value")
    }
}
