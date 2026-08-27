#![forbid(unsafe_code)]

mod anisotropic_gp3d;
mod berman_turner;
mod beta_binomial_group_gender_agreement;
mod beta_binomial_group_gender_regression;
mod beta_binomial_group_gender_sbc;
mod beta_binomial_group_gender_sensitivity;
mod beta_binomial_group_gender_slide_agreement;
mod beta_binomial_group_gender_slide_hierarchy;
mod beta_binomial_group_gender_slide_sensitivity;
mod beta_binomial_group_regression;
mod beta_binomial_group_regression_agreement;
mod beta_binomial_group_regression_sbc;
mod beta_binomial_group_regression_sensitivity;
mod beta_binomial_hierarchy;
mod beta_binomial_hierarchy_agreement;
mod beta_binomial_hierarchy_sbc;
mod beta_binomial_hierarchy_sensitivity;
mod bym;
mod bym2;
mod car;
mod complementarity;
mod cross_modal;
mod distance_to_resource;
mod embedding_envelope;
mod embedding_factor;
mod embedding_kernel;
mod embedding_spatial;
mod fused_gromov;
mod geyer;
mod gmrf;
mod gp;
mod graph_signal;
mod gridded_lgcp;
mod gridded_lgcp_agreement;
mod gridded_lgcp_fit;
mod gridded_lgcp_sbc;
mod gridded_lgcp_sensitivity;
mod gridded_lgcp_spatial_ppc;
mod grouped_conformal;
mod hierarchical;
mod hierarchical_agreement;
mod hierarchical_sbc;
mod hierarchical_sensitivity;
mod icar;
mod inhomogeneous_poisson;
mod inhomogeneous_poisson_fit;
mod inla;
mod joint_mark;
mod laplace;
mod late_fusion;
mod matern_cluster;
mod meta_analysis;
mod mixture_of_experts;
mod model;
mod model_comparison;
mod multi_output_gp;
mod multiscale_kernel;
mod multitype;
mod nngp;
mod partial_fused_gromov;
mod partial_transport;
mod point_process_ppc;
mod prediction_calibration;
mod prediction_safety;
mod predictive_process;
mod predictive_stacking;
mod prior_sensitivity;
mod psis_loo;
mod replicated;
mod result;
mod retrieval;
mod sar;
mod sar_fit;
mod sbc;
mod smc;
mod spatial_varying_coefficient;
mod strauss;
mod strauss_gibbs;
mod strauss_pseudolikelihood;
mod student_t_hierarchy;
mod student_t_hierarchy_agreement;
mod student_t_hierarchy_sbc;
mod student_t_hierarchy_sensitivity;
mod thomas;
mod thomas_minimum_contrast;
mod transport;
mod variational_gp;
mod weights;

pub use anisotropic_gp3d::{
    anisotropic_matern32_covariance, AnisotropicGp3dFit, AnisotropicGp3dInputIdentity,
    AnisotropicGp3dModelIr, AnisotropicGp3dObservation, AnisotropicGp3dPosterior,
    AnisotropicGp3dPredictionCoordinate, AnisotropicGp3dPredictionSummary,
    AnisotropicGp3dResourceLimits, AnisotropicGp3dSpec, AnisotropicGp3dWorkerRequest,
    AnisotropicGp3dWorkerResult,
};
pub use berman_turner::{
    berman_turner_refinement, BermanTurnerError, BermanTurnerNode, BermanTurnerRefinementResult,
    BermanTurnerRefinementSpec, BermanTurnerResolution,
};
pub use beta_binomial_group_gender_agreement::{
    BetaBinomialGroupGenderAgreementComparison, BetaBinomialGroupGenderAgreementPolicy,
    BetaBinomialGroupGenderAgreementResult, BetaBinomialGroupGenderBackendSummary,
    NumpyroBetaBinomialGroupGenderWorkerRequest, NumpyroBetaBinomialGroupGenderWorkerResult,
};
pub use beta_binomial_group_gender_regression::{
    beta_binomial_group_gender_data_sha256, BetaBinomialGroupGenderPatientData,
    BetaBinomialGroupGenderPatientPosterior, BetaBinomialGroupGenderPosteriorPredictive,
    BetaBinomialGroupGenderRegressionInputIdentity, BetaBinomialGroupGenderRegressionModelIr,
    BetaBinomialGroupGenderRegressionPosterior, BetaBinomialGroupGenderRegressionResourceLimits,
    BetaBinomialGroupGenderRegressionResult, BetaBinomialGroupGenderRegressionSpec,
    BetaBinomialGroupGenderRegressionWorkerRequest, BetaBinomialGroupGenderRegressionWorkerResult,
};
pub use beta_binomial_group_gender_sbc::{
    BetaBinomialGroupGenderSbcCalibrationPolicy, BetaBinomialGroupGenderSbcDiagnostics,
    BetaBinomialGroupGenderSbcReplicate, BetaBinomialGroupGenderSbcResourceLimits,
    BetaBinomialGroupGenderSbcResult, NumpyroBetaBinomialGroupGenderSbcWorkerRequest,
    NumpyroBetaBinomialGroupGenderSbcWorkerResult,
};
pub use beta_binomial_group_gender_sensitivity::{
    BetaBinomialGroupGenderSensitivityResult, BetaBinomialGroupGenderSensitivityScenario,
    BetaBinomialGroupGenderSensitivityScenarioResult,
    BetaBinomialGroupGenderSensitivityScenarioRun,
};
pub use beta_binomial_group_gender_slide_agreement::{
    BetaBinomialGroupGenderSlideAgreementComparison, BetaBinomialGroupGenderSlideAgreementPolicy,
    BetaBinomialGroupGenderSlideAgreementResult, BetaBinomialGroupGenderSlideBackendSummary,
    NumpyroBetaBinomialGroupGenderSlideHierarchyWorkerRequest,
    NumpyroBetaBinomialGroupGenderSlideHierarchyWorkerResult,
};
pub use beta_binomial_group_gender_slide_hierarchy::{
    beta_binomial_group_gender_slide_data_sha256, BetaBinomialGroupGenderSlideData,
    BetaBinomialGroupGenderSlideHierarchyInputIdentity,
    BetaBinomialGroupGenderSlideHierarchyModelIr, BetaBinomialGroupGenderSlideHierarchyPosterior,
    BetaBinomialGroupGenderSlideHierarchyResourceLimits,
    BetaBinomialGroupGenderSlideHierarchyResult, BetaBinomialGroupGenderSlideHierarchySpec,
    BetaBinomialGroupGenderSlideHierarchyWorkerRequest,
    BetaBinomialGroupGenderSlideHierarchyWorkerResult,
    BetaBinomialGroupGenderSlidePatientPosterior, BetaBinomialGroupGenderSlidePosterior,
    BetaBinomialGroupGenderSlidePosteriorPredictive,
};
pub use beta_binomial_group_gender_slide_sensitivity::{
    BetaBinomialGroupGenderSlideSensitivityResult, BetaBinomialGroupGenderSlideSensitivityScenario,
    BetaBinomialGroupGenderSlideSensitivityScenarioResult,
    BetaBinomialGroupGenderSlideSensitivityScenarioRun,
};
pub use beta_binomial_group_regression::{
    beta_binomial_group_data_sha256, BetaBinomialGroupPatientData,
    BetaBinomialGroupPatientPosterior, BetaBinomialGroupPosteriorPredictive,
    BetaBinomialGroupRegressionInputIdentity, BetaBinomialGroupRegressionModelIr,
    BetaBinomialGroupRegressionPosterior, BetaBinomialGroupRegressionResourceLimits,
    BetaBinomialGroupRegressionResult, BetaBinomialGroupRegressionSpec,
    BetaBinomialGroupRegressionWorkerRequest, BetaBinomialGroupRegressionWorkerResult,
};
pub use beta_binomial_group_regression_agreement::{
    BetaBinomialGroupAgreementComparison, BetaBinomialGroupAgreementPolicy,
    BetaBinomialGroupAgreementResult, BetaBinomialGroupBackendSummary,
    NumpyroBetaBinomialGroupRegressionWorkerRequest,
    NumpyroBetaBinomialGroupRegressionWorkerResult,
};
pub use beta_binomial_group_regression_sbc::{
    BetaBinomialGroupSbcCalibrationPolicy, BetaBinomialGroupSbcDiagnostics,
    BetaBinomialGroupSbcReplicate, BetaBinomialGroupSbcResourceLimits, BetaBinomialGroupSbcResult,
    NumpyroBetaBinomialGroupSbcWorkerRequest, NumpyroBetaBinomialGroupSbcWorkerResult,
};
pub use beta_binomial_group_regression_sensitivity::{
    BetaBinomialGroupSensitivityResult, BetaBinomialGroupSensitivityScenario,
    BetaBinomialGroupSensitivityScenarioResult, BetaBinomialGroupSensitivityScenarioRun,
};
pub use beta_binomial_hierarchy::{
    patient_data_sha256, BetaBinomialHierarchyInputIdentity, BetaBinomialHierarchyModelIr,
    BetaBinomialHierarchyPosterior, BetaBinomialHierarchyResourceLimits,
    BetaBinomialHierarchyResult, BetaBinomialHierarchySpec, BetaBinomialHierarchyWorkerRequest,
    BetaBinomialHierarchyWorkerResult, BetaBinomialPatientData, BetaBinomialPatientPosterior,
    BetaBinomialPosteriorPredictive,
};
pub use beta_binomial_hierarchy_agreement::{
    BetaBinomialAgreementComparison, BetaBinomialAgreementPolicy, BetaBinomialAgreementResult,
    BetaBinomialBackendSummary, BetaBinomialParameterAgreement, BetaBinomialPatientAgreement,
    NumpyroBetaBinomialHierarchyWorkerRequest, NumpyroBetaBinomialHierarchyWorkerResult,
};
pub use beta_binomial_hierarchy_sbc::{
    BetaBinomialSbcCalibrationPolicy, BetaBinomialSbcDiagnostics, BetaBinomialSbcReplicate,
    BetaBinomialSbcResourceLimits, BetaBinomialSbcResult, NumpyroBetaBinomialSbcWorkerRequest,
    NumpyroBetaBinomialSbcWorkerResult,
};
pub use beta_binomial_hierarchy_sensitivity::{
    BetaBinomialSensitivityResult, BetaBinomialSensitivityScenario,
    BetaBinomialSensitivityScenarioResult, BetaBinomialSensitivityScenarioRun,
};
pub use bym::{
    BymFitResult, BymFitSpec, BymFitWorkerRequest, BymFitWorkerResult, BymInputIdentity,
    BymModelIr, BymPosterior, BymPosteriorPredictive, BymRegionSummary,
};
pub use bym2::{
    Bym2FitResult, Bym2FitSpec, Bym2FitWorkerRequest, Bym2FitWorkerResult, Bym2IcarScaling,
    Bym2InputIdentity, Bym2ModelIr, Bym2Posterior, Bym2RegionSummary,
};
pub use fused_gromov::{
    FgwInitializationResult, FgwPlanEntry, FgwResources, FgwSupport, FusedGromovWassersteinSpec,
    FusedGromovWassersteinWorkerRequest, FusedGromovWassersteinWorkerResult,
};
pub use geyer::{geyer_saturation_statistic, GeyerPointSummary, GeyerSaturationResult};
pub use gmrf::{gmrf_log_density, GmrfConstraint, GmrfDensityError, GmrfDensityResult, GmrfSpec};
pub use gp::{
    ExactGpFit, ExactGpInputIdentity, ExactGpSpec, ExactGpWorkerRequest, ExactGpWorkerResult,
    GpObservation, GpPredictionCoordinate, GpResourceLimits,
};
pub use graph_signal::{
    graph_dirichlet_energy, graph_smoothness_permutation_test, local_embedding_roughness,
    GraphDirichletEnergyResult, GraphDirichletEnergySpec, GraphEnergyNormalization, GraphLaplacian,
    GraphSignalEdge, GraphSignalError, GraphSignalNode, GraphSmoothnessPermutationResult,
    GraphSmoothnessPermutationSpec, LocalEmbeddingRoughnessResult, LocalEmbeddingRoughnessRow,
    LocalEmbeddingRoughnessSpec, StratifiedGraphSignalNode,
};
pub use gridded_lgcp::{
    build_gridded_lgcp, GriddedLgcpCell, GriddedLgcpError, GriddedLgcpModel, GriddedLgcpModelIr,
    GriddedLgcpSpec,
};
pub use gridded_lgcp_agreement::{
    GriddedLgcpAgreementComparison, GriddedLgcpAgreementPolicy, GriddedLgcpAgreementResult,
    GriddedLgcpBackendSummary, LgcpFieldAgreement, LgcpScalarAgreement,
    NumpyroGriddedLgcpWorkerRequest, NumpyroGriddedLgcpWorkerResult,
};
pub use gridded_lgcp_fit::{
    GriddedLgcpCellPosterior, GriddedLgcpFitInputIdentity, GriddedLgcpFitModelIr,
    GriddedLgcpFitResourceLimits, GriddedLgcpFitResult, GriddedLgcpFitSpec,
    GriddedLgcpFitWorkerRequest, GriddedLgcpFitWorkerResult, GriddedLgcpPosterior,
    GriddedLgcpPosteriorPredictive, GriddedLgcpPredictionApproximation,
    GriddedLgcpPredictionControls, GriddedLgcpPredictionResult, GriddedLgcpPredictiveDraw,
    GriddedLgcpPredictivePattern, GriddedLgcpPredictivePoint,
};
pub use gridded_lgcp_sbc::{
    GriddedLgcpSbcCalibrationPolicy, GriddedLgcpSbcDiagnostics, GriddedLgcpSbcFailure,
    GriddedLgcpSbcParameterDiagnostics, GriddedLgcpSbcReplicate, GriddedLgcpSbcResourceLimits,
    GriddedLgcpSbcResult, NumpyroGriddedLgcpSbcWorkerRequest, NumpyroGriddedLgcpSbcWorkerResult,
};
pub use gridded_lgcp_sensitivity::{
    GriddedLgcpSensitivityResult, GriddedLgcpSensitivityScenario,
    GriddedLgcpSensitivityScenarioResult, GriddedLgcpSensitivityScenarioRun,
};
pub use gridded_lgcp_spatial_ppc::{
    gridded_lgcp_spatial_ppc, GriddedLgcpSpatialPpcResult, GriddedLgcpSpatialPpcSummaries,
    GriddedLgcpSpatialPpcSummary,
};
pub use grouped_conformal::{
    CoverageRow, GroupedConformalModel, GroupedConformalPatient, GroupedConformalPrediction,
    GroupedConformalResources, GroupedConformalSpec, GroupedConformalWorkerRequest,
    GroupedConformalWorkerResult, GroupedCoverage,
};
pub use hierarchical::{
    GaussianHierarchyFit, GaussianHierarchyInputIdentity, GaussianHierarchySpec,
    GaussianHierarchyWorkerRequest, HalfNormalPrior, HierarchicalPatientData,
    HierarchicalPosteriorPredictive, HierarchicalWorkerResult, NormalPrior, PartialPoolingSummary,
    ScalarPosteriorSummary,
};
pub use hierarchical_agreement::{
    HierarchicalAgreementComparison, HierarchicalAgreementPolicy, HierarchicalAgreementResult,
    HierarchicalBackendSummary, NumpyroHierarchyWorkerRequest, NumpyroHierarchyWorkerResult,
    ParameterAgreement,
};
pub use hierarchical_sbc::{
    HierarchicalSbcCalibrationPolicy, HierarchicalSbcDiagnostics, HierarchicalSbcFailure,
    HierarchicalSbcParameterDiagnostics, HierarchicalSbcReplicate, HierarchicalSbcResourceLimits,
    HierarchicalSbcResult, NumpyroHierarchySbcWorkerRequest, NumpyroHierarchySbcWorkerResult,
};
pub use hierarchical_sensitivity::{
    HierarchicalPriorScenario, HierarchicalPriorScenarioResult, HierarchicalPriorScenarioRun,
    HierarchicalPriorSensitivityResult,
};
pub use icar::{build_icar_plan, IcarPlan, IcarPlanError};
pub use inhomogeneous_poisson::{
    inhomogeneous_poisson_log_likelihood, InhomogeneousPoissonError, InhomogeneousPoissonEvent,
    InhomogeneousPoissonLikelihoodResult, InhomogeneousPoissonSpec, MidpointQuadratureValue,
    RectangularWindow,
};
pub use inhomogeneous_poisson_fit::{
    InhomogeneousPoissonCellSummary, InhomogeneousPoissonFitInputIdentity,
    InhomogeneousPoissonFitModelIr, InhomogeneousPoissonFitResourceLimits,
    InhomogeneousPoissonFitResult, InhomogeneousPoissonFitSpec,
    InhomogeneousPoissonFitWorkerRequest, InhomogeneousPoissonFitWorkerResult,
    InhomogeneousPoissonPosterior, InhomogeneousPoissonPosteriorPredictive,
};
pub use inla::{
    InlaGridDiagnostics, InlaGridPoint, InlaGridSpec, InlaHmcComparison, InlaLatentMarginal,
    InlaPosteriorPredictive, PoissonInlaFit, PoissonInlaInputIdentity, PoissonInlaModelIr,
    PoissonInlaSpec, PoissonInlaWorkerRequest, PoissonInlaWorkerResult,
};
pub use joint_mark::{
    build_joint_continuous_mark_model, build_joint_location_mark_model, JointCategoricalMarkModel,
    JointCategoricalMarkModelIr, JointCategoricalMarkSpec, JointCategoricalPoint,
    JointContinuousMarkModel, JointContinuousMarkModelIr, JointContinuousMarkSpec,
    JointContinuousPoint, JointLocationGridRow, JointMarkError,
};
pub use laplace::{
    LaplaceHessian, LaplaceMode, LaplaceOptimizerDiagnostics, LaplaceOptimizerSpec,
    LaplacePosteriorPredictive, LaplaceRateApproximation, PoissonExposureObservation,
    PoissonLaplaceFit, PoissonLaplaceInputIdentity, PoissonLaplaceModelIr, PoissonLaplaceSpec,
    PoissonLaplaceWorkerRequest, PoissonLaplaceWorkerResult,
};
pub use late_fusion::{
    LateFusionCalibrator, LateFusionMetric, LateFusionModel, LateFusionPatient,
    LateFusionPrediction, LateFusionResources, LateFusionSpec, LateFusionWorkerRequest,
    LateFusionWorkerResult, MissingScenario, ModalityAblation,
};
pub use matern_cluster::{
    simulate_matern_cluster_process, MaternClusterBoundary, MaternClusterProcessError,
    MaternClusterProcessResult, MaternClusterProcessSpec,
};
pub use meta_analysis::{
    MetaAnalysisFit, MetaAnalysisInputIdentity, MetaAnalysisSpec, MetaAnalysisWorkerRequest,
    MetaAnalysisWorkerResult, SiteEstimate,
};
pub use mixture_of_experts::{
    MixtureCalibrator, MixtureGateModel, MixtureMetrics, MixtureOfExpertsPatient,
    MixtureOfExpertsResources, MixtureOfExpertsSpec, MixtureOfExpertsWorkerRequest,
    MixtureOfExpertsWorkerResult, MixturePrediction,
};
pub use model_comparison::{
    compare_psis_loo_models, BayesianModelComparisonResult, ComparedModel, PairwiseElpdDifference,
    PsisLooComparisonInput,
};
pub use multi_output_gp::{
    MultiOutputGpFit, MultiOutputGpInputIdentity, MultiOutputGpObservation, MultiOutputGpSpec,
    MultiOutputGpWorkerRequest, MultiOutputGpWorkerResult,
};
pub use multiscale_kernel::{
    multiscale_embedding_kernel, MultiscaleBaseKernel, MultiscaleEmbeddingKernelResult,
    MultiscaleEmbeddingKernelSpec, MultiscaleEmbeddingSummary, MultiscaleKernelError,
    MultiscaleKernelScaleResult, MultiscaleKernelSensitivity, MultiscaleKernelWeight,
};
pub use multitype::{
    multitype_papangelou, MultitypeBaseline, MultitypeContribution, MultitypeError,
    MultitypeInteraction, MultitypePapangelouResult, MultitypePapangelouSpec, MultitypePoint,
};
pub use nngp::{
    build_nngp, full_gp_log_density, nngp_log_density, NngpError, NngpObservation, NngpPlan,
    NngpSpec,
};
pub use partial_fused_gromov::{
    PartialFgwFit, PartialFgwResources, PartialFusedGromovWassersteinSpec,
    PartialFusedGromovWassersteinWorkerRequest, PartialFusedGromovWassersteinWorkerResult,
};
pub use partial_transport::{
    PartialTransportOptimizer, PartialTransportPlanEntry, PartialTransportResources,
    PartialTransportSpec, PartialTransportWorkerRequest, PartialTransportWorkerResult,
};
pub use point_process_ppc::{
    posterior_predictive_point_process_diagnostics, PointProcessPpcCurveRow, PointProcessPpcError,
    PointProcessPpcResult, PointProcessPpcSpec, PpcPoint, PpcReplicatedPoint,
};
pub use prediction_calibration::{
    CalibratedPrediction, PlattCalibrator, PredictionCalibrationMetrics,
    PredictionCalibrationResources, PredictionCalibrationRow, PredictionCalibrationSpec,
    PredictionCalibrationWorkerRequest, PredictionCalibrationWorkerResult, ReliabilityBin,
};
pub use prediction_safety::{
    apply_abstention, mahalanobis_ood_score, AbstentionDecision, AbstentionPolicy,
    AbstentionStatus, MahalanobisOodResult, MahalanobisOodSpec, OodRepresentationUnit,
    OodUnitScore, PredictionForAbstention, PredictionSafetyError,
};
pub use predictive_process::{
    low_rank_predictive_process, FieldCoordinate1D, PredictiveProcessError, PredictiveProcessPlan,
    PredictiveProcessSpec,
};
pub use predictive_stacking::{
    PredictiveStackingPatient, PredictiveStackingResources, PredictiveStackingSpec,
    PredictiveStackingWorkerRequest, PredictiveStackingWorkerResult, StackingPatientDensity,
    StackingWeight, StackingWeightSensitivity,
};
pub use prior_sensitivity::{
    NormalMeanPriorSensitivitySpec, NormalPriorAlternative, PriorSensitivityInputIdentity,
    PriorSensitivityModelIr, PriorSensitivityResourceLimits, PriorSensitivityResult,
    PriorSensitivitySummary, PriorSensitivityWorkerRequest, PriorSensitivityWorkerResult,
};
pub use psis_loo::{
    PointwiseLogLikelihoodDraw, PsisLooInputIdentity, PsisLooPointwise, PsisLooResourceLimits,
    PsisLooResult, PsisLooSpec, PsisLooWorkerRequest, PsisLooWorkerResult,
};

pub use car::{
    car_density, CarDensityError, CarDensityResult, CarMode, CarSpec, IslandPolicy,
    RegionFieldValue,
};
pub use complementarity::{
    expected_model_features, CellPatchComplementaritySpec, CellPatchComplementarityWorkerRequest,
    CellPatchComplementarityWorkerResult, ComplementarityFeatureNames,
    ComplementarityFoldSelection, ComplementarityModelResult, ComplementarityPatientRow,
    ComplementarityPrediction,
};
pub use cross_modal::{
    cross_modal_covariance_by_distance, CrossModalCovarianceError, CrossModalCovarianceMatrix,
    CrossModalCovarianceMatrixArtifact, CrossModalCovarianceResult, CrossModalCovarianceSpec,
    CrossModalCovarianceSummary, CrossModalMeanPolicy, CrossModalPair, EmbeddingModalityRow,
};
pub use distance_to_resource::{
    fit_distance_to_resource_model, DerivedResourceDistance, DistanceOutcomeObservation,
    DistancePosterior, DistancePosteriorPredictive, DistancePrediction, DistanceResponsePoint,
    DistanceToResourceFit, DistanceToResourceModel, DistanceToResourceSpec,
    GaussianPosteriorSummary, PatientPredictiveCheck, ResourcePredictiveCheck, ResourceSegment,
};
pub use embedding_envelope::{
    test_embedding_spatial_dependence, EmbeddingEnvelopeCurveRow, EmbeddingEnvelopeError,
    EmbeddingEnvelopeRow, EmbeddingSpatialCurveFunction, EmbeddingSpatialDependenceResult,
    EmbeddingSpatialDependenceSpec,
};
pub use embedding_factor::{
    joint_location_embedding_latent_factor_model, EmbeddingFactorError, EmbeddingFactorModel,
    EmbeddingFactorModelIr, EmbeddingFactorPoint, EmbeddingFactorSpec,
};
pub use embedding_kernel::{
    build_embedding_kernel, kernel_mark_correlation, EmbeddingKernelArtifact, EmbeddingKernelError,
    EmbeddingKernelKind, EmbeddingKernelSpec, KernelEmbeddingRow, KernelMarkCorrelationCurve,
    KernelMarkCorrelationResult, KernelMarkCorrelationRow, KernelMarkCorrelationSpec,
};
pub use embedding_spatial::{
    embedding_cross_covariance_by_distance, vector_semivariogram, EmbeddingCrossCovarianceMatrix,
    EmbeddingCrossCovarianceMatrixArtifact, EmbeddingCrossCovarianceResult,
    EmbeddingCrossCovarianceSpec, EmbeddingCrossCovarianceSummary, EmbeddingDistanceBin,
    EmbeddingPairWeight, EmbeddingSpatialError, EmbeddingSpatialPoint,
    ProjectedEmbeddingInputIdentity, ProjectedEmbeddingPoint, ProjectedEmbeddingVariogramResult,
    ProjectedEmbeddingVariogramSpec, ProjectedEmbeddingVariogramWorkerRequest,
    ProjectedEmbeddingVariogramWorkerResult, ProjectedSplitCurve, ProjectedVariogramRow,
    ProjectionArtifact, VectorSemivariogramResult, VectorSemivariogramRow, VectorSemivariogramSpec,
};
pub use model::{
    sha256_hex, BackendContract, DiagnosticPolicy, NormalMeanModelIr, NormalMeanSpec,
    NormalMeanWorkerRequest, NutsSamplingSpec, WorkerResourceLimits,
};
pub use replicated::{
    replicated_hierarchical_lgcp, ReplicatedError, ReplicatedFieldPolicy, ReplicatedLgcpModel,
    ReplicatedLgcpModelIr, ReplicatedLgcpPattern, ReplicatedLgcpSpec,
};
pub use result::{
    FitState, NormalMeanDiagnostics, NormalMeanFit, NormalMeanInputIdentity, NormalMeanPosterior,
    NormalMeanPosteriorPredictive, SamplingSummary, WorkerBackend, WorkerResult,
};
pub use retrieval::{
    build_region_retrieval_index, retrieve_analogous_regions, QueryRegion, RegionRetrievalError,
    RegionRetrievalIndexArtifact, RegionRetrievalMatch, RegionRetrievalResult,
    RetrievalLeakagePolicy, TrainingRegion,
};
pub use sar::{
    sar_gaussian_log_likelihood, SarError, SarImpact, SarLikelihoodResult, SarModelType, SarSpec,
};
pub use sar_fit::{
    SarCoefficientSummary, SarFitInputIdentity, SarFitModelIr, SarFitPosterior, SarFitResult,
    SarFitSpec, SarFitWorkerRequest, SarFitWorkerResult, SarImpactSummary, SarPosteriorPredictive,
    SarScalarSummary,
};
pub use sbc::{
    NormalMeanSbcModelIr, NormalMeanSbcResult, NormalMeanSbcSpec, NormalMeanSbcWorkerRequest,
    NormalMeanSbcWorkerResult, SbcDiagnostics, SbcExecutionSpec, SbcFailure, SbcReplicate,
    SbcResourceLimits,
};
pub use smc::{
    NormalMeanSmcFit, NormalMeanSmcInputIdentity, NormalMeanSmcWorkerRequest,
    NormalMeanSmcWorkerResult, SmcChain, SmcDiagnostics, SmcEvidence, SmcResourceLimits,
    SmcSamplingSpec, SmcStage,
};
pub use spatial_varying_coefficient::{
    SpatialCoefficientFieldSummary, SpatialCoefficientInputIdentity, SpatialCoefficientObservation,
    SpatialCoefficientPosterior, SpatialCoefficientPosteriorPredictive,
    SpatialVaryingCoefficientFit, SpatialVaryingCoefficientModelIr, SpatialVaryingCoefficientSpec,
    SpatialVaryingCoefficientWorkerRequest, SpatialVaryingCoefficientWorkerResult,
};
pub use strauss::{
    strauss_papangelou, strauss_statistics, StraussError, StraussPapangelou, StraussPoint,
    StraussProposal, StraussSufficientStatistics,
};
pub use strauss_gibbs::{
    simulate_strauss_birth_death, StraussBirthDeathDiagnostics, StraussBirthDeathError,
    StraussBirthDeathResult, StraussBirthDeathSpec,
};
pub use strauss_pseudolikelihood::{
    StraussPseudolikelihoodBounds, StraussPseudolikelihoodFitSummary,
    StraussPseudolikelihoodInputIdentity, StraussPseudolikelihoodModelIr,
    StraussPseudolikelihoodRefinement, StraussPseudolikelihoodResolution,
    StraussPseudolikelihoodResources, StraussPseudolikelihoodResult, StraussPseudolikelihoodRow,
    StraussPseudolikelihoodSpec, StraussPseudolikelihoodWorkerRequest,
    StraussPseudolikelihoodWorkerResult,
};
pub use student_t_hierarchy::{
    StudentTHierarchyModelIr, StudentTHierarchyPosterior, StudentTHierarchyPosteriorPredictive,
    StudentTHierarchyResult, StudentTHierarchySpec, StudentTHierarchyWorkerRequest,
    StudentTHierarchyWorkerResult, StudentTLikelihood,
};
pub use student_t_hierarchy_agreement::{
    NumpyroStudentTHierarchyWorkerRequest, NumpyroStudentTHierarchyWorkerResult,
    StudentTHierarchyAgreementComparison, StudentTHierarchyAgreementPolicy,
    StudentTHierarchyAgreementResult, StudentTHierarchyBackendSummary, StudentTParameterAgreement,
    StudentTPatientMeanAgreement,
};
pub use student_t_hierarchy_sbc::{
    NumpyroStudentTSbcWorkerRequest, NumpyroStudentTSbcWorkerResult, StudentTSbcCalibrationPolicy,
    StudentTSbcDiagnostics, StudentTSbcReplicate, StudentTSbcResourceLimits, StudentTSbcResult,
};
pub use student_t_hierarchy_sensitivity::{
    StudentTHierarchySensitivityResult, StudentTSensitivityScenario,
    StudentTSensitivityScenarioResult, StudentTSensitivityScenarioRun,
};
pub use thomas::{
    simulate_thomas_process, NeymanScottCounts, NeymanScottOffspring, NeymanScottParent,
    ThomasCounts, ThomasOffspring, ThomasParent, ThomasProcessError, ThomasProcessResult,
    ThomasProcessSpec, ThomasTruncation,
};
pub use thomas_minimum_contrast::{
    ThomasKCurveRow, ThomasLikelihoodComparison, ThomasMinimumContrastBounds,
    ThomasMinimumContrastCurveFitRow, ThomasMinimumContrastFitSummary,
    ThomasMinimumContrastInputIdentity, ThomasMinimumContrastModelIr,
    ThomasMinimumContrastResources, ThomasMinimumContrastResult, ThomasMinimumContrastSpec,
    ThomasMinimumContrastWorkerRequest, ThomasMinimumContrastWorkerResult,
};
pub use transport::{
    entropic_soft_assignment, sinkhorn_ot, unbalanced_sinkhorn, EntropicSoftAssignmentResult,
    EntropicSoftAssignmentSensitivity, EntropicSoftAssignmentSpec, SinkhornResult, SinkhornSpec,
    TransportError, TransportMass, TransportPlanEntry, UnbalancedSinkhornResult,
    UnbalancedSinkhornSpec,
};
pub use variational_gp::{
    VariationalGpFit, VariationalGpInputIdentity, VariationalGpSpec, VariationalGpWorkerRequest,
    VariationalGpWorkerResult,
};
pub use weights::{
    validate_spatial_weights, DiagonalPolicy, NormalizationPolicy, SpatialEdge,
    SpatialWeightsError, SpatialWeightsPolicy, SymmetryPolicy, ValidatedSpatialWeights,
    ValidatedWeight,
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BayesError {
    #[error("invalid Bayesian specification: {0}")]
    InvalidSpec(String),
    #[error("Bayesian worker contract violation: {0}")]
    WorkerContract(String),
    #[error("Bayesian JSON boundary failed: {0}")]
    Json(#[from] serde_json::Error),
}
