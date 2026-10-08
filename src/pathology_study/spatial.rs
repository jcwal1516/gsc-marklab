use super::common::*;
use crate::{
    inhomogeneous_spatial as native, BinaryCompartmentPartition2D, CompartmentPartitionLimits,
    InhomogeneousCategoricalCrossPairCorrelationConfig,
    InhomogeneousCategoricalCrossPairCorrelationError, InhomogeneousPairCorrelationConfig,
    InhomogeneousSpatialConfig, InhomogeneousSpatialError, InhomogeneousSpatialLimits,
    ObservationWindow2D, Pattern, PatternMeta, PiecewiseCompartmentPairCorrelationConfig,
    PiecewiseCompartmentSpatialConfig, PiecewiseCompartmentSpatialLimits, Result,
};
use marklab_cohort::{
    functional_two_sample_permutation, FunctionalCurve, FunctionalPermutationSpec,
    FunctionalTestStatistic,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialDesign {
    pub group_a: String,
    pub group_b: String,
    pub permutations: usize,
    pub seed: u64,
    pub alpha: f64,
    pub statistic: Statistic,
    pub phenotype: Option<String>,
    pub source: Option<String>,
    pub target: Option<String>,
    pub radii_um: Vec<f64>,
    pub pair_bandwidth_um: f64,
    pub intensity: Intensity,
    pub compartments: Vec<Compartments>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Statistic {
    LMinusR,
    GMinusOne,
    CrossGMinusOne,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Intensity {
    pub model: IntensityModel,
    pub bandwidth_um: f64,
    pub grid: [usize; 2],
    pub minimum_intensity_per_um2: f64,
    pub cross_fit_folds: Option<usize>,
    pub bandwidth_candidates_um: Vec<f64>,
    pub null_mode: NullMode,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IntensityModel {
    Gaussian,
    BinaryCompartments,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NullMode {
    Plugin,
    Refit,
    ConditionalCompartments,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Compartments {
    pub slide_id: String,
    pub negative_id: String,
    pub positive_id: String,
    pub negative: Value,
    pub positive: Value,
    pub provenance: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SpatialSlide {
    pub slide_id: String,
    pub patient_id: String,
    pub group: String,
    pub coordinate_frame_id: String,
    pub area_mm2: f64,
    pub window: Value,
    pub mark_id: String,
    pub values: Vec<Option<f64>>,
    pub eligible: Vec<bool>,
    pub p_global: Option<f64>,
    pub slide_family_p: Option<f64>,
    pub null_model: Option<String>,
    pub intensity: Value,
    pub support: Value,
    pub unavailable: Option<String>,
    pub charged_work: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct SpatialPatient {
    pub patient_id: String,
    pub group: String,
    pub area_mm2: f64,
    pub values: Option<Vec<f64>>,
    pub unavailable: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct SpatialInference {
    pub p_value: Option<f64>,
    pub statistic: Option<f64>,
    pub difference: Option<Vec<f64>>,
    pub unavailable: Option<String>,
}

/// Slide curves and area-weighted patient functional inference with explicit null fitting.
#[derive(Clone, Debug, Serialize)]
pub struct PathologySpatialStudyResult {
    pub format: &'static str,
    pub version: u32,
    pub study_id: String,
    pub phenotypes: Phenotypes,
    pub design: SpatialDesign,
    pub limits: Limits,
    pub slides: Vec<SpatialSlide>,
    pub patients: Vec<SpatialPatient>,
    pub inference: SpatialInference,
    pub work_units: usize,
    pub estimated_storage_bytes: usize,
    pub claim_scope: &'static str,
}

enum Failure {
    Unavailable(String),
    Fatal(String),
}
impl From<InhomogeneousSpatialError> for Failure {
    fn from(error: InhomogeneousSpatialError) -> Self {
        use InhomogeneousSpatialError::*;
        match error {
            InsufficientPoints
            | NearZeroIntensity { .. }
            | PointOnCompartmentInterface { .. }
            | SparseCompartment { .. } => Self::Unavailable(error.to_string()),
            _ => Self::Fatal(error.to_string()),
        }
    }
}
impl From<InhomogeneousCategoricalCrossPairCorrelationError> for Failure {
    fn from(error: InhomogeneousCategoricalCrossPairCorrelationError) -> Self {
        match error {
            InhomogeneousCategoricalCrossPairCorrelationError::Inhomogeneous(e) => e.into(),
            InhomogeneousCategoricalCrossPairCorrelationError::SparseLevel { .. } => {
                Self::Unavailable(error.to_string())
            }
            _ => Self::Fatal(error.to_string()),
        }
    }
}
fn serialized(value: impl Serialize) -> std::result::Result<Value, Failure> {
    serde_json::to_value(value).map_err(|e| Failure::Fatal(e.to_string()))
}

/// Analyze a prespecified curve per slide and compare complete area-weighted patient curves.
pub fn analyze_pathology_spatial_study(bytes: &[u8]) -> Result<PathologySpatialStudyResult> {
    let (recipe, windows, mut budget) =
        parse::<SpatialDesign>(bytes, "marklab.pathology_spatial_study_recipe")?;
    validate(&recipe)?;
    let d = &recipe.design;
    let patient_count = recipe
        .study
        .slides
        .iter()
        .map(|s| &s.patient_id)
        .collect::<BTreeSet<_>>()
        .len();
    budget.work(
        patient_count
            .saturating_mul(d.radii_um.len())
            .saturating_mul(d.permutations + 1),
    )?;
    let mut slides = Vec::new();
    for (index, (slide, window)) in recipe.study.slides.iter().zip(&windows).enumerate() {
        let partition = if d.intensity.model == IntensityModel::BinaryCompartments {
            let c = d
                .compartments
                .iter()
                .find(|c| c.slide_id == slide.slide_id)
                .expect("validated partition");
            let negative = framed_window(&c.negative, &slide.coordinate_frame_id, recipe.limits)?;
            let positive = framed_window(&c.positive, &slide.coordinate_frame_id, recipe.limits)?;
            budget.geometry(
                window,
                negative.translation_segment_count() + positive.translation_segment_count(),
            )?;
            budget.retain(negative.boundary_storage_bytes() + positive.boundary_storage_bytes())?;
            Some(
                BinaryCompartmentPartition2D::new(
                    window.clone(),
                    &c.negative_id,
                    negative,
                    &c.positive_id,
                    positive,
                    CompartmentPartitionLimits::new(recipe.limits.maximum_geometry_vertices)
                        .map_err(|e| invalid(e.to_string()))?,
                )
                .map_err(|e| invalid(e.to_string()))?,
            )
        } else {
            None
        };
        let allowance =
            (budget.limits.maximum_work - budget.work) / (recipe.study.slides.len() - index);
        let memory = budget.limits.memory_budget_bytes - budget.bytes;
        let pattern = pattern(slide, d.phenotype.as_deref())?;
        let mut output = SpatialSlide {
            slide_id: slide.slide_id.clone(),
            patient_id: slide.patient_id.clone(),
            group: slide.group.clone(),
            coordinate_frame_id: slide.coordinate_frame_id.clone(),
            area_mm2: window.area_um2() / 1e6,
            window: slide.window.clone(),
            mark_id: if d.statistic == Statistic::CrossGMinusOne {
                "phenotype"
            } else {
                "unmarked_or_selected_phenotype"
            }
            .into(),
            values: vec![None; d.radii_um.len()],
            eligible: vec![false; d.radii_um.len()],
            p_global: None,
            slide_family_p: None,
            null_model: None,
            intensity: Value::Null,
            support: Value::Null,
            unavailable: None,
            charged_work: 0,
        };
        match evaluate(
            &pattern,
            slide,
            window,
            d,
            recipe.limits,
            allowance,
            memory,
            d.seed.wrapping_add(index as u64),
            &recipe.study.phenotypes.measurement_status,
            partition.as_ref(),
        ) {
            Ok(result) => {
                let field = match d.statistic {
                    Statistic::LMinusR => "l",
                    Statistic::GMinusOne => "g",
                    Statistic::CrossGMinusOne => "cross_g",
                };
                let curve = result["curve"]
                    .as_array()
                    .ok_or_else(|| invalid("missing native curve"))?;
                output.values = curve
                    .iter()
                    .zip(&d.radii_um)
                    .map(|(p, r)| {
                        p[field]
                            .as_f64()
                            .map(|v| {
                                finite(
                                    v - if d.statistic == Statistic::LMinusR {
                                        *r
                                    } else {
                                        1.0
                                    },
                                )
                            })
                            .transpose()
                    })
                    .collect::<Result<Vec<_>>>()?;
                output.eligible = curve
                    .iter()
                    .map(|p| p["inference_eligible"].as_bool().unwrap_or(false))
                    .collect();
                output.null_model = result["inference"]["null_model"]
                    .as_str()
                    .map(str::to_owned);
                output.intensity = if d.statistic == Statistic::CrossGMinusOne {
                    json!({"source":result["source_intensity"],"target":result["target_intensity"]})
                } else {
                    result["intensity"].clone()
                };
                output.support = result["curve"].clone();
                output.charged_work = [
                    "intensity_evaluations",
                    "total_pair_visits",
                    "compartment_queries",
                ]
                .iter()
                .map(|k| result[*k].as_u64().unwrap_or(0) as usize)
                .sum::<usize>()
                    + result["inference"]["null_draws"].as_u64().unwrap_or(0) as usize;
                budget.work(output.charged_work)?;
                budget.retain(
                    result["estimated_storage_bytes"]
                        .as_u64()
                        .ok_or_else(|| invalid("missing native memory estimate"))?
                        as usize,
                )?;
                if output.values.len() != d.radii_um.len()
                    || output.values.iter().any(Option::is_none)
                    || output.eligible.iter().any(|v| !*v)
                {
                    output.unavailable = Some(
                        "complete declared radius axis is unsupported; no bins removed".into(),
                    );
                } else {
                    output.p_global = result["inference"]["p_global"].as_f64();
                    output.slide_family_p = output
                        .p_global
                        .map(|p| (p * recipe.study.slides.len() as f64).min(1.0));
                }
            }
            Err(Failure::Unavailable(reason)) => {
                output.unavailable = Some(reason);
                output.charged_work = allowance;
                budget.work(allowance)?;
            }
            Err(Failure::Fatal(reason)) => return Err(invalid(reason)),
        }
        slides.push(output);
    }
    let mut grouped = BTreeMap::<&str, Vec<&SpatialSlide>>::new();
    for s in &slides {
        grouped.entry(&s.patient_id).or_default().push(s);
    }
    let mut patients = Vec::new();
    for (id, ss) in grouped {
        let area = finite(ss.iter().map(|s| s.area_mm2).sum())?;
        let unavailable = ss
            .iter()
            .any(|s| s.unavailable.is_some())
            .then(|| "at least one slide lacks the complete curve; no slides dropped".into());
        let values = if unavailable.is_none() {
            Some(
                (0..d.radii_um.len())
                    .map(|i| {
                        finite(
                            ss.iter()
                                .map(|s| s.area_mm2 * s.values[i].expect("complete curve"))
                                .sum::<f64>()
                                / area,
                        )
                    })
                    .collect::<Result<Vec<_>>>()?,
            )
        } else {
            None
        };
        patients.push(SpatialPatient {
            patient_id: id.into(),
            group: ss[0].group.clone(),
            area_mm2: area,
            values,
            unavailable,
        });
    }
    let mut inference = SpatialInference {
        p_value: None,
        statistic: None,
        difference: None,
        unavailable: None,
    };
    if patients.iter().any(|p| p.values.is_none()) {
        inference.unavailable = Some("patient curve unavailable; no patients dropped".into());
    } else {
        let curves = patients
            .iter()
            .map(|p| FunctionalCurve {
                patient_id: p.patient_id.clone(),
                group: p.group.clone(),
                axis: d.radii_um.clone(),
                values: p.values.clone().expect("complete"),
            })
            .collect::<Vec<_>>();
        match functional_two_sample_permutation(
            &curves,
            &FunctionalPermutationSpec {
                group_a: d.group_a.clone(),
                group_b: d.group_b.clone(),
                statistic: FunctionalTestStatistic::L2,
                permutations: d.permutations,
                seed: d.seed,
            },
        ) {
            Ok(r) => {
                inference.p_value = Some(r.p_value);
                inference.statistic = Some(r.observed_statistic);
                inference.difference = Some(r.observed_difference);
            }
            Err(e) => inference.unavailable = Some(e.to_string()),
        }
    }
    Ok(PathologySpatialStudyResult{format:"marklab.pathology_spatial_study_result",version:1,study_id:recipe.study.study_id,phenotypes:recipe.study.phenotypes,design:recipe.design,limits:recipe.limits,slides,patients,inference,
        work_units:budget.work,estimated_storage_bytes:budget.bytes,claim_scope:"experimental_context_conditioned_curves; fitted_null_calibration_not_established; failed_slides_charge_reserved_work"})
}

fn validate(recipe: &Recipe<SpatialDesign>) -> Result<()> {
    let d = &recipe.design;
    let i = &d.intensity;
    inference_design(&d.group_a, &d.group_b, d.permutations, d.alpha)?;
    if d.radii_um.len() < 2
        || d.radii_um.len() > 64
        || d.radii_um.iter().any(|r| !r.is_finite() || *r <= 0.0)
        || d.radii_um.windows(2).any(|r| r[0] >= r[1])
        || recipe
            .study
            .slides
            .iter()
            .any(|s| s.group != d.group_a && s.group != d.group_b)
        || [&d.phenotype, &d.source, &d.target]
            .into_iter()
            .flatten()
            .any(|n| !recipe.study.phenotypes.names.contains(n))
    {
        return Err(invalid(
            "invalid spatial axis, groups or phenotype selection",
        ));
    }
    if d.statistic == Statistic::CrossGMinusOne {
        if d.phenotype.is_some()
            || d.source.is_none()
            || d.target.is_none()
            || d.source == d.target
            || i.model != IntensityModel::Gaussian
        {
            return Err(invalid(
                "cross-g requires distinct source/target phenotypes and Gaussian intensity",
            ));
        }
    } else if d.source.is_some() || d.target.is_some() {
        return Err(invalid("source/target only apply to cross-g"));
    }
    if !i.bandwidth_candidates_um.is_empty()
        && (d.statistic != Statistic::LMinusR
            || i.model != IntensityModel::Gaussian
            || i.cross_fit_folds.is_some()
            || i.bandwidth_candidates_um.len() > 16
            || i.bandwidth_candidates_um
                .iter()
                .any(|b| !b.is_finite() || *b <= 0.0)
            || i.bandwidth_candidates_um.windows(2).any(|b| b[0] >= b[1]))
    {
        return Err(invalid(
            "bandwidth selection supports increasing Gaussian L-r leave-one-out candidates only",
        ));
    }
    if i.model == IntensityModel::Gaussian {
        if i.null_mode == NullMode::ConditionalCompartments || !d.compartments.is_empty() {
            return Err(invalid(
                "Gaussian null requires plugin or refit and no compartment declarations",
            ));
        }
    } else {
        if i.null_mode != NullMode::ConditionalCompartments
            || i.cross_fit_folds.is_some()
            || !i.bandwidth_candidates_um.is_empty()
        {
            return Err(invalid(
                "binary compartments require their count-conditioned null",
            ));
        }
        let ids = d
            .compartments
            .iter()
            .map(|c| &c.slide_id)
            .collect::<BTreeSet<_>>();
        if ids.len() != recipe.study.slides.len()
            || ids.len() != d.compartments.len()
            || recipe
                .study
                .slides
                .iter()
                .any(|s| !ids.contains(&s.slide_id))
            || d.compartments.iter().any(|c| !text(&c.provenance))
        {
            return Err(invalid(
                "one declared exact compartment partition per slide is required",
            ));
        }
    }
    Ok(())
}

fn pattern(slide: &Slide, phenotype: Option<&str>) -> Result<Pattern> {
    let cells = slide
        .cells
        .iter()
        .filter(|c| phenotype.is_none_or(|p| c.phenotype == p))
        .collect::<Vec<_>>();
    let mut pattern = Pattern::from_arrays(
        cells.iter().map(|c| c.x_um).collect(),
        cells.iter().map(|c| c.y_um).collect(),
        vec![0; cells.len()],
        PatternMeta {
            case_id: slide.patient_id.clone(),
            timepoint: "study".into(),
            protein: phenotype.unwrap_or("all").into(),
            slide_id: Some(slide.slide_id.clone()),
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )?;
    pattern.cell_ids = Some(
        cells
            .iter()
            .map(|c| c.id.clone())
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    Ok(pattern)
}

#[allow(clippy::too_many_arguments)]
fn evaluate(
    pattern: &Pattern,
    slide: &Slide,
    window: &ObservationWindow2D,
    d: &SpatialDesign,
    limits: Limits,
    work: usize,
    memory: usize,
    seed: u64,
    measurement_status: &str,
    partition: Option<&BinaryCompartmentPartition2D>,
) -> std::result::Result<Value, Failure> {
    let share = work / 3;
    if share == 0 || memory == 0 {
        return Err(Failure::Fatal("spatial resource budget exhausted".into()));
    }
    if d.intensity.model == IntensityModel::BinaryCompartments {
        let partition = partition.expect("admitted partition");
        let cfg = PiecewiseCompartmentSpatialConfig::new(
            d.radii_um.clone(),
            d.permutations,
            seed,
            d.alpha,
            PiecewiseCompartmentSpatialLimits::new(
                limits.maximum_cells,
                d.radii_um.len(),
                share,
                share,
                share,
                memory,
            )?,
        )?;
        return match d.statistic {
            Statistic::LMinusR => serialized(
                native::analyze_piecewise_compartment_spatial_pattern(pattern, partition, &cfg)?,
            ),
            Statistic::GMinusOne => {
                serialized(native::analyze_piecewise_compartment_pair_correlation(
                    pattern,
                    partition,
                    &PiecewiseCompartmentPairCorrelationConfig::new(cfg, d.pair_bandwidth_um)?,
                )?)
            }
            Statistic::CrossGMinusOne => unreachable!("validated statistic"),
        };
    }
    let i = &d.intensity;
    let probes = i.grid[0]
        .checked_mul(i.grid[1])
        .ok_or_else(|| Failure::Fatal("probe count overflow".into()))?;
    if probes > 1_000_000 {
        return Err(Failure::Fatal("probe limit exceeded".into()));
    }
    let bounds = InhomogeneousSpatialLimits::new(
        limits.maximum_cells,
        d.radii_um.len(),
        probes,
        share,
        share,
        share,
        memory,
    )?;
    let config = if let Some(folds) = i.cross_fit_folds {
        InhomogeneousSpatialConfig::new_cross_fitted(
            d.radii_um.clone(),
            i.bandwidth_um,
            i.grid,
            folds,
            d.permutations,
            seed,
            d.alpha,
            i.minimum_intensity_per_um2,
            bounds,
        )?
    } else {
        InhomogeneousSpatialConfig::new(
            d.radii_um.clone(),
            i.bandwidth_um,
            i.grid,
            d.permutations,
            seed,
            d.alpha,
            i.minimum_intensity_per_um2,
            bounds,
        )?
    };
    let policy = native::study_fit::StudyFitPolicy {
        refit: i.null_mode == NullMode::Refit,
        bandwidth_candidates_um: i.bandwidth_candidates_um.clone(),
    };
    match d.statistic {
        Statistic::LMinusR => {
            serialized(native::analyze_inhomogeneous_spatial_pattern_with_policy(
                pattern, window, &config, &policy,
            )?)
        }
        Statistic::GMinusOne => {
            serialized(native::analyze_inhomogeneous_pair_correlation_with_policy(
                pattern,
                window,
                &InhomogeneousPairCorrelationConfig::new(config, d.pair_bandwidth_um)?,
                &policy,
            )?)
        }
        Statistic::CrossGMinusOne => {
            let source = d.source.as_ref().expect("source");
            let target = d.target.as_ref().expect("target");
            let source_rows = slide
                .cells
                .iter()
                .enumerate()
                .filter_map(|(i, c)| (&c.phenotype == source).then_some(i))
                .collect();
            let target_rows = slide
                .cells
                .iter()
                .enumerate()
                .filter_map(|(i, c)| (&c.phenotype == target).then_some(i))
                .collect();
            let cfg = InhomogeneousCategoricalCrossPairCorrelationConfig::new(
                config,
                d.pair_bandwidth_um,
                source,
                target,
            )?;
            serialized(native::analyze_category_rows(
                pattern,
                window,
                &cfg,
                source_rows,
                target_rows,
                "phenotype",
                measurement_status,
                &policy,
            )?)
        }
    }
}
