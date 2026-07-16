use std::{fs::File, path::Path, time::Instant};

use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use crate::{
    data::{Pattern, PatternMeta},
    errors::{MmrspaceError, Result},
    geom::{mask::TumorMask, spatial_index::mean_nearest_neighbor_distance},
};

use super::super::{
    CategoricalStratumEncoder, DenseOptionalColumn, PatternLoadDiagnostics, PatternLoadResult,
};

pub fn load_pattern_parquet_with_diagnostics(
    path: impl AsRef<Path>,
    mask: &TumorMask,
) -> Result<PatternLoadResult> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|source| MmrspaceError::io(path, source))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|err| MmrspaceError::Schema(err.to_string()))?;
    let reader = builder
        .build()
        .map_err(|err| MmrspaceError::Schema(err.to_string()))?;

    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut marks = Vec::new();
    let mut mark_prob = DenseOptionalColumn::default();
    let mut tumor_probability = DenseOptionalColumn::default();
    let mut nucleus_area = DenseOptionalColumn::default();
    let mut qc_bins = DenseOptionalColumn::default();
    let mut component_ids = DenseOptionalColumn::default();
    let mut local_dab = DenseOptionalColumn::default();
    let mut local_hema = DenseOptionalColumn::default();
    let mut meta: Option<PatternMeta> = None;
    let mut total_rows_in_mask = 0_usize;
    let mut retained_rows = 0_usize;
    let mut saw_internal_control = false;
    let mut saw_artifact_exclusion = false;
    let mut artifact_excluded_rows = 0_usize;
    let mut saw_nonviable_exclusion = false;
    let mut nonviable_excluded_rows = 0_usize;
    let mut internal_control_bin = CategoricalStratumEncoder::default();
    let mut block_id_strata = CategoricalStratumEncoder::default();
    let mut slide_region_strata = CategoricalStratumEncoder::default();
    let mut histologic_compartment_strata = CategoricalStratumEncoder::default();
    let mut stain_batch_strata = CategoricalStratumEncoder::default();

    let mask_filter_start = Instant::now();
    let mask_filter_span = tracing::info_span!("mmrspace_stage", stage_name = "mask_filter");
    let mask_filter_enter = mask_filter_span.enter();
    for batch in reader {
        let batch = batch.map_err(|err| MmrspaceError::Schema(err.to_string()))?;
        let columns = super::schema::BatchColumns::try_new(&batch)?;
        saw_internal_control |= columns.internal_control.is_some();
        saw_artifact_exclusion |= columns.has_artifact_columns();
        saw_nonviable_exclusion |= columns.has_nonviable_columns();

        for row_index in 0..batch.num_rows() {
            let row = super::row::DecodedRow::decode(&columns, row_index)?;
            if !mask.contains(row.x_um, row.y_um) {
                continue;
            }
            total_rows_in_mask += 1;
            if row.artifact_excluded {
                artifact_excluded_rows += 1;
            }
            if row.nonviable_excluded {
                nonviable_excluded_rows += 1;
            }
            if !row.valid_tumor
                || !row.valid_ihc
                || !row.internal_control_is_valid()
                || row.artifact_excluded
                || row.nonviable_excluded
            {
                continue;
            }
            retained_rows += 1;

            if let Some(existing) = &meta {
                if existing.case_id != row.case_id
                    || existing.timepoint != row.timepoint
                    || existing.protein != row.protein
                {
                    return Err(MmrspaceError::Schema(
                        "Parquet input must contain one case_id/timepoint/protein group".into(),
                    ));
                }
            } else {
                meta = Some(PatternMeta {
                    case_id: row.case_id.clone(),
                    timepoint: row.timepoint.clone(),
                    protein: row.protein.clone(),
                    slide_id: row.slide_id.clone(),
                    section_id: row.section_id.clone(),
                    stain_batch: row.stain_batch.clone(),
                    block_id: row.block_id.clone(),
                    region_id: row.region_id.clone(),
                });
            }

            x.push(row.x_um);
            y.push(row.y_um);
            marks.push(row.mark);
            internal_control_bin.push_optional(row.internal_control.as_deref());
            block_id_strata.push_optional(row.block_id.as_deref());
            slide_region_strata
                .push_optional(row.slide_region.as_deref().or(row.region_id.as_deref()));
            histologic_compartment_strata.push_optional(row.histologic_compartment.as_deref());
            stain_batch_strata.push_optional(row.stain_batch.as_deref());
            mark_prob.push(row.mark_probability, "mark_probability")?;
            tumor_probability.push(row.tumor_probability, "tumor_probability")?;
            nucleus_area.push(row.nucleus_area_um2, "nucleus_area_um2")?;
            qc_bins.push(row.qc_bin, "qc_bin")?;
            component_ids.push(row.component_id, "component_id")?;
            local_dab.push(row.local_dab_od, "local_dab_od")?;
            local_hema.push(row.local_hematoxylin_od, "local_hematoxylin_od")?;
        }
    }
    drop(mask_filter_enter);
    let mask_filter = mask_filter_start.elapsed();

    let meta = meta.ok_or_else(|| {
        MmrspaceError::Validation("no valid tumor/IHC cells remained after mask filtering".into())
    })?;
    let mut pattern = Pattern::from_arrays(x, y, marks, meta)?;
    pattern.mark_prob = mark_prob.finish();
    pattern.tumor_probability = tumor_probability.finish();
    pattern.nucleus_area_um2 = nucleus_area.finish();
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
    pattern.local_dab_od = local_dab.finish();
    pattern.local_hematoxylin_od = local_hema.finish();
    pattern.window.area_um2 = mask.area_um2();
    pattern.window.l_eff_um = mask.effective_diameter_um();
    let nearest_neighbor_start = Instant::now();
    let nearest_neighbor_span =
        tracing::info_span!("mmrspace_stage", stage_name = "nearest_neighbor");
    let nearest_neighbor_enter = nearest_neighbor_span.enter();
    pattern.window.d_nn_mean_um = mean_nearest_neighbor_distance(&pattern.x_um, &pattern.y_um)
        .ok_or_else(|| {
            MmrspaceError::Validation(
                "at least two retained cells are required to estimate nearest-neighbor distance"
                    .into(),
            )
        })?;
    drop(nearest_neighbor_enter);
    let nearest_neighbor = nearest_neighbor_start.elapsed();
    pattern.window.valid_mask_fraction =
        crate::qc::ihc_validity::validity_fraction(retained_rows, total_rows_in_mask);
    if saw_internal_control {
        pattern.internal_control_valid_fraction = Some(pattern.window.valid_mask_fraction);
    }
    if saw_artifact_exclusion {
        pattern.artifact_excluded_fraction = Some(crate::qc::ihc_validity::validity_fraction(
            artifact_excluded_rows,
            total_rows_in_mask,
        ));
    }
    if saw_nonviable_exclusion {
        pattern.nonviable_excluded_fraction = Some(crate::qc::ihc_validity::validity_fraction(
            nonviable_excluded_rows,
            total_rows_in_mask,
        ));
    }

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
