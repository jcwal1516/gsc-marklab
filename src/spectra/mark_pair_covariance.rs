use crate::{data::Pattern, geom::spatial_index::SpatialIndex2D};

#[cfg(test)]
thread_local! {
    static PLAN_BUILD_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_plan_build_call_count() {
    PLAN_BUILD_CALLS.set(0);
}

#[cfg(test)]
pub(crate) fn plan_build_call_count() -> usize {
    PLAN_BUILD_CALLS.get()
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarkPairCovarianceBin {
    pub r_min_um: f64,
    pub r_max_um: f64,
    pub value: Option<f64>,
    pub count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PairBin {
    source: usize,
    target: usize,
    bin_index: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarkPairCovariancePlan {
    point_count: usize,
    pairs: Vec<PairBin>,
    bin_edges: Vec<f64>,
    pair_counts: Vec<usize>,
}

impl MarkPairCovariancePlan {
    #[cfg(test)]
    pub fn new(pattern: &Pattern, bin_width_um: f64, max_r_um: f64) -> Option<Self> {
        let index = SpatialIndex2D::new(&pattern.x_um, &pattern.y_um).ok()?;
        Self::new_with_index(pattern, &index, bin_width_um, max_r_um)
    }

    pub(crate) fn new_with_index(
        pattern: &Pattern,
        index: &SpatialIndex2D,
        bin_width_um: f64,
        max_r_um: f64,
    ) -> Option<Self> {
        #[cfg(test)]
        PLAN_BUILD_CALLS.set(PLAN_BUILD_CALLS.get() + 1);
        if pattern.len() < 2
            || index.len() != pattern.len()
            || bin_width_um <= 0.0
            || max_r_um <= 0.0
            || !bin_width_um.is_finite()
            || !max_r_um.is_finite()
        {
            return None;
        }
        let n_bins = (max_r_um / bin_width_um).ceil() as usize;
        if n_bins == 0 {
            return None;
        }

        let mut pairs = Vec::new();
        let mut pair_counts = vec![0usize; n_bins];
        for source in 0..pattern.len() {
            index
                .visit_within_radius(source, max_r_um, |neighbor| {
                    if neighbor.index <= source || neighbor.distance_um >= max_r_um {
                        return;
                    }
                    let bin_index = (neighbor.distance_um / bin_width_um).floor() as usize;
                    if bin_index < n_bins {
                        pairs.push(PairBin {
                            source,
                            target: neighbor.index,
                            bin_index,
                        });
                        pair_counts[bin_index] += 1;
                    }
                })
                .ok()?;
        }
        pairs.sort_unstable_by_key(|pair| (pair.source, pair.target));
        let bin_edges = (0..=n_bins)
            .map(|index| index as f64 * bin_width_um)
            .collect();

        Some(Self {
            point_count: pattern.len(),
            pairs,
            bin_edges,
            pair_counts,
        })
    }

    pub fn evaluate(&self, marks: &[u8]) -> Option<Vec<MarkPairCovarianceBin>> {
        if marks.len() != self.point_count
            || marks.len() < 2
            || marks.iter().any(|mark| *mark != 0 && *mark != 1)
        {
            return None;
        }
        let p_hat = marks.iter().filter(|mark| **mark == 1).count() as f64 / marks.len() as f64;
        let centered = marks
            .iter()
            .map(|mark| f64::from(*mark) - p_hat)
            .collect::<Vec<_>>();
        let mut sums = vec![0.0; self.pair_counts.len()];
        for pair in &self.pairs {
            *sums.get_mut(pair.bin_index)? +=
                centered.get(pair.source)? * centered.get(pair.target)?;
        }

        Some(
            sums.into_iter()
                .zip(self.pair_counts.iter().copied())
                .enumerate()
                .map(|(index, (sum, count))| MarkPairCovarianceBin {
                    r_min_um: self.bin_edges[index],
                    r_max_um: self.bin_edges[index + 1],
                    value: (count > 0).then_some(sum / count as f64),
                    count,
                })
                .collect(),
        )
    }
}

/// Average centered binary-mark products in half-open distance bins.
///
/// For prevalence `p_hat`, each contributing pair adds
/// `(mark_i - p_hat) * (mark_j - p_hat)`. This is a mark covariance summary;
/// it is not a density-normalized point-process pair-correlation function.
#[cfg(test)]
pub fn mark_pair_covariance(
    pattern: &Pattern,
    bin_width_um: f64,
    max_r_um: f64,
) -> Option<Vec<MarkPairCovarianceBin>> {
    mark_pair_covariance_for_marks(pattern, &pattern.mark, bin_width_um, max_r_um)
}

/// Evaluate mark-pair covariance for an alternate binary mark assignment.
#[cfg(test)]
pub fn mark_pair_covariance_for_marks(
    pattern: &Pattern,
    marks: &[u8],
    bin_width_um: f64,
    max_r_um: f64,
) -> Option<Vec<MarkPairCovarianceBin>> {
    let plan = MarkPairCovariancePlan::new(pattern, bin_width_um, max_r_um)?;
    plan.evaluate(marks)
}
