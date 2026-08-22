use crate::data::Pattern;

#[derive(Clone, Debug, PartialEq)]
pub struct MarkPairCovarianceBin {
    pub r_min_um: f64,
    pub r_max_um: f64,
    pub value: Option<f64>,
    pub count: usize,
}

/// Average centered binary-mark products in half-open distance bins.
///
/// For prevalence `p_hat`, each contributing pair adds
/// `(mark_i - p_hat) * (mark_j - p_hat)`. This is a mark covariance summary;
/// it is not a density-normalized point-process pair-correlation function.
pub fn mark_pair_covariance(
    pattern: &Pattern,
    bin_width_um: f64,
    max_r_um: f64,
) -> Option<Vec<MarkPairCovarianceBin>> {
    mark_pair_covariance_for_marks(pattern, &pattern.mark, bin_width_um, max_r_um)
}

/// Evaluate mark-pair covariance for an alternate binary mark assignment.
pub fn mark_pair_covariance_for_marks(
    pattern: &Pattern,
    marks: &[u8],
    bin_width_um: f64,
    max_r_um: f64,
) -> Option<Vec<MarkPairCovarianceBin>> {
    if pattern.len() < 2
        || marks.len() != pattern.len()
        || marks.iter().any(|mark| *mark != 0 && *mark != 1)
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

    let mut sums = vec![0.0; n_bins];
    let mut counts = vec![0usize; n_bins];
    let p_hat = marks.iter().filter(|mark| **mark == 1).count() as f64 / marks.len() as f64;
    let centered = marks
        .iter()
        .map(|mark| f64::from(*mark) - p_hat)
        .collect::<Vec<_>>();

    for i in 0..pattern.len() {
        for j in (i + 1)..pattern.len() {
            let dx = pattern.x_um[i] - pattern.x_um[j];
            let dy = pattern.y_um[i] - pattern.y_um[j];
            let distance = (dx * dx + dy * dy).sqrt();
            if distance >= max_r_um {
                continue;
            }
            let bin = (distance / bin_width_um).floor() as usize;
            if let Some(sum) = sums.get_mut(bin) {
                *sum += centered[i] * centered[j];
                counts[bin] += 1;
            }
        }
    }

    Some(
        sums.into_iter()
            .zip(counts)
            .enumerate()
            .map(|(index, (sum, count))| MarkPairCovarianceBin {
                r_min_um: index as f64 * bin_width_um,
                r_max_um: (index + 1) as f64 * bin_width_um,
                value: (count > 0).then_some(sum / count as f64),
                count,
            })
            .collect(),
    )
}
