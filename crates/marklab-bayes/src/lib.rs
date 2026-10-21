//! Bayesian model requests, worker results and posterior checks for Marklab.
//!
//! Most models are fitted by pinned Python workers (PyMC, NumPyro or NumPy/SciPy). For each
//! model this crate builds a typed worker request, validates the worker's result, and turns
//! it into a result document:
//!
//! - **Requests** (`*WorkerRequest::new`) check the specification and resource limits, and
//!   record the model as an intermediate representation (`*ModelIr`) that names its
//!   likelihood, priors and parameterization. Every request also carries a
//!   [`BackendContract`]: the backend and Python versions and the SHA-256 of the environment
//!   lock and worker script, so a result can be traced to the exact code that produced it.
//! - **Results** (`*WorkerResult::validate`) reject a worker output whose identity, request
//!   hash, dimensions or summaries do not match the request. A fit counts as complete only
//!   when its sampler diagnostics satisfy the request's [`DiagnosticPolicy`] (R-hat, bulk
//!   and tail ESS, E-BFMI, divergences and tree-depth hits); otherwise its summaries are
//!   still reported, but marked nonconverged and diagnostic-only.
//! - **Calibration checks** come in three kinds per model: simulation-based calibration
//!   (`*_sbc`), whose rank histograms and interval coverage are recomputed here from the
//!   replicate rows; two-backend agreement (`*_agreement`), which compares PyMC and NumPyro
//!   posteriors within their combined Monte Carlo standard error; and prior sensitivity
//!   (`*_sensitivity`) over a declared grid of priors.
//!
//! A few estimators run entirely in Rust, for example the CAR, SAR and GMRF densities, the
//! Berman-Turner quadrature.
#![forbid(unsafe_code)]

pub mod advanced;
mod agreement_metric;
mod agreement_status;
mod anisotropic_gp3d;
mod sensitivity_status;
pub use marklab_core::backend::versions as backend_versions;
mod berman_turner;
mod beta_binomial_group_gender_agreement;
mod beta_binomial_group_gender_regression;
mod beta_binomial_group_gender_sbc;
mod beta_binomial_group_gender_sensitivity;
mod beta_binomial_group_gender_slide_agreement;
mod beta_binomial_group_gender_slide_hierarchy;
mod beta_binomial_group_gender_slide_sbc;
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
mod dirichlet_multinomial_group;
mod dirichlet_multinomial_group_agreement;
mod dirichlet_multinomial_group_sbc;
mod dirichlet_multinomial_group_sensitivity;
mod distance_to_resource;
mod gaussian_crossed_nested_hierarchy;
mod geyer;
mod gmrf;
mod gp;
mod gridded_lgcp;
mod gridded_lgcp_agreement;
mod gridded_lgcp_fit;
mod gridded_lgcp_sbc;
mod gridded_lgcp_sensitivity;
mod gridded_lgcp_spatial_ppc;
mod hierarchical;
mod hierarchical_agreement;
mod hierarchical_sbc;
mod hierarchical_sensitivity;
mod hurdle_beta_binomial_group;
mod icar;
mod inhomogeneous_poisson;
mod inhomogeneous_poisson_fit;
mod inla;
mod joint_mark;
mod laplace;
mod meta_analysis;
mod model;
mod model_comparison;
mod multi_output_gp;
mod multitype;
mod negative_binomial_hierarchy;
mod nngp;
mod nonproportional_ordinal_group_site;
mod ordinal_group;
mod ordinal_group_site_hierarchy;
mod ordinal_heldout;
mod point_process_ppc;
mod prediction_calibration;
mod prediction_safety;
mod predictive_process;
mod predictive_stacking;
mod prior_sensitivity;
pub mod protocol;
mod psis_loo;
mod replicated;
mod result;
mod sar;
mod sar_fit;
mod sbc;
mod smc;
mod sparse_car;
mod spatial_varying_coefficient;
mod strauss;
mod strauss_gibbs;
mod strauss_pseudolikelihood;
mod student_t_hierarchy;
mod student_t_hierarchy_agreement;
mod student_t_hierarchy_sbc;
mod student_t_hierarchy_sensitivity;
mod validation;
mod variational_gp;
mod weights;

pub use agreement_metric::{
    combined_mcse, compare_elements, compare_parameter, intervals_overlap, ElementwiseAgreement,
    ParameterAgreement, PatientAgreement,
};
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
    BetaBinomialGroupGenderPatientData, BetaBinomialGroupGenderPatientPosterior,
    BetaBinomialGroupGenderPosteriorPredictive, BetaBinomialGroupGenderRegressionInputIdentity,
    BetaBinomialGroupGenderRegressionModelIr, BetaBinomialGroupGenderRegressionPosterior,
    BetaBinomialGroupGenderRegressionResourceLimits, BetaBinomialGroupGenderRegressionResult,
    BetaBinomialGroupGenderRegressionSpec, BetaBinomialGroupGenderRegressionWorkerRequest,
    BetaBinomialGroupGenderRegressionWorkerResult,
};
pub use beta_binomial_group_gender_sbc::{
    BetaBinomialGroupGenderSbcDiagnostics, BetaBinomialGroupGenderSbcReplicate,
    BetaBinomialGroupGenderSbcResourceLimits, BetaBinomialGroupGenderSbcResult,
    NumpyroBetaBinomialGroupGenderSbcWorkerRequest, NumpyroBetaBinomialGroupGenderSbcWorkerResult,
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
    BetaBinomialGroupGenderSlideData, BetaBinomialGroupGenderSlideHierarchyInputIdentity,
    BetaBinomialGroupGenderSlideHierarchyModelIr, BetaBinomialGroupGenderSlideHierarchyPosterior,
    BetaBinomialGroupGenderSlideHierarchyResourceLimits,
    BetaBinomialGroupGenderSlideHierarchyResult, BetaBinomialGroupGenderSlideHierarchySpec,
    BetaBinomialGroupGenderSlideHierarchyWorkerRequest,
    BetaBinomialGroupGenderSlideHierarchyWorkerResult,
    BetaBinomialGroupGenderSlidePatientPosterior, BetaBinomialGroupGenderSlidePosterior,
    BetaBinomialGroupGenderSlidePosteriorPredictive,
};
pub use beta_binomial_group_gender_slide_sbc::{
    BetaBinomialGroupGenderSlideSbcDiagnostics, BetaBinomialGroupGenderSlideSbcReplicate,
    BetaBinomialGroupGenderSlideSbcResourceLimits, BetaBinomialGroupGenderSlideSbcResult,
    NumpyroBetaBinomialGroupGenderSlideSbcWorkerRequest,
    NumpyroBetaBinomialGroupGenderSlideSbcWorkerResult,
};
pub use beta_binomial_group_gender_slide_sensitivity::{
    BetaBinomialGroupGenderSlideSensitivityResult, BetaBinomialGroupGenderSlideSensitivityScenario,
    BetaBinomialGroupGenderSlideSensitivityScenarioResult,
    BetaBinomialGroupGenderSlideSensitivityScenarioRun,
};
pub use beta_binomial_group_regression::{
    BetaBinomialGroupPatientData, BetaBinomialGroupPatientPosterior,
    BetaBinomialGroupPosteriorPredictive, BetaBinomialGroupRegressionInputIdentity,
    BetaBinomialGroupRegressionModelIr, BetaBinomialGroupRegressionPosterior,
    BetaBinomialGroupRegressionResourceLimits, BetaBinomialGroupRegressionResult,
    BetaBinomialGroupRegressionSpec, BetaBinomialGroupRegressionWorkerRequest,
    BetaBinomialGroupRegressionWorkerResult,
};
pub use beta_binomial_group_regression_agreement::{
    BetaBinomialGroupAgreementComparison, BetaBinomialGroupAgreementPolicy,
    BetaBinomialGroupAgreementResult, BetaBinomialGroupBackendSummary,
    NumpyroBetaBinomialGroupRegressionWorkerRequest,
    NumpyroBetaBinomialGroupRegressionWorkerResult,
};
pub use beta_binomial_group_regression_sbc::{
    BetaBinomialGroupSbcDiagnostics, BetaBinomialGroupSbcReplicate,
    BetaBinomialGroupSbcResourceLimits, BetaBinomialGroupSbcResult,
    NumpyroBetaBinomialGroupSbcWorkerRequest, NumpyroBetaBinomialGroupSbcWorkerResult,
};
pub use beta_binomial_group_regression_sensitivity::{
    BetaBinomialGroupSensitivityResult, BetaBinomialGroupSensitivityScenario,
    BetaBinomialGroupSensitivityScenarioResult, BetaBinomialGroupSensitivityScenarioRun,
};
pub use beta_binomial_hierarchy::{
    BetaBinomialHierarchyInputIdentity, BetaBinomialHierarchyModelIr,
    BetaBinomialHierarchyPosterior, BetaBinomialHierarchyResourceLimits,
    BetaBinomialHierarchyResult, BetaBinomialHierarchySpec, BetaBinomialHierarchyWorkerRequest,
    BetaBinomialHierarchyWorkerResult, BetaBinomialPatientData, BetaBinomialPatientPosterior,
    BetaBinomialPosteriorPredictive,
};
pub use beta_binomial_hierarchy_agreement::{
    BetaBinomialAgreementComparison, BetaBinomialAgreementPolicy, BetaBinomialAgreementResult,
    BetaBinomialBackendSummary, NumpyroBetaBinomialHierarchyWorkerRequest,
    NumpyroBetaBinomialHierarchyWorkerResult,
};
pub use beta_binomial_hierarchy_sbc::{
    BetaBinomialSbcDiagnostics, BetaBinomialSbcReplicate, BetaBinomialSbcResourceLimits,
    BetaBinomialSbcResult, NumpyroBetaBinomialSbcWorkerRequest, NumpyroBetaBinomialSbcWorkerResult,
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
pub use dirichlet_multinomial_group::{
    DirichletMultinomialClassPosterior, DirichletMultinomialClassPredictive,
    DirichletMultinomialGroupInputIdentity, DirichletMultinomialGroupModelIr,
    DirichletMultinomialGroupPosterior, DirichletMultinomialGroupPosteriorPredictive,
    DirichletMultinomialGroupResourceLimits, DirichletMultinomialGroupResult,
    DirichletMultinomialGroupSpec, DirichletMultinomialGroupWorkerRequest,
    DirichletMultinomialGroupWorkerResult, DirichletMultinomialPatientData,
};
pub use dirichlet_multinomial_group_agreement::{
    DirichletMultinomialClassAgreement, DirichletMultinomialGroupAgreementComparison,
    DirichletMultinomialGroupAgreementPolicy, DirichletMultinomialGroupAgreementResult,
    DirichletMultinomialGroupBackendSummary, NumpyroDirichletMultinomialGroupWorkerRequest,
    NumpyroDirichletMultinomialGroupWorkerResult,
};
pub use dirichlet_multinomial_group_sbc::{
    DirichletMultinomialGroupSbcDiagnostics, DirichletMultinomialGroupSbcReplicate,
    DirichletMultinomialGroupSbcResourceLimits, DirichletMultinomialGroupSbcResult,
    NumpyroDirichletMultinomialGroupSbcWorkerRequest,
    NumpyroDirichletMultinomialGroupSbcWorkerResult,
};
pub use dirichlet_multinomial_group_sensitivity::{
    DirichletMultinomialGroupSensitivityResult, DirichletMultinomialGroupSensitivityScenario,
    DirichletMultinomialGroupSensitivityScenarioResult,
    DirichletMultinomialGroupSensitivityScenarioRun,
};
pub use gaussian_crossed_nested_hierarchy::{
    GaussianCrossedNestedCounts, GaussianCrossedNestedHierarchyFit,
    GaussianCrossedNestedHierarchyModelIr, GaussianCrossedNestedHierarchyResources,
    GaussianCrossedNestedHierarchySpec, GaussianCrossedNestedHierarchyWorkerRequest,
    GaussianCrossedNestedInputIdentity, GaussianCrossedNestedObservation,
    GaussianCrossedNestedPosterior, GaussianCrossedNestedPpc, GaussianCrossedNestedWorkerResult,
    GaussianVariancePartition,
};
pub use geyer::{geyer_saturation_statistic, GeyerPointSummary, GeyerSaturationResult};
pub use gmrf::{gmrf_log_density, GmrfConstraint, GmrfDensityError, GmrfDensityResult, GmrfSpec};
pub use gp::{
    ExactGpFit, ExactGpInputIdentity, ExactGpSpec, ExactGpWorkerRequest, ExactGpWorkerResult,
    GpObservation, GpPredictionCoordinate, GpResourceLimits,
};
pub use gridded_lgcp::{
    build_gridded_lgcp, GriddedLgcpCell, GriddedLgcpError, GriddedLgcpModel, GriddedLgcpModelIr,
    GriddedLgcpSpec,
};
pub use gridded_lgcp_agreement::{
    GriddedLgcpAgreementComparison, GriddedLgcpAgreementPolicy, GriddedLgcpAgreementResult,
    GriddedLgcpBackendSummary, NumpyroGriddedLgcpWorkerRequest, NumpyroGriddedLgcpWorkerResult,
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
    GriddedLgcpSbcReplicate, GriddedLgcpSbcResourceLimits, GriddedLgcpSbcResult,
    NumpyroGriddedLgcpSbcWorkerRequest, NumpyroGriddedLgcpSbcWorkerResult,
};
pub use gridded_lgcp_sensitivity::{
    GriddedLgcpSensitivityResult, GriddedLgcpSensitivityScenario,
    GriddedLgcpSensitivityScenarioResult, GriddedLgcpSensitivityScenarioRun,
};
pub use gridded_lgcp_spatial_ppc::{
    gridded_lgcp_spatial_ppc, GriddedLgcpSpatialPpcResult, GriddedLgcpSpatialPpcSummaries,
    GriddedLgcpSpatialPpcSummary,
};
pub use hierarchical::{
    GaussianHierarchyFit, GaussianHierarchyInputIdentity, GaussianHierarchySpec,
    GaussianHierarchyWorkerRequest, HalfNormalPrior, HierarchicalPatientData,
    HierarchicalPosteriorPredictive, HierarchicalWorkerResult, NormalPrior, PartialPoolingSummary,
};
pub use hierarchical_agreement::{
    HierarchicalAgreementComparison, HierarchicalAgreementPolicy, HierarchicalAgreementResult,
    HierarchicalBackendSummary, NumpyroHierarchyWorkerRequest, NumpyroHierarchyWorkerResult,
};
pub use hierarchical_sbc::{
    HierarchicalSbcCalibrationPolicy, HierarchicalSbcDiagnostics, HierarchicalSbcFailure,
    HierarchicalSbcReplicate, HierarchicalSbcResourceLimits, HierarchicalSbcResult,
    NumpyroHierarchySbcWorkerRequest, NumpyroHierarchySbcWorkerResult,
};
pub use hierarchical_sensitivity::{
    HierarchicalPriorScenario, HierarchicalPriorScenarioResult, HierarchicalPriorScenarioRun,
    HierarchicalPriorSensitivityResult,
};
pub use hurdle_beta_binomial_group::{
    HurdleBetaBinomialGroupInputIdentity, HurdleBetaBinomialGroupModelIr,
    HurdleBetaBinomialGroupPatientData, HurdleBetaBinomialGroupPosterior,
    HurdleBetaBinomialGroupPosteriorPredictive, HurdleBetaBinomialGroupResourceLimits,
    HurdleBetaBinomialGroupResult, HurdleBetaBinomialGroupSpec,
    HurdleBetaBinomialGroupWorkerRequest, HurdleBetaBinomialGroupWorkerResult,
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
pub use meta_analysis::{
    MetaAnalysisFit, MetaAnalysisInputIdentity, MetaAnalysisSpec, MetaAnalysisWorkerRequest,
    MetaAnalysisWorkerResult, SiteEstimate,
};
pub use model_comparison::{
    compare_psis_loo_models, BayesianModelComparisonResult, ComparedModel, PairwiseElpdDifference,
    PsisLooComparisonInput,
};
pub use multi_output_gp::{
    MultiOutputGpFit, MultiOutputGpInputIdentity, MultiOutputGpObservation, MultiOutputGpSpec,
    MultiOutputGpWorkerRequest, MultiOutputGpWorkerResult,
};
pub use multitype::{
    multitype_papangelou, MultitypeBaseline, MultitypeContribution, MultitypeError,
    MultitypeInteraction, MultitypePapangelouResult, MultitypePapangelouSpec, MultitypePoint,
};
pub use nngp::{
    build_nngp, full_gp_log_density, nngp_log_density, NngpError, NngpObservation, NngpPlan,
    NngpSpec,
};
pub use nonproportional_ordinal_group_site::{
    NonproportionalOrdinalGroupSiteInputIdentity, NonproportionalOrdinalGroupSiteModelIr,
    NonproportionalOrdinalGroupSitePosterior, NonproportionalOrdinalGroupSitePosteriorPredictive,
    NonproportionalOrdinalGroupSiteResources, NonproportionalOrdinalGroupSiteResult,
    NonproportionalOrdinalGroupSiteSpec, NonproportionalOrdinalGroupSiteWorkerRequest,
    NonproportionalOrdinalGroupSiteWorkerResult, NonproportionalOrdinalSitePosterior,
    OrdinalThresholdGroupEffect,
};
pub use ordinal_group::{
    OrdinalCutpointPosterior, OrdinalGroupInputIdentity, OrdinalGroupModelIr,
    OrdinalGroupPatientData, OrdinalGroupPosterior, OrdinalGroupPosteriorPredictive,
    OrdinalGroupResourceLimits, OrdinalGroupResult, OrdinalGroupSpec, OrdinalGroupWorkerRequest,
    OrdinalGroupWorkerResult, OrdinalLevelPosterior, OrdinalLevelPredictive,
};
pub use ordinal_group_site_hierarchy::{
    OrdinalGroupSiteHierarchyInputIdentity, OrdinalGroupSiteHierarchyModelIr,
    OrdinalGroupSiteHierarchyPosterior, OrdinalGroupSiteHierarchyPosteriorPredictive,
    OrdinalGroupSiteHierarchyResourceLimits, OrdinalGroupSiteHierarchyResult,
    OrdinalGroupSiteHierarchySpec, OrdinalGroupSiteHierarchyWorkerRequest,
    OrdinalGroupSiteHierarchyWorkerResult, OrdinalGroupSitePatientData, OrdinalSitePosterior,
};
pub use ordinal_heldout::{
    ordinal_site_heldout_comparison, OrdinalSiteFoldScore, OrdinalSiteHeldoutComparison,
    OrdinalSiteHeldoutSpec,
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
    evaluate_normal_mean_prior_sensitivity, NormalMeanPriorSensitivityResult,
    NormalMeanPriorSensitivitySpec, NormalPriorAlternative, PriorSensitivityInputIdentity,
    PriorSensitivityModelIr, PriorSensitivityResourceLimits, PriorSensitivityResult,
    PriorSensitivitySummary, PriorSensitivityWorkerRequest, PriorSensitivityWorkerResult,
};
pub use psis_loo::{
    PointwiseLogLikelihoodDraw, PsisLooInputIdentity, PsisLooPointwise, PsisLooResourceLimits,
    PsisLooResult, PsisLooSpec, PsisLooWorkerRequest, PsisLooWorkerResult,
};
pub use sparse_car::{
    fit_sparse_car, SparseCarDiagnostics, SparseCarError, SparseCarObservation,
    SparseCarPredictive, SparseCarPredictiveCheck, SparseCarRegionSummary, SparseCarResult,
    SparseCarSpec,
};

pub use car::{
    car_density, CarDensityError, CarDensityResult, CarMode, CarSpec, IslandPolicy,
    RegionFieldValue,
};
pub use distance_to_resource::{
    fit_distance_to_resource_model, DerivedResourceDistance, DistanceOutcomeObservation,
    DistancePosterior, DistancePosteriorPredictive, DistancePrediction, DistanceResponsePoint,
    DistanceToResourceFit, DistanceToResourceModel, DistanceToResourceSpec,
    GaussianPosteriorSummary, PatientPredictiveCheck, ResourcePredictiveCheck, ResourceSegment,
};
pub use model::{
    is_worker_timeout, json_sha256, sha256_hex, BackendContract, DiagnosticPolicy,
    NormalMeanModelIr, NormalMeanSpec, NormalMeanWorkerRequest, NutsSamplingSpec,
    WorkerResourceLimits, MAXIMUM_WORKER_TIMEOUT_SECONDS,
};
pub use negative_binomial_hierarchy::{
    NegativeBinomialHierarchyDesign, NegativeBinomialHierarchyModel,
    NegativeBinomialHierarchyObservation, NegativeBinomialHierarchyPatientPosterior,
    NegativeBinomialHierarchyPatientPredictive, NegativeBinomialHierarchyPosterior,
    NegativeBinomialHierarchyPosteriorPredictive, NegativeBinomialHierarchyPriorPatientRate,
    NegativeBinomialHierarchyPriorPredictive, NegativeBinomialHierarchyPriors,
    NegativeBinomialHierarchyResult, NegativeBinomialHierarchyRoiPosterior,
    NegativeBinomialHierarchySlidePosterior, NegativeBinomialHierarchySpec,
    NegativeBinomialHierarchyWorkerRequest,
};
pub use replicated::{
    replicated_hierarchical_lgcp, ReplicatedError, ReplicatedFieldPolicy, ReplicatedLgcpModel,
    ReplicatedLgcpModelIr, ReplicatedLgcpPattern, ReplicatedLgcpSpec,
};
pub use result::{
    FitState, McseScalarSummary, NormalMeanFit, NormalMeanInputIdentity, NormalMeanPosterior,
    NormalMeanPosteriorPredictive, PosteriorDiagnostics, SamplingSummary, ScalarSummary,
    WorkerBackend, WorkerResult,
};
pub use sar::{
    sar_gaussian_log_likelihood, SarError, SarImpact, SarLikelihoodResult, SarModelType, SarSpec,
};
pub use sar_fit::{
    SarCoefficientSummary, SarFitInputIdentity, SarFitModelIr, SarFitPosterior, SarFitResult,
    SarFitSpec, SarFitWorkerRequest, SarFitWorkerResult, SarImpactSummary, SarPosteriorPredictive,
};
pub use sbc::{
    NormalMeanSbcModelIr, NormalMeanSbcResult, NormalMeanSbcSpec, NormalMeanSbcWorkerRequest,
    NormalMeanSbcWorkerResult, SbcCalibrationPolicy, SbcControls, SbcDiagnostics, SbcExecutionSpec,
    SbcFailure, SbcParameterDiagnostics, SbcReplicate, SbcResourceLimits,
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
    StudentTHierarchyAgreementResult, StudentTHierarchyBackendSummary,
};
pub use student_t_hierarchy_sbc::{
    NumpyroStudentTSbcWorkerRequest, NumpyroStudentTSbcWorkerResult, StudentTSbcDiagnostics,
    StudentTSbcReplicate, StudentTSbcResourceLimits, StudentTSbcResult,
};
pub use student_t_hierarchy_sensitivity::{
    StudentTHierarchySensitivityResult, StudentTSensitivityScenario,
    StudentTSensitivityScenarioResult, StudentTSensitivityScenarioRun,
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

pub use agreement_status::AgreementStatus;
pub use marklab_core::backend::{ClaimStatus, WorkerError as BayesError};
pub use sensitivity_status::SensitivityStatus;
