//! Shared pair traversal for exact isotropic and translation estimators.
//!
//! Correction implementations retain the original arithmetic order and work budgets;
//! callers retain their result types, seed domains, and storage estimates.

pub(crate) mod document;
pub(crate) mod inference;

use std::collections::BTreeMap;

use crate::{
    cross_pair_correlation::{
        CategoricalCrossPairCorrelationConfig, CategoricalCrossPairCorrelationError,
    },
    isotropic_spatial::edge::IsotropicArcBudget,
    mark_pair_plan::MarkPairPlan,
    marks::DeclaredScalarPatternInput,
    pair_correlation::epanechnikov_weight,
    spatial::{IsotropicSpatialError, TranslationSpatialError},
    translation_spatial::edge::TranslationOverlapBudget,
    ObservationWindow2D,
};

pub(crate) struct PairGeometry {
    left: usize,
    right: usize,
    left_xy: [f64; 2],
    right_xy: [f64; 2],
    distance: f64,
}

pub(crate) trait EdgeWeighting {
    type Error: std::fmt::Display;
    const NAME: &'static str;
    const EVALUATIONS_PER_PAIR: usize;
    fn charge_pair(&mut self) -> Result<(), Self::Error>;
    fn pair(
        &mut self,
        window: &ObservationWindow2D,
        pair: &PairGeometry,
    ) -> Result<[f64; 2], Self::Error>;
    fn measure(values: [f64; 2]) -> f64;
    fn ordered_weight(values: [f64; 2], area: f64) -> f64;
    fn directed_weight(value: f64, area: f64, kernel: f64) -> f64;
    fn overflow() -> Self::Error;
    fn invalid(reason: String) -> Self::Error;
}

impl EdgeWeighting for IsotropicArcBudget {
    type Error = IsotropicSpatialError;
    const NAME: &'static str = "isotropic";
    const EVALUATIONS_PER_PAIR: usize = 2;
    fn charge_pair(&mut self) -> Result<(), Self::Error> {
        self.charge_pair()
    }
    fn pair(
        &mut self,
        window: &ObservationWindow2D,
        pair: &PairGeometry,
    ) -> Result<[f64; 2], Self::Error> {
        Ok([
            self.visible_fraction(
                window,
                pair.left_xy[0],
                pair.left_xy[1],
                pair.distance,
                pair.left,
                pair.right,
            )?,
            self.visible_fraction(
                window,
                pair.right_xy[0],
                pair.right_xy[1],
                pair.distance,
                pair.right,
                pair.left,
            )?,
        ])
    }
    fn measure(values: [f64; 2]) -> f64 {
        values[0] + values[1]
    }
    fn ordered_weight(values: [f64; 2], _area: f64) -> f64 {
        1.0 / values[0] + 1.0 / values[1]
    }
    fn directed_weight(value: f64, _area: f64, kernel: f64) -> f64 {
        kernel / value
    }
    fn overflow() -> Self::Error {
        IsotropicSpatialError::SizeOverflow
    }
    fn invalid(reason: String) -> Self::Error {
        IsotropicSpatialError::InvalidPattern { reason }
    }
}

impl EdgeWeighting for TranslationOverlapBudget {
    type Error = TranslationSpatialError;
    const NAME: &'static str = "translation";
    const EVALUATIONS_PER_PAIR: usize = 1;
    fn charge_pair(&mut self) -> Result<(), Self::Error> {
        self.charge_pair()
    }
    fn pair(
        &mut self,
        window: &ObservationWindow2D,
        pair: &PairGeometry,
    ) -> Result<[f64; 2], Self::Error> {
        Ok([
            self.overlap(
                window,
                pair.right_xy[0] - pair.left_xy[0],
                pair.right_xy[1] - pair.left_xy[1],
                pair.left,
                pair.right,
            )?,
            0.0,
        ])
    }
    fn measure(values: [f64; 2]) -> f64 {
        values[0]
    }
    fn ordered_weight(values: [f64; 2], area: f64) -> f64 {
        2.0 * area / values[0]
    }
    fn directed_weight(value: f64, area: f64, kernel: f64) -> f64 {
        kernel * area / value
    }
    fn overflow() -> Self::Error {
        TranslationSpatialError::SizeOverflow
    }
    fn invalid(reason: String) -> Self::Error {
        TranslationSpatialError::InvalidPattern { reason }
    }
}

pub(crate) struct CurveWork {
    /// Directed counts for isotropic correction, unordered counts for translation.
    pub(crate) pair_counts: Vec<usize>,
    pub(crate) evaluations: Vec<usize>,
    pub(crate) measure_sums: Vec<f64>,
    pub(crate) weighted_sums: Vec<f64>,
}

/// `bandwidth = None` accumulates K/L suffix counts; `Some` uses compact Epanechnikov support.
pub(crate) fn estimate_curve<C: EdgeWeighting>(
    x: &[f64],
    y: &[f64],
    window: &ObservationWindow2D,
    radii: &[f64],
    bandwidth: Option<f64>,
    correction: &mut C,
) -> Result<CurveWork, C::Error> {
    let mut work = CurveWork {
        pair_counts: vec![0; radii.len()],
        evaluations: vec![0; radii.len()],
        measure_sums: vec![0.0; radii.len()],
        weighted_sums: vec![0.0; radii.len()],
    };
    let maximum_radius = *radii.last().expect("validated radii");
    for left in 0..x.len() {
        for right in (left + 1)..x.len() {
            correction.charge_pair()?;
            let distance = (x[right] - x[left]).hypot(y[right] - y[left]);
            let (start, end) = if let Some(width) = bandwidth {
                if distance >= maximum_radius + width {
                    continue;
                }
                (
                    radii.partition_point(|r| *r <= distance - width),
                    radii.partition_point(|r| *r < distance + width),
                )
            } else {
                if distance > maximum_radius {
                    continue;
                }
                let start = radii.partition_point(|r| *r < distance);
                (start, (start + 1).min(radii.len()))
            };
            if start == end {
                continue;
            }
            let values = correction.pair(
                window,
                &PairGeometry {
                    left,
                    right,
                    left_xy: [x[left], y[left]],
                    right_xy: [x[right], y[right]],
                    distance,
                },
            )?;
            let measure = C::measure(values);
            let weight = C::ordered_weight(values, window.area_um2());
            if bandwidth.is_some() && C::EVALUATIONS_PER_PAIR == 1 && !weight.is_finite() {
                return Err(C::invalid("translation pair weight is non-finite".into()));
            }
            for (index, radius) in radii.iter().enumerate().take(end).skip(start) {
                work.pair_counts[index] = work.pair_counts[index]
                    .checked_add(C::EVALUATIONS_PER_PAIR)
                    .ok_or_else(C::overflow)?;
                work.evaluations[index] = work.evaluations[index]
                    .checked_add(C::EVALUATIONS_PER_PAIR)
                    .ok_or_else(C::overflow)?;
                work.measure_sums[index] += measure;
                work.weighted_sums[index] += match bandwidth {
                    Some(width) => {
                        epanechnikov_weight(*radius, distance, width)
                            .expect("strict compact support")
                            * weight
                    }
                    None => weight,
                };
                if !work.measure_sums[index].is_finite() || !work.weighted_sums[index].is_finite() {
                    let quantity = if bandwidth.is_some() {
                        "pair-correlation"
                    } else {
                        "pair"
                    };
                    return Err(C::invalid(format!(
                        "{} {quantity} accumulation is non-finite",
                        C::NAME
                    )));
                }
            }
        }
    }
    if bandwidth.is_none() {
        for values in [&mut work.pair_counts, &mut work.evaluations] {
            for index in 1..values.len() {
                values[index] = values[index]
                    .checked_add(values[index - 1])
                    .ok_or_else(C::overflow)?;
            }
        }
        for values in [&mut work.measure_sums, &mut work.weighted_sums] {
            for index in 1..values.len() {
                values[index] += values[index - 1];
                if !values[index].is_finite() {
                    return Err(C::invalid(format!(
                        "{} cumulative sum is non-finite",
                        C::NAME
                    )));
                }
            }
        }
    }
    Ok(work)
}

pub(crate) fn cache_corrections<C: EdgeWeighting>(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    config: &CategoricalCrossPairCorrelationConfig,
    plan: &MarkPairPlan,
    correction: &mut C,
) -> Result<BTreeMap<(usize, usize), f64>, C::Error> {
    let pattern = input.pattern();
    let mut cache = BTreeMap::new();
    for pair in &plan.pairs {
        if pair.source >= pair.target {
            continue;
        }
        correction.charge_pair()?;
        let start = config
            .radii_um()
            .partition_point(|radius| *radius <= pair.distance_um - config.bandwidth_um());
        let end = config
            .radii_um()
            .partition_point(|radius| *radius < pair.distance_um + config.bandwidth_um());
        if start == end {
            continue;
        }
        let values = correction.pair(
            window,
            &PairGeometry {
                left: pair.source,
                right: pair.target,
                left_xy: [pattern.x_um[pair.source], pattern.y_um[pair.source]],
                right_xy: [pattern.x_um[pair.target], pattern.y_um[pair.target]],
                distance: pair.distance_um,
            },
        )?;
        cache.insert((pair.source, pair.target), values[0]);
        if C::EVALUATIONS_PER_PAIR == 2 {
            cache.insert((pair.target, pair.source), values[1]);
        }
    }
    Ok(cache)
}

pub(crate) struct CrossWork {
    pub(crate) counts: Vec<usize>,
    pub(crate) measure_sums: Vec<f64>,
    pub(crate) weighted_sums: Vec<f64>,
}

pub(crate) fn estimate_cross<C: EdgeWeighting>(
    codes: &[u32],
    levels: (u32, u32),
    area: f64,
    plan: &MarkPairPlan,
    cache: &BTreeMap<(usize, usize), f64>,
    config: &CategoricalCrossPairCorrelationConfig,
) -> Result<CrossWork, CategoricalCrossPairCorrelationError> {
    let mut work = CrossWork {
        counts: vec![0; config.radii_um().len()],
        measure_sums: vec![0.0; config.radii_um().len()],
        weighted_sums: vec![0.0; config.radii_um().len()],
    };
    for pair in &plan.pairs {
        if codes[pair.source] != levels.0 || codes[pair.target] != levels.1 {
            continue;
        }
        let key = if C::EVALUATIONS_PER_PAIR == 2 {
            (pair.source, pair.target)
        } else {
            (pair.source.min(pair.target), pair.source.max(pair.target))
        };
        let Some(value) = cache.get(&key).copied() else {
            continue;
        };
        let start = config
            .radii_um()
            .partition_point(|radius| *radius <= pair.distance_um - config.bandwidth_um());
        let end = config
            .radii_um()
            .partition_point(|radius| *radius < pair.distance_um + config.bandwidth_um());
        for index in start..end {
            let kernel = epanechnikov_weight(
                config.radii_um()[index],
                pair.distance_um,
                config.bandwidth_um(),
            )
            .expect("strict compact support");
            work.counts[index] = work.counts[index]
                .checked_add(1)
                .ok_or(CategoricalCrossPairCorrelationError::SizeOverflow)?;
            work.measure_sums[index] += value;
            work.weighted_sums[index] += C::directed_weight(value, area, kernel);
            if C::EVALUATIONS_PER_PAIR == 2
                && (!work.measure_sums[index].is_finite() || !work.weighted_sums[index].is_finite())
            {
                return Err(CategoricalCrossPairCorrelationError::Dependency(format!(
                    "{} cross-g accumulation is non-finite",
                    C::NAME
                )));
            }
        }
    }
    Ok(work)
}
