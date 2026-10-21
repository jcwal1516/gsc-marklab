//! Spatial embedding, kernel, cross-modal, and predictive complementarity analyses.

mod complementarity;
mod cross_modal;
mod embedding_envelope;
mod embedding_factor;
mod embedding_kernel;
mod embedding_spatial;
mod multiscale_kernel;

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

pub use multiscale_kernel::{
    multiscale_embedding_kernel, MultiscaleBaseKernel, MultiscaleEmbeddingKernelResult,
    MultiscaleEmbeddingKernelSpec, MultiscaleEmbeddingSummary, MultiscaleKernelError,
    MultiscaleKernelScaleResult, MultiscaleKernelSensitivity, MultiscaleKernelWeight,
};
