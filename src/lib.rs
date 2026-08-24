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
#[cfg(feature = "cli")]
mod cli;
mod common;
mod comparison;
mod config;
mod data;
mod diagnostics;
mod errors;
mod geom;
mod inference;
mod io;
mod multimodal;
mod multiscale_residual;
mod neighborhood;
mod output;
mod perf;
mod periodogram;
mod permutation;
mod prepost;
mod qc;
mod registration;
mod spectra;
#[cfg(feature = "cli")]
mod synthetic_smoke;
mod workflow;
#[cfg(feature = "wsi")]
mod wsi;

#[cfg(feature = "cli")]
#[doc(hidden)]
pub use cli::run_cli;

pub use api::{AnalysisEngine, MarkedAnalysisRun};
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
pub use io::{PatternLoadDiagnostics, PatternLoadResult, PatternLoader};
pub use marklab_data::{
    BlockId, CellId, CohortHierarchy, CoordinateFrame, CoordinateFrameId, CoordinateRegistry,
    CoordinateSpace, CoordinateUnit, CoreId, FrameTransform, HierarchyId, HierarchyKind,
    HierarchyNode, ImageCoordinateConvention, PatchId, PatientId, RegionId, RepeatedMeasureSet,
    ReplicationRole, SectionId, SerialSectionSeries, SiteId, SlideId, SpatialAxis, SpecimenId,
    TimepointId, TransformId, TransformMatrix, UncertaintyId, UncertaintyReference,
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
    verify_patch_embedding_table_parquet_from_store, write_patch_embedding_table_arrow,
    write_patch_embedding_table_parquet, write_region_embedding_table_arrow,
    write_region_embedding_table_parquet, write_slide_embedding_table_arrow,
    write_slide_embedding_table_parquet, MultiscaleMatrixArrowPreflight,
    MultiscaleMatrixParquetPreflight,
};
pub use marklab_embeddings::{
    ArtifactAvailabilityFailure, CanonicalDecimal, CellEmbeddingArtifact,
    CellEmbeddingArtifactRole, CellEmbeddingBlock, CellEmbeddingExecutionProvenance,
    CellEmbeddingInputArtifacts, CellEmbeddingModelProvenance, CellEmbeddingProvenance,
    CellEmbeddingRow, CellEmbeddingRowLink, CellEmbeddingRowLinkEntry, CellEmbeddingTable,
    CellEmbeddingTensorContract, CellEmbeddingView, CellIdentityMap, CellIdentityMapEntry,
    CellPatchAnchor, CellPatchAssignment, CellPatchAssignmentMode, CellPatchAssignmentStatus,
    CellPatchContributor, CellPatchEdge, CellPatchLink, CellPatchLinkBindings,
    CellPatchLinkProducer, CellPatchWeight, CellVitCsvField, CellVitNpyMatrix, CellVitNpySummary,
    CsvFailure, DeclaredCellPatchAssignment, EffectiveReceptiveField, EmbeddingArtifactGraphError,
    EmbeddingDtype, EmbeddingEntityKind, EmbeddingError, EmbeddingQcSummary,
    EmbeddingSpatialContext, EmbeddingStatus, ExpectedCellSet, ExpectedPatchSet, ExpectedRegionSet,
    ExpectedSlideSet, ImportFailure, ManifestFailure, MultiscaleArtifactBinding,
    MultiscaleDirectPatchInputArtifacts, MultiscaleDirectPatchModelProvenance,
    MultiscaleEmbeddingArtifactGraphError, MultiscaleEmbeddingArtifactRole,
    MultiscaleEmbeddingDerivationContract, MultiscaleEmbeddingError,
    MultiscaleEmbeddingExecutionProvenance, MultiscaleEmbeddingProvenance,
    MultiscaleEmbeddingProvenanceVariant, MultiscaleEmbeddingQcSummary, MultiscaleEmbeddingSupport,
    MultiscaleEmbeddingSupportVariant, NpyFailure, NpyVersion, PatchBoundaryPolicy,
    PatchEmbeddingBlock, PatchEmbeddingContext, PatchEmbeddingInputNormalization,
    PatchEmbeddingRow, PatchEmbeddingSourceRowLink, PatchEmbeddingSourceRowLinkEntry,
    PatchEmbeddingTable, PatchEmbeddingView, PatchFootprint, PatchFootprintSet, PatchIdentityMap,
    PatchIdentityMapEntry, PatchNormalizationDecimal, PatchOverlapEdge, PatchOverlapGraph,
    PatchRegionAssessment, PatchRegionAssessmentBindings, PatchRegionDeclaration, PatchRegionLink,
    PatchRegionRelation, PatchSourceEntityEntry, PatchSourceEntitySet, PositiveRational,
    ReconciliationFailure, RegionEmbeddingBlock, RegionEmbeddingRow, RegionEmbeddingTable,
    RegionEmbeddingView, SlideEmbeddingBlock, SlideEmbeddingRow, SlideEmbeddingTable,
    SlideEmbeddingView, SourceBundleBudgets, SourceBundleError, SourceFileKind, SourceIoFailure,
    SourceIoOperation, VerifiedCellEmbeddingArtifactGraph, VerifiedCellEmbeddingRowLinkArtifact,
    VerifiedCellEmbeddingTableArtifact, VerifiedCellPatchAssignmentArtifact,
    VerifiedCellPatchEdgeArtifact, VerifiedCellPatchLinkArtifact,
    VerifiedDerivedRegionEmbeddingArtifactGraph, VerifiedDirectPatchEmbeddingArtifactGraph,
    VerifiedPatchEmbeddingSupportArtifact, VerifiedPatchEmbeddingTableArtifact,
    VerifiedPatchFootprintArtifact, VerifiedPatchOverlapArtifact, VerifiedPatchRegionLinkArtifact,
    VerifiedRegionEmbeddingSupportArtifact,
};
pub use marklab_workflow::{
    ArtifactCatalog, ArtifactCatalogError, ArtifactDraft, ArtifactId, ArtifactKey, ArtifactLocator,
    ArtifactPublication, ArtifactRecord, ArtifactRecordError, ArtifactRef, ArtifactSchema,
    ArtifactStoreError, CacheKeyMaterial, CacheStatus, ContentDigest, ContentDigestParseError,
    ContentDigestWriter, LocalArtifactStore, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeRun, NodeSpec, ProjectError, PublicationDisposition, RecoveryIssue, RecoveryIssueReason,
    RecoveryReport, SchedulerLimits, StoreId, SuccessfulRun, TableColumn, TableColumnType,
    TableFormat, TableManifest, TableManifestError, TableScalarType, VerifiedReaderError,
    WorkflowError, WorkflowGraph, WorkflowNode,
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
    TerritoryPrePostSummary, TerritoryProfile, TimingStage, WindowSummary, RESULT_FORMAT_VERSION,
};
pub use prepost::{
    compare_marked_prepost, compare_multimodal_prepost, compare_multimodal_prepost_with_margin,
};
pub use registration::{
    landmarks::LandmarkPair,
    transform::{Transform2D, TransformKind},
};
pub use workflow::MarkedAnalysisNode;
#[cfg(feature = "wsi")]
pub use wsi::{
    PlaneSelection, RegionRequest, RgbaRegion, SlideLevelMetadata, SlideMetadata, SlideOpenOptions,
    SlideReader, SlideSampleType, SlideSceneMetadata, SlideSeriesMetadata,
};
