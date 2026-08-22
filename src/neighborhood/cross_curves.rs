use crate::{
    common::seeds::{derive_seed, SeedEndpoint},
    errors::{MarklabError, Result},
    inference::scalar_pvalues::{permutation_p_value_with_spec, PermutationTestSpec, Tail},
    multimodal::cell_table::{primary_label, FusedCell},
    output::{CrossInteractionCurve, CrossInteractionPoint},
};

use super::label_permutation::shuffle_labels_within_sections;

/// Build a deterministic cross-label distance curve over registered cell coordinates.
///
/// Bins are half-open intervals `[r_min, r_max)`. Distances greater than or
/// equal to `max_r_um` are excluded, including the right edge of the final
/// partial bin.
pub fn cross_interaction_curve(
    cells: &[FusedCell],
    label_a: &str,
    label_b: &str,
    bin_width_um: f64,
    max_r_um: f64,
    permutations: usize,
    seed: u64,
) -> Result<CrossInteractionCurve> {
    validate_curve_config(label_a, label_b, bin_width_um, max_r_um, permutations)?;
    validate_registered_coordinates(cells)?;

    let bin_count = (max_r_um / bin_width_um).ceil() as usize;
    let labels = cells.iter().map(primary_label).collect::<Vec<_>>();
    let sections = cells
        .iter()
        .map(|cell| cell.source_section)
        .collect::<Vec<_>>();
    let observed_counts = count_pair_bins(
        cells,
        &labels,
        label_a,
        label_b,
        bin_width_um,
        max_r_um,
        bin_count,
    );
    let null_summary = permutation_summary(
        cells,
        &labels,
        &sections,
        PairSpec {
            label_a,
            label_b,
            bin_width_um,
            max_r_um,
            bin_count,
        },
        &observed_counts,
        permutations,
        seed,
    )?;

    let points = observed_counts
        .iter()
        .enumerate()
        .map(|(index, count)| {
            let r_min_um = index as f64 * bin_width_um;
            CrossInteractionPoint {
                r_min_um,
                r_max_um: (r_min_um + bin_width_um).min(max_r_um),
                value: Some(*count as f64),
                inference_eligible: true,
                lower_global_envelope: Some(null_summary.lower[index] as f64),
                upper_global_envelope: Some(null_summary.upper[index] as f64),
                count: *count,
            }
        })
        .collect::<Vec<_>>();

    Ok(CrossInteractionCurve {
        label_a: label_a.to_owned(),
        label_b: label_b.to_owned(),
        points,
        p_global: Some(null_summary.p_global),
    })
}

fn count_pair_bins(
    cells: &[FusedCell],
    labels: &[Option<String>],
    label_a: &str,
    label_b: &str,
    bin_width_um: f64,
    max_r_um: f64,
    bin_count: usize,
) -> Vec<usize> {
    let mut counts = vec![0usize; bin_count];
    for source in 0..cells.len() {
        for target in (source + 1)..cells.len() {
            if !labels_match_pair(
                labels[source].as_deref(),
                labels[target].as_deref(),
                label_a,
                label_b,
            ) {
                continue;
            }

            let distance_um = euclidean_distance_um(&cells[source], &cells[target]);
            if distance_um >= max_r_um {
                continue;
            }

            let bin_index = (distance_um / bin_width_um).floor() as usize;
            if let Some(count) = counts.get_mut(bin_index) {
                *count += 1;
            }
        }
    }

    counts
}

#[derive(Clone, Copy)]
struct PairSpec<'a> {
    label_a: &'a str,
    label_b: &'a str,
    bin_width_um: f64,
    max_r_um: f64,
    bin_count: usize,
}

struct NullSummary {
    lower: Vec<usize>,
    upper: Vec<usize>,
    p_global: f64,
}

fn permutation_summary(
    cells: &[FusedCell],
    labels: &[Option<String>],
    sections: &[crate::multimodal::cell_table::CellSection],
    pair: PairSpec<'_>,
    observed_counts: &[usize],
    permutations: usize,
    seed: u64,
) -> Result<NullSummary> {
    let mut lower = observed_counts.to_vec();
    let mut upper = observed_counts.to_vec();
    let observed_stat = max_bin_count(observed_counts);
    let mut null_statistics = Vec::with_capacity(permutations);
    let mut shuffled = labels.to_vec();

    for permutation in 0..permutations {
        shuffle_labels_within_sections(
            labels,
            sections,
            &mut shuffled,
            derive_seed(seed, SeedEndpoint::CrossInteraction, permutation),
        );
        let null_counts = count_pair_bins(
            cells,
            &shuffled,
            pair.label_a,
            pair.label_b,
            pair.bin_width_um,
            pair.max_r_um,
            pair.bin_count,
        );
        null_statistics.push(max_bin_count(&null_counts) as f64);
        for index in 0..pair.bin_count {
            lower[index] = lower[index].min(null_counts[index]);
            upper[index] = upper[index].max(null_counts[index]);
        }
    }

    Ok(NullSummary {
        lower,
        upper,
        p_global: permutation_p_value_with_spec(
            observed_stat as f64,
            &null_statistics,
            PermutationTestSpec::new(Tail::OneSidedHigh, 1),
        )?,
    })
}

fn max_bin_count(counts: &[usize]) -> usize {
    counts.iter().copied().max().unwrap_or(0)
}

fn validate_curve_config(
    label_a: &str,
    label_b: &str,
    bin_width_um: f64,
    max_r_um: f64,
    permutations: usize,
) -> Result<()> {
    if label_a.trim().is_empty() || label_b.trim().is_empty() {
        return Err(MarklabError::Config(
            "cross interaction curve labels must be non-empty".into(),
        ));
    }
    if !bin_width_um.is_finite() || bin_width_um <= 0.0 {
        return Err(MarklabError::Config(
            "cross interaction curve bin width must be positive and finite".into(),
        ));
    }
    if !max_r_um.is_finite() || max_r_um <= 0.0 {
        return Err(MarklabError::Config(
            "cross interaction curve max distance must be positive and finite".into(),
        ));
    }
    if permutations == 0 {
        return Err(MarklabError::Config(
            "cross interaction curve permutations must be greater than zero".into(),
        ));
    }
    Ok(())
}

fn validate_registered_coordinates(cells: &[FusedCell]) -> Result<()> {
    for (index, cell) in cells.iter().enumerate() {
        if !cell.x_um_registered.is_finite() || !cell.y_um_registered.is_finite() {
            return Err(MarklabError::Schema(format!(
                "fused cell {index} ({}) has non-finite registered coordinates",
                cell.source_cell_id
            )));
        }
    }
    Ok(())
}

fn labels_match_pair(
    left: Option<&str>,
    right: Option<&str>,
    label_a: &str,
    label_b: &str,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) if label_a == label_b => left == label_a && right == label_b,
        (Some(left), Some(right)) => {
            (left == label_a && right == label_b) || (left == label_b && right == label_a)
        }
        _ => false,
    }
}

fn euclidean_distance_um(source: &FusedCell, target: &FusedCell) -> f64 {
    let dx = source.x_um_registered - target.x_um_registered;
    let dy = source.y_um_registered - target.y_um_registered;
    dx.hypot(dy)
}
