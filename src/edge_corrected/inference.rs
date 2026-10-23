//! Conditional-CSR simulation and envelope construction shared by exact pair-correlation estimators.

use crate::{
    classical::{
        sample_conditional_csr, ClassicalInferenceSummary, ClassicalNullDesign,
        ClassicalSpatialLimits, SpatialGeometryPlan2D,
    },
    common::{
        finite::canonical_zero,
        seeds::{derive_seed, SeedEndpoint},
    },
    permutation::envelopes::GlobalEnvelope,
    ObservationWindow2D,
};

pub(crate) struct CurveEvaluation<P> {
    pub(crate) points: Vec<P>,
    pub(crate) values: Vec<f64>,
    pub(crate) eligible: Vec<bool>,
}

pub(crate) struct CsrResult<P> {
    pub(crate) curve: Vec<P>,
    pub(crate) inference: Option<ClassicalInferenceSummary>,
    pub(crate) candidate_draws: usize,
}

pub(crate) enum CsrError<E> {
    Allocation,
    Simulation(String),
    Evaluation(E),
    Inference(String),
}

pub(crate) fn conditional_csr_envelope<P, E>(
    window: &ObservationWindow2D,
    null: &ClassicalNullDesign,
    limits: ClassicalSpatialLimits,
    seed_endpoint: SeedEndpoint,
    observed: CurveEvaluation<P>,
    mut evaluate: impl FnMut(&[f64], &[f64]) -> Result<CurveEvaluation<P>, E>,
    mut attach_bounds: impl FnMut(&mut P, f64, f64),
) -> Result<CsrResult<P>, CsrError<E>> {
    let CurveEvaluation {
        mut points,
        values,
        mut eligible,
    } = observed;
    let mut simulated = Vec::new();
    simulated
        .try_reserve_exact(null.simulations)
        .map_err(|_| CsrError::Allocation)?;
    let mut candidate_draws = 0;
    for simulation in 0..null.simulations {
        let seed = derive_seed(null.seed, seed_endpoint, simulation);
        let (x, y) = sample_conditional_csr(
            window,
            null.conditioned_point_count,
            seed,
            &mut candidate_draws,
            limits.maximum_csr_draws,
        )
        .map_err(|error| CsrError::Simulation(error.to_string()))?;
        SpatialGeometryPlan2D::new(&x, &y, window, limits)
            .map_err(|error| CsrError::Simulation(error.to_string()))?;
        let evaluated = evaluate(&x, &y).map_err(CsrError::Evaluation)?;
        for (joint, current) in eligible.iter_mut().zip(&evaluated.eligible) {
            *joint &= *current;
        }
        simulated.push(evaluated.values);
    }
    let inference = if eligible.iter().any(|eligible| *eligible) {
        let envelope = GlobalEnvelope::from_curves_with_eligibility(
            &values, &simulated, null.alpha, &eligible,
        )
        .map_err(|error| CsrError::Inference(error.to_string()))?;
        for (index, point) in points.iter_mut().enumerate() {
            if eligible[index] {
                attach_bounds(
                    point,
                    canonical_zero(envelope.lower[index]),
                    canonical_zero(envelope.upper[index]),
                );
            }
        }
        Some(ClassicalInferenceSummary {
            p_global: canonical_zero(envelope.p_global),
            erl_depth: canonical_zero(envelope.erl_depth),
            critical_depth: canonical_zero(envelope.critical_depth),
            simulations: envelope.n_permutations,
            eligible_radius_count: eligible.iter().filter(|value| **value).count(),
        })
    } else {
        None
    };
    Ok(CsrResult {
        curve: points,
        inference,
        candidate_draws,
    })
}
