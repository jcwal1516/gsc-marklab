use std::{path::Path, time::Instant};

use crate::{
    data::{Pattern, PatternMeta},
    errors::{MarklabError, Result},
    geom::{mask::TumorMask, spatial_index::mean_nearest_neighbor_distance},
};

use super::{
    CategoricalStratumEncoder, DenseOptionalColumn, PatternBuildCounters, PatternLoadDiagnostics,
    PatternLoadResult, PatternRowQc,
};

mod decoder;
mod schema;

pub fn load_pattern_csv_with_diagnostics(
    path: impl AsRef<Path>,
    mask: &TumorMask,
) -> Result<PatternLoadResult> {
    let path_ref = path.as_ref();
    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut marks = Vec::new();
    let mut qc_bins = DenseOptionalColumn::default();
    let mut component_ids = DenseOptionalColumn::default();
    let mut mark_prob = DenseOptionalColumn::default();
    let mut tumor_probability = DenseOptionalColumn::default();
    let mut nucleus_area = DenseOptionalColumn::default();
    let mut local_dab = DenseOptionalColumn::default();
    let mut local_hema = DenseOptionalColumn::default();
    let mut meta: Option<PatternMeta> = None;
    let mut qc_counters = PatternBuildCounters::default();
    let mut internal_control_bin = CategoricalStratumEncoder::default();
    let mut block_id_strata = CategoricalStratumEncoder::default();
    let mut slide_region_strata = CategoricalStratumEncoder::default();
    let mut histologic_compartment_strata = CategoricalStratumEncoder::default();
    let mut stain_batch_strata = CategoricalStratumEncoder::default();

    let mask_filter_start = Instant::now();
    let mask_filter_span = tracing::info_span!("marklab_stage", stage_name = "mask_filter");
    let mask_filter_enter = mask_filter_span.enter();
    let decoded = decoder::read_rows(path_ref)?;
    for row in decoded.rows {
        if !mask.contains(row.x_um, row.y_um) {
            continue;
        }
        let artifact_excluded = row.artifact.unwrap_or(false)
            || row.edge_artifact.unwrap_or(false)
            || row.fold_artifact.unwrap_or(false);
        let nonviable_excluded =
            row.necrosis.unwrap_or(false) || row.nonviable_therapy_effect.unwrap_or(false);
        let row_qc = PatternRowQc {
            valid_tumor: row.valid_tumor,
            valid_ihc: row.valid_ihc,
            internal_control_valid: decoded
                .has_internal_control
                .then(|| internal_control_is_valid(&row.internal_control_local)),
            artifact_excluded: decoded.has_artifact_columns.then_some(artifact_excluded),
            nonviable_excluded: decoded.has_nonviable_columns.then_some(nonviable_excluded),
        };
        if !qc_counters.observe(row_qc) {
            continue;
        }

        if let Some(existing) = &meta {
            if existing.case_id != row.case_id
                || existing.timepoint != row.timepoint
                || existing.protein != row.protein
            {
                return Err(MarklabError::Schema(
                    "CSV input must contain one case_id/timepoint/protein group".into(),
                ));
            }
        } else {
            meta = Some(PatternMeta {
                case_id: row.case_id.clone(),
                timepoint: row.timepoint.clone(),
                protein: row.protein.clone(),
                slide_id: nonempty(row.slide_id.clone()),
                section_id: nonempty(row.section_id.clone()),
                stain_batch: nonempty(row.stain_batch.clone()),
                block_id: nonempty(row.block_id.clone()),
                region_id: nonempty(row.region_id.clone()),
            });
        }

        x.push(row.x_um);
        y.push(row.y_um);
        marks.push(row.mark);
        internal_control_bin.push_optional(row.internal_control_local.as_deref());
        block_id_strata.push_optional(row.block_id.as_deref());
        slide_region_strata.push_optional(row.slide_region.as_deref().or(row.region_id.as_deref()));
        histologic_compartment_strata.push_optional(row.histologic_compartment.as_deref());
        stain_batch_strata.push_optional(row.stain_batch.as_deref());
        qc_bins.push(row.qc_bin, "qc_bin")?;
        component_ids.push(row.component_id, "component_id")?;
        mark_prob.push(
            row.mark_probability
                .map(|value| super::checked_probability(value, "mark_probability"))
                .transpose()?,
            "mark_probability",
        )?;
        tumor_probability.push(
            row.tumor_probability
                .map(|value| super::checked_probability(value, "tumor_probability"))
                .transpose()?,
            "tumor_probability",
        )?;
        nucleus_area.push(
            row.nucleus_area_um2
                .map(|value| super::checked_positive(value, "nucleus_area_um2"))
                .transpose()?,
            "nucleus_area_um2",
        )?;
        local_dab.push(
            row.local_dab_od
                .map(|value| super::checked_finite(value, "local_dab_od"))
                .transpose()?,
            "local_dab_od",
        )?;
        local_hema.push(
            row.local_hematoxylin_od
                .map(|value| super::checked_finite(value, "local_hematoxylin_od"))
                .transpose()?,
            "local_hematoxylin_od",
        )?;
    }
    drop(mask_filter_enter);
    let mask_filter = mask_filter_start.elapsed();

    qc_counters.validate_denominator()?;
    let meta = meta.ok_or_else(|| {
        MarklabError::Validation("no valid tumor/IHC cells remained after mask filtering".into())
    })?;

    let mut pattern = Pattern::from_arrays(x, y, marks, meta)?;
    pattern.qc_bin = qc_bins.finish();
    pattern.component_id = component_ids.finish();
    insert_finished_stratum(
        &mut pattern.categorical_strata,
        "internal_control_bin",
        internal_control_bin,
    );
    insert_finished_stratum(&mut pattern.categorical_strata, "block_id", block_id_strata);
    insert_finished_stratum(
        &mut pattern.categorical_strata,
        "slide_region",
        slide_region_strata,
    );
    insert_finished_stratum(
        &mut pattern.categorical_strata,
        "histologic_compartment",
        histologic_compartment_strata,
    );
    insert_finished_stratum(
        &mut pattern.categorical_strata,
        "stain_batch",
        stain_batch_strata,
    );
    pattern.mark_prob = mark_prob.finish();
    pattern.tumor_probability = tumor_probability.finish();
    pattern.nucleus_area_um2 = nucleus_area.finish();
    pattern.local_dab_od = local_dab.finish();
    pattern.local_hematoxylin_od = local_hema.finish();
    pattern.window.area_um2 = mask.area_um2();
    pattern.window.l_eff_um = mask.effective_diameter_um();
    let nearest_neighbor_start = Instant::now();
    let nearest_neighbor_span =
        tracing::info_span!("marklab_stage", stage_name = "nearest_neighbor");
    let nearest_neighbor_enter = nearest_neighbor_span.enter();
    pattern.window.d_nn_mean_um = mean_nearest_neighbor_distance(&pattern.x_um, &pattern.y_um)
        .ok_or_else(|| {
            MarklabError::Validation(
                "at least two retained cells are required to estimate nearest-neighbor distance"
                    .into(),
            )
        })?;
    drop(nearest_neighbor_enter);
    let nearest_neighbor = nearest_neighbor_start.elapsed();
    qc_counters.apply_to(&mut pattern)?;

    Ok(PatternLoadResult {
        pattern,
        diagnostics: PatternLoadDiagnostics {
            mask_filter,
            nearest_neighbor,
        },
    })
}

fn insert_finished_stratum(
    strata: &mut std::collections::BTreeMap<String, Box<[u32]>>,
    name: &str,
    encoder: CategoricalStratumEncoder,
) {
    if let Some(values) = encoder.finish() {
        strata.insert(name.to_owned(), values);
    }
}

fn internal_control_is_valid(value: &Option<String>) -> bool {
    match value.as_deref().map(str::trim).map(str::to_ascii_lowercase) {
        None => false,
        Some(value) => value == "valid",
    }
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    })
}
