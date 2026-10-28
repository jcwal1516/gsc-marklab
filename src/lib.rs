#![forbid(unsafe_code)]
//! Supported API for marked-pattern, multimodal, and bounded WSI analysis.
//!
//! The compatibility contract is what this crate root re-exports: the core analysis types
//! directly, and each method family through a public module ([`spatial`], [`marks`],
//! [`autocorrelation`], [`compartments`], [`embeddings`], [`workflow`], [`pathology`] and
//! [`backend`]). Algorithm and orchestration modules remain private.

#[cfg(all(test, feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
#[global_allocator]
static TEST_ALLOCATOR: dhat::Alloc = dhat::Alloc;

#[cfg(test)]
mod algorithm_tests;
mod api;
mod arbitrary_window_ipp;
mod binary_nucleus_area_contrast;
mod categorical_cooccurrence;
mod categorical_enrichment;
mod categorical_input;
mod categorical_neighborhood_mixing;
mod categorical_neighborhood_mixing_workflow;
mod categorical_pair;
mod categorical_pair_workflow;
mod categorical_support_mixing;
mod categorical_support_mixing_workflow;
mod cell_embedding_cross_covariance;
mod cell_embedding_mark;
mod cell_embedding_mark_workflow;
mod cell_embedding_nucleus_area;
mod cell_embedding_probability;
mod classical;
mod classical_workflow;
#[cfg(feature = "cli")]
#[doc(hidden)]
pub mod cli;
mod common;
mod comparison;
mod compartment_cell_mixing;
mod compartment_cell_mixing_workflow;
mod compartment_contact;
mod compartment_contact_workflow;
mod compartment_fragmentation;
mod compartment_fragmentation_workflow;
mod compartment_interface;
mod compartment_interface_workflow;
mod config;
#[cfg(feature = "parquet")]
mod contained_patch_nucleus_area_contrast;
mod continuous_mark_correlation;
mod continuous_mark_correlation_workflow;
mod cross_pair_correlation;
mod data;
mod diagnostics;
mod edge_corrected;
mod errors;
#[doc(hidden)]
pub mod exact_float_json;
mod geom;
mod inference;
mod inhomogeneous_spatial;
mod io;
mod isotropic_pair_correlation;
mod isotropic_pair_correlation_workflow;
mod isotropic_spatial;
mod isotropic_spatial_workflow;
mod local_multivariate_moran;
mod mark_pair_plan;
mod mark_weighted_k;
mod mark_weighted_k_workflow;
mod marked_prepost_dag;
mod marked_workflow;
mod measurement_status_wire;
mod memory_budget;
mod multimodal;
mod multiplex_study;
mod multiscale_residual;
mod nearest_space;
mod nearest_space_workflow;
mod neighborhood;
mod observation_window_artifact;
mod ordinal_composition;
mod ordinal_composition_workflow;
mod output;
mod pair_correlation;
mod pair_correlation_workflow;
mod pathology_architecture;
mod pathology_maps;
mod pathology_study;
mod periodogram;
mod permutation;
mod prepost;
mod probability_pair;
mod probability_pair_workflow;
mod python_backend;
mod qc;
mod registration;
mod result_vocabulary;
mod scalar_mark;
mod scalar_variogram;
mod scalar_variogram_workflow;
mod soft_class_composition;
mod soft_class_composition_workflow;
mod soft_multiscale_neighborhood;
mod soft_multiscale_neighborhood_workflow;
mod soft_neighborhood_composition;
mod soft_neighborhood_composition_workflow;
mod soft_pair_mixing;
mod soft_pair_mixing_workflow;
pub mod space;
mod spatial_autocorrelation;
mod spatial_autocorrelation_workflow;
mod spectra;
mod structure_descriptors;
#[cfg(feature = "cli")]
mod synthetic_smoke;
mod top1_pair_mixing_bounds;
mod top1_pair_mixing_bounds_workflow;
mod translation_pair_correlation;
mod translation_pair_correlation_workflow;
mod translation_spatial;
mod translation_spatial_workflow;
mod workflow_codec;
mod workflow_node;
#[cfg(feature = "wsi")]
mod wsi;

pub use api::{AnalysisEngine, DeclaredMarkedAnalysisRun, MarkedAnalysisRun};
pub use config::{
    AnalysisConfig, AnalysisConfigSection, ComparisonSection, ComponentMode, CurveMargins,
    DiagnosticsSection, InferenceSection, MultiscaleResidualSection, NeighborhoodNullModel,
    NeighborhoodSection, OutputSection, PerformanceSection, PeriodogramSection, PermutationSection,
    PermutationStratum, RegistrationSection, RegistrationTransform, SpectrumSection, ThreadSetting,
    ValidationSection,
};
pub use data::{Pattern, PatternMeta, TumorWindow};
pub use errors::{MarklabError, Result};
pub use geom::mask::TumorMask;
pub use geom::window::{
    ObservationWindow2D, ObservationWindowDescriptor, ObservationWindowError,
    ObservationWindowLimits,
};
pub use io::{PatternLoadDiagnostics, PatternLoadResult, PatternLoader};
pub use marklab_data::{
    BlockId, CellId, CohortHierarchy, CoordinateFrame, CoordinateFrameId, CoordinateRegistry,
    CoordinateSpace, CoordinateUnit, CoreId, FrameTransform, HierarchyId, HierarchyKind,
    HierarchyNode, ImageCoordinateConvention, MeasurementStatus, PatchId, PatientId, RegionId,
    RepeatedMeasureSet, ReplicationRole, SectionId, SerialSectionSeries, SiteId, SlideId,
    SpatialAxis, SpecimenId, TimepointId, TransformId, TransformMatrix, UncertaintyId,
    UncertaintyReference,
};
pub use multimodal::{
    CellExtrapolationRecord, CellSection, FusedCell, HeCell, IhcCell, LandmarkHullAvailability,
    MultimodalAnalysisRun, MultimodalEngine, MultimodalInput, NullModelSensitivityResult,
    RegistrationExtrapolation, RegistrationResidual,
};
pub use neighborhood::graph::{SpatialEdge, SpatialGraph};
pub use output::{
    AnalysisResult, AnalysisSection, AnalysisStatus, AnisotropySummary, ArtifactStatus,
    BetaPosteriorGroupSummary, BetaPosteriorSummary, ComponentAnalysisSummary,
    ComponentModeSelection, CrossInteractionCurve, CrossInteractionPoint,
    CurveComparisonAvailability, CurveComparisonMethod, CurveComparisonResult, DiagnosticsResult,
    EnrichmentStatisticUnavailableReason, FunctionalSummary, FusedCellSummary,
    GraphSmoothingLabelPairSummary, GraphSmoothingSummary, Interpretation, InterpretationClass,
    LabelFraction, MarkPairCovariancePoint, MarkedPatternResult, MultimodalResult,
    MultiscaleResidualSummary, NeighborhoodEnrichmentResult, NeighborhoodTerritory, OutputManifest,
    OutputWriter, PrePostResult, PrimaryEndpoint, PrimaryEndpointKind, Provenance, QcSummary,
    RegistrationSummary, ResidualTerritory, ResolvedComponentMode, ResultDocument, ScaleEnergyBand,
    ScaleEnergyPoint, SpectrumConfoundingConclusion, SpectrumNullInferenceSummary,
    SpectrumNullModel, SpectrumNullSensitivitySummary, SpectrumPoint, SpectrumSummary, StatusFlag,
    TerritoryPrePostSummary, TerritoryProfile, TimingStage, WindowSummary, XiPeakShell,
    XiResolution, RESULT_FORMAT_VERSION,
};
pub use prepost::{
    compare_declared_marked_prepost, compare_declared_marked_prevalence, compare_marked_prepost,
    compare_multimodal_prepost, compare_multimodal_prepost_with_margin, DeclaredMarkedPrePostError,
    DeclaredMarkedPrePostResult, DeclaredMarkedPrevalenceChange, DeclaredMarkedPrevalenceStatus,
};
pub use registration::{
    landmarks::LandmarkPair,
    transform::{Transform2D, TransformKind},
};
#[cfg(feature = "wsi")]
pub use wsi::{
    PlaneSelection, RegionRequest, RgbaRegion, SlideLevelMetadata, SlideMetadata, SlideOpenOptions,
    SlideReader, SlideSampleType, SlideSceneMetadata, SlideSeriesMetadata,
};

/// Point-pattern statistics: K/L functions, pair correlation, cross-type pair correlation,
/// nearest-neighbour spacing and inhomogeneous intensity models.
pub mod spatial {
    pub use crate::arbitrary_window_ipp::{
        arbitrary_window_ipp_log_likelihood, ArbitraryWindowIppError, ArbitraryWindowIppEvent,
        ArbitraryWindowIppLikelihoodResult, ArbitraryWindowIppLimits,
        ArbitraryWindowIppQuadratureNode, ArbitraryWindowIppSpec, ArbitraryWindowIppWindowSummary,
    };
    pub use crate::classical::{
        analyze_classical_spatial_pattern, ClassicalCacheStatus, ClassicalConfigurationSummary,
        ClassicalGeometrySummary, ClassicalInferenceSummary, ClassicalNullDesign,
        ClassicalNullModel, ClassicalRandomizationUnit, ClassicalSpatialConfig,
        ClassicalSpatialError, ClassicalSpatialLimits, ClassicalSpatialResult,
        ClassicalSpatialResultDocument, ClassicalSpatialStatus, ClassicalWindowSummary,
        ClassicalWorkflowIdentity, HomogeneousKlPoint, KlPointStatus, CLASSICAL_SPATIAL_FORMAT,
        CLASSICAL_SPATIAL_FORMAT_VERSION,
    };
    pub use crate::classical_workflow::ClassicalSpatialAnalysisNode;
    pub use crate::cross_pair_correlation::{
        categorical_cross_pair_correlation, isotropic_categorical_cross_pair_correlation,
        translation_categorical_cross_pair_correlation,
        CategoricalCrossPairCorrelationAnalysisNode, CategoricalCrossPairCorrelationConfig,
        CategoricalCrossPairCorrelationError, CategoricalCrossPairCorrelationInference,
        CategoricalCrossPairCorrelationPoint, CategoricalCrossPairCorrelationResult,
        IsotropicCategoricalCrossPairCorrelationAnalysisNode,
        IsotropicCategoricalCrossPairCorrelationConfig,
        IsotropicCategoricalCrossPairCorrelationPoint,
        IsotropicCategoricalCrossPairCorrelationResult,
        TranslationCategoricalCrossPairCorrelationAnalysisNode,
        TranslationCategoricalCrossPairCorrelationConfig,
        TranslationCategoricalCrossPairCorrelationPoint,
        TranslationCategoricalCrossPairCorrelationResult,
    };
    pub use crate::inhomogeneous_spatial::{
        analyze_inhomogeneous_pair_correlation, analyze_inhomogeneous_spatial_pattern,
        analyze_piecewise_compartment_pair_correlation,
        analyze_piecewise_compartment_spatial_pattern,
        analyze_selected_inhomogeneous_spatial_pattern,
        inhomogeneous_categorical_cross_pair_correlation, GaussianBandwidthCandidateScore,
        GaussianBandwidthSelectedSpatialAnalysisNode, GaussianBandwidthSelectionConfig,
        GaussianBandwidthSelectionLimits, GaussianBandwidthSelectionSummary,
        InhomogeneousCategoricalCrossEdgeCorrection, InhomogeneousCategoricalCrossEdgeWork,
        InhomogeneousCategoricalCrossPairCorrelationAnalysisNode,
        InhomogeneousCategoricalCrossPairCorrelationConfig,
        InhomogeneousCategoricalCrossPairCorrelationError,
        InhomogeneousCategoricalCrossPairCorrelationPoint,
        InhomogeneousCategoricalCrossPairCorrelationResult, InhomogeneousIntensityGridPoint,
        InhomogeneousIntensityPoint, InhomogeneousIntensitySummary,
        InhomogeneousPairCorrelationAnalysisNode, InhomogeneousPairCorrelationConfig,
        InhomogeneousPairCorrelationPoint, InhomogeneousPairCorrelationResult,
        InhomogeneousSpatialAnalysisNode, InhomogeneousSpatialConfig, InhomogeneousSpatialError,
        InhomogeneousSpatialInference, InhomogeneousSpatialLimits, InhomogeneousSpatialPoint,
        InhomogeneousSpatialPointStatus, InhomogeneousSpatialResult,
        PiecewiseCompartmentIntensityLevel, PiecewiseCompartmentIntensityPoint,
        PiecewiseCompartmentIntensitySummary, PiecewiseCompartmentPairCorrelationAnalysisNode,
        PiecewiseCompartmentPairCorrelationConfig, PiecewiseCompartmentPairCorrelationResult,
        PiecewiseCompartmentRole, PiecewiseCompartmentSpatialAnalysisNode,
        PiecewiseCompartmentSpatialConfig, PiecewiseCompartmentSpatialLimits,
        PiecewiseCompartmentSpatialResult, SelectedInhomogeneousSpatialResult,
    };
    pub use crate::isotropic_pair_correlation::{
        analyze_isotropic_pair_correlation, IsotropicPairCorrelationConfig,
        IsotropicPairCorrelationConfigurationSummary, IsotropicPairCorrelationGeometrySummary,
        IsotropicPairCorrelationPoint, IsotropicPairCorrelationResult,
        IsotropicPairCorrelationResultDocument, ISOTROPIC_PAIR_CORRELATION_FORMAT,
        ISOTROPIC_PAIR_CORRELATION_FORMAT_VERSION,
    };
    pub use crate::isotropic_pair_correlation_workflow::IsotropicPairCorrelationAnalysisNode;
    pub use crate::isotropic_spatial::{
        analyze_isotropic_spatial_pattern, IsotropicCacheStatus, IsotropicConfigurationSummary,
        IsotropicGeometrySummary, IsotropicKlPoint, IsotropicSpatialConfig, IsotropicSpatialError,
        IsotropicSpatialLimits, IsotropicSpatialResult, IsotropicSpatialResultDocument,
        IsotropicSpatialStatus, IsotropicWorkflowIdentity, ISOTROPIC_SPATIAL_FORMAT,
        ISOTROPIC_SPATIAL_FORMAT_VERSION,
    };
    pub use crate::isotropic_spatial_workflow::IsotropicSpatialAnalysisNode;
    pub use crate::nearest_space::{
        analyze_nearest_space_pattern, DistributionPointStatus, JPointStatus,
        NearestSpaceCacheStatus, NearestSpaceComponentInference, NearestSpaceConfig,
        NearestSpaceConfigurationSummary, NearestSpaceError, NearestSpaceGeometrySummary,
        NearestSpaceInferenceSummary, NearestSpaceLimits, NearestSpacePoint,
        NearestSpaceProbeSummary, NearestSpaceResult, NearestSpaceResultDocument,
        NearestSpaceStatus, NearestSpaceWorkflowIdentity, NEAREST_SPACE_FORMAT,
        NEAREST_SPACE_FORMAT_VERSION,
    };
    pub use crate::nearest_space_workflow::NearestSpaceAnalysisNode;
    pub use crate::pair_correlation::{
        homogeneous_pair_correlation, HomogeneousPairCorrelationConfig,
        HomogeneousPairCorrelationError, HomogeneousPairCorrelationInference,
        HomogeneousPairCorrelationResult, PairCorrelationKernel, PairCorrelationPoint,
        PairCorrelationPointStatus,
    };
    pub use crate::pair_correlation_workflow::HomogeneousPairCorrelationAnalysisNode;
    pub use crate::result_vocabulary::{
        ArcMeasureOwner, BandwidthSelectionMethod, BandwidthSelectionObjective, BandwidthTieBreak,
        BoundaryDistanceOwner, CompartmentInterfaceEventPolicy, DuplicatePointPolicy,
        EdgeCorrection, FiniteResultPolicy, InhomogeneousNullModel, IntensityBoundaryCorrection,
        IntensityCrossFit, IntensityEstimator, IntensityKernel, IppClaimStatus, IppStatisticalUnit,
        LikelihoodNullModel, NearestNeighborOwner, OverlapMeasureOwner, PairTraversal,
        ProbeSamplingMethod, SpatialExecutionMode, SpatialRandomizationUnit, WindowCoordinateFrame,
        WindowCoordinateUnit, WindowMembershipPolicy,
    };
    pub use crate::translation_pair_correlation::{
        analyze_translation_pair_correlation, TranslationPairCorrelationConfig,
        TranslationPairCorrelationConfigurationSummary, TranslationPairCorrelationGeometrySummary,
        TranslationPairCorrelationPoint, TranslationPairCorrelationResult,
        TranslationPairCorrelationResultDocument, TRANSLATION_PAIR_CORRELATION_FORMAT,
        TRANSLATION_PAIR_CORRELATION_FORMAT_VERSION,
    };
    pub use crate::translation_pair_correlation_workflow::TranslationPairCorrelationAnalysisNode;
    pub use crate::translation_spatial::{
        analyze_translation_spatial_pattern, TranslationCacheStatus,
        TranslationConfigurationSummary, TranslationGeometrySummary, TranslationKlPoint,
        TranslationSpatialConfig, TranslationSpatialError, TranslationSpatialLimits,
        TranslationSpatialResult, TranslationSpatialResultDocument, TranslationSpatialStatus,
        TranslationWorkflowIdentity, TRANSLATION_SPATIAL_FORMAT,
        TRANSLATION_SPATIAL_FORMAT_VERSION,
    };
    pub use crate::translation_spatial_workflow::TranslationSpatialAnalysisNode;
}

/// Point-pattern structure descriptors at the Research tier: Morisita quadrat dispersion,
/// ordinary kriging with leave-one-block-out checks, bond-orientational order, box-counting
/// dimension with lacunarity of a declared mask, the Mapper graph of cell features and the
/// Landy–Szalay two-point correlation, and the structure factor with hyperuniformity indices.
pub mod structure {
    pub use crate::structure_descriptors::bond_order::{
        bond_orientational_order_from_json, BondOrientationalOrderResult, CellOrder, NeighborRule,
        BOND_ORIENTATIONAL_ORDER_FORMAT,
    };
    pub use crate::structure_descriptors::hyperuniformity::{
        hyperuniformity_from_json, HyperuniformityResult, Rectangle as HyperuniformityWindow,
        WavenumberBin, WavevectorValue, HYPERUNIFORMITY_FORMAT,
    };
    pub use crate::structure_descriptors::kriging::{
        ordinary_kriging_from_json, BlockCrossValidation, BlockDesign, HeldOutPrediction,
        HeldOutSummary, KrigingPrediction, Neighborhood, OrdinaryKrigingResult, Variogram,
        VariogramModel, ORDINARY_KRIGING_FORMAT,
    };
    pub use crate::structure_descriptors::lacunarity::{
        lacunarity_fractal_dimension_from_json, BoxScale, LacunarityFractalDimensionResult,
        LACUNARITY_FRACTAL_DIMENSION_FORMAT,
    };
    pub use crate::structure_descriptors::landy_szalay::{
        landy_szalay_from_json, CorrelationBin, LandySzalayResult, LANDY_SZALAY_FORMAT,
    };
    pub use crate::structure_descriptors::mapper::{
        mapper_graph_from_json, Cover as MapperCover, LensInterval, MapperEdge, MapperGraphResult,
        MapperNode, MAPPER_GRAPH_FORMAT,
    };
    pub use crate::structure_descriptors::morisita::{
        morisita_dispersion_from_json, MorisitaDispersionResult, Quadrat, QuadratCount,
        MORISITA_DISPERSION_FORMAT,
    };
    pub use crate::structure_descriptors::nematic::{
        nematic_orientation_from_json, DefectDetectionConfig, DefectStatus, LocalNematicConfig,
        LocalNematicOrder, NematicDefectPlaquette, NematicEstimate, NematicEstimateStatus,
        NematicFieldConfig, NematicFieldSample, NematicLimits, NematicOrientationResult,
        NematicTensor, NEMATIC_ORIENTATION_FORMAT,
    };
}

/// Mark statistics: categorical, continuous, ordinal, probability and soft-class marks,
/// mark-weighted K, neighbourhood composition and mutual information between two marks.
pub mod marks {
    pub use crate::categorical_cooccurrence::{
        categorical_cooccurrence, categorical_cooccurrence_from_json, CategoricalCooccurrenceCell,
        CategoricalCooccurrenceChannel, CategoricalCooccurrenceEntry, CategoricalCooccurrenceError,
        CategoricalCooccurrenceLimits, CategoricalCooccurrenceMatrix,
        CategoricalCooccurrenceResult, CategoricalCooccurrenceStatus,
        CATEGORICAL_COOCCURRENCE_FORMAT, CATEGORICAL_COOCCURRENCE_VERSION,
    };
    pub use crate::categorical_enrichment::{
        categorical_enrichment_from_json, CategoricalEnrichmentPairResult,
        CategoricalEnrichmentResult,
    };
    pub use crate::categorical_neighborhood_mixing::{
        categorical_neighborhood_mixing, CategoricalNeighborhoodClassSummary,
        CategoricalNeighborhoodMixingConfig, CategoricalNeighborhoodMixingError,
        CategoricalNeighborhoodMixingLimits, CategoricalNeighborhoodMixingResult,
    };
    pub use crate::categorical_neighborhood_mixing_workflow::CategoricalNeighborhoodMixingAnalysisNode;
    pub use crate::categorical_pair::{
        categorical_mark_connection_cross_k, CategoricalPairComponentInference,
        CategoricalPairConfig, CategoricalPairError, CategoricalPairGeometrySummary,
        CategoricalPairInferenceSummary, CategoricalPairLimits, CategoricalPairPoint,
        CategoricalPairPointStatus, CategoricalPairResult,
    };
    pub use crate::categorical_pair_workflow::CategoricalPairAnalysisNode;
    pub use crate::categorical_support_mixing::{
        categorical_support_mixing, CategoricalSupportMixingCell, CategoricalSupportMixingConfig,
        CategoricalSupportMixingError, CategoricalSupportMixingLimits,
        CategoricalSupportMixingResult,
    };
    pub use crate::categorical_support_mixing_workflow::CategoricalSupportMixingAnalysisNode;
    pub use crate::continuous_mark_correlation::{
        continuous_mark_correlation, ContinuousMarkCorrelationComponentInference,
        ContinuousMarkCorrelationConfig, ContinuousMarkCorrelationError,
        ContinuousMarkCorrelationGeometrySummary, ContinuousMarkCorrelationInferenceSummary,
        ContinuousMarkCorrelationLimits, ContinuousMarkCorrelationPoint,
        ContinuousMarkCorrelationPointStatus, ContinuousMarkCorrelationResult,
    };
    pub use crate::continuous_mark_correlation_workflow::ContinuousMarkCorrelationAnalysisNode;
    pub use crate::mark_weighted_k::{
        continuous_mark_weighted_k, MarkWeightedKComponentInference, MarkWeightedKConfig,
        MarkWeightedKError, MarkWeightedKGeometrySummary, MarkWeightedKInferenceSummary,
        MarkWeightedKLimits, MarkWeightedKPoint, MarkWeightedKPointStatus, MarkWeightedKResult,
    };
    pub use crate::mark_weighted_k_workflow::MarkWeightedKAnalysisNode;
    pub use crate::ordinal_composition::{
        ordinal_class_composition, OrdinalClassCompositionConfig, OrdinalClassCompositionError,
        OrdinalClassCompositionLimits, OrdinalClassCompositionResult,
    };
    pub use crate::ordinal_composition_workflow::OrdinalClassCompositionAnalysisNode;
    pub use crate::probability_pair::{
        probability_mark_connection, ProbabilityPairComponentInference, ProbabilityPairConfig,
        ProbabilityPairError, ProbabilityPairGeometrySummary, ProbabilityPairInferenceSummary,
        ProbabilityPairLimits, ProbabilityPairPoint, ProbabilityPairPointStatus,
        ProbabilityPairResult,
    };
    pub use crate::probability_pair_workflow::ProbabilityPairAnalysisNode;
    pub use crate::result_vocabulary::{
        EdgeCorrection, MarkNormalization, MarkNullModel, MarkPermutationUnit,
        MultiplicityCorrection, ProbabilityContributionMode, ProbabilityRowNullModel,
        WeightFunction,
    };
    pub use crate::scalar_mark::{
        AssayMarkDeclaration, AssayMarkValues, BinaryMarkDeclaration, BinaryMarkOrigin,
        DeclaredMarkUse, DeclaredScalarIdentity, DeclaredScalarInputError,
        DeclaredScalarPatternInput, HistologicCompartmentMarkDeclaration, MarkTable,
        MissingnessPolicy, NucleusAreaUm2MarkDeclaration, OrdinalMarkDeclaration,
        ProbabilityMarkDeclaration, ProbabilitySimplexMarkDeclaration,
        ProbabilityThresholdComparator, ScalarMarkColumn, ScalarMarkId, ScalarMarkModality,
        ScalarMarkUnit, ScalarMarkValueKind, VectorArtifactRefMarkDeclaration,
    };
    pub use crate::soft_class_composition::{
        soft_class_composition, SoftClassCompositionClass, SoftClassCompositionError,
        SoftClassCompositionLimits, SoftClassCompositionResult,
    };
    pub use crate::soft_class_composition_workflow::SoftClassCompositionAnalysisNode;
    pub use crate::soft_multiscale_neighborhood::{
        soft_multiscale_neighborhood_composition, SoftMultiscaleNeighborhoodConfig,
        SoftMultiscaleNeighborhoodError, SoftMultiscaleNeighborhoodLimits,
        SoftMultiscaleNeighborhoodResult, SoftMultiscaleNeighborhoodScale,
    };
    pub use crate::soft_multiscale_neighborhood_workflow::SoftMultiscaleNeighborhoodAnalysisNode;
    pub use crate::soft_neighborhood_composition::{
        soft_neighborhood_composition, SoftNeighborhoodCompositionConfig,
        SoftNeighborhoodCompositionError, SoftNeighborhoodCompositionLimits,
        SoftNeighborhoodCompositionResult, SoftNeighborhoodCompositionRow,
    };
    pub use crate::soft_neighborhood_composition_workflow::SoftNeighborhoodCompositionAnalysisNode;
    pub use crate::soft_pair_mixing::{
        soft_pair_mixing, SoftPairMixingCell, SoftPairMixingConfig, SoftPairMixingError,
        SoftPairMixingLimits, SoftPairMixingResult,
    };
    pub use crate::soft_pair_mixing_workflow::SoftPairMixingAnalysisNode;
    pub use crate::structure_descriptors::mutual_information::{
        mutual_information_from_json, MutualInformationResult, NullModel as MutualInformationNull,
        MUTUAL_INFORMATION_FORMAT,
    };
    pub use crate::structure_descriptors::partial_information::{
        partial_information_decomposition_from_json, DiscreteInformationVariable,
        PartialInformationDecompositionResult, TargetSpecificInformation,
        PARTIAL_INFORMATION_FORMAT,
    };
    pub use crate::top1_pair_mixing_bounds::{
        top1_pair_mixing_bounds, Top1PairMixingBoundCell, Top1PairMixingBoundsConfig,
        Top1PairMixingBoundsError, Top1PairMixingBoundsLimits, Top1PairMixingBoundsResult,
    };
    pub use crate::top1_pair_mixing_bounds_workflow::Top1PairMixingBoundsAnalysisNode;
    pub use marklab_cohort::InferenceAlternative;
}

/// Spatial autocorrelation of marks: global Moran and Geary, local multivariate Moran,
/// and scalar variograms.
pub mod autocorrelation {
    pub use crate::local_multivariate_moran::{
        local_multivariate_moran_permutation, LocalMultivariateMoranError,
        LocalMultivariateMoranLimits, LocalMultivariateMoranLocation, LocalMultivariateMoranPoint,
        LocalMultivariateMoranResult,
    };
    pub use crate::result_vocabulary::{
        EdgeCorrection, MoranNullModel, MoranPopulationClaim, MoranStatisticalUnit,
        MultiplicityCorrection, VariogramInferenceStatus, VariogramMultiplicity,
    };
    pub use crate::scalar_variogram::{
        scalar_semivariogram, scalar_semivariogram_permutation, ScalarVariogramBin,
        ScalarVariogramConditioning, ScalarVariogramEnvelopeRow, ScalarVariogramError,
        ScalarVariogramInferenceDesign, ScalarVariogramInferenceLimits,
        ScalarVariogramInferenceResult, ScalarVariogramLimits, ScalarVariogramResult,
        ScalarVariogramRow,
    };
    pub use crate::scalar_variogram_workflow::ScalarVariogramAnalysisNode;
    pub use crate::spatial_autocorrelation::{
        global_geary_permutation, global_moran_permutation, summarize_assay_spatial,
        AssaySpatialInput, AssaySpatialLimits, AssaySpatialOutcome, AssaySpatialSummary,
        AssaySpatialUnavailable, GlobalGearyAlternative, GlobalGearyDesign, GlobalGearyError,
        GlobalGearyLimits, GlobalGearyResult, GlobalMoranAlternative, GlobalMoranConditioning,
        GlobalMoranDesign, GlobalMoranError, GlobalMoranLimits, GlobalMoranResult,
        GlobalMoranWeightPolicy,
    };
    pub use crate::spatial_autocorrelation_workflow::{
        GlobalMoranAnalysisNode, GlobalMoranPrePostNode, GlobalMoranPrePostResult,
    };
}

/// Tissue-compartment analyses: compartment partitions, cell mixing, contact,
/// fragmentation and interface profiles.
pub mod compartments {
    pub use crate::compartment_cell_mixing::{
        analyze_compartment_cell_mixing, CompartmentCellMixingConfig, CompartmentCellMixingError,
        CompartmentCellMixingLimits, CompartmentCellMixingResult, CompartmentCellMixingSummary,
    };
    pub use crate::compartment_cell_mixing_workflow::CompartmentCellMixingAnalysisNode;
    pub use crate::compartment_contact::{
        compartment_contact_fractions, CompartmentContactFraction, CompartmentContactResult,
    };
    pub use crate::compartment_contact_workflow::CompartmentContactAnalysisNode;
    pub use crate::compartment_fragmentation::{
        compartment_fragmentation, CompartmentFragmentationResult, CompartmentFragmentationSummary,
    };
    pub use crate::compartment_fragmentation_workflow::CompartmentFragmentationAnalysisNode;
    pub use crate::compartment_interface::{
        analyze_compartment_interface_profile, CompartmentInterfaceCell, CompartmentInterfaceError,
        CompartmentInterfaceLimits, CompartmentInterfaceProfile, CompartmentInterfaceSummary,
    };
    pub use crate::compartment_interface_workflow::CompartmentInterfaceAnalysisNode;
    pub use crate::geom::window::{
        BinaryCompartmentPartition2D, CompartmentPartitionDescriptor, CompartmentPartitionError,
        CompartmentPartitionLimits,
    };
}

/// Cell, patch, region and slide embedding artifacts and the analyses built on them.
pub mod embeddings {
    #[cfg(feature = "parquet")]
    pub use marklab_embeddings::{
        publish_columnar, read_columnar, verify_columnar, with_verified_columnar_reader, ArrowIpc,
        CellEmbeddingRowLinkEntity, CellEmbeddingRowLinkReadContext,
        CellEmbeddingRowLinkVerifyContext, CellEmbeddingTableEntity, CellEmbeddingTableReadContext,
        CellEmbeddingTableWriteContext, CellPatchAssignmentEntity, CellPatchAssignmentReadContext,
        CellPatchEdgeEntity, CellPatchEdgeReadContext, CellPatchReadContext,
        CellPatchVerifyContext, ColumnarEntity, ColumnarFormat, Parquet, PatchEmbeddingTableEntity,
        PatchEmbeddingTableReadContext, PatchEmbeddingTableVerifyContext, PatchFootprintEntity,
        PatchFootprintReadContext, PatchFootprintVerifyContext, PatchFootprintWriteContext,
        PatchOverlapEntity, PatchOverlapReadContext, PatchOverlapVerifyContext,
        PatchOverlapWriteContext, PatchRegionEntity, PatchRegionReadContext,
        PatchRegionVerifyContext, RegionEmbeddingTableEntity, RegionEmbeddingTableReadContext,
        SlideEmbeddingTableEntity, SlideEmbeddingTableReadContext, VerifiedColumnarReader,
    };

    pub use crate::binary_nucleus_area_contrast::{
        declared_binary_group_nucleus_area_contrast, DeclaredBinaryGroupNucleusAreaContrast,
        DeclaredBinaryGroupNucleusAreaContrastError, DeclaredBinaryGroupNucleusAreaContrastStatus,
    };
    pub use crate::cell_embedding_mark::{
        declared_binary_cell_embedding_centroid_discrepancy,
        DeclaredBinaryCellEmbeddingCentroidDiscrepancy,
        DeclaredBinaryCellEmbeddingCentroidDiscrepancyError,
        DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus,
        DeclaredBinaryCellEmbeddingGroupCounts,
    };
    pub use crate::cell_embedding_mark_workflow::DeclaredBinaryCellEmbeddingCentroidNode;
    pub use crate::cell_embedding_nucleus_area::{
        declared_nucleus_area_cell_embedding_cross_covariance_energy,
        DeclaredNucleusAreaCellEmbeddingCrossCovarianceEnergy,
        DeclaredNucleusAreaCellEmbeddingCrossCovarianceError,
        DeclaredNucleusAreaCellEmbeddingCrossCovarianceStatus,
    };
    pub use crate::cell_embedding_probability::{
        declared_probability_cell_embedding_cross_covariance_energy,
        DeclaredProbabilityCellEmbeddingCrossCovarianceEnergy,
        DeclaredProbabilityCellEmbeddingCrossCovarianceError,
        DeclaredProbabilityCellEmbeddingCrossCovarianceStatus,
    };
    #[cfg(feature = "parquet")]
    pub use crate::contained_patch_nucleus_area_contrast::{
        contained_patch_binary_nucleus_area_contrast, ContainedPatchBinaryNucleusAreaContrast,
        ContainedPatchBinaryNucleusAreaContrastError,
        ContainedPatchBinaryNucleusAreaContrastStatus,
    };
    #[cfg(feature = "parquet")]
    pub use marklab_embeddings::{
        contained_cell_patch_embedding_dispersion, ContainedCellPatchEmbeddingDispersion,
        ContainedCellPatchEmbeddingDispersionError, ContainedCellPatchEmbeddingDispersionStatus,
    };
    pub use marklab_embeddings::{
        finalize_region_embedding_table_from_patches, finalize_slide_embedding_table_from_patches,
        finalize_slide_embedding_table_from_regions, patch_overlap_embedding_dispersion,
        patch_region_embedding_dispersion, slide_embedding_aggregation_path_discrepancy,
        ArtifactAvailabilityFailure, CanonicalDecimal, CellEmbeddingArtifact,
        CellEmbeddingArtifactRole, CellEmbeddingBlock, CellEmbeddingExecutionProvenance,
        CellEmbeddingInputArtifacts, CellEmbeddingModelProvenance, CellEmbeddingProvenance,
        CellEmbeddingRow, CellEmbeddingRowLink, CellEmbeddingRowLinkEntry, CellEmbeddingTable,
        CellEmbeddingTensorContract, CellEmbeddingView, CellIdentityMap, CellIdentityMapEntry,
        CellPatchAnchor, CellPatchAssignment, CellPatchAssignmentMode, CellPatchAssignmentStatus,
        CellPatchContributor, CellPatchEdge, CellPatchLink, CellPatchLinkBindings,
        CellPatchLinkProducer, CellPatchWeight, CellVitCsvField, CellVitNpyMatrix,
        CellVitNpySummary, CsvFailure, DeclaredCellPatchAssignment,
        DerivedRegionEmbeddingTableCandidate, DerivedSlideEmbeddingTableCandidate,
        EffectiveReceptiveField, EmbeddingArtifactGraphError, EmbeddingDtype, EmbeddingEntityKind,
        EmbeddingError, EmbeddingFinalizationBudgets, EmbeddingNpyMatrix, EmbeddingNpySummary,
        EmbeddingQcSummary, EmbeddingSpatialContext, EmbeddingStatus, ExpectedCellSet,
        ExpectedPatchSet, ExpectedRegionSet, ExpectedSlideSet, ImportFailure, ManifestFailure,
        MultiscaleArtifactBinding, MultiscaleDirectPatchInputArtifacts,
        MultiscaleDirectPatchModelProvenance, MultiscaleEmbeddingArtifactGraphError,
        MultiscaleEmbeddingArtifactRole, MultiscaleEmbeddingDerivationContract,
        MultiscaleEmbeddingError, MultiscaleEmbeddingExecutionProvenance,
        MultiscaleEmbeddingProvenance, MultiscaleEmbeddingProvenanceVariant,
        MultiscaleEmbeddingQcSummary, MultiscaleEmbeddingSupport,
        MultiscaleEmbeddingSupportVariant, NpyFailure, NpyVersion, PatchBoundaryPolicy,
        PatchEmbeddingBlock, PatchEmbeddingContext, PatchEmbeddingInputNormalization,
        PatchEmbeddingRow, PatchEmbeddingSourceRowLink, PatchEmbeddingSourceRowLinkEntry,
        PatchEmbeddingTable, PatchEmbeddingView, PatchFootprint, PatchFootprintSet,
        PatchIdentityMap, PatchIdentityMapEntry, PatchNormalizationDecimal, PatchOverlapEdge,
        PatchOverlapEmbeddingDispersion, PatchOverlapEmbeddingDispersionError,
        PatchOverlapEmbeddingDispersionStatus, PatchOverlapGraph, PatchRegionAssessment,
        PatchRegionAssessmentBindings, PatchRegionDeclaration, PatchRegionEmbeddingDispersion,
        PatchRegionEmbeddingDispersionError, PatchRegionEmbeddingDispersionStatus, PatchRegionLink,
        PatchRegionRelation, PatchSourceEntityEntry, PatchSourceEntitySet, PositiveRational,
        ReconciliationFailure, RegionEmbeddingBlock, RegionEmbeddingRow, RegionEmbeddingTable,
        RegionEmbeddingView, SlideEmbeddingAggregationPathDiscrepancy,
        SlideEmbeddingAggregationPathDiscrepancyError,
        SlideEmbeddingAggregationPathDiscrepancyStatus, SlideEmbeddingBlock, SlideEmbeddingRow,
        SlideEmbeddingTable, SlideEmbeddingView, SourceBundleBudgets, SourceBundleError,
        SourceFileKind, SourceIoFailure, SourceIoOperation, VerifiedCellEmbeddingArtifactGraph,
        VerifiedCellEmbeddingRowLinkArtifact, VerifiedCellEmbeddingTableArtifact,
        VerifiedCellPatchAssignmentArtifact, VerifiedCellPatchEdgeArtifact,
        VerifiedCellPatchLinkArtifact, VerifiedDerivedRegionEmbeddingArtifactGraph,
        VerifiedDerivedSlideEmbeddingArtifactGraph, VerifiedDirectPatchEmbeddingArtifactGraph,
        VerifiedPatchEmbeddingSupportArtifact, VerifiedPatchEmbeddingTableArtifact,
        VerifiedPatchFootprintArtifact, VerifiedPatchOverlapArtifact,
        VerifiedPatchRegionLinkArtifact, VerifiedRegionEmbeddingSupportArtifact,
        VerifiedRegionEmbeddingTableArtifact, VerifiedSlideEmbeddingSupportArtifact,
        VerifiedSlideEmbeddingTableArtifact,
    };
    #[cfg(feature = "csv")]
    pub use marklab_embeddings::{
        import_cellvit_he_bundle_bytes, import_cellvit_he_bundle_from_store,
        import_cellvit_he_bundle_readers, CellVitCsvSummary, CellVitHeArtifactBindings,
        CellVitHeImportCandidate, CellVitHeImportRequest, ImportedCellVitHeBundle,
        MissingPromotionField, SourceBundleReconciler, SourceBundleReconciliation,
    };
    #[cfg(feature = "parquet")]
    pub use marklab_embeddings::{
        preflight_cell_embedding_row_link_arrow_bytes,
        preflight_cell_embedding_row_link_parquet_bytes,
        preflight_cell_embedding_table_arrow_bytes, preflight_cell_embedding_table_parquet_bytes,
        preflight_cell_patch_assignment_table_arrow_bytes,
        preflight_cell_patch_assignment_table_parquet_bytes,
        preflight_cell_patch_edge_table_arrow_bytes, preflight_cell_patch_edge_table_parquet_bytes,
        preflight_patch_footprint_set_arrow_bytes, preflight_patch_footprint_set_parquet_bytes,
        preflight_patch_overlap_graph_arrow_bytes, preflight_patch_overlap_graph_parquet_bytes,
        preflight_patch_region_link_arrow_bytes, preflight_patch_region_link_parquet_bytes,
        publish_cell_embedding_row_link_arrow, publish_cell_embedding_row_link_parquet,
        publish_cell_embedding_table_arrow, publish_cell_embedding_table_parquet,
        publish_cell_patch_assignment_table_arrow, publish_cell_patch_assignment_table_parquet,
        publish_cell_patch_edge_table_arrow, publish_cell_patch_edge_table_parquet,
        publish_patch_footprint_set_arrow, publish_patch_footprint_set_parquet,
        publish_patch_overlap_graph_arrow, publish_patch_overlap_graph_parquet,
        publish_patch_region_link_arrow, publish_patch_region_link_parquet,
        read_cell_embedding_table_arrow_bytes, read_cell_embedding_table_arrow_from_store,
        read_cell_embedding_table_parquet_bytes, read_cell_embedding_table_parquet_from_store,
        scan_cell_embedding_table_arrow_bytes, scan_cell_embedding_table_arrow_from_store,
        scan_cell_embedding_table_parquet_bytes, scan_cell_embedding_table_parquet_from_store,
        validate_cell_embedding_row_link_arrow_bytes,
        validate_cell_embedding_row_link_arrow_from_store,
        validate_cell_embedding_row_link_parquet_bytes,
        validate_cell_embedding_row_link_parquet_from_store,
        validate_cell_patch_assignment_table_arrow_bytes,
        validate_cell_patch_assignment_table_arrow_from_store,
        validate_cell_patch_assignment_table_parquet_bytes,
        validate_cell_patch_assignment_table_parquet_from_store,
        validate_cell_patch_edge_table_arrow_bytes,
        validate_cell_patch_edge_table_arrow_from_store,
        validate_cell_patch_edge_table_parquet_bytes,
        validate_cell_patch_edge_table_parquet_from_store,
        validate_patch_footprint_set_arrow_bytes, validate_patch_footprint_set_arrow_from_store,
        validate_patch_footprint_set_parquet_bytes,
        validate_patch_footprint_set_parquet_from_store, validate_patch_overlap_graph_arrow_bytes,
        validate_patch_overlap_graph_arrow_from_store, validate_patch_overlap_graph_parquet_bytes,
        validate_patch_overlap_graph_parquet_from_store, validate_patch_region_link_arrow_bytes,
        validate_patch_region_link_arrow_from_store, validate_patch_region_link_parquet_bytes,
        validate_patch_region_link_parquet_from_store, verify_cell_embedding_row_link_arrow_bytes,
        verify_cell_embedding_row_link_arrow_from_store,
        verify_cell_embedding_row_link_parquet_bytes,
        verify_cell_embedding_row_link_parquet_from_store, verify_cell_embedding_table_arrow_bytes,
        verify_cell_embedding_table_arrow_from_store, verify_cell_embedding_table_parquet_bytes,
        verify_cell_embedding_table_parquet_from_store,
        verify_cell_patch_assignment_table_arrow_bytes,
        verify_cell_patch_assignment_table_arrow_from_store,
        verify_cell_patch_assignment_table_parquet_bytes,
        verify_cell_patch_assignment_table_parquet_from_store,
        verify_cell_patch_edge_table_arrow_bytes, verify_cell_patch_edge_table_arrow_from_store,
        verify_cell_patch_edge_table_parquet_bytes,
        verify_cell_patch_edge_table_parquet_from_store, verify_patch_footprint_set_arrow_bytes,
        verify_patch_footprint_set_arrow_from_store, verify_patch_footprint_set_parquet_bytes,
        verify_patch_footprint_set_parquet_from_store, verify_patch_overlap_graph_arrow_bytes,
        verify_patch_overlap_graph_arrow_from_store, verify_patch_overlap_graph_parquet_bytes,
        verify_patch_overlap_graph_parquet_from_store, verify_patch_region_link_arrow_bytes,
        verify_patch_region_link_arrow_from_store, verify_patch_region_link_parquet_bytes,
        verify_patch_region_link_parquet_from_store, write_cell_embedding_row_link_arrow,
        write_cell_embedding_row_link_parquet, write_cell_embedding_table_arrow,
        write_cell_embedding_table_parquet, write_cell_patch_assignment_table_arrow,
        write_cell_patch_assignment_table_parquet, write_cell_patch_edge_table_arrow,
        write_cell_patch_edge_table_parquet, write_patch_footprint_set_arrow,
        write_patch_footprint_set_parquet, write_patch_overlap_graph_arrow,
        write_patch_overlap_graph_parquet, write_patch_region_link_arrow,
        write_patch_region_link_parquet, ArrowIpcFailure, CellEmbeddingArrowPreflight,
        CellEmbeddingParquetPreflight, CellEmbeddingRowLinkArrowPreflight,
        CellEmbeddingRowLinkParquetPreflight, CellEmbeddingTablePhysicalBindings,
        CellPatchAssignmentArrowPreflight, CellPatchAssignmentParquetPreflight,
        CellPatchEdgeArrowPreflight, CellPatchEdgeParquetPreflight,
        CellPatchInputArtifactGraphError, CellPatchInputArtifactRole, ColumnarError,
        ColumnarPublicationError, ColumnarWriteSummary, EmbeddingColumnarBudgets,
        MultiscaleEmbeddingTableReadBindings, ParquetFailure, PatchFootprintArrowPreflight,
        PatchFootprintParquetPreflight, PatchOverlapArrowPreflight, PatchOverlapParquetPreflight,
        PatchRegionArrowPreflight, PatchRegionInputArtifactGraphError,
        PatchRegionInputArtifactRole, PatchRegionParquetPreflight, RowLinkColumnarWriteSummary,
        SpatialColumnarWriteSummary, VerifiedCellPatchInputArtifactGraph,
        VerifiedPatchRegionInputArtifactGraph,
    };
    #[cfg(feature = "parquet")]
    pub use marklab_embeddings::{
        preflight_patch_embedding_table_arrow_bytes, preflight_patch_embedding_table_parquet_bytes,
        preflight_region_embedding_table_arrow_bytes,
        preflight_region_embedding_table_parquet_bytes,
        preflight_slide_embedding_table_arrow_bytes, preflight_slide_embedding_table_parquet_bytes,
        publish_patch_embedding_table_arrow, publish_patch_embedding_table_parquet,
        publish_region_embedding_table_arrow, publish_region_embedding_table_parquet,
        publish_slide_embedding_table_arrow, publish_slide_embedding_table_parquet,
        read_patch_embedding_table_arrow_bytes, read_patch_embedding_table_arrow_from_store,
        validate_patch_embedding_table_arrow_bytes,
        validate_patch_embedding_table_arrow_from_store,
        validate_patch_embedding_table_parquet_bytes,
        validate_patch_embedding_table_parquet_from_store,
        validate_region_embedding_table_arrow_bytes,
        validate_region_embedding_table_arrow_from_store,
        validate_region_embedding_table_parquet_bytes,
        validate_region_embedding_table_parquet_from_store,
        validate_slide_embedding_table_arrow_bytes,
        validate_slide_embedding_table_arrow_from_store,
        validate_slide_embedding_table_parquet_bytes,
        validate_slide_embedding_table_parquet_from_store,
        verify_patch_embedding_table_arrow_bytes, verify_patch_embedding_table_arrow_from_store,
        verify_patch_embedding_table_parquet_bytes,
        verify_patch_embedding_table_parquet_from_store, verify_region_embedding_table_arrow_bytes,
        verify_region_embedding_table_arrow_from_store,
        verify_region_embedding_table_parquet_bytes,
        verify_region_embedding_table_parquet_from_store, verify_slide_embedding_table_arrow_bytes,
        verify_slide_embedding_table_arrow_from_store, verify_slide_embedding_table_parquet_bytes,
        verify_slide_embedding_table_parquet_from_store, write_patch_embedding_table_arrow,
        write_patch_embedding_table_parquet, write_region_embedding_table_arrow,
        write_region_embedding_table_parquet, write_slide_embedding_table_arrow,
        write_slide_embedding_table_parquet, MultiscaleMatrixArrowPreflight,
        MultiscaleMatrixParquetPreflight,
    };
}

/// Workflow graphs, durable analysis nodes for every method, and the marked pre/post DAG.
pub mod workflow {
    pub use crate::marked_prepost_dag::{
        execute_marked_prepost_dag, plan_marked_prepost_dag, MarkedPrePostDagError,
        MarkedPrePostDagLimits, MarkedPrePostDagPlan, MarkedPrePostDagRun, MarkedPrePostDagTarget,
    };
    pub use crate::marked_workflow::{
        DeclaredMarkedAnalysisNode, DeclaredMarkedAnalysisResult, MarkedAnalysisNode,
        MarkedPrePostNode,
    };
    pub use marklab_project::{
        ArtifactCatalog, ArtifactCatalogError, ArtifactDraft, ArtifactId, ArtifactKey,
        ArtifactLocator, ArtifactPublication, ArtifactRecord, ArtifactRecordError, ArtifactRef,
        ArtifactSchema, ArtifactStoreError, ContentDigest, ContentDigestParseError,
        ContentDigestWriter, DurableCommitDisposition, DurableExecutionRequest, DurableOpenReport,
        DurableProject, DurableProjectError, DurableProjectLimits, DurableRecoveryAction,
        DurableReplay, LocalArtifactStore, MarklabProject, NativeRuntimeProvenance, ProjectError,
        PublicationDisposition, RecoveryIssue, RecoveryIssueReason, RecoveryReport, StoreId,
        SuccessfulRun, TableColumn, TableColumnType, TableFormat, TableManifest,
        TableManifestError, TableScalarType, VerifiedReaderError,
    };
    pub use marklab_workflow::{
        execute_algorithm, execute_algorithm_with_store, CacheKeyMaterial, CacheStatus,
        ExecuteAlgorithmError, LocalScheduler, NodeError, NodeId, NodeRun, NodeSpec,
        SchedulerLimits, WorkflowError, WorkflowGraph, WorkflowNode,
    };
}

/// Pathology map and architecture recipes, pathology studies and multiplex studies.
pub mod pathology {
    pub use crate::multiplex_study::{
        analyze_multiplex_study, execute_multiplex_study, publish_multiplex_study,
        MultiplexStudyResult, MultiplexStudyRun, MultiplexStudyTarget,
    };
    pub use crate::pathology_architecture::{
        analyze_pathology_architecture, publish_pathology_architecture, PathologyArchitectureResult,
    };
    pub use crate::pathology_maps::{
        analyze_pathology_maps, publish_pathology_maps, PathologyMapsResult,
    };
    pub use crate::pathology_study::{
        analyze_pathology_composition, analyze_pathology_scan, analyze_pathology_spatial_study,
        publish_pathology_composition, publish_pathology_scan, publish_pathology_spatial_study,
        PathologyCompositionResult, PathologyScanResult, PathologySpatialStudyResult,
    };
}

/// Locating the pinned Python backend environment and its assets.
pub mod backend {
    pub use crate::python_backend::{
        python_backend_assets_root, python_backend_cache, python_backend_command,
        python_backend_interpreter, PythonBackendRuntimeError,
    };
}
