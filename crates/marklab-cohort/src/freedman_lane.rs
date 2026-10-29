use crate::{
    ols::{OlsContext, OlsFit},
    CohortInferenceError, InferenceDesign, PermutationAlternative,
};

pub(crate) struct FreedmanLaneFit {
    pub(crate) observed: OlsFit,
    pub(crate) p_value: f64,
}

pub(crate) fn fit(
    outcomes: &[f64],
    reduced: &[Vec<f64>],
    full: &[Vec<f64>],
    target_index: usize,
    absorbed: usize,
    design: &InferenceDesign,
    alternative: PermutationAlternative,
) -> Result<FreedmanLaneFit, CohortInferenceError> {
    let reduced_fit = OlsContext::Covariate.fit(reduced, outcomes, None, absorbed)?;
    let observed = OlsContext::Covariate.fit(full, outcomes, Some(target_index), absorbed)?;
    let residuals = outcomes
        .iter()
        .zip(&reduced_fit.fitted)
        .map(|(outcome, fitted)| outcome - fitted)
        .collect::<Vec<_>>();
    let mut lower_tail = 0usize;
    let mut upper_tail = 0usize;
    let mut permuted = vec![0.0; outcomes.len()];
    for replicate in 0..design.permutations() {
        let indices = design
            .permuted_indices(replicate)
            .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?;
        for index in 0..outcomes.len() {
            permuted[index] = reduced_fit.fitted[index] + residuals[indices[index]];
        }
        let statistic = OlsContext::Covariate
            .fit(full, &permuted, Some(target_index), absorbed)?
            .target_statistic;
        lower_tail += usize::from(statistic <= observed.target_statistic);
        upper_tail += usize::from(statistic >= observed.target_statistic);
    }
    let denominator = (design.permutations() + 1) as f64;
    let p_value = match alternative {
        PermutationAlternative::Less => (lower_tail as f64 + 1.0) / denominator,
        PermutationAlternative::Greater => (upper_tail as f64 + 1.0) / denominator,
        PermutationAlternative::TwoSided => {
            (2.0 * ((lower_tail.min(upper_tail) as f64 + 1.0) / denominator)).min(1.0)
        }
    };
    Ok(FreedmanLaneFit { observed, p_value })
}
