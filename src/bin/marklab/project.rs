use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand, ValueEnum};
use marklab::{
    ArbitraryWindowIppLikelihoodResult, MarkedPatternResult, MarkedPrePostNode, ResultDocument,
};
use marklab_bayes::{
    BackendContract, BetaBinomialGroupGenderRegressionWorkerResult,
    BetaBinomialGroupGenderSlideHierarchyWorkerResult, BetaBinomialGroupRegressionWorkerResult,
    BetaBinomialHierarchyWorkerResult, DirichletMultinomialGroupWorkerResult,
    FusedGromovWassersteinWorkerResult, GriddedLgcpFitWorkerResult, HierarchicalWorkerResult,
    HurdleBetaBinomialGroupWorkerResult, NonproportionalOrdinalGroupSiteWorkerResult,
    NutsSamplingSpec, OrdinalGroupSiteHierarchyWorkerResult, OrdinalGroupWorkerResult,
    StudentTHierarchyWorkerResult, WorkerResult,
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
use marklab_topology::{
    WitnessPersistenceBottleneckStabilityResult, WitnessPersistenceResult,
    WitnessPersistenceStabilityResult,
};
use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, DurableRecoveryAction, LocalScheduler, MarklabProject,
    NativeRuntimeProvenance, NodeError, NodeId, NodeSpec, SchedulerLimits, WorkflowGraph,
    WorkflowNode,
};

use super::{
    bayes::{
        self, fused_gromov::PreparedFusedGromovWasserstein, ArbitraryWindowIppFitResult,
        BayesCliError, PreparedArbitraryWindowIpp, PreparedArbitraryWindowIppFit,
        PreparedBetaBinomialGroupGenderRegression, PreparedBetaBinomialGroupGenderSlideHierarchy,
        PreparedBetaBinomialGroupRegression, PreparedBetaBinomialHierarchy,
        PreparedDirichletMultinomialGroup, PreparedGaussianHierarchy, PreparedGriddedLgcpFit,
        PreparedHurdleBetaBinomialGroup, PreparedNonproportionalOrdinalGroupSite,
        PreparedNormalMean, PreparedOrdinalGroup, PreparedOrdinalGroupSiteHierarchy,
        PreparedStudentTHierarchy,
    },
    topology::{
        self, PreparedWitnessPersistence, PreparedWitnessPersistenceBottleneckStability,
        PreparedWitnessPersistenceStability, TopologyCliError,
    },
};

#[path = "project/adaptive_window_spde.rs"]
mod adaptive_window_spde;
#[path = "project/arbitrary_window_ipp_spatial_ppc.rs"]
mod arbitrary_window_ipp_spatial_ppc;
#[path = "project/causal_workflows.rs"]
mod causal_workflows;
#[path = "project/cell_patch_complementarity.rs"]
mod cell_patch_complementarity;
#[path = "project/cohort_cluster_covariate.rs"]
mod cohort_cluster_covariate;
#[path = "project/cohort_energy.rs"]
mod cohort_energy;
#[path = "project/cohort_hierarchical_bootstrap.rs"]
mod cohort_hierarchical_bootstrap;
#[path = "project/cohort_hierarchical_max_t.rs"]
mod cohort_hierarchical_max_t;
#[path = "project/cohort_margin_inference.rs"]
mod cohort_margin_inference;
#[path = "project/cohort_max_t.rs"]
mod cohort_max_t;
#[path = "project/cohort_mmd.rs"]
mod cohort_mmd;
#[path = "project/cohort_multisite_covariate.rs"]
mod cohort_multisite_covariate;
#[path = "project/correlated_multitype_lgcp.rs"]
mod correlated_multitype_lgcp;
#[path = "project/embedding_cross_covariance.rs"]
mod embedding_cross_covariance;
#[path = "project/embedding_spatial_dependence_envelope.rs"]
mod embedding_spatial_dependence_envelope;
#[path = "project/gaussian_crossed_nested_hierarchy.rs"]
mod gaussian_crossed_nested_hierarchy;
#[path = "project/graph_dirichlet_energy.rs"]
mod graph_dirichlet_energy;
#[path = "project/graph_motif_summary.rs"]
mod graph_motif_summary;
#[path = "project/graph_smoothness_permutation.rs"]
mod graph_smoothness_permutation;
#[path = "project/joint_location_embedding.rs"]
mod joint_location_embedding;
#[path = "project/joint_location_mark.rs"]
mod joint_location_mark;
#[path = "project/kernel_mark_correlation.rs"]
mod kernel_mark_correlation;
#[path = "project/local_embedding_roughness.rs"]
mod local_embedding_roughness;
#[path = "project/local_multivariate_moran.rs"]
mod local_multivariate_moran;
#[path = "project/longitudinal_kalman.rs"]
mod longitudinal_kalman;
#[path = "project/max_t_calibration.rs"]
mod max_t_calibration;
#[path = "project/multiscale_embedding_kernel.rs"]
mod multiscale_embedding_kernel;
#[path = "project/multitype_lgcp_sbc.rs"]
mod multitype_lgcp_sbc;
#[path = "project/nonstationary_adaptive_window_spde.rs"]
mod nonstationary_adaptive_window_spde;
#[path = "project/ordinal_site_heldout.rs"]
mod ordinal_site_heldout;
#[path = "project/patient_nested_fields.rs"]
mod patient_nested_fields;
#[path = "project/projected_embedding_variograms.rs"]
mod projected_embedding_variograms;
#[path = "project/region_retrieval.rs"]
mod region_retrieval;
#[path = "project/smc_abc.rs"]
mod smc_abc;
#[path = "project/spatial3d_k.rs"]
mod spatial3d_k;
#[path = "project/spatial3d_registered.rs"]
mod spatial3d_registered;
#[path = "project/spatial3d_registered_longitudinal.rs"]
mod spatial3d_registered_longitudinal;
#[path = "project/spatial3d_voxel_k.rs"]
mod spatial3d_voxel_k;
#[path = "project/spatial_varying_coefficient.rs"]
mod spatial_varying_coefficient;
#[path = "project/vector_semivariogram.rs"]
mod vector_semivariogram;

const MAXIMUM_EXECUTABLE_BYTES: u64 = 1024 * 1024 * 1024;
const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;
const PROJECT_CONTROL_BYTES: usize = 1024 * 1024;
const PROJECT_LEDGER_BYTES: usize = 16 * 1024 * 1024;
const PROJECT_LEDGER_RECORDS: usize = 10_000;
const PROJECT_RECORD_BYTES: usize = 64 * 1024;
const MAXIMUM_RESULT_BYTES: usize = 1024 * 1024;
const MAXIMUM_WITNESS_RESULT_BYTES: usize = 32 * 1024 * 1024;
const MARKED_RESULT_KIND: &str = "application/vnd.marklab.result+json;version=0.3";

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
                backend_version: "6.3.0",
                python_version: "3.12",
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
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.hierarchical-normal-observations;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-hierarchical-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_hierarchical_worker_result",
                node_id: "pymc-hierarchical-normal",
                node_kind: "bayesian_hierarchical_fit",
                implementation_identity: "marklab-project-pymc-hierarchical-normal-node-v1",
                deterministic_controls: "seeded-nuts-hierarchical-request",
            },
            Self::PymcBetaBinomialHierarchy => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.beta-binomial-patient-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-beta-binomial-hierarchy-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_beta_binomial_hierarchy_worker_result",
                node_id: "pymc-beta-binomial-hierarchy",
                node_kind: "bayesian_count_hierarchical_fit",
                implementation_identity: "marklab-project-pymc-beta-binomial-hierarchy-node-v1",
                deterministic_controls: "seeded-nuts-beta-binomial-hierarchical-request",
            },
            Self::PymcBetaBinomialGroupRegression => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.beta-binomial-patient-group-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-beta-binomial-group-regression-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_beta_binomial_group_regression_worker_result",
                node_id: "pymc-beta-binomial-group-regression",
                node_kind: "bayesian_count_group_regression_fit",
                implementation_identity:
                    "marklab-project-pymc-beta-binomial-group-regression-node-v1",
                deterministic_controls: "seeded-nuts-beta-binomial-group-regression-request",
            },
            Self::PymcDirichletMultinomialGroup => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.dirichlet-multinomial-patient-group-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-dirichlet-multinomial-group-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_dirichlet_multinomial_group_worker_result",
                node_id: "pymc-dirichlet-multinomial-group",
                node_kind: "bayesian_multiclass_count_group_regression_fit",
                implementation_identity:
                    "marklab-project-pymc-dirichlet-multinomial-group-node-v1",
                deterministic_controls: "seeded-nuts-dirichlet-multinomial-group-request",
            },
            Self::PymcOrdinalGroup => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
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
                backend_version: "6.3.0",
                python_version: "3.12",
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
                backend_id: "pymc", backend_version: "6.3.0", python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &["application/vnd.marklab.source.ordinal-patient-group-site-outcomes;version=1"],
                output_kind: "application/vnd.marklab.pymc-ordinal-group-site-hierarchy-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_ordinal_group_site_hierarchy_worker_result",
                node_id: "pymc-ordinal-group-site-hierarchy", node_kind: "bayesian_ordinal_site_hierarchy_fit",
                implementation_identity: "marklab-project-pymc-ordinal-group-site-hierarchy-node-v1",
                deterministic_controls: "seeded-nuts-ordinal-group-site-hierarchy-request",
            },
            Self::PymcNonproportionalOrdinalGroupSite => StaticBackendDescriptor {
                backend_id:"pymc",backend_version:"6.3.0",python_version:"3.12",license:"Apache-2.0",
                input_kinds:&["application/vnd.marklab.source.nonproportional-ordinal-patient-group-site-outcomes;version=1"],
                output_kind:"application/vnd.marklab.pymc-nonproportional-ordinal-group-site-worker-result+json;version=1",
                result_schema_id:"marklab.pymc_nonproportional_ordinal_group_site_worker_result",
                node_id:"pymc-nonproportional-ordinal-group-site",node_kind:"bayesian_nonproportional_ordinal_site_fit",
                implementation_identity:"marklab-project-pymc-nonproportional-ordinal-group-site-node-v1",
                deterministic_controls:"seeded-nuts-nonproportional-ordinal-group-site-request",
            },
            Self::PymcBetaBinomialGroupGenderRegression => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.beta-binomial-patient-group-gender-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-beta-binomial-group-gender-regression-worker-result+json;version=1",
                result_schema_id:
                    "marklab.pymc_beta_binomial_group_gender_regression_worker_result",
                node_id: "pymc-beta-binomial-group-gender-regression",
                node_kind: "bayesian_count_group_gender_regression_fit",
                implementation_identity:
                    "marklab-project-pymc-beta-binomial-group-gender-regression-node-v1",
                deterministic_controls:
                    "seeded-nuts-beta-binomial-group-gender-regression-request",
            },
            Self::PymcBetaBinomialGroupGenderSlideHierarchy => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.beta-binomial-group-gender-slide-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-beta-binomial-group-gender-slide-hierarchy-worker-result+json;version=1",
                result_schema_id:
                    "marklab.pymc_beta_binomial_group_gender_slide_hierarchy_worker_result",
                node_id: "pymc-beta-binomial-group-gender-slide-hierarchy",
                node_kind: "bayesian_repeated_slide_patient_hierarchical_fit",
                implementation_identity:
                    "marklab-project-pymc-beta-binomial-group-gender-slide-hierarchy-node-v1",
                deterministic_controls:
                    "seeded-nuts-beta-binomial-group-gender-slide-hierarchy-request",
            },
            Self::PymcStudentTHierarchy => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.student-t-hierarchy-observations;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-student-t-hierarchy-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_student_t_hierarchy_worker_result",
                node_id: "pymc-student-t-hierarchy",
                node_kind: "bayesian_robust_hierarchical_fit",
                implementation_identity: "marklab-project-pymc-student-t-hierarchy-node-v1",
                deterministic_controls: "seeded-nuts-student-t-hierarchical-request",
            },
            Self::PymcGriddedLgcp => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.point-events;version=1",
                    "application/vnd.marklab.source.gridded-covariates;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-gridded-lgcp-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_gridded_lgcp_worker_result",
                node_id: "pymc-gridded-lgcp",
                node_kind: "bayesian_point_process_field_fit",
                implementation_identity: "marklab-project-pymc-gridded-lgcp-node-v1",
                deterministic_controls: "seeded-nuts-fixed-grid-lgcp-request",
            },
            Self::PymcArbitraryWindowIpp => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
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
                backend_version: "6.3.0",
                python_version: "3.12",
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
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.arbitrary-window-lgcp-events+csv;version=1",
                    "application/vnd.marklab.source.arbitrary-window-lgcp-membership+csv;version=1",
                    "application/vnd.marklab.source.arbitrary-window-lgcp-quadrature+csv;version=1",
                    "application/vnd.marklab.source.observation-window+geojson;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-arbitrary-window-lgcp-fit+json;version=1",
                result_schema_id: "marklab.pymc_arbitrary_window_lgcp_fit_result",
                node_id: "pymc-arbitrary-window-lgcp-fit",
                node_kind: "bayesian_point_process_latent_field_fit",
                implementation_identity: "marklab-project-pymc-arbitrary-window-lgcp-fit-node-v1",
                deterministic_controls:
                    "seeded-nuts-weighted-exact-window-fixed-matern-lgcp-request",
            },
            Self::PymcReplicatedArbitraryWindowLgcp => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.replicated-arbitrary-window-lgcp+csv;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-replicated-arbitrary-window-lgcp-fit+json;version=2",
                result_schema_id:
                    "marklab.pymc_replicated_arbitrary_window_lgcp_fit_result_v2",
                node_id: "pymc-replicated-arbitrary-window-lgcp-fit",
                node_kind: "bayesian_replicated_point_process_latent_field_fit",
                implementation_identity:
                    "marklab-project-pymc-replicated-arbitrary-window-lgcp-fit-node-v2",
                deterministic_controls:
                    "seeded-nuts-patient-pattern-hierarchy-fixed-matern-request",
            },
            Self::PymcReplicatedArbitraryWindowLgcpInferredKernel => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.replicated-arbitrary-window-lgcp+csv;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-replicated-arbitrary-window-lgcp-inferred-kernel-fit+json;version=1",
                result_schema_id:
                    "marklab.pymc_replicated_arbitrary_window_lgcp_inferred_kernel_fit_result",
                node_id: "pymc-replicated-arbitrary-window-lgcp-inferred-kernel-fit",
                node_kind: "bayesian_replicated_point_process_inferred_latent_field_fit",
                implementation_identity:
                    "marklab-project-pymc-replicated-arbitrary-window-lgcp-inferred-kernel-fit-node-v1",
                deterministic_controls:
                    "seeded-nuts-patient-pattern-hierarchy-inferred-shared-matern-request",
            },
            Self::PymcReplicatedArbitraryWindowMultitypeLgcpInferredKernel => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.replicated-arbitrary-window-multitype-lgcp+csv;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-replicated-arbitrary-window-multitype-lgcp-inferred-kernel-fit+json;version=1",
                result_schema_id:
                    "marklab.pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_fit_result",
                node_id: "pymc-replicated-arbitrary-window-multitype-lgcp-inferred-kernel-fit",
                node_kind: "bayesian_replicated_multitype_point_process_inferred_latent_field_fit",
                implementation_identity:
                    "marklab-project-pymc-replicated-arbitrary-window-multitype-lgcp-inferred-kernel-fit-node-v1",
                deterministic_controls:
                    "seeded-nuts-patient-pattern-type-hierarchy-inferred-shared-matern-request",
            },
            Self::PymcReplicatedArbitraryWindowMultitypeLgcp => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.replicated-arbitrary-window-multitype-lgcp+csv;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-replicated-arbitrary-window-multitype-lgcp-fit+json;version=1",
                result_schema_id:
                    "marklab.pymc_replicated_arbitrary_window_multitype_lgcp_fit_result",
                node_id: "pymc-replicated-arbitrary-window-multitype-lgcp-fit",
                node_kind: "bayesian_replicated_multitype_point_process_latent_field_fit",
                implementation_identity:
                    "marklab-project-pymc-replicated-arbitrary-window-multitype-lgcp-fit-node-v1",
                deterministic_controls:
                    "seeded-nuts-patient-pattern-type-hierarchy-fixed-matern-request",
            },
            Self::PymcConditionalMultitypeMark => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.conditional-multitype-mark+csv;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-conditional-multitype-mark-fit+json;version=1",
                result_schema_id: "marklab.conditional_multitype_mark_fit",
                node_id: "pymc-conditional-multitype-mark-fit",
                node_kind: "bayesian_conditional_multitype_mark_fit",
                implementation_identity:
                    "marklab-project-pymc-conditional-multitype-mark-fit-node-v1",
                deterministic_controls:
                    "seeded-nuts-fixed-location-radius-graph-conditional-mark-request",
            },
            Self::PymcReplicatedConditionalMultitypeMark => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.replicated-conditional-multitype-mark+csv;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-replicated-conditional-multitype-mark-fit+json;version=1",
                result_schema_id: "marklab.replicated_conditional_multitype_mark_fit",
                node_id: "pymc-replicated-conditional-multitype-mark-fit",
                node_kind: "bayesian_patient_replicated_conditional_multitype_mark_fit",
                implementation_identity:
                    "marklab-project-pymc-replicated-conditional-multitype-mark-fit-node-v2",
                deterministic_controls:
                    "seeded-nuts-patient-pattern-fixed-location-conditional-mark-request",
            },
            Self::PotFusedGromovWasserstein => StaticBackendDescriptor {
                backend_id: "pot",
                backend_version: "0.9.7.post1",
                python_version: "3.12",
                license: "MIT",
                input_kinds: &["application/vnd.marklab.source.fgw-input+json;version=1"],
                output_kind:
                    "application/vnd.marklab.pot-fused-gromov-worker-result+json;version=1",
                result_schema_id: "marklab.pot_fused_gromov_wasserstein_worker_result",
                node_id: "pot-fused-gromov-wasserstein",
                node_kind: "descriptive_alignment",
                implementation_identity: "marklab-project-pot-fused-gromov-node-v1",
                deterministic_controls: "fixed-initialization-order-and-solver-controls",
            },
        }
    }
}

impl StaticBackendDescriptor {
    fn validate_request_backend(self, backend: &BackendContract) -> Result<(), BayesCliError> {
        if backend.name != self.backend_id
            || backend.version != self.backend_version
            || backend.python_version != self.python_version
            || !is_sha256(&backend.environment_lock_sha256)
            || !is_sha256(&backend.worker_sha256)
        {
            return Err(BayesCliError::Input(format!(
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

    fn result_schema(self) -> Result<ArtifactSchema, BayesCliError> {
        ArtifactSchema::new(self.result_schema_id, 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct ProjectCli {
    #[command(subcommand)]
    command: ProjectTopLevel,
}

#[derive(Debug, Subcommand)]
enum ProjectTopLevel {
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
}

#[derive(Debug, clap::Args)]
struct ArbitraryWindowIppProjectArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    events: PathBuf,
    #[arg(long)]
    quadrature: PathBuf,
    #[arg(long)]
    window: PathBuf,
    #[arg(long, allow_hyphen_values = true)]
    intercept: f64,
    #[arg(long, allow_hyphen_values = true)]
    coefficient: f64,
    #[arg(long)]
    maximum_events: usize,
    #[arg(long)]
    maximum_quadrature_nodes: usize,
    #[arg(long)]
    maximum_work: usize,
    #[arg(long, default_value_t = 16 * 1024 * 1024)]
    maximum_retained_bytes: usize,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, clap::Args)]
struct ArbitraryWindowIppFitProjectArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    events: PathBuf,
    #[arg(long)]
    quadrature: PathBuf,
    #[arg(long)]
    window: PathBuf,
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long, allow_hyphen_values = true)]
    coefficient_prior_mean: f64,
    #[arg(long)]
    coefficient_prior_sd: f64,
    #[arg(long)]
    chains: u32,
    #[arg(long)]
    tune: u32,
    #[arg(long)]
    draws: u32,
    #[arg(long)]
    target_accept: f64,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    maximum_events: usize,
    #[arg(long)]
    maximum_quadrature_nodes: usize,
    #[arg(long)]
    maximum_draw_node_work: u64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, clap::Args)]
struct ArbitraryWindowIppSpatialPpcProjectArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    events: PathBuf,
    #[arg(long)]
    event_membership: PathBuf,
    #[arg(long)]
    quadrature: PathBuf,
    #[arg(long)]
    window: PathBuf,
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long, allow_hyphen_values = true)]
    coefficient_prior_mean: f64,
    #[arg(long)]
    coefficient_prior_sd: f64,
    #[arg(long)]
    chains: u32,
    #[arg(long)]
    tune: u32,
    #[arg(long)]
    draws: u32,
    #[arg(long)]
    target_accept: f64,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    prediction_seed: u64,
    #[arg(long)]
    neighbor_radius_um: f64,
    #[arg(long)]
    maximum_events: usize,
    #[arg(long)]
    maximum_quadrature_nodes: usize,
    #[arg(long)]
    maximum_neighbor_pairs: usize,
    #[arg(long)]
    maximum_draw_node_work: u64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, clap::Args)]
struct ArbitraryWindowLgcpFitProjectArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    events: PathBuf,
    #[arg(long)]
    event_membership: PathBuf,
    #[arg(long)]
    quadrature: PathBuf,
    #[arg(long)]
    window: PathBuf,
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long, allow_hyphen_values = true)]
    coefficient_prior_mean: f64,
    #[arg(long)]
    coefficient_prior_sd: f64,
    #[arg(long)]
    field_amplitude: f64,
    #[arg(long)]
    field_length_scale_um: f64,
    #[arg(long)]
    jitter: f64,
    #[arg(long)]
    chains: u32,
    #[arg(long)]
    tune: u32,
    #[arg(long)]
    draws: u32,
    #[arg(long)]
    target_accept: f64,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    maximum_events: usize,
    #[arg(long)]
    maximum_quadrature_nodes: usize,
    #[arg(long)]
    maximum_draw_node_work: u64,
    #[arg(long)]
    prediction_replicates: u32,
    #[arg(long)]
    prediction_seed: u64,
    #[arg(long)]
    maximum_predictive_points: u64,
    #[arg(long)]
    neighbor_radius_um: f64,
    #[arg(long)]
    maximum_neighbor_pairs: usize,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, clap::Args)]
struct ReplicatedArbitraryWindowLgcpFitProjectArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    reference_group: String,
    #[arg(long)]
    comparison_group: String,
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long)]
    group_effect_prior_sd: f64,
    #[arg(long)]
    covariate_effect_prior_sd: f64,
    #[arg(long)]
    patient_sd_prior_scale: f64,
    #[arg(long)]
    pattern_sd_prior_scale: f64,
    #[arg(long)]
    field_amplitude: f64,
    #[arg(long)]
    field_length_scale_um: f64,
    #[arg(long)]
    jitter: f64,
    #[arg(long)]
    chains: u32,
    #[arg(long)]
    tune: u32,
    #[arg(long)]
    draws: u32,
    #[arg(long)]
    target_accept: f64,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    maximum_patients: usize,
    #[arg(long)]
    maximum_patterns: usize,
    #[arg(long)]
    maximum_nodes_per_pattern: usize,
    #[arg(long)]
    maximum_total_nodes: usize,
    #[arg(long)]
    maximum_total_events: u64,
    #[arg(long)]
    maximum_draw_node_work: u64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, clap::Args)]
struct ReplicatedArbitraryWindowLgcpInferredKernelProjectArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    reference_group: String,
    #[arg(long)]
    comparison_group: String,
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long)]
    group_effect_prior_sd: f64,
    #[arg(long)]
    covariate_effect_prior_sd: f64,
    #[arg(long)]
    patient_sd_prior_scale: f64,
    #[arg(long)]
    pattern_sd_prior_scale: f64,
    #[arg(long)]
    field_amplitude_prior_scale: f64,
    #[arg(long)]
    field_length_scale_prior_scale_um: f64,
    #[arg(long)]
    jitter: f64,
    #[arg(long)]
    chains: u32,
    #[arg(long)]
    tune: u32,
    #[arg(long)]
    draws: u32,
    #[arg(long)]
    target_accept: f64,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    maximum_patients: usize,
    #[arg(long)]
    maximum_patterns: usize,
    #[arg(long)]
    maximum_nodes_per_pattern: usize,
    #[arg(long)]
    maximum_total_nodes: usize,
    #[arg(long)]
    maximum_total_events: u64,
    #[arg(long)]
    maximum_draw_node_work: u64,
    #[arg(long)]
    maximum_kernel_cube_work: u64,
    #[arg(long)]
    maximum_tree_depth: u32,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, clap::Args)]
struct ReplicatedArbitraryWindowMultitypeLgcpProjectArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    reference_group: String,
    #[arg(long)]
    comparison_group: String,
    #[arg(long)]
    reference_type: String,
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long)]
    group_effect_prior_sd: f64,
    #[arg(long)]
    covariate_effect_prior_sd: f64,
    #[arg(long)]
    patient_sd_prior_scale: f64,
    #[arg(long)]
    pattern_sd_prior_scale: f64,
    #[arg(long)]
    field_amplitude: f64,
    #[arg(long)]
    field_length_scale_um: f64,
    #[arg(long)]
    jitter: f64,
    #[arg(long)]
    chains: u32,
    #[arg(long)]
    tune: u32,
    #[arg(long)]
    draws: u32,
    #[arg(long)]
    target_accept: f64,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    maximum_patients: usize,
    #[arg(long)]
    maximum_patterns: usize,
    #[arg(long)]
    maximum_types: usize,
    #[arg(long)]
    maximum_nodes_per_pattern: usize,
    #[arg(long)]
    maximum_total_nodes: usize,
    #[arg(long)]
    maximum_total_node_type_rows: usize,
    #[arg(long)]
    maximum_total_events: u64,
    #[arg(long)]
    maximum_draw_node_type_work: u64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, clap::Args)]
struct ReplicatedArbitraryWindowMultitypeLgcpInferredKernelProjectArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    reference_group: String,
    #[arg(long)]
    comparison_group: String,
    #[arg(long)]
    reference_type: String,
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long)]
    group_effect_prior_sd: f64,
    #[arg(long)]
    covariate_effect_prior_sd: f64,
    #[arg(long)]
    patient_sd_prior_scale: f64,
    #[arg(long)]
    pattern_sd_prior_scale: f64,
    #[arg(long)]
    field_amplitude_prior_scale: f64,
    #[arg(long)]
    field_length_scale_prior_scale_um: f64,
    #[arg(long)]
    jitter: f64,
    #[arg(long)]
    chains: u32,
    #[arg(long)]
    tune: u32,
    #[arg(long)]
    draws: u32,
    #[arg(long)]
    target_accept: f64,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    maximum_patients: usize,
    #[arg(long)]
    maximum_patterns: usize,
    #[arg(long)]
    maximum_types: usize,
    #[arg(long)]
    maximum_nodes_per_pattern: usize,
    #[arg(long)]
    maximum_total_nodes: usize,
    #[arg(long)]
    maximum_total_node_type_rows: usize,
    #[arg(long)]
    maximum_total_events: u64,
    #[arg(long)]
    maximum_draw_node_type_work: u64,
    #[arg(long)]
    maximum_kernel_cube_work: u64,
    #[arg(long)]
    maximum_tree_depth: u32,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, clap::Args)]
struct ConditionalMultitypeMarkProjectArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    reference_type: String,
    #[arg(long)]
    radius_um: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long)]
    interaction_prior_sd: f64,
    #[arg(long)]
    chains: u32,
    #[arg(long)]
    tune: u32,
    #[arg(long)]
    draws: u32,
    #[arg(long)]
    target_accept: f64,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    maximum_points: usize,
    #[arg(long)]
    maximum_types: usize,
    #[arg(long)]
    maximum_neighbor_visits: u64,
    #[arg(long)]
    maximum_edges: usize,
    #[arg(long)]
    maximum_draw_parameter_work: u64,
    #[arg(long)]
    maximum_working_bytes: usize,
    #[arg(long)]
    maximum_tree_depth: u32,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, clap::Args)]
struct SmcAbcGrowthFrontProjectArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    diffusion_um2_per_time: f64,
    #[arg(long)]
    carrying_capacity: f64,
    #[arg(long)]
    final_time: f64,
    #[arg(long)]
    time_step: f64,
    #[arg(long)]
    front_threshold_fraction: f64,
    #[arg(long)]
    observed_final_mass: f64,
    #[arg(long)]
    mass_scale: f64,
    #[arg(long)]
    growth_rate_prior_min: f64,
    #[arg(long)]
    growth_rate_prior_max: f64,
    #[arg(long, value_delimiter = ',')]
    epsilon_schedule: Vec<f64>,
    #[arg(long)]
    particles: u32,
    #[arg(long)]
    maximum_proposals_per_stage: u32,
    #[arg(long)]
    maximum_cell_steps_per_proposal: u64,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, clap::Args)]
struct ReplicatedConditionalMultitypeMarkProjectArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    reference_group: String,
    #[arg(long)]
    reference_type: String,
    #[arg(long)]
    radius_um: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long)]
    interaction_prior_sd: f64,
    #[arg(long)]
    group_effect_prior_sd: f64,
    #[arg(long)]
    patient_sd_prior_scale: f64,
    #[arg(long)]
    pattern_sd_prior_scale: f64,
    #[arg(long)]
    chains: u32,
    #[arg(long)]
    tune: u32,
    #[arg(long)]
    draws: u32,
    #[arg(long)]
    target_accept: f64,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    maximum_patients: usize,
    #[arg(long)]
    maximum_patterns: usize,
    #[arg(long)]
    maximum_points: usize,
    #[arg(long)]
    maximum_types: usize,
    #[arg(long)]
    maximum_neighbor_visits: u64,
    #[arg(long)]
    maximum_edges: usize,
    #[arg(long)]
    maximum_draw_parameter_work: u64,
    #[arg(long)]
    maximum_working_bytes: usize,
    #[arg(long)]
    maximum_tree_depth: u32,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ProjectNoninferiorityDirection {
    HigherIsBetter,
    LowerIsBetter,
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    TestCellPatchComplementarity {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        outer_folds: u32,
        #[arg(long)]
        inner_folds: u32,
        #[arg(long)]
        ridge_alphas: String,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    CohortEnergy {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        group_a: String,
        #[arg(long)]
        group_b: String,
        #[arg(long)]
        metric: String,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        maximum_patients: usize,
        #[arg(long)]
        maximum_features: usize,
        #[arg(long)]
        maximum_distance_elements: u64,
        #[arg(long)]
        maximum_energy_evaluations: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    CausalRandomizedInterference {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    GaussianEig {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    CohortClusterCovariatePermutation {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        group_a: String,
        #[arg(long)]
        group_b: String,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long, value_enum)]
        alternative: super::cohort::CliAlternative,
        #[arg(long)]
        maximum_patients: usize,
        #[arg(long)]
        maximum_clusters: usize,
        #[arg(long)]
        maximum_covariates: usize,
        #[arg(long)]
        maximum_patient_covariate_cells: u64,
        #[arg(long)]
        maximum_ols_work: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    CohortHierarchicalMaxT {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        group_a: String,
        #[arg(long)]
        group_b: String,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        step_down: bool,
        #[arg(long)]
        maximum_patients: usize,
        #[arg(long)]
        maximum_families: usize,
        #[arg(long)]
        maximum_endpoints: usize,
        #[arg(long)]
        maximum_cells: u64,
        #[arg(long)]
        maximum_permutation_endpoint_evaluations: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    CohortHierarchicalBootstrap {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        replicates: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        maximum_patients: usize,
        #[arg(long)]
        maximum_specimens: usize,
        #[arg(long)]
        maximum_bootstrap_draws: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    CohortEquivalence {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        lower_margin: f64,
        #[arg(long, allow_hyphen_values = true)]
        upper_margin: f64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        margin_rationale: String,
        #[arg(long)]
        out: PathBuf,
    },
    CohortNoninferiority {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long, value_enum)]
        direction: ProjectNoninferiorityDirection,
        #[arg(long)]
        margin: f64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        margin_rationale: String,
        #[arg(long)]
        out: PathBuf,
    },
    CohortMaxT {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        group_a: String,
        #[arg(long)]
        group_b: String,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        step_down: bool,
        #[arg(long)]
        maximum_patients: usize,
        #[arg(long)]
        maximum_endpoints: usize,
        #[arg(long)]
        maximum_patient_endpoint_cells: u64,
        #[arg(long)]
        maximum_permutation_endpoint_evaluations: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    PatientNestedFields {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        group_a: String,
        #[arg(long)]
        group_b: String,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        maximum_patients: usize,
        #[arg(long)]
        maximum_specimens: usize,
        #[arg(long)]
        maximum_endpoints: usize,
        #[arg(long)]
        maximum_permutation_endpoint_evaluations: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    MaxTCalibration {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        group_a_count: usize,
        #[arg(long, value_delimiter = ',')]
        family_sizes: Vec<usize>,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        maximum_assignments: usize,
        #[arg(long)]
        maximum_assignment_endpoint_evaluations: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    CohortMmd {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        group_a: String,
        #[arg(long)]
        group_b: String,
        #[arg(long)]
        kernel: String,
        #[arg(long)]
        bandwidth: Option<f64>,
        #[arg(long)]
        estimator: String,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        maximum_patients: usize,
        #[arg(long)]
        maximum_features: usize,
        #[arg(long)]
        maximum_kernel_elements: u64,
        #[arg(long)]
        maximum_mmd_evaluations: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    CohortMultisiteCovariateContrast {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        group_a: String,
        #[arg(long)]
        group_b: String,
        #[arg(long, value_enum)]
        model: super::cohort::CliMultisiteModel,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        maximum_patients: usize,
        #[arg(long)]
        maximum_sites: usize,
        #[arg(long)]
        maximum_covariates: usize,
        #[arg(long)]
        maximum_patient_covariate_cells: u64,
        #[arg(long)]
        maximum_ols_work: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    ProjectedEmbeddingVariograms {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        bins: PathBuf,
        #[arg(long)]
        components: u32,
        #[arg(long)]
        permutations: u32,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        maximum_pair_visits: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    VectorSemivariogram {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        bins: PathBuf,
        #[arg(long)]
        weights: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        maximum_points: usize,
        #[arg(long)]
        maximum_dimension: usize,
        #[arg(long)]
        maximum_pair_visits: u64,
        #[arg(long)]
        memory_budget_mib: usize,
    },
    LocalMultivariateMoran {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        window: PathBuf,
        #[arg(long)]
        radius_um: f64,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        maximum_points: usize,
        #[arg(long)]
        maximum_dimension: usize,
        #[arg(long)]
        maximum_directed_edges: usize,
        #[arg(long)]
        maximum_permutation_edge_evaluations: usize,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    EmbeddingCrossCovarianceByDistance {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        bins: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        maximum_points: usize,
        #[arg(long)]
        maximum_dimension: usize,
        #[arg(long)]
        maximum_pair_visits: u64,
        #[arg(long)]
        maximum_matrix_elements: u64,
        #[arg(long)]
        memory_budget_mib: usize,
    },
    KernelMarkCorrelation {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        bins: PathBuf,
        #[arg(long)]
        kernel: String,
        #[arg(long)]
        global_reference_tolerance: f64,
        #[arg(long)]
        maximum_points: usize,
        #[arg(long)]
        maximum_dimension: usize,
        #[arg(long)]
        maximum_pair_visits: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    EmbeddingSpatialDependenceEnvelope {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        bins: PathBuf,
        #[arg(long)]
        curve: String,
        #[arg(long)]
        permutations: u32,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        maximum_points: usize,
        #[arg(long)]
        maximum_dimension: usize,
        #[arg(long)]
        maximum_pair_visits: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    GraphDirichletEnergy {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        nodes: PathBuf,
        #[arg(long)]
        edges: PathBuf,
        #[arg(long)]
        laplacian: String,
        #[arg(long)]
        normalization: String,
        #[arg(long)]
        maximum_nodes: usize,
        #[arg(long)]
        maximum_edges: usize,
        #[arg(long)]
        maximum_dimension: usize,
        #[arg(long)]
        maximum_component_edge_visits: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    GraphSmoothnessPermutationTest {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        nodes: PathBuf,
        #[arg(long)]
        edges: PathBuf,
        #[arg(long)]
        laplacian: String,
        #[arg(long)]
        permutations: u32,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        maximum_nodes: usize,
        #[arg(long)]
        maximum_edges: usize,
        #[arg(long)]
        maximum_dimension: usize,
        #[arg(long)]
        maximum_component_edge_visits: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    LocalEmbeddingRoughness {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        nodes: PathBuf,
        #[arg(long)]
        edges: PathBuf,
        #[arg(long)]
        epsilon: f64,
        #[arg(long)]
        maximum_nodes: usize,
        #[arg(long)]
        maximum_edges: usize,
        #[arg(long)]
        maximum_dimension: usize,
        #[arg(long)]
        maximum_component_edge_visits: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    LongitudinalKalmanSmooth {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    MultiscaleEmbeddingKernel {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        weights: PathBuf,
        #[arg(long)]
        sample_a: String,
        #[arg(long)]
        sample_b: String,
        #[arg(long)]
        base_kernel: String,
        #[arg(long)]
        kernel_scale: Option<f64>,
        #[arg(long)]
        maximum_summaries: usize,
        #[arg(long)]
        maximum_dimension: usize,
        #[arg(long)]
        maximum_component_scale_visits: u64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        out: PathBuf,
    },
    RegionRetrieval {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        training: PathBuf,
        #[arg(long)]
        query: PathBuf,
        #[arg(long)]
        k: u32,
        #[arg(long)]
        leakage_policy: String,
        #[arg(long)]
        maximum_component_candidate_visits: u64,
        #[arg(long)]
        out: PathBuf,
    },
    MarkedPrepost {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        pre: PathBuf,
        #[arg(long)]
        post: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SparseRadiusHeat {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SparseRadiusBasis {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    GraphMotifTriangleSummary {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SmcAbcGrowthFront(Box<SmcAbcGrowthFrontProjectArgs>),
    SpatialVaryingCoefficient {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        global_predictor_name: String,
        #[arg(long)]
        spatial_predictor_name: String,
        #[arg(long, allow_hyphen_values = true)]
        intercept_prior_mean: f64,
        #[arg(long)]
        intercept_prior_sd: f64,
        #[arg(long)]
        coefficient_prior_sd: f64,
        #[arg(long)]
        amplitude_prior_sd: f64,
        #[arg(long)]
        length_scale_prior_sd_um: f64,
        #[arg(long)]
        known_noise_sd: f64,
        #[arg(long)]
        jitter: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    Spatial3dKFunction {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Spatial3dVoxelKFunction {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Spatial3dRegisteredSerialVoxelK {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Spatial3dRegisteredLongitudinalVoxelKChange {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SparseRadiusFourierEnergy {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SparseRadiusHeatStability {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SparseRadiusDiffusionWavelet {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SparseRadiusScattering {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    WitnessPersistence {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    WitnessPersistenceStability {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    WitnessPersistenceBottleneckStability {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    AdaptiveWindowSpde {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    NonstationaryAdaptiveWindowSpde {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    ArbitraryWindowIppLikelihood(Box<ArbitraryWindowIppProjectArgs>),
    FitArbitraryWindowIpp(Box<ArbitraryWindowIppFitProjectArgs>),
    ArbitraryWindowIppSpatialPpc(Box<ArbitraryWindowIppSpatialPpcProjectArgs>),
    ArbitraryWindowLgcp(Box<ArbitraryWindowLgcpFitProjectArgs>),
    ReplicatedArbitraryWindowLgcp(Box<ReplicatedArbitraryWindowLgcpFitProjectArgs>),
    ReplicatedArbitraryWindowLgcpInferredKernel(
        Box<ReplicatedArbitraryWindowLgcpInferredKernelProjectArgs>,
    ),
    ReplicatedArbitraryWindowMultitypeLgcp(Box<ReplicatedArbitraryWindowMultitypeLgcpProjectArgs>),
    ReplicatedArbitraryWindowMultitypeLgcpInferredKernel(
        Box<ReplicatedArbitraryWindowMultitypeLgcpInferredKernelProjectArgs>,
    ),
    ReplicatedArbitraryWindowMultitypeLgcpInferredKernelSbc(Box<multitype_lgcp_sbc::ProjectArgs>),
    CorrelatedReplicatedArbitraryWindowMultitypeLgcp(Box<correlated_multitype_lgcp::ProjectArgs>),
    JointReplicatedLocationEmbedding(Box<joint_location_embedding::ProjectArgs>),
    JointReplicatedLocationMark(Box<joint_location_mark::ProjectArgs>),
    GaussianCrossedNestedHierarchy(Box<gaussian_crossed_nested_hierarchy::ProjectArgs>),
    ConditionalMultitypeMark(Box<ConditionalMultitypeMarkProjectArgs>),
    ReplicatedConditionalMultitypeMark(Box<ReplicatedConditionalMultitypeMarkProjectArgs>),
    NormalMean {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        prior_mean: f64,
        #[arg(long)]
        prior_sd: f64,
        #[arg(long)]
        known_sigma: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    HierarchicalNormal {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        global_prior_mean: f64,
        #[arg(long)]
        global_prior_sd: f64,
        #[arg(long)]
        between_patient_sd_prior: f64,
        #[arg(long)]
        known_sigma: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    StudentTHierarchy {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        global_prior_mean: f64,
        #[arg(long)]
        global_prior_sd: f64,
        #[arg(long)]
        between_patient_sd_prior: f64,
        #[arg(long)]
        observation_sd_prior: f64,
        #[arg(long)]
        degrees_of_freedom_excess_rate: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    BetaBinomialHierarchy {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        population_alpha: f64,
        #[arg(long)]
        population_beta: f64,
        #[arg(long)]
        concentration_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    BetaBinomialGroupRegression {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long, allow_hyphen_values = true)]
        intercept_prior_mean: f64,
        #[arg(long)]
        intercept_prior_sd: f64,
        #[arg(long)]
        group_effect_prior_sd: f64,
        #[arg(long)]
        concentration_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    DirichletMultinomialGroup {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long)]
        logit_prior_sd: f64,
        #[arg(long)]
        group_effect_prior_sd: f64,
        #[arg(long)]
        concentration_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    OrdinalGroup {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long)]
        ordered_levels: String,
        #[arg(long)]
        cutpoint_prior_sd: f64,
        #[arg(long)]
        group_effect_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    OrdinalGroupSiteHierarchy {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long)]
        ordered_levels: String,
        #[arg(long)]
        cutpoint_prior_sd: f64,
        #[arg(long)]
        group_effect_prior_sd: f64,
        #[arg(long)]
        site_intercept_sd_prior_sd: f64,
        #[arg(long)]
        site_group_slope_sd_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    NonproportionalOrdinalGroupSite {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long)]
        ordered_levels: String,
        #[arg(long)]
        cutpoint_prior_sd: f64,
        #[arg(long)]
        site_intercept_sd_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    OrdinalSiteHeldoutComparison {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long)]
        ordered_levels: String,
        #[arg(long)]
        smoothing: f64,
        #[arg(long)]
        optimizer_tolerance: f64,
        #[arg(long)]
        maximum_optimizer_evaluations: u64,
        #[arg(long)]
        out: PathBuf,
    },
    HurdleBetaBinomialGroup {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long, allow_hyphen_values = true)]
        presence_intercept_prior_mean: f64,
        #[arg(long)]
        presence_intercept_prior_sd: f64,
        #[arg(long)]
        presence_group_effect_prior_sd: f64,
        #[arg(long, allow_hyphen_values = true)]
        abundance_intercept_prior_mean: f64,
        #[arg(long)]
        abundance_intercept_prior_sd: f64,
        #[arg(long)]
        abundance_group_effect_prior_sd: f64,
        #[arg(long)]
        concentration_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    BetaBinomialGroupGenderRegression {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long)]
        reference_gender: String,
        #[arg(long)]
        comparison_gender: String,
        #[arg(long, allow_hyphen_values = true)]
        intercept_prior_mean: f64,
        #[arg(long)]
        intercept_prior_sd: f64,
        #[arg(long)]
        group_effect_prior_sd: f64,
        #[arg(long)]
        gender_effect_prior_sd: f64,
        #[arg(long)]
        concentration_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    BetaBinomialGroupGenderSlideHierarchy {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long)]
        reference_gender: String,
        #[arg(long)]
        comparison_gender: String,
        #[arg(long, allow_hyphen_values = true)]
        intercept_prior_mean: f64,
        #[arg(long)]
        intercept_prior_sd: f64,
        #[arg(long)]
        group_effect_prior_sd: f64,
        #[arg(long)]
        gender_effect_prior_sd: f64,
        #[arg(long)]
        patient_log_odds_sd_prior_sd: f64,
        #[arg(long)]
        slide_concentration_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    GriddedLgcp {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        events: PathBuf,
        #[arg(long)]
        grid: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        xmin_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        ymin_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        xmax_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        ymax_um: f64,
        #[arg(long)]
        grid_x: u32,
        #[arg(long)]
        grid_y: u32,
        #[arg(long, allow_hyphen_values = true)]
        intercept_prior_mean: f64,
        #[arg(long)]
        intercept_prior_sd: f64,
        #[arg(long, allow_hyphen_values = true)]
        coefficient_prior_mean: f64,
        #[arg(long)]
        coefficient_prior_sd: f64,
        #[arg(long)]
        field_amplitude: f64,
        #[arg(long)]
        field_length_scale_um: f64,
        #[arg(long)]
        jitter: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    FusedGromovWasserstein {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        epsilon: f64,
        #[arg(long)]
        feature_scale: f64,
        #[arg(long)]
        structure_scale: f64,
        #[arg(long)]
        tolerance: f64,
        #[arg(long)]
        maximum_iterations: u32,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    match ProjectCli::parse_from(std::env::args_os()).command {
        ProjectTopLevel::Project {
            command:
                ProjectCommand::MaxTCalibration {
                    project,
                    input,
                    group_a_count,
                    family_sizes,
                    alpha,
                    maximum_assignments,
                    maximum_assignment_endpoint_evaluations,
                    memory_budget_mib,
                    out,
                },
        } => max_t_calibration::run(
            project,
            input,
            group_a_count,
            family_sizes,
            alpha,
            maximum_assignments,
            maximum_assignment_endpoint_evaluations,
            memory_budget_mib,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::TestCellPatchComplementarity {
                    project,
                    input,
                    outer_folds,
                    inner_folds,
                    ridge_alphas,
                    permutations,
                    seed,
                    timeout_seconds,
                    memory_budget_mib,
                    out,
                },
        } => cell_patch_complementarity::run(
            project,
            input,
            outer_folds,
            inner_folds,
            ridge_alphas,
            permutations,
            seed,
            timeout_seconds,
            memory_budget_mib,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::CohortEnergy {
                    project,
                    input,
                    group_a,
                    group_b,
                    metric,
                    permutations,
                    seed,
                    maximum_patients,
                    maximum_features,
                    maximum_distance_elements,
                    maximum_energy_evaluations,
                    memory_budget_mib,
                    out,
                },
        } => cohort_energy::run(
            project,
            input,
            group_a,
            group_b,
            metric,
            permutations,
            seed,
            maximum_patients,
            maximum_features,
            maximum_distance_elements,
            maximum_energy_evaluations,
            memory_budget_mib,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::CausalRandomizedInterference {
                    project,
                    input,
                    out,
                },
        } => causal_workflows::run_randomized_interference(project, input, out),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::GaussianEig {
                    project,
                    input,
                    out,
                },
        } => causal_workflows::run_gaussian_eig(project, input, out),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::CohortClusterCovariatePermutation {
                    project,
                    input,
                    group_a,
                    group_b,
                    permutations,
                    seed,
                    alternative,
                    maximum_patients,
                    maximum_clusters,
                    maximum_covariates,
                    maximum_patient_covariate_cells,
                    maximum_ols_work,
                    memory_budget_mib,
                    out,
                },
        } => cohort_cluster_covariate::run(
            project,
            input,
            group_a,
            group_b,
            permutations,
            seed,
            alternative,
            maximum_patients,
            maximum_clusters,
            maximum_covariates,
            maximum_patient_covariate_cells,
            maximum_ols_work,
            memory_budget_mib,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::CohortHierarchicalMaxT {
                    project,
                    input,
                    group_a,
                    group_b,
                    permutations,
                    seed,
                    alpha,
                    step_down,
                    maximum_patients,
                    maximum_families,
                    maximum_endpoints,
                    maximum_cells,
                    maximum_permutation_endpoint_evaluations,
                    memory_budget_mib,
                    out,
                },
        } => cohort_hierarchical_max_t::run(
            project,
            input,
            group_a,
            group_b,
            permutations,
            seed,
            alpha,
            step_down,
            maximum_patients,
            maximum_families,
            maximum_endpoints,
            maximum_cells,
            maximum_permutation_endpoint_evaluations,
            memory_budget_mib,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::CohortHierarchicalBootstrap {
                    project,
                    input,
                    replicates,
                    seed,
                    alpha,
                    maximum_patients,
                    maximum_specimens,
                    maximum_bootstrap_draws,
                    memory_budget_mib,
                    out,
                },
        } => cohort_hierarchical_bootstrap::run(
            project,
            input,
            replicates,
            seed,
            alpha,
            maximum_patients,
            maximum_specimens,
            maximum_bootstrap_draws,
            memory_budget_mib,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::CohortEquivalence {
                    project,
                    input,
                    lower_margin,
                    upper_margin,
                    alpha,
                    margin_rationale,
                    out,
                },
        } => cohort_margin_inference::run_equivalence(
            project,
            input,
            lower_margin,
            upper_margin,
            alpha,
            margin_rationale,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::CohortNoninferiority {
                    project,
                    input,
                    direction,
                    margin,
                    alpha,
                    margin_rationale,
                    out,
                },
        } => cohort_margin_inference::run_noninferiority(
            project,
            input,
            match direction {
                ProjectNoninferiorityDirection::HigherIsBetter => {
                    marklab_cohort::NoninferiorityDirection::HigherIsBetter
                }
                ProjectNoninferiorityDirection::LowerIsBetter => {
                    marklab_cohort::NoninferiorityDirection::LowerIsBetter
                }
            },
            margin,
            alpha,
            margin_rationale,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::PatientNestedFields {
                    project,
                    input,
                    group_a,
                    group_b,
                    permutations,
                    seed,
                    alpha,
                    maximum_patients,
                    maximum_specimens,
                    maximum_endpoints,
                    maximum_permutation_endpoint_evaluations,
                    memory_budget_mib,
                    out,
                },
        } => patient_nested_fields::run(
            project,
            input,
            group_a,
            group_b,
            permutations,
            seed,
            alpha,
            maximum_patients,
            maximum_specimens,
            maximum_endpoints,
            maximum_permutation_endpoint_evaluations,
            memory_budget_mib,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::CohortMaxT {
                    project,
                    input,
                    group_a,
                    group_b,
                    permutations,
                    seed,
                    alpha,
                    step_down,
                    maximum_patients,
                    maximum_endpoints,
                    maximum_patient_endpoint_cells,
                    maximum_permutation_endpoint_evaluations,
                    memory_budget_mib,
                    out,
                },
        } => cohort_max_t::run(
            project,
            input,
            group_a,
            group_b,
            permutations,
            seed,
            alpha,
            step_down,
            maximum_patients,
            maximum_endpoints,
            maximum_patient_endpoint_cells,
            maximum_permutation_endpoint_evaluations,
            memory_budget_mib,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::CohortMmd {
                    project,
                    input,
                    group_a,
                    group_b,
                    kernel,
                    bandwidth,
                    estimator,
                    permutations,
                    seed,
                    maximum_patients,
                    maximum_features,
                    maximum_kernel_elements,
                    maximum_mmd_evaluations,
                    memory_budget_mib,
                    out,
                },
        } => cohort_mmd::run(
            project,
            input,
            group_a,
            group_b,
            kernel,
            bandwidth,
            estimator,
            permutations,
            seed,
            maximum_patients,
            maximum_features,
            maximum_kernel_elements,
            maximum_mmd_evaluations,
            memory_budget_mib,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::CohortMultisiteCovariateContrast {
                    project,
                    input,
                    group_a,
                    group_b,
                    model,
                    alpha,
                    maximum_patients,
                    maximum_sites,
                    maximum_covariates,
                    maximum_patient_covariate_cells,
                    maximum_ols_work,
                    memory_budget_mib,
                    out,
                },
        } => cohort_multisite_covariate::run(
            project,
            input,
            group_a,
            group_b,
            model,
            alpha,
            maximum_patients,
            maximum_sites,
            maximum_covariates,
            maximum_patient_covariate_cells,
            maximum_ols_work,
            memory_budget_mib,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::ProjectedEmbeddingVariograms {
                    project,
                    input,
                    bins,
                    components,
                    permutations,
                    seed,
                    maximum_pair_visits,
                    timeout_seconds,
                    out,
                },
        } => projected_embedding_variograms::run(
            project,
            input,
            bins,
            components,
            permutations,
            seed,
            maximum_pair_visits,
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::VectorSemivariogram {
                    project,
                    input,
                    bins,
                    weights,
                    out,
                    maximum_points,
                    maximum_dimension,
                    maximum_pair_visits,
                    memory_budget_mib,
                },
        } => vector_semivariogram::run(
            project,
            input,
            bins,
            weights,
            out,
            maximum_points,
            maximum_dimension,
            maximum_pair_visits,
            memory_budget_mib,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::LocalMultivariateMoran {
                    project,
                    input,
                    window,
                    radius_um,
                    permutations,
                    seed,
                    maximum_points,
                    maximum_dimension,
                    maximum_directed_edges,
                    maximum_permutation_edge_evaluations,
                    memory_budget_mib,
                    out,
                },
        } => local_multivariate_moran::run(
            project,
            input,
            window,
            radius_um,
            permutations,
            seed,
            maximum_points,
            maximum_dimension,
            maximum_directed_edges,
            maximum_permutation_edge_evaluations,
            memory_budget_mib,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::EmbeddingCrossCovarianceByDistance {
                    project,
                    input,
                    bins,
                    out,
                    maximum_points,
                    maximum_dimension,
                    maximum_pair_visits,
                    maximum_matrix_elements,
                    memory_budget_mib,
                },
        } => embedding_cross_covariance::run(
            project,
            input,
            bins,
            out,
            maximum_points,
            maximum_dimension,
            maximum_pair_visits,
            maximum_matrix_elements,
            memory_budget_mib,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::KernelMarkCorrelation {
                    project,
                    input,
                    bins,
                    kernel,
                    global_reference_tolerance,
                    maximum_points,
                    maximum_dimension,
                    maximum_pair_visits,
                    memory_budget_mib,
                    out,
                },
        } => kernel_mark_correlation::run(
            project,
            input,
            bins,
            kernel,
            global_reference_tolerance,
            maximum_points,
            maximum_dimension,
            maximum_pair_visits,
            memory_budget_mib,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::EmbeddingSpatialDependenceEnvelope {
                    project,
                    input,
                    bins,
                    curve,
                    permutations,
                    alpha,
                    seed,
                    maximum_points,
                    maximum_dimension,
                    maximum_pair_visits,
                    memory_budget_mib,
                    out,
                },
        } => embedding_spatial_dependence_envelope::run(
            project,
            input,
            bins,
            curve,
            permutations,
            alpha,
            seed,
            maximum_points,
            maximum_dimension,
            maximum_pair_visits,
            memory_budget_mib,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::GraphDirichletEnergy {
                    project,
                    nodes,
                    edges,
                    laplacian,
                    normalization,
                    maximum_nodes,
                    maximum_edges,
                    maximum_dimension,
                    maximum_component_edge_visits,
                    memory_budget_mib,
                    out,
                },
        } => graph_dirichlet_energy::run(
            project,
            nodes,
            edges,
            laplacian,
            normalization,
            maximum_nodes,
            maximum_edges,
            maximum_dimension,
            maximum_component_edge_visits,
            memory_budget_mib,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::GraphSmoothnessPermutationTest {
                    project,
                    nodes,
                    edges,
                    laplacian,
                    permutations,
                    seed,
                    maximum_nodes,
                    maximum_edges,
                    maximum_dimension,
                    maximum_component_edge_visits,
                    memory_budget_mib,
                    out,
                },
        } => graph_smoothness_permutation::run(
            project,
            nodes,
            edges,
            laplacian,
            permutations,
            seed,
            maximum_nodes,
            maximum_edges,
            maximum_dimension,
            maximum_component_edge_visits,
            memory_budget_mib,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::LocalEmbeddingRoughness {
                    project,
                    nodes,
                    edges,
                    epsilon,
                    maximum_nodes,
                    maximum_edges,
                    maximum_dimension,
                    maximum_component_edge_visits,
                    memory_budget_mib,
                    out,
                },
        } => local_embedding_roughness::run(
            project,
            nodes,
            edges,
            epsilon,
            maximum_nodes,
            maximum_edges,
            maximum_dimension,
            maximum_component_edge_visits,
            memory_budget_mib,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::LongitudinalKalmanSmooth {
                    project,
                    input,
                    out,
                },
        } => longitudinal_kalman::run(project, input, out),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::MultiscaleEmbeddingKernel {
                    project,
                    input,
                    weights,
                    sample_a,
                    sample_b,
                    base_kernel,
                    kernel_scale,
                    maximum_summaries,
                    maximum_dimension,
                    maximum_component_scale_visits,
                    memory_budget_mib,
                    out,
                },
        } => multiscale_embedding_kernel::run(
            project,
            input,
            weights,
            sample_a,
            sample_b,
            base_kernel,
            kernel_scale,
            maximum_summaries,
            maximum_dimension,
            maximum_component_scale_visits,
            memory_budget_mib,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::RegionRetrieval {
                    project,
                    training,
                    query,
                    k,
                    leakage_policy,
                    maximum_component_candidate_visits,
                    out,
                },
        } => region_retrieval::run(
            project,
            training,
            query,
            k,
            leakage_policy,
            maximum_component_candidate_visits,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::MarkedPrepost {
                    project,
                    pre,
                    post,
                    out,
                },
        } => run_marked_prepost(project, pre, post, out),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::SparseRadiusHeat {
                    project,
                    input,
                    out,
                },
        } => run_sparse_radius_heat(project, input, out),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::SparseRadiusBasis {
                    project,
                    input,
                    out,
                },
        } => run_sparse_radius_basis(project, input, out),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::GraphMotifTriangleSummary {
                    project,
                    input,
                    out,
                },
        } => graph_motif_summary::run(project, input, out),
        ProjectTopLevel::Project {
            command: ProjectCommand::SmcAbcGrowthFront(arguments),
        } => smc_abc::run(*arguments),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::SpatialVaryingCoefficient {
                    project,
                    input,
                    global_predictor_name,
                    spatial_predictor_name,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    coefficient_prior_sd,
                    amplitude_prior_sd,
                    length_scale_prior_sd_um,
                    known_noise_sd,
                    jitter,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => spatial_varying_coefficient::run(
            project,
            input,
            global_predictor_name,
            spatial_predictor_name,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_sd,
            amplitude_prior_sd,
            length_scale_prior_sd_um,
            known_noise_sd,
            jitter,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::Spatial3dKFunction {
                    project,
                    input,
                    out,
                },
        } => spatial3d_k::run(project, input, out),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::Spatial3dVoxelKFunction {
                    project,
                    input,
                    out,
                },
        } => spatial3d_voxel_k::run(project, input, out),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::Spatial3dRegisteredSerialVoxelK {
                    project,
                    input,
                    out,
                },
        } => spatial3d_registered::run(project, input, out),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::Spatial3dRegisteredLongitudinalVoxelKChange {
                    project,
                    input,
                    out,
                },
        } => spatial3d_registered_longitudinal::run(project, input, out),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::SparseRadiusFourierEnergy {
                    project,
                    input,
                    out,
                },
        } => run_sparse_radius_fourier_energy(project, input, out),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::SparseRadiusHeatStability {
                    project,
                    input,
                    out,
                },
        } => run_sparse_radius_heat_stability(project, input, out),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::SparseRadiusDiffusionWavelet {
                    project,
                    input,
                    out,
                },
        } => run_sparse_radius_diffusion_wavelet(project, input, out),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::SparseRadiusScattering {
                    project,
                    input,
                    out,
                },
        } => run_sparse_radius_scattering(project, input, out),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::WitnessPersistence {
                    project,
                    input,
                    out,
                },
        } => run_witness_persistence(project, input, out),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::WitnessPersistenceStability {
                    project,
                    input,
                    out,
                },
        } => run_witness_persistence_stability(project, input, out),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::WitnessPersistenceBottleneckStability {
                    project,
                    input,
                    out,
                },
        } => run_witness_persistence_bottleneck_stability(project, input, out),
        ProjectTopLevel::Project {
            command: ProjectCommand::ArbitraryWindowIppLikelihood(arguments),
        } => run_arbitrary_window_ipp(
            arguments.project,
            arguments.events,
            arguments.quadrature,
            arguments.window,
            arguments.intercept,
            arguments.coefficient,
            arguments.maximum_events,
            arguments.maximum_quadrature_nodes,
            arguments.maximum_work,
            arguments.maximum_retained_bytes,
            arguments.out,
        ),
        ProjectTopLevel::Project {
            command: ProjectCommand::FitArbitraryWindowIpp(arguments),
        } => run_arbitrary_window_ipp_fit(
            arguments.project,
            arguments.events,
            arguments.quadrature,
            arguments.window,
            arguments.intercept_prior_mean,
            arguments.intercept_prior_sd,
            arguments.coefficient_prior_mean,
            arguments.coefficient_prior_sd,
            NutsSamplingSpec {
                chains: arguments.chains,
                tune_per_chain: arguments.tune,
                draws_per_chain: arguments.draws,
                target_accept: arguments.target_accept,
                seed: arguments.seed,
            },
            arguments.maximum_events,
            arguments.maximum_quadrature_nodes,
            arguments.maximum_draw_node_work,
            arguments.timeout_seconds,
            arguments.out,
        ),
        ProjectTopLevel::Project {
            command: ProjectCommand::ArbitraryWindowIppSpatialPpc(arguments),
        } => arbitrary_window_ipp_spatial_ppc::run(*arguments),
        ProjectTopLevel::Project {
            command: ProjectCommand::ArbitraryWindowLgcp(arguments),
        } => run_arbitrary_window_lgcp(
            arguments.project,
            arguments.events,
            arguments.event_membership,
            arguments.quadrature,
            arguments.window,
            arguments.intercept_prior_mean,
            arguments.intercept_prior_sd,
            arguments.coefficient_prior_mean,
            arguments.coefficient_prior_sd,
            arguments.field_amplitude,
            arguments.field_length_scale_um,
            arguments.jitter,
            NutsSamplingSpec {
                chains: arguments.chains,
                tune_per_chain: arguments.tune,
                draws_per_chain: arguments.draws,
                target_accept: arguments.target_accept,
                seed: arguments.seed,
            },
            arguments.maximum_events,
            arguments.maximum_quadrature_nodes,
            arguments.maximum_draw_node_work,
            arguments.prediction_replicates,
            arguments.prediction_seed,
            arguments.maximum_predictive_points,
            arguments.neighbor_radius_um,
            arguments.maximum_neighbor_pairs,
            arguments.timeout_seconds,
            arguments.out,
        ),
        ProjectTopLevel::Project {
            command: ProjectCommand::ReplicatedArbitraryWindowLgcp(arguments),
        } => run_replicated_arbitrary_window_lgcp(
            arguments.project,
            arguments.input,
            arguments.reference_group,
            arguments.comparison_group,
            arguments.intercept_prior_mean,
            arguments.intercept_prior_sd,
            arguments.group_effect_prior_sd,
            arguments.covariate_effect_prior_sd,
            arguments.patient_sd_prior_scale,
            arguments.pattern_sd_prior_scale,
            arguments.field_amplitude,
            arguments.field_length_scale_um,
            arguments.jitter,
            NutsSamplingSpec {
                chains: arguments.chains,
                tune_per_chain: arguments.tune,
                draws_per_chain: arguments.draws,
                target_accept: arguments.target_accept,
                seed: arguments.seed,
            },
            arguments.maximum_patients,
            arguments.maximum_patterns,
            arguments.maximum_nodes_per_pattern,
            arguments.maximum_total_nodes,
            arguments.maximum_total_events,
            arguments.maximum_draw_node_work,
            arguments.timeout_seconds,
            arguments.out,
        ),
        ProjectTopLevel::Project {
            command: ProjectCommand::ReplicatedArbitraryWindowLgcpInferredKernel(arguments),
        } => run_replicated_arbitrary_window_lgcp_inferred_kernel(
            arguments.project,
            arguments.input,
            arguments.reference_group,
            arguments.comparison_group,
            arguments.intercept_prior_mean,
            arguments.intercept_prior_sd,
            arguments.group_effect_prior_sd,
            arguments.covariate_effect_prior_sd,
            arguments.patient_sd_prior_scale,
            arguments.pattern_sd_prior_scale,
            arguments.field_amplitude_prior_scale,
            arguments.field_length_scale_prior_scale_um,
            arguments.jitter,
            NutsSamplingSpec {
                chains: arguments.chains,
                tune_per_chain: arguments.tune,
                draws_per_chain: arguments.draws,
                target_accept: arguments.target_accept,
                seed: arguments.seed,
            },
            arguments.maximum_patients,
            arguments.maximum_patterns,
            arguments.maximum_nodes_per_pattern,
            arguments.maximum_total_nodes,
            arguments.maximum_total_events,
            arguments.maximum_draw_node_work,
            arguments.maximum_kernel_cube_work,
            arguments.maximum_tree_depth,
            arguments.timeout_seconds,
            arguments.out,
        ),
        ProjectTopLevel::Project {
            command: ProjectCommand::ReplicatedArbitraryWindowMultitypeLgcp(arguments),
        } => run_replicated_arbitrary_window_multitype_lgcp(*arguments),
        ProjectTopLevel::Project {
            command: ProjectCommand::ReplicatedArbitraryWindowMultitypeLgcpInferredKernel(arguments),
        } => run_replicated_arbitrary_window_multitype_lgcp_inferred_kernel(*arguments),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::ReplicatedArbitraryWindowMultitypeLgcpInferredKernelSbc(arguments),
        } => multitype_lgcp_sbc::run(*arguments),
        ProjectTopLevel::Project {
            command: ProjectCommand::CorrelatedReplicatedArbitraryWindowMultitypeLgcp(arguments),
        } => correlated_multitype_lgcp::run(*arguments),
        ProjectTopLevel::Project {
            command: ProjectCommand::JointReplicatedLocationEmbedding(arguments),
        } => joint_location_embedding::run(*arguments),
        ProjectTopLevel::Project {
            command: ProjectCommand::GaussianCrossedNestedHierarchy(arguments),
        } => gaussian_crossed_nested_hierarchy::run(*arguments),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::AdaptiveWindowSpde {
                    project,
                    input,
                    out,
                },
        } => adaptive_window_spde::run(project, input, out),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::NonstationaryAdaptiveWindowSpde {
                    project,
                    input,
                    out,
                },
        } => nonstationary_adaptive_window_spde::run(project, input, out),
        ProjectTopLevel::Project {
            command: ProjectCommand::JointReplicatedLocationMark(arguments),
        } => joint_location_mark::run(*arguments),
        ProjectTopLevel::Project {
            command: ProjectCommand::ConditionalMultitypeMark(arguments),
        } => run_conditional_multitype_mark(*arguments),
        ProjectTopLevel::Project {
            command: ProjectCommand::ReplicatedConditionalMultitypeMark(arguments),
        } => run_replicated_conditional_multitype_mark(*arguments),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::NormalMean {
                    project,
                    input,
                    prior_mean,
                    prior_sd,
                    known_sigma,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_normal_mean(
            project,
            input,
            prior_mean,
            prior_sd,
            known_sigma,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::HierarchicalNormal {
                    project,
                    input,
                    global_prior_mean,
                    global_prior_sd,
                    between_patient_sd_prior,
                    known_sigma,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_hierarchical_normal(
            project,
            input,
            global_prior_mean,
            global_prior_sd,
            between_patient_sd_prior,
            known_sigma,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::BetaBinomialHierarchy {
                    project,
                    input,
                    population_alpha,
                    population_beta,
                    concentration_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_beta_binomial_hierarchy(
            project,
            input,
            population_alpha,
            population_beta,
            concentration_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::BetaBinomialGroupRegression {
                    project,
                    input,
                    reference_group,
                    comparison_group,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    group_effect_prior_sd,
                    concentration_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_beta_binomial_group_regression(
            project,
            input,
            reference_group,
            comparison_group,
            intercept_prior_mean,
            intercept_prior_sd,
            group_effect_prior_sd,
            concentration_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::DirichletMultinomialGroup {
                    project,
                    input,
                    reference_group,
                    comparison_group,
                    logit_prior_sd,
                    group_effect_prior_sd,
                    concentration_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_dirichlet_multinomial_group(
            project,
            input,
            reference_group,
            comparison_group,
            logit_prior_sd,
            group_effect_prior_sd,
            concentration_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::OrdinalGroup {
                    project,
                    input,
                    reference_group,
                    comparison_group,
                    ordered_levels,
                    cutpoint_prior_sd,
                    group_effect_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_ordinal_group(
            project,
            input,
            reference_group,
            comparison_group,
            ordered_levels.split(',').map(str::to_owned).collect(),
            cutpoint_prior_sd,
            group_effect_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::OrdinalGroupSiteHierarchy {
                    project,
                    input,
                    reference_group,
                    comparison_group,
                    ordered_levels,
                    cutpoint_prior_sd,
                    group_effect_prior_sd,
                    site_intercept_sd_prior_sd,
                    site_group_slope_sd_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_ordinal_group_site_hierarchy(
            project,
            input,
            reference_group,
            comparison_group,
            ordered_levels.split(',').map(str::to_owned).collect(),
            cutpoint_prior_sd,
            group_effect_prior_sd,
            site_intercept_sd_prior_sd,
            site_group_slope_sd_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::NonproportionalOrdinalGroupSite {
                    project,
                    input,
                    reference_group,
                    comparison_group,
                    ordered_levels,
                    cutpoint_prior_sd,
                    site_intercept_sd_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_nonproportional_ordinal_group_site(
            project,
            input,
            reference_group,
            comparison_group,
            ordered_levels.split(',').map(str::to_owned).collect(),
            cutpoint_prior_sd,
            site_intercept_sd_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::OrdinalSiteHeldoutComparison {
                    project,
                    input,
                    reference_group,
                    comparison_group,
                    ordered_levels,
                    smoothing,
                    optimizer_tolerance,
                    maximum_optimizer_evaluations,
                    out,
                },
        } => ordinal_site_heldout::run(
            project,
            input,
            reference_group,
            comparison_group,
            ordered_levels.split(',').map(str::to_owned).collect(),
            smoothing,
            optimizer_tolerance,
            maximum_optimizer_evaluations,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::HurdleBetaBinomialGroup {
                    project,
                    input,
                    reference_group,
                    comparison_group,
                    presence_intercept_prior_mean,
                    presence_intercept_prior_sd,
                    presence_group_effect_prior_sd,
                    abundance_intercept_prior_mean,
                    abundance_intercept_prior_sd,
                    abundance_group_effect_prior_sd,
                    concentration_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_hurdle_beta_binomial_group(
            project,
            input,
            reference_group,
            comparison_group,
            presence_intercept_prior_mean,
            presence_intercept_prior_sd,
            presence_group_effect_prior_sd,
            abundance_intercept_prior_mean,
            abundance_intercept_prior_sd,
            abundance_group_effect_prior_sd,
            concentration_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::BetaBinomialGroupGenderRegression {
                    project,
                    input,
                    reference_group,
                    comparison_group,
                    reference_gender,
                    comparison_gender,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    group_effect_prior_sd,
                    gender_effect_prior_sd,
                    concentration_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_beta_binomial_group_gender_regression(
            project,
            input,
            reference_group,
            comparison_group,
            reference_gender,
            comparison_gender,
            intercept_prior_mean,
            intercept_prior_sd,
            group_effect_prior_sd,
            gender_effect_prior_sd,
            concentration_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::BetaBinomialGroupGenderSlideHierarchy {
                    project,
                    input,
                    reference_group,
                    comparison_group,
                    reference_gender,
                    comparison_gender,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    group_effect_prior_sd,
                    gender_effect_prior_sd,
                    patient_log_odds_sd_prior_sd,
                    slide_concentration_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_beta_binomial_group_gender_slide_hierarchy(
            project,
            input,
            reference_group,
            comparison_group,
            reference_gender,
            comparison_gender,
            intercept_prior_mean,
            intercept_prior_sd,
            group_effect_prior_sd,
            gender_effect_prior_sd,
            patient_log_odds_sd_prior_sd,
            slide_concentration_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::StudentTHierarchy {
                    project,
                    input,
                    global_prior_mean,
                    global_prior_sd,
                    between_patient_sd_prior,
                    observation_sd_prior,
                    degrees_of_freedom_excess_rate,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_student_t_hierarchy(
            project,
            input,
            global_prior_mean,
            global_prior_sd,
            between_patient_sd_prior,
            observation_sd_prior,
            degrees_of_freedom_excess_rate,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::GriddedLgcp {
                    project,
                    events,
                    grid,
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    grid_x,
                    grid_y,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    coefficient_prior_mean,
                    coefficient_prior_sd,
                    field_amplitude,
                    field_length_scale_um,
                    jitter,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_gridded_lgcp(
            project,
            events,
            grid,
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            grid_x,
            grid_y,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_mean,
            coefficient_prior_sd,
            field_amplitude,
            field_length_scale_um,
            jitter,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::FusedGromovWasserstein {
                    project,
                    input,
                    alpha,
                    epsilon,
                    feature_scale,
                    structure_scale,
                    tolerance,
                    maximum_iterations,
                    timeout_seconds,
                    out,
                },
        } => run_fused_gromov_wasserstein(
            project,
            input,
            alpha,
            epsilon,
            feature_scale,
            structure_scale,
            tolerance,
            maximum_iterations,
            timeout_seconds,
            out,
        ),
    }
}

fn run_sparse_radius_heat(
    project_path: PathBuf,
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let input_kind = "application/vnd.marklab.source.graph-sparse-radius-heat+json;version=1";
    let before = source_artifact(&input_path, input_kind)?;
    let metadata = fs::metadata(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "sparse radius heat input must be a regular file within 16 MiB".into(),
        ));
    }
    let request_bytes = fs::read(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    let spec: GraphSparseRadiusHeatSpec = serde_json::from_slice(&request_bytes)?;
    let after = source_artifact(&input_path, input_kind)?;
    if before != after {
        return Err(BayesCliError::Input(
            "sparse radius heat input changed while the durable request was prepared".into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = SparseRadiusHeatProjectNode::new(input_path, before, spec, request_bytes)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let schema = ArtifactSchema::new("marklab.graph_sparse_radius_heat_result", 1)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        schema,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::publish_json(&output_path, &run.output)?;
    eprintln!("project sparse-radius-heat cache_status={cache_status}");
    Ok(())
}

fn run_sparse_radius_basis(
    project_path: PathBuf,
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let input_kind = "application/vnd.marklab.source.graph-sparse-radius-basis+json;version=1";
    let before = source_artifact(&input_path, input_kind)?;
    let metadata = fs::metadata(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "sparse radius basis input must be a regular file within 16 MiB".into(),
        ));
    }
    let request_bytes = fs::read(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    let spec: GraphSparseRadiusBasisSpec = serde_json::from_slice(&request_bytes)?;
    let after = source_artifact(&input_path, input_kind)?;
    if before != after {
        return Err(BayesCliError::Input(
            "sparse radius basis input changed while the durable request was prepared".into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = SparseRadiusBasisProjectNode::new(input_path, before, spec, request_bytes)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let schema = ArtifactSchema::new("marklab.graph_sparse_radius_basis_result", 1)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        schema,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::publish_json(&output_path, &run.output)?;
    eprintln!("project sparse-radius-basis cache_status={cache_status}");
    Ok(())
}

fn run_sparse_radius_fourier_energy(
    project_path: PathBuf,
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let input_kind =
        "application/vnd.marklab.source.graph-sparse-radius-fourier-energy+json;version=1";
    let before = source_artifact(&input_path, input_kind)?;
    let metadata = fs::metadata(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "sparse radius Fourier energy input must be a regular file within 16 MiB".into(),
        ));
    }
    let request_bytes = fs::read(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    let spec: GraphSparseRadiusFourierEnergySpec = serde_json::from_slice(&request_bytes)?;
    let after = source_artifact(&input_path, input_kind)?;
    if before != after {
        return Err(BayesCliError::Input(
            "sparse radius Fourier energy input changed while the durable request was prepared"
                .into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = SparseRadiusFourierEnergyProjectNode::new(input_path, before, spec, request_bytes)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let schema = ArtifactSchema::new("marklab.graph_sparse_radius_fourier_energy_result", 1)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        schema,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::publish_json(&output_path, &run.output)?;
    eprintln!("project sparse-radius-fourier-energy cache_status={cache_status}");
    Ok(())
}

fn run_sparse_radius_heat_stability(
    project_path: PathBuf,
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let input_kind =
        "application/vnd.marklab.source.graph-sparse-radius-heat-stability+json;version=1";
    let before = source_artifact(&input_path, input_kind)?;
    let metadata = fs::metadata(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "sparse radius heat stability input must be a regular file within 16 MiB".into(),
        ));
    }
    let request_bytes = fs::read(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    let spec: GraphSparseRadiusHeatStabilitySpec = serde_json::from_slice(&request_bytes)?;
    let after = source_artifact(&input_path, input_kind)?;
    if before != after {
        return Err(BayesCliError::Input(
            "sparse radius heat stability input changed while the durable request was prepared"
                .into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = SparseRadiusHeatStabilityProjectNode::new(input_path, before, spec, request_bytes)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let schema = ArtifactSchema::new("marklab.graph_sparse_radius_heat_stability_result", 1)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        schema,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::publish_json(&output_path, &run.output)?;
    eprintln!("project sparse-radius-heat-stability cache_status={cache_status}");
    Ok(())
}

fn run_sparse_radius_diffusion_wavelet(
    project_path: PathBuf,
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let input_kind =
        "application/vnd.marklab.source.graph-sparse-radius-diffusion-wavelet+json;version=1";
    let before = source_artifact(&input_path, input_kind)?;
    let metadata = fs::metadata(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "sparse radius diffusion wavelet input must be a regular file within 16 MiB".into(),
        ));
    }
    let request_bytes = fs::read(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    let spec: GraphSparseRadiusDiffusionWaveletSpec = serde_json::from_slice(&request_bytes)?;
    let after = source_artifact(&input_path, input_kind)?;
    if before != after {
        return Err(BayesCliError::Input(
            "sparse radius diffusion wavelet input changed while the durable request was prepared"
                .into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node =
        SparseRadiusDiffusionWaveletProjectNode::new(input_path, before, spec, request_bytes)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let schema = ArtifactSchema::new("marklab.graph_sparse_radius_diffusion_wavelet_result", 1)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        schema,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::publish_json(&output_path, &run.output)?;
    eprintln!("project sparse-radius-diffusion-wavelet cache_status={cache_status}");
    Ok(())
}

fn run_sparse_radius_scattering(
    project_path: PathBuf,
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let input_kind = "application/vnd.marklab.source.graph-sparse-radius-scattering+json;version=1";
    let before = source_artifact(&input_path, input_kind)?;
    let metadata = fs::metadata(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "sparse radius scattering input must be a regular file within 16 MiB".into(),
        ));
    }
    let request_bytes = fs::read(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    let spec: GraphSparseRadiusScatteringSpec = serde_json::from_slice(&request_bytes)?;
    let after = source_artifact(&input_path, input_kind)?;
    if before != after {
        return Err(BayesCliError::Input(
            "sparse radius scattering input changed while the durable request was prepared".into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = SparseRadiusScatteringProjectNode::new(input_path, before, spec, request_bytes)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let schema = ArtifactSchema::new("marklab.graph_sparse_radius_scattering_result", 1)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        schema,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::publish_json(&output_path, &run.output)?;
    eprintln!("project sparse-radius-scattering cache_status={cache_status}");
    Ok(())
}

fn run_witness_persistence(
    project_path: PathBuf,
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let input_kind = "application/vnd.marklab.source.witness-persistence+json;version=1";
    let before = source_artifact(&input_path, input_kind)?;
    let prepared = topology::prepare_witness_persistence(&input_path).map_err(topology_error)?;
    let after = source_artifact(&input_path, input_kind)?;
    if before != after {
        return Err(BayesCliError::Input(
            "witness persistence input changed while the durable request was prepared".into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_WITNESS_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_WITNESS_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = WitnessPersistenceProjectNode::new(input_path, before, prepared)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_WITNESS_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let schema = ArtifactSchema::new("marklab.gudhi_witness_persistence_result", 1)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        schema,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::publish_json(&output_path, &run.output)?;
    eprintln!("project witness-persistence cache_status={cache_status}");
    Ok(())
}

fn run_witness_persistence_stability(
    project_path: PathBuf,
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let input_kind = "application/vnd.marklab.source.witness-persistence-stability+json;version=1";
    let before = source_artifact(&input_path, input_kind)?;
    let prepared =
        topology::prepare_witness_persistence_stability(&input_path).map_err(topology_error)?;
    let after = source_artifact(&input_path, input_kind)?;
    if before != after {
        return Err(BayesCliError::Input(
            "witness persistence stability input changed while the durable request was prepared"
                .into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_WITNESS_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_WITNESS_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = WitnessPersistenceStabilityProjectNode::new(input_path, before, prepared)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_WITNESS_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let schema = ArtifactSchema::new("marklab.gudhi_witness_persistence_stability_result", 1)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        schema,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::publish_json(&output_path, &run.output)?;
    eprintln!("project witness-persistence-stability cache_status={cache_status}");
    Ok(())
}

fn run_witness_persistence_bottleneck_stability(
    project_path: PathBuf,
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let input_kind =
        "application/vnd.marklab.source.witness-persistence-bottleneck-stability+json;version=1";
    let before = source_artifact(&input_path, input_kind)?;
    let prepared = topology::prepare_witness_persistence_bottleneck_stability(&input_path)
        .map_err(topology_error)?;
    let after = source_artifact(&input_path, input_kind)?;
    if before != after {
        return Err(BayesCliError::Input(
            "witness bottleneck stability input changed while the durable request was prepared"
                .into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = WitnessPersistenceBottleneckStabilityProjectNode::new(input_path, before, prepared)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let schema = ArtifactSchema::new(
        "marklab.gudhi_witness_persistence_bottleneck_stability_result",
        1,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        schema,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::publish_json(&output_path, &run.output)?;
    eprintln!("project witness-persistence-bottleneck-stability cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_arbitrary_window_ipp(
    project_path: PathBuf,
    events_path: PathBuf,
    quadrature_path: PathBuf,
    window_path: PathBuf,
    intercept: f64,
    coefficient: f64,
    maximum_events: usize,
    maximum_quadrature_nodes: usize,
    maximum_work: usize,
    maximum_retained_bytes: usize,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let kinds = [
        "application/vnd.marklab.source.arbitrary-window-ipp-events+csv;version=1",
        "application/vnd.marklab.source.arbitrary-window-ipp-quadrature+csv;version=1",
        "application/vnd.marklab.source.observation-window+geojson;version=1",
    ];
    let paths = [events_path, quadrature_path, window_path];
    let before = [
        source_artifact(&paths[0], kinds[0])?,
        source_artifact(&paths[1], kinds[1])?,
        source_artifact(&paths[2], kinds[2])?,
    ];
    let prepared = bayes::prepare_arbitrary_window_ipp(
        paths[0].clone(),
        paths[1].clone(),
        paths[2].clone(),
        intercept,
        coefficient,
        maximum_events,
        maximum_quadrature_nodes,
        maximum_work,
        maximum_retained_bytes,
    )?;
    let after = [
        source_artifact(&paths[0], kinds[0])?,
        source_artifact(&paths[1], kinds[1])?,
        source_artifact(&paths[2], kinds[2])?,
    ];
    if before != after {
        return Err(BayesCliError::Input(
            "arbitrary-window IPP source changed while the durable request was prepared".into(),
        ));
    }
    let configuration_bytes = serde_json::to_vec(&serde_json::json!({
        "intercept": intercept,
        "coefficient": coefficient,
        "maximum_events": maximum_events,
        "maximum_quadrature_nodes": maximum_quadrature_nodes,
        "maximum_work": maximum_work,
        "maximum_retained_bytes": maximum_retained_bytes,
    }))?;
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    for artifact in &before {
        project
            .register_reference(artifact.clone())
            .map_err(|error| BayesCliError::Input(error.to_string()))?;
    }
    let node = ArbitraryWindowIppProjectNode::new(paths, before, prepared, configuration_bytes)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let schema = ArtifactSchema::new("marklab.arbitrary_window_ipp_likelihood_result", 1)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        schema,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::publish_json(&output_path, &run.output)?;
    eprintln!("project arbitrary-window-ipp-likelihood cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_arbitrary_window_ipp_fit(
    project_path: PathBuf,
    events_path: PathBuf,
    quadrature_path: PathBuf,
    window_path: PathBuf,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_mean: f64,
    coefficient_prior_sd: f64,
    sampling: NutsSamplingSpec,
    maximum_events: usize,
    maximum_quadrature_nodes: usize,
    maximum_draw_node_work: u64,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcArbitraryWindowIpp.descriptor();
    let paths = [events_path, quadrature_path, window_path];
    let before = [
        source_artifact(&paths[0], backend.input_kinds[0])?,
        source_artifact(&paths[1], backend.input_kinds[1])?,
        source_artifact(&paths[2], backend.input_kinds[2])?,
    ];
    let prepared = bayes::prepare_arbitrary_window_ipp_fit(
        paths[0].clone(),
        paths[1].clone(),
        paths[2].clone(),
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_mean,
        coefficient_prior_sd,
        sampling,
        maximum_events,
        maximum_quadrature_nodes,
        maximum_draw_node_work,
        timeout_seconds,
    )?;
    let after = [
        source_artifact(&paths[0], backend.input_kinds[0])?,
        source_artifact(&paths[1], backend.input_kinds[1])?,
        source_artifact(&paths[2], backend.input_kinds[2])?,
    ];
    if before != after {
        return Err(BayesCliError::Input(
            "arbitrary-window IPP fit source changed while the durable request was prepared".into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    for artifact in &before {
        project
            .register_reference(artifact.clone())
            .map_err(|error| BayesCliError::Input(error.to_string()))?;
    }
    let node = ArbitraryWindowIppFitProjectNode::new(paths, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::publish_json(&output_path, &run.output)?;
    eprintln!("project fit-arbitrary-window-ipp cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_arbitrary_window_lgcp(
    project_path: PathBuf,
    events_path: PathBuf,
    membership_path: PathBuf,
    quadrature_path: PathBuf,
    window_path: PathBuf,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_mean: f64,
    coefficient_prior_sd: f64,
    field_amplitude: f64,
    field_length_scale_um: f64,
    jitter: f64,
    sampling: NutsSamplingSpec,
    maximum_events: usize,
    maximum_quadrature_nodes: usize,
    maximum_draw_node_work: u64,
    prediction_replicates: u32,
    prediction_seed: u64,
    maximum_predictive_points: u64,
    neighbor_radius_um: f64,
    maximum_neighbor_pairs: usize,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcArbitraryWindowLgcp.descriptor();
    let paths = [events_path, membership_path, quadrature_path, window_path];
    let before = [
        source_artifact(&paths[0], backend.input_kinds[0])?,
        source_artifact(&paths[1], backend.input_kinds[1])?,
        source_artifact(&paths[2], backend.input_kinds[2])?,
        source_artifact(&paths[3], backend.input_kinds[3])?,
    ];
    let prepared = bayes::prepare_arbitrary_window_lgcp_fit(
        paths[0].clone(),
        paths[1].clone(),
        paths[2].clone(),
        paths[3].clone(),
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_mean,
        coefficient_prior_sd,
        field_amplitude,
        field_length_scale_um,
        jitter,
        sampling,
        maximum_events,
        maximum_quadrature_nodes,
        maximum_draw_node_work,
        prediction_replicates,
        prediction_seed,
        maximum_predictive_points,
        neighbor_radius_um,
        maximum_neighbor_pairs,
        timeout_seconds,
    )?;
    let after = [
        source_artifact(&paths[0], backend.input_kinds[0])?,
        source_artifact(&paths[1], backend.input_kinds[1])?,
        source_artifact(&paths[2], backend.input_kinds[2])?,
        source_artifact(&paths[3], backend.input_kinds[3])?,
    ];
    if before != after {
        return Err(BayesCliError::Input(
            "arbitrary-window LGCP source changed while the durable request was prepared".into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    for artifact in &before {
        project
            .register_reference(artifact.clone())
            .map_err(|error| BayesCliError::Input(error.to_string()))?;
    }
    let node = ArbitraryWindowLgcpProjectNode::new(paths, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::publish_json(&output_path, &run.output)?;
    eprintln!("project arbitrary-window-lgcp cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_replicated_arbitrary_window_lgcp(
    project_path: PathBuf,
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    group_effect_prior_sd: f64,
    covariate_effect_prior_sd: f64,
    patient_sd_prior_scale: f64,
    pattern_sd_prior_scale: f64,
    field_amplitude: f64,
    field_length_scale_um: f64,
    jitter: f64,
    sampling: NutsSamplingSpec,
    maximum_patients: usize,
    maximum_patterns: usize,
    maximum_nodes_per_pattern: usize,
    maximum_total_nodes: usize,
    maximum_total_events: u64,
    maximum_draw_node_work: u64,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcReplicatedArbitraryWindowLgcp.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_replicated_arbitrary_window_lgcp_fit(
        input_path.clone(),
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
        sampling,
        maximum_patients,
        maximum_patterns,
        maximum_nodes_per_pattern,
        maximum_total_nodes,
        maximum_total_events,
        maximum_draw_node_work,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "replicated LGCP source changed while the durable request was prepared".into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node =
        ReplicatedArbitraryWindowLgcpProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::publish_json(&output_path, &run.output)?;
    eprintln!("project replicated-arbitrary-window-lgcp cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_replicated_arbitrary_window_lgcp_inferred_kernel(
    project_path: PathBuf,
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    group_effect_prior_sd: f64,
    covariate_effect_prior_sd: f64,
    patient_sd_prior_scale: f64,
    pattern_sd_prior_scale: f64,
    field_amplitude_prior_scale: f64,
    field_length_scale_prior_scale_um: f64,
    jitter: f64,
    sampling: NutsSamplingSpec,
    maximum_patients: usize,
    maximum_patterns: usize,
    maximum_nodes_per_pattern: usize,
    maximum_total_nodes: usize,
    maximum_total_events: u64,
    maximum_draw_node_work: u64,
    maximum_kernel_cube_work: u64,
    maximum_tree_depth: u32,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend =
        StaticBackendWorkflow::PymcReplicatedArbitraryWindowLgcpInferredKernel.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_replicated_arbitrary_window_lgcp_inferred_kernel(
        input_path.clone(),
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
        sampling,
        maximum_patients,
        maximum_patterns,
        maximum_nodes_per_pattern,
        maximum_total_nodes,
        maximum_total_events,
        maximum_draw_node_work,
        maximum_kernel_cube_work,
        maximum_tree_depth,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "inferred-kernel replicated LGCP source changed while prepared".into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = ReplicatedArbitraryWindowLgcpInferredKernelProjectNode::new(
        input_path, before, prepared, backend,
    )?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::publish_json(&output_path, &run.output)?;
    eprintln!(
        "project replicated-arbitrary-window-lgcp-inferred-kernel cache_status={cache_status}"
    );
    Ok(())
}

fn run_replicated_arbitrary_window_multitype_lgcp(
    arguments: ReplicatedArbitraryWindowMultitypeLgcpProjectArgs,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcReplicatedArbitraryWindowMultitypeLgcp.descriptor();
    let before = source_artifact(&arguments.input, backend.input_kinds[0])?;
    let input_bytes = fs::read(&arguments.input).map_err(|source| BayesCliError::Io {
        path: arguments.input.clone(),
        source,
    })?;
    if input_bytes.len() as u64 > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "replicated multitype LGCP durable input exceeds its source ceiling".into(),
        ));
    }
    let output_path = arguments.out.clone();
    let prepared = bayes::prepare_replicated_arbitrary_window_multitype_lgcp(
        bayes::ReplicatedArbitraryWindowMultitypeLgcpArgs {
            input: arguments.input.clone(),
            reference_group: arguments.reference_group,
            comparison_group: arguments.comparison_group,
            reference_type: arguments.reference_type,
            intercept_prior_mean: arguments.intercept_prior_mean,
            intercept_prior_sd: arguments.intercept_prior_sd,
            group_effect_prior_sd: arguments.group_effect_prior_sd,
            covariate_effect_prior_sd: arguments.covariate_effect_prior_sd,
            patient_sd_prior_scale: arguments.patient_sd_prior_scale,
            pattern_sd_prior_scale: arguments.pattern_sd_prior_scale,
            field_amplitude: arguments.field_amplitude,
            field_length_scale_um: arguments.field_length_scale_um,
            jitter: arguments.jitter,
            chains: arguments.chains,
            tune: arguments.tune,
            draws: arguments.draws,
            target_accept: arguments.target_accept,
            seed: arguments.seed,
            maximum_patients: arguments.maximum_patients,
            maximum_patterns: arguments.maximum_patterns,
            maximum_types: arguments.maximum_types,
            maximum_nodes_per_pattern: arguments.maximum_nodes_per_pattern,
            maximum_total_nodes: arguments.maximum_total_nodes,
            maximum_total_node_type_rows: arguments.maximum_total_node_type_rows,
            maximum_total_events: arguments.maximum_total_events,
            maximum_draw_node_type_work: arguments.maximum_draw_node_type_work,
            timeout_seconds: arguments.timeout_seconds,
            out: PathBuf::new(),
        },
    )?;
    backend.validate_request_backend(prepared.backend())?;
    let after = source_artifact(&arguments.input, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "replicated multitype LGCP source changed while prepared".into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&arguments.project, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = ReplicatedArbitraryWindowMultitypeLgcpProjectNode::new(
        arguments.input,
        before,
        prepared,
        backend,
    )?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::publish_json(&output_path, &run.output)?;
    eprintln!("project replicated-arbitrary-window-multitype-lgcp cache_status={cache_status}");
    Ok(())
}

fn run_conditional_multitype_mark(
    arguments: ConditionalMultitypeMarkProjectArgs,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcConditionalMultitypeMark.descriptor();
    let before = source_artifact(&arguments.input, backend.input_kinds[0])?;
    let input_bytes = fs::read(&arguments.input).map_err(|source| BayesCliError::Io {
        path: arguments.input.clone(),
        source,
    })?;
    if input_bytes.len() as u64 > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "conditional multitype source exceeds input ceiling".into(),
        ));
    }
    let input_sha256 = marklab_bayes::sha256_hex(&input_bytes);
    let request_backend = bayes::conditional_multitype_mark_backend_contract()?;
    backend.validate_request_backend(&request_backend)?;
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
    let after = source_artifact(&arguments.input, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "conditional multitype source changed while prepared".into(),
        ));
    }
    let output_path = arguments.out.clone();
    let project_path = arguments.project.clone();
    let node = ConditionalMultitypeMarkProjectNode::new(
        arguments.input.clone(),
        before.clone(),
        bayes::ConditionalMultitypeMarkArgs {
            input: arguments.input,
            reference_type: arguments.reference_type,
            radius_um: arguments.radius_um,
            intercept_prior_sd: arguments.intercept_prior_sd,
            interaction_prior_sd: arguments.interaction_prior_sd,
            chains: arguments.chains,
            tune: arguments.tune,
            draws: arguments.draws,
            target_accept: arguments.target_accept,
            seed: arguments.seed,
            maximum_points: arguments.maximum_points,
            maximum_types: arguments.maximum_types,
            maximum_neighbor_visits: arguments.maximum_neighbor_visits,
            maximum_edges: arguments.maximum_edges,
            maximum_draw_parameter_work: arguments.maximum_draw_parameter_work,
            maximum_working_bytes: arguments.maximum_working_bytes,
            maximum_tree_depth: arguments.maximum_tree_depth,
            timeout_seconds: arguments.timeout_seconds,
            out: PathBuf::new(),
        },
        input_sha256,
        request_backend,
        configuration_bytes,
        backend,
    )?;
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::publish_json(&output_path, &run.output)?;
    eprintln!("project conditional-multitype-mark cache_status={cache_status}");
    Ok(())
}

fn run_replicated_arbitrary_window_multitype_lgcp_inferred_kernel(
    arguments: ReplicatedArbitraryWindowMultitypeLgcpInferredKernelProjectArgs,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcReplicatedArbitraryWindowMultitypeLgcpInferredKernel
        .descriptor();
    let before = source_artifact(&arguments.input, backend.input_kinds[0])?;
    let output_path = arguments.out.clone();
    let prepared = bayes::prepare_replicated_arbitrary_window_multitype_lgcp_inferred_kernel(
        bayes::ReplicatedArbitraryWindowMultitypeLgcpInferredKernelArgs {
            input: arguments.input.clone(),
            reference_group: arguments.reference_group,
            comparison_group: arguments.comparison_group,
            reference_type: arguments.reference_type,
            intercept_prior_mean: arguments.intercept_prior_mean,
            intercept_prior_sd: arguments.intercept_prior_sd,
            group_effect_prior_sd: arguments.group_effect_prior_sd,
            covariate_effect_prior_sd: arguments.covariate_effect_prior_sd,
            patient_sd_prior_scale: arguments.patient_sd_prior_scale,
            pattern_sd_prior_scale: arguments.pattern_sd_prior_scale,
            field_amplitude_prior_scale: arguments.field_amplitude_prior_scale,
            field_length_scale_prior_scale_um: arguments.field_length_scale_prior_scale_um,
            jitter: arguments.jitter,
            chains: arguments.chains,
            tune: arguments.tune,
            draws: arguments.draws,
            target_accept: arguments.target_accept,
            seed: arguments.seed,
            maximum_patients: arguments.maximum_patients,
            maximum_patterns: arguments.maximum_patterns,
            maximum_types: arguments.maximum_types,
            maximum_nodes_per_pattern: arguments.maximum_nodes_per_pattern,
            maximum_total_nodes: arguments.maximum_total_nodes,
            maximum_total_node_type_rows: arguments.maximum_total_node_type_rows,
            maximum_total_events: arguments.maximum_total_events,
            maximum_draw_node_type_work: arguments.maximum_draw_node_type_work,
            maximum_kernel_cube_work: arguments.maximum_kernel_cube_work,
            maximum_tree_depth: arguments.maximum_tree_depth,
            timeout_seconds: arguments.timeout_seconds,
            out: PathBuf::new(),
        },
    )?;
    backend.validate_request_backend(prepared.backend())?;
    let after = source_artifact(&arguments.input, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "replicated multitype inferred-kernel source changed while prepared".into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&arguments.project, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = ReplicatedArbitraryWindowMultitypeLgcpInferredKernelProjectNode::new(
        arguments.input,
        before,
        prepared,
        backend,
    )?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::publish_json(&output_path, &run.output)?;
    eprintln!(
        "project replicated-arbitrary-window-multitype-lgcp-inferred-kernel cache_status={cache_status}"
    );
    Ok(())
}

fn run_replicated_conditional_multitype_mark(
    arguments: ReplicatedConditionalMultitypeMarkProjectArgs,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcReplicatedConditionalMultitypeMark.descriptor();
    let before = source_artifact(&arguments.input, backend.input_kinds[0])?;
    let input_bytes = fs::read(&arguments.input).map_err(|source| BayesCliError::Io {
        path: arguments.input.clone(),
        source,
    })?;
    if input_bytes.len() as u64 > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "replicated conditional-mark source exceeds durable input ceiling".into(),
        ));
    }
    let input_sha256 = marklab_bayes::sha256_hex(&input_bytes);
    let request_backend = bayes::replicated_conditional_multitype_mark_backend_contract()?;
    backend.validate_request_backend(&request_backend)?;
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
    let after = source_artifact(&arguments.input, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "replicated conditional-mark source changed while prepared".into(),
        ));
    }
    let output_path = arguments.out.clone();
    let project_path = arguments.project.clone();
    let node = ReplicatedConditionalMultitypeMarkProjectNode::new(
        arguments.input.clone(),
        before.clone(),
        bayes::ReplicatedConditionalMultitypeMarkArgs {
            input: arguments.input,
            reference_group: arguments.reference_group,
            reference_type: arguments.reference_type,
            radius_um: arguments.radius_um,
            intercept_prior_sd: arguments.intercept_prior_sd,
            interaction_prior_sd: arguments.interaction_prior_sd,
            group_effect_prior_sd: arguments.group_effect_prior_sd,
            patient_sd_prior_scale: arguments.patient_sd_prior_scale,
            pattern_sd_prior_scale: arguments.pattern_sd_prior_scale,
            chains: arguments.chains,
            tune: arguments.tune,
            draws: arguments.draws,
            target_accept: arguments.target_accept,
            seed: arguments.seed,
            maximum_patients: arguments.maximum_patients,
            maximum_patterns: arguments.maximum_patterns,
            maximum_points: arguments.maximum_points,
            maximum_types: arguments.maximum_types,
            maximum_neighbor_visits: arguments.maximum_neighbor_visits,
            maximum_edges: arguments.maximum_edges,
            maximum_draw_parameter_work: arguments.maximum_draw_parameter_work,
            maximum_working_bytes: arguments.maximum_working_bytes,
            maximum_tree_depth: arguments.maximum_tree_depth,
            timeout_seconds: arguments.timeout_seconds,
            out: PathBuf::new(),
        },
        input_sha256,
        request_backend,
        configuration_bytes,
        backend,
    )?;
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::publish_json(&output_path, &run.output)?;
    eprintln!("project replicated-conditional-multitype-mark cache_status={cache_status}");
    Ok(())
}

fn run_marked_prepost(
    project_path: PathBuf,
    pre_path: PathBuf,
    post_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let pre_path = result_document_path(pre_path);
    let post_path = result_document_path(post_path);
    let pre_source = source_artifact(&pre_path, MARKED_RESULT_KIND)?;
    let post_source = source_artifact(&post_path, MARKED_RESULT_KIND)?;
    let pre = MarkedResultImportNode::new("marked-pre-import", pre_path, pre_source.clone())?;
    let post = MarkedResultImportNode::new("marked-post-import", post_path, post_source.clone())?;

    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(pre_source)
        .and_then(|()| project.register_reference(post_source))
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let import_graph = WorkflowGraph::new([pre.spec().clone(), post.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let import_schema = ArtifactSchema::new("marklab.marked_pattern_result", 1)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let pre_run = execute_algorithm(
        &mut durable,
        &mut project,
        &import_graph,
        &pre,
        &scheduler,
        import_schema.clone(),
        runtime.clone(),
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let post_run = execute_algorithm(
        &mut durable,
        &mut project,
        &import_graph,
        &post,
        &scheduler,
        import_schema,
        runtime.clone(),
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let comparison = MarkedPrePostNode::new(
        NodeId::new("marked-prepost").map_err(|error| BayesCliError::Input(error.to_string()))?,
        pre.spec().id().clone(),
        &pre_run,
        post.spec().id().clone(),
        &post_run,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let graph = WorkflowGraph::new([
        pre.spec().clone(),
        post.spec().clone(),
        comparison.spec().clone(),
    ])
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let comparison_run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &comparison,
        &scheduler,
        ArtifactSchema::new("marklab.marked_prepost_result", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;

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

fn cache_status_name(status: CacheStatus) -> &'static str {
    match status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    }
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
    fn new(node_id: &str, input_path: PathBuf, input: ArtifactRef) -> Result<Self, BayesCliError> {
        let output = read_marked_result(&input_path)?;
        let observed = source_artifact(&input_path, MARKED_RESULT_KIND)?;
        if observed != input {
            return Err(BayesCliError::Input(format!(
                "marked result changed while its durable import was prepared: {}",
                input_path.display()
            )));
        }
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(node_id).map_err(|error| BayesCliError::Input(error.to_string()))?,
                "marked_result_import",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
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
            return Err(NodeError::input(BayesCliError::Input(format!(
                "marked result no longer matches its durable identity: {}",
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
            implementation_identity: "marklab-project-marked-result-import-v1",
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

fn read_marked_result(path: &Path) -> Result<MarkedPatternResult, BayesCliError> {
    let mut file = File::open(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    let mut bytes = Vec::new();
    copy_bounded(&mut file, &mut bytes, MAXIMUM_INPUT_BYTES).map_err(|source| {
        BayesCliError::Io {
            path: path.to_owned(),
            source,
        }
    })?;
    let json = std::str::from_utf8(&bytes)
        .map_err(|error| BayesCliError::Input(format!("invalid result UTF-8: {error}")))?;
    ResultDocument::from_json(json)
        .and_then(ResultDocument::into_marked_pattern)
        .map_err(|error| BayesCliError::Input(error.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn run_normal_mean(
    project_path: PathBuf,
    input_path: PathBuf,
    prior_mean: f64,
    prior_sd: f64,
    known_sigma: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcNormalMean.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_normal_mean(
        input_path.clone(),
        prior_mean,
        prior_sd,
        known_sigma,
        sampling,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "normal-mean input changed while the durable request was prepared".into(),
        ));
    }

    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);

    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = NormalMeanProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_fit(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project normal-mean cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_hierarchical_normal(
    project_path: PathBuf,
    input_path: PathBuf,
    global_prior_mean: f64,
    global_prior_sd: f64,
    between_patient_sd_prior: f64,
    known_sigma: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcHierarchicalNormal.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_hierarchical(
        input_path.clone(),
        global_prior_mean,
        global_prior_sd,
        between_patient_sd_prior,
        known_sigma,
        sampling,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "hierarchical-normal input changed while the durable request was prepared".into(),
        ));
    }

    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);

    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = HierarchicalNormalProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_fit(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project hierarchical-normal cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_beta_binomial_hierarchy(
    project_path: PathBuf,
    input_path: PathBuf,
    population_alpha: f64,
    population_beta: f64,
    concentration_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcBetaBinomialHierarchy.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_beta_binomial_hierarchy(
        input_path.clone(),
        population_alpha,
        population_beta,
        concentration_prior_sd,
        sampling,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "beta-binomial-hierarchy input changed while the durable request was prepared".into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = BetaBinomialHierarchyProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_result(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project beta-binomial-hierarchy cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_beta_binomial_group_regression(
    project_path: PathBuf,
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    group_effect_prior_sd: f64,
    concentration_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcBetaBinomialGroupRegression.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_beta_binomial_group_regression(
        input_path.clone(),
        reference_group,
        comparison_group,
        intercept_prior_mean,
        intercept_prior_sd,
        group_effect_prior_sd,
        concentration_prior_sd,
        sampling,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "beta-binomial-group-regression input changed while the durable request was prepared"
                .into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = BetaBinomialGroupRegressionProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_result(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project beta-binomial-group-regression cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_dirichlet_multinomial_group(
    project_path: PathBuf,
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    logit_prior_sd: f64,
    group_effect_prior_sd: f64,
    concentration_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcDirichletMultinomialGroup.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_dirichlet_multinomial_group(
        input_path.clone(),
        reference_group,
        comparison_group,
        logit_prior_sd,
        group_effect_prior_sd,
        concentration_prior_sd,
        sampling,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "dirichlet-multinomial-group input changed while the durable request was prepared"
                .into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = DirichletMultinomialGroupProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_result(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project dirichlet-multinomial-group cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_ordinal_group(
    project_path: PathBuf,
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    ordered_levels: Vec<String>,
    cutpoint_prior_sd: f64,
    group_effect_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcOrdinalGroup.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_ordinal_group(
        input_path.clone(),
        reference_group,
        comparison_group,
        ordered_levels,
        cutpoint_prior_sd,
        group_effect_prior_sd,
        sampling,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "ordinal-group input changed while the durable request was prepared".into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = OrdinalGroupProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_result(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project ordinal-group cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_ordinal_group_site_hierarchy(
    project_path: PathBuf,
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    ordered_levels: Vec<String>,
    cutpoint_prior_sd: f64,
    group_effect_prior_sd: f64,
    site_intercept_sd_prior_sd: f64,
    site_group_slope_sd_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcOrdinalGroupSiteHierarchy.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_ordinal_group_site_hierarchy(
        input_path.clone(),
        reference_group,
        comparison_group,
        ordered_levels,
        cutpoint_prior_sd,
        group_effect_prior_sd,
        site_intercept_sd_prior_sd,
        site_group_slope_sd_prior_sd,
        sampling,
        timeout_seconds,
    )?;
    if before != source_artifact(&input_path, backend.input_kinds[0])? {
        return Err(BayesCliError::Input(
            "ordinal-group-site-hierarchy input changed while preparing".into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|e| BayesCliError::Backend(e.to_string()))?;
    report_recovery(&durable);
    let request = prepared.request.clone();
    let identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|e| BayesCliError::Input(e.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let node = OrdinalGroupSiteHierarchyProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|e| BayesCliError::Backend(e.to_string()))?;
    let status = if run.cache_status == CacheStatus::Hit {
        "hit"
    } else {
        "miss"
    };
    bayes::publish_json(&output_path, &run.output.into_result(request, identity))?;
    eprintln!("project ordinal-group-site-hierarchy cache_status={status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_nonproportional_ordinal_group_site(
    project_path: PathBuf,
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    ordered_levels: Vec<String>,
    cutpoint_prior_sd: f64,
    site_intercept_sd_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcNonproportionalOrdinalGroupSite.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_nonproportional_ordinal_group_site(
        input_path.clone(),
        reference_group,
        comparison_group,
        ordered_levels,
        cutpoint_prior_sd,
        site_intercept_sd_prior_sd,
        sampling,
        timeout_seconds,
    )?;
    if before != source_artifact(&input_path, backend.input_kinds[0])? {
        return Err(BayesCliError::Input(
            "nonproportional ordinal input changed while preparing".into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|e| BayesCliError::Backend(e.to_string()))?;
    report_recovery(&durable);
    let request = prepared.request.clone();
    let identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|e| BayesCliError::Input(e.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let node =
        NonproportionalOrdinalGroupSiteProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|e| BayesCliError::Input(e.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|e| BayesCliError::Backend(e.to_string()))?;
    let status = if run.cache_status == CacheStatus::Hit {
        "hit"
    } else {
        "miss"
    };
    bayes::publish_json(&output_path, &run.output.into_result(request, identity))?;
    eprintln!("project nonproportional-ordinal-group-site cache_status={status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_hurdle_beta_binomial_group(
    project_path: PathBuf,
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    presence_intercept_prior_mean: f64,
    presence_intercept_prior_sd: f64,
    presence_group_effect_prior_sd: f64,
    abundance_intercept_prior_mean: f64,
    abundance_intercept_prior_sd: f64,
    abundance_group_effect_prior_sd: f64,
    concentration_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcHurdleBetaBinomialGroup.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_hurdle_beta_binomial_group(
        input_path.clone(),
        reference_group,
        comparison_group,
        presence_intercept_prior_mean,
        presence_intercept_prior_sd,
        presence_group_effect_prior_sd,
        abundance_intercept_prior_mean,
        abundance_intercept_prior_sd,
        abundance_group_effect_prior_sd,
        concentration_prior_sd,
        sampling,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "hurdle-beta-binomial-group input changed while the durable request was prepared"
                .into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = HurdleBetaBinomialGroupProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_result(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project hurdle-beta-binomial-group cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_beta_binomial_group_gender_regression(
    project_path: PathBuf,
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    reference_gender: String,
    comparison_gender: String,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    group_effect_prior_sd: f64,
    gender_effect_prior_sd: f64,
    concentration_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcBetaBinomialGroupGenderRegression.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_beta_binomial_group_gender_regression(
        input_path.clone(),
        reference_group,
        comparison_group,
        reference_gender,
        comparison_gender,
        intercept_prior_mean,
        intercept_prior_sd,
        group_effect_prior_sd,
        gender_effect_prior_sd,
        concentration_prior_sd,
        sampling,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "beta-binomial-group-gender-regression input changed while the durable request was prepared"
                .into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node =
        BetaBinomialGroupGenderRegressionProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_result(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project beta-binomial-group-gender-regression cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_beta_binomial_group_gender_slide_hierarchy(
    project_path: PathBuf,
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    reference_gender: String,
    comparison_gender: String,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    group_effect_prior_sd: f64,
    gender_effect_prior_sd: f64,
    patient_log_odds_sd_prior_sd: f64,
    slide_concentration_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcBetaBinomialGroupGenderSlideHierarchy.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_beta_binomial_group_gender_slide_hierarchy(
        input_path.clone(),
        reference_group,
        comparison_group,
        reference_gender,
        comparison_gender,
        intercept_prior_mean,
        intercept_prior_sd,
        group_effect_prior_sd,
        gender_effect_prior_sd,
        patient_log_odds_sd_prior_sd,
        slide_concentration_prior_sd,
        sampling,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "beta-binomial-group-gender-slide-hierarchy input changed while the durable request was prepared"
                .into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = BetaBinomialGroupGenderSlideHierarchyProjectNode::new(
        input_path, before, prepared, backend,
    )?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_result(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project beta-binomial-group-gender-slide-hierarchy cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_student_t_hierarchy(
    project_path: PathBuf,
    input_path: PathBuf,
    global_prior_mean: f64,
    global_prior_sd: f64,
    between_patient_sd_prior: f64,
    observation_sd_prior: f64,
    degrees_of_freedom_excess_rate: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcStudentTHierarchy.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_student_t_hierarchy(
        input_path.clone(),
        global_prior_mean,
        global_prior_sd,
        between_patient_sd_prior,
        observation_sd_prior,
        degrees_of_freedom_excess_rate,
        sampling,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "student-t-hierarchy input changed while the durable request was prepared".into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = StudentTHierarchyProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_result(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project student-t-hierarchy cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_gridded_lgcp(
    project_path: PathBuf,
    events_path: PathBuf,
    grid_path: PathBuf,
    xmin_um: f64,
    ymin_um: f64,
    xmax_um: f64,
    ymax_um: f64,
    grid_x: u32,
    grid_y: u32,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_mean: f64,
    coefficient_prior_sd: f64,
    field_amplitude: f64,
    field_length_scale_um: f64,
    jitter: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcGriddedLgcp.descriptor();
    let before = [
        source_artifact(&events_path, backend.input_kinds[0])?,
        source_artifact(&grid_path, backend.input_kinds[1])?,
    ];
    let prepared = bayes::prepare_gridded_lgcp(
        events_path.clone(),
        grid_path.clone(),
        xmin_um,
        ymin_um,
        xmax_um,
        ymax_um,
        grid_x,
        grid_y,
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_mean,
        coefficient_prior_sd,
        field_amplitude,
        field_length_scale_um,
        jitter,
        sampling,
        timeout_seconds,
        None,
    )?;
    let after = [
        source_artifact(&events_path, backend.input_kinds[0])?,
        source_artifact(&grid_path, backend.input_kinds[1])?,
    ];
    if before != after {
        return Err(BayesCliError::Input(
            "gridded-LGCP input changed while the durable request was prepared".into(),
        ));
    }

    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);

    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    for input in &before {
        project
            .register_reference(input.clone())
            .map_err(|error| BayesCliError::Input(error.to_string()))?;
    }
    let node = GriddedLgcpProjectNode::new(events_path, grid_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_fit(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project gridded-lgcp cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_fused_gromov_wasserstein(
    project_path: PathBuf,
    input_path: PathBuf,
    alpha: f64,
    epsilon: f64,
    feature_scale: f64,
    structure_scale: f64,
    tolerance: f64,
    maximum_iterations: u32,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PotFusedGromovWasserstein.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::fused_gromov::prepare(
        input_path.clone(),
        alpha,
        epsilon,
        feature_scale,
        structure_scale,
        tolerance,
        maximum_iterations,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "FGW input changed while the durable request was prepared".into(),
        ));
    }

    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);

    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = FusedGromovProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::fused_gromov::publish_result(&output_path, &node.prepared, run.output)?;
    eprintln!("project fused-gromov-wasserstein cache_status={cache_status}");
    Ok(())
}

struct SparseRadiusHeatProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    graph_spec: GraphSparseRadiusHeatSpec,
    request_bytes: Vec<u8>,
}

impl SparseRadiusHeatProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        graph_spec: GraphSparseRadiusHeatSpec,
        request_bytes: Vec<u8>,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("sparse-radius-heat")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "graph_signal_transform",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            graph_spec,
            request_bytes,
        })
    }
}

impl WorkflowNode for SparseRadiusHeatProjectNode {
    type Output = GraphSparseRadiusHeatResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(
            &self.input_path,
            "application/vnd.marklab.source.graph-sparse-radius-heat+json;version=1",
        )
        .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "sparse radius heat input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_framed([
                b"marklab-project-sparse-radius-heat-node-v1".as_slice(),
                self.request_bytes.as_slice(),
            ]),
            execution_policy: b"native-safe-rust-sparse-radius-heat-v1",
            implementation_identity: "marklab-project-sparse-radius-heat-node-v1",
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        graph_sparse_radius_heat_workflow(self.graph_spec.clone()).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: GraphSparseRadiusHeatResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        output
            .validate_for_spec(&self.graph_spec)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        "application/vnd.marklab.graph-sparse-radius-heat-result+json;version=1"
    }
}

struct SparseRadiusBasisProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    graph_spec: GraphSparseRadiusBasisSpec,
    request_bytes: Vec<u8>,
}

impl SparseRadiusBasisProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        graph_spec: GraphSparseRadiusBasisSpec,
        request_bytes: Vec<u8>,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("sparse-radius-basis")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "graph_sparse_low_frequency_basis",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            graph_spec,
            request_bytes,
        })
    }
}

impl WorkflowNode for SparseRadiusBasisProjectNode {
    type Output = GraphSparseRadiusBasisResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(
            &self.input_path,
            "application/vnd.marklab.source.graph-sparse-radius-basis+json;version=1",
        )
        .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "sparse radius basis input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_framed([
                b"marklab-project-sparse-radius-basis-node-v1".as_slice(),
                self.request_bytes.as_slice(),
            ]),
            execution_policy: b"native-safe-rust-sparse-radius-basis-v1",
            implementation_identity: "marklab-project-sparse-radius-basis-node-v1",
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        graph_sparse_radius_basis_workflow(self.graph_spec.clone()).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: GraphSparseRadiusBasisResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        output
            .validate_for_spec(&self.graph_spec)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        "application/vnd.marklab.graph-sparse-radius-basis-result+json;version=1"
    }
}

struct SparseRadiusFourierEnergyProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    graph_spec: GraphSparseRadiusFourierEnergySpec,
    request_bytes: Vec<u8>,
}

impl SparseRadiusFourierEnergyProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        graph_spec: GraphSparseRadiusFourierEnergySpec,
        request_bytes: Vec<u8>,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("sparse-radius-fourier-energy")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "graph_sparse_fourier_energy",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            graph_spec,
            request_bytes,
        })
    }
}

impl WorkflowNode for SparseRadiusFourierEnergyProjectNode {
    type Output = GraphSparseRadiusFourierEnergyResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(
            &self.input_path,
            "application/vnd.marklab.source.graph-sparse-radius-fourier-energy+json;version=1",
        )
        .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "sparse radius Fourier energy input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_framed([
                b"marklab-project-sparse-radius-fourier-energy-node-v1".as_slice(),
                self.request_bytes.as_slice(),
            ]),
            execution_policy: b"native-safe-rust-sparse-radius-fourier-energy-v1",
            implementation_identity: "marklab-project-sparse-radius-fourier-energy-node-v1",
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        graph_sparse_radius_fourier_energy_workflow(self.graph_spec.clone())
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: GraphSparseRadiusFourierEnergyResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        output
            .validate_for_spec(&self.graph_spec)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        "application/vnd.marklab.graph-sparse-radius-fourier-energy-result+json;version=1"
    }
}

struct SparseRadiusHeatStabilityProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    graph_spec: GraphSparseRadiusHeatStabilitySpec,
    request_bytes: Vec<u8>,
}

impl SparseRadiusHeatStabilityProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        graph_spec: GraphSparseRadiusHeatStabilitySpec,
        request_bytes: Vec<u8>,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("sparse-radius-heat-stability")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "graph_coordinate_perturbation_stability",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            graph_spec,
            request_bytes,
        })
    }
}

impl WorkflowNode for SparseRadiusHeatStabilityProjectNode {
    type Output = GraphSparseRadiusHeatStabilityResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(
            &self.input_path,
            "application/vnd.marklab.source.graph-sparse-radius-heat-stability+json;version=1",
        )
        .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "sparse radius heat stability input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_framed([
                b"marklab-project-sparse-radius-heat-stability-node-v1".as_slice(),
                self.request_bytes.as_slice(),
            ]),
            execution_policy: b"native-safe-rust-sparse-radius-heat-stability-v1",
            implementation_identity: "marklab-project-sparse-radius-heat-stability-node-v1",
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        graph_sparse_radius_heat_stability_workflow(self.graph_spec.clone())
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: GraphSparseRadiusHeatStabilityResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        output
            .validate_for_spec(&self.graph_spec)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        "application/vnd.marklab.graph-sparse-radius-heat-stability-result+json;version=1"
    }
}

struct SparseRadiusDiffusionWaveletProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    graph_spec: GraphSparseRadiusDiffusionWaveletSpec,
    request_bytes: Vec<u8>,
}

impl SparseRadiusDiffusionWaveletProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        graph_spec: GraphSparseRadiusDiffusionWaveletSpec,
        request_bytes: Vec<u8>,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("sparse-radius-diffusion-wavelet")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "graph_sparse_diffusion_wavelet",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            graph_spec,
            request_bytes,
        })
    }
}

impl WorkflowNode for SparseRadiusDiffusionWaveletProjectNode {
    type Output = GraphSparseRadiusDiffusionWaveletResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(
            &self.input_path,
            "application/vnd.marklab.source.graph-sparse-radius-diffusion-wavelet+json;version=1",
        )
        .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "sparse radius diffusion wavelet input no longer matches its durable identity"
                    .into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_framed([
                b"marklab-project-sparse-radius-diffusion-wavelet-node-v1".as_slice(),
                self.request_bytes.as_slice(),
            ]),
            execution_policy: b"native-safe-rust-sparse-radius-diffusion-wavelet-v1",
            implementation_identity: "marklab-project-sparse-radius-diffusion-wavelet-node-v1",
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        graph_sparse_radius_diffusion_wavelet_workflow(self.graph_spec.clone())
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: GraphSparseRadiusDiffusionWaveletResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        output
            .validate_for_spec(&self.graph_spec)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        "application/vnd.marklab.graph-sparse-radius-diffusion-wavelet-result+json;version=1"
    }
}

struct SparseRadiusScatteringProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    graph_spec: GraphSparseRadiusScatteringSpec,
    request_bytes: Vec<u8>,
}

impl SparseRadiusScatteringProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        graph_spec: GraphSparseRadiusScatteringSpec,
        request_bytes: Vec<u8>,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("sparse-radius-scattering")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "graph_sparse_scattering",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            graph_spec,
            request_bytes,
        })
    }
}

impl WorkflowNode for SparseRadiusScatteringProjectNode {
    type Output = GraphSparseRadiusScatteringResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(
            &self.input_path,
            "application/vnd.marklab.source.graph-sparse-radius-scattering+json;version=1",
        )
        .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "sparse radius scattering input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_framed([
                b"marklab-project-sparse-radius-scattering-node-v1".as_slice(),
                self.request_bytes.as_slice(),
            ]),
            execution_policy: b"native-safe-rust-sparse-radius-scattering-v1",
            implementation_identity: "marklab-project-sparse-radius-scattering-node-v1",
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        graph_sparse_radius_scattering_workflow(self.graph_spec.clone())
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: GraphSparseRadiusScatteringResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        output
            .validate_for_spec(&self.graph_spec)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        "application/vnd.marklab.graph-sparse-radius-scattering-result+json;version=1"
    }
}

struct WitnessPersistenceStabilityProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedWitnessPersistenceStability,
}

impl WitnessPersistenceStabilityProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedWitnessPersistenceStability,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("gudhi-witness-persistence-stability")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "topological_witness_coordinate_stability",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
        })
    }
}

impl WorkflowNode for WitnessPersistenceStabilityProjectNode {
    type Output = WitnessPersistenceStabilityResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(
            &self.input_path,
            "application/vnd.marklab.source.witness-persistence-stability+json;version=1",
        )
        .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "witness persistence stability input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        let mut frames = Vec::with_capacity(self.prepared.runs.len() + 1);
        frames.push(b"marklab-project-witness-persistence-stability-node-v1".as_slice());
        frames.extend(
            self.prepared
                .runs
                .iter()
                .map(|run| run.request_bytes.as_slice()),
        );
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_framed(frames),
            execution_policy: b"bounded-gudhi-witness-persistence-stability-v1",
            implementation_identity: "marklab-project-witness-persistence-stability-node-v1",
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        topology::execute_witness_persistence_stability(&self.prepared)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: WitnessPersistenceStabilityResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        topology::validate_witness_persistence_stability_result(&self.prepared, &output)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        "application/vnd.marklab.gudhi-witness-persistence-stability-result+json;version=1"
    }
}

struct WitnessPersistenceBottleneckStabilityProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedWitnessPersistenceBottleneckStability,
}

impl WitnessPersistenceBottleneckStabilityProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedWitnessPersistenceBottleneckStability,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("gudhi-witness-persistence-bottleneck-stability")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "topological_witness_coordinate_bottleneck_stability",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
        })
    }
}

impl WorkflowNode for WitnessPersistenceBottleneckStabilityProjectNode {
    type Output = WitnessPersistenceBottleneckStabilityResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(
            &self.input_path,
            "application/vnd.marklab.source.witness-persistence-bottleneck-stability+json;version=1",
        )
        .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "witness bottleneck stability input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        let mut frames = Vec::with_capacity(self.prepared.stability.runs.len() + 3);
        frames.push(b"marklab-project-witness-persistence-bottleneck-stability-node-v1".as_slice());
        frames.extend(
            self.prepared
                .stability
                .runs
                .iter()
                .map(|run| run.request_bytes.as_slice()),
        );
        frames.push(self.prepared.backend.environment_lock_sha256.as_bytes());
        frames.push(self.prepared.backend.worker_sha256.as_bytes());
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_framed(frames),
            execution_policy: b"bounded-gudhi-witness-persistence-bottleneck-stability-v1",
            implementation_identity:
                "marklab-project-witness-persistence-bottleneck-stability-node-v1",
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        topology::execute_witness_persistence_bottleneck_stability(&self.prepared)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: WitnessPersistenceBottleneckStabilityResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        topology::validate_witness_persistence_bottleneck_stability_result(&self.prepared, &output)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        "application/vnd.marklab.gudhi-witness-persistence-bottleneck-stability-result+json;version=1"
    }
}

struct WitnessPersistenceProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedWitnessPersistence,
}

impl WitnessPersistenceProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedWitnessPersistence,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("gudhi-witness-persistence")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "topological_witness_approximation",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
        })
    }
}

impl WorkflowNode for WitnessPersistenceProjectNode {
    type Output = WitnessPersistenceResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(
            &self.input_path,
            "application/vnd.marklab.source.witness-persistence+json;version=1",
        )
        .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "witness persistence input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_framed([
                b"marklab-project-gudhi-witness-persistence-node-v1".as_slice(),
                self.prepared.request_bytes.as_slice(),
            ]),
            execution_policy: b"bounded-pinned-gudhi-subprocess-v1",
            implementation_identity: "marklab-project-gudhi-witness-persistence-node-v1",
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        topology::execute_witness_persistence(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: WitnessPersistenceResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_bytes)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        "application/vnd.marklab.gudhi-witness-persistence-result+json;version=1"
    }
}

struct ArbitraryWindowIppProjectNode {
    spec: NodeSpec,
    input_paths: [PathBuf; 3],
    input_artifacts: [ArtifactRef; 3],
    prepared: PreparedArbitraryWindowIpp,
    configuration_bytes: Vec<u8>,
}

impl ArbitraryWindowIppProjectNode {
    fn new(
        input_paths: [PathBuf; 3],
        input_artifacts: [ArtifactRef; 3],
        prepared: PreparedArbitraryWindowIpp,
        configuration_bytes: Vec<u8>,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("arbitrary-window-ipp-likelihood")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "fixed_point_process_likelihood",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_paths,
            input_artifacts,
            prepared,
            configuration_bytes,
        })
    }
}

impl WorkflowNode for ArbitraryWindowIppProjectNode {
    type Output = ArbitraryWindowIppLikelihoodResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let kinds = [
            "application/vnd.marklab.source.arbitrary-window-ipp-events+csv;version=1",
            "application/vnd.marklab.source.arbitrary-window-ipp-quadrature+csv;version=1",
            "application/vnd.marklab.source.observation-window+geojson;version=1",
        ];
        for ((path, artifact), kind) in self
            .input_paths
            .iter()
            .zip(&self.input_artifacts)
            .zip(kinds)
        {
            let observed = source_artifact(path, kind).map_err(NodeError::input)?;
            if &observed != artifact {
                return Err(NodeError::input(BayesCliError::Input(
                    "arbitrary-window IPP source no longer matches its durable identity".into(),
                )));
            }
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_framed([
                b"marklab-project-arbitrary-window-ipp-likelihood-node-v1".as_slice(),
                self.configuration_bytes.as_slice(),
            ]),
            execution_policy: b"native-safe-rust-weighted-arbitrary-window-ipp-v1",
            implementation_identity: "marklab-project-arbitrary-window-ipp-likelihood-node-v1",
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_arbitrary_window_ipp(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: ArbitraryWindowIppLikelihoodResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        output
            .validate_for_spec(&self.prepared.window, &self.prepared.spec)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        "application/vnd.marklab.arbitrary-window-ipp-likelihood-result+json;version=1"
    }
}

struct ArbitraryWindowIppFitProjectNode {
    spec: NodeSpec,
    input_paths: [PathBuf; 3],
    input_artifacts: [ArtifactRef; 3],
    prepared: PreparedArbitraryWindowIppFit,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl ArbitraryWindowIppFitProjectNode {
    fn new(
        input_paths: [PathBuf; 3],
        input_artifacts: [ArtifactRef; 3],
        prepared: PreparedArbitraryWindowIppFit,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_paths,
            input_artifacts,
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for ArbitraryWindowIppFitProjectNode {
    type Output = ArbitraryWindowIppFitResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        for ((path, artifact), kind) in self
            .input_paths
            .iter()
            .zip(&self.input_artifacts)
            .zip(self.backend.input_kinds)
        {
            let observed = source_artifact(path, kind).map_err(NodeError::input)?;
            if &observed != artifact {
                return Err(NodeError::input(BayesCliError::Input(
                    "arbitrary-window IPP fit source no longer matches its durable identity".into(),
                )));
            }
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_arbitrary_window_ipp_fit(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: ArbitraryWindowIppFitResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        output.validate(&self.prepared).map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct ArbitraryWindowLgcpProjectNode {
    spec: NodeSpec,
    input_paths: [PathBuf; 4],
    input_artifacts: [ArtifactRef; 4],
    prepared: bayes::PreparedArbitraryWindowLgcpFit,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl ArbitraryWindowLgcpProjectNode {
    fn new(
        input_paths: [PathBuf; 4],
        input_artifacts: [ArtifactRef; 4],
        prepared: bayes::PreparedArbitraryWindowLgcpFit,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_paths,
            input_artifacts,
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for ArbitraryWindowLgcpProjectNode {
    type Output = bayes::ArbitraryWindowLgcpFitResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        for ((path, artifact), kind) in self
            .input_paths
            .iter()
            .zip(&self.input_artifacts)
            .zip(self.backend.input_kinds)
        {
            let observed = source_artifact(path, kind).map_err(NodeError::input)?;
            if &observed != artifact {
                return Err(NodeError::input(BayesCliError::Input(
                    "arbitrary-window LGCP source no longer matches its durable identity".into(),
                )));
            }
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_arbitrary_window_lgcp_fit(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: bayes::ArbitraryWindowLgcpFitResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        output
            .validate_for(&self.prepared)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct ReplicatedArbitraryWindowLgcpProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifact: ArtifactRef,
    prepared: bayes::PreparedReplicatedArbitraryWindowLgcpFit,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl ReplicatedArbitraryWindowLgcpProjectNode {
    fn new(
        input_path: PathBuf,
        input_artifact: ArtifactRef,
        prepared: bayes::PreparedReplicatedArbitraryWindowLgcpFit,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifact,
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for ReplicatedArbitraryWindowLgcpProjectNode {
    type Output = bayes::ReplicatedArbitraryWindowLgcpFitResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        std::slice::from_ref(&self.input_artifact)
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifact {
            return Err(NodeError::input(BayesCliError::Input(
                "replicated LGCP source no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_replicated_arbitrary_window_lgcp_fit(&self.prepared)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: bayes::ReplicatedArbitraryWindowLgcpFitResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        output
            .validate_for(&self.prepared)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct ReplicatedArbitraryWindowMultitypeLgcpProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifact: ArtifactRef,
    prepared: bayes::PreparedReplicatedArbitraryWindowMultitypeLgcp,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl ReplicatedArbitraryWindowMultitypeLgcpProjectNode {
    fn new(
        input_path: PathBuf,
        input_artifact: ArtifactRef,
        prepared: bayes::PreparedReplicatedArbitraryWindowMultitypeLgcp,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(prepared.backend())?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifact,
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for ReplicatedArbitraryWindowMultitypeLgcpProjectNode {
    type Output = bayes::ReplicatedArbitraryWindowMultitypeLgcpResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        std::slice::from_ref(&self.input_artifact)
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifact {
            return Err(NodeError::input(BayesCliError::Input(
                "replicated multitype LGCP source no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(self.prepared.backend(), self.prepared.request_bytes()),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_replicated_arbitrary_window_multitype_lgcp(&self.prepared)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: bayes::ReplicatedArbitraryWindowMultitypeLgcpResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        output
            .validate_for(&self.prepared)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct ReplicatedArbitraryWindowLgcpInferredKernelProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifact: ArtifactRef,
    prepared: bayes::PreparedReplicatedArbitraryWindowLgcpInferredKernel,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

struct ReplicatedArbitraryWindowMultitypeLgcpInferredKernelProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifact: ArtifactRef,
    prepared: bayes::PreparedReplicatedArbitraryWindowMultitypeLgcpInferredKernel,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl ReplicatedArbitraryWindowMultitypeLgcpInferredKernelProjectNode {
    fn new(
        input_path: PathBuf,
        input_artifact: ArtifactRef,
        prepared: bayes::PreparedReplicatedArbitraryWindowMultitypeLgcpInferredKernel,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(prepared.backend())?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifact,
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for ReplicatedArbitraryWindowMultitypeLgcpInferredKernelProjectNode {
    type Output = bayes::ReplicatedArbitraryWindowMultitypeLgcpInferredKernelOutput;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        std::slice::from_ref(&self.input_artifact)
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifact {
            return Err(NodeError::input(BayesCliError::Input(
                "replicated multitype inferred-kernel source no longer matches durable identity"
                    .into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(self.prepared.backend(), self.prepared.request_bytes()),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_replicated_arbitrary_window_multitype_lgcp_inferred_kernel(&self.prepared)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: bayes::ReplicatedArbitraryWindowMultitypeLgcpInferredKernelOutput =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        output
            .validate_for(&self.prepared)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

impl ReplicatedArbitraryWindowLgcpInferredKernelProjectNode {
    fn new(
        input_path: PathBuf,
        input_artifact: ArtifactRef,
        prepared: bayes::PreparedReplicatedArbitraryWindowLgcpInferredKernel,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(prepared.backend())?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifact,
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for ReplicatedArbitraryWindowLgcpInferredKernelProjectNode {
    type Output = bayes::ReplicatedArbitraryWindowLgcpInferredKernelResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        std::slice::from_ref(&self.input_artifact)
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifact {
            return Err(NodeError::input(BayesCliError::Input(
                "inferred-kernel replicated LGCP source no longer matches durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(self.prepared.backend(), self.prepared.request_bytes()),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_replicated_arbitrary_window_lgcp_inferred_kernel(&self.prepared)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: bayes::ReplicatedArbitraryWindowLgcpInferredKernelResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        output
            .validate_for(&self.prepared)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct ConditionalMultitypeMarkProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifact: ArtifactRef,
    arguments: bayes::ConditionalMultitypeMarkArgs,
    input_sha256: String,
    request_backend: BackendContract,
    configuration_bytes: Vec<u8>,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl ConditionalMultitypeMarkProjectNode {
    #[allow(clippy::too_many_arguments)]
    fn new(
        input_path: PathBuf,
        input_artifact: ArtifactRef,
        arguments: bayes::ConditionalMultitypeMarkArgs,
        input_sha256: String,
        request_backend: BackendContract,
        configuration_bytes: Vec<u8>,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&request_backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifact,
            arguments,
            input_sha256,
            request_backend,
            configuration_bytes,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for ConditionalMultitypeMarkProjectNode {
    type Output = bayes::ConditionalMultitypeMarkResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        std::slice::from_ref(&self.input_artifact)
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifact {
            return Err(NodeError::input(BayesCliError::Input(
                "conditional multitype source no longer matches durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.request_backend, &self.configuration_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_conditional_multitype_mark(self.arguments.clone())
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: bayes::ConditionalMultitypeMarkResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        output
            .validate_for(
                &self.input_sha256,
                &self.arguments.reference_type,
                self.arguments.radius_um,
                &self.request_backend,
            )
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct ReplicatedConditionalMultitypeMarkProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifact: ArtifactRef,
    arguments: bayes::ReplicatedConditionalMultitypeMarkArgs,
    input_sha256: String,
    request_backend: BackendContract,
    configuration_bytes: Vec<u8>,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl ReplicatedConditionalMultitypeMarkProjectNode {
    #[allow(clippy::too_many_arguments)]
    fn new(
        input_path: PathBuf,
        input_artifact: ArtifactRef,
        arguments: bayes::ReplicatedConditionalMultitypeMarkArgs,
        input_sha256: String,
        request_backend: BackendContract,
        configuration_bytes: Vec<u8>,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&request_backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifact,
            arguments,
            input_sha256,
            request_backend,
            configuration_bytes,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for ReplicatedConditionalMultitypeMarkProjectNode {
    type Output = bayes::ReplicatedConditionalMultitypeMarkResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        std::slice::from_ref(&self.input_artifact)
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifact {
            return Err(NodeError::input(BayesCliError::Input(
                "replicated conditional-mark source no longer matches durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.request_backend, &self.configuration_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_replicated_conditional_multitype_mark(self.arguments.clone())
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: bayes::ReplicatedConditionalMultitypeMarkResult =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        output
            .validate_for(
                &self.input_sha256,
                &self.arguments.reference_group,
                &self.arguments.reference_type,
                self.arguments.radius_um,
                &self.request_backend,
            )
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct NormalMeanProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedNormalMean,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl NormalMeanProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedNormalMean,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for NormalMeanProjectNode {
    type Output = WorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "normal-mean input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_normal_mean(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: WorkerResult = serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct HierarchicalNormalProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedGaussianHierarchy,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl HierarchicalNormalProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedGaussianHierarchy,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for HierarchicalNormalProjectNode {
    type Output = HierarchicalWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "hierarchical-normal input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_hierarchical(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: HierarchicalWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct BetaBinomialHierarchyProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedBetaBinomialHierarchy,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl BetaBinomialHierarchyProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedBetaBinomialHierarchy,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for BetaBinomialHierarchyProjectNode {
    type Output = BetaBinomialHierarchyWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "beta-binomial-hierarchy input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_beta_binomial_hierarchy(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: BetaBinomialHierarchyWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct BetaBinomialGroupRegressionProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedBetaBinomialGroupRegression,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl BetaBinomialGroupRegressionProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedBetaBinomialGroupRegression,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for BetaBinomialGroupRegressionProjectNode {
    type Output = BetaBinomialGroupRegressionWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "beta-binomial-group-regression input no longer matches its durable identity"
                    .into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_beta_binomial_group_regression(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: BetaBinomialGroupRegressionWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct DirichletMultinomialGroupProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedDirichletMultinomialGroup,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl DirichletMultinomialGroupProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedDirichletMultinomialGroup,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for DirichletMultinomialGroupProjectNode {
    type Output = DirichletMultinomialGroupWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "dirichlet-multinomial-group input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_dirichlet_multinomial_group(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: DirichletMultinomialGroupWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct OrdinalGroupProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedOrdinalGroup,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl OrdinalGroupProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedOrdinalGroup,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for OrdinalGroupProjectNode {
    type Output = OrdinalGroupWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "ordinal-group input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_ordinal_group(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: OrdinalGroupWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct OrdinalGroupSiteHierarchyProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedOrdinalGroupSiteHierarchy,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}
impl OrdinalGroupSiteHierarchyProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedOrdinalGroupSiteHierarchy,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id).map_err(|e| BayesCliError::Input(e.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|e| BayesCliError::Input(e.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}
impl WorkflowNode for OrdinalGroupSiteHierarchyProjectNode {
    type Output = OrdinalGroupSiteHierarchyWorkerResult;
    fn spec(&self) -> &NodeSpec {
        &self.spec
    }
    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }
    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "ordinal-group-site-hierarchy input no longer matches durable identity".into(),
            )));
        }
        Ok(())
    }
    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }
    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_ordinal_group_site_hierarchy(&self.prepared).map_err(NodeError::execution)
    }
    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }
    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: Self::Output = serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }
    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct NonproportionalOrdinalGroupSiteProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedNonproportionalOrdinalGroupSite,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}
impl NonproportionalOrdinalGroupSiteProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedNonproportionalOrdinalGroupSite,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id).map_err(|e| BayesCliError::Input(e.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|e| BayesCliError::Input(e.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}
impl WorkflowNode for NonproportionalOrdinalGroupSiteProjectNode {
    type Output = NonproportionalOrdinalGroupSiteWorkerResult;
    fn spec(&self) -> &NodeSpec {
        &self.spec
    }
    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }
    fn verify_input_content(&self) -> Result<(), NodeError> {
        let x = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if x != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "nonproportional ordinal input identity changed".into(),
            )));
        }
        Ok(())
    }
    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }
    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_nonproportional_ordinal_group_site(&self.prepared)
            .map_err(NodeError::execution)
    }
    fn encode_output(&self, o: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(o)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }
    fn decode_output(&self, b: &[u8]) -> Result<Self::Output, NodeError> {
        let o: Self::Output = serde_json::from_slice(b).map_err(NodeError::decode)?;
        o.validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(o)
    }
    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct HurdleBetaBinomialGroupProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedHurdleBetaBinomialGroup,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl HurdleBetaBinomialGroupProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedHurdleBetaBinomialGroup,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for HurdleBetaBinomialGroupProjectNode {
    type Output = HurdleBetaBinomialGroupWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "hurdle-beta-binomial-group input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_hurdle_beta_binomial_group(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: HurdleBetaBinomialGroupWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct BetaBinomialGroupGenderRegressionProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedBetaBinomialGroupGenderRegression,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl BetaBinomialGroupGenderRegressionProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedBetaBinomialGroupGenderRegression,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for BetaBinomialGroupGenderRegressionProjectNode {
    type Output = BetaBinomialGroupGenderRegressionWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "beta-binomial-group-gender-regression input no longer matches its durable identity"
                    .into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_beta_binomial_group_gender_regression(&self.prepared)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: BetaBinomialGroupGenderRegressionWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct BetaBinomialGroupGenderSlideHierarchyProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedBetaBinomialGroupGenderSlideHierarchy,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl BetaBinomialGroupGenderSlideHierarchyProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedBetaBinomialGroupGenderSlideHierarchy,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for BetaBinomialGroupGenderSlideHierarchyProjectNode {
    type Output = BetaBinomialGroupGenderSlideHierarchyWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "beta-binomial-group-gender-slide-hierarchy input no longer matches its durable identity"
                    .into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_beta_binomial_group_gender_slide_hierarchy(&self.prepared)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: BetaBinomialGroupGenderSlideHierarchyWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct StudentTHierarchyProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedStudentTHierarchy,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl StudentTHierarchyProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedStudentTHierarchy,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for StudentTHierarchyProjectNode {
    type Output = StudentTHierarchyWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "student-t-hierarchy input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_student_t_hierarchy(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: StudentTHierarchyWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct GriddedLgcpProjectNode {
    spec: NodeSpec,
    input_paths: [PathBuf; 2],
    input_artifacts: [ArtifactRef; 2],
    prepared: PreparedGriddedLgcpFit,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl GriddedLgcpProjectNode {
    fn new(
        events_path: PathBuf,
        grid_path: PathBuf,
        inputs: [ArtifactRef; 2],
        prepared: PreparedGriddedLgcpFit,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_paths: [events_path, grid_path],
            input_artifacts: inputs,
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for GriddedLgcpProjectNode {
    type Output = GriddedLgcpFitWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        for index in 0..self.input_paths.len() {
            let observed =
                source_artifact(&self.input_paths[index], self.backend.input_kinds[index])
                    .map_err(NodeError::input)?;
            if observed != self.input_artifacts[index] {
                return Err(NodeError::input(BayesCliError::Input(
                    "gridded-LGCP input no longer matches its durable identity".into(),
                )));
            }
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_gridded_lgcp(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: GriddedLgcpFitWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct FusedGromovProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedFusedGromovWasserstein,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl FusedGromovProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedFusedGromovWasserstein,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for FusedGromovProjectNode {
    type Output = FusedGromovWassersteinWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "FGW input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::fused_gromov::execute(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        let mut encoded = serde_json::to_vec(output).map_err(NodeError::encoding)?;
        for _ in 0..8 {
            let normalized: FusedGromovWassersteinWorkerResult =
                serde_json::from_slice(&encoded).map_err(NodeError::encoding)?;
            let next = serde_json::to_vec(&normalized).map_err(NodeError::encoding)?;
            if next == encoded {
                return Ok(next.into_boxed_slice());
            }
            encoded = next;
        }
        Err(NodeError::encoding(BayesCliError::Input(
            "typed POT result JSON did not reach a stable canonical representation".into(),
        )))
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: FusedGromovWassersteinWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

fn source_artifact(path: &Path, kind: &str) -> Result<ArtifactRef, BayesCliError> {
    let mut file = File::open(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    let mut digest = ContentDigest::builder();
    let copied = copy_bounded(&mut file, &mut digest, MAXIMUM_INPUT_BYTES).map_err(|source| {
        BayesCliError::Io {
            path: path.to_owned(),
            source,
        }
    })?;
    let (digest, byte_len) = digest.finish();
    if copied != byte_len {
        return Err(BayesCliError::Input(
            "source hashing byte count mismatch".into(),
        ));
    }
    ArtifactRef::new(kind, digest, byte_len)
        .map_err(|error| BayesCliError::Input(error.to_string()))
}

fn topology_error(error: TopologyCliError) -> BayesCliError {
    match error {
        TopologyCliError::Input(message) => BayesCliError::Input(message),
        TopologyCliError::Backend(message) => BayesCliError::Backend(message),
        TopologyCliError::Io { path, source } => BayesCliError::Io { path, source },
        TopologyCliError::Json(error) => BayesCliError::Json(error),
    }
}

pub(super) fn native_runtime_provenance() -> Result<NativeRuntimeProvenance, BayesCliError> {
    let executable = executable_artifact()?;
    let git_sha = match env!("MARKLAB_BUILD_GIT_SHA") {
        "" => None,
        value => Some(value.to_owned()),
    };
    let git_dirty = match env!("MARKLAB_BUILD_GIT_DIRTY") {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    };
    NativeRuntimeProvenance::new(
        env!("CARGO_PKG_VERSION"),
        git_sha,
        git_dirty,
        env!("MARKLAB_BUILD_RUSTC"),
        compiled_features(),
        executable,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))
}

fn executable_artifact() -> Result<ArtifactRef, BayesCliError> {
    let path = std::env::current_exe().map_err(|source| BayesCliError::Io {
        path: PathBuf::from("current executable"),
        source,
    })?;
    let mut file = File::open(&path).map_err(|source| BayesCliError::Io {
        path: path.clone(),
        source,
    })?;
    let mut digest = ContentDigest::builder();
    let copied =
        copy_bounded(&mut file, &mut digest, MAXIMUM_EXECUTABLE_BYTES).map_err(|source| {
            BayesCliError::Io {
                path: path.clone(),
                source,
            }
        })?;
    let (digest, byte_len) = digest.finish();
    if copied != byte_len {
        return Err(BayesCliError::Input(
            "executable hashing byte count mismatch".into(),
        ));
    }
    ArtifactRef::new("application/vnd.marklab.executable", digest, byte_len)
        .map_err(|error| BayesCliError::Input(error.to_string()))
}

fn copy_bounded(
    reader: &mut dyn Read,
    writer: &mut dyn Write,
    maximum: u64,
) -> std::io::Result<u64> {
    let mut limited = reader.take(maximum.saturating_add(1));
    let copied = std::io::copy(&mut limited, writer)?;
    if copied > maximum {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "input exceeds its provenance hashing limit",
        ));
    }
    Ok(copied)
}

fn compiled_features() -> Vec<String> {
    let candidates = [
        (cfg!(feature = "allocator-mimalloc"), "allocator-mimalloc"),
        (true, "cli"),
        (cfg!(feature = "csv"), "csv"),
        (cfg!(feature = "dhat-heap"), "dhat-heap"),
        (cfg!(feature = "parallel"), "parallel"),
        (cfg!(feature = "parquet"), "parquet"),
        (cfg!(feature = "wsi"), "wsi"),
    ];
    candidates
        .into_iter()
        .filter_map(|(enabled, name)| enabled.then_some(name.to_owned()))
        .collect()
}

fn report_recovery(project: &DurableProject) {
    let report = project.open_report();
    if report.action() != DurableRecoveryAction::None
        || !report.artifact_store().quarantined().is_empty()
        || !report.artifact_store().issues().is_empty()
    {
        eprintln!(
            "project recovery: action={:?}, quarantined_objects={}, retained_issues={}",
            report.action(),
            report.artifact_store().quarantined().len(),
            report.artifact_store().issues().len()
        );
    }
}

pub(crate) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<ProjectCli>(|| run_cli().map_err(bayes::into_marklab_error))
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
            python_version: "3.12",
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
