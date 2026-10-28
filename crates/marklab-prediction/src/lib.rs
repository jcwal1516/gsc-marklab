#![forbid(unsafe_code)]

mod grouped_conformal;
mod kernel_fusion;
mod late_fusion;
mod logistic;
mod logistic_cv;
mod mixture_of_experts;
mod retrieval;

pub use grouped_conformal::{
    CoverageRow, GroupedConformalModel, GroupedConformalPatient, GroupedConformalPrediction,
    GroupedConformalResources, GroupedConformalSpec, GroupedConformalWorkerRequest,
    GroupedConformalWorkerResult, GroupedCoverage,
};
pub use kernel_fusion::{
    learn_multimodal_kernels, KernelFusionError, KernelFusionFoldResult, KernelFusionKernel,
    KernelFusionLimits, KernelFusionModality, KernelFusionModelResult, KernelFusionPatient,
    KernelFusionPrediction, KernelFusionResourceSummary, KernelFusionResult, KernelFusionSpec,
};
pub use late_fusion::{
    LateFusionCalibrator, LateFusionMetric, LateFusionModel, LateFusionPatient,
    LateFusionPrediction, LateFusionResources, LateFusionSpec, LateFusionWorkerRequest,
    LateFusionWorkerResult, MissingScenario, ModalityAblation,
};
pub use logistic::{
    ClassWeight, LogisticError, LogisticModel, LogisticProblem, MAXIMUM_LOGISTIC_DIMENSION,
    MAXIMUM_LOGISTIC_VALUES,
};
pub use logistic_cv::{
    area_under_roc, average_precision, cross_validate_logistic, LogisticCvFold,
    LogisticCvPrediction, LogisticCvResult, LogisticCvSpec, MAXIMUM_LOGISTIC_C_VALUES,
    MAXIMUM_LOGISTIC_FOLDS,
};
pub use mixture_of_experts::{
    MixtureCalibrator, MixtureGateModel, MixtureMetrics, MixtureOfExpertsPatient,
    MixtureOfExpertsResources, MixtureOfExpertsSpec, MixtureOfExpertsWorkerRequest,
    MixtureOfExpertsWorkerResult, MixturePrediction,
};
pub use retrieval::{
    build_region_retrieval_index, retrieve_analogous_regions, QueryRegion, RegionRetrievalError,
    RegionRetrievalIndexArtifact, RegionRetrievalMatch, RegionRetrievalResult,
    RetrievalLeakagePolicy, TrainingRegion,
};
