#![forbid(unsafe_code)]
//! Supported API for marked-pattern, multimodal, and bounded WSI analysis.
//!
//! The compatibility contract is the set of types re-exported from this crate
//! root. Algorithm and orchestration modules remain private.

#[cfg(all(test, feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
#[global_allocator]
static TEST_ALLOCATOR: dhat::Alloc = dhat::Alloc;

#[cfg(test)]
mod algorithm_tests;
mod api;
mod binary_nucleus_area_contrast;
mod categorical_neighborhood_mixing;
mod categorical_neighborhood_mixing_workflow;
mod categorical_pair;
mod categorical_pair_workflow;
mod cell_embedding_mark;
mod cell_embedding_mark_workflow;
mod cell_embedding_nucleus_area;
mod cell_embedding_probability;
mod classical;
mod classical_workflow;
#[cfg(feature = "cli")]
mod cli;
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
mod errors;
#[doc(hidden)]
pub mod exact_float_json;
mod geom;
mod inference;
mod inhomogeneous_spatial;
mod io;
mod mark_pair_plan;
mod mark_weighted_k;
mod mark_weighted_k_workflow;
mod marked_prepost_dag;
mod multimodal;
mod multiscale_residual;
mod nearest_space;
mod nearest_space_workflow;
mod neighborhood;
mod ordinal_composition;
mod ordinal_composition_workflow;
mod output;
mod pair_correlation;
mod pair_correlation_workflow;
mod perf;
mod periodogram;
mod permutation;
mod prepost;
mod probability_pair;
mod probability_pair_workflow;
mod qc;
mod registration;
mod scalar_mark;
mod scalar_variogram;
mod soft_class_composition;
mod soft_class_composition_workflow;
mod soft_multiscale_neighborhood;
mod soft_multiscale_neighborhood_workflow;
mod soft_neighborhood_composition;
mod soft_neighborhood_composition_workflow;
mod spatial_autocorrelation;
mod spatial_autocorrelation_workflow;
mod spectra;
#[cfg(feature = "cli")]
mod synthetic_smoke;
mod workflow;
#[cfg(feature = "wsi")]
mod wsi;

#[cfg(feature = "cli")]
#[doc(hidden)]
pub use cli::run_cli;

pub use api::{AnalysisEngine, DeclaredMarkedAnalysisRun, MarkedAnalysisRun};
pub use binary_nucleus_area_contrast::{
    declared_binary_group_nucleus_area_contrast, DeclaredBinaryGroupNucleusAreaContrast,
    DeclaredBinaryGroupNucleusAreaContrastError, DeclaredBinaryGroupNucleusAreaContrastStatus,
};
pub use categorical_neighborhood_mixing::{
    categorical_neighborhood_mixing, CategoricalNeighborhoodClassSummary,
    CategoricalNeighborhoodMixingConfig, CategoricalNeighborhoodMixingError,
    CategoricalNeighborhoodMixingLimits, CategoricalNeighborhoodMixingResult,
};
pub use categorical_neighborhood_mixing_workflow::CategoricalNeighborhoodMixingAnalysisNode;
pub use categorical_pair::{
    categorical_mark_connection_cross_k, CategoricalPairComponentInference, CategoricalPairConfig,
    CategoricalPairError, CategoricalPairGeometrySummary, CategoricalPairInferenceSummary,
    CategoricalPairLimits, CategoricalPairPoint, CategoricalPairPointStatus, CategoricalPairResult,
};
pub use categorical_pair_workflow::CategoricalPairAnalysisNode;
pub use cell_embedding_mark::{
    declared_binary_cell_embedding_centroid_discrepancy,
    DeclaredBinaryCellEmbeddingCentroidDiscrepancy,
    DeclaredBinaryCellEmbeddingCentroidDiscrepancyError,
    DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus, DeclaredBinaryCellEmbeddingGroupCounts,
};
pub use cell_embedding_mark_workflow::DeclaredBinaryCellEmbeddingCentroidNode;
pub use cell_embedding_nucleus_area::{
    declared_nucleus_area_cell_embedding_cross_covariance_energy,
    DeclaredNucleusAreaCellEmbeddingCrossCovarianceEnergy,
    DeclaredNucleusAreaCellEmbeddingCrossCovarianceError,
    DeclaredNucleusAreaCellEmbeddingCrossCovarianceStatus,
};
pub use cell_embedding_probability::{
    declared_probability_cell_embedding_cross_covariance_energy,
    DeclaredProbabilityCellEmbeddingCrossCovarianceEnergy,
    DeclaredProbabilityCellEmbeddingCrossCovarianceError,
    DeclaredProbabilityCellEmbeddingCrossCovarianceStatus,
};
pub use classical::{
    analyze_classical_spatial_pattern, ClassicalCacheStatus, ClassicalConfigurationSummary,
    ClassicalGeometrySummary, ClassicalInferenceSummary, ClassicalNullDesign, ClassicalNullModel,
    ClassicalRandomizationUnit, ClassicalSpatialConfig, ClassicalSpatialError,
    ClassicalSpatialLimits, ClassicalSpatialResult, ClassicalSpatialResultDocument,
    ClassicalSpatialStatus, ClassicalWindowSummary, ClassicalWorkflowIdentity, HomogeneousKlPoint,
    KlPointStatus, CLASSICAL_SPATIAL_FORMAT, CLASSICAL_SPATIAL_FORMAT_VERSION,
};
pub use classical_workflow::ClassicalSpatialAnalysisNode;
pub use compartment_cell_mixing::{
    analyze_compartment_cell_mixing, CompartmentCellMixingConfig, CompartmentCellMixingError,
    CompartmentCellMixingLimits, CompartmentCellMixingResult, CompartmentCellMixingSummary,
};
pub use compartment_cell_mixing_workflow::CompartmentCellMixingAnalysisNode;
pub use compartment_contact::{
    compartment_contact_fractions, CompartmentContactFraction, CompartmentContactResult,
};
pub use compartment_contact_workflow::CompartmentContactAnalysisNode;
pub use compartment_fragmentation::{
    compartment_fragmentation, CompartmentFragmentationResult, CompartmentFragmentationSummary,
};
pub use compartment_fragmentation_workflow::CompartmentFragmentationAnalysisNode;
pub use compartment_interface::{
    analyze_compartment_interface_profile, CompartmentInterfaceCell, CompartmentInterfaceError,
    CompartmentInterfaceLimits, CompartmentInterfaceProfile, CompartmentInterfaceSummary,
};
pub use compartment_interface_workflow::CompartmentInterfaceAnalysisNode;
pub use config::{
    AnalysisConfig, AnalysisConfigSection, ComparisonSection, ComponentMode, CurveMargins,
    DiagnosticsSection, InferenceSection, MultiscaleResidualSection, NeighborhoodNullModel,
    NeighborhoodSection, OutputSection, PerformanceSection, PeriodogramSection, PermutationSection,
    PermutationStratum, RegistrationSection, RegistrationTransform, SpectrumSection, ThreadSetting,
    ValidationSection,
};
#[cfg(feature = "parquet")]
pub use contained_patch_nucleus_area_contrast::{
    contained_patch_binary_nucleus_area_contrast, ContainedPatchBinaryNucleusAreaContrast,
    ContainedPatchBinaryNucleusAreaContrastError, ContainedPatchBinaryNucleusAreaContrastStatus,
};
pub use continuous_mark_correlation::{
    continuous_mark_correlation, ContinuousMarkCorrelationComponentInference,
    ContinuousMarkCorrelationConfig, ContinuousMarkCorrelationError,
    ContinuousMarkCorrelationGeometrySummary, ContinuousMarkCorrelationInferenceSummary,
    ContinuousMarkCorrelationLimits, ContinuousMarkCorrelationPoint,
    ContinuousMarkCorrelationPointStatus, ContinuousMarkCorrelationResult,
};
pub use continuous_mark_correlation_workflow::ContinuousMarkCorrelationAnalysisNode;
pub use cross_pair_correlation::{
    categorical_cross_pair_correlation, CategoricalCrossPairCorrelationAnalysisNode,
    CategoricalCrossPairCorrelationConfig, CategoricalCrossPairCorrelationError,
    CategoricalCrossPairCorrelationInference, CategoricalCrossPairCorrelationPoint,
    CategoricalCrossPairCorrelationResult,
};
pub use data::{Pattern, PatternMeta, TumorWindow};
pub use errors::{MarklabError, Result};
pub use geom::mask::TumorMask;
pub use geom::window::{
    BinaryCompartmentPartition2D, CompartmentPartitionDescriptor, CompartmentPartitionError,
    CompartmentPartitionLimits, ObservationWindow2D, ObservationWindowDescriptor,
    ObservationWindowError, ObservationWindowLimits,
};
pub use inhomogeneous_spatial::{
    analyze_inhomogeneous_pair_correlation, analyze_inhomogeneous_spatial_pattern,
    InhomogeneousIntensityGridPoint, InhomogeneousIntensityPoint, InhomogeneousIntensitySummary,
    InhomogeneousPairCorrelationAnalysisNode, InhomogeneousPairCorrelationConfig,
    InhomogeneousPairCorrelationPoint, InhomogeneousPairCorrelationResult,
    InhomogeneousSpatialAnalysisNode, InhomogeneousSpatialConfig, InhomogeneousSpatialError,
    InhomogeneousSpatialInference, InhomogeneousSpatialLimits, InhomogeneousSpatialPoint,
    InhomogeneousSpatialPointStatus, InhomogeneousSpatialResult,
};
pub use io::{PatternLoadDiagnostics, PatternLoadResult, PatternLoader};
pub use mark_weighted_k::{
    continuous_mark_weighted_k, MarkWeightedKComponentInference, MarkWeightedKConfig,
    MarkWeightedKError, MarkWeightedKGeometrySummary, MarkWeightedKInferenceSummary,
    MarkWeightedKLimits, MarkWeightedKPoint, MarkWeightedKPointStatus, MarkWeightedKResult,
};
pub use mark_weighted_k_workflow::MarkWeightedKAnalysisNode;
pub use marked_prepost_dag::{
    execute_marked_prepost_dag, plan_marked_prepost_dag, MarkedPrePostDagError,
    MarkedPrePostDagLimits, MarkedPrePostDagPlan, MarkedPrePostDagRun, MarkedPrePostDagTarget,
};
pub use marklab_data::{
    BlockId, CellId, CohortHierarchy, CoordinateFrame, CoordinateFrameId, CoordinateRegistry,
    CoordinateSpace, CoordinateUnit, CoreId, FrameTransform, HierarchyId, HierarchyKind,
    HierarchyNode, ImageCoordinateConvention, MeasurementStatus, PatchId, PatientId, RegionId,
    RepeatedMeasureSet, ReplicationRole, SectionId, SerialSectionSeries, SiteId, SlideId,
    SpatialAxis, SpecimenId, TimepointId, TransformId, TransformMatrix, UncertaintyId,
    UncertaintyReference,
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
    CellPatchLinkProducer, CellPatchWeight, CellVitCsvField, CellVitNpyMatrix, CellVitNpySummary,
    CsvFailure, DeclaredCellPatchAssignment, DerivedRegionEmbeddingTableCandidate,
    DerivedSlideEmbeddingTableCandidate, EffectiveReceptiveField, EmbeddingArtifactGraphError,
    EmbeddingDtype, EmbeddingEntityKind, EmbeddingError, EmbeddingFinalizationBudgets,
    EmbeddingQcSummary, EmbeddingSpatialContext, EmbeddingStatus, ExpectedCellSet,
    ExpectedPatchSet, ExpectedRegionSet, ExpectedSlideSet, ImportFailure, ManifestFailure,
    MultiscaleArtifactBinding, MultiscaleDirectPatchInputArtifacts,
    MultiscaleDirectPatchModelProvenance, MultiscaleEmbeddingArtifactGraphError,
    MultiscaleEmbeddingArtifactRole, MultiscaleEmbeddingDerivationContract,
    MultiscaleEmbeddingError, MultiscaleEmbeddingExecutionProvenance,
    MultiscaleEmbeddingProvenance, MultiscaleEmbeddingProvenanceVariant,
    MultiscaleEmbeddingQcSummary, MultiscaleEmbeddingSupport, MultiscaleEmbeddingSupportVariant,
    NpyFailure, NpyVersion, PatchBoundaryPolicy, PatchEmbeddingBlock, PatchEmbeddingContext,
    PatchEmbeddingInputNormalization, PatchEmbeddingRow, PatchEmbeddingSourceRowLink,
    PatchEmbeddingSourceRowLinkEntry, PatchEmbeddingTable, PatchEmbeddingView, PatchFootprint,
    PatchFootprintSet, PatchIdentityMap, PatchIdentityMapEntry, PatchNormalizationDecimal,
    PatchOverlapEdge, PatchOverlapEmbeddingDispersion, PatchOverlapEmbeddingDispersionError,
    PatchOverlapEmbeddingDispersionStatus, PatchOverlapGraph, PatchRegionAssessment,
    PatchRegionAssessmentBindings, PatchRegionDeclaration, PatchRegionEmbeddingDispersion,
    PatchRegionEmbeddingDispersionError, PatchRegionEmbeddingDispersionStatus, PatchRegionLink,
    PatchRegionRelation, PatchSourceEntityEntry, PatchSourceEntitySet, PositiveRational,
    ReconciliationFailure, RegionEmbeddingBlock, RegionEmbeddingRow, RegionEmbeddingTable,
    RegionEmbeddingView, SlideEmbeddingAggregationPathDiscrepancy,
    SlideEmbeddingAggregationPathDiscrepancyError, SlideEmbeddingAggregationPathDiscrepancyStatus,
    SlideEmbeddingBlock, SlideEmbeddingRow, SlideEmbeddingTable, SlideEmbeddingView,
    SourceBundleBudgets, SourceBundleError, SourceFileKind, SourceIoFailure, SourceIoOperation,
    VerifiedCellEmbeddingArtifactGraph, VerifiedCellEmbeddingRowLinkArtifact,
    VerifiedCellEmbeddingTableArtifact, VerifiedCellPatchAssignmentArtifact,
    VerifiedCellPatchEdgeArtifact, VerifiedCellPatchLinkArtifact,
    VerifiedDerivedRegionEmbeddingArtifactGraph, VerifiedDerivedSlideEmbeddingArtifactGraph,
    VerifiedDirectPatchEmbeddingArtifactGraph, VerifiedPatchEmbeddingSupportArtifact,
    VerifiedPatchEmbeddingTableArtifact, VerifiedPatchFootprintArtifact,
    VerifiedPatchOverlapArtifact, VerifiedPatchRegionLinkArtifact,
    VerifiedRegionEmbeddingSupportArtifact, VerifiedRegionEmbeddingTableArtifact,
    VerifiedSlideEmbeddingSupportArtifact, VerifiedSlideEmbeddingTableArtifact,
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
    preflight_cell_embedding_row_link_arrow_bytes, preflight_cell_embedding_row_link_parquet_bytes,
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
    validate_cell_patch_edge_table_arrow_bytes, validate_cell_patch_edge_table_arrow_from_store,
    validate_cell_patch_edge_table_parquet_bytes,
    validate_cell_patch_edge_table_parquet_from_store, validate_patch_footprint_set_arrow_bytes,
    validate_patch_footprint_set_arrow_from_store, validate_patch_footprint_set_parquet_bytes,
    validate_patch_footprint_set_parquet_from_store, validate_patch_overlap_graph_arrow_bytes,
    validate_patch_overlap_graph_arrow_from_store, validate_patch_overlap_graph_parquet_bytes,
    validate_patch_overlap_graph_parquet_from_store, validate_patch_region_link_arrow_bytes,
    validate_patch_region_link_arrow_from_store, validate_patch_region_link_parquet_bytes,
    validate_patch_region_link_parquet_from_store, verify_cell_embedding_row_link_arrow_bytes,
    verify_cell_embedding_row_link_arrow_from_store, verify_cell_embedding_row_link_parquet_bytes,
    verify_cell_embedding_row_link_parquet_from_store, verify_cell_embedding_table_arrow_bytes,
    verify_cell_embedding_table_arrow_from_store, verify_cell_embedding_table_parquet_bytes,
    verify_cell_embedding_table_parquet_from_store, verify_cell_patch_assignment_table_arrow_bytes,
    verify_cell_patch_assignment_table_arrow_from_store,
    verify_cell_patch_assignment_table_parquet_bytes,
    verify_cell_patch_assignment_table_parquet_from_store,
    verify_cell_patch_edge_table_arrow_bytes, verify_cell_patch_edge_table_arrow_from_store,
    verify_cell_patch_edge_table_parquet_bytes, verify_cell_patch_edge_table_parquet_from_store,
    verify_patch_footprint_set_arrow_bytes, verify_patch_footprint_set_arrow_from_store,
    verify_patch_footprint_set_parquet_bytes, verify_patch_footprint_set_parquet_from_store,
    verify_patch_overlap_graph_arrow_bytes, verify_patch_overlap_graph_arrow_from_store,
    verify_patch_overlap_graph_parquet_bytes, verify_patch_overlap_graph_parquet_from_store,
    verify_patch_region_link_arrow_bytes, verify_patch_region_link_arrow_from_store,
    verify_patch_region_link_parquet_bytes, verify_patch_region_link_parquet_from_store,
    write_cell_embedding_row_link_arrow, write_cell_embedding_row_link_parquet,
    write_cell_embedding_table_arrow, write_cell_embedding_table_parquet,
    write_cell_patch_assignment_table_arrow, write_cell_patch_assignment_table_parquet,
    write_cell_patch_edge_table_arrow, write_cell_patch_edge_table_parquet,
    write_patch_footprint_set_arrow, write_patch_footprint_set_parquet,
    write_patch_overlap_graph_arrow, write_patch_overlap_graph_parquet,
    write_patch_region_link_arrow, write_patch_region_link_parquet, ArrowIpcFailure,
    CellEmbeddingArrowPreflight, CellEmbeddingParquetPreflight, CellEmbeddingRowLinkArrowPreflight,
    CellEmbeddingRowLinkParquetPreflight, CellEmbeddingTablePhysicalBindings,
    CellPatchAssignmentArrowPreflight, CellPatchAssignmentParquetPreflight,
    CellPatchEdgeArrowPreflight, CellPatchEdgeParquetPreflight, CellPatchInputArtifactGraphError,
    CellPatchInputArtifactRole, ColumnarWriteSummary, EmbeddingColumnarBudgets,
    EmbeddingColumnarError, EmbeddingColumnarPublicationError, MultiscaleColumnarError,
    MultiscaleColumnarPublicationError, ParquetFailure, PatchFootprintArrowPreflight,
    PatchFootprintParquetPreflight, PatchOverlapArrowPreflight, PatchOverlapParquetPreflight,
    PatchRegionArrowPreflight, PatchRegionInputArtifactGraphError, PatchRegionInputArtifactRole,
    PatchRegionParquetPreflight, RowLinkColumnarWriteSummary, SpatialArrowFailure,
    SpatialColumnarWriteSummary, SpatialParquetFailure, VerifiedCellPatchInputArtifactGraph,
    VerifiedPatchRegionInputArtifactGraph,
};
#[cfg(feature = "parquet")]
pub use marklab_embeddings::{
    preflight_patch_embedding_table_arrow_bytes, preflight_patch_embedding_table_parquet_bytes,
    preflight_region_embedding_table_arrow_bytes, preflight_region_embedding_table_parquet_bytes,
    preflight_slide_embedding_table_arrow_bytes, preflight_slide_embedding_table_parquet_bytes,
    publish_patch_embedding_table_arrow, publish_patch_embedding_table_parquet,
    publish_region_embedding_table_arrow, publish_region_embedding_table_parquet,
    publish_slide_embedding_table_arrow, publish_slide_embedding_table_parquet,
    validate_patch_embedding_table_arrow_bytes, validate_patch_embedding_table_arrow_from_store,
    validate_patch_embedding_table_parquet_bytes,
    validate_patch_embedding_table_parquet_from_store, validate_region_embedding_table_arrow_bytes,
    validate_region_embedding_table_arrow_from_store,
    validate_region_embedding_table_parquet_bytes,
    validate_region_embedding_table_parquet_from_store, validate_slide_embedding_table_arrow_bytes,
    validate_slide_embedding_table_arrow_from_store, validate_slide_embedding_table_parquet_bytes,
    validate_slide_embedding_table_parquet_from_store, verify_patch_embedding_table_arrow_bytes,
    verify_patch_embedding_table_arrow_from_store, verify_patch_embedding_table_parquet_bytes,
    verify_patch_embedding_table_parquet_from_store, verify_region_embedding_table_arrow_bytes,
    verify_region_embedding_table_arrow_from_store, verify_region_embedding_table_parquet_bytes,
    verify_region_embedding_table_parquet_from_store, verify_slide_embedding_table_arrow_bytes,
    verify_slide_embedding_table_arrow_from_store, verify_slide_embedding_table_parquet_bytes,
    verify_slide_embedding_table_parquet_from_store, write_patch_embedding_table_arrow,
    write_patch_embedding_table_parquet, write_region_embedding_table_arrow,
    write_region_embedding_table_parquet, write_slide_embedding_table_arrow,
    write_slide_embedding_table_parquet, MultiscaleMatrixArrowPreflight,
    MultiscaleMatrixParquetPreflight,
};
pub use marklab_workflow::{
    execute_algorithm, execute_algorithm_with_store, ArtifactCatalog, ArtifactCatalogError,
    ArtifactDraft, ArtifactId, ArtifactKey, ArtifactLocator, ArtifactPublication, ArtifactRecord,
    ArtifactRecordError, ArtifactRef, ArtifactSchema, ArtifactStoreError, CacheKeyMaterial,
    CacheStatus, ContentDigest, ContentDigestParseError, ContentDigestWriter,
    DurableCommitDisposition, DurableExecutionRequest, DurableOpenReport, DurableProject,
    DurableProjectError, DurableProjectLimits, DurableRecoveryAction, DurableReplay,
    ExecuteAlgorithmError, LocalArtifactStore, LocalScheduler, MarklabProject,
    NativeRuntimeProvenance, NodeError, NodeId, NodeRun, NodeSpec, ProjectError,
    PublicationDisposition, RecoveryIssue, RecoveryIssueReason, RecoveryReport, SchedulerLimits,
    StoreId, SuccessfulRun, TableColumn, TableColumnType, TableFormat, TableManifest,
    TableManifestError, TableScalarType, VerifiedReaderError, WorkflowError, WorkflowGraph,
    WorkflowNode,
};
pub use multimodal::{
    CellExtrapolationRecord, CellSection, FusedCell, HeCell, IhcCell, LandmarkHullAvailability,
    MultimodalAnalysisRun, MultimodalEngine, MultimodalInput, NullModelSensitivityResult,
    RegistrationExtrapolation, RegistrationResidual,
};
pub use nearest_space::{
    analyze_nearest_space_pattern, DistributionPointStatus, JPointStatus, NearestSpaceCacheStatus,
    NearestSpaceComponentInference, NearestSpaceConfig, NearestSpaceConfigurationSummary,
    NearestSpaceError, NearestSpaceGeometrySummary, NearestSpaceInferenceSummary,
    NearestSpaceLimits, NearestSpacePoint, NearestSpaceProbeSummary, NearestSpaceResult,
    NearestSpaceResultDocument, NearestSpaceStatus, NearestSpaceWorkflowIdentity,
    NEAREST_SPACE_FORMAT, NEAREST_SPACE_FORMAT_VERSION,
};
pub use nearest_space_workflow::NearestSpaceAnalysisNode;
pub use neighborhood::graph::{SpatialEdge, SpatialGraph};
pub use ordinal_composition::{
    ordinal_class_composition, OrdinalClassCompositionConfig, OrdinalClassCompositionError,
    OrdinalClassCompositionLimits, OrdinalClassCompositionResult,
};
pub use ordinal_composition_workflow::OrdinalClassCompositionAnalysisNode;
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
    TerritoryPrePostSummary, TerritoryProfile, TimingStage, WindowSummary, RESULT_FORMAT_VERSION,
};
pub use pair_correlation::{
    homogeneous_pair_correlation, HomogeneousPairCorrelationConfig,
    HomogeneousPairCorrelationError, HomogeneousPairCorrelationInference,
    HomogeneousPairCorrelationResult, PairCorrelationKernel, PairCorrelationPoint,
    PairCorrelationPointStatus,
};
pub use pair_correlation_workflow::HomogeneousPairCorrelationAnalysisNode;
pub use prepost::{
    compare_declared_marked_prepost, compare_declared_marked_prevalence, compare_marked_prepost,
    compare_multimodal_prepost, compare_multimodal_prepost_with_margin, DeclaredMarkedPrePostError,
    DeclaredMarkedPrePostResult, DeclaredMarkedPrevalenceChange, DeclaredMarkedPrevalenceStatus,
};
pub use probability_pair::{
    probability_mark_connection, ProbabilityPairComponentInference, ProbabilityPairConfig,
    ProbabilityPairError, ProbabilityPairGeometrySummary, ProbabilityPairInferenceSummary,
    ProbabilityPairLimits, ProbabilityPairPoint, ProbabilityPairPointStatus, ProbabilityPairResult,
};
pub use probability_pair_workflow::ProbabilityPairAnalysisNode;
pub use registration::{
    landmarks::LandmarkPair,
    transform::{Transform2D, TransformKind},
};
pub use scalar_mark::{
    BinaryMarkDeclaration, BinaryMarkOrigin, DeclaredMarkUse, DeclaredScalarIdentity,
    DeclaredScalarInputError, DeclaredScalarPatternInput, HistologicCompartmentMarkDeclaration,
    MarkTable, MissingnessPolicy, NucleusAreaUm2MarkDeclaration, OrdinalMarkDeclaration,
    ProbabilityMarkDeclaration, ProbabilitySimplexMarkDeclaration, ProbabilityThresholdComparator,
    ScalarMarkColumn, ScalarMarkId, ScalarMarkModality, ScalarMarkUnit, ScalarMarkValueKind,
    VectorArtifactRefMarkDeclaration,
};
pub use scalar_variogram::{
    scalar_semivariogram, scalar_semivariogram_permutation, ScalarVariogramBin,
    ScalarVariogramConditioning, ScalarVariogramEnvelopeRow, ScalarVariogramError,
    ScalarVariogramInferenceDesign, ScalarVariogramInferenceLimits, ScalarVariogramInferenceResult,
    ScalarVariogramLimits, ScalarVariogramResult, ScalarVariogramRow,
};
pub use soft_class_composition::{
    soft_class_composition, SoftClassCompositionClass, SoftClassCompositionError,
    SoftClassCompositionLimits, SoftClassCompositionResult,
};
pub use soft_class_composition_workflow::SoftClassCompositionAnalysisNode;
pub use soft_multiscale_neighborhood::{
    soft_multiscale_neighborhood_composition, SoftMultiscaleNeighborhoodConfig,
    SoftMultiscaleNeighborhoodError, SoftMultiscaleNeighborhoodLimits,
    SoftMultiscaleNeighborhoodResult, SoftMultiscaleNeighborhoodScale,
};
pub use soft_multiscale_neighborhood_workflow::SoftMultiscaleNeighborhoodAnalysisNode;
pub use soft_neighborhood_composition::{
    soft_neighborhood_composition, SoftNeighborhoodCompositionConfig,
    SoftNeighborhoodCompositionError, SoftNeighborhoodCompositionLimits,
    SoftNeighborhoodCompositionResult, SoftNeighborhoodCompositionRow,
};
pub use soft_neighborhood_composition_workflow::SoftNeighborhoodCompositionAnalysisNode;
pub use spatial_autocorrelation::{
    global_geary_permutation, global_moran_permutation, GlobalGearyAlternative, GlobalGearyDesign,
    GlobalGearyError, GlobalGearyLimits, GlobalGearyResult, GlobalMoranAlternative,
    GlobalMoranConditioning, GlobalMoranDesign, GlobalMoranError, GlobalMoranLimits,
    GlobalMoranResult, GlobalMoranWeightPolicy,
};
pub use spatial_autocorrelation_workflow::{
    GlobalMoranAnalysisNode, GlobalMoranPrePostNode, GlobalMoranPrePostResult,
};
pub use workflow::{
    DeclaredMarkedAnalysisNode, DeclaredMarkedAnalysisResult, MarkedAnalysisNode, MarkedPrePostNode,
};
#[cfg(feature = "wsi")]
pub use wsi::{
    PlaneSelection, RegionRequest, RgbaRegion, SlideLevelMetadata, SlideMetadata, SlideOpenOptions,
    SlideReader, SlideSampleType, SlideSceneMetadata, SlideSeriesMetadata,
};
