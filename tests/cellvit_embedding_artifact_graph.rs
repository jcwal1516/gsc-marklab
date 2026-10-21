#![cfg(feature = "parquet")]

use std::{collections::BTreeMap, fs, mem::size_of};

#[cfg(feature = "csv")]
use marklab::embeddings::{
    import_cellvit_he_bundle_bytes, CellVitHeArtifactBindings, CellVitHeImportRequest,
    SourceBundleBudgets,
};
use marklab::{
    embeddings::{
        preflight_cell_embedding_table_arrow_bytes, publish_cell_embedding_row_link_arrow,
        read_cell_embedding_table_arrow_bytes, read_cell_embedding_table_arrow_from_store,
        read_cell_embedding_table_parquet_bytes, read_cell_embedding_table_parquet_from_store,
        scan_cell_embedding_table_arrow_bytes, scan_cell_embedding_table_arrow_from_store,
        scan_cell_embedding_table_parquet_bytes, scan_cell_embedding_table_parquet_from_store,
        verify_cell_embedding_row_link_arrow_from_store, verify_cell_embedding_table_arrow_bytes,
        verify_cell_embedding_table_parquet_bytes, write_cell_embedding_table_arrow,
        write_cell_embedding_table_parquet, ArrowIpcFailure, ArtifactAvailabilityFailure,
        CanonicalDecimal, CellEmbeddingArtifact, CellEmbeddingArtifactRole,
        CellEmbeddingExecutionProvenance, CellEmbeddingInputArtifacts,
        CellEmbeddingModelProvenance, CellEmbeddingProvenance, CellEmbeddingRow,
        CellEmbeddingRowLink, CellEmbeddingRowLinkEntry, CellEmbeddingTable,
        CellEmbeddingTablePhysicalBindings, CellEmbeddingTensorContract, CellIdentityMap,
        CellIdentityMapEntry, ColumnarError, EmbeddingArtifactGraphError, EmbeddingColumnarBudgets,
        EmbeddingDtype, EmbeddingSpatialContext, EmbeddingStatus, ExpectedCellSet,
        PatchBoundaryPolicy, PositiveRational, VerifiedCellEmbeddingArtifactGraph,
    },
    workflow::{
        ArtifactCatalog, ArtifactKey, ArtifactLocator, ArtifactRecord, ArtifactRef, ArtifactSchema,
        ArtifactStoreError, LocalArtifactStore, StoreId, TableColumn, TableColumnType, TableFormat,
        TableManifest, TableScalarType, VerifiedReaderError,
    },
    CellId, CohortHierarchy, CoordinateFrame, CoordinateFrameId, CoordinateRegistry,
    CoordinateSpace, CoordinateUnit, FrameTransform, HierarchyId, HierarchyNode,
    ImageCoordinateConvention, PatientId, ReplicationRole, SlideId, SpatialAxis, TransformId,
    TransformMatrix,
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
