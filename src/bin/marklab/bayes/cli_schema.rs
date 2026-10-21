use super::*;

mod command_dispatch;

use crate::bayes::NutsArgs;
pub(crate) use command_dispatch::run_cli;

#[derive(Debug, Args)]
pub(crate) struct BetaBinomialGroupGenderSlideHierarchyAgreementArgs {
    #[command(flatten)]
    pub(super) model: BetaBinomialGroupGenderSlideHierarchyModelArgs,
    /// Largest standardized difference accepted between backend estimates.
    #[arg(long)]
    pub(super) maximum_standardized_difference: f64,
    /// Minimum absolute comparison tolerance for probability.
    #[arg(long)]
    pub(super) minimum_probability_tolerance: f64,
    /// Minimum absolute comparison tolerance for log odds.
    #[arg(long)]
    pub(super) minimum_log_odds_tolerance: f64,
    /// Minimum absolute comparison tolerance for patient standard deviation.
    #[arg(long)]
    pub(super) minimum_patient_sd_tolerance: f64,
    /// Minimum absolute comparison tolerance for concentration.
    #[arg(long)]
    pub(super) minimum_concentration_tolerance: f64,
    /// Minimum absolute comparison tolerance for patient probability.
    #[arg(long)]
    pub(super) minimum_patient_probability_tolerance: f64,
    /// Minimum absolute comparison tolerance for patient effect.
    #[arg(long)]
    pub(super) minimum_patient_effect_tolerance: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BetaBinomialGroupGenderSlideHierarchySensitivityArgs {
    #[command(flatten)]
    pub(super) model: BetaBinomialGroupGenderSlideHierarchyModelArgs,
    /// Multiplier defining the lower prior-scale sensitivity scenario.
    #[arg(long)]
    pub(super) lower_scale_multiplier: f64,
    /// Multiplier defining the upper prior-scale sensitivity scenario.
    #[arg(long)]
    pub(super) upper_scale_multiplier: f64,
    /// Smallest standardized posterior change considered material.
    #[arg(long)]
    pub(super) material_standardized_shift: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BetaBinomialGroupGenderSlideHierarchySbcArgs {
    #[command(flatten)]
    pub(super) model: BetaBinomialGroupGenderSlideHierarchyModelArgs,
    /// Number of simulated datasets used for posterior calibration.
    #[arg(long)]
    pub(super) replicates: u32,
    /// Smallest admitted p-value for simulation-based rank uniformity.
    #[arg(long)]
    pub(super) minimum_rank_uniformity_p_value: f64,
    /// Smallest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    pub(super) minimum_coverage_90: f64,
    /// Largest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    pub(super) maximum_coverage_90: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct DirichletMultinomialGroupModelArgs {
    /// CSV with patient_id, group, class_id and count columns, one row per patient and class.
    #[arg(long)]
    pub(crate) input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    pub(crate) reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    pub(crate) comparison_group: String,
    /// Prior scale for logit.
    #[arg(long)]
    pub(crate) logit_prior_sd: f64,
    /// Prior scale for group effect.
    #[arg(long)]
    pub(crate) group_effect_prior_sd: f64,
    /// Prior scale for concentration.
    #[arg(long)]
    pub(crate) concentration_prior_sd: f64,
    #[command(flatten)]
    pub(crate) nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
}

#[derive(Debug, Args)]
pub(crate) struct DirichletMultinomialGroupArgs {
    #[command(flatten)]
    pub(super) model: DirichletMultinomialGroupModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct DirichletMultinomialGroupAgreementArgs {
    #[command(flatten)]
    pub(super) model: DirichletMultinomialGroupModelArgs,
    /// Largest standardized difference accepted between backend estimates.
    #[arg(long)]
    pub(super) maximum_standardized_difference: f64,
    /// Minimum absolute comparison tolerance for probability.
    #[arg(long)]
    pub(super) minimum_probability_tolerance: f64,
    /// Minimum absolute comparison tolerance for concentration.
    #[arg(long)]
    pub(super) minimum_concentration_tolerance: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct DirichletMultinomialGroupSensitivityArgs {
    #[command(flatten)]
    pub(super) model: DirichletMultinomialGroupModelArgs,
    /// Multiplier defining the lower prior-scale sensitivity scenario.
    #[arg(long)]
    pub(super) lower_scale_multiplier: f64,
    /// Multiplier defining the upper prior-scale sensitivity scenario.
    #[arg(long)]
    pub(super) upper_scale_multiplier: f64,
    /// Smallest standardized posterior change considered material.
    #[arg(long)]
    pub(super) material_standardized_shift: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct DirichletMultinomialGroupSbcArgs {
    #[command(flatten)]
    pub(super) model: DirichletMultinomialGroupModelArgs,
    /// Number of simulated datasets used for posterior calibration.
    #[arg(long)]
    pub(super) replicates: u32,
    /// Smallest admitted p-value for simulation-based rank uniformity.
    #[arg(long)]
    pub(super) minimum_rank_uniformity_p_value: f64,
    /// Smallest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    pub(super) minimum_coverage_90: f64,
    /// Largest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    pub(super) maximum_coverage_90: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Subcommand)]
pub(crate) enum BayesCommand {
    #[command(flatten)]
    Hierarchy(HierarchyCommands),
    #[command(flatten)]
    Fields(FieldsCommands),
    #[command(flatten)]
    SimulationInference(SimulationInferenceCommands),
    #[command(flatten)]
    Inference(InferenceCommands),
    #[command(flatten)]
    PointProcesses(PointProcessesCommands),
    #[command(flatten)]
    Embeddings(EmbeddingsCommands),
    #[command(flatten)]
    Prediction(PredictionCommands),
    #[command(flatten)]
    Transport(TransportCommands),
    #[command(flatten)]
    ArbitraryWindow(ArbitraryWindowCommands),
    #[command(flatten)]
    MarkModels(MarkModelsCommands),
}

// Flattened families preserve command paths while bounding generated parser stack frames.
#[derive(Debug, Subcommand)]
pub(crate) enum HierarchyCommands {
    /// Fit overdispersed counts with hierarchical rate variation.
    NegativeBinomialHierarchy(NegativeBinomialHierarchyArgs),
    /// Fit a Gaussian mean with a conjugate normal prior.
    NormalMean(NormalMeanArgs),
    /// Fit patient success counts with beta-binomial overdispersion.
    BetaBinomialHierarchy(BetaBinomialHierarchyArgs),
    /// Compare PyMC and NumPyro posterior estimates for hierarchical beta-binomial counts.
    BetaBinomialHierarchyAgreement(BetaBinomialHierarchyAgreementArgs),
    /// Assess sensitivity to declared prior and model settings for hierarchical beta-binomial counts.
    BetaBinomialHierarchySensitivity(BetaBinomialHierarchySensitivityArgs),
    /// Check posterior calibration with simulated data for hierarchical beta-binomial counts.
    BetaBinomialHierarchySbc(BetaBinomialHierarchySbcArgs),
    /// Estimate a group contrast in beta-binomial counts.
    BetaBinomialGroupRegression(BetaBinomialGroupRegressionArgs),
    /// Estimate group and gender effects in beta-binomial counts.
    BetaBinomialGroupGenderRegression(BetaBinomialGroupGenderRegressionArgs),
    /// Compare PyMC and NumPyro posterior estimates for beta-binomial group and gender effects.
    BetaBinomialGroupGenderRegressionAgreement(BetaBinomialGroupGenderRegressionAgreementArgs),
    /// Assess sensitivity to declared prior and model settings for beta-binomial group and gender effects.
    BetaBinomialGroupGenderRegressionSensitivity(BetaBinomialGroupGenderRegressionSensitivityArgs),
    /// Check posterior calibration with simulated data for beta-binomial group and gender effects.
    BetaBinomialGroupGenderRegressionSbc(BetaBinomialGroupGenderRegressionSbcArgs),
    /// Fit beta-binomial counts with patient and slide variation.
    BetaBinomialGroupGenderSlideHierarchy(BetaBinomialGroupGenderSlideHierarchyArgs),
    /// Compare PyMC and NumPyro posterior estimates for beta-binomial group contrasts.
    BetaBinomialGroupRegressionAgreement(BetaBinomialGroupRegressionAgreementArgs),
    /// Assess sensitivity to declared prior and model settings for beta-binomial group contrasts.
    BetaBinomialGroupRegressionSensitivity(BetaBinomialGroupRegressionSensitivityArgs),
    /// Check posterior calibration with simulated data for beta-binomial group contrasts.
    BetaBinomialGroupRegressionSbc(BetaBinomialGroupRegressionSbcArgs),
    /// Fit a robust Student-t hierarchy across patients.
    StudentTHierarchy(StudentTHierarchyArgs),
    /// Compare PyMC and NumPyro posterior estimates for a Student-t patient hierarchy.
    StudentTHierarchyAgreement(StudentTHierarchyAgreementArgs),
    /// Assess sensitivity to declared prior and model settings for a Student-t patient hierarchy.
    StudentTHierarchySensitivity(StudentTHierarchySensitivityArgs),
    /// Check posterior calibration with simulated data for a Student-t patient hierarchy.
    StudentTHierarchySbc(StudentTHierarchySbcArgs),
    /// Fit a Gaussian hierarchy across patients.
    HierarchicalNormal(HierarchicalNormalArgs),
    /// Compare PyMC and NumPyro posterior estimates for a Gaussian patient hierarchy.
    HierarchicalNormalAgreement(HierarchicalNormalAgreementArgs),
    /// Assess prior sensitivity for a Gaussian patient hierarchy.
    HierarchicalNormalPriorSensitivity(HierarchicalNormalPriorSensitivityArgs),
    /// Check posterior calibration with simulated data for a Gaussian patient hierarchy.
    HierarchicalNormalSbc(HierarchicalNormalSbcArgs),
    /// Pool study estimates while accounting for between-study heterogeneity.
    MetaAnalysis(MetaAnalysisArgs),
    /// Compare PyMC and NumPyro posterior estimates for a beta-binomial patient/slide hierarchy.
    BetaBinomialGroupGenderSlideHierarchyAgreement(
        Box<BetaBinomialGroupGenderSlideHierarchyAgreementArgs>,
    ),
    /// Assess sensitivity to declared prior and model settings for a beta-binomial patient/slide hierarchy.
    BetaBinomialGroupGenderSlideHierarchySensitivity(
        Box<BetaBinomialGroupGenderSlideHierarchySensitivityArgs>,
    ),
    /// Check posterior calibration with simulated data for a beta-binomial patient/slide hierarchy.
    BetaBinomialGroupGenderSlideHierarchySbc(Box<BetaBinomialGroupGenderSlideHierarchySbcArgs>),
    /// Estimate group differences in multitype count compositions.
    DirichletMultinomialGroup(Box<DirichletMultinomialGroupArgs>),
    /// Compare PyMC and NumPyro posterior estimates for Dirichlet-multinomial group compositions.
    DirichletMultinomialGroupAgreement(Box<DirichletMultinomialGroupAgreementArgs>),
    /// Assess sensitivity to declared prior and model settings for Dirichlet-multinomial group compositions.
    DirichletMultinomialGroupSensitivity(Box<DirichletMultinomialGroupSensitivityArgs>),
    /// Check posterior calibration with simulated data for Dirichlet-multinomial group compositions.
    DirichletMultinomialGroupSbc(Box<DirichletMultinomialGroupSbcArgs>),
    /// Fit a Gaussian model with crossed and nested grouping effects.
    GaussianCrossedNestedHierarchy(Box<gaussian_crossed_nested_hierarchy::Options>),
    /// Separate group effects on presence and positive abundance.
    HurdleBetaBinomialGroup(Box<hurdle_beta_binomial_group::HurdleBetaBinomialGroupArgs>),
    /// Allow group effects to vary across ordered-outcome cutpoints.
    NonproportionalOrdinalGroupSite(
        Box<nonproportional_ordinal_group_site::NonproportionalOrdinalGroupSiteArgs>,
    ),
    /// Estimate group effects on ordered outcomes.
    OrdinalGroup(Box<ordinal_group::OrdinalGroupArgs>),
    /// Fit ordered outcomes with site variation.
    OrdinalGroupSiteHierarchy(Box<ordinal_group_site_hierarchy::OrdinalGroupSiteHierarchyArgs>),
    /// Compare ordinal models by holding out entire sites.
    OrdinalSiteHeldoutComparison(Box<ordinal_site_heldout::OrdinalSiteHeldoutComparisonArgs>),
}

#[derive(Debug, Subcommand)]
pub(crate) enum FieldsCommands {
    /// Fit a Gaussian process to spatial observations.
    GpRegression(GpRegressionArgs),
    /// Fit a 3-D Gaussian process with separate axis length scales.
    #[command(name = "anisotropic-gp-3d")]
    AnisotropicGp3d(AnisotropicGp3dArgs),
    /// Fit correlated spatial outputs with a shared Gaussian process.
    MultiOutputGp(MultiOutputGpArgs),
    /// Fit an inducing-point Gaussian process approximation.
    VariationalGp(VariationalGpArgs),
    /// Construct a low-rank spatial predictive-process approximation.
    PredictiveProcess(PredictiveProcessArgs),
    /// Evaluate a nearest-neighbor Gaussian process density.
    NngpDensity(NngpDensityArgs),
    /// Validate a spatial weight matrix and its normalization policy.
    ValidateWeights(ValidateWeightsArgs),
    /// Evaluate a conditional autoregressive field density.
    CarDensity(CarDensityArgs),
    /// Fit a sparse conditional autoregressive spatial model.
    SparseCarFit(SparseCarFitArgs),
    /// Evaluate a Gaussian Markov random field density under declared constraints.
    GmrfDensity(GmrfDensityArgs),
    /// Evaluate a simultaneous autoregressive spatial likelihood.
    SarLikelihood(SarLikelihoodArgs),
    /// Fit a simultaneous autoregressive spatial regression.
    SarFit(SarFitArgs),
    /// Fit structured and unstructured spatial effects with a BYM model.
    BymFit(BymFitArgs),
    /// Fit scaled spatial and independent effects with a BYM2 model.
    Bym2Fit(Bym2FitArgs),
    /// Fit a regression whose selected coefficient varies over space.
    SpatialVaryingCoefficient(SpatialVaryingCoefficientArgs),
    /// Model measurements as a function of distance to a resource.
    DistanceToResource(DistanceToResourceArgs),
}

#[derive(Debug, Subcommand)]
pub(crate) enum SimulationInferenceCommands {
    /// Infer a growth rate by rejection ABC against simulated front summaries.
    RejectionAbcGrowthFront(RejectionAbcGrowthFrontArgs),
    /// Infer a growth rate with sequentially tightened ABC tolerances.
    SmcAbcGrowthFront(SmcAbcGrowthFrontArgs),
    /// Infer a growth rate with a simulated-summary Gaussian likelihood.
    SyntheticLikelihoodGrowthFront(SyntheticLikelihoodGrowthFrontArgs),
    /// Check rejection-ABC calibration on simulated growth fronts.
    GrowthFrontRejectionAbcSbc(GrowthFrontRejectionAbcSbcArgs),
    /// Check whether observed summaries lie within simulation support.
    SimulationOod(SimulationOodArgs),
    /// Compare observed growth summaries with posterior predictive simulations.
    GrowthFrontPosteriorPredictiveLab(GrowthFrontPosteriorPredictiveLabArgs),
}

#[derive(Debug, Subcommand)]
pub(crate) enum InferenceCommands {
    /// Estimate a Gaussian-mean posterior with sequential Monte Carlo.
    NormalMeanSmc(NormalMeanSmcArgs),
    /// Approximate a Poisson log-rate posterior with Laplace's method.
    PoissonLogRateLaplace(PoissonLogRateLaplaceArgs),
    /// Integrate a Poisson-lognormal posterior over a precision grid.
    PoissonLognormalInla(PoissonLognormalInlaArgs),
    /// Estimate leave-one-out predictive performance with Pareto-smoothed weights.
    PsisLoo(PsisLooArgs),
    /// Rank 2 to 16 models by PSIS-LOO expected log predictive density.
    CompareModels(CompareModelsArgs),
    /// Check posterior calibration with simulated data for a Gaussian mean.
    NormalMeanSbc(NormalMeanSbcArgs),
    /// Assess prior sensitivity for a Gaussian mean.
    NormalMeanPriorSensitivity(NormalMeanPriorSensitivityArgs),
}

#[derive(Debug, Subcommand)]
pub(crate) enum PointProcessesCommands {
    /// Evaluate a covariate-dependent Poisson point-process likelihood.
    InhomogeneousPoissonLikelihood(InhomogeneousPoissonLikelihoodArgs),
    /// Estimate covariate effects in a Poisson intensity model.
    FitInhomogeneousPoisson(FitInhomogeneousPoissonArgs),
    /// Check Poisson quadrature accuracy across grid refinements.
    BermanTurnerRefinement(BermanTurnerRefinementArgs),
    /// Prepare a gridded log-Gaussian Cox-process model.
    BuildGriddedLgcp(BuildGriddedLgcpArgs),
    /// Fit a log-Gaussian Cox-process intensity on a rectangular grid.
    FitGriddedLgcp(FitGriddedLgcpArgs),
    /// Compare PyMC and NumPyro posterior estimates for a gridded spatial intensity model.
    GriddedLgcpAgreement(GriddedLgcpAgreementArgs),
    /// Check posterior calibration with simulated data for a gridded spatial intensity model.
    GriddedLgcpSbc(GriddedLgcpSbcArgs),
    /// Assess sensitivity to declared prior and model settings for a gridded spatial intensity model.
    GriddedLgcpSensitivity(GriddedLgcpSensitivityArgs),
    /// Compare observed spatial summaries with posterior predictions for a gridded spatial intensity model.
    GriddedLgcpSpatialPpc(GriddedLgcpSpatialPpcArgs),
    /// Draw point patterns from a fitted gridded intensity model.
    SimulateGriddedLgcpPosteriorPredictive(SimulateGriddedLgcpPosteriorPredictiveArgs),
    /// Simulate Gaussian offspring around Poisson-distributed parent points.
    SimulateThomasProcess(SimulateThomasProcessArgs),
    /// Simulate offspring uniformly within disks around parent points.
    SimulateMaternClusterProcess(SimulateMaternClusterProcessArgs),
    /// Fit Thomas-process parameters by minimum contrast to a K curve.
    FitThomasMinimumContrast(FitThomasMinimumContrastArgs),
    /// Compute interaction statistics for a Strauss point process.
    StraussStatistics(StraussStatisticsArgs),
    /// Simulate a Strauss process with birth-death updates.
    SimulateStraussBirthDeath(SimulateStraussBirthDeathArgs),
    /// Estimate a Strauss model by quadrature pseudolikelihood.
    FitStraussPseudolikelihood(FitStraussPseudolikelihoodArgs),
    /// Compute the saturated interaction statistic for a Geyer model.
    GeyerSaturationStatistic(GeyerSaturationStatisticArgs),
    /// Evaluate conditional intensities for a multitype interaction model.
    MultitypePapangelou(MultitypePapangelouArgs),
    /// Prepare a joint spatial-location and binary-mark model.
    BuildJointLocationMarkModel(BuildJointLocationMarkModelArgs),
    /// Prepare a joint spatial-location and continuous-mark model.
    BuildJointContinuousMarkModel(BuildJointContinuousMarkModelArgs),
    /// Prepare a joint location and embedding-factor model.
    BuildJointLocationEmbeddingFactorModel(BuildJointLocationEmbeddingFactorModelArgs),
    /// Prepare a hierarchical intensity model across replicated patterns.
    BuildReplicatedHierarchicalLgcp(BuildReplicatedHierarchicalLgcpArgs),
    /// Compare observed and simulated spatial point-process summaries.
    PointProcessPosteriorPredictiveDiagnostics(PointProcessPosteriorPredictiveDiagnosticsArgs),
}

#[derive(Debug, Subcommand)]
pub(crate) enum EmbeddingsCommands {
    /// Measure embedding-vector dissimilarity across distance bins.
    VectorSemivariogram(VectorSemivariogramArgs),
    /// Compute semivariograms after projecting embedding vectors.
    ProjectedEmbeddingVariograms(ProjectedEmbeddingVariogramsArgs),
    /// Measure cross-covariance between embedding features by distance.
    EmbeddingCrossCovarianceByDistance(EmbeddingCrossCovarianceByDistanceArgs),
    /// Measure distance-dependent covariance between two modalities.
    CrossModalCovarianceByDistance(CrossModalCovarianceByDistanceArgs),
    /// Measure spatial mark dependence with an embedding kernel.
    KernelMarkCorrelation(KernelMarkCorrelationArgs),
    /// Compare embedding dependence curves with permutation envelopes.
    EmbeddingSpatialDependenceEnvelope(EmbeddingSpatialDependenceEnvelopeArgs),
    /// Measure signal roughness across weighted graph edges.
    GraphDirichletEnergy(GraphDirichletEnergyArgs),
    /// Test graph-signal smoothness against permutations.
    GraphSmoothnessPermutationTest(GraphSmoothnessPermutationTestArgs),
    /// Measure local embedding variation around graph nodes.
    LocalEmbeddingRoughness(LocalEmbeddingRoughnessArgs),
    /// Test whether cell and patch embeddings add complementary information.
    TestCellPatchComplementarity(TestCellPatchComplementarityArgs),
    /// Combine embedding kernels across declared spatial scales.
    MultiscaleEmbeddingKernel(MultiscaleEmbeddingKernelArgs),
    /// Retrieve embedding-nearest regions under a declared leakage policy.
    RetrieveAnalogousRegions(RetrieveAnalogousRegionsArgs),
}

#[derive(Debug, Subcommand)]
pub(crate) enum PredictionCommands {
    /// Abstain from predictions that exceed declared uncertainty or support limits.
    ApplyAbstention(ApplyAbstentionArgs),
    /// Score observations against the training distribution.
    OodScore(OodScoreArgs),
    /// Calibrate predicted probabilities using held-out observations.
    CalibratePredictions(CalibratePredictionsArgs),
    /// Construct prediction sets with grouped conformal calibration.
    GroupedConformal(GroupedConformalArgs),
    /// Combine predictions from separate modalities.
    LateFusion(LateFusionArgs),
    /// Weight predictive models using their held-out predictive performance.
    PredictiveStacking(PredictiveStackingArgs),
    /// Fit a gated mixture of modality-specific predictive experts.
    MixtureOfExpertsFusion(MixtureOfExpertsFusionArgs),
}

#[derive(Debug, Subcommand)]
pub(crate) enum TransportCommands {
    /// Compute entropically regularized optimal transport.
    SinkhornOt(SinkhornOtArgs),
    /// Compute soft transport assignments with optional unmatched mass.
    EntropicSoftAssignment(EntropicSoftAssignmentArgs),
    /// Align samples using both feature costs and within-sample structure.
    FusedGromovWasserstein(FusedGromovWassersteinArgs),
    /// Align a declared mass using feature and structural transport costs.
    PartialFusedGromovWasserstein(PartialFusedGromovWassersteinArgs),
    /// Align feature and structure with KL-relaxed marginals (fused unbalanced GW).
    UnbalancedFusedGromovWasserstein(UnbalancedFusedGromovWassersteinArgs),
    /// Compute regularized transport while allowing marginal mass changes.
    UnbalancedSinkhorn(UnbalancedSinkhornArgs),
    /// Transport only a declared portion of the source and target mass.
    PartialOt(PartialOtArgs),
}

pub(crate) fn execute(command: BayesCommand) -> marklab::Result<()> {
    run_cli(command).map_err(into_marklab_error)
}

#[derive(Debug, Args)]
pub(crate) struct NegativeBinomialHierarchyArgs {
    /// JSON NegativeBinomialHierarchySpec: observations, priors, sampling and resource limits.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct StudentTHierarchyModelArgs {
    /// CSV with exactly patient_id and observation columns, one row per observation.
    #[arg(long)]
    pub(crate) input: PathBuf,
    /// Prior mean of the global mean.
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) global_prior_mean: f64,
    /// Prior standard deviation of the global mean.
    #[arg(long)]
    pub(crate) global_prior_sd: f64,
    /// Half-normal prior scale for the between-patient standard deviation.
    #[arg(long)]
    pub(crate) between_patient_sd_prior: f64,
    /// Half-normal prior scale for the observation standard deviation.
    #[arg(long)]
    pub(crate) observation_sd_prior: f64,
    /// Exponential prior rate for Student-t degrees of freedom above its lower bound.
    #[arg(long)]
    pub(crate) degrees_of_freedom_excess_rate: f64,
    #[command(flatten)]
    pub(crate) nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
}

#[derive(Debug, Args)]
pub(crate) struct StudentTHierarchyArgs {
    #[command(flatten)]
    pub(super) model: StudentTHierarchyModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct StudentTHierarchyAgreementArgs {
    #[command(flatten)]
    pub(super) model: StudentTHierarchyModelArgs,
    /// Largest standardized difference accepted between backend estimates.
    #[arg(long)]
    pub(super) maximum_standardized_difference: f64,
    /// Minimum absolute comparison tolerance for location scale.
    #[arg(long)]
    pub(super) minimum_location_scale_tolerance: f64,
    /// Minimum absolute comparison tolerance for degrees of freedom.
    #[arg(long)]
    pub(super) minimum_degrees_of_freedom_tolerance: f64,
    /// Minimum absolute comparison tolerance for patient.
    #[arg(long)]
    pub(super) minimum_patient_tolerance: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct StudentTHierarchySensitivityArgs {
    #[command(flatten)]
    pub(super) model: StudentTHierarchyModelArgs,
    /// Multiplier defining the lower prior-scale sensitivity scenario.
    #[arg(long)]
    pub(super) lower_scale_multiplier: f64,
    /// Multiplier defining the upper prior-scale sensitivity scenario.
    #[arg(long)]
    pub(super) upper_scale_multiplier: f64,
    /// Smallest standardized posterior change considered material.
    #[arg(long)]
    pub(super) material_standardized_shift: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct StudentTHierarchySbcArgs {
    #[command(flatten)]
    pub(super) model: StudentTHierarchyModelArgs,
    /// Number of simulated datasets used for posterior calibration.
    #[arg(long)]
    pub(super) replicates: u32,
    /// Smallest admitted p-value for simulation-based rank uniformity.
    #[arg(long)]
    pub(super) minimum_rank_uniformity_p_value: f64,
    /// Smallest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    pub(super) minimum_coverage_90: f64,
    /// Largest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    pub(super) maximum_coverage_90: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct BetaBinomialHierarchyModelArgs {
    /// CSV with patient_id, successes and trials columns, one row per patient.
    #[arg(long)]
    pub(crate) input: PathBuf,
    /// Population-probability beta prior first shape parameter.
    #[arg(long)]
    pub(crate) population_alpha: f64,
    /// Population-probability beta prior second shape parameter.
    #[arg(long)]
    pub(crate) population_beta: f64,
    /// Prior scale for concentration.
    #[arg(long)]
    pub(crate) concentration_prior_sd: f64,
    #[command(flatten)]
    pub(crate) nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
}

#[derive(Debug, Args)]
pub(crate) struct BetaBinomialHierarchyArgs {
    #[command(flatten)]
    pub(super) model: BetaBinomialHierarchyModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BetaBinomialHierarchyAgreementArgs {
    #[command(flatten)]
    pub(super) model: BetaBinomialHierarchyModelArgs,
    /// Largest standardized difference accepted between backend estimates.
    #[arg(long)]
    pub(super) maximum_standardized_difference: f64,
    /// Minimum absolute comparison tolerance for probability.
    #[arg(long)]
    pub(super) minimum_probability_tolerance: f64,
    /// Minimum absolute comparison tolerance for concentration.
    #[arg(long)]
    pub(super) minimum_concentration_tolerance: f64,
    /// Minimum absolute comparison tolerance for patient.
    #[arg(long)]
    pub(super) minimum_patient_tolerance: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BetaBinomialHierarchySensitivityArgs {
    #[command(flatten)]
    pub(super) model: BetaBinomialHierarchyModelArgs,
    /// Multiplier defining the lower prior-scale sensitivity scenario.
    #[arg(long)]
    pub(super) lower_scale_multiplier: f64,
    /// Multiplier defining the upper prior-scale sensitivity scenario.
    #[arg(long)]
    pub(super) upper_scale_multiplier: f64,
    /// Smallest standardized posterior change considered material.
    #[arg(long)]
    pub(super) material_standardized_shift: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BetaBinomialHierarchySbcArgs {
    #[command(flatten)]
    pub(super) model: BetaBinomialHierarchyModelArgs,
    /// Number of simulated datasets used for posterior calibration.
    #[arg(long)]
    pub(super) replicates: u32,
    /// Smallest admitted p-value for simulation-based rank uniformity.
    #[arg(long)]
    pub(super) minimum_rank_uniformity_p_value: f64,
    /// Smallest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    pub(super) minimum_coverage_90: f64,
    /// Largest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    pub(super) maximum_coverage_90: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct BetaBinomialGroupRegressionModelArgs {
    /// CSV with patient_id, group, successes and trials columns, one row per patient.
    #[arg(long)]
    pub(crate) input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    pub(crate) reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    pub(crate) comparison_group: String,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    pub(crate) intercept_prior_sd: f64,
    /// Prior scale for group effect.
    #[arg(long)]
    pub(crate) group_effect_prior_sd: f64,
    /// Prior scale for concentration.
    #[arg(long)]
    pub(crate) concentration_prior_sd: f64,
    #[command(flatten)]
    pub(crate) nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
}

#[derive(Debug, Args)]
pub(crate) struct BetaBinomialGroupRegressionArgs {
    #[command(flatten)]
    pub(super) model: BetaBinomialGroupRegressionModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct BetaBinomialGroupGenderRegressionModelArgs {
    /// CSV with patient_id, group, gender, successes and trials columns, one row per patient.
    #[arg(long)]
    pub(crate) input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    pub(crate) reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    pub(crate) comparison_group: String,
    /// Gender level used as the regression reference.
    #[arg(long)]
    pub(crate) reference_gender: String,
    /// Gender level whose effect is compared with the reference level.
    #[arg(long)]
    pub(crate) comparison_gender: String,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    pub(crate) intercept_prior_sd: f64,
    /// Prior scale for group effect.
    #[arg(long)]
    pub(crate) group_effect_prior_sd: f64,
    /// Prior scale for gender effect.
    #[arg(long)]
    pub(crate) gender_effect_prior_sd: f64,
    /// Prior scale for concentration.
    #[arg(long)]
    pub(crate) concentration_prior_sd: f64,
    #[command(flatten)]
    pub(crate) nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
}

#[derive(Debug, Args)]
pub(crate) struct BetaBinomialGroupGenderRegressionArgs {
    #[command(flatten)]
    pub(super) model: BetaBinomialGroupGenderRegressionModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BetaBinomialGroupGenderRegressionAgreementArgs {
    #[command(flatten)]
    pub(super) model: BetaBinomialGroupGenderRegressionModelArgs,
    /// Largest standardized difference accepted between backend estimates.
    #[arg(long)]
    pub(super) maximum_standardized_difference: f64,
    /// Minimum absolute comparison tolerance for probability.
    #[arg(long)]
    pub(super) minimum_probability_tolerance: f64,
    /// Minimum absolute comparison tolerance for log odds.
    #[arg(long)]
    pub(super) minimum_log_odds_tolerance: f64,
    /// Minimum absolute comparison tolerance for concentration.
    #[arg(long)]
    pub(super) minimum_concentration_tolerance: f64,
    /// Minimum absolute comparison tolerance for patient.
    #[arg(long)]
    pub(super) minimum_patient_tolerance: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BetaBinomialGroupGenderRegressionSbcArgs {
    #[command(flatten)]
    pub(super) model: BetaBinomialGroupGenderRegressionModelArgs,
    /// Number of simulated datasets used for posterior calibration.
    #[arg(long)]
    pub(super) replicates: u32,
    /// Smallest admitted p-value for simulation-based rank uniformity.
    #[arg(long)]
    pub(super) minimum_rank_uniformity_p_value: f64,
    /// Smallest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    pub(super) minimum_coverage_90: f64,
    /// Largest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    pub(super) maximum_coverage_90: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct BetaBinomialGroupGenderSlideHierarchyModelArgs {
    /// CSV with slide_id, patient_id, group, gender, successes and trials, one row per slide.
    #[arg(long)]
    pub(crate) input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    pub(crate) reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    pub(crate) comparison_group: String,
    /// Gender level used as the regression reference.
    #[arg(long)]
    pub(crate) reference_gender: String,
    /// Gender level whose effect is compared with the reference level.
    #[arg(long)]
    pub(crate) comparison_gender: String,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    pub(crate) intercept_prior_sd: f64,
    /// Prior scale for group effect.
    #[arg(long)]
    pub(crate) group_effect_prior_sd: f64,
    /// Prior scale for gender effect.
    #[arg(long)]
    pub(crate) gender_effect_prior_sd: f64,
    /// Prior scale for patient log odds standard deviation.
    #[arg(long)]
    pub(crate) patient_log_odds_sd_prior_sd: f64,
    /// Prior scale for slide concentration.
    #[arg(long)]
    pub(crate) slide_concentration_prior_sd: f64,
    #[command(flatten)]
    pub(crate) nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
}

#[derive(Debug, Args)]
pub(crate) struct BetaBinomialGroupGenderSlideHierarchyArgs {
    #[command(flatten)]
    pub(super) model: BetaBinomialGroupGenderSlideHierarchyModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BetaBinomialGroupRegressionAgreementArgs {
    #[command(flatten)]
    pub(super) model: BetaBinomialGroupRegressionModelArgs,
    /// Largest standardized difference accepted between backend estimates.
    #[arg(long)]
    pub(super) maximum_standardized_difference: f64,
    /// Minimum absolute comparison tolerance for probability.
    #[arg(long)]
    pub(super) minimum_probability_tolerance: f64,
    /// Minimum absolute comparison tolerance for log odds.
    #[arg(long)]
    pub(super) minimum_log_odds_tolerance: f64,
    /// Minimum absolute comparison tolerance for concentration.
    #[arg(long)]
    pub(super) minimum_concentration_tolerance: f64,
    /// Minimum absolute comparison tolerance for patient.
    #[arg(long)]
    pub(super) minimum_patient_tolerance: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BetaBinomialGroupRegressionSensitivityArgs {
    #[command(flatten)]
    pub(super) model: BetaBinomialGroupRegressionModelArgs,
    /// Multiplier defining the lower prior-scale sensitivity scenario.
    #[arg(long)]
    pub(super) lower_scale_multiplier: f64,
    /// Multiplier defining the upper prior-scale sensitivity scenario.
    #[arg(long)]
    pub(super) upper_scale_multiplier: f64,
    /// Smallest standardized posterior change considered material.
    #[arg(long)]
    pub(super) material_standardized_shift: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BetaBinomialGroupRegressionSbcArgs {
    #[command(flatten)]
    pub(super) model: BetaBinomialGroupRegressionModelArgs,
    /// Number of simulated datasets used for posterior calibration.
    #[arg(long)]
    pub(super) replicates: u32,
    /// Smallest admitted p-value for simulation-based rank uniformity.
    #[arg(long)]
    pub(super) minimum_rank_uniformity_p_value: f64,
    /// Smallest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    pub(super) minimum_coverage_90: f64,
    /// Largest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    pub(super) maximum_coverage_90: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct HierarchicalNormalModelArgs {
    /// Prior mean of the global mean.
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) global_prior_mean: f64,
    /// Prior standard deviation of the global mean.
    #[arg(long)]
    pub(crate) global_prior_sd: f64,
    /// Half-normal prior scale for the between-patient standard deviation.
    #[arg(long)]
    pub(crate) between_patient_sd_prior: f64,
    /// Known standard deviation of Gaussian observation noise.
    #[arg(long)]
    pub(crate) known_sigma: f64,
    #[command(flatten)]
    pub(crate) nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
}

#[derive(Debug, Args)]
pub(crate) struct HierarchicalNormalArgs {
    /// CSV with exactly patient_id and observation columns, one row per observation.
    #[arg(long)]
    pub(super) input: PathBuf,
    #[command(flatten)]
    pub(super) model: HierarchicalNormalModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct HierarchicalNormalAgreementArgs {
    /// CSV with exactly patient_id and observation columns, one row per observation.
    #[arg(long)]
    pub(super) input: PathBuf,
    #[command(flatten)]
    pub(super) model: HierarchicalNormalModelArgs,
    /// Largest standardized difference accepted between backend estimates.
    #[arg(long)]
    pub(super) maximum_standardized_difference: f64,
    /// Minimum absolute comparison tolerance for absolute.
    #[arg(long)]
    pub(super) minimum_absolute_tolerance: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct HierarchicalNormalPriorSensitivityArgs {
    /// CSV with exactly patient_id and observation columns, one row per observation.
    #[arg(long)]
    pub(super) input: PathBuf,
    #[command(flatten)]
    pub(super) model: HierarchicalNormalModelArgs,
    /// Multiplier defining the lower prior-scale sensitivity scenario.
    #[arg(long)]
    pub(super) lower_scale_multiplier: f64,
    /// Multiplier defining the upper prior-scale sensitivity scenario.
    #[arg(long)]
    pub(super) upper_scale_multiplier: f64,
    /// Smallest standardized posterior change considered material.
    #[arg(long)]
    pub(super) material_standardized_shift: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct HierarchicalNormalSbcArgs {
    #[command(flatten)]
    pub(super) model: HierarchicalNormalModelArgs,
    /// Number of patients simulated for each calibration replicate.
    #[arg(long)]
    pub(super) patients: u32,
    /// Number of observations simulated for each patient.
    #[arg(long)]
    pub(super) observations_per_patient: u32,
    /// Number of simulated datasets used for posterior calibration.
    #[arg(long)]
    pub(super) replicates: u32,
    /// Smallest admitted p-value for simulation-based rank uniformity.
    #[arg(long)]
    pub(super) minimum_rank_uniformity_p_value: f64,
    /// Smallest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    pub(super) minimum_coverage_90: f64,
    /// Largest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    pub(super) maximum_coverage_90: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct MetaAnalysisArgs {
    /// CSV with exactly site_id, effect, standard_error and covariate columns, one row per site.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Name of the covariate included in the model.
    #[arg(long)]
    pub(super) covariate_name: String,
    /// Covariate value for prediction at a new site.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) new_site_covariate: f64,
    /// Prior mean of the global mean.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) global_prior_mean: f64,
    /// Prior standard deviation of the global mean.
    #[arg(long)]
    pub(super) global_prior_sd: f64,
    /// Prior scale for covariate.
    #[arg(long)]
    pub(super) covariate_prior_sd: f64,
    /// Prior scale for heterogeneity.
    #[arg(long)]
    pub(super) heterogeneity_prior_sd: f64,
    #[command(flatten)]
    pub(super) nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct GpRegressionArgs {
    /// CSV with exactly observation_id, x_um and value columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Locations and covariates at which predictions are requested.
    #[arg(long)]
    pub(super) predict: PathBuf,
    /// Prior mean of the constant GP mean function.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) mean_prior_mean: f64,
    /// Prior standard deviation of the constant GP mean function.
    #[arg(long)]
    pub(super) mean_prior_sd: f64,
    /// Prior scale for amplitude.
    #[arg(long)]
    pub(super) amplitude_prior_sd: f64,
    /// Prior scale, in micrometers, for length scale.
    #[arg(long)]
    pub(super) length_scale_prior_sd_um: f64,
    /// Prior scale for noise.
    #[arg(long)]
    pub(super) noise_prior_sd: f64,
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

#[derive(Debug, Args)]
pub(crate) struct AnisotropicGp3dArgs {
    /// CSV with exactly observation_id, x_um, y_um, z_um and value columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Locations and covariates at which predictions are requested.
    #[arg(long)]
    pub(super) predict: PathBuf,
    /// Prior mean of the constant GP mean function.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) mean_prior_mean: f64,
    /// Prior standard deviation of the constant GP mean function.
    #[arg(long)]
    pub(super) mean_prior_sd: f64,
    /// Prior scale for amplitude.
    #[arg(long)]
    pub(super) amplitude_prior_sd: f64,
    /// Prior scale, in micrometers, for length scale x.
    #[arg(long)]
    pub(super) length_scale_x_prior_sd_um: f64,
    /// Prior scale, in micrometers, for length scale y.
    #[arg(long)]
    pub(super) length_scale_y_prior_sd_um: f64,
    /// Prior scale, in micrometers, for length scale z.
    #[arg(long)]
    pub(super) length_scale_z_prior_sd_um: f64,
    /// Prior scale for noise.
    #[arg(long)]
    pub(super) noise_prior_sd: f64,
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

#[derive(Debug, Args)]
pub(crate) struct MultiOutputGpArgs {
    /// CSV with exactly coordinate_id, x_um, output_a and output_b columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Locations and covariates at which predictions are requested.
    #[arg(long)]
    pub(super) predict: PathBuf,
    /// Name identifying the first modeled output.
    #[arg(long)]
    pub(super) output_a_name: String,
    /// Name identifying the second modeled output.
    #[arg(long)]
    pub(super) output_b_name: String,
    /// Prior standard deviation of the constant GP mean function.
    #[arg(long)]
    pub(super) mean_prior_sd: f64,
    /// Prior scale for amplitude.
    #[arg(long)]
    pub(super) amplitude_prior_sd: f64,
    /// Prior scale, in micrometers, for length scale.
    #[arg(long)]
    pub(super) length_scale_prior_sd_um: f64,
    /// Prior scale for loading b.
    #[arg(long)]
    pub(super) loading_b_prior_sd: f64,
    /// Observation-noise standard deviation for the first output.
    #[arg(long)]
    pub(super) noise_a_sd: f64,
    /// Observation-noise standard deviation for the second output.
    #[arg(long)]
    pub(super) noise_b_sd: f64,
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

#[derive(Debug, Args)]
pub(crate) struct VariationalGpArgs {
    /// CSV with exactly observation_id, x_um and value columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Locations and covariates at which predictions are requested.
    #[arg(long)]
    pub(super) predict: PathBuf,
    /// Prior mean of the constant GP mean function.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) mean_prior_mean: f64,
    /// Prior standard deviation of the constant GP mean function.
    #[arg(long)]
    pub(super) mean_prior_sd: f64,
    /// Prior scale for amplitude.
    #[arg(long)]
    pub(super) amplitude_prior_sd: f64,
    /// Prior scale, in micrometers, for length scale.
    #[arg(long)]
    pub(super) length_scale_prior_sd_um: f64,
    /// Prior scale for noise.
    #[arg(long)]
    pub(super) noise_prior_sd: f64,
    /// Diagonal stabilization added to covariance matrices.
    #[arg(long)]
    pub(super) jitter: f64,
    /// Number of inducing locations in the variational Gaussian process.
    #[arg(long)]
    pub(super) inducing_points: u32,
    /// Number of optimization initializations.
    #[arg(long)]
    pub(super) starts: u32,
    /// Number of optimization or sampling iterations.
    #[arg(long)]
    pub(super) iterations: u32,
    /// Step size for variational optimization.
    #[arg(long)]
    pub(super) learning_rate: f64,
    /// Posterior draws used to propagate fitted uncertainty.
    #[arg(long)]
    pub(super) posterior_draws: u32,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct PredictiveProcessArgs {
    /// CSV with exactly coordinate_id and x_um columns, one row per field location.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Spatial knot locations for the predictive-process approximation.
    #[arg(long)]
    pub(super) knots: PathBuf,
    /// Marginal amplitude of the spatial covariance kernel.
    #[arg(long)]
    pub(super) amplitude: f64,
    /// Spatial covariance length scale in micrometers.
    #[arg(long)]
    pub(super) length_scale_um: f64,
    /// Diagonal stabilization added to covariance matrices.
    #[arg(long)]
    pub(super) jitter: f64,
    /// Apply the predictive-process diagonal variance correction.
    #[arg(long)]
    pub(super) diagonal_correction: bool,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct NngpDensityArgs {
    /// CSV with exactly coordinate_id, x_um and field_value columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Mean of the Gaussian-process field.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) mean: f64,
    /// Marginal amplitude of the spatial covariance kernel.
    #[arg(long)]
    pub(super) amplitude: f64,
    /// Spatial covariance length scale in micrometers.
    #[arg(long)]
    pub(super) length_scale_um: f64,
    /// Number of nearest neighbors retained in the approximation.
    #[arg(long)]
    pub(super) neighbors: usize,
    /// Diagonal stabilization added to covariance matrices.
    #[arg(long)]
    pub(super) jitter: f64,
    /// Tolerance for detecting negligible or inconsistent variance.
    #[arg(long)]
    pub(super) variance_tolerance: f64,
    /// Compute the full Gaussian-process reference for comparison.
    #[arg(long)]
    pub(super) full_reference: bool,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct ValidateWeightsArgs {
    /// Declared regions and their memberships.
    #[arg(long)]
    pub(super) regions: PathBuf,
    /// Table of spatial graph edges and their declared weights.
    #[arg(long)]
    pub(super) edges: PathBuf,
    /// Policy for symmetry of the spatial weight matrix.
    #[arg(long, value_enum)]
    pub(super) symmetry: weights::CliSymmetry,
    /// Policy for diagonal entries of the spatial weight matrix.
    #[arg(long, value_enum)]
    pub(super) diagonal: weights::CliDiagonal,
    /// Normalization rule for the spatial weight matrix.
    #[arg(long, value_enum)]
    pub(super) normalization: weights::CliNormalization,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct CarDensityArgs {
    /// Declared regions and their memberships.
    #[arg(long)]
    pub(super) regions: PathBuf,
    /// Table of spatial graph edges and their declared weights.
    #[arg(long)]
    pub(super) edges: PathBuf,
    /// Latent spatial field values whose density is evaluated.
    #[arg(long)]
    pub(super) field: PathBuf,
    /// Conditional autoregressive model variant.
    #[arg(long, value_enum)]
    pub(super) mode: car::CliCarMode,
    /// Precision parameter of the conditional autoregressive field.
    #[arg(long)]
    pub(super) tau: f64,
    /// Spatial dependence coefficient.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) rho: f64,
    /// Numerical tolerance for the declared linear constraints.
    #[arg(long)]
    pub(super) constraint_tolerance: f64,
    /// Treatment of graph nodes with no neighbors.
    #[arg(long, value_enum)]
    pub(super) island_policy: car::CliIslandPolicy,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct SparseCarFitArgs {
    /// JSON SparseCarSpec: observations, edges, rho, tau, draws, seed and solver limits.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct GmrfDensityArgs {
    /// Declared regions and their memberships.
    #[arg(long)]
    pub(super) regions: PathBuf,
    /// Precision of the Gaussian field or declared prior.
    #[arg(long)]
    pub(super) precision: PathBuf,
    /// Latent spatial field values whose density is evaluated.
    #[arg(long)]
    pub(super) field: PathBuf,
    /// File containing linear constraints on the latent field.
    #[arg(long)]
    pub(super) constraints: PathBuf,
    /// Numerical tolerance for the declared linear constraints.
    #[arg(long)]
    pub(super) constraint_tolerance: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct SarLikelihoodArgs {
    /// Declared regions and their memberships.
    #[arg(long)]
    pub(super) regions: PathBuf,
    /// Table of spatial graph edges and their declared weights.
    #[arg(long)]
    pub(super) edges: PathBuf,
    /// Observation table and regression covariates for the spatial model.
    #[arg(long)]
    pub(super) data: PathBuf,
    /// Declared regression coefficients.
    #[arg(long)]
    pub(super) coefficients: PathBuf,
    /// Model variant used for the declared spatial or multisite analysis.
    #[arg(long, value_enum)]
    pub(super) model: sar::CliSarModel,
    /// Spatial dependence coefficient.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) rho: f64,
    /// Residual standard deviation of the spatial autoregressive model.
    #[arg(long)]
    pub(super) sigma: f64,
    /// Interpretation of the supplied spatial weight matrix.
    #[arg(long, value_enum)]
    pub(super) interpretation: sar::CliSarInterpretation,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct SarFitArgs {
    /// Declared regions and their memberships.
    #[arg(long)]
    pub(super) regions: PathBuf,
    /// Table of spatial graph edges and their declared weights.
    #[arg(long)]
    pub(super) edges: PathBuf,
    /// Observation table and regression covariates for the spatial model.
    #[arg(long)]
    pub(super) data: PathBuf,
    /// Model variant used for the declared spatial or multisite analysis.
    #[arg(long, value_enum)]
    pub(super) model: sar::CliSarModel,
    /// Interpretation of the supplied spatial weight matrix.
    #[arg(long, value_enum)]
    pub(super) interpretation: sar::CliSarInterpretation,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    pub(super) intercept_prior_sd: f64,
    /// Prior scale for coefficient.
    #[arg(long)]
    pub(super) coefficient_prior_sd: f64,
    /// Absolute bound on the spatial dependence coefficient.
    #[arg(long)]
    pub(super) rho_bound: f64,
    /// Prior scale for sigma.
    #[arg(long)]
    pub(super) sigma_prior_sd: f64,
    #[command(flatten)]
    pub(super) nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BymFitArgs {
    /// Declared regions and their memberships.
    #[arg(long)]
    pub(super) regions: PathBuf,
    /// Table of spatial graph edges and their declared weights.
    #[arg(long)]
    pub(super) edges: PathBuf,
    /// Observation table and regression covariates for the spatial model.
    #[arg(long)]
    pub(super) data: PathBuf,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    pub(super) intercept_prior_sd: f64,
    /// Prior scale for coefficient.
    #[arg(long)]
    pub(super) coefficient_prior_sd: f64,
    /// Half-normal prior scale for the structured (ICAR) effect standard deviation.
    #[arg(long)]
    pub(super) structured_sd_prior: f64,
    /// Half-normal prior scale for the unstructured effect standard deviation.
    #[arg(long)]
    pub(super) unstructured_sd_prior: f64,
    #[command(flatten)]
    pub(super) nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct Bym2FitArgs {
    /// Declared regions and their memberships.
    #[arg(long)]
    pub(super) regions: PathBuf,
    /// Table of spatial graph edges and their declared weights.
    #[arg(long)]
    pub(super) edges: PathBuf,
    /// Observation table and regression covariates for the spatial model.
    #[arg(long)]
    pub(super) data: PathBuf,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    pub(super) intercept_prior_sd: f64,
    /// Prior scale for coefficient.
    #[arg(long)]
    pub(super) coefficient_prior_sd: f64,
    /// Half-normal prior scale for the total spatial effect standard deviation.
    #[arg(long)]
    pub(super) total_sd_prior: f64,
    /// Beta prior first shape for the BYM2 mixing fraction.
    #[arg(long)]
    pub(super) phi_alpha: f64,
    /// Beta prior second shape for the BYM2 mixing fraction.
    #[arg(long)]
    pub(super) phi_beta: f64,
    #[command(flatten)]
    pub(super) nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct SpatialVaryingCoefficientArgs {
    /// CSV with coordinate_id, x_um, y and the two named predictor columns, in that order.
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

#[derive(Debug, Args)]
pub(crate) struct DistanceToResourceArgs {
    /// JSON with resources, spline_knots_um and observations arrays.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Prior scale for coefficient.
    #[arg(long)]
    pub(super) coefficient_prior_sd: f64,
    /// Prior scale for patient effect.
    #[arg(long)]
    pub(super) patient_effect_prior_sd: f64,
    /// Known observation-noise standard deviation.
    #[arg(long)]
    pub(super) known_noise_sd: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct SmcAbcGrowthFrontArgs {
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

#[derive(Debug, Args)]
pub(crate) struct SimulationOodArgs {
    /// JSON SimulationOodSpec: feature_names, reference and calibration simulations, observed.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct NormalMeanSmcArgs {
    /// CSV with a single observation column, one row per observation.
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
    /// Number of particles retained by the inference algorithm.
    #[arg(long)]
    pub(super) particles: u32,
    /// Number of independent posterior sampling chains.
    #[arg(long)]
    pub(super) chains: u32,
    /// Effective sample-size target for sequential Monte Carlo adaptation.
    #[arg(long)]
    pub(super) ess_target: f64,
    /// Correlation threshold used by the sequential Monte Carlo adaptation.
    #[arg(long)]
    pub(super) correlation_threshold: f64,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct PoissonLogRateLaplaceArgs {
    /// CSV with exactly observation_id, count and exposure columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Mean of the prior distribution.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) prior_mean: f64,
    /// Scale of the prior distribution.
    #[arg(long)]
    pub(super) prior_sd: f64,
    /// Initial log-rate for posterior mode optimization.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) initial_log_rate: f64,
    /// Upper bound on iterations.
    #[arg(long)]
    pub(super) max_iterations: u32,
    /// Maximum gradient magnitude accepted at the optimized solution.
    #[arg(long)]
    pub(super) gradient_tolerance: f64,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct PoissonLognormalInlaArgs {
    /// CSV with exactly region_id, count and exposure columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Prior mean of the latent log-rate.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) latent_mean: f64,
    /// Shape of the gamma prior on latent precision.
    #[arg(long)]
    pub(super) tau_shape: f64,
    /// Rate of the gamma prior on latent precision.
    #[arg(long)]
    pub(super) tau_rate: f64,
    /// Lower endpoint of the log-precision integration grid.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) log_tau_min: f64,
    /// Upper endpoint of the log-precision integration grid.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) log_tau_max: f64,
    /// Number of points in the log-precision integration grid.
    #[arg(long)]
    pub(super) grid_points: u32,
    /// Maximum posterior mass allowed at the precision-grid endpoints.
    #[arg(long)]
    pub(super) endpoint_mass_limit: f64,
    #[command(flatten)]
    pub(super) nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct PsisLooArgs {
    /// CSV of pointwise draws with exactly chain, draw, unit_id and log_likelihood columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Name identifying the candidate predictive model.
    #[arg(long)]
    pub(super) model_name: String,
    /// Likelihood contribution used for predictive model comparison.
    #[arg(long)]
    pub(super) likelihood_target: String,
    /// Expected digest identifying the data used by the fitted models.
    #[arg(long)]
    pub(super) data_identity_sha256: String,
    /// Expected digest identifying the preprocessing used by the models.
    #[arg(long)]
    pub(super) preprocessing_identity_sha256: String,
    /// Sampling unit held out for predictive evaluation.
    #[arg(long)]
    pub(super) heldout_unit: String,
    /// Relative sampling efficiency used by Pareto-smoothed importance sampling.
    #[arg(long)]
    pub(super) relative_efficiency: f64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct CompareModelsArgs {
    /// A `bayes psis-loo` result file for one model; repeat the flag for each model.
    #[arg(long)]
    pub(super) input: Vec<PathBuf>,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct NormalMeanSbcArgs {
    /// Mean of the prior distribution.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) prior_mean: f64,
    /// Scale of the prior distribution.
    #[arg(long)]
    pub(super) prior_sd: f64,
    /// Known standard deviation of Gaussian observation noise.
    #[arg(long)]
    pub(super) known_sigma: f64,
    /// Number of observations simulated for each calibration replicate.
    #[arg(long)]
    pub(super) observations_per_replicate: u32,
    /// Number of simulated datasets used for posterior calibration.
    #[arg(long)]
    pub(super) replicates: u32,
    /// Posterior draws used to propagate fitted uncertainty.
    #[arg(long)]
    pub(super) posterior_draws: u32,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct NormalMeanPriorSensitivityArgs {
    /// CSV with a single observation column, one row per observation.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Prespecified prior configurations to compare.
    #[arg(long)]
    pub(super) priors: PathBuf,
    /// Baseline prior configuration for the sensitivity comparison.
    #[arg(long)]
    pub(super) base_prior: String,
    /// Known standard deviation of Gaussian observation noise.
    #[arg(long)]
    pub(super) known_sigma: f64,
    /// Parameter threshold defining the posterior decision.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) decision_threshold: f64,
    /// Posterior probability required for the declared decision.
    #[arg(long)]
    pub(super) decision_probability_threshold: f64,
    /// Smallest posterior-mean change considered scientifically material.
    #[arg(long)]
    pub(super) material_mean_shift: f64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct InhomogeneousPoissonLikelihoodArgs {
    /// Observed point events with coordinates and model covariates.
    #[arg(long)]
    pub(super) events: PathBuf,
    /// Quadrature nodes, weights and covariates for spatial integration.
    #[arg(long)]
    pub(super) quadrature: PathBuf,
    /// Lower observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmin_um: f64,
    /// Lower observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymin_um: f64,
    /// Upper observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmax_um: f64,
    /// Upper observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymax_um: f64,
    /// Number of quadrature or simulation grid cells along x.
    #[arg(long)]
    pub(super) grid_x: u32,
    /// Number of quadrature or simulation grid cells along y.
    #[arg(long)]
    pub(super) grid_y: u32,
    /// Intercept on the model's linear predictor scale.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept: f64,
    /// Covariate coefficient on the model's linear predictor scale.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) coefficient: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct FitInhomogeneousPoissonArgs {
    /// Observed point events with coordinates and model covariates.
    #[arg(long)]
    pub(super) events: PathBuf,
    /// Quadrature nodes, weights and covariates for spatial integration.
    #[arg(long)]
    pub(super) quadrature: PathBuf,
    /// Lower observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmin_um: f64,
    /// Lower observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymin_um: f64,
    /// Upper observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmax_um: f64,
    /// Upper observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymax_um: f64,
    /// Number of quadrature or simulation grid cells along x.
    #[arg(long)]
    pub(super) grid_x: u32,
    /// Number of quadrature or simulation grid cells along y.
    #[arg(long)]
    pub(super) grid_y: u32,
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
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BermanTurnerRefinementArgs {
    /// Observed point events with coordinates and model covariates.
    #[arg(long)]
    pub(super) events: PathBuf,
    /// Coarse version of: quadrature nodes, weights and covariates for spatial integration.
    #[arg(long)]
    pub(super) coarse_quadrature: PathBuf,
    /// Fine version of: quadrature nodes, weights and covariates for spatial integration.
    #[arg(long)]
    pub(super) fine_quadrature: PathBuf,
    /// Lower observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmin_um: f64,
    /// Lower observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymin_um: f64,
    /// Upper observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmax_um: f64,
    /// Upper observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymax_um: f64,
    /// Coarse version of: number of quadrature or simulation grid cells along x.
    #[arg(long)]
    pub(super) coarse_grid_x: u32,
    /// Coarse version of: number of quadrature or simulation grid cells along y.
    #[arg(long)]
    pub(super) coarse_grid_y: u32,
    /// Fine version of: number of quadrature or simulation grid cells along x.
    #[arg(long)]
    pub(super) fine_grid_x: u32,
    /// Fine version of: number of quadrature or simulation grid cells along y.
    #[arg(long)]
    pub(super) fine_grid_y: u32,
    /// Intercept on the model's linear predictor scale.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) intercept: f64,
    /// Covariate coefficient on the model's linear predictor scale.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) coefficient: f64,
    /// Required tolerance for convergence across quadrature refinements.
    #[arg(long)]
    pub(super) convergence_tolerance: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BuildGriddedLgcpArgs {
    /// Observed point events with coordinates and model covariates.
    #[arg(long)]
    pub(super) events: PathBuf,
    /// Grid or quadrature table with cell covariates and offsets.
    #[arg(long)]
    pub(super) grid: PathBuf,
    /// Lower observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmin_um: f64,
    /// Lower observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymin_um: f64,
    /// Upper observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmax_um: f64,
    /// Upper observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymax_um: f64,
    /// Number of quadrature or simulation grid cells along x.
    #[arg(long)]
    pub(super) grid_x: u32,
    /// Number of quadrature or simulation grid cells along y.
    #[arg(long)]
    pub(super) grid_y: u32,
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
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct FitGriddedLgcpArgs {
    #[command(flatten)]
    pub(super) model: GriddedLgcpModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct GriddedLgcpModelArgs {
    /// Observed point events with coordinates and model covariates.
    #[arg(long)]
    pub(crate) events: PathBuf,
    /// Grid or quadrature table with cell covariates and offsets.
    #[arg(long)]
    pub(crate) grid: PathBuf,
    /// Lower observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) xmin_um: f64,
    /// Lower observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) ymin_um: f64,
    /// Upper observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) xmax_um: f64,
    /// Upper observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) ymax_um: f64,
    /// Number of quadrature or simulation grid cells along x.
    #[arg(long)]
    pub(crate) grid_x: u32,
    /// Number of quadrature or simulation grid cells along y.
    #[arg(long)]
    pub(crate) grid_y: u32,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    pub(crate) intercept_prior_sd: f64,
    /// Prior mean for coefficient.
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) coefficient_prior_mean: f64,
    /// Prior scale for coefficient.
    #[arg(long)]
    pub(crate) coefficient_prior_sd: f64,
    /// Amplitude of the latent spatial field covariance.
    #[arg(long)]
    pub(crate) field_amplitude: f64,
    /// Spatial covariance length scale in micrometers.
    #[arg(long)]
    pub(crate) field_length_scale_um: f64,
    /// Diagonal stabilization added to covariance matrices.
    #[arg(long)]
    pub(crate) jitter: f64,
    #[command(flatten)]
    pub(crate) nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
}

#[derive(Debug, Args)]
pub(crate) struct GriddedLgcpAgreementArgs {
    #[command(flatten)]
    pub(super) model: GriddedLgcpModelArgs,
    /// Largest standardized difference accepted between backend estimates.
    #[arg(long)]
    pub(super) maximum_standardized_difference: f64,
    /// Minimum absolute comparison tolerance for parameter.
    #[arg(long)]
    pub(super) minimum_parameter_tolerance: f64,
    /// Minimum absolute comparison tolerance for field.
    #[arg(long)]
    pub(super) minimum_field_tolerance: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct GriddedLgcpSbcArgs {
    #[command(flatten)]
    pub(super) model: GriddedLgcpModelArgs,
    /// Number of simulated datasets used for posterior calibration.
    #[arg(long)]
    pub(super) replicates: u32,
    /// Smallest admitted p-value for simulation-based rank uniformity.
    #[arg(long)]
    pub(super) minimum_rank_uniformity_p_value: f64,
    /// Smallest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    pub(super) minimum_coverage_90: f64,
    /// Largest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    pub(super) maximum_coverage_90: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct GriddedLgcpSensitivityArgs {
    #[command(flatten)]
    pub(super) model: GriddedLgcpModelArgs,
    /// Multiplier defining the lower prior-scale sensitivity scenario.
    #[arg(long)]
    pub(super) lower_scale_multiplier: f64,
    /// Multiplier defining the upper prior-scale sensitivity scenario.
    #[arg(long)]
    pub(super) upper_scale_multiplier: f64,
    /// Smallest standardized posterior change considered material.
    #[arg(long)]
    pub(super) material_standardized_shift: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct GriddedLgcpSpatialPpcArgs {
    #[command(flatten)]
    pub(super) model: GriddedLgcpModelArgs,
    /// Number of simulated or resampled datasets used by the analysis.
    #[arg(long)]
    pub(super) replicates: u32,
    /// Random seed for posterior predictive simulation.
    #[arg(long)]
    pub(super) prediction_seed: u64,
    /// Upper bound on total points.
    #[arg(long)]
    pub(super) maximum_total_points: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct SimulateThomasProcessArgs {
    /// Lower observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmin_um: f64,
    /// Lower observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymin_um: f64,
    /// Upper observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmax_um: f64,
    /// Upper observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymax_um: f64,
    /// Expected parent-point intensity per square micrometer.
    #[arg(long)]
    pub(super) kappa_parent_per_um2: f64,
    /// Expected offspring count per parent point.
    #[arg(long)]
    pub(super) mu_offspring: f64,
    /// Gaussian offspring displacement standard deviation in micrometers.
    #[arg(long)]
    pub(super) sigma_um: f64,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Upper bound on parents.
    #[arg(long)]
    pub(super) maximum_parents: u64,
    /// Upper bound on offspring.
    #[arg(long)]
    pub(super) maximum_offspring: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct SimulateMaternClusterProcessArgs {
    /// Lower observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmin_um: f64,
    /// Lower observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymin_um: f64,
    /// Upper observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmax_um: f64,
    /// Upper observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymax_um: f64,
    /// Expected parent-point intensity per square micrometer.
    #[arg(long)]
    pub(super) kappa_parent_per_um2: f64,
    /// Expected offspring count per parent point.
    #[arg(long)]
    pub(super) mu_offspring: f64,
    /// Spatial radius in micrometers.
    #[arg(long)]
    pub(super) radius_um: f64,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Upper bound on parents.
    #[arg(long)]
    pub(super) maximum_parents: u64,
    /// Upper bound on offspring.
    #[arg(long)]
    pub(super) maximum_offspring: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct FitThomasMinimumContrastArgs {
    /// CSV K curve with exactly radius_um, observed_k_um2 and weight columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Observed event intensity per square micrometer.
    #[arg(long)]
    pub(super) observed_intensity_per_um2: f64,
    /// Lower search bound for parent intensity, per square micrometer.
    #[arg(long)]
    pub(super) kappa_min_per_um2: f64,
    /// Upper search bound for parent intensity, per square micrometer.
    #[arg(long)]
    pub(super) kappa_max_per_um2: f64,
    /// Lower search bound for Gaussian cluster scale, in micrometers.
    #[arg(long)]
    pub(super) sigma_min_um: f64,
    /// Upper search bound for Gaussian cluster scale, in micrometers.
    #[arg(long)]
    pub(super) sigma_max_um: f64,
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

#[derive(Debug, Args)]
pub(crate) struct StraussStatisticsArgs {
    /// CSV of points with exactly point_id, x_um and y_um columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Point-interaction radius in micrometers.
    #[arg(long)]
    pub(super) interaction_radius_um: f64,
    /// Proposed event x coordinate in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) proposal_x_um: f64,
    /// Proposed event y coordinate in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) proposal_y_um: f64,
    /// Baseline Strauss intensity per square micrometer.
    #[arg(long)]
    pub(super) beta_per_um2: f64,
    /// Strauss pair-interaction parameter.
    #[arg(long)]
    pub(super) gamma: f64,
    /// Upper bound on pair visits.
    #[arg(long)]
    pub(super) maximum_pair_visits: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct SimulateStraussBirthDeathArgs {
    /// Lower observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmin_um: f64,
    /// Lower observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymin_um: f64,
    /// Upper observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmax_um: f64,
    /// Upper observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymax_um: f64,
    /// Baseline Strauss intensity per square micrometer.
    #[arg(long)]
    pub(super) beta_per_um2: f64,
    /// Strauss pair-interaction parameter.
    #[arg(long)]
    pub(super) gamma: f64,
    /// Point-interaction radius in micrometers.
    #[arg(long)]
    pub(super) interaction_radius_um: f64,
    /// Number of optimization or sampling iterations.
    #[arg(long)]
    pub(super) iterations: u32,
    /// Initial sampler iterations to discard.
    #[arg(long)]
    pub(super) burn_in: u32,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Upper bound on points.
    #[arg(long)]
    pub(super) maximum_points: u32,
    /// Upper bound on neighbor visits.
    #[arg(long)]
    pub(super) maximum_neighbor_visits: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct FitStraussPseudolikelihoodArgs {
    /// CSV of points with exactly point_id, x_um and y_um columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Lower observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmin_um: f64,
    /// Lower observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymin_um: f64,
    /// Upper observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmax_um: f64,
    /// Upper observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymax_um: f64,
    /// Point-interaction radius in micrometers.
    #[arg(long)]
    pub(super) interaction_radius_um: f64,
    /// Coarse version of: number of quadrature or simulation grid cells along x.
    #[arg(long)]
    pub(super) coarse_grid_x: u32,
    /// Coarse version of: number of quadrature or simulation grid cells along y.
    #[arg(long)]
    pub(super) coarse_grid_y: u32,
    /// Fine version of: number of quadrature or simulation grid cells along x.
    #[arg(long)]
    pub(super) fine_grid_x: u32,
    /// Fine version of: number of quadrature or simulation grid cells along y.
    #[arg(long)]
    pub(super) fine_grid_y: u32,
    /// Lower search bound for Strauss baseline intensity, per square micrometer.
    #[arg(long)]
    pub(super) beta_min_per_um2: f64,
    /// Upper search bound for Strauss baseline intensity, per square micrometer.
    #[arg(long)]
    pub(super) beta_max_per_um2: f64,
    /// Lower search bound for the Strauss interaction parameter.
    #[arg(long)]
    pub(super) gamma_min: f64,
    /// Upper search bound for the Strauss interaction parameter.
    #[arg(long)]
    pub(super) gamma_max: f64,
    /// Upper bound on iterations.
    #[arg(long)]
    pub(super) maximum_iterations: u32,
    /// Upper bound on neighbor visits.
    #[arg(long)]
    pub(super) maximum_neighbor_visits: u64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct GeyerSaturationStatisticArgs {
    /// CSV of points with exactly point_id, x_um and y_um columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Point-interaction radius in micrometers.
    #[arg(long)]
    pub(super) interaction_radius_um: f64,
    /// Maximum neighbor contribution per point in the Geyer statistic.
    #[arg(long)]
    pub(super) saturation: u32,
    /// Upper bound on pair visits.
    #[arg(long)]
    pub(super) maximum_pair_visits: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct MultitypePapangelouArgs {
    /// Observed spatial points and their declared measurements.
    #[arg(long)]
    pub(super) points: PathBuf,
    /// Baseline intensity parameters for each declared point type.
    #[arg(long)]
    pub(super) baselines: PathBuf,
    /// Declared interactions between point types.
    #[arg(long)]
    pub(super) interactions: PathBuf,
    /// Point type of the proposed event.
    #[arg(long)]
    pub(super) proposal_type: String,
    /// Proposed event x coordinate in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) proposal_x_um: f64,
    /// Proposed event y coordinate in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) proposal_y_um: f64,
    /// Upper bound on visits.
    #[arg(long)]
    pub(super) maximum_visits: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BuildJointLocationMarkModelArgs {
    /// Observed spatial points and their declared measurements.
    #[arg(long)]
    pub(super) points: PathBuf,
    /// Grid or quadrature table with cell covariates and offsets.
    #[arg(long)]
    pub(super) grid: PathBuf,
    /// Lower observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmin_um: f64,
    /// Lower observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymin_um: f64,
    /// Upper observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmax_um: f64,
    /// Upper observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymax_um: f64,
    /// Number of quadrature or simulation grid cells along x.
    #[arg(long)]
    pub(super) grid_x: u32,
    /// Number of quadrature or simulation grid cells along y.
    #[arg(long)]
    pub(super) grid_y: u32,
    /// Mark level used as the regression reference.
    #[arg(long)]
    pub(super) reference_mark: String,
    /// Prior scale for location.
    #[arg(long)]
    pub(super) location_prior_sd: f64,
    /// Prior scale for mark coefficient.
    #[arg(long)]
    pub(super) mark_coefficient_prior_sd: f64,
    /// Prior scale for mark field.
    #[arg(long)]
    pub(super) mark_field_prior_sd: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BuildReplicatedHierarchicalLgcpArgs {
    /// CSV: pattern_id, patient_id, window/grid/covariate_sha256, event_count and cell_count.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Declared sharing policy for latent fields across replicates.
    #[arg(long)]
    pub(super) field_policy: String,
    /// Prior standard deviation of the global mean.
    #[arg(long)]
    pub(super) global_prior_sd: f64,
    /// Prior scale for patient intercept.
    #[arg(long)]
    pub(super) patient_intercept_prior_sd: f64,
    /// Amplitude of the population-level spatial covariance field.
    #[arg(long)]
    pub(super) population_field_amplitude: f64,
    /// Population-level spatial covariance length scale in micrometers.
    #[arg(long)]
    pub(super) population_field_length_scale_um: f64,
    /// Amplitude of replicate-specific spatial field variation.
    #[arg(long)]
    pub(super) replicate_field_amplitude: f64,
    /// Diagonal stabilization added to covariance matrices.
    #[arg(long)]
    pub(super) jitter: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct CrossModalCovarianceByDistanceArgs {
    /// Input embedding table for the first modality.
    #[arg(long)]
    pub(super) a_input: PathBuf,
    /// Input embedding table for the second modality.
    #[arg(long)]
    pub(super) b_input: PathBuf,
    /// Declared cross-modal observation pairs and their distances.
    #[arg(long)]
    pub(super) pairs: PathBuf,
    /// Distance-bin edges used to group spatial observation pairs.
    #[arg(long)]
    pub(super) bins: PathBuf,
    /// Policy for centering observations before spatial estimation.
    #[arg(long)]
    pub(super) mean_policy: String,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: u32,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Upper bound on pairs.
    #[arg(long)]
    pub(super) maximum_pairs: u64,
    /// Upper bound on component pair visits.
    #[arg(long)]
    pub(super) maximum_component_pair_visits: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct KernelMarkCorrelationArgs {
    /// CSV with object_id, biological_unit, split, x_um, y_um and two or more embedding_* columns.
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
    /// Upper bound on pair visits.
    #[arg(long)]
    pub(super) maximum_pair_visits: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct EmbeddingSpatialDependenceEnvelopeArgs {
    /// CSV with object_id, permutation_stratum, x_um, y_um and two or more embedding columns.
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
    /// Upper bound on pair visits.
    #[arg(long)]
    pub(super) maximum_pair_visits: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct TestCellPatchComplementarityArgs {
    /// Patient CSV with patient_id, outer_fold, inner_fold and target columns followed by
    /// grouped feature columns (`cell_`, `patch_` and the other declared prefixes).
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
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct MultiscaleEmbeddingKernelArgs {
    /// CSV of summaries with sample_id, scale_um and embedding_* columns.
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
    /// Upper bound on component scale visits.
    #[arg(long)]
    pub(super) maximum_component_scale_visits: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct RetrieveAnalogousRegionsArgs {
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

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub(super) enum CalibrationMethod {
    PlattLogistic,
}

#[derive(Debug, Args)]
pub(crate) struct CalibratePredictionsArgs {
    /// CSV with exactly patient_id, split, score and label columns, one row per patient.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Probability-calibration method.
    #[arg(long, value_enum)]
    pub(super) method: CalibrationMethod,
    /// Number of probability bins used to assess prediction calibration.
    #[arg(long)]
    pub(super) bins: u32,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct GroupedConformalArgs {
    /// CSV with patient_id, split, site, subgroup and label, then two or more feature columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Target miscoverage probability for the conformal prediction sets.
    #[arg(long)]
    pub(super) alpha: f64,
    /// L2 penalty on fitted predictive coefficients.
    #[arg(long)]
    pub(super) l2_penalty: f64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct LateFusionArgs {
    /// CSV with patient_id, split, base_prediction_source, label, then per-modality probabilities.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// L2 penalty on fitted predictive coefficients.
    #[arg(long)]
    pub(super) l2_penalty: f64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct PredictiveStackingArgs {
    /// CSV with patient_id, held_out_unit, then one log predictive density column per model.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct MixtureOfExpertsFusionArgs {
    /// CSV with patient_id, split, expert_prediction_source, label, context_* and expert columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// L2 penalty on fitted predictive coefficients.
    #[arg(long)]
    pub(super) l2_penalty: f64,
    /// Entropy penalty on mixture-of-experts gating weights.
    #[arg(long)]
    pub(super) entropy_regularization: f64,
    /// Validation-score quantile used to set the support threshold.
    #[arg(long)]
    pub(super) ood_validation_quantile: f64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct FusedGromovWassersteinArgs {
    /// JSON with source and target supports and row-major source and target structure matrices.
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

#[derive(Debug, Args)]
pub(crate) struct PartialFusedGromovWassersteinArgs {
    /// JSON with source and target supports and row-major source and target structure matrices.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Total mass to match in the partial transport problem.
    #[arg(long)]
    pub(super) transported_mass: f64,
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

#[derive(Debug, Args)]
pub(crate) struct UnbalancedFusedGromovWassersteinArgs {
    /// JSON with source and target supports and row-major source and target structure matrices.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Weight of the linear feature-cost term relative to the structural term.
    #[arg(long)]
    pub(super) linear_weight: f64,
    /// Strength of the KL penalty holding each marginal to its supplied masses.
    #[arg(long)]
    pub(super) marginal_relaxation: f64,
    /// Entropic regularization; 0 uses the unregularized majorization-minimization solver.
    #[arg(long)]
    pub(super) epsilon: f64,
    /// Scale applied to feature dissimilarities in the transport objective.
    #[arg(long)]
    pub(super) feature_scale: f64,
    /// Scale applied to structural dissimilarities in the transport objective.
    #[arg(long)]
    pub(super) structure_scale: f64,
    /// Stop when the L1 change of the coupling between outer iterations falls below this.
    #[arg(long)]
    pub(super) tolerance: f64,
    /// Upper bound on outer block-coordinate iterations.
    #[arg(long)]
    pub(super) maximum_iterations: u32,
    /// Tolerance of each inner unbalanced transport solve.
    #[arg(long)]
    pub(super) inner_tolerance: f64,
    /// Upper bound on iterations of each inner unbalanced transport solve.
    #[arg(long)]
    pub(super) maximum_inner_iterations: u32,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BuildJointContinuousMarkModelArgs {
    /// Observed spatial points and their declared measurements.
    #[arg(long)]
    pub(super) points: PathBuf,
    /// Grid or quadrature table with cell covariates and offsets.
    #[arg(long)]
    pub(super) grid: PathBuf,
    /// Lower observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmin_um: f64,
    /// Lower observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymin_um: f64,
    /// Upper observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmax_um: f64,
    /// Upper observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymax_um: f64,
    /// Number of quadrature or simulation grid cells along x.
    #[arg(long)]
    pub(super) grid_x: u32,
    /// Number of quadrature or simulation grid cells along y.
    #[arg(long)]
    pub(super) grid_y: u32,
    /// Known measurement-noise standard deviation of the continuous mark.
    #[arg(long)]
    pub(super) known_mark_noise_sd: f64,
    /// Amplitude of the spatial field shared by location and mark models.
    #[arg(long)]
    pub(super) shared_field_amplitude: f64,
    /// Length scale of the shared spatial field, in micrometers.
    #[arg(long)]
    pub(super) shared_field_length_scale_um: f64,
    /// Diagonal stabilization added to covariance matrices.
    #[arg(long)]
    pub(super) jitter: f64,
    /// Prior scale for mark loading.
    #[arg(long)]
    pub(super) mark_loading_prior_sd: f64,
    /// Prior scale for private field.
    #[arg(long)]
    pub(super) private_field_prior_sd: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct VectorSemivariogramArgs {
    /// CSV with object_id,x_um,y_um and feature columns; coordinates only when --vectors is set.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Optional row-aligned, finite float32 NPY matrix from any embedding model.
    #[arg(long)]
    pub(super) vectors: Option<PathBuf>,
    /// Distance-bin edges used to group spatial observation pairs.
    #[arg(long)]
    pub(super) bins: PathBuf,
    /// Optional table of pair weights for the semivariogram.
    #[arg(long)]
    pub(super) weights: Option<PathBuf>,
    /// Upper bound on pair visits.
    #[arg(long)]
    pub(super) maximum_pair_visits: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct ProjectedEmbeddingVariogramsArgs {
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

#[derive(Debug, Args)]
pub(crate) struct EmbeddingCrossCovarianceByDistanceArgs {
    /// CSV with object_id, x_um and y_um columns followed by two or more embedding columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Distance-bin edges used to group spatial observation pairs.
    #[arg(long)]
    pub(super) bins: PathBuf,
    /// Upper bound on pair visits.
    #[arg(long)]
    pub(super) maximum_pair_visits: u64,
    /// Upper bound on matrix elements.
    #[arg(long)]
    pub(super) maximum_matrix_elements: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct GraphDirichletEnergyArgs {
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
    /// Upper bound on component edge visits.
    #[arg(long)]
    pub(super) maximum_component_edge_visits: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct GraphSmoothnessPermutationTestArgs {
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
    /// Upper bound on component edge visits.
    #[arg(long)]
    pub(super) maximum_component_edge_visits: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct LocalEmbeddingRoughnessArgs {
    /// Graph-node observations and their embedding or signal values.
    #[arg(long)]
    pub(super) nodes: PathBuf,
    /// Table of spatial graph edges and their declared weights.
    #[arg(long)]
    pub(super) edges: PathBuf,
    /// Stabilizing constant used when normalizing local embedding roughness.
    #[arg(long)]
    pub(super) epsilon: f64,
    /// Upper bound on component edge visits.
    #[arg(long)]
    pub(super) maximum_component_edge_visits: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct ApplyAbstentionArgs {
    /// CSV with exactly prediction_id, prediction, uncertainty and ood_score columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Largest prediction uncertainty accepted before abstaining.
    #[arg(long)]
    pub(super) maximum_uncertainty: f64,
    /// Largest out-of-distribution score accepted before abstaining.
    #[arg(long)]
    pub(super) maximum_ood_score: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct OodScoreArgs {
    /// CSV with unit_id, split and domain, then two or more representation columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Shrinkage used to regularize the reference covariance estimate.
    #[arg(long)]
    pub(super) shrinkage: f64,
    /// Validation-score quantile defining the acceptance threshold.
    #[arg(long)]
    pub(super) validation_quantile: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct SinkhornOtArgs {
    /// Source locations or masses for the transport problem.
    #[arg(long)]
    pub(super) source: PathBuf,
    /// Target locations or masses for the transport problem.
    #[arg(long)]
    pub(super) target: PathBuf,
    /// Source-to-target transport cost matrix.
    #[arg(long)]
    pub(super) cost: PathBuf,
    /// Entropy regularization strength of the transport objective.
    #[arg(long)]
    pub(super) epsilon: f64,
    /// Numerical tolerance used by the algorithm.
    #[arg(long)]
    pub(super) tolerance: f64,
    /// Upper bound on iterations.
    #[arg(long)]
    pub(super) maximum_iterations: u32,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct EntropicSoftAssignmentArgs {
    /// Source locations or masses for the transport problem.
    #[arg(long)]
    pub(super) source: PathBuf,
    /// Target locations or masses for the transport problem.
    #[arg(long)]
    pub(super) target: PathBuf,
    /// Source-to-target transport cost matrix.
    #[arg(long)]
    pub(super) cost: PathBuf,
    /// Entropy regularization strength of the transport objective.
    #[arg(long)]
    pub(super) epsilon: f64,
    /// Cost assigned to an unmatched observation in soft assignment.
    #[arg(long)]
    pub(super) dustbin_cost: f64,
    /// Numerical tolerance used by the algorithm.
    #[arg(long)]
    pub(super) tolerance: f64,
    /// Upper bound on iterations.
    #[arg(long)]
    pub(super) maximum_iterations: u32,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct UnbalancedSinkhornArgs {
    /// Source locations or masses for the transport problem.
    #[arg(long)]
    pub(super) source: PathBuf,
    /// Target locations or masses for the transport problem.
    #[arg(long)]
    pub(super) target: PathBuf,
    /// Source-to-target transport cost matrix.
    #[arg(long)]
    pub(super) cost: PathBuf,
    /// Entropy regularization strength of the transport objective.
    #[arg(long)]
    pub(super) epsilon: f64,
    /// Penalty for deviation from the source transport marginal.
    #[arg(long)]
    pub(super) tau_source: f64,
    /// Penalty for deviation from the target transport marginal.
    #[arg(long)]
    pub(super) tau_target: f64,
    /// Numerical tolerance used by the algorithm.
    #[arg(long)]
    pub(super) tolerance: f64,
    /// Upper bound on iterations.
    #[arg(long)]
    pub(super) maximum_iterations: u32,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct PartialOtArgs {
    /// Source locations or masses for the transport problem.
    #[arg(long)]
    pub(super) source: PathBuf,
    /// Target locations or masses for the transport problem.
    #[arg(long)]
    pub(super) target: PathBuf,
    /// Source-to-target transport cost matrix.
    #[arg(long)]
    pub(super) cost: PathBuf,
    /// Total mass to match in the partial transport problem.
    #[arg(long)]
    pub(super) transported_mass: f64,
    /// Entropy regularization strength of the transport objective.
    #[arg(long)]
    pub(super) epsilon: f64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(super) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BetaBinomialGroupGenderRegressionSensitivityArgs {
    #[command(flatten)]
    pub(super) model: BetaBinomialGroupGenderRegressionModelArgs,
    /// Multiplier defining the lower prior-scale sensitivity scenario.
    #[arg(long)]
    pub(super) lower_scale_multiplier: f64,
    /// Multiplier defining the upper prior-scale sensitivity scenario.
    #[arg(long)]
    pub(super) upper_scale_multiplier: f64,
    /// Smallest standardized posterior change considered material.
    #[arg(long)]
    pub(super) material_standardized_shift: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct RejectionAbcGrowthFrontArgs {
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
    /// Largest standardized simulation discrepancy accepted by rejection ABC.
    #[arg(long)]
    pub(super) epsilon: f64,
    /// Number of accepted ABC parameter draws to retain.
    #[arg(long)]
    pub(super) accepted_draws: u32,
    /// Upper bound on proposals.
    #[arg(long)]
    pub(super) maximum_proposals: u32,
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

#[derive(Debug, Args)]
pub(crate) struct SyntheticLikelihoodGrowthFrontArgs {
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
    /// Observed maximum density at the final simulation time.
    #[arg(long)]
    pub(super) observed_maximum_density: f64,
    /// Simulation-summary noise standard deviation for total mass.
    #[arg(long)]
    pub(super) mass_noise_sd: f64,
    /// Simulation-summary noise standard deviation for maximum density.
    #[arg(long)]
    pub(super) maximum_density_noise_sd: f64,
    /// Lower bound of the uniform growth-rate prior.
    #[arg(long)]
    pub(super) growth_rate_prior_min: f64,
    /// Upper bound of the uniform growth-rate prior.
    #[arg(long)]
    pub(super) growth_rate_prior_max: f64,
    /// Simulation replicates used for each synthetic-likelihood evaluation.
    #[arg(long)]
    pub(super) replicates: u32,
    /// Shrinkage applied to the simulated-summary covariance estimate.
    #[arg(long)]
    pub(super) covariance_shrinkage: f64,
    /// Number of optimization or sampling iterations.
    #[arg(long)]
    pub(super) iterations: u32,
    /// Initial sampler iterations to discard.
    #[arg(long)]
    pub(super) burn_in: u32,
    /// Standard deviation of the sampler's proposal distribution.
    #[arg(long)]
    pub(super) proposal_sd: f64,
    /// Upper bound on cell steps per simulation.
    #[arg(long)]
    pub(super) maximum_cell_steps_per_simulation: u64,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct GrowthFrontRejectionAbcSbcArgs {
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
    /// Scale used to standardize the simulated-versus-observed mass discrepancy.
    #[arg(long)]
    pub(super) mass_scale: f64,
    /// Lower bound of the uniform growth-rate prior.
    #[arg(long)]
    pub(super) growth_rate_prior_min: f64,
    /// Upper bound of the uniform growth-rate prior.
    #[arg(long)]
    pub(super) growth_rate_prior_max: f64,
    /// Largest standardized simulation discrepancy accepted by rejection ABC.
    #[arg(long)]
    pub(super) epsilon: f64,
    /// Posterior draws used to propagate fitted uncertainty.
    #[arg(long)]
    pub(super) posterior_draws: u32,
    /// Upper bound on proposals per replicate.
    #[arg(long)]
    pub(super) maximum_proposals_per_replicate: u32,
    /// Number of simulated datasets used for posterior calibration.
    #[arg(long)]
    pub(super) replicates: u32,
    /// Nominal posterior interval probability for coverage checks.
    #[arg(long)]
    pub(super) coverage_probability: f64,
    /// Upper bound on cell steps per simulation.
    #[arg(long)]
    pub(super) maximum_cell_steps_per_simulation: u64,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct GrowthFrontPosteriorPredictiveLabArgs {
    /// JSON with initial {position_um, density} points and posterior_growth_rate_draws.
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
    /// Observed maximum density at the final simulation time.
    #[arg(long)]
    pub(super) observed_maximum_density: f64,
    /// Number of simulated or resampled datasets used by the analysis.
    #[arg(long)]
    pub(super) replicates: u32,
    /// Probability contained in the reported predictive interval.
    #[arg(long)]
    pub(super) interval_probability: f64,
    /// Tail probability used to flag posterior predictive discrepancies.
    #[arg(long)]
    pub(super) discrepancy_alpha: f64,
    /// Upper bound on cell steps per replicate.
    #[arg(long)]
    pub(super) maximum_cell_steps_per_replicate: u64,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct SimulateGriddedLgcpPosteriorPredictiveArgs {
    #[command(flatten)]
    pub(super) model: GriddedLgcpModelArgs,
    /// Number of simulated or resampled datasets used by the analysis.
    #[arg(long)]
    pub(super) replicates: u32,
    /// Random seed for posterior predictive simulation.
    #[arg(long)]
    pub(super) prediction_seed: u64,
    /// Upper bound on total points.
    #[arg(long)]
    pub(super) maximum_total_points: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct BuildJointLocationEmbeddingFactorModelArgs {
    /// Observed spatial points and their declared measurements.
    #[arg(long)]
    pub(super) points: PathBuf,
    /// Grid or quadrature table with cell covariates and offsets.
    #[arg(long)]
    pub(super) grid: PathBuf,
    /// Lower observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmin_um: f64,
    /// Lower observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymin_um: f64,
    /// Upper observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmax_um: f64,
    /// Upper observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymax_um: f64,
    /// Number of quadrature or simulation grid cells along x.
    #[arg(long)]
    pub(super) grid_x: u32,
    /// Number of quadrature or simulation grid cells along y.
    #[arg(long)]
    pub(super) grid_y: u32,
    /// Number of shared latent factors.
    #[arg(long)]
    pub(super) factors: u32,
    /// Amplitude of the latent spatial field covariance.
    #[arg(long)]
    pub(super) field_amplitude: f64,
    /// Spatial covariance length scale in micrometers.
    #[arg(long)]
    pub(super) field_length_scale_um: f64,
    /// Diagonal stabilization added to covariance matrices.
    #[arg(long)]
    pub(super) jitter: f64,
    /// Prior scale for loading.
    #[arg(long)]
    pub(super) loading_prior_sd: f64,
    /// Prior scale for noise.
    #[arg(long)]
    pub(super) noise_prior_sd: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct PointProcessPosteriorPredictiveDiagnosticsArgs {
    /// Observed summaries to compare with model simulations.
    #[arg(long)]
    pub(super) observed: PathBuf,
    /// Treat the supplied patterns as declared replicated observations.
    #[arg(long)]
    pub(super) replicated: PathBuf,
    /// CSV file with a single `radius_um` column listing the prespecified radii.
    #[arg(long)]
    pub(super) radii: PathBuf,
    /// Lower observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmin_um: f64,
    /// Lower observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymin_um: f64,
    /// Upper observation-window bound along x, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) xmax_um: f64,
    /// Upper observation-window bound along y, in micrometers.
    #[arg(long, allow_hyphen_values = true)]
    pub(super) ymax_um: f64,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Upper bound on pair visits.
    #[arg(long)]
    pub(super) maximum_pair_visits: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct NormalMeanArgs {
    /// CSV with a single observation column, one row per observation.
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

#[derive(Debug, Subcommand)]
pub(crate) enum ArbitraryWindowCommands {
    /// Compare PyMC and NumPyro posterior estimates for a polygonal-window Poisson intensity model.
    ArbitraryWindowIppAgreement(Box<arbitrary_window_ipp_agreement::AgreementArguments>),
    /// Fit a Poisson intensity model in a polygonal observation window.
    FitArbitraryWindowIpp(Box<arbitrary_window_ipp_fit::FitArguments>),
    /// Assess quadrature-refinement sensitivity for a polygonal-window Poisson intensity model.
    ArbitraryWindowIppQuadratureSensitivity(
        Box<arbitrary_window_ipp_quadrature_sensitivity::Arguments>,
    ),
    /// Check posterior calibration with simulated data for a polygonal-window Poisson intensity model.
    ArbitraryWindowIppSbc(Box<arbitrary_window_ipp_sbc::SbcArguments>),
    /// Assess sensitivity to declared prior and model settings for a polygonal-window Poisson intensity model.
    ArbitraryWindowIppSensitivity(Box<arbitrary_window_ipp_sensitivity::SensitivityArguments>),
    /// Compare observed spatial summaries with posterior predictions for a polygonal-window Poisson intensity model.
    ArbitraryWindowIppSpatialPpc(Box<arbitrary_window_ipp_spatial_ppc::Arguments>),
    /// Compare PyMC and NumPyro posterior estimates for a polygonal-window log-Gaussian Cox process.
    ArbitraryWindowLgcpAgreement(Box<arbitrary_window_lgcp_agreement::Arguments>),
    /// Fit a log-Gaussian Cox process in a polygonal observation window.
    FitArbitraryWindowLgcp(Box<arbitrary_window_lgcp_fit::Arguments>),
    /// Assess quadrature-refinement sensitivity for a polygonal-window log-Gaussian Cox process.
    ArbitraryWindowLgcpQuadratureSensitivity(
        Box<arbitrary_window_lgcp_quadrature_sensitivity::Arguments>,
    ),
    /// Check posterior calibration with simulated data for a polygonal-window log-Gaussian Cox process.
    ArbitraryWindowLgcpSbc(Box<arbitrary_window_lgcp_sbc::Arguments>),
    /// Assess sensitivity to declared prior and model settings for a polygonal-window log-Gaussian Cox process.
    ArbitraryWindowLgcpSensitivity(Box<arbitrary_window_lgcp_sensitivity::Arguments>),
    /// Fit correlated multitype spatial fields across replicated windows.
    FitCorrelatedReplicatedArbitraryWindowMultitypeLgcp(
        Box<correlated_replicated_arbitrary_window_multitype_lgcp::Args>,
    ),
    /// Compare PyMC and NumPyro posterior estimates for replicated polygonal-window intensity fields.
    ReplicatedArbitraryWindowLgcpAgreement(
        Box<replicated_arbitrary_window_lgcp_agreement::Arguments>,
    ),
    /// Fit a log-Gaussian Cox process across replicated polygonal windows.
    FitReplicatedArbitraryWindowLgcp(Box<replicated_arbitrary_window_lgcp_fit::Arguments>),
    /// Infer shared kernel parameters across replicated spatial intensity fields.
    FitReplicatedArbitraryWindowLgcpInferredKernel(
        Box<replicated_arbitrary_window_lgcp_inferred_kernel::Arguments>,
    ),
    /// Compare PyMC and NumPyro posterior estimates for replicated intensity fields with inferred kernels.
    ReplicatedArbitraryWindowLgcpInferredKernelAgreement(
        Box<replicated_arbitrary_window_lgcp_inferred_kernel_agreement::Args>,
    ),
    /// Check posterior calibration with simulated data for replicated intensity fields with inferred kernels.
    ReplicatedArbitraryWindowLgcpInferredKernelSbc(
        Box<replicated_arbitrary_window_lgcp_inferred_kernel_sbc::Args>,
    ),
    /// Check posterior calibration with simulated data for replicated polygonal-window intensity fields.
    ReplicatedArbitraryWindowLgcpSbc(Box<replicated_arbitrary_window_lgcp_sbc::Arguments>),
    /// Assess sensitivity to declared prior and model settings for replicated polygonal-window intensity fields.
    ReplicatedArbitraryWindowLgcpSensitivity(
        Box<replicated_arbitrary_window_lgcp_sensitivity::Arguments>,
    ),
    /// Fit cell-type intensities across replicated windows.
    FitReplicatedArbitraryWindowMultitypeLgcp(
        Box<replicated_arbitrary_window_multitype_lgcp::Args>,
    ),
    /// Compare PyMC and NumPyro posterior estimates for replicated multitype intensity fields.
    ReplicatedArbitraryWindowMultitypeLgcpAgreement(
        Box<replicated_arbitrary_window_multitype_lgcp_agreement::Args>,
    ),
    /// Infer spatial kernels for replicated multitype intensity fields.
    FitReplicatedArbitraryWindowMultitypeLgcpInferredKernel(
        Box<replicated_arbitrary_window_multitype_lgcp_inferred_kernel::Args>,
    ),
    /// Compare PyMC and NumPyro posterior estimates for replicated multitype fields with inferred kernels.
    ReplicatedArbitraryWindowMultitypeLgcpInferredKernelAgreement(
        Box<replicated_arbitrary_window_multitype_lgcp_inferred_kernel_agreement::Args>,
    ),
    /// Check prior predictive behavior for replicated multitype fields with inferred kernels.
    ReplicatedArbitraryWindowMultitypeLgcpInferredKernelPriorCalibration(
        Box<replicated_arbitrary_window_multitype_lgcp_inferred_kernel_prior_calibration::Args>,
    ),
    /// Check posterior calibration with simulated data for replicated multitype fields with inferred kernels.
    ReplicatedArbitraryWindowMultitypeLgcpInferredKernelSbc(
        Box<replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc::Args>,
    ),
    /// Check prior predictive behavior for replicated multitype intensity fields.
    ReplicatedArbitraryWindowMultitypeLgcpPriorCalibration(
        Box<replicated_arbitrary_window_multitype_lgcp_prior_calibration::Args>,
    ),
    /// Assess sensitivity to declared prior and model settings for replicated multitype intensity fields.
    ReplicatedArbitraryWindowMultitypeLgcpSensitivity(
        Box<replicated_arbitrary_window_multitype_lgcp_sensitivity::Args>,
    ),
    /// Evaluate an inhomogeneous Poisson likelihood in a polygonal window.
    ArbitraryWindowIppLikelihood(Box<arbitrary_window_ipp::ArbitraryWindowIppLikelihoodArgs>),
}

#[derive(Debug, Subcommand)]
pub(crate) enum MarkModelsCommands {
    /// Fit cell-type probabilities conditional on observed locations.
    FitConditionalMultitypeMark(Box<conditional_multitype_mark::Args>),
    /// Compare PyMC and NumPyro posterior estimates for location-conditional cell-type probabilities.
    ConditionalMultitypeMarkAgreement(Box<conditional_multitype_mark_agreement::Args>),
    /// Check posterior calibration with simulated data for location-conditional cell-type probabilities.
    ConditionalMultitypeMarkSbc(Box<conditional_multitype_mark_sbc::Args>),
    /// Assess sensitivity to declared prior and model settings for location-conditional cell-type probabilities.
    ConditionalMultitypeMarkSensitivity(Box<conditional_multitype_mark_sensitivity::Args>),
    /// Fit a joint model of replicated locations and embedding features.
    FitJointReplicatedLocationEmbedding(Box<joint_replicated_location_embedding::Args>),
    /// Fit a joint model of replicated locations and scalar marks.
    FitJointReplicatedLocationMark(Box<joint_replicated_location_mark::Args>),
    /// Fit location-conditional cell-type probabilities across replicates.
    FitReplicatedConditionalMultitypeMark(Box<replicated_conditional_multitype_mark::Args>),
    /// Compare PyMC and NumPyro posterior estimates for replicated location-conditional cell-type probabilities.
    ReplicatedConditionalMultitypeMarkAgreement(
        Box<replicated_conditional_multitype_mark_agreement::Args>,
    ),
    /// Check posterior calibration with simulated data for replicated location-conditional cell-type probabilities.
    ReplicatedConditionalMultitypeMarkSbc(Box<replicated_conditional_multitype_mark_sbc::Args>),
    /// Assess sensitivity to declared prior and model settings for replicated location-conditional cell-type probabilities.
    ReplicatedConditionalMultitypeMarkSensitivity(
        Box<replicated_conditional_multitype_mark_sensitivity::Args>,
    ),
}

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct OrdinalGroupModelArgs {
    /// CSV with patient_id, group and outcome columns, one row per patient.
    #[arg(long)]
    pub(crate) input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    pub(crate) reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    pub(crate) comparison_group: String,
    /// Outcome labels in their prespecified ordinal order.
    #[arg(long, value_delimiter = ',', required = true)]
    pub(crate) ordered_levels: Vec<String>,
    /// Prior scale for ordered-outcome cutpoints.
    #[arg(long)]
    pub(crate) cutpoint_prior_sd: f64,
    /// Prior scale for group effect.
    #[arg(long)]
    pub(crate) group_effect_prior_sd: f64,
    #[command(flatten)]
    pub(crate) nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
}

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct OrdinalGroupSiteHierarchyModelArgs {
    /// CSV with patient_id, site_id, group and outcome columns, one row per patient.
    #[arg(long)]
    pub(crate) input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    pub(crate) reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    pub(crate) comparison_group: String,
    /// Outcome labels in their prespecified ordinal order.
    #[arg(long, value_delimiter = ',', required = true)]
    pub(crate) ordered_levels: Vec<String>,
    /// Prior scale for ordered-outcome cutpoints.
    #[arg(long)]
    pub(crate) cutpoint_prior_sd: f64,
    /// Prior scale for group effect.
    #[arg(long)]
    pub(crate) group_effect_prior_sd: f64,
    /// Prior scale for site intercept standard deviation.
    #[arg(long)]
    pub(crate) site_intercept_sd_prior_sd: f64,
    /// Prior scale for site group slope standard deviation.
    #[arg(long)]
    pub(crate) site_group_slope_sd_prior_sd: f64,
    #[command(flatten)]
    pub(crate) nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
}

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct NonproportionalOrdinalGroupSiteModelArgs {
    /// CSV with patient_id, site_id, group and outcome columns, one row per patient.
    #[arg(long)]
    pub(crate) input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    pub(crate) reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    pub(crate) comparison_group: String,
    /// Outcome labels in their prespecified ordinal order.
    #[arg(long, value_delimiter = ',', required = true)]
    pub(crate) ordered_levels: Vec<String>,
    /// Prior scale for ordered-outcome cutpoints.
    #[arg(long)]
    pub(crate) cutpoint_prior_sd: f64,
    /// Prior scale for site intercept standard deviation.
    #[arg(long)]
    pub(crate) site_intercept_sd_prior_sd: f64,
    #[command(flatten)]
    pub(crate) nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
}

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct OrdinalSiteHeldoutComparisonModelArgs {
    /// CSV with patient_id, site_id, group and outcome columns, one row per patient.
    #[arg(long)]
    pub(crate) input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    pub(crate) reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    pub(crate) comparison_group: String,
    /// Outcome labels in their prespecified ordinal order.
    #[arg(long, value_delimiter = ',', required = true)]
    pub(crate) ordered_levels: Vec<String>,
    /// Smoothing strength used in the held-out ordinal comparison.
    #[arg(long)]
    pub(crate) smoothing: f64,
    /// Stopping tolerance for the numerical optimizer.
    #[arg(long)]
    pub(crate) optimizer_tolerance: f64,
    /// Upper bound on optimizer evaluations.
    #[arg(long)]
    pub(crate) maximum_optimizer_evaluations: u64,
}

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct HurdleBetaBinomialGroupModelArgs {
    /// CSV with patient_id, group, successes and trials columns, one row per patient.
    #[arg(long)]
    pub(crate) input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    pub(crate) reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    pub(crate) comparison_group: String,
    /// Prior mean for presence intercept.
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) presence_intercept_prior_mean: f64,
    /// Prior scale for presence intercept.
    #[arg(long)]
    pub(crate) presence_intercept_prior_sd: f64,
    /// Prior scale for presence group effect.
    #[arg(long)]
    pub(crate) presence_group_effect_prior_sd: f64,
    /// Prior mean for abundance intercept.
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) abundance_intercept_prior_mean: f64,
    /// Prior scale for abundance intercept.
    #[arg(long)]
    pub(crate) abundance_intercept_prior_sd: f64,
    /// Prior scale for abundance group effect.
    #[arg(long)]
    pub(crate) abundance_group_effect_prior_sd: f64,
    /// Prior scale for concentration.
    #[arg(long)]
    pub(crate) concentration_prior_sd: f64,
    #[command(flatten)]
    pub(crate) nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
}
