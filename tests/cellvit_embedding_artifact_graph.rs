#![cfg(feature = "parquet")]

use std::{collections::BTreeMap, fs, mem::size_of};

#[cfg(feature = "csv")]
use marklab::{
    import_cellvit_he_bundle_bytes, CellVitHeArtifactBindings, CellVitHeImportRequest,
    SourceBundleBudgets,
};
use marklab::{
    preflight_cell_embedding_table_arrow_bytes, publish_cell_embedding_row_link_arrow,
    read_cell_embedding_table_arrow_bytes, read_cell_embedding_table_arrow_from_store,
    read_cell_embedding_table_parquet_bytes, read_cell_embedding_table_parquet_from_store,
    scan_cell_embedding_table_arrow_bytes, scan_cell_embedding_table_arrow_from_store,
    scan_cell_embedding_table_parquet_bytes, scan_cell_embedding_table_parquet_from_store,
    verify_cell_embedding_row_link_arrow_from_store, verify_cell_embedding_table_arrow_bytes,
    verify_cell_embedding_table_parquet_bytes, write_cell_embedding_table_arrow,
    write_cell_embedding_table_parquet, ArrowIpcFailure, ArtifactAvailabilityFailure,
    ArtifactCatalog, ArtifactKey, ArtifactLocator, ArtifactRecord, ArtifactRef, ArtifactSchema,
    ArtifactStoreError, CanonicalDecimal, CellEmbeddingArtifact, CellEmbeddingArtifactRole,
    CellEmbeddingExecutionProvenance, CellEmbeddingInputArtifacts, CellEmbeddingModelProvenance,
    CellEmbeddingProvenance, CellEmbeddingRow, CellEmbeddingRowLink, CellEmbeddingRowLinkEntry,
    CellEmbeddingTable, CellEmbeddingTablePhysicalBindings, CellEmbeddingTensorContract, CellId,
    CellIdentityMap, CellIdentityMapEntry, CohortHierarchy, CoordinateFrame, CoordinateFrameId,
    CoordinateRegistry, CoordinateSpace, CoordinateUnit, EmbeddingArtifactGraphError,
    EmbeddingColumnarBudgets, EmbeddingColumnarError, EmbeddingDtype, EmbeddingSpatialContext,
    EmbeddingStatus, ExpectedCellSet, FrameTransform, HierarchyId, HierarchyNode,
    ImageCoordinateConvention, LocalArtifactStore, PatchBoundaryPolicy, PatientId,
    PositiveRational, ReplicationRole, SlideId, SpatialAxis, StoreId, TableColumn, TableColumnType,
    TableFormat, TableManifest, TableScalarType, TransformId, TransformMatrix,
    VerifiedCellEmbeddingArtifactGraph, VerifiedReaderError,
};
use proptest::prelude::*;
use tempfile::TempDir;

#[path = "cellvit_embedding_artifact_graph/fixture_support.rs"]
mod fixture_support;
use fixture_support::*;

#[path = "cellvit_embedding_artifact_graph/physical_artifact.rs"]
mod physical_artifact;

#[path = "cellvit_embedding_artifact_graph/declared_embedding_support.rs"]
mod declared_embedding_support;

#[path = "cellvit_embedding_artifact_graph/declared_binary_centroid.rs"]
mod declared_binary_centroid;

#[path = "cellvit_embedding_artifact_graph/declared_binary_centroid_workflow.rs"]
mod declared_binary_centroid_workflow;

#[path = "cellvit_embedding_artifact_graph/probability_cross_covariance.rs"]
mod probability_cross_covariance;

#[path = "cellvit_embedding_artifact_graph/nucleus_area_cross_covariance.rs"]
mod nucleus_area_cross_covariance;

#[path = "cellvit_embedding_artifact_graph/contained_cell_patch_embedding_dispersion.rs"]
mod contained_cell_patch_embedding_dispersion;

#[allow(dead_code)]
#[path = "support/declared_scalar.rs"]
mod declared_scalar_support;
