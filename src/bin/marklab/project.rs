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

mod worker_contract;
use marklab_core::backend::is_sha256;
use worker_contract::StaticBackendWorkflow;

mod graph_transforms;
use graph_transforms::{
    run_sparse_radius_basis, run_sparse_radius_diffusion_wavelet, run_sparse_radius_fourier_energy,
    run_sparse_radius_heat, run_sparse_radius_heat_stability, run_sparse_radius_scattering,
};

mod witness;
use witness::{
    run_witness_persistence, run_witness_persistence_bottleneck_stability,
    run_witness_persistence_stability,
};

mod spatial_models;
use spatial_models::{
    run_arbitrary_window_ipp, run_arbitrary_window_ipp_fit, run_arbitrary_window_lgcp,
    run_conditional_multitype_mark, run_replicated_arbitrary_window_lgcp,
    run_replicated_arbitrary_window_lgcp_inferred_kernel,
    run_replicated_arbitrary_window_multitype_lgcp,
    run_replicated_arbitrary_window_multitype_lgcp_inferred_kernel,
    run_replicated_conditional_multitype_mark,
};

mod marked_prepost;
use marked_prepost::run_marked_prepost;

mod hierarchical_models;
use hierarchical_models::{
    run_beta_binomial_group_gender_regression, run_beta_binomial_group_gender_slide_hierarchy,
    run_beta_binomial_group_regression, run_beta_binomial_hierarchy,
    run_dirichlet_multinomial_group, run_fused_gromov_wasserstein, run_gridded_lgcp,
    run_hierarchical_normal, run_hurdle_beta_binomial_group,
    run_nonproportional_ordinal_group_site, run_normal_mean, run_ordinal_group,
    run_ordinal_group_site_hierarchy, run_student_t_hierarchy,
};

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

pub(crate) fn execute(command: ProjectCommand) -> marklab::Result<()> {
    run_cli(command).map_err(bayes::into_marklab_error)
}
