use std::collections::BTreeMap;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;

use super::{
    estimation::{
        contrasts, enumerate_assignments, estimate_exposure_mean, exposure, exposure_probabilities,
        exposures, ht_contrast,
    },
    validation::{combination_count, compile_clusters, compile_graph, validate_units},
    CausalError, InterferenceRandomizationResult, JointExposureProbabilities, ObservedUnitExposure,
    RandomizedInterferenceResult, RandomizedInterferenceSpec, UnitExposureProbabilities,
    MAXIMUM_ASSIGNMENT_STATES,
};

pub fn randomized_binary_interference(
    mut spec: RandomizedInterferenceSpec,
) -> Result<RandomizedInterferenceResult, CausalError> {
    spec.units.sort_by(|left, right| {
        left.cluster_id
            .cmp(&right.cluster_id)
            .then_with(|| left.unit_id.cmp(&right.unit_id))
    });
    validate_units(&spec)?;
    let unit_indices = spec
        .units
        .iter()
        .enumerate()
        .map(|(index, unit)| (unit.unit_id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let adjacency = compile_graph(&spec, &unit_indices)?;
    let clusters = compile_clusters(&spec)?;
    let state_count = clusters.iter().try_fold(1_u64, |product, cluster| {
        product
            .checked_mul(combination_count(
                cluster.members.len(),
                cluster.treated_units,
            )?)
            .ok_or_else(|| CausalError::Resource("assignment-state count overflowed".into()))
    })?;
    if state_count > spec.maximum_assignment_states || state_count > MAXIMUM_ASSIGNMENT_STATES {
        return Err(CausalError::Resource(format!(
            "assignment states {state_count} exceed caller or built-in maximum"
        )));
    }
    let unit_count = spec.units.len() as u64;
    let evaluations = state_count
        .checked_mul(unit_count)
        .and_then(|value| value.checked_mul(8))
        .and_then(|value| {
            u64::try_from(spec.permutations + 1)
                .ok()
                .and_then(|replicates| replicates.checked_mul(unit_count))
                .and_then(|randomization| randomization.checked_mul(4))
                .and_then(|randomization| value.checked_add(randomization))
        })
        .ok_or_else(|| CausalError::Resource("unit-assignment work overflowed".into()))?;
    if evaluations > spec.maximum_unit_assignment_evaluations {
        return Err(CausalError::Resource(format!(
            "planned unit-operation upper bound {evaluations} exceeds declared maximum {}",
            spec.maximum_unit_assignment_evaluations
        )));
    }
    let states = enumerate_assignments(spec.units.len(), &clusters);
    if states.len() as u64 != state_count {
        return Err(CausalError::Numerical(
            "enumerated assignment-state count disagrees with exact plan".into(),
        ));
    }
    let state_exposures = states
        .iter()
        .map(|state| exposures(state, &adjacency))
        .collect::<Vec<_>>();
    let probabilities = exposure_probabilities(&state_exposures, spec.units.len());
    let observed_treatment = spec
        .units
        .iter()
        .map(|unit| unit.treatment)
        .collect::<Vec<_>>();
    let observed = exposures(&observed_treatment, &adjacency);
    let observed_exposures = spec
        .units
        .iter()
        .zip(&observed)
        .map(|(unit, exposure)| ObservedUnitExposure {
            unit_id: unit.unit_id.clone(),
            own_treated: exposure.own_treated,
            neighbor_any_treated: exposure.neighbor_any_treated,
        })
        .collect::<Vec<_>>();
    let exposure_probability_rows = spec
        .units
        .iter()
        .zip(&probabilities)
        .map(|(unit, probabilities)| UnitExposureProbabilities {
            unit_id: unit.unit_id.clone(),
            probabilities: JointExposureProbabilities {
                untreated_neighbor_untreated: probabilities[0],
                untreated_neighbor_treated: probabilities[1],
                treated_neighbor_untreated: probabilities[2],
                treated_neighbor_treated: probabilities[3],
            },
        })
        .collect::<Vec<_>>();
    let targets = [
        exposure(false, false),
        exposure(false, true),
        exposure(true, false),
        exposure(true, true),
    ];
    let exposure_means = targets
        .iter()
        .map(|&target| {
            estimate_exposure_mean(
                target,
                &spec.units,
                &observed,
                &probabilities,
                &state_exposures,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let contrasts = contrasts(&exposure_means);
    let observed_test = ht_contrast(
        spec.test_exposure_high,
        spec.test_exposure_low,
        &spec.units,
        &observed,
        &probabilities,
    )?;
    let mut rng = ChaCha20Rng::seed_from_u64(spec.seed);
    let mut null_values = Vec::with_capacity(spec.permutations);
    for _ in 0..spec.permutations {
        let state = rng.gen_range(0..states.len());
        null_values.push(ht_contrast(
            spec.test_exposure_high,
            spec.test_exposure_low,
            &spec.units,
            &state_exposures[state],
            &probabilities,
        )?);
    }
    let extreme_permutations = null_values
        .iter()
        .filter(|value| value.abs() >= observed_test.abs())
        .count();
    let p_value = (extreme_permutations + 1) as f64 / (spec.permutations + 1) as f64;
    Ok(RandomizedInterferenceResult {
        format: "marklab.randomized_binary_interference",
        version: 1,
        analysis_level: "clustered_units",
        null_family: "randomized_interference_fixed_outcomes",
        randomization_unit: "complete_cluster_assignment_state",
        unit_count: spec.units.len(),
        cluster_count: clusters.len(),
        design_provenance: spec.design_provenance,
        graph_provenance: spec.graph_provenance,
        assignment_mechanism: "complete_randomization_within_cluster",
        exposure_mapping: "binary_any_treated_neighbor",
        compiled_assumptions: vec![
            "treatment_precedes_outcome",
            "baseline_covariates_are_pre_treatment",
            "all_units_eligible",
            "interference_graph_prespecified",
            "assignment_randomized_independently_by_cluster",
            "fixed_outcomes_under_randomization_test_null",
        ],
        assignment_states: state_count,
        observed_exposures,
        exposure_probabilities: exposure_probability_rows,
        exposure_means,
        contrasts,
        randomization_test: InterferenceRandomizationResult {
            high: spec.test_exposure_high,
            low: spec.test_exposure_low,
            statistic: "horvitz_thompson_exposure_mean_difference",
            observed: observed_test,
            null_values,
            extreme_permutations,
            p_value,
            alternative: "two_sided_inclusive_plus_one",
        },
        unit_assignment_evaluations: evaluations,
        random_seed_namespace: "randomized_binary_interference_v1_chacha20",
        claim_status: "randomized_design_mechanics_only",
    })
}
