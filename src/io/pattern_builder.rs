use std::{collections::BTreeMap, time::Instant};

use crate::{
    data::{Pattern, PatternMeta},
    errors::{MarklabError, Result},
    geom::{mask::TumorMask, spatial_index::mean_nearest_neighbor_distance},
};

use super::{
    checked_finite, checked_positive, checked_probability, row::DecodedCellRow,
    PatternLoadDiagnostics, PatternLoadResult,
};

#[derive(Debug, Default)]
struct CategoricalStratumEncoder {
    values: Vec<u32>,
    ids: BTreeMap<String, u32>,
    saw_nonmissing: bool,
}

impl CategoricalStratumEncoder {
    fn push_optional(&mut self, value: Option<&str>) {
        let normalized = value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("__missing__");
        self.saw_nonmissing |= normalized != "__missing__";
        let next_id = self.ids.len() as u32;
        let id = *self.ids.entry(normalized.to_owned()).or_insert(next_id);
        self.values.push(id);
    }

    fn finish(self) -> Option<Box<[u32]>> {
        self.saw_nonmissing.then(|| self.values.into_boxed_slice())
    }
}

#[derive(Debug, Default)]
struct DenseOptionalColumn<T> {
    values: Vec<T>,
    presence: Option<bool>,
}

impl<T> DenseOptionalColumn<T> {
    fn push(&mut self, value: Option<T>, column: &str) -> Result<()> {
        let present = value.is_some();
        if self.presence.is_some_and(|expected| expected != present) {
            return Err(MarklabError::Schema(format!(
                "{column} must be populated for every retained row or none"
            )));
        }
        self.presence.get_or_insert(present);
        if let Some(value) = value {
            self.values.push(value);
        }
        Ok(())
    }

    fn finish(self) -> Option<Box<[T]>> {
        (self.presence == Some(true)).then(|| self.values.into_boxed_slice())
    }
}

#[derive(Clone, Copy, Debug)]
struct PatternRowQc {
    valid_tumor: bool,
    valid_ihc: bool,
    internal_control_valid: Option<bool>,
    artifact_excluded: Option<bool>,
    nonviable_excluded: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PatternBuildCounters {
    in_mask: usize,
    valid_tumor: usize,
    valid_ihc: usize,
    valid_internal_control: usize,
    artifact_excluded: usize,
    nonviable_excluded: usize,
    retained: usize,
    saw_internal_control: bool,
    saw_artifact: bool,
    saw_nonviable: bool,
}

impl PatternBuildCounters {
    fn observe(&mut self, row: PatternRowQc) -> bool {
        self.in_mask += 1;
        self.valid_tumor += usize::from(row.valid_tumor);
        self.valid_ihc += usize::from(row.valid_ihc);
        if let Some(valid) = row.internal_control_valid {
            self.saw_internal_control = true;
            self.valid_internal_control += usize::from(valid);
        }
        if let Some(excluded) = row.artifact_excluded {
            self.saw_artifact = true;
            self.artifact_excluded += usize::from(excluded);
        }
        if let Some(excluded) = row.nonviable_excluded {
            self.saw_nonviable = true;
            self.nonviable_excluded += usize::from(excluded);
        }

        let retained = row.valid_tumor
            && row.valid_ihc
            && row.internal_control_valid.unwrap_or(true)
            && !row.artifact_excluded.unwrap_or(false)
            && !row.nonviable_excluded.unwrap_or(false);
        self.retained += usize::from(retained);
        retained
    }

    fn validate_denominator(&self) -> Result<()> {
        if self.in_mask == 0 {
            return Err(MarklabError::Validation(
                "no cells fell inside the tumor mask; QC fractions are undefined".into(),
            ));
        }
        Ok(())
    }

    fn apply_to(self, pattern: &mut Pattern) -> Result<()> {
        self.validate_denominator()?;

        let fraction = |count| Some(count as f64 / self.in_mask as f64);
        pattern.valid_tumor_fraction = fraction(self.valid_tumor);
        pattern.valid_ihc_fraction = fraction(self.valid_ihc);
        pattern.internal_control_valid_fraction = self
            .saw_internal_control
            .then(|| self.valid_internal_control as f64 / self.in_mask as f64);
        pattern.artifact_excluded_fraction = self
            .saw_artifact
            .then(|| self.artifact_excluded as f64 / self.in_mask as f64);
        pattern.nonviable_excluded_fraction = self
            .saw_nonviable
            .then(|| self.nonviable_excluded as f64 / self.in_mask as f64);
        pattern.window.valid_mask_fraction = self.retained as f64 / self.in_mask as f64;
        Ok(())
    }
}

pub(crate) struct PatternBuilder<'a> {
    mask: &'a TumorMask,
    source_name: &'static str,
    decode_and_filter_start: Instant,
    x: Vec<f64>,
    y: Vec<f64>,
    marks: Vec<u8>,
    qc_bins: DenseOptionalColumn<u16>,
    component_ids: DenseOptionalColumn<u32>,
    mark_prob: DenseOptionalColumn<f32>,
    tumor_probability: DenseOptionalColumn<f32>,
    nucleus_area: DenseOptionalColumn<f32>,
    local_dab: DenseOptionalColumn<f32>,
    local_hema: DenseOptionalColumn<f32>,
    meta: Option<PatternMeta>,
    qc_counters: PatternBuildCounters,
    internal_control_bin: CategoricalStratumEncoder,
    block_id_strata: CategoricalStratumEncoder,
    slide_region_strata: CategoricalStratumEncoder,
    histologic_compartment_strata: CategoricalStratumEncoder,
    stain_batch_strata: CategoricalStratumEncoder,
}

impl<'a> PatternBuilder<'a> {
    pub(crate) fn new(mask: &'a TumorMask, source_name: &'static str) -> Self {
        Self {
            mask,
            source_name,
            decode_and_filter_start: Instant::now(),
            x: Vec::new(),
            y: Vec::new(),
            marks: Vec::new(),
            qc_bins: DenseOptionalColumn::default(),
            component_ids: DenseOptionalColumn::default(),
            mark_prob: DenseOptionalColumn::default(),
            tumor_probability: DenseOptionalColumn::default(),
            nucleus_area: DenseOptionalColumn::default(),
            local_dab: DenseOptionalColumn::default(),
            local_hema: DenseOptionalColumn::default(),
            meta: None,
            qc_counters: PatternBuildCounters::default(),
            internal_control_bin: CategoricalStratumEncoder::default(),
            block_id_strata: CategoricalStratumEncoder::default(),
            slide_region_strata: CategoricalStratumEncoder::default(),
            histologic_compartment_strata: CategoricalStratumEncoder::default(),
            stain_batch_strata: CategoricalStratumEncoder::default(),
        }
    }

    pub(crate) fn push(&mut self, mut row: DecodedCellRow, row_number: usize) -> Result<()> {
        if !row.x_um.is_finite() || !row.y_um.is_finite() {
            return Err(MarklabError::Schema(format!(
                "{} row {row_number} has non-finite coordinates",
                self.source_name
            )));
        }
        if row.mark > 1 {
            return Err(MarklabError::Schema(format!(
                "{} row {row_number} mark must be 0 or 1",
                self.source_name
            )));
        }
        if !self.mask.contains(row.x_um, row.y_um) {
            return Ok(());
        }

        let row_qc = PatternRowQc {
            valid_tumor: row.valid_tumor,
            valid_ihc: row.valid_ihc,
            internal_control_valid: row.internal_control.as_ref().map(|state| state.is_valid()),
            artifact_excluded: row
                .artifact_flags
                .is_available()
                .then(|| row.artifact_flags.is_excluded()),
            nonviable_excluded: row
                .nonviable_flags
                .is_available()
                .then(|| row.nonviable_flags.is_excluded()),
        };
        if !self.qc_counters.observe(row_qc) {
            return Ok(());
        }

        row.slide_id = nonempty(row.slide_id);
        row.section_id = nonempty(row.section_id);
        row.stain_batch = nonempty(row.stain_batch);
        row.block_id = nonempty(row.block_id);
        row.region_id = nonempty(row.region_id);
        row.slide_region = nonempty(row.slide_region);
        row.histologic_compartment = nonempty(row.histologic_compartment);

        if let Some(existing) = &self.meta {
            if existing.case_id != row.case_id
                || existing.timepoint != row.timepoint
                || existing.protein != row.protein
            {
                return Err(MarklabError::Schema(format!(
                    "{} input must contain one case_id/timepoint/protein group",
                    self.source_name
                )));
            }
        } else {
            self.meta = Some(PatternMeta {
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

        self.x.push(row.x_um);
        self.y.push(row.y_um);
        self.marks.push(row.mark);
        self.internal_control_bin.push_optional(
            row.internal_control
                .as_ref()
                .and_then(|state| state.label()),
        );
        self.block_id_strata.push_optional(row.block_id.as_deref());
        self.slide_region_strata
            .push_optional(row.slide_region.as_deref().or(row.region_id.as_deref()));
        self.histologic_compartment_strata
            .push_optional(row.histologic_compartment.as_deref());
        self.stain_batch_strata
            .push_optional(row.stain_batch.as_deref());
        self.qc_bins.push(row.qc_bin, "qc_bin")?;
        self.component_ids.push(row.component_id, "component_id")?;
        self.mark_prob.push(
            row.mark_probability
                .map(|value| checked_probability(value, "mark_probability"))
                .transpose()?,
            "mark_probability",
        )?;
        self.tumor_probability.push(
            row.tumor_probability
                .map(|value| checked_probability(value, "tumor_probability"))
                .transpose()?,
            "tumor_probability",
        )?;
        self.nucleus_area.push(
            row.nucleus_area_um2
                .map(|value| checked_positive(value, "nucleus_area_um2"))
                .transpose()?,
            "nucleus_area_um2",
        )?;
        self.local_dab.push(
            row.local_dab_od
                .map(|value| checked_finite(value, "local_dab_od"))
                .transpose()?,
            "local_dab_od",
        )?;
        self.local_hema.push(
            row.local_hematoxylin_od
                .map(|value| checked_finite(value, "local_hematoxylin_od"))
                .transpose()?,
            "local_hematoxylin_od",
        )?;
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<PatternLoadResult> {
        let decode_and_filter = self.decode_and_filter_start.elapsed();
        self.qc_counters.validate_denominator()?;
        let meta = self.meta.ok_or_else(|| {
            MarklabError::Validation(
                "no valid tumor/IHC cells remained after mask filtering".into(),
            )
        })?;

        let mut pattern = Pattern::from_arrays(self.x, self.y, self.marks, meta)?;
        pattern.qc_bin = self.qc_bins.finish();
        pattern.component_id = self.component_ids.finish();
        insert_finished_stratum(
            &mut pattern.categorical_strata,
            "internal_control_bin",
            self.internal_control_bin,
        );
        insert_finished_stratum(
            &mut pattern.categorical_strata,
            "block_id",
            self.block_id_strata,
        );
        insert_finished_stratum(
            &mut pattern.categorical_strata,
            "slide_region",
            self.slide_region_strata,
        );
        insert_finished_stratum(
            &mut pattern.categorical_strata,
            "histologic_compartment",
            self.histologic_compartment_strata,
        );
        insert_finished_stratum(
            &mut pattern.categorical_strata,
            "stain_batch",
            self.stain_batch_strata,
        );
        pattern.mark_prob = self.mark_prob.finish();
        pattern.tumor_probability = self.tumor_probability.finish();
        pattern.nucleus_area_um2 = self.nucleus_area.finish();
        pattern.local_dab_od = self.local_dab.finish();
        pattern.local_hematoxylin_od = self.local_hema.finish();
        pattern.window.area_um2 = self.mask.area_um2();
        pattern.window.l_eff_um = self.mask.effective_diameter_um();

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
        self.qc_counters.apply_to(&mut pattern)?;

        Ok(PatternLoadResult {
            pattern,
            diagnostics: PatternLoadDiagnostics {
                decode_and_filter,
                nearest_neighbor,
            },
        })
    }
}

fn insert_finished_stratum(
    strata: &mut BTreeMap<String, Box<[u32]>>,
    name: &str,
    encoder: CategoricalStratumEncoder,
) {
    if let Some(values) = encoder.finish() {
        strata.insert(name.to_owned(), values);
    }
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    })
}
