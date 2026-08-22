use std::time::Instant;

use crate::{
    config::{AnalysisConfig, NeighborhoodNullModel, RegistrationTransform},
    diagnostics::graph_smoothing::graph_smoothing_with_labels,
    errors::{MarklabError, Result},
    geom::spatial_index::SpatialIndex2D,
    multimodal::{
        cells::{AnalysisMetadata, HeCell, IhcCell},
        fusion::{fuse_registered_cells, FusionMeta},
        labels::PrimaryLabelEncoding,
        memory::MultimodalMemoryBudget,
        null_sensitivity::{
            analyze_null_model, NullModelAnalysisContext, NullModelSensitivityResult,
        },
        registration_artifacts::{
            analyze_registration_artifacts, RegistrationExtrapolation, RegistrationResidual,
        },
    },
    neighborhood::{
        cross_curves::cross_interaction_curves_with_index,
        enrichment::{edge_enrichment_with_labels, LabelPair},
        graph::{build_spatial_graph_with_index, GraphConfig, SpatialGraph},
        profiles::{compare_territory_profiles, territory_profiles_with_index},
        territories::{detect_mmr_abnormal_territories_with_index, TerritoryDomainConfig},
    },
    output::{
        AnalysisSection, AnalysisStatus, DiagnosticsResult, FusedCellSummary, Interpretation,
        InterpretationClass, MultimodalResult, TimingStage,
    },
    registration::{
        landmarks::LandmarkPair,
        qc::registration_qc,
        transform::{fit_affine, fit_rigid, Transform2D},
    },
};

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(test)]
static MULTIMODAL_ANALYSIS_CALLS: AtomicUsize = AtomicUsize::new(0);

#[cfg(test)]
pub(crate) fn reset_multimodal_analysis_call_count() {
    MULTIMODAL_ANALYSIS_CALLS.store(0, Ordering::SeqCst);
}

#[cfg(test)]
pub(crate) fn multimodal_analysis_call_count() -> usize {
    MULTIMODAL_ANALYSIS_CALLS.load(Ordering::SeqCst)
}

#[derive(Clone, Debug, PartialEq)]
pub struct MultimodalInput {
    pub he_cells: Vec<HeCell>,
    pub ihc_cells: Vec<IhcCell>,
    pub landmarks: Vec<LandmarkPair>,
    pub case_id: String,
    pub timepoint: String,
    pub protein: String,
}

pub struct MultimodalEngine {
    config: AnalysisConfig,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MultimodalAnalysisRun {
    pub result: MultimodalResult,
    pub transform: Transform2D,
    pub graph: SpatialGraph,
    pub null_model_sensitivity: Vec<NullModelSensitivityResult>,
    pub registration_residuals: Vec<RegistrationResidual>,
    pub extrapolation: RegistrationExtrapolation,
}

impl MultimodalEngine {
    pub fn new(config: AnalysisConfig) -> Result<Self> {
        config.validate()?;
        if config.diagnostics.beta_posterior_groups {
            return Err(MarklabError::Config(
                "beta_posterior_groups diagnostic requires marked-pattern input; multimodal analyze supports graph_smoothing only".into(),
            ));
        }
        if !config.registration.enabled {
            return Err(MarklabError::Config(
                "multimodal analyze requires [registration].enabled = true".into(),
            ));
        }
        if !config.neighborhood.enabled {
            return Err(MarklabError::Config(
                "multimodal analyze requires [neighborhood].enabled = true".into(),
            ));
        }
        Ok(Self { config })
    }

    pub fn analyze(&self, input: &MultimodalInput) -> Result<MultimodalResult> {
        Ok(self.analyze_run(input)?.result)
    }

    pub fn analyze_run(&self, input: &MultimodalInput) -> Result<MultimodalAnalysisRun> {
        #[cfg(test)]
        MULTIMODAL_ANALYSIS_CALLS.fetch_add(1, Ordering::SeqCst);

        let mut timings =
            Vec::with_capacity(15usize.saturating_add(self.config.neighborhood.null_models.len()));
        timed_multimodal_stage(&mut timings, "validate_input", || {
            validate_input(input, &self.config)
        })?;
        let mut memory_budget = MultimodalMemoryBudget::for_run(&self.config, input)?;
        let transform = timed_multimodal_stage(&mut timings, "registration_fit", || {
            match self.config.registration.transform {
                RegistrationTransform::Affine => fit_affine(&input.landmarks),
                RegistrationTransform::Rigid => fit_rigid(&input.landmarks),
            }
        })?;
        let registration = timed_multimodal_stage(&mut timings, "registration_qc", || {
            registration_qc(
                &input.landmarks,
                &transform,
                self.config.registration.claim_distance_multiplier,
            )
        })?;
        if registration.rmse_um > self.config.registration.max_rmse_um {
            return Err(MarklabError::Validation(format!(
                "registration RMSE {:.3} um exceeds configured max_rmse_um {:.3} um",
                registration.rmse_um, self.config.registration.max_rmse_um
            )));
        }

        let registration_error_um = registration.usable_min_distance_um / 2.0;
        let fusion_meta = FusionMeta {
            analysis: AnalysisMetadata {
                case_id: input.case_id.clone(),
                timepoint: input.timepoint.clone(),
                protein: input.protein.clone(),
            },
            registration_error_um: Some(registration_error_um),
        };
        let fused = timed_multimodal_stage(&mut timings, "fusion", || {
            fuse_registered_cells(&input.he_cells, &input.ihc_cells, &transform, &fusion_meta)
        })?;
        let label_storage_upper_bound = memory_budget.reserve_label_encoding_and_index(&fused)?;
        let primary_labels = timed_multimodal_stage(&mut timings, "label_encoding", || {
            PrimaryLabelEncoding::new(&fused)
        })?;
        debug_assert!(primary_labels.estimated_storage_bytes() <= label_storage_upper_bound);
        let spatial_index = timed_multimodal_stage(&mut timings, "spatial_index", || {
            SpatialIndex2D::from_points(
                fused
                    .iter()
                    .map(|cell| [cell.x_um_registered, cell.y_um_registered]),
            )
        })?;
        memory_budget.reserve_configured_label_pairs(&self.config.neighborhood.label_pairs)?;
        let label_pairs = self
            .config
            .neighborhood
            .label_pairs
            .iter()
            .map(|pair| LabelPair::new(pair[0].clone(), pair[1].clone()))
            .collect::<Vec<_>>();
        memory_budget.reserve_registration_and_enrichment_results(
            input,
            &fused,
            &label_pairs,
            self.config.neighborhood.null_models.len(),
        )?;
        let graph_build = timed_multimodal_stage(&mut timings, "graph", || {
            build_spatial_graph_with_index(
                &fused,
                &spatial_index,
                GraphConfig {
                    radius_um: Some(self.config.neighborhood.radius_um),
                    k_nearest: (self.config.neighborhood.k_nearest > 0)
                        .then_some(self.config.neighborhood.k_nearest),
                },
                memory_budget.remaining_bytes(),
            )
        })?;
        memory_budget.observe_transient(
            "graph construction",
            graph_build.estimated_peak_storage_bytes,
        )?;
        let graph = graph_build.graph;
        memory_budget.reserve_retained("spatial graph", graph.estimated_storage_bytes())?;
        memory_budget.observe_registration_scratch(input.landmarks.len())?;
        let registration_artifacts =
            timed_multimodal_stage(&mut timings, "artifact_projections", || {
                analyze_registration_artifacts(&input.landmarks, &transform, &fused)
            })?;
        memory_budget.observe_enrichment_scratch(fused.len(), self.config.permutation.b)?;
        let enrichment = timed_multimodal_stage(&mut timings, "enrichment_primary", || {
            edge_enrichment_with_labels(
                &fused,
                &primary_labels,
                &graph,
                &label_pairs,
                self.config.permutation.b,
                self.config.permutation.seed,
            )
        })?;
        let mut null_model_sensitivity =
            Vec::with_capacity(self.config.neighborhood.null_models.len());
        let null_model_context = NullModelAnalysisContext {
            fused: &fused,
            labels: &primary_labels,
            graph: &graph,
            label_pairs: &label_pairs,
            primary_source_section: &enrichment,
            permutations: self.config.permutation.b,
            seed: self.config.permutation.seed,
        };
        for null_model in self.config.neighborhood.null_models.iter().copied() {
            let result =
                timed_multimodal_stage(&mut timings, null_model_timing_name(null_model), || {
                    analyze_null_model(&null_model_context, null_model)
                })?;
            null_model_sensitivity.push(result);
        }
        let cross_bin_width_um = (self.config.neighborhood.radius_um / 5.0).max(1.0);
        let cross_interaction_analysis =
            timed_multimodal_stage(&mut timings, "cross_interaction_curves", || {
                cross_interaction_curves_with_index(
                    &fused,
                    &spatial_index,
                    &primary_labels,
                    &label_pairs,
                    cross_bin_width_um,
                    self.config.neighborhood.radius_um,
                    self.config.permutation.b,
                    self.config.permutation.seed,
                    self.config.inference.family_wise_alpha,
                    memory_budget.remaining_bytes(),
                )
            })?;
        memory_budget.observe_transient(
            "cross-interaction execution",
            cross_interaction_analysis.estimated_peak_storage_bytes,
        )?;
        let cross_interaction_curves = cross_interaction_analysis.curves;
        memory_budget.reserve_cross_curves(&cross_interaction_curves)?;
        let neighborhood_territory_analysis =
            timed_multimodal_stage(&mut timings, "territory_detection", || {
                detect_mmr_abnormal_territories_with_index(
                    &fused,
                    &primary_labels,
                    &spatial_index,
                    TerritoryDomainConfig {
                        eps_um: self.config.neighborhood.territory_eps_um,
                        min_cells: self.config.neighborhood.territory_min_cells,
                        min_radius_um: self.config.neighborhood.territory_min_radius_um,
                    },
                    memory_budget.remaining_bytes(),
                )
            })?;
        memory_budget.observe_transient(
            "territory neighborhood execution",
            neighborhood_territory_analysis.estimated_peak_storage_bytes,
        )?;
        let neighborhood_territories = neighborhood_territory_analysis.territories;
        memory_budget.reserve_territories(&neighborhood_territories)?;
        memory_budget
            .reserve_profiles_and_comparisons(neighborhood_territories.len(), &primary_labels)?;
        let territory_profiles =
            timed_multimodal_stage(&mut timings, "territory_profiles", || {
                territory_profiles_with_index(
                    &neighborhood_territories,
                    &fused,
                    &primary_labels,
                    &spatial_index,
                    0.0,
                )
            })?;
        let territory_comparisons =
            timed_multimodal_stage(&mut timings, "territory_comparison", || {
                compare_territory_profiles(
                    &territory_profiles,
                    self.config.comparison.margins.territory_profile,
                )
            })?;

        let territory_comparisons = if territory_comparisons.is_empty() {
            AnalysisSection::InsufficientData {
                reason: "territory-profile comparison requires at least two comparable groups"
                    .into(),
            }
        } else {
            AnalysisSection::available(territory_comparisons)
        };
        if self.config.diagnostics.graph_smoothing {
            memory_budget.reserve_graph_smoothing(
                graph.n_nodes,
                graph.edges.len(),
                &primary_labels,
                &label_pairs,
            )?;
        }
        let diagnostics = timed_multimodal_stage(&mut timings, "diagnostics", || -> Result<_> {
            if self.config.diagnostics.graph_smoothing {
                Ok(AnalysisSection::available(DiagnosticsResult {
                    beta_posterior_groups: None,
                    graph_smoothing: Some(graph_smoothing_with_labels(
                        &fused,
                        &primary_labels,
                        &graph,
                        &label_pairs,
                    )?),
                }))
            } else {
                Ok(AnalysisSection::Disabled)
            }
        })?;

        let n_fused_cells = fused.len();
        let n_mmr_abnormal = fused.iter().filter(|cell| cell.mmr_mark == Some(1)).count();
        memory_budget.reserve_interpretation(
            "Multimodal registration, fusion, and neighborhood enrichment summary.",
        )?;
        let estimated_peak_memory_mib = memory_budget.peak_mib();
        let mut result =
            timed_multimodal_stage(&mut timings, "result_assembly", || MultimodalResult {
                case_id: fusion_meta.analysis.case_id,
                timepoint: fusion_meta.analysis.timepoint,
                protein: fusion_meta.analysis.protein,
                status: AnalysisStatus::Ok,
                registration: AnalysisSection::available(registration),
                fused_cell_summary: AnalysisSection::available(FusedCellSummary {
                    n_he_cells: input.he_cells.len(),
                    n_ihc_cells: input.ihc_cells.len(),
                    n_fused_cells,
                    registration_error_um: Some(registration_error_um),
                }),
                fused_cells: fused,
                neighborhood_enrichment: AnalysisSection::available(enrichment),
                cross_interaction_curves: AnalysisSection::available(cross_interaction_curves),
                neighborhood_territories: AnalysisSection::available(neighborhood_territories),
                territory_profiles: AnalysisSection::available(territory_profiles),
                territory_comparisons,
                diagnostics,
                timings: Vec::new(),
                interpretation: Interpretation {
                    class: InterpretationClass::MultimodalSummary,
                    text: "Multimodal registration, fusion, and neighborhood enrichment summary."
                        .into(),
                },
            });
        annotate_multimodal_timings(
            &mut timings,
            n_fused_cells,
            n_mmr_abnormal,
            self.config.permutation.b,
            estimated_peak_memory_mib,
        );
        result.timings = timings;

        Ok(MultimodalAnalysisRun {
            result,
            transform,
            graph,
            null_model_sensitivity,
            registration_residuals: registration_artifacts.residuals,
            extrapolation: registration_artifacts.extrapolation,
        })
    }
}

fn timed_multimodal_stage<T>(
    timings: &mut Vec<TimingStage>,
    stage_name: &'static str,
    operation: impl FnOnce() -> T,
) -> T {
    let started = Instant::now();
    let output = operation();
    timings.push(TimingStage {
        stage_name: stage_name.into(),
        wall_ms: started.elapsed().as_secs_f64() * 1_000.0,
        cpu_threads: 1,
        n_cells: 0,
        n_marked: 0,
        n_k_modes: 0,
        n_permutations: 0,
        estimated_peak_memory_mib: 0.0,
    });
    output
}

fn annotate_multimodal_timings(
    timings: &mut [TimingStage],
    n_cells: usize,
    n_mmr_abnormal: usize,
    n_permutations: usize,
    estimated_peak_memory_mib: f64,
) {
    for timing in timings {
        timing.n_cells = n_cells;
        timing.n_marked = n_mmr_abnormal;
        timing.n_permutations = n_permutations;
        timing.estimated_peak_memory_mib = estimated_peak_memory_mib;
    }
}

const fn null_model_timing_name(null_model: NeighborhoodNullModel) -> &'static str {
    match null_model {
        NeighborhoodNullModel::SourceSection => "enrichment_null_source_section",
        NeighborhoodNullModel::SourceSectionDensity => "enrichment_null_source_section_density",
        NeighborhoodNullModel::SourceSectionCellClass => {
            "enrichment_null_source_section_cell_class"
        }
        NeighborhoodNullModel::SourceSectionRegistrationQc => {
            "enrichment_null_source_section_registration_qc"
        }
    }
}

fn validate_input(input: &MultimodalInput, config: &AnalysisConfig) -> Result<()> {
    for (name, value) in [
        ("case_id", input.case_id.as_str()),
        ("timepoint", input.timepoint.as_str()),
        ("protein", input.protein.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(MarklabError::Schema(format!(
                "MultimodalInput.{name} must not be blank"
            )));
        }
    }
    if input.landmarks.len() < config.registration.min_landmarks {
        return Err(MarklabError::Validation(format!(
            "registration requires at least {} landmarks, found {}",
            config.registration.min_landmarks,
            input.landmarks.len()
        )));
    }
    Ok(())
}
