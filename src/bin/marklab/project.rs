use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use marklab::{
    spatial::ArbitraryWindowIppLikelihoodResult, workflow::MarkedPrePostNode, MarkedPatternResult,
    ResultDocument,
};
use marklab_bayes::{
    BackendContract, BetaBinomialGroupGenderRegressionWorkerResult,
    BetaBinomialGroupGenderSlideHierarchyWorkerResult, BetaBinomialGroupRegressionWorkerResult,
    BetaBinomialHierarchyWorkerResult, DirichletMultinomialGroupWorkerResult,
    GriddedLgcpFitWorkerResult, HierarchicalWorkerResult, HurdleBetaBinomialGroupWorkerResult,
    NonproportionalOrdinalGroupSiteWorkerResult, OrdinalGroupSiteHierarchyWorkerResult,
    OrdinalGroupWorkerResult, StudentTHierarchyWorkerResult, WorkerResult,
};
use marklab_graph::{
    graph_sparse_radius_basis_workflow, graph_sparse_radius_diffusion_wavelet_workflow,
    graph_sparse_radius_fourier_energy_workflow, graph_sparse_radius_heat_stability_workflow,
    graph_sparse_radius_heat_workflow, graph_sparse_radius_scattering_workflow,
    GraphSparseRadiusBasisResult, GraphSparseRadiusBasisSpec,
    GraphSparseRadiusDiffusionWaveletResult, GraphSparseRadiusDiffusionWaveletSpec,
    GraphSparseRadiusFourierEnergyResult, GraphSparseRadiusFourierEnergySpec,
    GraphSparseRadiusHeatResult, GraphSparseRadiusHeatSpec, GraphSparseRadiusHeatStabilityResult,
    GraphSparseRadiusHeatStabilitySpec, GraphSparseRadiusScatteringResult,
    GraphSparseRadiusScatteringSpec,
};
use marklab_project::{ArtifactRef, ContentDigest};
use marklab_topology::{
    WitnessPersistenceBottleneckStabilityResult, WitnessPersistenceResult,
    WitnessPersistenceStabilityResult,
};
use marklab_transport::FusedGromovWassersteinWorkerResult;
use marklab_workflow::{
    CacheKeyMaterial, NodeError, NodeId, NodeSpec, WorkflowGraph, WorkflowNode,
};

use super::{
    bayes::{self, ArbitraryWindowIppFitResult, CliError, GriddedLgcpModelArgs},
    topology,
};

mod cli_schema;
mod durable;

pub(crate) use cli_schema::ProjectCommand;
use cli_schema::{
    ArbitraryWindowIppSpatialPpcProjectArgs, InProject, SmcAbcGrowthFrontProjectArgs,
};
use marklab_bayes::backend_versions::{PYMC_VERSION, PYTHON_VERSION};

use cli_schema::{
    ArbitraryWindowIppFitProjectArgs, ArbitraryWindowIppProjectArgs,
    ArbitraryWindowLgcpFitProjectArgs, BetaBinomialGroupGenderRegressionProjectArgs,
    BetaBinomialGroupGenderSlideHierarchyProjectArgs, BetaBinomialGroupRegressionProjectArgs,
    BetaBinomialHierarchyProjectArgs, DirichletMultinomialGroupProjectArgs,
    FusedGromovWassersteinProjectArgs, GriddedLgcpProjectArgs, HierarchicalNormalProjectArgs,
    HurdleBetaBinomialGroupProjectArgs, MarkedPrepostProjectArgs,
    NonproportionalOrdinalGroupSiteProjectArgs, NormalMeanProjectArgs, OrdinalGroupProjectArgs,
    OrdinalGroupSiteHierarchyProjectArgs, ReplicatedArbitraryWindowLgcpFitProjectArgs,
    ReplicatedArbitraryWindowLgcpInferredKernelProjectArgs, SparseRadiusBasisProjectArgs,
    SparseRadiusDiffusionWaveletProjectArgs, SparseRadiusFourierEnergyProjectArgs,
    SparseRadiusHeatProjectArgs, SparseRadiusHeatStabilityProjectArgs,
    SparseRadiusScatteringProjectArgs, StudentTHierarchyProjectArgs,
    WitnessPersistenceBottleneckStabilityProjectArgs, WitnessPersistenceProjectArgs,
    WitnessPersistenceStabilityProjectArgs,
};
pub(super) use durable::native_runtime_provenance;
use durable::{
    cache_status_name, invalid, run_native_spec, source_artifact, Codec, NativeSpecCommand,
    NodeContract, Session, Source, SourceNode,
};

mod adaptive_window_spde;
mod arbitrary_window_ipp_spatial_ppc;
mod causal_workflows;
mod cell_patch_complementarity;
mod cohort_cluster_covariate;
mod cohort_energy;
mod cohort_hierarchical_bootstrap;
mod cohort_hierarchical_max_t;
mod cohort_margin_inference;
mod cohort_max_t;
mod cohort_mmd;
mod cohort_multisite_covariate;
mod correlated_multitype_lgcp;
mod embedding_cross_covariance;
mod embedding_spatial_dependence_envelope;
mod gaussian_crossed_nested_hierarchy;
mod graph_dirichlet_energy;
mod graph_motif_summary;
mod graph_smoothness_permutation;
mod joint_location_embedding;
mod joint_location_mark;
mod kernel_mark_correlation;
mod local_embedding_roughness;
mod local_multivariate_moran;
mod longitudinal_kalman;
mod max_t_calibration;
mod multiscale_embedding_kernel;
mod multitype_lgcp_sbc;
mod negative_binomial_hierarchy;
mod nonstationary_adaptive_window_spde;
mod ordinal_site_heldout;
mod patient_nested_fields;
mod projected_embedding_variograms;
mod region_retrieval;
mod smc_abc;
mod sparse_car;
mod spatial3d_k;
mod spatial3d_registered;
mod spatial3d_registered_longitudinal;
mod spatial3d_voxel_k;
mod spatial_varying_coefficient;
mod vector_semivariogram;

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;
const MAXIMUM_RESULT_BYTES: usize = 1024 * 1024;
const MAXIMUM_WITNESS_RESULT_BYTES: usize = 32 * 1024 * 1024;
const MARKED_RESULT_KIND: &str = "application/vnd.marklab.result+json;version=0.4";

#[derive(Clone, Copy)]
enum StaticBackendWorkflow {
    PymcNormalMean,
    PymcHierarchicalNormal,
    PymcBetaBinomialHierarchy,
    PymcBetaBinomialGroupRegression,
    PymcDirichletMultinomialGroup,
    PymcOrdinalGroup,
    PymcHurdleBetaBinomialGroup,
    PymcOrdinalGroupSiteHierarchy,
    PymcNonproportionalOrdinalGroupSite,
    PymcBetaBinomialGroupGenderRegression,
    PymcBetaBinomialGroupGenderSlideHierarchy,
    PymcStudentTHierarchy,
    PymcGriddedLgcp,
    PymcArbitraryWindowIpp,
    PymcArbitraryWindowIppSpatialPpc,
    PymcArbitraryWindowLgcp,
    PymcReplicatedArbitraryWindowLgcp,
    PymcReplicatedArbitraryWindowLgcpInferredKernel,
    PymcReplicatedArbitraryWindowMultitypeLgcpInferredKernel,
    PymcReplicatedArbitraryWindowMultitypeLgcp,
    PymcConditionalMultitypeMark,
    PymcReplicatedConditionalMultitypeMark,
    PotFusedGromovWasserstein,
}

#[derive(Clone, Copy)]
struct StaticBackendDescriptor {
    backend_id: &'static str,
    backend_version: &'static str,
    python_version: &'static str,
    license: &'static str,
    input_kinds: &'static [&'static str],
    output_kind: &'static str,
    result_schema_id: &'static str,
    node_id: &'static str,
    node_kind: &'static str,
    implementation_identity: &'static str,
    deterministic_controls: &'static str,
}

impl StaticBackendWorkflow {
    fn descriptor(self) -> StaticBackendDescriptor {
        match self {
            Self::PymcNormalMean => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &["application/vnd.marklab.source.normal-mean-observations;version=1"],
                output_kind: "application/vnd.marklab.pymc-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_worker_result",
                node_id: "pymc-normal-mean",
                node_kind: "bayesian_fit",
                implementation_identity: "marklab-project-pymc-normal-mean-node-v1",
                deterministic_controls: "seeded-nuts-request",
            },
            Self::PymcHierarchicalNormal => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.hierarchical-normal-observations;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-hierarchical-worker-result+json;version=2",
                result_schema_id: "marklab.pymc_hierarchical_worker_result",
                node_id: "pymc-hierarchical-normal",
                node_kind: "bayesian_hierarchical_fit",
                implementation_identity: "marklab-project-pymc-hierarchical-normal-node-v2",
                deterministic_controls: "seeded-nuts-hierarchical-request",
            },
            Self::PymcBetaBinomialHierarchy => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.beta-binomial-patient-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-beta-binomial-hierarchy-worker-result+json;version=2",
                result_schema_id: "marklab.pymc_beta_binomial_hierarchy_worker_result",
                node_id: "pymc-beta-binomial-hierarchy",
                node_kind: "bayesian_count_hierarchical_fit",
                implementation_identity: "marklab-project-pymc-beta-binomial-hierarchy-node-v2",
                deterministic_controls: "seeded-nuts-beta-binomial-hierarchical-request",
            },
            Self::PymcBetaBinomialGroupRegression => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.beta-binomial-patient-group-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-beta-binomial-group-regression-worker-result+json;version=2",
                result_schema_id: "marklab.pymc_beta_binomial_group_regression_worker_result",
                node_id: "pymc-beta-binomial-group-regression",
                node_kind: "bayesian_count_group_regression_fit",
                implementation_identity:
                    "marklab-project-pymc-beta-binomial-group-regression-node-v2",
                deterministic_controls: "seeded-nuts-beta-binomial-group-regression-request",
            },
            Self::PymcDirichletMultinomialGroup => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.dirichlet-multinomial-patient-group-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-dirichlet-multinomial-group-worker-result+json;version=2",
                result_schema_id: "marklab.pymc_dirichlet_multinomial_group_worker_result",
                node_id: "pymc-dirichlet-multinomial-group",
                node_kind: "bayesian_multiclass_count_group_regression_fit",
                implementation_identity:
                    "marklab-project-pymc-dirichlet-multinomial-group-node-v2",
                deterministic_controls: "seeded-nuts-dirichlet-multinomial-group-request",
            },
            Self::PymcOrdinalGroup => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.ordinal-patient-group-outcomes;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-ordinal-group-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_ordinal_group_worker_result",
                node_id: "pymc-ordinal-group",
                node_kind: "bayesian_ordinal_group_regression_fit",
                implementation_identity: "marklab-project-pymc-ordinal-group-node-v1",
                deterministic_controls: "seeded-nuts-ordinal-group-request",
            },
            Self::PymcHurdleBetaBinomialGroup => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.hurdle-beta-binomial-patient-group-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-hurdle-beta-binomial-group-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_hurdle_beta_binomial_group_worker_result",
                node_id: "pymc-hurdle-beta-binomial-group",
                node_kind: "bayesian_hurdle_count_group_regression_fit",
                implementation_identity:
                    "marklab-project-pymc-hurdle-beta-binomial-group-node-v1",
                deterministic_controls: "seeded-nuts-hurdle-beta-binomial-group-request",
            },
            Self::PymcOrdinalGroupSiteHierarchy => StaticBackendDescriptor {
                backend_id: "pymc", backend_version: PYMC_VERSION, python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &["application/vnd.marklab.source.ordinal-patient-group-site-outcomes;version=1"],
                output_kind: "application/vnd.marklab.pymc-ordinal-group-site-hierarchy-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_ordinal_group_site_hierarchy_worker_result",
                node_id: "pymc-ordinal-group-site-hierarchy", node_kind: "bayesian_ordinal_site_hierarchy_fit",
                implementation_identity: "marklab-project-pymc-ordinal-group-site-hierarchy-node-v1",
                deterministic_controls: "seeded-nuts-ordinal-group-site-hierarchy-request",
            },
            Self::PymcNonproportionalOrdinalGroupSite => StaticBackendDescriptor {
                backend_id:"pymc",backend_version: PYMC_VERSION,python_version: PYTHON_VERSION,license:"Apache-2.0",
                input_kinds:&["application/vnd.marklab.source.nonproportional-ordinal-patient-group-site-outcomes;version=1"],
                output_kind:"application/vnd.marklab.pymc-nonproportional-ordinal-group-site-worker-result+json;version=1",
                result_schema_id:"marklab.pymc_nonproportional_ordinal_group_site_worker_result",
                node_id:"pymc-nonproportional-ordinal-group-site",node_kind:"bayesian_nonproportional_ordinal_site_fit",
                implementation_identity:"marklab-project-pymc-nonproportional-ordinal-group-site-node-v1",
                deterministic_controls:"seeded-nuts-nonproportional-ordinal-group-site-request",
            },
            Self::PymcBetaBinomialGroupGenderRegression => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.beta-binomial-patient-group-gender-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-beta-binomial-group-gender-regression-worker-result+json;version=2",
                result_schema_id:
                    "marklab.pymc_beta_binomial_group_gender_regression_worker_result",
                node_id: "pymc-beta-binomial-group-gender-regression",
                node_kind: "bayesian_count_group_gender_regression_fit",
                implementation_identity:
                    "marklab-project-pymc-beta-binomial-group-gender-regression-node-v2",
                deterministic_controls:
                    "seeded-nuts-beta-binomial-group-gender-regression-request",
            },
            Self::PymcBetaBinomialGroupGenderSlideHierarchy => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.beta-binomial-group-gender-slide-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-beta-binomial-group-gender-slide-hierarchy-worker-result+json;version=2",
                result_schema_id:
                    "marklab.pymc_beta_binomial_group_gender_slide_hierarchy_worker_result",
                node_id: "pymc-beta-binomial-group-gender-slide-hierarchy",
                node_kind: "bayesian_repeated_slide_patient_hierarchical_fit",
                implementation_identity:
                    "marklab-project-pymc-beta-binomial-group-gender-slide-hierarchy-node-v2",
                deterministic_controls:
                    "seeded-nuts-beta-binomial-group-gender-slide-hierarchy-request",
            },
            Self::PymcStudentTHierarchy => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.student-t-hierarchy-observations;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-student-t-hierarchy-worker-result+json;version=2",
                result_schema_id: "marklab.pymc_student_t_hierarchy_worker_result",
                node_id: "pymc-student-t-hierarchy",
                node_kind: "bayesian_robust_hierarchical_fit",
                implementation_identity: "marklab-project-pymc-student-t-hierarchy-node-v2",
                deterministic_controls: "seeded-nuts-student-t-hierarchical-request",
            },
            Self::PymcGriddedLgcp => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.point-events;version=1",
                    "application/vnd.marklab.source.gridded-covariates;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-gridded-lgcp-worker-result+json;version=2",
                result_schema_id: "marklab.pymc_gridded_lgcp_worker_result",
                node_id: "pymc-gridded-lgcp",
                node_kind: "bayesian_point_process_field_fit",
                implementation_identity: "marklab-project-pymc-gridded-lgcp-node-v2",
                deterministic_controls: "seeded-nuts-fixed-grid-lgcp-request",
            },
            Self::PymcArbitraryWindowIpp => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.arbitrary-window-ipp-events+csv;version=1",
                    "application/vnd.marklab.source.arbitrary-window-ipp-quadrature+csv;version=1",
                    "application/vnd.marklab.source.observation-window+geojson;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-arbitrary-window-ipp-fit+json;version=1",
                result_schema_id: "marklab.pymc_arbitrary_window_ipp_fit_result",
                node_id: "pymc-arbitrary-window-ipp-fit",
                node_kind: "bayesian_point_process_fit",
                implementation_identity: "marklab-project-pymc-arbitrary-window-ipp-fit-node-v1",
                deterministic_controls: "seeded-nuts-weighted-exact-window-ipp-request",
            },
            Self::PymcArbitraryWindowIppSpatialPpc => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.arbitrary-window-ipp-events+csv;version=1",
                    "application/vnd.marklab.source.arbitrary-window-ipp-membership+csv;version=1",
                    "application/vnd.marklab.source.arbitrary-window-ipp-quadrature+csv;version=1",
                    "application/vnd.marklab.source.observation-window+geojson;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-arbitrary-window-ipp-spatial-ppc+json;version=1",
                result_schema_id: "marklab.pymc_arbitrary_window_ipp_spatial_ppc_result",
                node_id: "pymc-arbitrary-window-ipp-spatial-ppc",
                node_kind: "bayesian_point_process_spatial_posterior_predictive_check",
                implementation_identity:
                    "marklab-project-pymc-arbitrary-window-ipp-spatial-ppc-node-v1",
                deterministic_controls:
                    "seeded-nuts-and-posterior-predictive-exact-window-ipp-request",
            },
            Self::PymcArbitraryWindowLgcp => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.arbitrary-window-lgcp-events+csv;version=1",
                    "application/vnd.marklab.source.arbitrary-window-lgcp-membership+csv;version=1",
                    "application/vnd.marklab.source.arbitrary-window-lgcp-quadrature+csv;version=1",
                    "application/vnd.marklab.source.observation-window+geojson;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-arbitrary-window-lgcp-fit+json;version=2",
                result_schema_id: "marklab.pymc_arbitrary_window_lgcp_fit_result",
                node_id: "pymc-arbitrary-window-lgcp-fit",
                node_kind: "bayesian_point_process_latent_field_fit",
                implementation_identity: "marklab-project-pymc-arbitrary-window-lgcp-fit-node-v2",
                deterministic_controls:
                    "seeded-nuts-weighted-exact-window-fixed-matern-lgcp-request",
            },
            Self::PymcReplicatedArbitraryWindowLgcp => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.replicated-arbitrary-window-lgcp+csv;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-replicated-arbitrary-window-lgcp-fit+json;version=3",
                result_schema_id:
                    "marklab.pymc_replicated_arbitrary_window_lgcp_fit_result_v2",
                node_id: "pymc-replicated-arbitrary-window-lgcp-fit",
                node_kind: "bayesian_replicated_point_process_latent_field_fit",
                implementation_identity:
                    "marklab-project-pymc-replicated-arbitrary-window-lgcp-fit-node-v3",
                deterministic_controls:
                    "seeded-nuts-patient-pattern-hierarchy-fixed-matern-request",
            },
            Self::PymcReplicatedArbitraryWindowLgcpInferredKernel => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.replicated-arbitrary-window-lgcp+csv;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-replicated-arbitrary-window-lgcp-inferred-kernel-fit+json;version=2",
                result_schema_id:
                    "marklab.pymc_replicated_arbitrary_window_lgcp_inferred_kernel_fit_result",
                node_id: "pymc-replicated-arbitrary-window-lgcp-inferred-kernel-fit",
                node_kind: "bayesian_replicated_point_process_inferred_latent_field_fit",
                implementation_identity:
                    "marklab-project-pymc-replicated-arbitrary-window-lgcp-inferred-kernel-fit-node-v2",
                deterministic_controls:
                    "seeded-nuts-patient-pattern-hierarchy-inferred-shared-matern-request",
            },
            Self::PymcReplicatedArbitraryWindowMultitypeLgcpInferredKernel => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.replicated-arbitrary-window-multitype-lgcp+csv;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-replicated-arbitrary-window-multitype-lgcp-inferred-kernel-fit+json;version=2",
                result_schema_id:
                    "marklab.pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_fit_result",
                node_id: "pymc-replicated-arbitrary-window-multitype-lgcp-inferred-kernel-fit",
                node_kind: "bayesian_replicated_multitype_point_process_inferred_latent_field_fit",
                implementation_identity:
                    "marklab-project-pymc-replicated-arbitrary-window-multitype-lgcp-inferred-kernel-fit-node-v2",
                deterministic_controls:
                    "seeded-nuts-patient-pattern-type-hierarchy-inferred-shared-matern-request",
            },
            Self::PymcReplicatedArbitraryWindowMultitypeLgcp => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.replicated-arbitrary-window-multitype-lgcp+csv;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-replicated-arbitrary-window-multitype-lgcp-fit+json;version=2",
                result_schema_id:
                    "marklab.pymc_replicated_arbitrary_window_multitype_lgcp_fit_result",
                node_id: "pymc-replicated-arbitrary-window-multitype-lgcp-fit",
                node_kind: "bayesian_replicated_multitype_point_process_latent_field_fit",
                implementation_identity:
                    "marklab-project-pymc-replicated-arbitrary-window-multitype-lgcp-fit-node-v2",
                deterministic_controls:
                    "seeded-nuts-patient-pattern-type-hierarchy-fixed-matern-request",
            },
            Self::PymcConditionalMultitypeMark => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.conditional-multitype-mark+csv;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-conditional-multitype-mark-fit+json;version=2",
                result_schema_id: "marklab.conditional_multitype_mark_fit",
                node_id: "pymc-conditional-multitype-mark-fit",
                node_kind: "bayesian_conditional_multitype_mark_fit",
                implementation_identity:
                    "marklab-project-pymc-conditional-multitype-mark-fit-node-v2",
                deterministic_controls:
                    "seeded-nuts-fixed-location-radius-graph-conditional-mark-request",
            },
            Self::PymcReplicatedConditionalMultitypeMark => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.replicated-conditional-multitype-mark+csv;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-replicated-conditional-multitype-mark-fit+json;version=2",
                result_schema_id: "marklab.replicated_conditional_multitype_mark_fit",
                node_id: "pymc-replicated-conditional-multitype-mark-fit",
                node_kind: "bayesian_patient_replicated_conditional_multitype_mark_fit",
                implementation_identity:
                    "marklab-project-pymc-replicated-conditional-multitype-mark-fit-node-v3",
                deterministic_controls:
                    "seeded-nuts-patient-pattern-fixed-location-conditional-mark-request",
            },
            Self::PotFusedGromovWasserstein => StaticBackendDescriptor {
                backend_id: "pot",
                backend_version: "0.9.7.post1",
                python_version: PYTHON_VERSION,
                license: "MIT",
                input_kinds: &["application/vnd.marklab.source.fgw-input+json;version=1"],
                output_kind:
                    "application/vnd.marklab.pot-fused-gromov-worker-result+json;version=1",
                result_schema_id: "marklab.pot_fused_gromov_wasserstein_worker_result",
                node_id: "pot-fused-gromov-wasserstein",
                node_kind: "descriptive_alignment",
                implementation_identity: "marklab-project-pot-fused-gromov-node-v2",
                deterministic_controls: "fixed-initialization-order-and-solver-controls",
            },
        }
    }
}

impl StaticBackendDescriptor {
    fn validate_request_backend(self, backend: &BackendContract) -> Result<(), CliError> {
        if backend.name != self.backend_id
            || backend.version != self.backend_version
            || backend.python_version != self.python_version
            || !is_sha256(&backend.environment_lock_sha256)
            || !is_sha256(&backend.worker_sha256)
        {
            return Err(CliError::Input(format!(
                "{} durable backend identity is incomplete or differs from its static descriptor",
                self.backend_id
            )));
        }
        Ok(())
    }

    fn configuration_digest(
        self,
        backend: &BackendContract,
        request_bytes: &[u8],
    ) -> ContentDigest {
        let mut frames = vec![
            b"marklab-static-durable-backend-v1".as_slice(),
            self.backend_id.as_bytes(),
            self.backend_version.as_bytes(),
            self.python_version.as_bytes(),
            self.license.as_bytes(),
            backend.environment_lock_sha256.as_bytes(),
            backend.worker_sha256.as_bytes(),
        ];
        frames.extend(self.input_kinds.iter().map(|kind| kind.as_bytes()));
        frames.extend([
            self.result_schema_id.as_bytes(),
            self.deterministic_controls.as_bytes(),
            request_bytes,
        ]);
        ContentDigest::from_framed(frames)
    }

    fn execution_policy(self) -> Vec<u8> {
        format!(
            "marklab-static-process-policy-v1\0backend={}\0version={}\0python={}\0license={}\0controls={}\0environment=cleared\0network=cleared\0bounded-process=true",
            self.backend_id,
            self.backend_version,
            self.python_version,
            self.license,
            self.deterministic_controls
        )
        .into_bytes()
    }

    fn result_schema(self) -> (&'static str, u32) {
        (self.result_schema_id, 1)
    }

    /// Node contract for a worker request whose backend matches this descriptor.
    fn contract(
        self,
        backend: &BackendContract,
        request_bytes: &[u8],
        codec: Codec,
    ) -> Result<NodeContract, CliError> {
        self.validate_request_backend(backend)?;
        Ok(NodeContract {
            id: self.node_id,
            kind: self.node_kind,
            version: 1,
            configuration_digest: self.configuration_digest(backend, request_bytes),
            execution_policy: Cow::Owned(self.execution_policy()),
            implementation_identity: self.implementation_identity,
            output_kind: self.output_kind,
            codec,
        })
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn run_cli(command: ProjectCommand) -> Result<(), CliError> {
    match command {
        ProjectCommand::NegativeBinomialHierarchy(args) => negative_binomial_hierarchy::run(args),
        ProjectCommand::MaxTCalibration(args) => max_t_calibration::run(args),
        ProjectCommand::TestCellPatchComplementarity(args) => cell_patch_complementarity::run(args),
        ProjectCommand::CohortEnergy(args) => cohort_energy::run(args),
        ProjectCommand::CausalRandomizedInterference(args) => {
            causal_workflows::run_randomized_interference(args)
        }
        ProjectCommand::GaussianEig(args) => causal_workflows::run_gaussian_eig(args),
        ProjectCommand::CohortClusterCovariatePermutation(args) => {
            cohort_cluster_covariate::run(args)
        }
        ProjectCommand::CohortHierarchicalMaxT(args) => cohort_hierarchical_max_t::run(args),
        ProjectCommand::CohortHierarchicalBootstrap(args) => {
            cohort_hierarchical_bootstrap::run(args)
        }
        ProjectCommand::CohortEquivalence(args) => cohort_margin_inference::run_equivalence(args),
        ProjectCommand::CohortNoninferiority(args) => {
            cohort_margin_inference::run_noninferiority(args)
        }
        ProjectCommand::PatientNestedFields(args) => patient_nested_fields::run(args),
        ProjectCommand::CohortMaxT(args) => cohort_max_t::run(args),
        ProjectCommand::CohortMmd(args) => cohort_mmd::run(args),
        ProjectCommand::CohortMultisiteCovariateContrast(args) => {
            cohort_multisite_covariate::run(args)
        }
        ProjectCommand::ProjectedEmbeddingVariograms(args) => {
            projected_embedding_variograms::run(args)
        }
        ProjectCommand::VectorSemivariogram(args) => vector_semivariogram::run(args),
        ProjectCommand::LocalMultivariateMoran(args) => local_multivariate_moran::run(args),
        ProjectCommand::EmbeddingCrossCovarianceByDistance(args) => {
            embedding_cross_covariance::run(args)
        }
        ProjectCommand::KernelMarkCorrelation(args) => kernel_mark_correlation::run(args),
        ProjectCommand::EmbeddingSpatialDependenceEnvelope(args) => {
            embedding_spatial_dependence_envelope::run(args)
        }
        ProjectCommand::GraphDirichletEnergy(args) => graph_dirichlet_energy::run(args),
        ProjectCommand::GraphSmoothnessPermutationTest(args) => {
            graph_smoothness_permutation::run(args)
        }
        ProjectCommand::LocalEmbeddingRoughness(args) => local_embedding_roughness::run(args),
        ProjectCommand::LongitudinalKalmanSmooth(args) => longitudinal_kalman::run(args),
        ProjectCommand::MultiscaleEmbeddingKernel(args) => multiscale_embedding_kernel::run(args),
        ProjectCommand::RegionRetrieval(args) => region_retrieval::run(args),
        ProjectCommand::MarkedPrepost(args) => run_marked_prepost(args),
        ProjectCommand::SparseRadiusHeat(args) => run_sparse_radius_heat(args),
        ProjectCommand::SparseRadiusBasis(args) => run_sparse_radius_basis(args),
        ProjectCommand::GraphMotifTriangleSummary(args) => graph_motif_summary::run(args),
        ProjectCommand::SmcAbcGrowthFront(args) => smc_abc::run(*args),
        ProjectCommand::SpatialVaryingCoefficient(args) => spatial_varying_coefficient::run(args),
        ProjectCommand::SparseCarFit(args) => sparse_car::run(args),
        ProjectCommand::Spatial3dKFunction(args) => spatial3d_k::run(args),
        ProjectCommand::Spatial3dVoxelKFunction(args) => spatial3d_voxel_k::run(args),
        ProjectCommand::Spatial3dRegisteredSerialVoxelK(args) => spatial3d_registered::run(args),
        ProjectCommand::Spatial3dRegisteredLongitudinalVoxelKChange(args) => {
            spatial3d_registered_longitudinal::run(args)
        }
        ProjectCommand::SparseRadiusFourierEnergy(args) => run_sparse_radius_fourier_energy(args),
        ProjectCommand::SparseRadiusHeatStability(args) => run_sparse_radius_heat_stability(args),
        ProjectCommand::SparseRadiusDiffusionWavelet(args) => {
            run_sparse_radius_diffusion_wavelet(args)
        }
        ProjectCommand::SparseRadiusScattering(args) => run_sparse_radius_scattering(args),
        ProjectCommand::WitnessPersistence(args) => run_witness_persistence(args),
        ProjectCommand::WitnessPersistenceStability(args) => {
            run_witness_persistence_stability(args)
        }
        ProjectCommand::WitnessPersistenceBottleneckStability(args) => {
            run_witness_persistence_bottleneck_stability(args)
        }
        ProjectCommand::ArbitraryWindowIppLikelihood(args) => run_arbitrary_window_ipp(*args),
        ProjectCommand::FitArbitraryWindowIpp(args) => run_arbitrary_window_ipp_fit(*args),
        ProjectCommand::ArbitraryWindowIppSpatialPpc(args) => {
            arbitrary_window_ipp_spatial_ppc::run(*args)
        }
        ProjectCommand::ArbitraryWindowLgcp(args) => run_arbitrary_window_lgcp(*args),
        ProjectCommand::ReplicatedArbitraryWindowLgcp(args) => {
            run_replicated_arbitrary_window_lgcp(*args)
        }
        ProjectCommand::ReplicatedArbitraryWindowLgcpInferredKernel(args) => {
            run_replicated_arbitrary_window_lgcp_inferred_kernel(*args)
        }
        ProjectCommand::ReplicatedArbitraryWindowMultitypeLgcp(args) => {
            run_replicated_arbitrary_window_multitype_lgcp(*args)
        }
        ProjectCommand::ReplicatedArbitraryWindowMultitypeLgcpInferredKernel(args) => {
            run_replicated_arbitrary_window_multitype_lgcp_inferred_kernel(*args)
        }
        ProjectCommand::ReplicatedArbitraryWindowMultitypeLgcpInferredKernelSbc(args) => {
            multitype_lgcp_sbc::run(*args)
        }
        ProjectCommand::CorrelatedReplicatedArbitraryWindowMultitypeLgcp(args) => {
            correlated_multitype_lgcp::run(*args)
        }
        ProjectCommand::JointReplicatedLocationEmbedding(args) => {
            joint_location_embedding::run(*args)
        }
        ProjectCommand::GaussianCrossedNestedHierarchy(args) => {
            gaussian_crossed_nested_hierarchy::run(*args)
        }
        ProjectCommand::AdaptiveWindowSpde(args) => adaptive_window_spde::run(args),
        ProjectCommand::NonstationaryAdaptiveWindowSpde(args) => {
            nonstationary_adaptive_window_spde::run(args)
        }
        ProjectCommand::JointReplicatedLocationMark(args) => joint_location_mark::run(*args),
        ProjectCommand::ConditionalMultitypeMark(args) => run_conditional_multitype_mark(*args),
        ProjectCommand::ReplicatedConditionalMultitypeMark(args) => {
            run_replicated_conditional_multitype_mark(*args)
        }
        ProjectCommand::NormalMean(args) => run_normal_mean(args),
        ProjectCommand::HierarchicalNormal(args) => run_hierarchical_normal(args),
        ProjectCommand::BetaBinomialHierarchy(args) => run_beta_binomial_hierarchy(args),
        ProjectCommand::BetaBinomialGroupRegression(args) => {
            run_beta_binomial_group_regression(args)
        }
        ProjectCommand::DirichletMultinomialGroup(args) => run_dirichlet_multinomial_group(args),
        ProjectCommand::OrdinalGroup(args) => run_ordinal_group(args),
        ProjectCommand::OrdinalGroupSiteHierarchy(args) => run_ordinal_group_site_hierarchy(args),
        ProjectCommand::NonproportionalOrdinalGroupSite(args) => {
            run_nonproportional_ordinal_group_site(args)
        }
        ProjectCommand::OrdinalSiteHeldoutComparison(args) => ordinal_site_heldout::run(args),
        ProjectCommand::HurdleBetaBinomialGroup(args) => run_hurdle_beta_binomial_group(args),
        ProjectCommand::BetaBinomialGroupGenderRegression(args) => {
            run_beta_binomial_group_gender_regression(args)
        }
        ProjectCommand::BetaBinomialGroupGenderSlideHierarchy(args) => {
            run_beta_binomial_group_gender_slide_hierarchy(args)
        }
        ProjectCommand::StudentTHierarchy(args) => run_student_t_hierarchy(args),
        ProjectCommand::GriddedLgcp(args) => run_gridded_lgcp(args),
        ProjectCommand::FusedGromovWasserstein(args) => run_fused_gromov_wasserstein(args),
    }
}

const SPARSE_RADIUS_HEAT: NativeSpecCommand = NativeSpecCommand {
    name: "sparse-radius-heat",
    label: "sparse radius heat input",
    input_kind: "application/vnd.marklab.source.graph-sparse-radius-heat+json;version=1",
    node_id: "sparse-radius-heat",
    node_kind: "graph_signal_transform",
    node_version: 1,
    implementation_identity: "marklab-project-sparse-radius-heat-node-v1",
    execution_policy: b"native-safe-rust-sparse-radius-heat-v1",
    output_kind: "application/vnd.marklab.graph-sparse-radius-heat-result+json;version=1",
    result_schema: ("marklab.graph_sparse_radius_heat_result", 1),
    codec: Codec::ExactFloatJson,
};

fn run_sparse_radius_heat(args: SparseRadiusHeatProjectArgs) -> Result<(), CliError> {
    let SparseRadiusHeatProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_spec(
        &SPARSE_RADIUS_HEAT,
        &project_path,
        input_path,
        &output_path,
        |spec: &GraphSparseRadiusHeatSpec| {
            graph_sparse_radius_heat_workflow(spec.clone()).map_err(NodeError::execution)
        },
        |spec, output: &GraphSparseRadiusHeatResult| {
            output.validate_for_spec(spec).map_err(NodeError::decode)
        },
    )
}

const SPARSE_RADIUS_BASIS: NativeSpecCommand = NativeSpecCommand {
    name: "sparse-radius-basis",
    label: "sparse radius basis input",
    input_kind: "application/vnd.marklab.source.graph-sparse-radius-basis+json;version=2",
    node_id: "sparse-radius-basis",
    node_kind: "graph_sparse_low_frequency_basis",
    node_version: 2,
    implementation_identity: "marklab-project-sparse-radius-basis-node-v3",
    execution_policy: b"native-safe-rust-sparse-radius-basis-v3-complement-checked",
    output_kind: "application/vnd.marklab.graph-sparse-radius-basis-result+json;version=2",
    result_schema: ("marklab.graph_sparse_radius_basis_result", 2),
    codec: Codec::ExactFloatJson,
};

fn run_sparse_radius_basis(args: SparseRadiusBasisProjectArgs) -> Result<(), CliError> {
    let SparseRadiusBasisProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_spec(
        &SPARSE_RADIUS_BASIS,
        &project_path,
        input_path,
        &output_path,
        |spec: &GraphSparseRadiusBasisSpec| {
            graph_sparse_radius_basis_workflow(spec.clone()).map_err(NodeError::execution)
        },
        |spec, output: &GraphSparseRadiusBasisResult| {
            output.validate_for_spec(spec).map_err(NodeError::decode)
        },
    )
}

const SPARSE_RADIUS_FOURIER_ENERGY: NativeSpecCommand = NativeSpecCommand {
    name: "sparse-radius-fourier-energy",
    label: "sparse radius Fourier energy input",
    input_kind: "application/vnd.marklab.source.graph-sparse-radius-fourier-energy+json;version=2",
    node_id: "sparse-radius-fourier-energy",
    node_kind: "graph_sparse_fourier_energy",
    node_version: 2,
    implementation_identity: "marklab-project-sparse-radius-fourier-energy-node-v3",
    execution_policy: b"native-safe-rust-sparse-radius-fourier-energy-v3-complement-checked",
    output_kind: "application/vnd.marklab.graph-sparse-radius-fourier-energy-result+json;version=2",
    result_schema: ("marklab.graph_sparse_radius_fourier_energy_result", 2),
    codec: Codec::ExactFloatJson,
};

fn run_sparse_radius_fourier_energy(
    args: SparseRadiusFourierEnergyProjectArgs,
) -> Result<(), CliError> {
    let SparseRadiusFourierEnergyProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_spec(
        &SPARSE_RADIUS_FOURIER_ENERGY,
        &project_path,
        input_path,
        &output_path,
        |spec: &GraphSparseRadiusFourierEnergySpec| {
            graph_sparse_radius_fourier_energy_workflow(spec.clone()).map_err(NodeError::execution)
        },
        |spec, output: &GraphSparseRadiusFourierEnergyResult| {
            output.validate_for_spec(spec).map_err(NodeError::decode)
        },
    )
}

const SPARSE_RADIUS_HEAT_STABILITY: NativeSpecCommand = NativeSpecCommand {
    name: "sparse-radius-heat-stability",
    label: "sparse radius heat stability input",
    input_kind: "application/vnd.marklab.source.graph-sparse-radius-heat-stability+json;version=1",
    node_id: "sparse-radius-heat-stability",
    node_kind: "graph_coordinate_perturbation_stability",
    node_version: 1,
    implementation_identity: "marklab-project-sparse-radius-heat-stability-node-v1",
    execution_policy: b"native-safe-rust-sparse-radius-heat-stability-v1",
    output_kind: "application/vnd.marklab.graph-sparse-radius-heat-stability-result+json;version=1",
    result_schema: ("marklab.graph_sparse_radius_heat_stability_result", 1),
    codec: Codec::ExactFloatJson,
};

fn run_sparse_radius_heat_stability(
    args: SparseRadiusHeatStabilityProjectArgs,
) -> Result<(), CliError> {
    let SparseRadiusHeatStabilityProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_spec(
        &SPARSE_RADIUS_HEAT_STABILITY,
        &project_path,
        input_path,
        &output_path,
        |spec: &GraphSparseRadiusHeatStabilitySpec| {
            graph_sparse_radius_heat_stability_workflow(spec.clone()).map_err(NodeError::execution)
        },
        |spec, output: &GraphSparseRadiusHeatStabilityResult| {
            output.validate_for_spec(spec).map_err(NodeError::decode)
        },
    )
}

const SPARSE_RADIUS_DIFFUSION_WAVELET: NativeSpecCommand = NativeSpecCommand {
    name: "sparse-radius-diffusion-wavelet",
    label: "sparse radius diffusion wavelet input",
    input_kind:
        "application/vnd.marklab.source.graph-sparse-radius-diffusion-wavelet+json;version=1",
    node_id: "sparse-radius-diffusion-wavelet",
    node_kind: "graph_sparse_diffusion_wavelet",
    node_version: 1,
    implementation_identity: "marklab-project-sparse-radius-diffusion-wavelet-node-v1",
    execution_policy: b"native-safe-rust-sparse-radius-diffusion-wavelet-v1",
    output_kind:
        "application/vnd.marklab.graph-sparse-radius-diffusion-wavelet-result+json;version=1",
    result_schema: ("marklab.graph_sparse_radius_diffusion_wavelet_result", 1),
    codec: Codec::ExactFloatJson,
};

fn run_sparse_radius_diffusion_wavelet(
    args: SparseRadiusDiffusionWaveletProjectArgs,
) -> Result<(), CliError> {
    let SparseRadiusDiffusionWaveletProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_spec(
        &SPARSE_RADIUS_DIFFUSION_WAVELET,
        &project_path,
        input_path,
        &output_path,
        |spec: &GraphSparseRadiusDiffusionWaveletSpec| {
            graph_sparse_radius_diffusion_wavelet_workflow(spec.clone())
                .map_err(NodeError::execution)
        },
        |spec, output: &GraphSparseRadiusDiffusionWaveletResult| {
            output.validate_for_spec(spec).map_err(NodeError::decode)
        },
    )
}

const SPARSE_RADIUS_SCATTERING: NativeSpecCommand = NativeSpecCommand {
    name: "sparse-radius-scattering",
    label: "sparse radius scattering input",
    input_kind: "application/vnd.marklab.source.graph-sparse-radius-scattering+json;version=1",
    node_id: "sparse-radius-scattering",
    node_kind: "graph_sparse_scattering",
    node_version: 1,
    implementation_identity: "marklab-project-sparse-radius-scattering-node-v1",
    execution_policy: b"native-safe-rust-sparse-radius-scattering-v1",
    output_kind: "application/vnd.marklab.graph-sparse-radius-scattering-result+json;version=1",
    result_schema: ("marklab.graph_sparse_radius_scattering_result", 1),
    codec: Codec::ExactFloatJson,
};

fn run_sparse_radius_scattering(args: SparseRadiusScatteringProjectArgs) -> Result<(), CliError> {
    let SparseRadiusScatteringProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_spec(
        &SPARSE_RADIUS_SCATTERING,
        &project_path,
        input_path,
        &output_path,
        |spec: &GraphSparseRadiusScatteringSpec| {
            graph_sparse_radius_scattering_workflow(spec.clone()).map_err(NodeError::execution)
        },
        |spec, output: &GraphSparseRadiusScatteringResult| {
            output.validate_for_spec(spec).map_err(NodeError::decode)
        },
    )
}

fn run_witness_persistence(args: WitnessPersistenceProjectArgs) -> Result<(), CliError> {
    let WitnessPersistenceProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    const LABEL: &str = "witness persistence input";
    let source = Source::identify(
        input_path,
        "application/vnd.marklab.source.witness-persistence+json;version=1",
    )?;
    let prepared = topology::prepare_witness_persistence(source.path())?;
    source.ensure_unchanged(LABEL)?;
    let contract = NodeContract {
        id: "gudhi-witness-persistence",
        kind: "topological_witness_approximation",
        version: 1,
        configuration_digest: ContentDigest::from_framed([
            b"marklab-project-gudhi-witness-persistence-node-v1".as_slice(),
            prepared.request_bytes.as_slice(),
        ]),
        execution_policy: Cow::Borrowed(b"bounded-pinned-gudhi-subprocess-v1"),
        implementation_identity: "marklab-project-gudhi-witness-persistence-node-v1",
        output_kind: "application/vnd.marklab.gudhi-witness-persistence-result+json;version=1",
        codec: Codec::ExactFloatJson,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| topology::execute_witness_persistence(prepared).map_err(NodeError::execution),
        |prepared, output: &WitnessPersistenceResult| {
            output
                .validate(&prepared.request, &prepared.request_bytes)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        ("marklab.gudhi_witness_persistence_result", 1),
        MAXIMUM_WITNESS_RESULT_BYTES,
    )?;
    durable::publish(
        "witness-persistence",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn run_witness_persistence_stability(
    args: WitnessPersistenceStabilityProjectArgs,
) -> Result<(), CliError> {
    let WitnessPersistenceStabilityProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    const LABEL: &str = "witness persistence stability input";
    let source = Source::identify(
        input_path,
        "application/vnd.marklab.source.witness-persistence-stability+json;version=1",
    )?;
    let prepared = topology::prepare_witness_persistence_stability(source.path())?;
    source.ensure_unchanged(LABEL)?;
    let mut frames = vec![b"marklab-project-witness-persistence-stability-node-v1".as_slice()];
    frames.extend(prepared.runs.iter().map(|run| run.request_bytes.as_slice()));
    let contract = NodeContract {
        id: "gudhi-witness-persistence-stability",
        kind: "topological_witness_coordinate_stability",
        version: 1,
        configuration_digest: ContentDigest::from_framed(frames),
        execution_policy: Cow::Borrowed(b"bounded-gudhi-witness-persistence-stability-v1"),
        implementation_identity: "marklab-project-witness-persistence-stability-node-v1",
        output_kind:
            "application/vnd.marklab.gudhi-witness-persistence-stability-result+json;version=1",
        codec: Codec::ExactFloatJson,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| {
            topology::execute_witness_persistence_stability(prepared).map_err(NodeError::execution)
        },
        |prepared, output: &WitnessPersistenceStabilityResult| {
            topology::validate_witness_persistence_stability_result(prepared, output)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        ("marklab.gudhi_witness_persistence_stability_result", 1),
        MAXIMUM_WITNESS_RESULT_BYTES,
    )?;
    durable::publish(
        "witness-persistence-stability",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn run_witness_persistence_bottleneck_stability(
    args: WitnessPersistenceBottleneckStabilityProjectArgs,
) -> Result<(), CliError> {
    let WitnessPersistenceBottleneckStabilityProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    const LABEL: &str = "witness bottleneck stability input";
    let source = Source::identify(
        input_path,
        "application/vnd.marklab.source.witness-persistence-bottleneck-stability+json;version=1",
    )?;
    let prepared = topology::prepare_witness_persistence_bottleneck_stability(source.path())?;
    source.ensure_unchanged(LABEL)?;
    let mut frames =
        vec![b"marklab-project-witness-persistence-bottleneck-stability-node-v1".as_slice()];
    frames.extend(
        prepared
            .stability
            .runs
            .iter()
            .map(|run| run.request_bytes.as_slice()),
    );
    frames.push(prepared.backend.environment_lock_sha256.as_bytes());
    frames.push(prepared.backend.worker_sha256.as_bytes());
    let contract = NodeContract {
        id: "gudhi-witness-persistence-bottleneck-stability",
        kind: "topological_witness_coordinate_bottleneck_stability",
        version: 1,
        configuration_digest: ContentDigest::from_framed(frames),
        execution_policy: Cow::Borrowed(
            b"bounded-gudhi-witness-persistence-bottleneck-stability-v1",
        ),
        implementation_identity: "marklab-project-witness-persistence-bottleneck-stability-node-v1",
        output_kind: "application/vnd.marklab.gudhi-witness-persistence-bottleneck-stability-result+json;version=1",
        codec: Codec::ExactFloatJson,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| {
            topology::execute_witness_persistence_bottleneck_stability(prepared)
                .map_err(NodeError::execution)
        },
        |prepared, output: &WitnessPersistenceBottleneckStabilityResult| {
            topology::validate_witness_persistence_bottleneck_stability_result(prepared, output)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        (
            "marklab.gudhi_witness_persistence_bottleneck_stability_result",
            1,
        ),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "witness-persistence-bottleneck-stability",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn run_arbitrary_window_ipp(args: ArbitraryWindowIppProjectArgs) -> Result<(), CliError> {
    let ArbitraryWindowIppProjectArgs {
        project: project_path,
        events: events_path,
        quadrature: quadrature_path,
        window: window_path,
        intercept,
        coefficient,
        maximum_events,
        maximum_quadrature_nodes,
        maximum_work,
        maximum_retained_bytes,
        out: output_path,
    } = args;
    const LABEL: &str = "arbitrary-window IPP source";
    let sources = Source::identify_all(
        [events_path, quadrature_path, window_path],
        &[
            "application/vnd.marklab.source.arbitrary-window-ipp-events+csv;version=1",
            "application/vnd.marklab.source.arbitrary-window-ipp-quadrature+csv;version=1",
            "application/vnd.marklab.source.observation-window+geojson;version=1",
        ],
    )?;
    let prepared = bayes::prepare_arbitrary_window_ipp(
        sources[0].path().to_owned(),
        sources[1].path().to_owned(),
        sources[2].path().to_owned(),
        intercept,
        coefficient,
        marklab::spatial::ArbitraryWindowIppLimits {
            maximum_events,
            maximum_quadrature_nodes,
            maximum_work,
            maximum_retained_bytes,
        },
    )?;
    Source::ensure_all_unchanged(&sources, LABEL)?;
    let configuration_bytes = serde_json::to_vec(&serde_json::json!({
        "intercept": intercept,
        "coefficient": coefficient,
        "maximum_events": maximum_events,
        "maximum_quadrature_nodes": maximum_quadrature_nodes,
        "maximum_work": maximum_work,
        "maximum_retained_bytes": maximum_retained_bytes,
    }))?;
    let contract = NodeContract {
        id: "arbitrary-window-ipp-likelihood",
        kind: "fixed_point_process_likelihood",
        version: 1,
        configuration_digest: ContentDigest::from_framed([
            b"marklab-project-arbitrary-window-ipp-likelihood-node-v1".as_slice(),
            configuration_bytes.as_slice(),
        ]),
        execution_policy: Cow::Borrowed(b"native-safe-rust-weighted-arbitrary-window-ipp-v1"),
        implementation_identity: "marklab-project-arbitrary-window-ipp-likelihood-node-v1",
        output_kind:
            "application/vnd.marklab.arbitrary-window-ipp-likelihood-result+json;version=1",
        codec: Codec::ExactFloatJson,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        prepared,
        |prepared| bayes::execute_arbitrary_window_ipp(prepared).map_err(NodeError::execution),
        |prepared, output: &ArbitraryWindowIppLikelihoodResult| {
            output
                .validate_for_spec(&prepared.window, &prepared.spec)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.arbitrary_window_ipp_likelihood_result", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "arbitrary-window-ipp-likelihood",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn run_arbitrary_window_ipp_fit(args: ArbitraryWindowIppFitProjectArgs) -> Result<(), CliError> {
    let ArbitraryWindowIppFitProjectArgs {
        project: project_path,
        events: events_path,
        quadrature: quadrature_path,
        window: window_path,
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_mean,
        coefficient_prior_sd,
        nuts,
        maximum_events,
        maximum_quadrature_nodes,
        maximum_draw_node_work,
        timeout_seconds,
        out: output_path,
    } = args;
    let sampling = nuts.spec();
    const LABEL: &str = "arbitrary-window IPP fit source";
    let backend = StaticBackendWorkflow::PymcArbitraryWindowIpp.descriptor();
    let sources = Source::identify_all(
        [events_path, quadrature_path, window_path],
        backend.input_kinds,
    )?;
    let prepared =
        bayes::prepare_arbitrary_window_ipp_fit(crate::bayes::ArbitraryWindowIppFitParameters {
            events_path: sources[0].path().to_owned(),
            quadrature_path: sources[1].path().to_owned(),
            window_path: sources[2].path().to_owned(),
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_mean,
            coefficient_prior_sd,
            sampling,
            maximum_events,
            maximum_quadrature_nodes,
            maximum_draw_node_work,
            timeout_seconds,
        })?;
    Source::ensure_all_unchanged(&sources, LABEL)?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::ExactFloatJson,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        prepared,
        |prepared| bayes::execute_arbitrary_window_ipp_fit(prepared).map_err(NodeError::execution),
        |prepared, output: &ArbitraryWindowIppFitResult| {
            prepared.validate_result(output).map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "fit-arbitrary-window-ipp",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn run_arbitrary_window_lgcp(args: ArbitraryWindowLgcpFitProjectArgs) -> Result<(), CliError> {
    let ArbitraryWindowLgcpFitProjectArgs {
        project: project_path,
        events: events_path,
        event_membership: membership_path,
        quadrature: quadrature_path,
        window: window_path,
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_mean,
        coefficient_prior_sd,
        field_amplitude,
        field_length_scale_um,
        jitter,
        nuts,
        maximum_events,
        maximum_quadrature_nodes,
        maximum_draw_node_work,
        prediction_replicates,
        prediction_seed,
        maximum_predictive_points,
        neighbor_radius_um,
        maximum_neighbor_pairs,
        timeout_seconds,
        out: output_path,
    } = args;
    let sampling = nuts.spec();
    const LABEL: &str = "arbitrary-window LGCP source";
    let backend = StaticBackendWorkflow::PymcArbitraryWindowLgcp.descriptor();
    let sources = Source::identify_all(
        [events_path, membership_path, quadrature_path, window_path],
        backend.input_kinds,
    )?;
    let prepared =
        bayes::prepare_arbitrary_window_lgcp_fit(crate::bayes::ArbitraryWindowLgcpFitArgs {
            events: sources[0].path().to_owned(),
            event_membership: sources[1].path().to_owned(),
            quadrature: sources[2].path().to_owned(),
            window: sources[3].path().to_owned(),
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_mean,
            coefficient_prior_sd,
            field_amplitude,
            field_length_scale_um,
            jitter,
            chains: sampling.chains,
            tune: sampling.tune_per_chain,
            draws: sampling.draws_per_chain,
            target_accept: sampling.target_accept,
            seed: sampling.seed,
            maximum_events,
            maximum_quadrature_nodes,
            maximum_draw_node_work,
            prediction_replicates,
            prediction_seed,
            maximum_predictive_points,
            neighbor_radius_um,
            maximum_neighbor_pairs,
            timeout_seconds,
            out: std::path::PathBuf::new(),
        })?;
    Source::ensure_all_unchanged(&sources, LABEL)?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::ExactFloatJson,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        prepared,
        |prepared| bayes::execute_arbitrary_window_lgcp_fit(prepared).map_err(NodeError::execution),
        |prepared, output: &bayes::ArbitraryWindowLgcpFitResult| {
            prepared.validate_result(output).map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "arbitrary-window-lgcp",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn run_replicated_arbitrary_window_lgcp(
    args: ReplicatedArbitraryWindowLgcpFitProjectArgs,
) -> Result<(), CliError> {
    let ReplicatedArbitraryWindowLgcpFitProjectArgs {
        project: project_path,
        input: input_path,
        reference_group,
        comparison_group,
        intercept_prior_mean,
        intercept_prior_sd,
        group_effect_prior_sd,
        covariate_effect_prior_sd,
        patient_sd_prior_scale,
        pattern_sd_prior_scale,
        field_amplitude,
        field_length_scale_um,
        jitter,
        nuts,
        maximum_patients,
        maximum_patterns,
        maximum_nodes_per_pattern,
        maximum_total_nodes,
        maximum_total_events,
        maximum_draw_node_work,
        timeout_seconds,
        out: output_path,
    } = args;
    let sampling = nuts.spec();
    const LABEL: &str = "replicated LGCP source";
    let backend = StaticBackendWorkflow::PymcReplicatedArbitraryWindowLgcp.descriptor();
    let source = Source::identify(input_path.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_replicated_arbitrary_window_lgcp_fit(
        crate::bayes::ReplicatedArbitraryWindowLgcpFitArgs {
            input: input_path,
            reference_group,
            comparison_group,
            intercept_prior_mean,
            intercept_prior_sd,
            group_effect_prior_sd,
            covariate_effect_prior_sd,
            patient_sd_prior_scale,
            pattern_sd_prior_scale,
            field_amplitude,
            field_length_scale_um,
            jitter,
            chains: sampling.chains,
            tune: sampling.tune_per_chain,
            draws: sampling.draws_per_chain,
            target_accept: sampling.target_accept,
            seed: sampling.seed,
            maximum_patients,
            maximum_patterns,
            maximum_nodes_per_pattern,
            maximum_total_nodes,
            maximum_total_events,
            maximum_draw_node_work,
            timeout_seconds,
            out: std::path::PathBuf::new(),
        },
    )?;
    source.ensure_unchanged(LABEL)?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::ExactFloatJson,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_replicated_arbitrary_window_lgcp_fit(prepared)
                .map_err(NodeError::execution)
        },
        |prepared, output: &bayes::ReplicatedArbitraryWindowLgcpFitResult| {
            prepared.validate_result(output).map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "replicated-arbitrary-window-lgcp",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn run_replicated_arbitrary_window_lgcp_inferred_kernel(
    args: ReplicatedArbitraryWindowLgcpInferredKernelProjectArgs,
) -> Result<(), CliError> {
    let ReplicatedArbitraryWindowLgcpInferredKernelProjectArgs {
        project: project_path,
        input: input_path,
        reference_group,
        comparison_group,
        intercept_prior_mean,
        intercept_prior_sd,
        group_effect_prior_sd,
        covariate_effect_prior_sd,
        patient_sd_prior_scale,
        pattern_sd_prior_scale,
        field_amplitude_prior_scale,
        field_length_scale_prior_scale_um,
        jitter,
        nuts,
        maximum_patients,
        maximum_patterns,
        maximum_nodes_per_pattern,
        maximum_total_nodes,
        maximum_total_events,
        maximum_draw_node_work,
        maximum_kernel_cube_work,
        maximum_tree_depth,
        timeout_seconds,
        out: output_path,
    } = args;
    let sampling = nuts.spec();
    const LABEL: &str = "inferred-kernel replicated LGCP source";
    let backend =
        StaticBackendWorkflow::PymcReplicatedArbitraryWindowLgcpInferredKernel.descriptor();
    let source = Source::identify(input_path.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_replicated_arbitrary_window_lgcp_inferred_kernel(
        crate::bayes::ReplicatedArbitraryWindowLgcpInferredKernelArgs {
            input: input_path,
            reference_group,
            comparison_group,
            intercept_prior_mean,
            intercept_prior_sd,
            group_effect_prior_sd,
            covariate_effect_prior_sd,
            patient_sd_prior_scale,
            pattern_sd_prior_scale,
            field_amplitude_prior_scale,
            field_length_scale_prior_scale_um,
            jitter,
            chains: sampling.chains,
            tune: sampling.tune_per_chain,
            draws: sampling.draws_per_chain,
            target_accept: sampling.target_accept,
            seed: sampling.seed,
            maximum_patients,
            maximum_patterns,
            maximum_nodes_per_pattern,
            maximum_total_nodes,
            maximum_total_events,
            maximum_draw_node_work,
            maximum_kernel_cube_work,
            maximum_tree_depth,
            timeout_seconds,
            out: std::path::PathBuf::new(),
        },
    )?;
    source.ensure_unchanged(LABEL)?;
    let contract = backend.contract(
        prepared.backend(),
        prepared.request_bytes(),
        Codec::ExactFloatJson,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_replicated_arbitrary_window_lgcp_inferred_kernel(prepared)
                .map_err(NodeError::execution)
        },
        |prepared, output: &bayes::ReplicatedArbitraryWindowLgcpInferredKernelResult| {
            prepared.validate_result(output).map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "replicated-arbitrary-window-lgcp-inferred-kernel",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn run_replicated_arbitrary_window_multitype_lgcp(
    args: InProject<bayes::ReplicatedArbitraryWindowMultitypeLgcpArgs>,
) -> Result<(), CliError> {
    const LABEL: &str = "replicated multitype LGCP source";
    let InProject {
        project: project_path,
        command: mut arguments,
    } = args;
    let backend = StaticBackendWorkflow::PymcReplicatedArbitraryWindowMultitypeLgcp.descriptor();
    let source = Source::identify(arguments.input.clone(), backend.input_kinds[0])?;
    let output_path = std::mem::take(&mut arguments.out);
    let prepared = bayes::prepare_replicated_arbitrary_window_multitype_lgcp(arguments)?;
    source.ensure_unchanged(LABEL)?;
    let contract = backend.contract(
        prepared.backend(),
        prepared.request_bytes(),
        Codec::ExactFloatJson,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_replicated_arbitrary_window_multitype_lgcp(prepared)
                .map_err(NodeError::execution)
        },
        |prepared, output: &bayes::ReplicatedArbitraryWindowMultitypeLgcpResult| {
            prepared.validate_result(output).map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "replicated-arbitrary-window-multitype-lgcp",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn run_conditional_multitype_mark(
    args: InProject<bayes::ConditionalMultitypeMarkArgs>,
) -> Result<(), CliError> {
    const LABEL: &str = "conditional multitype source";
    let InProject {
        project: project_path,
        command: mut arguments,
    } = args;
    let backend = StaticBackendWorkflow::PymcConditionalMultitypeMark.descriptor();
    let input_bytes = crate::input_file::read_regular_file(
        &arguments.input,
        MAXIMUM_INPUT_BYTES,
        "conditional multitype source exceeds input ceiling",
    )?;
    let source = Source::from_bytes(
        arguments.input.clone(),
        backend.input_kinds[0],
        &input_bytes,
    )?;
    let input_sha256 = marklab_bayes::sha256_hex(&input_bytes);
    let request_backend = bayes::conditional_multitype_mark_backend_contract()?;
    let configuration_bytes = serde_json::to_vec(&serde_json::json!({
        "reference_type": &arguments.reference_type,
        "radius_um": arguments.radius_um,
        "intercept_prior_sd": arguments.intercept_prior_sd,
        "interaction_prior_sd": arguments.interaction_prior_sd,
        "chains": arguments.chains,
        "tune": arguments.tune,
        "draws": arguments.draws,
        "target_accept": arguments.target_accept,
        "seed": arguments.seed,
        "maximum_points": arguments.maximum_points,
        "maximum_types": arguments.maximum_types,
        "maximum_neighbor_visits": arguments.maximum_neighbor_visits,
        "maximum_edges": arguments.maximum_edges,
        "maximum_draw_parameter_work": arguments.maximum_draw_parameter_work,
        "maximum_working_bytes": arguments.maximum_working_bytes,
        "maximum_tree_depth": arguments.maximum_tree_depth,
        "timeout_seconds": arguments.timeout_seconds,
    }))?;
    source.ensure_unchanged(LABEL)?;
    let contract = backend.contract(
        &request_backend,
        &configuration_bytes,
        Codec::ExactFloatJson,
    )?;
    let output_path = std::mem::take(&mut arguments.out);
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        (arguments, input_sha256, request_backend),
        |(arguments, _, _)| {
            bayes::execute_conditional_multitype_mark(arguments.clone())
                .map_err(NodeError::execution)
        },
        |(arguments, input_sha256, request_backend),
         output: &bayes::ConditionalMultitypeMarkResult| {
            output
                .validate_for(
                    input_sha256,
                    &arguments.reference_type,
                    arguments.radius_um,
                    request_backend,
                )
                .map_err(bayes::protocol_error)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "conditional-multitype-mark",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn run_replicated_arbitrary_window_multitype_lgcp_inferred_kernel(
    args: InProject<bayes::ReplicatedArbitraryWindowMultitypeLgcpInferredKernelArgs>,
) -> Result<(), CliError> {
    const LABEL: &str = "replicated multitype inferred-kernel source";
    let InProject {
        project: project_path,
        command: mut arguments,
    } = args;
    let backend = StaticBackendWorkflow::PymcReplicatedArbitraryWindowMultitypeLgcpInferredKernel
        .descriptor();
    let source = Source::identify(arguments.input.clone(), backend.input_kinds[0])?;
    let output_path = std::mem::take(&mut arguments.out);
    let prepared =
        bayes::prepare_replicated_arbitrary_window_multitype_lgcp_inferred_kernel(arguments)?;
    source.ensure_unchanged(LABEL)?;
    let contract = backend.contract(
        prepared.backend(),
        prepared.request_bytes(),
        Codec::ExactFloatJson,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_replicated_arbitrary_window_multitype_lgcp_inferred_kernel(prepared)
                .map_err(NodeError::execution)
        },
        |prepared, output: &bayes::ReplicatedArbitraryWindowMultitypeLgcpInferredKernelOutput| {
            prepared.validate_result(output).map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "replicated-arbitrary-window-multitype-lgcp-inferred-kernel",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn run_replicated_conditional_multitype_mark(
    args: InProject<bayes::ReplicatedConditionalMultitypeMarkArgs>,
) -> Result<(), CliError> {
    const LABEL: &str = "replicated conditional-mark source";
    let InProject {
        project: project_path,
        command: mut arguments,
    } = args;
    let backend = StaticBackendWorkflow::PymcReplicatedConditionalMultitypeMark.descriptor();
    let input_bytes = crate::input_file::read_regular_file(
        &arguments.input,
        MAXIMUM_INPUT_BYTES,
        "replicated conditional-mark source exceeds durable input ceiling",
    )?;
    let source = Source::from_bytes(
        arguments.input.clone(),
        backend.input_kinds[0],
        &input_bytes,
    )?;
    let input_sha256 = marklab_bayes::sha256_hex(&input_bytes);
    let request_backend = bayes::replicated_conditional_multitype_mark_backend_contract()?;
    let configuration_bytes = serde_json::to_vec(&serde_json::json!({
        "reference_group": &arguments.reference_group,
        "reference_type": &arguments.reference_type,
        "radius_um": arguments.radius_um,
        "intercept_prior_sd": arguments.intercept_prior_sd,
        "interaction_prior_sd": arguments.interaction_prior_sd,
        "group_effect_prior_sd": arguments.group_effect_prior_sd,
        "patient_sd_prior_scale": arguments.patient_sd_prior_scale,
        "pattern_sd_prior_scale": arguments.pattern_sd_prior_scale,
        "chains": arguments.chains,
        "tune": arguments.tune,
        "draws": arguments.draws,
        "target_accept": arguments.target_accept,
        "seed": arguments.seed,
        "maximum_patients": arguments.maximum_patients,
        "maximum_patterns": arguments.maximum_patterns,
        "maximum_points": arguments.maximum_points,
        "maximum_types": arguments.maximum_types,
        "maximum_neighbor_visits": arguments.maximum_neighbor_visits,
        "maximum_edges": arguments.maximum_edges,
        "maximum_draw_parameter_work": arguments.maximum_draw_parameter_work,
        "maximum_working_bytes": arguments.maximum_working_bytes,
        "maximum_tree_depth": arguments.maximum_tree_depth,
        "timeout_seconds": arguments.timeout_seconds,
    }))?;
    source.ensure_unchanged(LABEL)?;
    let contract = backend.contract(
        &request_backend,
        &configuration_bytes,
        Codec::ExactFloatJson,
    )?;
    let output_path = std::mem::take(&mut arguments.out);
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        (arguments, input_sha256, request_backend),
        |(arguments, _, _)| {
            bayes::execute_replicated_conditional_multitype_mark(arguments.clone())
                .map_err(NodeError::execution)
        },
        |(arguments, input_sha256, request_backend),
         output: &bayes::ReplicatedConditionalMultitypeMarkResult| {
            output
                .validate_for(
                    input_sha256,
                    &arguments.reference_group,
                    &arguments.reference_type,
                    arguments.radius_um,
                    request_backend,
                )
                .map_err(bayes::protocol_error)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "replicated-conditional-multitype-mark",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn run_marked_prepost(args: MarkedPrepostProjectArgs) -> Result<(), CliError> {
    let MarkedPrepostProjectArgs {
        project: project_path,
        pre: pre_path,
        post: post_path,
        out: output_path,
    } = args;
    let pre_path = result_document_path(pre_path);
    let post_path = result_document_path(post_path);
    let pre_source = source_artifact(&pre_path, MARKED_RESULT_KIND)?;
    let post_source = source_artifact(&post_path, MARKED_RESULT_KIND)?;
    let pre = MarkedResultImportNode::new("marked-pre-import", pre_path, pre_source.clone())?;
    let post = MarkedResultImportNode::new("marked-post-import", post_path, post_source.clone())?;

    let mut session = Session::open(
        &project_path,
        &[pre_source, post_source],
        MAXIMUM_RESULT_BYTES,
    )?;
    let import_graph =
        WorkflowGraph::new([pre.spec().clone(), post.spec().clone()]).map_err(invalid)?;
    let import_schema = ("marklab.marked_pattern_result", 1);
    let pre_run = session.run(&import_graph, &pre, import_schema)?;
    let post_run = session.run(&import_graph, &post, import_schema)?;
    let comparison = MarkedPrePostNode::new(
        NodeId::new("marked-prepost").map_err(invalid)?,
        pre.spec().id().clone(),
        &pre_run,
        post.spec().id().clone(),
        &post_run,
    )
    .map_err(invalid)?;
    let graph = WorkflowGraph::new([
        pre.spec().clone(),
        post.spec().clone(),
        comparison.spec().clone(),
    ])
    .map_err(invalid)?;
    let comparison_run = session.run(&graph, &comparison, ("marklab.marked_prepost_result", 1))?;

    bayes::publish_json(
        &output_path,
        &ResultDocument::marked_prepost(comparison_run.output),
    )?;
    eprintln!(
        "project marked-prepost cache_status: pre={} post={} comparison={}",
        cache_status_name(pre_run.cache_status),
        cache_status_name(post_run.cache_status),
        cache_status_name(comparison_run.cache_status)
    );
    Ok(())
}

fn result_document_path(path: PathBuf) -> PathBuf {
    if path.is_dir() {
        path.join("result.json")
    } else {
        path
    }
}

struct MarkedResultImportNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    output: MarkedPatternResult,
}

impl MarkedResultImportNode {
    fn new(node_id: &str, input_path: PathBuf, input: ArtifactRef) -> Result<Self, CliError> {
        let output = read_marked_result(&input_path)?;
        let observed = source_artifact(&input_path, MARKED_RESULT_KIND)?;
        if observed != input {
            return Err(CliError::Input(format!(
                "marked result changed while its durable import was prepared: {}",
                input_path.display()
            )));
        }
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(node_id).map_err(|error| CliError::Input(error.to_string()))?,
                "marked_result_import",
                1,
                Vec::new(),
            )
            .map_err(|error| CliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            output,
        })
    }
}

impl WorkflowNode for MarkedResultImportNode {
    type Output = MarkedPatternResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed =
            source_artifact(&self.input_path, MARKED_RESULT_KIND).map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(CliError::Input(format!(
                "marked result no longer matches the one recorded in the project: {}",
                self.input_path.display()
            ))));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_bytes(
                b"marklab-marked-result-import-configuration-v1",
            ),
            execution_policy: b"bounded-typed-result-import-v1",
            implementation_identity: "marklab-project-marked-result-import-v2",
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        Ok(self.output.clone())
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        ResultDocument::marked(output.clone())
            .to_json_pretty()
            .map(String::into_bytes)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let json = std::str::from_utf8(bytes).map_err(NodeError::decode)?;
        ResultDocument::from_json(json)
            .and_then(ResultDocument::into_marked_pattern)
            .map_err(NodeError::decode)
    }

    fn output_kind(&self) -> &'static str {
        MARKED_RESULT_KIND
    }
}

fn read_marked_result(path: &Path) -> Result<MarkedPatternResult, CliError> {
    let bytes = crate::input_file::read_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        &format!(
            "marked result must be a regular file within {MAXIMUM_INPUT_BYTES} bytes: {}",
            path.display()
        ),
    )?;
    let json = std::str::from_utf8(&bytes)
        .map_err(|error| CliError::Input(format!("invalid result UTF-8: {error}")))?;
    ResultDocument::from_json(json)
        .and_then(ResultDocument::into_marked_pattern)
        .map_err(|error| CliError::Input(error.to_string()))
}

fn run_normal_mean(args: NormalMeanProjectArgs) -> Result<(), CliError> {
    let NormalMeanProjectArgs {
        project: project_path,
        input: input_path,
        prior_mean,
        prior_sd,
        known_sigma,
        nuts,
        timeout_seconds,
        out: output_path,
    } = args;
    let sampling = nuts.spec();
    let backend = StaticBackendWorkflow::PymcNormalMean.descriptor();
    let source = Source::identify(input_path.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_normal_mean(
        input_path,
        prior_mean,
        prior_sd,
        known_sigma,
        sampling,
        timeout_seconds,
    )?;
    source.ensure_unchanged("normal-mean input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "normal-mean input",
        vec![source],
        prepared,
        |prepared| bayes::execute_normal_mean(prepared).map_err(NodeError::execution),
        |prepared, output: &WorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_fit(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish("normal-mean", &output_path, &fit, run.cache_status)
}

fn run_hierarchical_normal(args: HierarchicalNormalProjectArgs) -> Result<(), CliError> {
    let HierarchicalNormalProjectArgs {
        model,
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcHierarchicalNormal.descriptor();
    let source = Source::identify(input_path.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_hierarchical(input_path, model)?;
    source.ensure_unchanged("hierarchical-normal input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "hierarchical-normal input",
        vec![source],
        prepared,
        |prepared| bayes::execute_hierarchical(prepared).map_err(NodeError::execution),
        |prepared, output: &HierarchicalWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_fit(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish("hierarchical-normal", &output_path, &fit, run.cache_status)
}

fn run_beta_binomial_hierarchy(args: BetaBinomialHierarchyProjectArgs) -> Result<(), CliError> {
    let BetaBinomialHierarchyProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcBetaBinomialHierarchy.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_beta_binomial_hierarchy(model)?;
    source.ensure_unchanged("beta-binomial-hierarchy input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "beta-binomial-hierarchy input",
        vec![source],
        prepared,
        |prepared| bayes::execute_beta_binomial_hierarchy(prepared).map_err(NodeError::execution),
        |prepared, output: &BetaBinomialHierarchyWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish(
        "beta-binomial-hierarchy",
        &output_path,
        &fit,
        run.cache_status,
    )
}

fn run_beta_binomial_group_regression(
    args: BetaBinomialGroupRegressionProjectArgs,
) -> Result<(), CliError> {
    let BetaBinomialGroupRegressionProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcBetaBinomialGroupRegression.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_beta_binomial_group_regression(model)?;
    source.ensure_unchanged("beta-binomial-group-regression input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "beta-binomial-group-regression input",
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_beta_binomial_group_regression(prepared).map_err(NodeError::execution)
        },
        |prepared, output: &BetaBinomialGroupRegressionWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish(
        "beta-binomial-group-regression",
        &output_path,
        &fit,
        run.cache_status,
    )
}

fn run_dirichlet_multinomial_group(
    args: DirichletMultinomialGroupProjectArgs,
) -> Result<(), CliError> {
    let DirichletMultinomialGroupProjectArgs {
        project: project_path,
        model,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcDirichletMultinomialGroup.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_dirichlet_multinomial_group(model)?;
    source.ensure_unchanged("dirichlet-multinomial-group input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "dirichlet-multinomial-group input",
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_dirichlet_multinomial_group(prepared).map_err(NodeError::execution)
        },
        |prepared, output: &DirichletMultinomialGroupWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish(
        "dirichlet-multinomial-group",
        &output_path,
        &fit,
        run.cache_status,
    )
}

fn run_ordinal_group(args: OrdinalGroupProjectArgs) -> Result<(), CliError> {
    let OrdinalGroupProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcOrdinalGroup.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_ordinal_group(model)?;
    source.ensure_unchanged("ordinal-group input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "ordinal-group input",
        vec![source],
        prepared,
        |prepared| bayes::execute_ordinal_group(prepared).map_err(NodeError::execution),
        |prepared, output: &OrdinalGroupWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish("ordinal-group", &output_path, &fit, run.cache_status)
}

fn run_ordinal_group_site_hierarchy(
    args: OrdinalGroupSiteHierarchyProjectArgs,
) -> Result<(), CliError> {
    let OrdinalGroupSiteHierarchyProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcOrdinalGroupSiteHierarchy.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_ordinal_group_site_hierarchy(model)?;
    source.ensure_unchanged("ordinal-group-site-hierarchy input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "ordinal-group-site-hierarchy input",
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_ordinal_group_site_hierarchy(prepared).map_err(NodeError::execution)
        },
        |prepared, output: &OrdinalGroupSiteHierarchyWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish(
        "ordinal-group-site-hierarchy",
        &output_path,
        &fit,
        run.cache_status,
    )
}

fn run_nonproportional_ordinal_group_site(
    args: NonproportionalOrdinalGroupSiteProjectArgs,
) -> Result<(), CliError> {
    let NonproportionalOrdinalGroupSiteProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcNonproportionalOrdinalGroupSite.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_nonproportional_ordinal_group_site(model)?;
    source.ensure_unchanged("nonproportional ordinal input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "nonproportional ordinal input",
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_nonproportional_ordinal_group_site(prepared)
                .map_err(NodeError::execution)
        },
        |prepared, output: &NonproportionalOrdinalGroupSiteWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish(
        "nonproportional-ordinal-group-site",
        &output_path,
        &fit,
        run.cache_status,
    )
}

fn run_hurdle_beta_binomial_group(
    args: HurdleBetaBinomialGroupProjectArgs,
) -> Result<(), CliError> {
    let HurdleBetaBinomialGroupProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcHurdleBetaBinomialGroup.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_hurdle_beta_binomial_group(model)?;
    source.ensure_unchanged("hurdle-beta-binomial-group input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "hurdle-beta-binomial-group input",
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_hurdle_beta_binomial_group(prepared).map_err(NodeError::execution)
        },
        |prepared, output: &HurdleBetaBinomialGroupWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish(
        "hurdle-beta-binomial-group",
        &output_path,
        &fit,
        run.cache_status,
    )
}

fn run_beta_binomial_group_gender_regression(
    args: BetaBinomialGroupGenderRegressionProjectArgs,
) -> Result<(), CliError> {
    let BetaBinomialGroupGenderRegressionProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcBetaBinomialGroupGenderRegression.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_beta_binomial_group_gender_regression(model)?;
    source.ensure_unchanged("beta-binomial-group-gender-regression input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "beta-binomial-group-gender-regression input",
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_beta_binomial_group_gender_regression(prepared)
                .map_err(NodeError::execution)
        },
        |prepared, output: &BetaBinomialGroupGenderRegressionWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish(
        "beta-binomial-group-gender-regression",
        &output_path,
        &fit,
        run.cache_status,
    )
}

fn run_beta_binomial_group_gender_slide_hierarchy(
    args: BetaBinomialGroupGenderSlideHierarchyProjectArgs,
) -> Result<(), CliError> {
    let BetaBinomialGroupGenderSlideHierarchyProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcBetaBinomialGroupGenderSlideHierarchy.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_beta_binomial_group_gender_slide_hierarchy(model)?;
    source.ensure_unchanged("beta-binomial-group-gender-slide-hierarchy input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "beta-binomial-group-gender-slide-hierarchy input",
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_beta_binomial_group_gender_slide_hierarchy(prepared)
                .map_err(NodeError::execution)
        },
        |prepared, output: &BetaBinomialGroupGenderSlideHierarchyWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish(
        "beta-binomial-group-gender-slide-hierarchy",
        &output_path,
        &fit,
        run.cache_status,
    )
}

fn run_student_t_hierarchy(args: StudentTHierarchyProjectArgs) -> Result<(), CliError> {
    let StudentTHierarchyProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcStudentTHierarchy.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_student_t_hierarchy(model)?;
    source.ensure_unchanged("student-t-hierarchy input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "student-t-hierarchy input",
        vec![source],
        prepared,
        |prepared| bayes::execute_student_t_hierarchy(prepared).map_err(NodeError::execution),
        |prepared, output: &StudentTHierarchyWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish("student-t-hierarchy", &output_path, &fit, run.cache_status)
}

fn run_gridded_lgcp(args: GriddedLgcpProjectArgs) -> Result<(), CliError> {
    let GriddedLgcpProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    const LABEL: &str = "gridded-LGCP input";
    let backend = StaticBackendWorkflow::PymcGriddedLgcp.descriptor();
    let sources = Source::identify_all(
        [model.events.clone(), model.grid.clone()],
        backend.input_kinds,
    )?;
    let prepared = bayes::prepare_gridded_lgcp(
        None,
        GriddedLgcpModelArgs {
            events: sources[0].path().to_owned(),
            grid: sources[1].path().to_owned(),
            ..model
        },
    )?;
    Source::ensure_all_unchanged(&sources, LABEL)?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        prepared,
        |prepared| bayes::execute_gridded_lgcp(prepared).map_err(NodeError::execution),
        |prepared, output: &GriddedLgcpFitWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_fit(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish("gridded-lgcp", &output_path, &fit, run.cache_status)
}

fn run_fused_gromov_wasserstein(args: FusedGromovWassersteinProjectArgs) -> Result<(), CliError> {
    let FusedGromovWassersteinProjectArgs {
        project: project_path,
        input: input_path,
        alpha,
        epsilon,
        feature_scale,
        structure_scale,
        tolerance,
        maximum_iterations,
        timeout_seconds,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PotFusedGromovWasserstein.descriptor();
    let source = Source::identify(input_path.clone(), backend.input_kinds[0])?;
    let prepared = bayes::fused_gromov::prepare(crate::bayes::fused_gromov::Parameters {
        input: input_path,
        alpha,
        epsilon,
        feature_scale,
        structure_scale,
        tolerance,
        maximum_iterations,
        timeout_seconds,
    })?;
    source.ensure_unchanged("FGW input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::ExactFloatJson,
    )?;
    let node = SourceNode::new(
        contract,
        "FGW input",
        vec![source],
        prepared,
        |prepared| bayes::fused_gromov::execute(prepared).map_err(NodeError::execution),
        |prepared, output: &FusedGromovWassersteinWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    bayes::fused_gromov::publish_result(&output_path, node.payload(), run.output)?;
    eprintln!(
        "project fused-gromov-wasserstein cache_status={}",
        cache_status_name(run.cache_status)
    );
    Ok(())
}

pub(crate) fn execute(command: ProjectCommand) -> marklab::Result<()> {
    run_cli(command).map_err(bayes::into_marklab_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backend(
        name: &'static str,
        version: &'static str,
        lock_byte: char,
        worker_byte: char,
    ) -> BackendContract {
        BackendContract {
            name,
            version,
            python_version: PYTHON_VERSION,
            environment_lock_sha256: lock_byte.to_string().repeat(64),
            worker_sha256: worker_byte.to_string().repeat(64),
        }
    }

    #[test]
    fn static_backend_identity_binds_environment_worker_and_request_bytes() {
        let descriptor = StaticBackendWorkflow::PymcNormalMean.descriptor();
        let exact = backend("pymc", "6.3.0", 'a', 'b');
        descriptor
            .validate_request_backend(&exact)
            .expect("exact descriptor");
        let baseline = descriptor.configuration_digest(&exact, b"request-a");

        let changed_lock = backend("pymc", "6.3.0", 'c', 'b');
        let changed_worker = backend("pymc", "6.3.0", 'a', 'd');
        assert_ne!(
            baseline,
            descriptor.configuration_digest(&changed_lock, b"request-a")
        );
        assert_ne!(
            baseline,
            descriptor.configuration_digest(&changed_worker, b"request-a")
        );
        assert_ne!(
            baseline,
            descriptor.configuration_digest(&exact, b"request-b")
        );

        let wrong_backend = backend("pot", "0.9.7.post1", 'a', 'b');
        assert!(descriptor.validate_request_backend(&wrong_backend).is_err());
    }
}
