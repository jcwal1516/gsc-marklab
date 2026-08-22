use crate::{
    common::stats::{median_ignoring_nonfinite, min_max_ignoring_nonfinite},
    data::Pattern,
    errors::{MarklabError, Result},
    output::{BetaPosteriorGroupSummary, BetaPosteriorSummary},
};

/// Summarize independent group prevalences under a fixed Beta(1, 1) prior.
///
/// Each group is modeled separately with a beta posterior for its binomial
/// mark probability. No shared beta-binomial dispersion model is fitted.
pub fn beta_posterior_group_summary(pattern: &Pattern) -> Result<BetaPosteriorSummary> {
    if pattern.is_empty() {
        return Err(MarklabError::Validation(
            "beta posterior group diagnostic requires at least one cell".into(),
        ));
    }

    let prior_alpha = 1.0;
    let prior_beta = 1.0;
    let (posterior_mean, credible_interval_95) =
        beta_posterior_summary(pattern.n_marked(), pattern.len(), prior_alpha, prior_beta)?;
    let groups = group_summaries(pattern, prior_alpha, prior_beta)?;
    let group_posterior_mean_range = posterior_mean_range(&groups);
    let mut diagnostics = vec![
        "Beta posterior group diagnostic with fixed Beta(1,1) prior.".into(),
        "Diagnostic output is exploratory and does not change the primary endpoint.".into(),
    ];
    if pattern.component_id.is_some() && groups.len() >= 2 {
        diagnostics
            .push("Grouped by component_id because multiple components were available.".into());
    } else {
        diagnostics.push(
            "Grouped by coordinate median quadrants because component groups were unavailable."
                .into(),
        );
    }

    Ok(BetaPosteriorSummary {
        diagnostic_name: "beta_posterior_group_summary_v1".into(),
        n_cells: pattern.len(),
        n_marked: pattern.n_marked(),
        prior_alpha,
        prior_beta,
        posterior_mean,
        credible_interval_95,
        group_posterior_mean_range,
        groups,
        diagnostics,
    })
}

fn group_summaries(
    pattern: &Pattern,
    prior_alpha: f64,
    prior_beta: f64,
) -> Result<Vec<BetaPosteriorGroupSummary>> {
    use std::collections::{BTreeMap, BTreeSet};

    let mut groups: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let use_components = pattern
        .component_id
        .as_deref()
        .filter(|ids| ids.len() == pattern.len())
        .map(|ids| ids.iter().copied().collect::<BTreeSet<_>>().len() >= 2)
        .unwrap_or(false);

    if use_components {
        let component_id = pattern.component_id.as_deref().expect("checked above");
        for (index, component) in component_id.iter().copied().enumerate() {
            push_group(
                &mut groups,
                format!("component:{component}"),
                pattern.mark[index],
            );
        }
    } else {
        let median_x = median_ignoring_nonfinite(pattern.x_um.as_ref()).ok_or_else(|| {
            MarklabError::Compute("beta posterior group x-coordinate median is undefined".into())
        })?;
        let median_y = median_ignoring_nonfinite(pattern.y_um.as_ref()).ok_or_else(|| {
            MarklabError::Compute("beta posterior group y-coordinate median is undefined".into())
        })?;
        for index in 0..pattern.len() {
            let x_bin = if pattern.x_um[index] <= median_x {
                "low_x"
            } else {
                "high_x"
            };
            let y_bin = if pattern.y_um[index] <= median_y {
                "low_y"
            } else {
                "high_y"
            };
            push_group(
                &mut groups,
                format!("quadrant:{x_bin}:{y_bin}"),
                pattern.mark[index],
            );
        }
    }

    groups
        .into_iter()
        .map(|(group, (n_cells, n_marked))| {
            let (posterior_mean, credible_interval_95) =
                beta_posterior_summary(n_marked, n_cells, prior_alpha, prior_beta)?;
            Ok(BetaPosteriorGroupSummary {
                group,
                n_cells,
                n_marked,
                posterior_mean,
                credible_interval_95,
            })
        })
        .collect()
}

fn push_group(
    groups: &mut std::collections::BTreeMap<String, (usize, usize)>,
    group: String,
    mark: u8,
) {
    let entry = groups.entry(group).or_insert((0, 0));
    entry.0 += 1;
    entry.1 += usize::from(mark == 1);
}

fn beta_posterior_summary(
    n_marked: usize,
    n_cells: usize,
    prior_alpha: f64,
    prior_beta: f64,
) -> Result<(f64, [f64; 2])> {
    use statrs::distribution::{Beta, ContinuousCDF};

    let alpha = prior_alpha + n_marked as f64;
    let beta = prior_beta + n_cells.saturating_sub(n_marked) as f64;
    let distribution = Beta::new(alpha, beta)
        .map_err(|err| MarklabError::Compute(format!("invalid beta posterior: {err}")))?;
    let mean = alpha / (alpha + beta);
    Ok((
        mean,
        [
            distribution.inverse_cdf(0.025),
            distribution.inverse_cdf(0.975),
        ],
    ))
}

fn posterior_mean_range(groups: &[BetaPosteriorGroupSummary]) -> f64 {
    if groups.len() < 2 {
        return 0.0;
    }
    let posterior_means = groups
        .iter()
        .map(|group| group.posterior_mean)
        .collect::<Vec<_>>();
    min_max_ignoring_nonfinite(&posterior_means)
        .map(|(min, max)| max - min)
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::PatternMeta;

    #[test]
    fn returns_deterministic_component_posteriors() {
        let mut pattern = Pattern::from_arrays(
            vec![0.0, 1.0, 2.0, 10.0, 11.0, 12.0],
            vec![0.0, 0.0, 0.0, 10.0, 10.0, 10.0],
            vec![1, 1, 0, 1, 0, 0],
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
        .expect("pattern");
        pattern.component_id = Some(vec![1, 1, 1, 2, 2, 2].into_boxed_slice());

        let output =
            beta_posterior_group_summary(&pattern).expect("beta posterior group diagnostic");

        assert_eq!(output.diagnostic_name, "beta_posterior_group_summary_v1");
        assert_eq!(output.n_cells, 6);
        assert_eq!(output.n_marked, 3);
        assert!((output.posterior_mean - 0.5).abs() < 1.0e-12);
        assert_eq!(output.groups.len(), 2);
        assert_eq!(output.groups[0].group, "component:1");
        assert!((output.groups[0].posterior_mean - 0.6).abs() < 1.0e-12);
        assert_eq!(output.groups[1].group, "component:2");
        assert!((output.groups[1].posterior_mean - 0.4).abs() < 1.0e-12);
        assert!((output.group_posterior_mean_range - 0.2).abs() < 1.0e-12);
        assert!(output.credible_interval_95[0] < output.posterior_mean);
        assert!(output.credible_interval_95[1] > output.posterior_mean);
    }

    #[test]
    fn coordinate_groups_use_average_even_medians() {
        let pattern = Pattern::from_arrays(
            vec![0.0, 1.0, 100.0, 101.0],
            vec![0.0, 1.0, 100.0, 101.0],
            vec![1, 0, 1, 0],
            PatternMeta {
                case_id: "case_even_median".into(),
                timepoint: "post".into(),
                protein: "MSH6".into(),
                slide_id: None,
                section_id: None,
                stain_batch: None,
                block_id: None,
                region_id: None,
            },
        )
        .expect("pattern");

        let output =
            beta_posterior_group_summary(&pattern).expect("beta posterior group diagnostic");
        let mut group_sizes = output
            .groups
            .iter()
            .map(|group| group.n_cells)
            .collect::<Vec<_>>();
        group_sizes.sort_unstable();

        assert_eq!(group_sizes, vec![2, 2]);
    }
}
