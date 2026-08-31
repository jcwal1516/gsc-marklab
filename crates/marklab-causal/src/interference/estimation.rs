use super::{
    validation::CompiledCluster, CausalError, CausalUnit, ExposureContrast, ExposureMeanEstimate,
    JointBinaryExposure,
};

pub(super) fn enumerate_assignments(
    unit_count: usize,
    clusters: &[CompiledCluster],
) -> Vec<Vec<bool>> {
    let mut states = vec![vec![false; unit_count]];
    for cluster in clusters {
        let choices = combinations(&cluster.members, cluster.treated_units);
        let mut expanded = Vec::with_capacity(states.len() * choices.len());
        for state in &states {
            for choice in &choices {
                let mut next = state.clone();
                for &index in choice {
                    next[index] = true;
                }
                expanded.push(next);
            }
        }
        states = expanded;
    }
    states
}

fn combinations(values: &[usize], choose: usize) -> Vec<Vec<usize>> {
    fn visit(
        values: &[usize],
        choose: usize,
        start: usize,
        current: &mut Vec<usize>,
        result: &mut Vec<Vec<usize>>,
    ) {
        if current.len() == choose {
            result.push(current.clone());
            return;
        }
        let needed = choose - current.len();
        for index in start..=values.len() - needed {
            current.push(values[index]);
            visit(values, choose, index + 1, current, result);
            current.pop();
        }
    }
    let mut result = Vec::new();
    visit(
        values,
        choose,
        0,
        &mut Vec::with_capacity(choose),
        &mut result,
    );
    result
}

pub(super) fn exposures(treatment: &[bool], adjacency: &[Vec<usize>]) -> Vec<JointBinaryExposure> {
    treatment
        .iter()
        .enumerate()
        .map(|(index, &own_treated)| JointBinaryExposure {
            own_treated,
            neighbor_any_treated: adjacency[index].iter().any(|&neighbor| treatment[neighbor]),
        })
        .collect()
}

pub(super) fn exposure_probabilities(
    states: &[Vec<JointBinaryExposure>],
    unit_count: usize,
) -> Vec<[f64; 4]> {
    let mut counts = vec![[0_u64; 4]; unit_count];
    for state in states {
        for (unit, exposure) in state.iter().enumerate() {
            counts[unit][exposure_index(*exposure)] += 1;
        }
    }
    counts
        .into_iter()
        .map(|counts| counts.map(|count| count as f64 / states.len() as f64))
        .collect()
}

pub(super) fn estimate_exposure_mean(
    target: JointBinaryExposure,
    units: &[CausalUnit],
    observed: &[JointBinaryExposure],
    probabilities: &[[f64; 4]],
    state_exposures: &[Vec<JointBinaryExposure>],
) -> Result<ExposureMeanEstimate, CausalError> {
    let index = exposure_index(target);
    let eligible_units = probabilities.iter().filter(|row| row[index] > 0.0).count();
    if eligible_units == 0 {
        return Ok(ExposureMeanEstimate {
            exposure: target,
            eligible_units: 0,
            observed_units: 0,
            ht_mean: None,
            hajek_mean: None,
            exact_fixed_outcome_ht_sd: None,
            status: "unavailable_no_positivity",
        });
    }
    let (numerator, denominator, observed_units) =
        weighted_terms(target, units, observed, probabilities);
    let ht_mean = numerator / eligible_units as f64;
    let hajek_mean = (denominator > 0.0).then_some(numerator / denominator);
    let rerandomized = state_exposures
        .iter()
        .map(|state| {
            let (numerator, _, _) = weighted_terms(target, units, state, probabilities);
            numerator / eligible_units as f64
        })
        .collect::<Vec<_>>();
    let mean = rerandomized.iter().sum::<f64>() / rerandomized.len() as f64;
    let variance = rerandomized
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / rerandomized.len() as f64;
    if !ht_mean.is_finite() || hajek_mean.is_some_and(|value| !value.is_finite()) {
        return Err(CausalError::Numerical("exposure mean is not finite".into()));
    }
    Ok(ExposureMeanEstimate {
        exposure: target,
        eligible_units,
        observed_units,
        ht_mean: Some(ht_mean),
        hajek_mean,
        exact_fixed_outcome_ht_sd: Some(variance.sqrt()),
        status: if hajek_mean.is_some() {
            "estimated"
        } else {
            "ht_only_no_observed_hajek_denominator"
        },
    })
}

fn weighted_terms(
    target: JointBinaryExposure,
    units: &[CausalUnit],
    observed: &[JointBinaryExposure],
    probabilities: &[[f64; 4]],
) -> (f64, f64, usize) {
    let index = exposure_index(target);
    let mut numerator = 0.0;
    let mut denominator = 0.0;
    let mut observed_units = 0;
    for unit in 0..units.len() {
        if observed[unit] == target && probabilities[unit][index] > 0.0 {
            let inverse = 1.0 / probabilities[unit][index];
            numerator += units[unit].outcome * inverse;
            denominator += inverse;
            observed_units += 1;
        }
    }
    (numerator, denominator, observed_units)
}

pub(super) fn contrasts(means: &[ExposureMeanEstimate]) -> Vec<ExposureContrast> {
    [
        (
            "direct_neighbor_untreated",
            exposure(true, false),
            exposure(false, false),
        ),
        (
            "direct_neighbor_treated",
            exposure(true, true),
            exposure(false, true),
        ),
        (
            "spillover_untreated",
            exposure(false, true),
            exposure(false, false),
        ),
        (
            "spillover_treated",
            exposure(true, true),
            exposure(true, false),
        ),
        ("total_joint", exposure(true, true), exposure(false, false)),
    ]
    .into_iter()
    .map(|(name, high, low)| {
        let high_mean = means[exposure_index(high)].hajek_mean;
        let low_mean = means[exposure_index(low)].hajek_mean;
        let estimate = high_mean.zip(low_mean).map(|(high, low)| high - low);
        ExposureContrast {
            name,
            high,
            low,
            estimate,
            status: if estimate.is_some() {
                "estimated"
            } else {
                "unavailable_observed_exposure_denominator"
            },
        }
    })
    .collect()
}

pub(super) fn ht_contrast(
    high: JointBinaryExposure,
    low: JointBinaryExposure,
    units: &[CausalUnit],
    observed: &[JointBinaryExposure],
    probabilities: &[[f64; 4]],
) -> Result<f64, CausalError> {
    let high_eligible = probabilities
        .iter()
        .filter(|row| row[exposure_index(high)] > 0.0)
        .count();
    let low_eligible = probabilities
        .iter()
        .filter(|row| row[exposure_index(low)] > 0.0)
        .count();
    if high_eligible == 0 || low_eligible == 0 {
        return Err(CausalError::Invalid(
            "randomization-test exposures must both satisfy positivity".into(),
        ));
    }
    let (high_numerator, _, _) = weighted_terms(high, units, observed, probabilities);
    let (low_numerator, _, _) = weighted_terms(low, units, observed, probabilities);
    let statistic = high_numerator / high_eligible as f64 - low_numerator / low_eligible as f64;
    if !statistic.is_finite() {
        return Err(CausalError::Numerical(
            "randomization-test statistic is not finite".into(),
        ));
    }
    Ok(statistic)
}

pub(super) const fn exposure(own_treated: bool, neighbor_any_treated: bool) -> JointBinaryExposure {
    JointBinaryExposure {
        own_treated,
        neighbor_any_treated,
    }
}

pub(super) const fn exposure_index(exposure: JointBinaryExposure) -> usize {
    exposure.own_treated as usize * 2 + exposure.neighbor_any_treated as usize
}
