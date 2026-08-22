use crate::{
    config::{AnalysisConfig, RegistrationTransform},
    diagnostics::graph_smoothing::graph_smoothing,
    errors::{MarklabError, Result},
    geom::spatial_index::SpatialIndex2D,
    multimodal::{
        cells::{AnalysisMetadata, HeCell, IhcCell},
        fusion::{fuse_registered_cells, FusionMeta},
        null_sensitivity::{analyze_null_model_sensitivity, NullModelSensitivityResult},
        registration_artifacts::{
            analyze_registration_artifacts, RegistrationExtrapolation, RegistrationResidual,
        },
    },
    neighborhood::{
        cross_curves::cross_interaction_curve,
        enrichment::{edge_enrichment, LabelPair},
        graph::{build_spatial_graph_with_index, GraphConfig, SpatialGraph},
        profiles::{compare_territory_profiles, territory_profiles_with_index},
        territories::{detect_mmr_abnormal_territories_with_index, TerritoryDomainConfig},
    },
    output::{
        AnalysisSection, DiagnosticsResult, FusedCellSummary, Interpretation, MultimodalResult,
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

        validate_input(input, &self.config)?;
        let transform = match self.config.registration.transform {
            RegistrationTransform::Affine => fit_affine(&input.landmarks)?,
            RegistrationTransform::Rigid => fit_rigid(&input.landmarks)?,
        };
        let registration = registration_qc(
            &input.landmarks,
            &transform,
            self.config.registration.claim_distance_multiplier,
        )?;
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
        let fused =
            fuse_registered_cells(&input.he_cells, &input.ihc_cells, &transform, &fusion_meta)?;
        let spatial_index = SpatialIndex2D::from_points(
            fused
                .iter()
                .map(|cell| [cell.x_um_registered, cell.y_um_registered]),
        )?;
        let graph = build_spatial_graph_with_index(
            &fused,
            &spatial_index,
            GraphConfig {
                radius_um: Some(self.config.neighborhood.radius_um),
                k_nearest: (self.config.neighborhood.k_nearest > 0)
                    .then_some(self.config.neighborhood.k_nearest),
            },
        )?;
        let registration_artifacts =
            analyze_registration_artifacts(&input.landmarks, &transform, &fused)?;
        let label_pairs = self
            .config
            .neighborhood
            .label_pairs
            .iter()
            .map(|pair| LabelPair::new(pair[0].clone(), pair[1].clone()))
            .collect::<Vec<_>>();
        let enrichment = edge_enrichment(
            &fused,
            &graph,
            &label_pairs,
            self.config.permutation.b,
            self.config.permutation.seed,
        )?;
        let null_model_sensitivity = analyze_null_model_sensitivity(
            &fused,
            &graph,
            &label_pairs,
            &self.config.neighborhood.null_models,
            &enrichment,
            self.config.permutation.b,
            self.config.permutation.seed,
        )?;
        let cross_bin_width_um = (self.config.neighborhood.radius_um / 5.0).max(1.0);
        let cross_interaction_curves = label_pairs
            .iter()
            .map(|pair| {
                cross_interaction_curve(
                    &fused,
                    &pair.label_a,
                    &pair.label_b,
                    cross_bin_width_um,
                    self.config.neighborhood.radius_um,
                    self.config.permutation.b,
                    self.config.permutation.seed,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        let neighborhood_territories = detect_mmr_abnormal_territories_with_index(
            &fused,
            &spatial_index,
            TerritoryDomainConfig {
                eps_um: self.config.neighborhood.territory_eps_um,
                min_cells: self.config.neighborhood.territory_min_cells,
                min_radius_um: self.config.neighborhood.territory_min_radius_um,
            },
        )?;
        let territory_profiles =
            territory_profiles_with_index(&neighborhood_territories, &fused, &spatial_index, 0.0)?;
        let territory_comparisons = compare_territory_profiles(
            &territory_profiles,
            self.config.comparison.margins.territory_profile,
        )?;

        let territory_comparisons = if territory_comparisons.is_empty() {
            AnalysisSection::InsufficientData {
                reason: "territory-profile comparison requires at least two comparable groups"
                    .into(),
            }
        } else {
            AnalysisSection::available(territory_comparisons)
        };
        let diagnostics = if self.config.diagnostics.graph_smoothing {
            AnalysisSection::available(DiagnosticsResult {
                beta_posterior_groups: None,
                graph_smoothing: Some(graph_smoothing(&fused, &graph, &label_pairs)?),
            })
        } else {
            AnalysisSection::Disabled
        };

        let result = MultimodalResult {
            case_id: fusion_meta.analysis.case_id,
            timepoint: fusion_meta.analysis.timepoint,
            protein: fusion_meta.analysis.protein,
            status: "ok".into(),
            registration: AnalysisSection::available(registration),
            fused_cell_summary: AnalysisSection::available(FusedCellSummary {
                n_he_cells: input.he_cells.len(),
                n_ihc_cells: input.ihc_cells.len(),
                n_fused_cells: fused.len(),
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
                class: "multimodal_summary".into(),
                text: "Multimodal registration, fusion, and neighborhood enrichment summary."
                    .into(),
            },
        };

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
