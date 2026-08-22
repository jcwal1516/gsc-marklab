use std::mem::size_of;

use crate::{
    common::{
        matrix::F64Matrix,
        seeds::{derive_seed, SeedEndpoint},
    },
    errors::{MarklabError, Result},
    geom::spatial_index::SpatialIndex2D,
    multimodal::{
        cells::FusedCell,
        labels::{PrimaryLabelEncoding, PrimaryLabelId},
    },
    output::{CrossInteractionCurve, CrossInteractionPoint},
    perf::counters::enforce_storage_budget,
    permutation::envelopes::GlobalEnvelope,
};

use super::{enrichment::LabelPair, label_permutation::LabelPermutationPlan};

#[cfg(test)]
thread_local! {
    static PLAN_BUILD_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_cross_interaction_plan_build_call_count() {
    PLAN_BUILD_CALLS.set(0);
}

#[cfg(test)]
pub(crate) fn cross_interaction_plan_build_call_count() -> usize {
    PLAN_BUILD_CALLS.get()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PairBin {
    source: usize,
    target: usize,
    bin: usize,
}

/// Fixed geometry for every cross-interaction label pair in one run.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CrossInteractionPlan {
    bin_width_um: f64,
    max_r_um: f64,
    pairs: Box<[PairBin]>,
    geometric_pair_counts: Box<[usize]>,
    estimated_build_storage_bytes: usize,
}

impl CrossInteractionPlan {
    pub(crate) fn new_with_index(
        cells: &[FusedCell],
        index: &SpatialIndex2D,
        bin_width_um: f64,
        max_r_um: f64,
        storage_budget_bytes: usize,
    ) -> Result<Self> {
        validate_geometry_config(bin_width_um, max_r_um)?;
        validate_registered_coordinates(cells)?;
        if index.len() != cells.len() {
            return Err(MarklabError::Geometry(format!(
                "spatial index has {} points for {} cross-interaction cells",
                index.len(),
                cells.len()
            )));
        }
        #[cfg(test)]
        PLAN_BUILD_CALLS.set(PLAN_BUILD_CALLS.get() + 1);

        let bin_count = (max_r_um / bin_width_um).ceil() as usize;
        let base_bytes = bin_count.saturating_mul(size_of::<usize>());
        enforce_storage_budget(
            "cross-interaction pair plan",
            base_bytes,
            storage_budget_bytes,
        )?;
        let mut pairs = Vec::new();
        let mut geometric_pair_counts = vec![0usize; bin_count];
        let mut budget_error = None;
        for source in 0..cells.len() {
            index.visit_within_radius(source, max_r_um, |neighbor| {
                if budget_error.is_some()
                    || neighbor.index <= source
                    || neighbor.distance_um >= max_r_um
                {
                    return;
                }
                let bin = (neighbor.distance_um / bin_width_um).floor() as usize;
                if bin >= bin_count {
                    return;
                }
                let required = base_bytes.saturating_add(
                    pairs
                        .len()
                        .saturating_add(1)
                        .saturating_mul(size_of::<PairBin>())
                        .saturating_mul(4),
                );
                if let Err(error) = enforce_storage_budget(
                    "cross-interaction pair plan",
                    required,
                    storage_budget_bytes,
                ) {
                    budget_error = Some(error);
                    return;
                }
                pairs.push(PairBin {
                    source,
                    target: neighbor.index,
                    bin,
                });
                geometric_pair_counts[bin] += 1;
            })?;
            if let Some(error) = budget_error.take() {
                return Err(error);
            }
        }
        pairs.sort_unstable_by_key(|pair| (pair.source, pair.target));
        let estimated_build_storage_bytes = base_bytes.saturating_add(
            pairs
                .len()
                .saturating_mul(size_of::<PairBin>())
                .saturating_mul(4),
        );
        Ok(Self {
            bin_width_um,
            max_r_um,
            pairs: pairs.into_boxed_slice(),
            geometric_pair_counts: geometric_pair_counts.into_boxed_slice(),
            estimated_build_storage_bytes,
        })
    }

    pub(crate) fn bin_count(&self) -> usize {
        self.geometric_pair_counts.len()
    }

    pub(crate) fn estimated_storage_bytes(&self) -> usize {
        self.pairs
            .len()
            .saturating_mul(size_of::<PairBin>())
            .saturating_add(
                self.geometric_pair_counts
                    .len()
                    .saturating_mul(size_of::<usize>()),
            )
    }

    pub(crate) fn estimated_build_storage_bytes(&self) -> usize {
        self.estimated_build_storage_bytes
    }

    fn evaluate_into(
        &self,
        labels: &[Option<PrimaryLabelId>],
        pair: EncodedLabelPair,
        counts: &mut Vec<usize>,
    ) -> Result<()> {
        if self
            .pairs
            .iter()
            .any(|entry| entry.source >= labels.len() || entry.target >= labels.len())
        {
            return Err(MarklabError::Validation(
                "cross-interaction labels do not cover every planned cell".into(),
            ));
        }
        counts.clear();
        counts.resize(self.bin_count(), 0);
        for entry in &self.pairs {
            if pair.matches(labels[entry.source], labels[entry.target]) {
                counts[entry.bin] += 1;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct EncodedLabelPair {
    a: Option<PrimaryLabelId>,
    b: Option<PrimaryLabelId>,
}

impl EncodedLabelPair {
    fn new(labels: &PrimaryLabelEncoding, pair: &LabelPair) -> Self {
        Self {
            a: labels.id_for(&pair.label_a),
            b: labels.id_for(&pair.label_b),
        }
    }

    fn matches(self, left: Option<PrimaryLabelId>, right: Option<PrimaryLabelId>) -> bool {
        match (left, right) {
            (Some(left), Some(right)) if self.a == self.b => {
                self.a == Some(left) && self.b == Some(right)
            }
            (Some(left), Some(right)) => {
                (self.a == Some(left) && self.b == Some(right))
                    || (self.b == Some(left) && self.a == Some(right))
            }
            _ => false,
        }
    }
}

#[derive(Debug)]
pub(crate) struct CrossInteractionAnalysis {
    pub(crate) curves: Vec<CrossInteractionCurve>,
    pub(crate) estimated_peak_storage_bytes: usize,
}

/// Build every configured cross-interaction curve over one indexed pair plan.
#[allow(clippy::too_many_arguments)]
pub(crate) fn cross_interaction_curves_with_index(
    cells: &[FusedCell],
    index: &SpatialIndex2D,
    labels: &PrimaryLabelEncoding,
    label_pairs: &[LabelPair],
    bin_width_um: f64,
    max_r_um: f64,
    permutations: usize,
    seed: u64,
    alpha: f64,
    storage_budget_bytes: usize,
) -> Result<CrossInteractionAnalysis> {
    validate_curve_config(label_pairs, bin_width_um, max_r_um, permutations, alpha)?;
    if labels.len() != cells.len() {
        return Err(MarklabError::Validation(format!(
            "primary label count {} does not match cross-interaction cell count {}",
            labels.len(),
            cells.len()
        )));
    }

    let bin_count = (max_r_um / bin_width_um).ceil() as usize;
    let sections = cells
        .iter()
        .map(|cell| cell.source_section)
        .collect::<Vec<_>>();
    let permutation_plan = LabelPermutationPlan::for_source_sections(&sections);
    let scratch_bytes = cross_interaction_scratch_bytes(
        cells.len(),
        permutation_plan.maximum_group_size(),
        bin_count,
        permutations,
        label_pairs,
    );
    enforce_storage_budget(
        "cross-interaction scratch and result",
        scratch_bytes,
        storage_budget_bytes,
    )?;
    let plan = CrossInteractionPlan::new_with_index(
        cells,
        index,
        bin_width_um,
        max_r_um,
        storage_budget_bytes.saturating_sub(scratch_bytes),
    )?;

    let mut shuffled = Vec::with_capacity(labels.len());
    let mut group_scratch = Vec::with_capacity(permutation_plan.maximum_group_size());
    let mut observed_counts = Vec::with_capacity(bin_count);
    let mut null_counts = Vec::with_capacity(bin_count);
    let mut curves = Vec::with_capacity(label_pairs.len());
    for pair in label_pairs {
        let encoded_pair = EncodedLabelPair::new(labels, pair);
        plan.evaluate_into(labels.ids(), encoded_pair, &mut observed_counts)?;
        let mut permutation_counts =
            F64Matrix::zeros(permutations, bin_count).ok_or_else(|| {
                MarklabError::Compute("invalid cross-interaction matrix dimensions".into())
            })?;
        for permutation in 0..permutations {
            permutation_plan.permute_into(
                labels.ids(),
                derive_seed(seed, SeedEndpoint::CrossInteraction, permutation),
                &mut shuffled,
                &mut group_scratch,
            )?;
            plan.evaluate_into(&shuffled, encoded_pair, &mut null_counts)?;
            let row = permutation_counts
                .row_mut(permutation)
                .expect("validated cross-interaction row");
            for (output, count) in row.iter_mut().zip(null_counts.iter().copied()) {
                *output = count as f64;
            }
        }
        curves.push(assemble_curve(
            pair,
            &plan,
            &observed_counts,
            &permutation_counts,
            alpha,
        )?);
    }

    Ok(CrossInteractionAnalysis {
        curves,
        estimated_peak_storage_bytes: plan
            .estimated_build_storage_bytes()
            .max(plan.estimated_storage_bytes())
            .saturating_add(scratch_bytes),
    })
}

/// Build one curve for focused callers and differential tests.
#[cfg(test)]
pub fn cross_interaction_curve(
    cells: &[FusedCell],
    label_a: &str,
    label_b: &str,
    bin_width_um: f64,
    max_r_um: f64,
    permutations: usize,
    seed: u64,
) -> Result<CrossInteractionCurve> {
    let labels = PrimaryLabelEncoding::new(cells)?;
    let index = SpatialIndex2D::from_points(
        cells
            .iter()
            .map(|cell| [cell.x_um_registered, cell.y_um_registered]),
    )?;
    let pair = LabelPair {
        label_a: label_a.to_owned(),
        label_b: label_b.to_owned(),
    };
    cross_interaction_curves_with_index(
        cells,
        &index,
        &labels,
        &[pair],
        bin_width_um,
        max_r_um,
        permutations,
        seed,
        0.05,
        usize::MAX,
    )?
    .curves
    .pop()
    .ok_or_else(|| MarklabError::Compute("missing cross-interaction result".into()))
}

fn assemble_curve(
    pair: &LabelPair,
    plan: &CrossInteractionPlan,
    observed_counts: &[usize],
    permutation_counts: &F64Matrix,
    alpha: f64,
) -> Result<CrossInteractionCurve> {
    let eligibility = plan
        .geometric_pair_counts
        .iter()
        .map(|count| *count > 0)
        .collect::<Vec<_>>();
    let observed = observed_counts
        .iter()
        .map(|count| *count as f64)
        .collect::<Vec<_>>();
    let envelope = eligibility
        .iter()
        .any(|eligible| *eligible)
        .then(|| {
            GlobalEnvelope::from_matrix_with_eligibility(
                &observed,
                permutation_counts,
                alpha,
                &eligibility,
            )
        })
        .transpose()?;
    let points = observed_counts
        .iter()
        .copied()
        .enumerate()
        .map(|(bin, count)| {
            let r_min_um = bin as f64 * plan.bin_width_um;
            let inference_eligible = eligibility[bin];
            CrossInteractionPoint {
                r_min_um,
                r_max_um: (r_min_um + plan.bin_width_um).min(plan.max_r_um),
                value: inference_eligible.then_some(count as f64),
                inference_eligible,
                lower_global_envelope: envelope
                    .as_ref()
                    .filter(|_| inference_eligible)
                    .map(|value| value.lower[bin]),
                upper_global_envelope: envelope
                    .as_ref()
                    .filter(|_| inference_eligible)
                    .map(|value| value.upper[bin]),
                count,
            }
        })
        .collect();

    Ok(CrossInteractionCurve {
        label_a: pair.label_a.clone(),
        label_b: pair.label_b.clone(),
        points,
        p_global: envelope.map(|value| value.p_global),
    })
}

fn cross_interaction_scratch_bytes(
    cell_count: usize,
    maximum_group_size: usize,
    bin_count: usize,
    permutations: usize,
    label_pairs: &[LabelPair],
) -> usize {
    let label_bytes = cell_count
        .saturating_add(maximum_group_size)
        .saturating_mul(size_of::<Option<PrimaryLabelId>>());
    let permutation_plan_bytes = cell_count
        .saturating_mul(size_of::<usize>())
        .saturating_add(
            cell_count.saturating_mul(size_of::<crate::multimodal::cells::CellSection>()),
        )
        .saturating_add(2usize.saturating_mul(size_of::<Vec<usize>>()));
    let count_bytes = bin_count
        .saturating_mul(size_of::<usize>())
        .saturating_mul(2);
    // ERL temporarily owns the original permutation matrix, an observed-plus-
    // permutation eligible matrix, and its rank matrix at the same time.
    let erl_curve_count = permutations.saturating_add(1);
    let matrix_bytes = permutations
        .saturating_add(erl_curve_count.saturating_mul(2))
        .saturating_mul(bin_count)
        .saturating_mul(size_of::<f64>());
    let erl_vector_bytes = erl_curve_count
        .saturating_mul(size_of::<f64>().saturating_add(size_of::<usize>()))
        .saturating_mul(4)
        .saturating_add(bin_count.saturating_mul(size_of::<f64>()).saturating_mul(2));
    let result_bytes = label_pairs
        .len()
        .saturating_mul(bin_count)
        .saturating_mul(size_of::<CrossInteractionPoint>())
        .saturating_add(
            label_pairs
                .len()
                .saturating_mul(size_of::<CrossInteractionCurve>()),
        )
        .saturating_add(
            label_pairs
                .iter()
                .map(|pair| pair.label_a.len().saturating_add(pair.label_b.len()))
                .sum::<usize>(),
        );
    label_bytes
        .saturating_add(permutation_plan_bytes)
        .saturating_add(count_bytes)
        .saturating_add(matrix_bytes)
        .saturating_add(erl_vector_bytes)
        .saturating_add(result_bytes)
}

fn validate_curve_config(
    label_pairs: &[LabelPair],
    bin_width_um: f64,
    max_r_um: f64,
    permutations: usize,
    alpha: f64,
) -> Result<()> {
    for (index, pair) in label_pairs.iter().enumerate() {
        if pair.label_a.trim().is_empty() || pair.label_b.trim().is_empty() {
            return Err(MarklabError::Config(format!(
                "cross interaction curve labels must be non-empty for pair {index}"
            )));
        }
    }
    validate_geometry_config(bin_width_um, max_r_um)?;
    if permutations == 0 {
        return Err(MarklabError::Config(
            "cross interaction curve permutations must be greater than zero".into(),
        ));
    }
    if !alpha.is_finite() || !(0.0..1.0).contains(&alpha) {
        return Err(MarklabError::Config(
            "cross interaction global-envelope alpha must be finite and strictly between zero and one".into(),
        ));
    }
    if (permutations + 1) as f64 * alpha < 1.0 {
        return Err(MarklabError::Config(
            "cross interaction global envelope requires (B + 1) * alpha >= 1".into(),
        ));
    }
    Ok(())
}

fn validate_geometry_config(bin_width_um: f64, max_r_um: f64) -> Result<()> {
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
