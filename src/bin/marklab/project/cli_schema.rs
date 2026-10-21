//! Project command arguments; execution remains in the existing workflow adapters.

use std::path::PathBuf;

use clap::{Subcommand, ValueEnum};

use crate::cohort::{CliEnergyMetric, CliMmdEstimator, CliMmdKernel};

use super::{
    correlated_multitype_lgcp, gaussian_crossed_nested_hierarchy, joint_location_embedding,
    joint_location_mark, multitype_lgcp_sbc,
};
use crate::bayes::NutsArgs;

#[derive(Debug, clap::Args)]
pub(crate) struct ArbitraryWindowIppProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// Observed point events with coordinates and model covariates.
    #[arg(long)]
    pub(super) events: PathBuf,
    /// Quadrature nodes, weights and covariates for spatial integration.
    #[arg(long)]
    pub(super) quadrature: PathBuf,
    /// Observation-window geometry used for spatial integration or edge correction.
    #[arg(long)]
    pub(super) window: PathBuf,
    /// Intercept on the model's linear predictor scale.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept: f64,
    /// Covariate coefficient on the model's linear predictor scale.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) coefficient: f64,
    /// Upper bound on events.
    #[arg(long)]
    pub(super) maximum_events: usize,
    /// Upper bound on quadrature nodes.
    #[arg(long)]
    pub(super) maximum_quadrature_nodes: usize,
    /// Upper bound on work.
    #[arg(long)]
    pub(super) maximum_work: usize,
    /// Upper bound on retained bytes.
    #[arg(long, default_value_t = 16 * 1024 * 1024)]
    pub(super) maximum_retained_bytes: usize,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct ArbitraryWindowIppFitProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// Observed point events with coordinates and model covariates.
    #[arg(long)]
    pub(super) events: PathBuf,
    /// Quadrature nodes, weights and covariates for spatial integration.
    #[arg(long)]
    pub(super) quadrature: PathBuf,
    /// Observation-window geometry used for spatial integration or edge correction.
    #[arg(long)]
    pub(super) window: PathBuf,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    pub(super) intercept_prior_sd: f64,
    /// Prior mean for coefficient.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) coefficient_prior_mean: f64,
    /// Prior scale for coefficient.
    #[arg(long)]
    pub(super) coefficient_prior_sd: f64,
    #[command(flatten)]
    pub(super) nuts: NutsArgs,
    /// Upper bound on events.
    #[arg(long)]
    pub(super) maximum_events: usize,
    /// Upper bound on quadrature nodes.
    #[arg(long)]
    pub(super) maximum_quadrature_nodes: usize,
    /// Upper bound on draw node work.
    #[arg(long)]
    pub(super) maximum_draw_node_work: u64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct ArbitraryWindowIppSpatialPpcProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// Observed point events with coordinates and model covariates.
    #[arg(long)]
    pub(super) events: PathBuf,
    /// Table binding observed events to their declared components or quadrature cells.
    #[arg(long)]
    pub(super) event_membership: PathBuf,
    /// Quadrature nodes, weights and covariates for spatial integration.
    #[arg(long)]
    pub(super) quadrature: PathBuf,
    /// Observation-window geometry used for spatial integration or edge correction.
    #[arg(long)]
    pub(super) window: PathBuf,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    pub(super) intercept_prior_sd: f64,
    /// Prior mean for coefficient.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) coefficient_prior_mean: f64,
    /// Prior scale for coefficient.
    #[arg(long)]
    pub(super) coefficient_prior_sd: f64,
    /// Number of independent posterior sampling chains.
    #[arg(long)]
    pub(super) chains: u32,
    /// Warmup iterations per posterior sampling chain.
    #[arg(long)]
    pub(super) tune: u32,
    /// Posterior draws retained per chain after warmup.
    #[arg(long)]
    pub(super) draws: u32,
    /// Target NUTS acceptance probability.
    #[arg(long)]
    pub(super) target_accept: f64,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Random seed for posterior predictive simulation.
    #[arg(long)]
    pub(super) prediction_seed: u64,
    /// Neighborhood radius in micrometers.
    #[arg(long)]
    pub(super) neighbor_radius_um: f64,
    /// Upper bound on events.
    #[arg(long)]
    pub(super) maximum_events: usize,
    /// Upper bound on quadrature nodes.
    #[arg(long)]
    pub(super) maximum_quadrature_nodes: usize,
    /// Upper bound on neighbor pairs.
    #[arg(long)]
    pub(super) maximum_neighbor_pairs: usize,
    /// Upper bound on draw node work.
    #[arg(long)]
    pub(super) maximum_draw_node_work: u64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct ArbitraryWindowLgcpFitProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// Observed point events with coordinates and model covariates.
    #[arg(long)]
    pub(super) events: PathBuf,
    /// Table binding observed events to their declared components or quadrature cells.
    #[arg(long)]
    pub(super) event_membership: PathBuf,
    /// Quadrature nodes, weights and covariates for spatial integration.
    #[arg(long)]
    pub(super) quadrature: PathBuf,
    /// Observation-window geometry used for spatial integration or edge correction.
    #[arg(long)]
    pub(super) window: PathBuf,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    pub(super) intercept_prior_sd: f64,
    /// Prior mean for coefficient.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) coefficient_prior_mean: f64,
    /// Prior scale for coefficient.
    #[arg(long)]
    pub(super) coefficient_prior_sd: f64,
    /// Amplitude of the latent spatial field covariance.
    #[arg(long)]
    pub(super) field_amplitude: f64,
    /// Spatial covariance length scale in micrometers.
    #[arg(long)]
    pub(super) field_length_scale_um: f64,
    /// Diagonal stabilization added to covariance matrices.
    #[arg(long)]
    pub(super) jitter: f64,
    #[command(flatten)]
    pub(super) nuts: NutsArgs,
    /// Upper bound on events.
    #[arg(long)]
    pub(super) maximum_events: usize,
    /// Upper bound on quadrature nodes.
    #[arg(long)]
    pub(super) maximum_quadrature_nodes: usize,
    /// Upper bound on draw node work.
    #[arg(long)]
    pub(super) maximum_draw_node_work: u64,
    /// Number of posterior predictive simulations.
    #[arg(long)]
    pub(super) prediction_replicates: u32,
    /// Random seed for posterior predictive simulation.
    #[arg(long)]
    pub(super) prediction_seed: u64,
    /// Upper bound on predictive points.
    #[arg(long)]
    pub(super) maximum_predictive_points: u64,
    /// Neighborhood radius in micrometers.
    #[arg(long)]
    pub(super) neighbor_radius_um: f64,
    /// Upper bound on neighbor pairs.
    #[arg(long)]
    pub(super) maximum_neighbor_pairs: usize,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct ReplicatedArbitraryWindowLgcpFitProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// Per-node quadrature CSV (pattern_id, patient_id, group, node_id, x_um, y_um, count, ...).
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    pub(super) reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    pub(super) comparison_group: String,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    pub(super) intercept_prior_sd: f64,
    /// Prior scale for group effect.
    #[arg(long)]
    pub(super) group_effect_prior_sd: f64,
    /// Prior scale for covariate effect.
    #[arg(long)]
    pub(super) covariate_effect_prior_sd: f64,
    /// Prior scale for patient standard deviation.
    #[arg(long)]
    pub(super) patient_sd_prior_scale: f64,
    /// Prior scale for pattern standard deviation.
    #[arg(long)]
    pub(super) pattern_sd_prior_scale: f64,
    /// Amplitude of the latent spatial field covariance.
    #[arg(long)]
    pub(super) field_amplitude: f64,
    /// Spatial covariance length scale in micrometers.
    #[arg(long)]
    pub(super) field_length_scale_um: f64,
    /// Diagonal stabilization added to covariance matrices.
    #[arg(long)]
    pub(super) jitter: f64,
    #[command(flatten)]
    pub(super) nuts: NutsArgs,
    /// Upper bound on patients.
    #[arg(long)]
    pub(super) maximum_patients: usize,
    /// Upper bound on patterns.
    #[arg(long)]
    pub(super) maximum_patterns: usize,
    /// Upper bound on nodes per pattern.
    #[arg(long)]
    pub(super) maximum_nodes_per_pattern: usize,
    /// Upper bound on total nodes.
    #[arg(long)]
    pub(super) maximum_total_nodes: usize,
    /// Upper bound on total events.
    #[arg(long)]
    pub(super) maximum_total_events: u64,
    /// Upper bound on draw node work.
    #[arg(long)]
    pub(super) maximum_draw_node_work: u64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct ReplicatedArbitraryWindowLgcpInferredKernelProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// Per-node quadrature CSV (pattern_id, patient_id, group, node_id, x_um, y_um, count, ...).
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    pub(super) reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    pub(super) comparison_group: String,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    pub(super) intercept_prior_sd: f64,
    /// Prior scale for group effect.
    #[arg(long)]
    pub(super) group_effect_prior_sd: f64,
    /// Prior scale for covariate effect.
    #[arg(long)]
    pub(super) covariate_effect_prior_sd: f64,
    /// Prior scale for patient standard deviation.
    #[arg(long)]
    pub(super) patient_sd_prior_scale: f64,
    /// Prior scale for pattern standard deviation.
    #[arg(long)]
    pub(super) pattern_sd_prior_scale: f64,
    /// Prior scale for field amplitude.
    #[arg(long)]
    pub(super) field_amplitude_prior_scale: f64,
    /// Prior scale, in micrometers, for field length scale.
    #[arg(long)]
    pub(super) field_length_scale_prior_scale_um: f64,
    /// Diagonal stabilization added to covariance matrices.
    #[arg(long)]
    pub(super) jitter: f64,
    #[command(flatten)]
    pub(super) nuts: NutsArgs,
    /// Upper bound on patients.
    #[arg(long)]
    pub(super) maximum_patients: usize,
    /// Upper bound on patterns.
    #[arg(long)]
    pub(super) maximum_patterns: usize,
    /// Upper bound on nodes per pattern.
    #[arg(long)]
    pub(super) maximum_nodes_per_pattern: usize,
    /// Upper bound on total nodes.
    #[arg(long)]
    pub(super) maximum_total_nodes: usize,
    /// Upper bound on total events.
    #[arg(long)]
    pub(super) maximum_total_events: u64,
    /// Upper bound on draw node work.
    #[arg(long)]
    pub(super) maximum_draw_node_work: u64,
    /// Upper bound on kernel cube work.
    #[arg(long)]
    pub(super) maximum_kernel_cube_work: u64,
    /// Upper bound on tree depth.
    #[arg(long)]
    pub(super) maximum_tree_depth: u32,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct SmcAbcGrowthFrontProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON with an initial array of {position_um, density} growth-front points.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Diffusion coefficient in square micrometers per unit time.
    #[arg(long)]
    pub(super) diffusion_um2_per_time: f64,
    /// Maximum density in the logistic growth model.
    #[arg(long)]
    pub(super) carrying_capacity: f64,
    /// Simulation end time in the model's time units.
    #[arg(long)]
    pub(super) final_time: f64,
    /// Simulation time step in the model's time units.
    #[arg(long)]
    pub(super) time_step: f64,
    /// Fraction of carrying capacity defining the reported front.
    #[arg(long)]
    pub(super) front_threshold_fraction: f64,
    /// Observed total density mass at the final time.
    #[arg(long)]
    pub(super) observed_final_mass: f64,
    /// Scale used to standardize the simulated-versus-observed mass discrepancy.
    #[arg(long)]
    pub(super) mass_scale: f64,
    /// Lower bound of the uniform growth-rate prior.
    #[arg(long)]
    pub(super) growth_rate_prior_min: f64,
    /// Upper bound of the uniform growth-rate prior.
    #[arg(long)]
    pub(super) growth_rate_prior_max: f64,
    /// Decreasing ABC acceptance tolerances for successive SMC stages.
    #[arg(long, value_delimiter = ',')]
    pub(super) epsilon_schedule: Vec<f64>,
    /// Number of particles retained by the inference algorithm.
    #[arg(long)]
    pub(super) particles: u32,
    /// Upper bound on proposals per stage.
    #[arg(long)]
    pub(super) maximum_proposals_per_stage: u32,
    /// Upper bound on cell steps per proposal.
    #[arg(long)]
    pub(super) maximum_cell_steps_per_proposal: u64,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum ProjectNoninferiorityDirection {
    HigherIsBetter,
    LowerIsBetter,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ProjectCommand {
    /// Test whether cell and patch embeddings add complementary information; cache results in a durable project.
    TestCellPatchComplementarity(TestCellPatchComplementarityProjectArgs),
    /// Compare multivariate patient distributions with an energy-distance test; cache results in a durable project.
    CohortEnergy(CohortEnergyProjectArgs),
    /// Estimate randomized treatment effects under declared spatial interference; cache results in a durable project.
    CausalRandomizedInterference(CausalRandomizedInterferenceProjectArgs),
    /// Estimate expected information gain for a Gaussian observation design; cache results in a durable project.
    GaussianEig(GaussianEigProjectArgs),
    /// Test covariate-adjusted group effects with cluster-level permutations; cache results in a durable project.
    CohortClusterCovariatePermutation(CohortClusterCovariatePermutationProjectArgs),
    /// Test nested patient endpoints with hierarchical max-T correction; cache results in a durable project.
    CohortHierarchicalMaxT(CohortHierarchicalMaxTProjectArgs),
    /// Bootstrap patients and their nested specimens; cache results in a durable project.
    CohortHierarchicalBootstrap(CohortHierarchicalBootstrapProjectArgs),
    /// Test whether a group contrast lies within prespecified equivalence margins; cache results in a durable project.
    CohortEquivalence(CohortEquivalenceProjectArgs),
    /// Test a group contrast against a prespecified noninferiority margin; cache results in a durable project.
    CohortNoninferiority(CohortNoninferiorityProjectArgs),
    /// Test multiple patient-level endpoints with max-T correction; cache results in a durable project.
    CohortMaxT(CohortMaxTProjectArgs),
    /// Reduce nested spatial fields to patient-level endpoint inference; cache results in a durable project.
    PatientNestedFields(PatientNestedFieldsProjectArgs),
    /// Evaluate max-T calibration on declared synthetic endpoint families; cache results in a durable project.
    MaxTCalibration(MaxTCalibrationProjectArgs),
    /// Compare multivariate patient distributions with a kernel MMD test; cache results in a durable project.
    CohortMmd(CohortMmdProjectArgs),
    /// Estimate site-aware group contrasts with covariate adjustment; cache results in a durable project.
    CohortMultisiteCovariateContrast(CohortMultisiteCovariateContrastProjectArgs),
    /// Compute semivariograms after projecting embedding vectors; cache results in a durable project.
    ProjectedEmbeddingVariograms(ProjectedEmbeddingVariogramsProjectArgs),
    /// Measure embedding-vector dissimilarity across distance bins; cache results in a durable project.
    VectorSemivariogram(VectorSemivariogramProjectArgs),
    /// Measure local spatial association in multivariate observations; cache results in a durable project.
    LocalMultivariateMoran(LocalMultivariateMoranProjectArgs),
    /// Measure cross-covariance between embedding features by distance; cache results in a durable project.
    EmbeddingCrossCovarianceByDistance(EmbeddingCrossCovarianceByDistanceProjectArgs),
    /// Measure spatial mark dependence with an embedding kernel; cache results in a durable project.
    KernelMarkCorrelation(KernelMarkCorrelationProjectArgs),
    /// Compare embedding dependence curves with permutation envelopes; cache results in a durable project.
    EmbeddingSpatialDependenceEnvelope(EmbeddingSpatialDependenceEnvelopeProjectArgs),
    /// Measure signal roughness across weighted graph edges; cache results in a durable project.
    GraphDirichletEnergy(GraphDirichletEnergyProjectArgs),
    /// Test graph-signal smoothness against permutations; cache results in a durable project.
    GraphSmoothnessPermutationTest(GraphSmoothnessPermutationTestProjectArgs),
    /// Measure local embedding variation around graph nodes; cache results in a durable project.
    LocalEmbeddingRoughness(LocalEmbeddingRoughnessProjectArgs),
    /// Smooth repeated observations with a linear Gaussian state-space model; cache results in a durable project.
    LongitudinalKalmanSmooth(LongitudinalKalmanSmoothProjectArgs),
    /// Combine embedding kernels across declared spatial scales; cache results in a durable project.
    MultiscaleEmbeddingKernel(MultiscaleEmbeddingKernelProjectArgs),
    /// Retrieve embedding-nearest regions under a declared leakage policy; cache results in a durable project.
    RegionRetrieval(RegionRetrievalProjectArgs),
    /// Compare marked spatial patterns before and after a declared transition; cache results in a durable project.
    MarkedPrepost(MarkedPrepostProjectArgs),
    /// Diffuse a signal on a sparse radius-neighbor graph; cache results in a durable project.
    SparseRadiusHeat(SparseRadiusHeatProjectArgs),
    /// Construct a spectral basis on a sparse radius-neighbor graph; cache results in a durable project.
    SparseRadiusBasis(SparseRadiusBasisProjectArgs),
    /// Summarize typed triangle motifs in a graph; cache results in a durable project.
    GraphMotifTriangleSummary(GraphMotifTriangleSummaryProjectArgs),
    /// Infer a growth rate with sequentially tightened ABC tolerances; cache results in a durable project.
    SmcAbcGrowthFront(Box<SmcAbcGrowthFrontProjectArgs>),
    /// Fit a regression whose selected coefficient varies over space; cache results in a durable project.
    SpatialVaryingCoefficient(SpatialVaryingCoefficientProjectArgs),
    /// Estimate a three-dimensional K function in a declared window; cache results in a durable project.
    Spatial3dKFunction(Spatial3dKFunctionProjectArgs),
    /// Estimate a three-dimensional K function within a voxel window; cache results in a durable project.
    Spatial3dVoxelKFunction(Spatial3dVoxelKFunctionProjectArgs),
    /// Propagate serial-section registration into a voxel-window K analysis; cache results in a durable project.
    Spatial3dRegisteredSerialVoxelK(Spatial3dRegisteredSerialVoxelKProjectArgs),
    /// Compare longitudinal voxel-window K curves under registration uncertainty; cache results in a durable project.
    Spatial3dRegisteredLongitudinalVoxelKChange(
        Spatial3dRegisteredLongitudinalVoxelKChangeProjectArgs,
    ),
    /// Measure signal energy in a sparse radius-graph spectral basis; cache results in a durable project.
    SparseRadiusFourierEnergy(SparseRadiusFourierEnergyProjectArgs),
    /// Assess heat-diffusion sensitivity to graph perturbations; cache results in a durable project.
    SparseRadiusHeatStability(SparseRadiusHeatStabilityProjectArgs),
    /// Compute diffusion-wavelet features on a sparse radius graph; cache results in a durable project.
    SparseRadiusDiffusionWavelet(SparseRadiusDiffusionWaveletProjectArgs),
    /// Compute multiscale scattering features on a sparse radius graph; cache results in a durable project.
    SparseRadiusScattering(SparseRadiusScatteringProjectArgs),
    /// Fit a sparse conditional autoregressive spatial model; cache results in a durable project.
    SparseCarFit(SparseCarFitProjectArgs),
    /// Fit overdispersed counts with hierarchical rate variation; cache results in a durable project.
    NegativeBinomialHierarchy(NegativeBinomialHierarchyProjectArgs),
    /// Compute witness-complex persistence from selected landmarks; cache results in a durable project.
    WitnessPersistence(WitnessPersistenceProjectArgs),
    /// Assess witness persistence across landmark perturbations; cache results in a durable project.
    WitnessPersistenceStability(WitnessPersistenceStabilityProjectArgs),
    /// Measure bottleneck stability of witness persistence diagrams; cache results in a durable project.
    WitnessPersistenceBottleneckStability(WitnessPersistenceBottleneckStabilityProjectArgs),
    /// Fit a spatial factor on an adaptive mesh inside a polygonal window; cache results in a durable project.
    AdaptiveWindowSpde(AdaptiveWindowSpdeProjectArgs),
    /// Fit an adaptive SPDE with declared local scale and anisotropy; cache results in a durable project.
    NonstationaryAdaptiveWindowSpde(NonstationaryAdaptiveWindowSpdeProjectArgs),
    /// Evaluate an inhomogeneous Poisson likelihood in a polygonal window; cache results in a durable project.
    ArbitraryWindowIppLikelihood(Box<ArbitraryWindowIppProjectArgs>),
    /// Fit a Poisson intensity model in a polygonal observation window; cache results in a durable project.
    FitArbitraryWindowIpp(Box<ArbitraryWindowIppFitProjectArgs>),
    /// Compare observed spatial summaries with posterior predictions for a polygonal-window Poisson intensity model; cache results in a durable project.
    ArbitraryWindowIppSpatialPpc(Box<ArbitraryWindowIppSpatialPpcProjectArgs>),
    /// Fit a log-Gaussian Cox process in a polygonal observation window; cache results in a durable project.
    ArbitraryWindowLgcp(Box<ArbitraryWindowLgcpFitProjectArgs>),
    /// Fit a log-Gaussian Cox process across replicated polygonal windows; cache results in a durable project.
    ReplicatedArbitraryWindowLgcp(Box<ReplicatedArbitraryWindowLgcpFitProjectArgs>),
    /// Infer shared kernel parameters across replicated spatial intensity fields; cache results in a durable project.
    ReplicatedArbitraryWindowLgcpInferredKernel(
        Box<ReplicatedArbitraryWindowLgcpInferredKernelProjectArgs>,
    ),
    /// Fit cell-type intensities across replicated windows; cache results in a durable project.
    ReplicatedArbitraryWindowMultitypeLgcp(
        Box<InProject<crate::bayes::ReplicatedArbitraryWindowMultitypeLgcpArgs>>,
    ),
    /// Infer spatial kernels for replicated multitype intensity fields; cache results in a durable project.
    ReplicatedArbitraryWindowMultitypeLgcpInferredKernel(
        Box<InProject<crate::bayes::ReplicatedArbitraryWindowMultitypeLgcpInferredKernelArgs>>,
    ),
    /// Check posterior calibration with simulated data for replicated multitype fields with inferred kernels; cache results in a durable project.
    ReplicatedArbitraryWindowMultitypeLgcpInferredKernelSbc(Box<multitype_lgcp_sbc::ProjectArgs>),
    /// Fit correlated multitype spatial fields across replicated windows; cache results in a durable project.
    CorrelatedReplicatedArbitraryWindowMultitypeLgcp(Box<correlated_multitype_lgcp::ProjectArgs>),
    /// Fit a joint model of replicated locations and embedding features; cache results in a durable project.
    JointReplicatedLocationEmbedding(Box<joint_location_embedding::ProjectArgs>),
    /// Fit a joint model of replicated locations and scalar marks; cache results in a durable project.
    JointReplicatedLocationMark(Box<joint_location_mark::ProjectArgs>),
    /// Fit a Gaussian model with crossed and nested grouping effects; cache results in a durable project.
    GaussianCrossedNestedHierarchy(Box<gaussian_crossed_nested_hierarchy::ProjectArgs>),
    /// Fit cell-type probabilities conditional on observed locations; cache results in a durable project.
    ConditionalMultitypeMark(Box<InProject<crate::bayes::ConditionalMultitypeMarkArgs>>),
    /// Fit location-conditional cell-type probabilities across replicates; cache results in a durable project.
    ReplicatedConditionalMultitypeMark(
        Box<InProject<crate::bayes::ReplicatedConditionalMultitypeMarkArgs>>,
    ),
    /// Fit a Gaussian mean with a conjugate normal prior; cache results in a durable project.
    NormalMean(NormalMeanProjectArgs),
    /// Fit a Gaussian hierarchy across patients; cache results in a durable project.
    HierarchicalNormal(HierarchicalNormalProjectArgs),
    /// Fit a robust Student-t hierarchy across patients; cache results in a durable project.
    StudentTHierarchy(StudentTHierarchyProjectArgs),
    /// Fit patient success counts with beta-binomial overdispersion; cache results in a durable project.
    BetaBinomialHierarchy(BetaBinomialHierarchyProjectArgs),
    /// Estimate a group contrast in beta-binomial counts; cache results in a durable project.
    BetaBinomialGroupRegression(BetaBinomialGroupRegressionProjectArgs),
    /// Estimate group differences in multitype count compositions; cache results in a durable project.
    DirichletMultinomialGroup(DirichletMultinomialGroupProjectArgs),
    /// Estimate group effects on ordered outcomes; cache results in a durable project.
    OrdinalGroup(OrdinalGroupProjectArgs),
    /// Fit ordered outcomes with site variation; cache results in a durable project.
    OrdinalGroupSiteHierarchy(OrdinalGroupSiteHierarchyProjectArgs),
    /// Allow group effects to vary across ordered-outcome cutpoints; cache results in a durable project.
    NonproportionalOrdinalGroupSite(NonproportionalOrdinalGroupSiteProjectArgs),
    /// Compare ordinal models by holding out entire sites; cache results in a durable project.
    OrdinalSiteHeldoutComparison(OrdinalSiteHeldoutComparisonProjectArgs),
    /// Separate group effects on presence and positive abundance; cache results in a durable project.
    HurdleBetaBinomialGroup(HurdleBetaBinomialGroupProjectArgs),
    /// Estimate group and gender effects in beta-binomial counts; cache results in a durable project.
    BetaBinomialGroupGenderRegression(BetaBinomialGroupGenderRegressionProjectArgs),
    /// Fit beta-binomial counts with patient and slide variation; cache results in a durable project.
    BetaBinomialGroupGenderSlideHierarchy(BetaBinomialGroupGenderSlideHierarchyProjectArgs),
    /// Fit a log-Gaussian Cox-process intensity on a rectangular grid; cache results in a durable project.
    GriddedLgcp(GriddedLgcpProjectArgs),
    /// Align samples using both feature costs and within-sample structure; cache results in a durable project.
    FusedGromovWasserstein(FusedGromovWassersteinProjectArgs),
}

/// A command's own arguments plus the durable project it runs in.
#[derive(Debug, clap::Args)]
pub(crate) struct InProject<A: clap::Args> {
    /// Durable project directory; created on first use.
    #[arg(long)]
    pub(super) project: PathBuf,
    #[command(flatten)]
    pub(super) command: A,
}

#[derive(Debug, clap::Args)]
pub(crate) struct NegativeBinomialHierarchyProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: patient/slide/ROI observations with count, area and predictor; priors; sampling.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct MaxTCalibrationProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV of fixed patient endpoint vectors: patient_id, then one numeric column per endpoint.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Number of simulated patients assigned to the first group.
    #[arg(long)]
    pub(super) group_a_count: usize,
    /// Endpoint counts for the declared calibration families.
    #[arg(long, value_delimiter = ',')]
    pub(super) family_sizes: Vec<usize>,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Upper bound on assignments.
    #[arg(long)]
    pub(super) maximum_assignments: usize,
    /// Upper bound on assignment endpoint evaluations.
    #[arg(long)]
    pub(super) maximum_assignment_endpoint_evaluations: u64,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct TestCellPatchComplementarityProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// Patient CSV: patient_id, outer_fold, inner_fold, target, then prefixed feature columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Number of outer folds used for held-out evaluation.
    #[arg(long)]
    pub(super) outer_folds: u32,
    /// Number of inner folds used to choose predictive tuning parameters.
    #[arg(long)]
    pub(super) inner_folds: u32,
    /// Candidate ridge penalties considered during predictive tuning.
    #[arg(long, value_delimiter = ',', required = true)]
    pub(super) ridge_alphas: Vec<f64>,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct CohortEnergyProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV with patient_id, group, feature and value columns, plus an optional block column.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Label of the first group in the contrast.
    #[arg(long)]
    pub(super) group_a: String,
    /// Label of the second group in the contrast.
    #[arg(long)]
    pub(super) group_b: String,
    /// Distance metric used to compare observations.
    #[arg(long, value_enum)]
    pub(super) metric: CliEnergyMetric,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,

    #[command(flatten)]
    pub(super) limits: CohortEnergyLimits,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Copy, Debug, clap::Args)]
pub(crate) struct CohortEnergyLimits {
    /// Upper bound on patients.
    #[arg(long)]
    pub(super) maximum_patients: usize,
    /// Upper bound on features.
    #[arg(long)]
    pub(super) maximum_features: usize,
    /// Upper bound on distance elements.
    #[arg(long)]
    pub(super) maximum_distance_elements: u64,
    /// Upper bound on energy evaluations.
    #[arg(long)]
    pub(super) maximum_energy_evaluations: u64,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
}

#[derive(Debug, clap::Args)]
pub(crate) struct CausalRandomizedInterferenceProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: units, graph_edges, cluster_assignments, test exposures, permutations and seed.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct GaussianEigProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: candidate_id, prior_mean, prior_sd, sensitivity, noise_sd and sample counts.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct CohortClusterCovariatePermutationProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV with patient_id, cluster_id, group, outcome, covariate and value columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Label of the first group in the contrast.
    #[arg(long)]
    pub(super) group_a: String,
    /// Label of the second group in the contrast.
    #[arg(long)]
    pub(super) group_b: String,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Direction of the alternative hypothesis.
    #[arg(long, value_enum)]
    pub(super) alternative: crate::cohort::CliAlternative,

    #[command(flatten)]
    pub(super) limits: CohortClusterCovariatePermutationLimits,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Copy, Debug, clap::Args)]
pub(crate) struct CohortClusterCovariatePermutationLimits {
    /// Upper bound on patients.
    #[arg(long)]
    pub(super) maximum_patients: usize,
    /// Upper bound on clusters.
    #[arg(long)]
    pub(super) maximum_clusters: usize,
    /// Upper bound on covariates.
    #[arg(long)]
    pub(super) maximum_covariates: usize,
    /// Upper bound on patient covariate cells.
    #[arg(long)]
    pub(super) maximum_patient_covariate_cells: u64,
    /// Upper bound on OLS work.
    #[arg(long)]
    pub(super) maximum_ols_work: u64,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
}

#[derive(Debug, clap::Args)]
pub(crate) struct CohortHierarchicalMaxTProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV with patient_id, group, family_order, family, endpoint and value columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Label of the first group in the contrast.
    #[arg(long)]
    pub(super) group_a: String,
    /// Label of the second group in the contrast.
    #[arg(long)]
    pub(super) group_b: String,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Apply step-down max-T multiplicity adjustment.
    #[arg(long)]
    pub(super) step_down: bool,

    #[command(flatten)]
    pub(super) limits: CohortHierarchicalMaxTLimits,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Copy, Debug, clap::Args)]
pub(crate) struct CohortHierarchicalMaxTLimits {
    /// Upper bound on patients.
    #[arg(long)]
    pub(super) maximum_patients: usize,
    /// Upper bound on families.
    #[arg(long)]
    pub(super) maximum_families: usize,
    /// Upper bound on endpoints.
    #[arg(long)]
    pub(super) maximum_endpoints: usize,
    /// Upper bound on cells.
    #[arg(long)]
    pub(super) maximum_cells: u64,
    /// Upper bound on permutation endpoint evaluations.
    #[arg(long)]
    pub(super) maximum_permutation_endpoint_evaluations: u64,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
}

#[derive(Debug, clap::Args)]
pub(crate) struct CohortHierarchicalBootstrapProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV with patient_id, specimen_id and endpoint columns, one row per specimen.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Number of bootstrap resamples used for the interval estimate.
    #[arg(long)]
    pub(super) replicates: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,

    #[command(flatten)]
    pub(super) limits: CohortHierarchicalBootstrapLimits,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Copy, Debug, clap::Args)]
pub(crate) struct CohortHierarchicalBootstrapLimits {
    /// Upper bound on patients.
    #[arg(long)]
    pub(super) maximum_patients: usize,
    /// Upper bound on specimens.
    #[arg(long)]
    pub(super) maximum_specimens: usize,
    /// Upper bound on bootstrap draws.
    #[arg(long)]
    pub(super) maximum_bootstrap_draws: u64,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
}

#[derive(Debug, clap::Args)]
pub(crate) struct CohortEquivalenceProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV with patient_id and effect columns, one effect per patient.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Lower prespecified equivalence margin.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) lower_margin: f64,
    /// Upper prespecified equivalence margin.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) upper_margin: f64,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Scientific rationale for the declared equivalence or noninferiority margin.
    #[arg(long)]
    pub(super) margin_rationale: String,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct CohortNoninferiorityProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV with patient_id and effect columns, one effect per patient.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Direction in which the noninferiority margin is interpreted.
    #[arg(long, value_enum)]
    pub(super) direction: ProjectNoninferiorityDirection,
    /// Prespecified noninferiority margin.
    #[arg(long)]
    pub(super) margin: f64,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Scientific rationale for the declared equivalence or noninferiority margin.
    #[arg(long)]
    pub(super) margin_rationale: String,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct PatientNestedFieldsProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV with patient_id, specimen_id, group, endpoint and value columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Label of the first group in the contrast.
    #[arg(long)]
    pub(super) group_a: String,
    /// Label of the second group in the contrast.
    #[arg(long)]
    pub(super) group_b: String,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Upper bound on patients.
    #[arg(long)]
    pub(super) maximum_patients: usize,
    /// Upper bound on specimens.
    #[arg(long)]
    pub(super) maximum_specimens: usize,
    /// Upper bound on endpoints.
    #[arg(long)]
    pub(super) maximum_endpoints: usize,
    /// Upper bound on permutation endpoint evaluations.
    #[arg(long)]
    pub(super) maximum_permutation_endpoint_evaluations: u64,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct CohortMaxTProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV with patient_id, group, endpoint and value columns, plus an optional block column.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Label of the first group in the contrast.
    #[arg(long)]
    pub(super) group_a: String,
    /// Label of the second group in the contrast.
    #[arg(long)]
    pub(super) group_b: String,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Apply step-down max-T multiplicity adjustment.
    #[arg(long)]
    pub(super) step_down: bool,

    #[command(flatten)]
    pub(super) limits: CohortMaxTLimits,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Copy, Debug, clap::Args)]
pub(crate) struct CohortMaxTLimits {
    /// Upper bound on patients.
    #[arg(long)]
    pub(super) maximum_patients: usize,
    /// Upper bound on endpoints.
    #[arg(long)]
    pub(super) maximum_endpoints: usize,
    /// Upper bound on patient endpoint cells.
    #[arg(long)]
    pub(super) maximum_patient_endpoint_cells: u64,
    /// Upper bound on permutation endpoint evaluations.
    #[arg(long)]
    pub(super) maximum_permutation_endpoint_evaluations: u64,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
}

#[derive(Debug, clap::Args)]
pub(crate) struct CohortMmdProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV with patient_id, group, feature and value columns, plus an optional block column.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Label of the first group in the contrast.
    #[arg(long)]
    pub(super) group_a: String,
    /// Label of the second group in the contrast.
    #[arg(long)]
    pub(super) group_b: String,
    /// Kernel used to compare observations.
    #[arg(long, value_enum)]
    pub(super) kernel: CliMmdKernel,
    /// Bandwidth of the kernel used to compare observations.
    #[arg(long)]
    pub(super) bandwidth: Option<f64>,
    /// Estimator used for the kernel discrepancy between groups.
    #[arg(long, value_enum)]
    pub(super) estimator: CliMmdEstimator,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,

    #[command(flatten)]
    pub(super) limits: CohortMmdLimits,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Copy, Debug, clap::Args)]
pub(crate) struct CohortMmdLimits {
    /// Upper bound on patients.
    #[arg(long)]
    pub(super) maximum_patients: usize,
    /// Upper bound on features.
    #[arg(long)]
    pub(super) maximum_features: usize,
    /// Upper bound on kernel elements.
    #[arg(long)]
    pub(super) maximum_kernel_elements: u64,
    /// Upper bound on MMD evaluations.
    #[arg(long)]
    pub(super) maximum_mmd_evaluations: u64,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
}

#[derive(Debug, clap::Args)]
pub(crate) struct CohortMultisiteCovariateContrastProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV with patient_id, site_id, group, endpoint, covariate and value columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Label of the first group in the contrast.
    #[arg(long)]
    pub(super) group_a: String,
    /// Label of the second group in the contrast.
    #[arg(long)]
    pub(super) group_b: String,
    /// Model variant used for the declared spatial or multisite analysis.
    #[arg(long, value_enum)]
    pub(super) model: crate::cohort::CliMultisiteModel,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,

    #[command(flatten)]
    pub(super) limits: CohortMultisiteCovariateContrastLimits,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Copy, Debug, clap::Args)]
pub(crate) struct CohortMultisiteCovariateContrastLimits {
    /// Upper bound on patients.
    #[arg(long)]
    pub(super) maximum_patients: usize,
    /// Upper bound on sites.
    #[arg(long)]
    pub(super) maximum_sites: usize,
    /// Upper bound on covariates.
    #[arg(long)]
    pub(super) maximum_covariates: usize,
    /// Upper bound on patient covariate cells.
    #[arg(long)]
    pub(super) maximum_patient_covariate_cells: u64,
    /// Upper bound on OLS work.
    #[arg(long)]
    pub(super) maximum_ols_work: u64,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
}

#[derive(Debug, clap::Args)]
pub(crate) struct ProjectedEmbeddingVariogramsProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV: object_id, biological_unit, split, permutation_stratum, x_um, y_um, embedding columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Distance-bin edges used to group spatial observation pairs.
    #[arg(long)]
    pub(super) bins: PathBuf,
    /// Projection components used for the embedding variograms.
    #[arg(long)]
    pub(super) components: u32,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: u32,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Upper bound on pair visits.
    #[arg(long)]
    pub(super) maximum_pair_visits: u64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct VectorSemivariogramProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV with object_id, x_um and y_um columns followed by two or more embedding columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Distance-bin edges used to group spatial observation pairs.
    #[arg(long)]
    pub(super) bins: PathBuf,
    /// Optional table of pair weights for the semivariogram.
    #[arg(long)]
    pub(super) weights: Option<PathBuf>,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
    /// Upper bound on points.
    #[arg(long)]
    pub(super) maximum_points: usize,
    /// Upper bound on dimension.
    #[arg(long)]
    pub(super) maximum_dimension: usize,
    /// Upper bound on pair visits.
    #[arg(long)]
    pub(super) maximum_pair_visits: u64,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
}

#[derive(Debug, clap::Args)]
pub(crate) struct LocalMultivariateMoranProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV: point_id, permutation_stratum, x_um, y_um, marks; or cell_id, x_um, y_um, cellvit_pc_*.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Observation-window geometry used for spatial integration or edge correction.
    #[arg(long)]
    pub(super) window: PathBuf,
    /// Spatial radius in micrometers.
    #[arg(long)]
    pub(super) radius_um: f64,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Upper bound on points.
    #[arg(long)]
    pub(super) maximum_points: usize,
    /// Upper bound on dimension.
    #[arg(long)]
    pub(super) maximum_dimension: usize,
    /// Upper bound on directed edges.
    #[arg(long)]
    pub(super) maximum_directed_edges: usize,
    /// Upper bound on permutation edge evaluations.
    #[arg(long)]
    pub(super) maximum_permutation_edge_evaluations: usize,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct EmbeddingCrossCovarianceByDistanceProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV with object_id, x_um and y_um columns followed by two or more embedding columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Distance-bin edges used to group spatial observation pairs.
    #[arg(long)]
    pub(super) bins: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
    /// Upper bound on points.
    #[arg(long)]
    pub(super) maximum_points: usize,
    /// Upper bound on dimension.
    #[arg(long)]
    pub(super) maximum_dimension: usize,
    /// Upper bound on pair visits.
    #[arg(long)]
    pub(super) maximum_pair_visits: u64,
    /// Upper bound on matrix elements.
    #[arg(long)]
    pub(super) maximum_matrix_elements: u64,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
}

#[derive(Debug, clap::Args)]
pub(crate) struct KernelMarkCorrelationProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV: object_id, biological_unit, split, x_um, y_um and two or more embedding_* columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Distance-bin edges used to group spatial observation pairs.
    #[arg(long)]
    pub(super) bins: PathBuf,
    /// Embedding kernel.
    #[arg(long, value_parser = ["linear", "cosine", "rbf", "laplacian"])]
    pub(super) kernel: String,
    /// Tolerance for comparison with the global kernel reference.
    #[arg(long)]
    pub(super) global_reference_tolerance: f64,

    #[command(flatten)]
    pub(super) limits: KernelMarkCorrelationLimits,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Copy, Debug, clap::Args)]
pub(crate) struct KernelMarkCorrelationLimits {
    /// Upper bound on points.
    #[arg(long)]
    pub(super) maximum_points: usize,
    /// Upper bound on dimension.
    #[arg(long)]
    pub(super) maximum_dimension: usize,
    /// Upper bound on pair visits.
    #[arg(long)]
    pub(super) maximum_pair_visits: u64,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
}

#[derive(Debug, clap::Args)]
pub(crate) struct EmbeddingSpatialDependenceEnvelopeProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV: object_id, permutation_stratum, x_um, y_um and two or more embedding columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Distance-bin edges used to group spatial observation pairs.
    #[arg(long)]
    pub(super) bins: PathBuf,
    /// Spatial dependence curve to compare with the null envelope.
    #[arg(long)]
    pub(super) curve: String,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: u32,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,

    #[command(flatten)]
    pub(super) limits: EmbeddingSpatialDependenceEnvelopeLimits,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Copy, Debug, clap::Args)]
pub(crate) struct EmbeddingSpatialDependenceEnvelopeLimits {
    /// Upper bound on points.
    #[arg(long)]
    pub(super) maximum_points: usize,
    /// Upper bound on dimension.
    #[arg(long)]
    pub(super) maximum_dimension: usize,
    /// Upper bound on pair visits.
    #[arg(long)]
    pub(super) maximum_pair_visits: u64,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
}

#[derive(Debug, clap::Args)]
pub(crate) struct GraphDirichletEnergyProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// Graph-node observations and their embedding or signal values.
    #[arg(long)]
    pub(super) nodes: PathBuf,
    /// Table of spatial graph edges and their declared weights.
    #[arg(long)]
    pub(super) edges: PathBuf,
    /// Graph Laplacian.
    #[arg(long, value_parser = ["combinatorial", "symmetric_normalized"])]
    pub(super) laplacian: String,
    /// How the graph energy is normalized.
    #[arg(long, value_parser = ["none", "signal", "edge_weight"])]
    pub(super) normalization: String,

    #[command(flatten)]
    pub(super) limits: GraphDirichletEnergyLimits,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Copy, Debug, clap::Args)]
pub(crate) struct GraphDirichletEnergyLimits {
    /// Upper bound on nodes.
    #[arg(long)]
    pub(super) maximum_nodes: usize,
    /// Upper bound on edges.
    #[arg(long)]
    pub(super) maximum_edges: usize,
    /// Upper bound on dimension.
    #[arg(long)]
    pub(super) maximum_dimension: usize,
    /// Upper bound on component edge visits.
    #[arg(long)]
    pub(super) maximum_component_edge_visits: u64,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
}

#[derive(Debug, clap::Args)]
pub(crate) struct GraphSmoothnessPermutationTestProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// Graph-node observations and their embedding or signal values.
    #[arg(long)]
    pub(super) nodes: PathBuf,
    /// Table of spatial graph edges and their declared weights.
    #[arg(long)]
    pub(super) edges: PathBuf,
    /// Graph Laplacian.
    #[arg(long, value_parser = ["combinatorial", "symmetric_normalized"])]
    pub(super) laplacian: String,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: u32,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,

    #[command(flatten)]
    pub(super) limits: GraphSmoothnessPermutationTestLimits,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Copy, Debug, clap::Args)]
pub(crate) struct GraphSmoothnessPermutationTestLimits {
    /// Upper bound on nodes.
    #[arg(long)]
    pub(super) maximum_nodes: usize,
    /// Upper bound on edges.
    #[arg(long)]
    pub(super) maximum_edges: usize,
    /// Upper bound on dimension.
    #[arg(long)]
    pub(super) maximum_dimension: usize,
    /// Upper bound on component edge visits.
    #[arg(long)]
    pub(super) maximum_component_edge_visits: u64,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
}

#[derive(Debug, clap::Args)]
pub(crate) struct LocalEmbeddingRoughnessProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// Graph-node observations and their embedding or signal values.
    #[arg(long)]
    pub(super) nodes: PathBuf,
    /// Table of spatial graph edges and their declared weights.
    #[arg(long)]
    pub(super) edges: PathBuf,
    /// Stabilizing constant used when normalizing local embedding roughness.
    #[arg(long)]
    pub(super) epsilon: f64,

    #[command(flatten)]
    pub(super) limits: LocalEmbeddingRoughnessLimits,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Copy, Debug, clap::Args)]
pub(crate) struct LocalEmbeddingRoughnessLimits {
    /// Upper bound on nodes.
    #[arg(long)]
    pub(super) maximum_nodes: usize,
    /// Upper bound on edges.
    #[arg(long)]
    pub(super) maximum_edges: usize,
    /// Upper bound on dimension.
    #[arg(long)]
    pub(super) maximum_dimension: usize,
    /// Upper bound on component edge visits.
    #[arg(long)]
    pub(super) maximum_component_edge_visits: u64,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
}

#[derive(Debug, clap::Args)]
pub(crate) struct LongitudinalKalmanSmoothProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON state-space spec: observations, transition/observation matrices, covariances, prior.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct MultiscaleEmbeddingKernelProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV of per-scale summaries: sample_id, scale_um and embedding_* columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Weights assigned to the declared embedding scales.
    #[arg(long)]
    pub(super) weights: PathBuf,
    /// First sample supplied for comparison.
    #[arg(long)]
    pub(super) sample_a: String,
    /// Second sample supplied for comparison.
    #[arg(long)]
    pub(super) sample_b: String,
    /// Kernel applied at each scale.
    #[arg(long, value_parser = ["linear", "cosine", "rbf", "laplacian"])]
    pub(super) base_kernel: String,
    /// Scale parameter of the embedding kernel.
    #[arg(long)]
    pub(super) kernel_scale: Option<f64>,

    #[command(flatten)]
    pub(super) limits: MultiscaleEmbeddingKernelLimits,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Copy, Debug, clap::Args)]
pub(crate) struct MultiscaleEmbeddingKernelLimits {
    /// Upper bound on summaries.
    #[arg(long)]
    pub(super) maximum_summaries: usize,
    /// Upper bound on dimension.
    #[arg(long)]
    pub(super) maximum_dimension: usize,
    /// Upper bound on component scale visits.
    #[arg(long)]
    pub(super) maximum_component_scale_visits: u64,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
}

#[derive(Debug, clap::Args)]
pub(crate) struct RegionRetrievalProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// Training observations used to fit or define the reference model.
    #[arg(long)]
    pub(super) training: PathBuf,
    /// Query regions to compare with the reference embedding collection.
    #[arg(long)]
    pub(super) query: PathBuf,
    /// Number of neighboring regions to retrieve.
    #[arg(long)]
    pub(super) k: u32,
    /// Rules excluding related observations from region retrieval.
    #[arg(long)]
    pub(super) leakage_policy: String,
    /// Upper bound on component candidate visits.
    #[arg(long)]
    pub(super) maximum_component_candidate_visits: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct MarkedPrepostProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// Result or observation set for the earlier timepoint.
    #[arg(long)]
    pub(super) pre: PathBuf,
    /// Result or observation set for the later timepoint.
    #[arg(long)]
    pub(super) post: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct SparseRadiusHeatProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: nodes, radius_um, time, tolerance, maximum_order and work limits.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct SparseRadiusBasisProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: nodes, radius_um, mode_count, solver tolerances and work limits.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct GraphMotifTriangleSummaryProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: labelled nodes, edges, motif_id, required_labels, permutations and seed.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct SpatialVaryingCoefficientProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV: coordinate_id, x_um, y, then the --global- and --spatial-predictor-name columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Predictor assigned a spatially constant regression coefficient.
    #[arg(long)]
    pub(super) global_predictor_name: String,
    /// Predictor whose regression coefficient varies over space.
    #[arg(long)]
    pub(super) spatial_predictor_name: String,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    pub(super) intercept_prior_sd: f64,
    /// Prior scale for coefficient.
    #[arg(long)]
    pub(super) coefficient_prior_sd: f64,
    /// Prior scale for amplitude.
    #[arg(long)]
    pub(super) amplitude_prior_sd: f64,
    /// Prior scale, in micrometers, for length scale.
    #[arg(long)]
    pub(super) length_scale_prior_sd_um: f64,
    /// Known observation-noise standard deviation.
    #[arg(long)]
    pub(super) known_noise_sd: f64,
    /// Diagonal stabilization added to covariance matrices.
    #[arg(long)]
    pub(super) jitter: f64,
    #[command(flatten)]
    pub(super) nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct SparseCarFitProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: observations, edges, rho, tau, draws, seed and solver limits.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct Spatial3dKFunctionProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: points, cuboid window, voxel_spacing, radii_um, correction and pair limit.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct Spatial3dVoxelKFunctionProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: points, voxel-union window, radii_um, correction and work limits.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct Spatial3dRegisteredSerialVoxelKProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: patient, specimen and timepoint IDs, registered sections, voxel window, radii_um.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct Spatial3dRegisteredLongitudinalVoxelKChangeProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: patient/lesion IDs, baseline and follow_up volumes, deformation draws, limits.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct SparseRadiusFourierEnergyProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: a sparse-radius basis spec plus projection and retained-byte limits.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct SparseRadiusHeatStabilityProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: sparse-radius heat settings plus jitter replicates, seed and total limits.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct SparseRadiusDiffusionWaveletProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: nodes, radius_um, wavelet times, tolerance and work limits.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct SparseRadiusScatteringProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: nodes, radius_um, times, scattering_order, tolerance and work limits.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct WitnessPersistenceProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: points, landmark method and count, max_scale_um, coefficient field and limits.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct WitnessPersistenceStabilityProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: witness persistence settings plus jitter replicates, seed and tolerances.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct WitnessPersistenceBottleneckStabilityProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: a witness stability spec plus bottleneck distance and execution limits.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct AdaptiveWindowSpdeProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: window_frame, MultiPolygon window, mesh controls, kappa, tau and observations.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct NonstationaryAdaptiveWindowSpdeProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON spec: window, mesh controls, background and parameter_regions, factor_observations.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct NormalMeanProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV with a single observation column.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Mean of the prior distribution.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) prior_mean: f64,
    /// Scale of the prior distribution.
    #[arg(long)]
    pub(super) prior_sd: f64,
    /// Known standard deviation of Gaussian observation noise.
    #[arg(long)]
    pub(super) known_sigma: f64,
    #[command(flatten)]
    pub(super) nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct HierarchicalNormalProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// CSV with patient_id and observation columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    #[command(flatten)]
    pub(super) model: crate::bayes::HierarchicalNormalModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct BetaBinomialHierarchyProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    #[command(flatten)]
    pub(super) model: crate::bayes::BetaBinomialHierarchyModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct BetaBinomialGroupRegressionProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    #[command(flatten)]
    pub(super) model: crate::bayes::BetaBinomialGroupRegressionModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct DirichletMultinomialGroupProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    #[command(flatten)]
    pub(super) model: crate::bayes::DirichletMultinomialGroupModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct OrdinalGroupProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    #[command(flatten)]
    pub(super) model: crate::bayes::OrdinalGroupModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct OrdinalGroupSiteHierarchyProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    #[command(flatten)]
    pub(super) model: crate::bayes::OrdinalGroupSiteHierarchyModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct NonproportionalOrdinalGroupSiteProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    #[command(flatten)]
    pub(super) model: crate::bayes::NonproportionalOrdinalGroupSiteModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct OrdinalSiteHeldoutComparisonProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    #[command(flatten)]
    pub(super) model: crate::bayes::OrdinalSiteHeldoutComparisonModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct HurdleBetaBinomialGroupProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    #[command(flatten)]
    pub(super) model: crate::bayes::HurdleBetaBinomialGroupModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct BetaBinomialGroupGenderRegressionProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    #[command(flatten)]
    pub(super) model: crate::bayes::BetaBinomialGroupGenderRegressionModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct BetaBinomialGroupGenderSlideHierarchyProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    #[command(flatten)]
    pub(super) model: crate::bayes::BetaBinomialGroupGenderSlideHierarchyModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct StudentTHierarchyProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    #[command(flatten)]
    pub(super) model: crate::bayes::StudentTHierarchyModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct GriddedLgcpProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    #[command(flatten)]
    pub(super) model: crate::bayes::GriddedLgcpModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct FusedGromovWassersteinProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    pub(super) project: PathBuf,
    /// JSON source and target supports (id, mass, features) with row-major structure matrices.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Weight of structural cost relative to feature cost in the fused transport objective.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Entropy regularization strength of the transport objective.
    #[arg(long)]
    pub(super) epsilon: f64,
    /// Scale applied to feature dissimilarities in the transport objective.
    #[arg(long)]
    pub(super) feature_scale: f64,
    /// Scale applied to structural dissimilarities in the transport objective.
    #[arg(long)]
    pub(super) structure_scale: f64,
    /// Numerical tolerance used by the algorithm.
    #[arg(long)]
    pub(super) tolerance: f64,
    /// Upper bound on iterations.
    #[arg(long)]
    pub(super) maximum_iterations: u32,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}
