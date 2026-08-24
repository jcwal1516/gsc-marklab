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

struct Fixture {
    _root: TempDir,
    store: LocalArtifactStore,
    catalog: ArtifactCatalog,
    provenance: CellEmbeddingProvenance,
    provenance_artifact_id: marklab::ArtifactId,
    expected: ExpectedCellSet,
    identity_map: CellIdentityMap,
    context: EmbeddingSpatialContext,
    row_link: CellEmbeddingRowLink,
    dimension: u32,
}

#[derive(Clone, Copy)]
enum LicenseAvailability {
    Managed,
    CatalogOnly,
    NonManagedLocal,
}

#[derive(Clone, Copy)]
enum FixtureEmbeddingStatus {
    Present,
    MissingVector,
    ExtractionFailed,
    QcRejected,
}

impl FixtureEmbeddingStatus {
    fn domain_status(self) -> EmbeddingStatus {
        match self {
            Self::Present => EmbeddingStatus::Present,
            Self::MissingVector => EmbeddingStatus::MissingVector,
            Self::ExtractionFailed => EmbeddingStatus::ExtractionFailed,
            Self::QcRejected => EmbeddingStatus::QcRejected,
        }
    }

    fn table_row(self, cell_id: CellId, dimension: u32) -> CellEmbeddingRow {
        match self {
            Self::Present => CellEmbeddingRow::present(cell_id, vec![0.25; dimension as usize]),
            status => CellEmbeddingRow::non_present(cell_id, status.domain_status())
                .expect("non-present fixture row"),
        }
    }
}

fn cell(value: &str) -> CellId {
    CellId::new(value).expect("cell ID")
}

fn draft_record(
    schema: &str,
    kind: &str,
    bytes: &[u8],
    dependencies: Vec<marklab::ArtifactId>,
    table: Option<TableManifest>,
) -> ArtifactRecord {
    ArtifactRecord::new(
        ArtifactRef::from_bytes(kind, bytes).expect("artifact content"),
        ArtifactSchema::new(schema, 1).expect("artifact schema"),
        table,
        dependencies,
        BTreeMap::new(),
        vec![ArtifactLocator::new(
            StoreId::new("source").expect("source store"),
            ArtifactKey::new(format!("fixtures/privacy-sentinel/{schema}")).expect("source key"),
            None,
        )
        .expect("source locator")],
    )
    .expect("artifact record")
}

fn publish_record(
    store: &LocalArtifactStore,
    schema: &str,
    kind: &str,
    bytes: &[u8],
    dependencies: Vec<marklab::ArtifactId>,
    table: Option<TableManifest>,
) -> ArtifactRecord {
    store
        .publish(
            &draft_record(schema, kind, bytes, dependencies, table),
            |writer| writer.write_all(bytes),
        )
        .expect("publish fixture")
        .into_record()
}

fn row_link_manifest(row_count: u64) -> TableManifest {
    TableManifest::new(
        TableFormat::ArrowIpcFile,
        "marklab.arrow-ipc.embedding-row-link.v1",
        row_count,
        vec![
            TableColumn::new(
                "cell_id",
                TableColumnType::Scalar(TableScalarType::Utf8),
                false,
            )
            .expect("cell column"),
            TableColumn::new(
                "source_cell_row",
                TableColumnType::Scalar(TableScalarType::U64),
                false,
            )
            .expect("source row column"),
            TableColumn::new(
                "source_embedding_row",
                TableColumnType::Scalar(TableScalarType::U64),
                true,
            )
            .expect("embedding row column"),
        ],
        vec!["cell_id".to_owned()],
    )
    .expect("row-link manifest")
}

fn embedding_manifest(row_count: u64, dimension: u32) -> TableManifest {
    TableManifest::new(
        TableFormat::ArrowIpcFile,
        "marklab.arrow-ipc.embedding-table.v1",
        row_count,
        vec![
            TableColumn::new(
                "cell_id",
                TableColumnType::Scalar(TableScalarType::Utf8),
                false,
            )
            .expect("cell column"),
            TableColumn::new(
                "embedding",
                TableColumnType::FixedSizeList {
                    element: TableScalarType::F32,
                    length: dimension,
                },
                false,
            )
            .expect("embedding column"),
            TableColumn::new(
                "embedding_status",
                TableColumnType::Scalar(TableScalarType::Utf8),
                false,
            )
            .expect("status column"),
        ],
        vec!["cell_id".to_owned()],
    )
    .expect("embedding manifest")
}

fn embedding_parquet_manifest(row_count: u64, dimension: u32) -> TableManifest {
    TableManifest::new(
        TableFormat::ParquetFile,
        "marklab.parquet.embedding-table.v1",
        row_count,
        embedding_manifest(row_count, dimension).columns().to_vec(),
        vec!["cell_id".to_owned()],
    )
    .expect("embedding Parquet manifest")
}

fn context() -> EmbeddingSpatialContext {
    let image_id = CoordinateFrameId::new("image-pixels").expect("image frame");
    let physical_id = CoordinateFrameId::new("slide-micrometers").expect("physical frame");
    let transform_id = TransformId::new("pixel-to-micrometer").expect("transform");
    let registry = CoordinateRegistry::new(
        vec![
            CoordinateFrame::new(
                image_id.clone(),
                vec![SpatialAxis::X, SpatialAxis::Y],
                CoordinateUnit::Pixel,
                CoordinateSpace::Image(ImageCoordinateConvention::PixelCenterAtInteger),
            )
            .expect("image frame"),
            CoordinateFrame::new(
                physical_id.clone(),
                vec![SpatialAxis::X, SpatialAxis::Y],
                CoordinateUnit::Micrometer,
                CoordinateSpace::Physical,
            )
            .expect("physical frame"),
        ],
        Vec::new(),
        vec![FrameTransform::new(
            transform_id.clone(),
            image_id.clone(),
            physical_id.clone(),
            TransformMatrix::affine_2d([0.5, 0.0, 0.0, 0.0, 0.5, 0.0]).expect("matrix"),
            None,
        )],
        Vec::new(),
    )
    .expect("registry");
    EmbeddingSpatialContext::new(
        &registry,
        image_id,
        physical_id,
        transform_id,
        PositiveRational::new(1, 2).expect("mpp x"),
        PositiveRational::new(1, 2).expect("mpp y"),
        [1_024, 1_024],
        [64, 64],
        [16, 16],
        PatchBoundaryPolicy::FullyContainedOnly,
    )
    .expect("context")
}

fn hierarchy(expected: &ExpectedCellSet) -> CohortHierarchy {
    let patient = HierarchyId::from(PatientId::new("patient").expect("patient"));
    let slide = HierarchyId::from(SlideId::new("slide").expect("slide"));
    let mut nodes = vec![
        HierarchyNode::new(patient.clone(), None, ReplicationRole::BiologicalUnit),
        HierarchyNode::new(
            slide.clone(),
            None,
            ReplicationRole::TechnicalReplicate {
                biological_source: patient,
            },
        ),
    ];
    nodes.extend(expected.cells().iter().cloned().map(|cell_id| {
        HierarchyNode::new(
            HierarchyId::from(cell_id),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    CohortHierarchy::new(nodes, Vec::new()).expect("hierarchy")
}

fn build_fixture(checkpoint_schema: &str, license_availability: LicenseAvailability) -> Fixture {
    build_fixture_with_status(
        checkpoint_schema,
        license_availability,
        FixtureEmbeddingStatus::Present,
    )
}

fn build_fixture_with_status(
    checkpoint_schema: &str,
    license_availability: LicenseAvailability,
    embedding_status: FixtureEmbeddingStatus,
) -> Fixture {
    build_fixture_with_rows(
        checkpoint_schema,
        license_availability,
        1_280,
        vec![(cell("cell-a"), embedding_status)],
    )
}

fn build_fixture_with_rows(
    checkpoint_schema: &str,
    license_availability: LicenseAvailability,
    dimension: u32,
    rows: Vec<(CellId, FixtureEmbeddingStatus)>,
) -> Fixture {
    build_fixture_with_rows_and_sources(
        checkpoint_schema,
        license_availability,
        dimension,
        rows,
        b"source-cells",
        b"source-vectors",
    )
}

fn build_fixture_with_rows_and_sources(
    checkpoint_schema: &str,
    license_availability: LicenseAvailability,
    dimension: u32,
    rows: Vec<(CellId, FixtureEmbeddingStatus)>,
    source_cells_bytes: &[u8],
    source_vectors_bytes: &[u8],
) -> Fixture {
    let root = TempDir::new().expect("temporary store");
    let store =
        LocalArtifactStore::open(root.path(), StoreId::new("local").expect("local store ID"))
            .expect("local store");
    let checkpoint = publish_record(
        &store,
        checkpoint_schema,
        "application/octet-stream",
        b"checkpoint",
        Vec::new(),
        None,
    );
    let source_snapshot = publish_record(
        &store,
        "marklab.source_snapshot",
        "application/octet-stream",
        b"source-snapshot",
        Vec::new(),
        None,
    );
    let license_draft = draft_record(
        "marklab.license_record",
        "text/plain",
        b"license",
        Vec::new(),
        None,
    );
    let license = match license_availability {
        LicenseAvailability::Managed => store
            .publish(&license_draft, |writer| writer.write_all(b"license"))
            .expect("publish license")
            .into_record(),
        LicenseAvailability::CatalogOnly => license_draft,
        LicenseAvailability::NonManagedLocal => {
            let key = "incoming/privacy-sentinel-license";
            std::fs::create_dir_all(root.path().join("incoming"))
                .expect("create non-managed local parent");
            std::fs::write(root.path().join(key), b"license")
                .expect("write non-managed local replica");
            ArtifactRecord::new(
                license_draft.content().clone(),
                license_draft.schema().clone(),
                None,
                Vec::new(),
                BTreeMap::new(),
                vec![ArtifactLocator::new(
                    StoreId::new("local").expect("local store ID"),
                    ArtifactKey::new(key).expect("non-managed local key"),
                    None,
                )
                .expect("non-managed local locator")],
            )
            .expect("non-managed local record")
        }
    };
    let preprocessing = publish_record(
        &store,
        "marklab.embedding_preprocessing",
        "application/json",
        b"preprocessing",
        Vec::new(),
        None,
    );
    let run_config = publish_record(
        &store,
        "marklab.embedding_run_config",
        "application/json",
        b"run-config",
        Vec::new(),
        None,
    );
    let environment = publish_record(
        &store,
        "marklab.execution_environment",
        "application/json",
        b"environment",
        Vec::new(),
        None,
    );
    let converter = publish_record(
        &store,
        "marklab.converter_manifest",
        "application/json",
        b"converter",
        Vec::new(),
        None,
    );
    let source_cells = publish_record(
        &store,
        "marklab.cell_embedding_source_cells",
        "text/csv;profile=marklab-cellvit-he-bundle-v1",
        source_cells_bytes,
        Vec::new(),
        None,
    );
    let source_vectors = publish_record(
        &store,
        "marklab.cell_embedding_source_npy",
        "application/x-npy;profile=marklab-cellvit-he-f4-v1",
        source_vectors_bytes,
        Vec::new(),
        None,
    );

    let expected = ExpectedCellSet::new(
        "all.v1",
        rows.iter().map(|(cell_id, _)| cell_id.clone()).collect(),
    )
    .expect("expected");
    let expected_bytes = expected.to_bytes().expect("expected bytes");
    let expected_record = publish_record(
        &store,
        "marklab.cell_embedding_expected_cells",
        "application/vnd.marklab.embedding-expected-cells.v1",
        &expected_bytes,
        Vec::new(),
        None,
    );
    let identity_map = CellIdentityMap::new(
        source_cells.id(),
        expected_record.id(),
        &expected,
        rows.iter()
            .enumerate()
            .map(|(index, (cell_id, _))| {
                CellIdentityMapEntry::new(format!("source-{index:08}"), cell_id.clone())
                    .expect("identity")
            })
            .collect(),
    )
    .expect("identity map");
    let identity_bytes = identity_map.to_bytes().expect("identity bytes");
    let identity_record = publish_record(
        &store,
        "marklab.cell_embedding_identity_map",
        "application/vnd.marklab.embedding-identity-map.v1",
        &identity_bytes,
        vec![source_cells.id(), expected_record.id()],
        None,
    );
    let context = context();
    let context_bytes = context.to_canonical_json().expect("context bytes");
    let context_record = publish_record(
        &store,
        "marklab.cell_embedding_spatial_context",
        "application/vnd.marklab.embedding-spatial-context.v1+json",
        &context_bytes,
        Vec::new(),
        None,
    );
    let hierarchy = hierarchy(&expected);
    let mut next_embedding_row = 0_u64;
    let row_link_entries = rows
        .iter()
        .enumerate()
        .map(|(index, (cell_id, status))| {
            let source_cell_row = u64::try_from(index).expect("fixture row index");
            match status {
                FixtureEmbeddingStatus::Present => {
                    let entry = CellEmbeddingRowLinkEntry::present(
                        cell_id.clone(),
                        source_cell_row,
                        next_embedding_row,
                    );
                    next_embedding_row += 1;
                    entry
                }
                FixtureEmbeddingStatus::MissingVector => {
                    CellEmbeddingRowLinkEntry::missing_vector(cell_id.clone(), source_cell_row)
                }
                FixtureEmbeddingStatus::ExtractionFailed => {
                    CellEmbeddingRowLinkEntry::extraction_failed(cell_id.clone(), source_cell_row)
                }
                FixtureEmbeddingStatus::QcRejected => {
                    let entry = CellEmbeddingRowLinkEntry::qc_rejected(
                        cell_id.clone(),
                        source_cell_row,
                        next_embedding_row,
                    );
                    next_embedding_row += 1;
                    entry
                }
            }
        })
        .collect();
    let row_link = CellEmbeddingRowLink::new(
        source_cells.id(),
        source_vectors.id(),
        expected_record.id(),
        identity_record.id(),
        converter.id(),
        &expected,
        &hierarchy,
        row_link_entries,
        16 * 1024 * 1024,
    )
    .expect("row link");
    let row_link_record = publish_cell_embedding_row_link_arrow(
        &store,
        &row_link,
        EmbeddingColumnarBudgets::new(
            64 * 1024 * 1024,
            64 * 1024 * 1024,
            64 * 1024 * 1024,
            64 * 1024 * 1024,
        ),
    )
    .expect("publish canonical row link")
    .into_record();

    let model = CellEmbeddingModelProvenance::new(
        "cellvit_sam_h",
        "1.0",
        "sam_h",
        checkpoint.id(),
        checkpoint.content().digest(),
        source_snapshot.id(),
        license.id(),
        "Apache-2.0",
        "doi:10.0000-example",
        "z4",
        32,
    )
    .expect("model provenance");
    let decimal = |value| CanonicalDecimal::new(value).expect("decimal");
    let tensor = CellEmbeddingTensorContract::rgb_he_raw(
        dimension,
        [decimal("0.485"), decimal("0.456"), decimal("0.406")],
        [decimal("0.229"), decimal("0.224"), decimal("0.225")],
    )
    .expect("tensor");
    let execution = CellEmbeddingExecutionProvenance::new(
        preprocessing.id(),
        run_config.id(),
        environment.id(),
        converter.id(),
        "marklab_cellvit_converter",
        "1.0",
        "cellvit_he_bundle",
        "1.0",
    )
    .expect("execution");
    let inputs = CellEmbeddingInputArtifacts::new(
        source_cells.id(),
        source_vectors.id(),
        expected_record.id(),
        identity_record.id(),
        context_record.id(),
        row_link_record.id(),
    );
    let provenance =
        CellEmbeddingProvenance::new(model, tensor, execution, inputs).expect("provenance");
    let provenance_bytes = provenance.to_canonical_json().expect("provenance bytes");
    let provenance_record = publish_record(
        &store,
        "marklab.cell_embedding_provenance",
        "application/vnd.marklab.embedding-provenance.v1+json",
        &provenance_bytes,
        provenance.direct_dependencies().to_vec(),
        None,
    );
    let provenance_artifact_id = provenance_record.id();
    let catalog = ArtifactCatalog::from_records([
        checkpoint,
        source_snapshot,
        license,
        preprocessing,
        run_config,
        environment,
        converter,
        source_cells,
        source_vectors,
        expected_record,
        identity_record,
        context_record,
        row_link_record,
        provenance_record,
    ])
    .expect("artifact catalog");
    Fixture {
        _root: root,
        store,
        catalog,
        provenance,
        provenance_artifact_id,
        expected,
        identity_map,
        context,
        row_link,
        dimension,
    }
}

fn replace_record(
    catalog: &ArtifactCatalog,
    replaced: marklab::ArtifactId,
    replacement: ArtifactRecord,
) -> ArtifactCatalog {
    ArtifactCatalog::from_records(
        catalog
            .iter()
            .filter(|(id, _record)| **id != replaced)
            .map(|(_id, record)| record.clone())
            .chain(std::iter::once(replacement)),
    )
    .expect("replacement catalog")
}

fn provenance_record(
    fixture: &Fixture,
    kind: &str,
    dependencies: Vec<marklab::ArtifactId>,
    semantic_metadata: BTreeMap<String, String>,
    table: Option<TableManifest>,
) -> ArtifactRecord {
    let bytes = fixture
        .provenance
        .to_canonical_json()
        .expect("provenance bytes");
    ArtifactRecord::new(
        ArtifactRef::from_bytes(kind, &bytes).expect("provenance content"),
        ArtifactSchema::new("marklab.cell_embedding_provenance", 1).expect("provenance schema"),
        table,
        dependencies,
        semantic_metadata,
        vec![ArtifactLocator::new(
            StoreId::new("source").expect("source store"),
            ArtifactKey::new("fixtures/privacy-sentinel/provenance-drift").expect("source key"),
            None,
        )
        .expect("source locator")],
    )
    .expect("provenance record")
}

fn record_with_schema<'a>(fixture: &'a Fixture, schema: &str) -> &'a ArtifactRecord {
    fixture
        .catalog
        .iter()
        .find_map(|(_id, record)| (record.schema().id() == schema).then_some(record))
        .expect("record with schema")
}

fn managed_path(root: &TempDir, record: &ArtifactRecord) -> std::path::PathBuf {
    let id = record.id().to_string();
    root.path().join("objects/sha256").join(&id[..2]).join(id)
}

fn verified_graph(fixture: &Fixture) -> VerifiedCellEmbeddingArtifactGraph {
    fixture
        .provenance
        .validate_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected,
            &fixture.identity_map,
            &fixture.context,
            &fixture.row_link,
            &fixture.catalog,
            &fixture.store,
        )
        .expect("verified artifact graph")
}

fn embedding_budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
    )
}

#[cfg(feature = "csv")]
fn all_present_source_csv(row_count: usize) -> Vec<u8> {
    let mut bytes = concat!(
        "cell_id,case_id,specimen_id,timepoint,fragment_id,roi_id,native_row,",
        "embedding_row,x_px,y_px,x_um,y_um,cell_type_id,cell_type_label,",
        "type_probability,nucleus_area_um2,nucleus_perimeter_um,eccentricity,",
        "solidity,circularity,qc_pass,block_500_id,split\r\n"
    )
    .as_bytes()
    .to_vec();
    for row in 0..row_count {
        bytes.extend_from_slice(
            format!(
                "source-{row:08},case,synthetic,time,fragment,roi,{row},{row},1,2,3,4,1,label,0.5,10,5,0.2,0.8,0.7,True,block,train\r\n"
            )
            .as_bytes(),
        );
    }
    bytes
}

#[cfg(feature = "csv")]
fn all_present_source_npy(rows: &[Vec<f32>]) -> Vec<u8> {
    let dictionary = format!(
        "{{'descr': '<f4', 'fortran_order': False, 'shape': ({}, 1280), }}",
        rows.len()
    );
    let padding = (16 - ((10 + dictionary.len() + 1) % 16)) % 16;
    let mut header = dictionary.into_bytes();
    header.resize(header.len() + padding, b' ');
    header.push(b'\n');
    let mut bytes = b"\x93NUMPY\x01\x00".to_vec();
    bytes.extend_from_slice(
        &u16::try_from(header.len())
            .expect("small NPY header")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&header);
    for row in rows {
        for value in row {
            bytes.extend_from_slice(&value.to_bits().to_le_bytes());
        }
    }
    bytes
}

fn write_embedding_table(
    fixture: &Fixture,
    row: CellEmbeddingRow,
) -> (CellEmbeddingTable, Vec<u8>, ArtifactRecord) {
    write_embedding_rows_arrow(fixture, vec![row], embedding_budgets())
}

fn write_embedding_rows_arrow(
    fixture: &Fixture,
    rows: Vec<CellEmbeddingRow>,
    budgets: EmbeddingColumnarBudgets,
) -> (CellEmbeddingTable, Vec<u8>, ArtifactRecord) {
    let expected_record = record_with_schema(fixture, "marklab.cell_embedding_expected_cells");
    let row_link_record = record_with_schema(fixture, "marklab.cell_embedding_row_link");
    let table = CellEmbeddingTable::from_rows(
        fixture.dimension,
        &fixture.expected,
        expected_record.id(),
        fixture.provenance_artifact_id,
        fixture.row_link.logical_digest(),
        rows,
        budgets.maximum_retained_bytes(),
    )
    .expect("embedding table");
    let bindings = CellEmbeddingTablePhysicalBindings::new(
        expected_record.id(),
        fixture.provenance_artifact_id,
        row_link_record.id(),
        fixture.row_link.logical_digest(),
        table.qc_summary().logical_digest(),
    )
    .expect("physical bindings");
    let mut bytes = Vec::new();
    write_cell_embedding_table_arrow(&mut bytes, &table, bindings, budgets)
        .expect("write Arrow table");
    let record = embedding_table_record(fixture, &bytes);
    (table, bytes, record)
}

fn embedding_table_record(fixture: &Fixture, bytes: &[u8]) -> ArtifactRecord {
    let expected_record = record_with_schema(fixture, "marklab.cell_embedding_expected_cells");
    let row_link_record = record_with_schema(fixture, "marklab.cell_embedding_row_link");
    draft_record(
        "marklab.cell_embedding_table",
        "application/vnd.marklab.cell-embedding-table.v1+arrow",
        bytes,
        vec![
            expected_record.id(),
            fixture.provenance_artifact_id,
            row_link_record.id(),
        ],
        Some(embedding_manifest(
            fixture.row_link.row_count(),
            fixture.dimension,
        )),
    )
}

fn write_embedding_table_parquet_fixture(
    fixture: &Fixture,
    row: CellEmbeddingRow,
) -> (CellEmbeddingTable, Vec<u8>, ArtifactRecord) {
    write_embedding_rows_parquet(fixture, vec![row], embedding_budgets())
}

fn write_embedding_rows_parquet(
    fixture: &Fixture,
    rows: Vec<CellEmbeddingRow>,
    budgets: EmbeddingColumnarBudgets,
) -> (CellEmbeddingTable, Vec<u8>, ArtifactRecord) {
    let expected_record = record_with_schema(fixture, "marklab.cell_embedding_expected_cells");
    let row_link_record = record_with_schema(fixture, "marklab.cell_embedding_row_link");
    let table = CellEmbeddingTable::from_rows(
        fixture.dimension,
        &fixture.expected,
        expected_record.id(),
        fixture.provenance_artifact_id,
        fixture.row_link.logical_digest(),
        rows,
        budgets.maximum_retained_bytes(),
    )
    .expect("embedding table");
    let bindings = CellEmbeddingTablePhysicalBindings::new(
        expected_record.id(),
        fixture.provenance_artifact_id,
        row_link_record.id(),
        fixture.row_link.logical_digest(),
        table.qc_summary().logical_digest(),
    )
    .expect("physical bindings");
    let mut bytes = Vec::new();
    write_cell_embedding_table_parquet(&mut bytes, &table, bindings, budgets)
        .expect("write Parquet table");
    let record = embedding_parquet_record(fixture, &bytes);
    (table, bytes, record)
}

fn exact_retained_limit<T: std::fmt::Debug>(
    mut operation: impl FnMut(usize) -> Result<T, EmbeddingColumnarError>,
) -> usize {
    let mut maximum = 0_usize;
    for _ in 0..8 {
        match operation(maximum) {
            Ok(_) => return maximum,
            Err(EmbeddingColumnarError::RetainedByteBudgetExceeded {
                required,
                maximum: observed_maximum,
            }) => {
                assert_eq!(observed_maximum, maximum);
                assert!(required > maximum);
                maximum = required;
            }
            result => panic!("expected retained-budget convergence, observed {result:?}"),
        }
    }
    panic!("retained-budget convergence exceeded eight charged allocation stages")
}

#[test]
fn compact_embedding_artifact_binds_records_shape_dtype_digest_and_qc() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (table, bytes, embedding_record) = write_embedding_table(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    let row_link_record = record_with_schema(&fixture, "marklab.cell_embedding_row_link");
    let embedding_receipt = verify_cell_embedding_table_arrow_bytes(
        &bytes,
        &embedding_record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("verified embedding receipt");
    let row_link_receipt = verify_cell_embedding_row_link_arrow_from_store(
        &fixture.store,
        row_link_record,
        &fixture.expected,
        &fixture.row_link,
        embedding_budgets(),
    )
    .expect("verified row-link receipt");
    let artifact = CellEmbeddingArtifact::new(embedding_receipt, row_link_receipt, verified)
        .expect("embedding artifact binding");

    assert_eq!(artifact.embedding_artifact_id(), embedding_record.id());
    assert_eq!(artifact.row_link_artifact_id(), row_link_record.id());
    assert_eq!(
        artifact.provenance_artifact_id(),
        fixture.provenance_artifact_id
    );
    assert_eq!(artifact.row_count(), 1);
    assert_eq!(artifact.dimension(), 1_280);
    assert_eq!(artifact.dtype(), EmbeddingDtype::F32);
    assert_eq!(
        artifact.logical_digest(),
        table.qc_summary().logical_digest()
    );
    assert_eq!(artifact.qc_summary(), table.qc_summary());

    let (parquet_table, parquet_bytes, parquet_record) = write_embedding_table_parquet_fixture(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    let parquet_receipt = verify_cell_embedding_table_parquet_bytes(
        &parquet_bytes,
        &parquet_record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("verified Parquet embedding receipt");
    let parquet_artifact = CellEmbeddingArtifact::new(parquet_receipt, row_link_receipt, verified)
        .expect("Parquet embedding artifact binding");
    assert_eq!(parquet_artifact.qc_summary(), parquet_table.qc_summary());
    assert_eq!(parquet_artifact.logical_digest(), artifact.logical_digest());
    assert_ne!(
        parquet_artifact.embedding_artifact_id(),
        artifact.embedding_artifact_id()
    );

    let wrong_row_link = record_with_schema(&fixture, "marklab.cell_embedding_expected_cells");
    assert!(matches!(
        verify_cell_embedding_row_link_arrow_from_store(
            &fixture.store,
            wrong_row_link,
            &fixture.expected,
            &fixture.row_link,
            embedding_budgets(),
        ),
        Err(VerifiedReaderError::Callback(
            EmbeddingColumnarError::ArtifactBindingMismatch
        ))
    ));

    let unrelated = build_fixture_with_status(
        "marklab.model_checkpoint",
        LicenseAvailability::Managed,
        FixtureEmbeddingStatus::MissingVector,
    );
    let unrelated_row_link = record_with_schema(&unrelated, "marklab.cell_embedding_row_link");
    let unrelated_receipt = verify_cell_embedding_row_link_arrow_from_store(
        &unrelated.store,
        unrelated_row_link,
        &unrelated.expected,
        &unrelated.row_link,
        embedding_budgets(),
    )
    .expect("unrelated verified row-link receipt");
    assert_eq!(
        CellEmbeddingArtifact::new(embedding_receipt, unrelated_receipt, verified),
        Err(marklab::EmbeddingError::ArtifactBindingMismatch)
    );
}

#[test]
fn verified_graph_rejects_embedding_dimension_drift_from_provenance() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let expected_record = record_with_schema(&fixture, "marklab.cell_embedding_expected_cells");
    let row_link_record = record_with_schema(&fixture, "marklab.cell_embedding_row_link");
    let table = CellEmbeddingTable::from_rows(
        1,
        &fixture.expected,
        expected_record.id(),
        fixture.provenance_artifact_id,
        fixture.row_link.logical_digest(),
        vec![CellEmbeddingRow::present(cell("cell-a"), vec![0.25])],
        1_024,
    )
    .expect("one-dimensional table");
    let bindings = CellEmbeddingTablePhysicalBindings::new(
        expected_record.id(),
        fixture.provenance_artifact_id,
        row_link_record.id(),
        fixture.row_link.logical_digest(),
        table.qc_summary().logical_digest(),
    )
    .expect("one-dimensional bindings");
    let mut bytes = Vec::new();
    write_cell_embedding_table_arrow(&mut bytes, &table, bindings, embedding_budgets())
        .expect("one-dimensional Arrow table");
    let record = draft_record(
        "marklab.cell_embedding_table",
        "application/vnd.marklab.cell-embedding-table.v1+arrow",
        &bytes,
        vec![
            expected_record.id(),
            fixture.provenance_artifact_id,
            row_link_record.id(),
        ],
        Some(embedding_manifest(1, 1)),
    );

    assert_eq!(
        verify_cell_embedding_table_arrow_bytes(
            &bytes,
            &record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::ArtifactBindingMismatch)
    );
}

#[test]
fn streaming_arrow_and_parquet_scans_match_materialized_qc_without_retaining_the_table() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let row = || CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]);
    let (arrow_table, arrow_bytes, arrow_draft) = write_embedding_table(&fixture, row());
    let arrow_record = fixture
        .store
        .publish(&arrow_draft, |writer| writer.write_all(&arrow_bytes))
        .expect("publish Arrow table")
        .into_record();
    let (parquet_table, parquet_bytes, parquet_draft) =
        write_embedding_table_parquet_fixture(&fixture, row());
    let parquet_record = fixture
        .store
        .publish(&parquet_draft, |writer| writer.write_all(&parquet_bytes))
        .expect("publish Parquet table")
        .into_record();

    let arrow_borrowed = scan_cell_embedding_table_arrow_bytes(
        &arrow_bytes,
        &arrow_draft,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("borrowed Arrow scan");
    let arrow_managed = scan_cell_embedding_table_arrow_from_store(
        &fixture.store,
        &arrow_record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("managed Arrow scan");
    let parquet_borrowed = scan_cell_embedding_table_parquet_bytes(
        &parquet_bytes,
        &parquet_draft,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("borrowed Parquet scan");
    let parquet_managed = scan_cell_embedding_table_parquet_from_store(
        &fixture.store,
        &parquet_record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("managed Parquet scan");

    assert_eq!(arrow_borrowed, arrow_table.qc_summary());
    assert_eq!(arrow_managed, arrow_borrowed);
    assert_eq!(parquet_borrowed, parquet_table.qc_summary());
    assert_eq!(parquet_managed, parquet_borrowed);
    assert_eq!(parquet_borrowed, arrow_borrowed);
}

#[test]
fn managed_embedding_scans_report_store_integrity_before_columnar_callbacks() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let graph = verified_graph(&fixture);
    let row = || CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]);
    let (_, arrow_bytes, arrow_draft) = write_embedding_table(&fixture, row());
    let arrow_record = fixture
        .store
        .publish(&arrow_draft, |writer| writer.write_all(&arrow_bytes))
        .expect("publish Arrow table")
        .into_record();
    let (_, parquet_bytes, parquet_draft) = write_embedding_table_parquet_fixture(&fixture, row());
    let parquet_record = fixture
        .store
        .publish(&parquet_draft, |writer| writer.write_all(&parquet_bytes))
        .expect("publish Parquet table")
        .into_record();

    let corrupt = |record: &ArtifactRecord, bytes: &[u8]| {
        let mut corrupted = bytes.to_vec();
        corrupted[0] ^= 1;
        fs::write(managed_path(&fixture._root, record), corrupted).expect("corrupt managed object");
    };
    corrupt(&arrow_record, &arrow_bytes);
    assert!(matches!(
        scan_cell_embedding_table_arrow_from_store(
            &fixture.store,
            &arrow_record,
            &fixture.expected,
            &fixture.row_link,
            graph,
            embedding_budgets(),
        ),
        Err(VerifiedReaderError::Store(
            ArtifactStoreError::ContentIntegrity { .. }
        ))
    ));

    corrupt(&parquet_record, &parquet_bytes);
    assert!(matches!(
        scan_cell_embedding_table_parquet_from_store(
            &fixture.store,
            &parquet_record,
            &fixture.expected,
            &fixture.row_link,
            graph,
            embedding_budgets(),
        ),
        Err(VerifiedReaderError::Store(
            ArtifactStoreError::ContentIntegrity { .. }
        ))
    ));
}

#[test]
fn every_embedding_status_has_identical_domain_arrow_and_parquet_qc() {
    let rows = vec![
        (cell("cell-a"), FixtureEmbeddingStatus::Present),
        (cell("cell-b"), FixtureEmbeddingStatus::MissingVector),
        (cell("cell-c"), FixtureEmbeddingStatus::ExtractionFailed),
        (cell("cell-d"), FixtureEmbeddingStatus::QcRejected),
    ];
    let fixture = build_fixture_with_rows(
        "marklab.model_checkpoint",
        LicenseAvailability::Managed,
        1_280,
        rows.clone(),
    );
    let verified = verified_graph(&fixture);
    let domain_rows = rows
        .into_iter()
        .map(|(cell_id, status)| status.table_row(cell_id, fixture.dimension))
        .collect::<Vec<_>>();
    let (arrow_table, arrow_bytes, arrow_record) =
        write_embedding_rows_arrow(&fixture, domain_rows.clone(), embedding_budgets());
    let (parquet_table, parquet_bytes, parquet_record) =
        write_embedding_rows_parquet(&fixture, domain_rows, embedding_budgets());

    let arrow_qc = scan_cell_embedding_table_arrow_bytes(
        &arrow_bytes,
        &arrow_record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("mixed Arrow scan");
    let parquet_qc = scan_cell_embedding_table_parquet_bytes(
        &parquet_bytes,
        &parquet_record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("mixed Parquet scan");

    assert_eq!(arrow_qc, arrow_table.qc_summary());
    assert_eq!(parquet_qc, parquet_table.qc_summary());
    assert_eq!(parquet_qc, arrow_qc);
    assert_eq!(arrow_qc.row_count(), 4);
    assert_eq!(arrow_qc.present_count(), 1);
    assert_eq!(arrow_qc.missing_vector_count(), 1);
    assert_eq!(arrow_qc.extraction_failed_count(), 1);
    assert_eq!(arrow_qc.qc_rejected_count(), 1);
    assert_eq!(arrow_qc.all_zero_present_count(), 0);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(12))]

    #[test]
    fn property_generated_domain_arrow_and_parquet_rows_are_semantically_identical(
        raw_statuses in any::<[u8; 4]>(),
        seeds in any::<[u16; 4]>(),
    ) {
        let cells = [cell("cell-a"), cell("cell-b"), cell("cell-c"), cell("cell-d")];
        let statuses = raw_statuses.map(|code| match code % 4 {
            0 => FixtureEmbeddingStatus::Present,
            1 => FixtureEmbeddingStatus::MissingVector,
            2 => FixtureEmbeddingStatus::ExtractionFailed,
            _ => FixtureEmbeddingStatus::QcRejected,
        });
        let fixture_rows = cells
            .iter()
            .cloned()
            .zip(statuses)
            .collect::<Vec<_>>();
        let fixture = build_fixture_with_rows(
            "marklab.model_checkpoint",
            LicenseAvailability::Managed,
            1_280,
            fixture_rows.clone(),
        );
        let graph = verified_graph(&fixture);
        let domain_rows = fixture_rows
            .into_iter()
            .zip(seeds)
            .map(|((cell_id, status), seed)| match status {
                FixtureEmbeddingStatus::Present => CellEmbeddingRow::present(
                    cell_id,
                    (0..1_280)
                        .map(|column| {
                            if column == 0 && seed % 2 == 0 {
                                -0.0
                            } else {
                                let column = u16::try_from(column).expect("property column");
                                f32::from(seed.wrapping_add(column)) / 16.0 + 0.25
                            }
                        })
                        .collect(),
                ),
                status => CellEmbeddingRow::non_present(cell_id, status.domain_status())
                    .expect("non-present property row"),
            })
            .collect::<Vec<_>>();
        let (domain, arrow_bytes, arrow_record) = write_embedding_rows_arrow(
            &fixture,
            domain_rows.clone(),
            embedding_budgets(),
        );
        let (_, parquet_bytes, parquet_record) = write_embedding_rows_parquet(
            &fixture,
            domain_rows,
            embedding_budgets(),
        );
        let arrow_qc = scan_cell_embedding_table_arrow_bytes(
            &arrow_bytes,
            &arrow_record,
            &fixture.expected,
            &fixture.row_link,
            graph,
            embedding_budgets(),
        )
        .expect("property Arrow scan");
        let parquet_qc = scan_cell_embedding_table_parquet_bytes(
            &parquet_bytes,
            &parquet_record,
            &fixture.expected,
            &fixture.row_link,
            graph,
            embedding_budgets(),
        )
        .expect("property Parquet scan");
        let arrow_table = read_cell_embedding_table_arrow_bytes(
            &arrow_bytes,
            &arrow_record,
            &fixture.expected,
            &fixture.row_link,
            graph,
            embedding_budgets(),
        )
        .expect("property Arrow read");
        let parquet_table = read_cell_embedding_table_parquet_bytes(
            &parquet_bytes,
            &parquet_record,
            &fixture.expected,
            &fixture.row_link,
            graph,
            embedding_budgets(),
        )
        .expect("property Parquet read");

        prop_assert_eq!(arrow_qc, domain.qc_summary());
        prop_assert_eq!(parquet_qc, arrow_qc);
        prop_assert_eq!(&arrow_table, &domain);
        prop_assert_eq!(&parquet_table, &arrow_table);
    }
}

#[cfg(feature = "csv")]
#[test]
fn all_present_npy_csv_domain_arrow_and_parquet_paths_share_exact_semantics() {
    let mut source_rows = vec![
        (0..1_280)
            .map(|column| column as f32 + 0.25)
            .collect::<Vec<_>>(),
        (0..1_280)
            .map(|column| -(column as f32) - 0.5)
            .collect::<Vec<_>>(),
    ];
    source_rows[0][3] = -0.0;
    source_rows[1][7] = -0.0;
    let expected_values = source_rows
        .iter()
        .flatten()
        .map(|value| if *value == 0.0 { 0.0 } else { *value })
        .collect::<Vec<_>>();
    let csv_bytes = all_present_source_csv(2);
    let npy_bytes = all_present_source_npy(&source_rows);
    let fixture = build_fixture_with_rows_and_sources(
        "marklab.model_checkpoint",
        LicenseAvailability::Managed,
        1_280,
        vec![
            (cell("cell-a"), FixtureEmbeddingStatus::Present),
            (cell("cell-b"), FixtureEmbeddingStatus::Present),
        ],
        &csv_bytes,
        &npy_bytes,
    );
    let graph = verified_graph(&fixture);
    let source_bindings = CellVitHeArtifactBindings::from_records(
        record_with_schema(&fixture, "marklab.cell_embedding_source_cells").clone(),
        record_with_schema(&fixture, "marklab.cell_embedding_source_npy").clone(),
        record_with_schema(&fixture, "marklab.cell_embedding_expected_cells").clone(),
        record_with_schema(&fixture, "marklab.cell_embedding_identity_map").clone(),
        record_with_schema(&fixture, "marklab.converter_manifest").clone(),
    )
    .expect("source artifact bindings");
    let hierarchy = hierarchy(&fixture.expected);
    let source_budgets = SourceBundleBudgets::new(
        u64::try_from(npy_bytes.len()).expect("NPY length"),
        u64::try_from(csv_bytes.len()).expect("CSV length"),
        64 * 1024,
        8 * 1024 * 1024,
        2 * 1_280 * 4,
    );
    let request = CellVitHeImportRequest::new(
        &fixture.expected,
        &fixture.identity_map,
        &hierarchy,
        &source_bindings,
        source_budgets,
    );
    let candidate = import_cellvit_he_bundle_bytes(&npy_bytes, &csv_bytes, request)
        .expect("bounded NPY/CSV import");
    assert_eq!(candidate.canonical_values(), expected_values);
    assert_eq!(candidate.row_link(), &fixture.row_link);
    let imported = candidate
        .finalize(&fixture.expected, &graph)
        .expect("verified source finalization");
    let table = imported.table();
    assert_eq!(
        table.scan_qc(1).expect("single-row blocks"),
        table.qc_summary()
    );

    let expected_record = record_with_schema(&fixture, "marklab.cell_embedding_expected_cells");
    let row_link_record = record_with_schema(&fixture, "marklab.cell_embedding_row_link");
    let physical_bindings = CellEmbeddingTablePhysicalBindings::new(
        expected_record.id(),
        fixture.provenance_artifact_id,
        row_link_record.id(),
        fixture.row_link.logical_digest(),
        table.qc_summary().logical_digest(),
    )
    .expect("physical table bindings");
    let mut arrow_bytes = Vec::new();
    write_cell_embedding_table_arrow(
        &mut arrow_bytes,
        table,
        physical_bindings,
        embedding_budgets(),
    )
    .expect("write imported Arrow table");
    let arrow_record = embedding_table_record(&fixture, &arrow_bytes);
    let mut parquet_bytes = Vec::new();
    write_cell_embedding_table_parquet(
        &mut parquet_bytes,
        table,
        physical_bindings,
        embedding_budgets(),
    )
    .expect("write imported Parquet table");
    let parquet_record = embedding_parquet_record(&fixture, &parquet_bytes);

    let arrow_qc = scan_cell_embedding_table_arrow_bytes(
        &arrow_bytes,
        &arrow_record,
        &fixture.expected,
        &fixture.row_link,
        graph,
        embedding_budgets(),
    )
    .expect("scan imported Arrow table");
    let parquet_qc = scan_cell_embedding_table_parquet_bytes(
        &parquet_bytes,
        &parquet_record,
        &fixture.expected,
        &fixture.row_link,
        graph,
        embedding_budgets(),
    )
    .expect("scan imported Parquet table");
    let arrow_table = read_cell_embedding_table_arrow_bytes(
        &arrow_bytes,
        &arrow_record,
        &fixture.expected,
        &fixture.row_link,
        graph,
        embedding_budgets(),
    )
    .expect("read imported Arrow table");
    let parquet_table = read_cell_embedding_table_parquet_bytes(
        &parquet_bytes,
        &parquet_record,
        &fixture.expected,
        &fixture.row_link,
        graph,
        embedding_budgets(),
    )
    .expect("read imported Parquet table");

    assert_eq!(arrow_qc, table.qc_summary());
    assert_eq!(parquet_qc, arrow_qc);
    for observed in [table, &arrow_table, &parquet_table] {
        let mut values = Vec::with_capacity(expected_values.len());
        for row in 0..observed.row_count() {
            values.extend_from_slice(
                observed
                    .row(row)
                    .expect("canonical row")
                    .vector()
                    .expect("all-present vector"),
            );
        }
        assert_eq!(values, expected_values);
    }
}

#[test]
fn multi_group_scans_succeed_at_a_retained_limit_that_rejects_materialization() {
    let rows = (0..8_193)
        .map(|index| {
            let status = match index % 4 {
                0 => FixtureEmbeddingStatus::Present,
                1 => FixtureEmbeddingStatus::MissingVector,
                2 => FixtureEmbeddingStatus::ExtractionFailed,
                _ => FixtureEmbeddingStatus::QcRejected,
            };
            (cell(&format!("cell-{index:08}")), status)
        })
        .collect::<Vec<_>>();
    let fixture = build_fixture_with_rows(
        "marklab.model_checkpoint",
        LicenseAvailability::Managed,
        1_280,
        rows.clone(),
    );
    let verified = verified_graph(&fixture);
    let domain_rows = rows
        .into_iter()
        .map(|(cell_id, status)| status.table_row(cell_id, fixture.dimension))
        .collect::<Vec<_>>();
    let generous = EmbeddingColumnarBudgets::new(
        512 * 1024 * 1024,
        512 * 1024 * 1024,
        512 * 1024 * 1024,
        512 * 1024 * 1024,
    );
    let (_, arrow_bytes, arrow_record) =
        write_embedding_rows_arrow(&fixture, domain_rows.clone(), generous);
    let (_, parquet_bytes, parquet_record) =
        write_embedding_rows_parquet(&fixture, domain_rows, generous);

    let arrow_retained = exact_retained_limit(|maximum_retained_bytes| {
        scan_cell_embedding_table_arrow_bytes(
            &arrow_bytes,
            &arrow_record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            EmbeddingColumnarBudgets::new(
                generous.maximum_file_bytes(),
                maximum_retained_bytes,
                generous.maximum_row_group_bytes(),
                generous.maximum_decoded_bytes(),
            ),
        )
    });
    let parquet_retained = exact_retained_limit(|maximum_retained_bytes| {
        scan_cell_embedding_table_parquet_bytes(
            &parquet_bytes,
            &parquet_record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            EmbeddingColumnarBudgets::new(
                generous.maximum_file_bytes(),
                maximum_retained_bytes,
                generous.maximum_row_group_bytes(),
                generous.maximum_decoded_bytes(),
            ),
        )
    });

    for (retained, result) in [
        (
            arrow_retained,
            read_cell_embedding_table_arrow_bytes(
                &arrow_bytes,
                &arrow_record,
                &fixture.expected,
                &fixture.row_link,
                verified,
                EmbeddingColumnarBudgets::new(
                    generous.maximum_file_bytes(),
                    arrow_retained,
                    generous.maximum_row_group_bytes(),
                    generous.maximum_decoded_bytes(),
                ),
            ),
        ),
        (
            parquet_retained,
            read_cell_embedding_table_parquet_bytes(
                &parquet_bytes,
                &parquet_record,
                &fixture.expected,
                &fixture.row_link,
                verified,
                EmbeddingColumnarBudgets::new(
                    generous.maximum_file_bytes(),
                    parquet_retained,
                    generous.maximum_row_group_bytes(),
                    generous.maximum_decoded_bytes(),
                ),
            ),
        ),
    ] {
        assert!(matches!(
            result,
            Err(EmbeddingColumnarError::RetainedByteBudgetExceeded { required, maximum })
                if maximum == retained && required > maximum
        ));
    }
}

#[test]
fn arrow_materialization_charges_the_decoded_batch_alongside_the_final_table() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (table, bytes, draft) = write_embedding_table(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    let expected_record = record_with_schema(&fixture, "marklab.cell_embedding_expected_cells");
    let row_link_record = record_with_schema(&fixture, "marklab.cell_embedding_row_link");
    let bindings = CellEmbeddingTablePhysicalBindings::new(
        expected_record.id(),
        fixture.provenance_artifact_id,
        row_link_record.id(),
        fixture.row_link.logical_digest(),
        table.qc_summary().logical_digest(),
    )
    .expect("physical bindings");

    let mut retained_preflight_bytes = 0_usize;
    loop {
        let budgets = EmbeddingColumnarBudgets::new(
            8 * 1024 * 1024,
            retained_preflight_bytes,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
        );
        match preflight_cell_embedding_table_arrow_bytes(
            &bytes,
            &fixture.expected,
            bindings,
            budgets,
        ) {
            Ok(_) => break,
            Err(EmbeddingColumnarError::RetainedByteBudgetExceeded { required, maximum }) => {
                assert!(required > maximum);
                retained_preflight_bytes = required;
            }
            result => panic!("expected retained preflight edge, observed {result:?}"),
        }
    }

    let final_table_bytes = fixture
        .expected
        .cells()
        .len()
        .checked_mul(fixture.dimension as usize)
        .and_then(|components| components.checked_mul(size_of::<f32>()))
        .and_then(|value_bytes| {
            value_bytes.checked_add(
                fixture.expected.cells().len()
                    * (size_of::<CellId>() + size_of::<EmbeddingStatus>()),
            )
        })
        .and_then(|retained| {
            fixture
                .expected
                .cells()
                .iter()
                .try_fold(retained, |total, cell_id| {
                    total.checked_add(cell_id.as_str().len())
                })
        })
        .expect("final table retained bytes");
    let previously_undercounted_peak = retained_preflight_bytes + final_table_bytes;
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &bytes,
            &draft,
            &fixture.expected,
            &fixture.row_link,
            verified,
            EmbeddingColumnarBudgets::new(
                8 * 1024 * 1024,
                previously_undercounted_peak,
                8 * 1024 * 1024,
                8 * 1024 * 1024,
            ),
        ),
        Err(EmbeddingColumnarError::RetainedByteBudgetExceeded {
            required,
            maximum,
        }) if required > maximum && maximum == previously_undercounted_peak
    ));
}

fn embedding_parquet_record(fixture: &Fixture, bytes: &[u8]) -> ArtifactRecord {
    let expected_record = record_with_schema(fixture, "marklab.cell_embedding_expected_cells");
    let row_link_record = record_with_schema(fixture, "marklab.cell_embedding_row_link");
    draft_record(
        "marklab.cell_embedding_table",
        "application/vnd.marklab.cell-embedding-table.v1+parquet",
        bytes,
        vec![
            expected_record.id(),
            fixture.provenance_artifact_id,
            row_link_record.id(),
        ],
        Some(embedding_parquet_manifest(
            fixture.row_link.row_count(),
            fixture.dimension,
        )),
    )
}

fn first_embedding_buffer_offset(bytes: &[u8], buffer_index: usize) -> usize {
    let footer_length_offset = bytes.len() - 10;
    let footer_length = i32::from_le_bytes(
        bytes[footer_length_offset..footer_length_offset + 4]
            .try_into()
            .expect("footer length"),
    ) as usize;
    let footer_start = bytes.len() - 10 - footer_length;
    let footer = arrow::ipc::root_as_footer(&bytes[footer_start..footer_start + footer_length])
        .expect("footer");
    let block = footer.recordBatches().expect("batches").get(0);
    let message_start = block.offset() as usize;
    let body_start = message_start + block.metaDataLength() as usize;
    let message = arrow::ipc::root_as_message(
        &bytes[message_start + 8..message_start + block.metaDataLength() as usize],
    )
    .expect("message");
    let buffer = message
        .header_as_record_batch()
        .expect("batch")
        .buffers()
        .expect("buffers")
        .get(buffer_index);
    body_start + buffer.offset() as usize
}

fn first_footer_record_block_offset(bytes: &[u8]) -> usize {
    let footer_length_offset = bytes.len() - 10;
    let footer_length = i32::from_le_bytes(
        bytes[footer_length_offset..footer_length_offset + 4]
            .try_into()
            .expect("footer length"),
    ) as usize;
    let footer_start = bytes.len() - 10 - footer_length;
    let footer_bytes = &bytes[footer_start..footer_start + footer_length];
    let footer = arrow::ipc::root_as_footer(footer_bytes).expect("footer");
    let raw = footer.recordBatches().expect("batches").get(0).0;
    footer_start
        + footer_bytes
            .windows(raw.len())
            .position(|window| window == raw)
            .expect("record block")
}

fn replace_all_same_length(bytes: &mut [u8], from: &[u8], to: &[u8]) -> usize {
    assert_eq!(from.len(), to.len());
    let offsets = bytes
        .windows(from.len())
        .enumerate()
        .filter_map(|(index, window)| (window == from).then_some(index))
        .collect::<Vec<_>>();
    for offset in &offsets {
        bytes[*offset..*offset + to.len()].copy_from_slice(to);
    }
    offsets.len()
}

#[test]
fn artifact_graph_requires_exact_records_dependencies_payloads_and_store_bytes() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = fixture
        .provenance
        .validate_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected,
            &fixture.identity_map,
            &fixture.context,
            &fixture.row_link,
            &fixture.catalog,
            &fixture.store,
        )
        .expect("verified artifact graph");
    assert_eq!(
        verified.provenance_artifact_id(),
        fixture.provenance_artifact_id
    );
    assert_eq!(verified.dependency_count(), 13);
}

#[test]
fn verified_graph_gates_full_arrow_table_materialization() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = fixture
        .provenance
        .validate_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected,
            &fixture.identity_map,
            &fixture.context,
            &fixture.row_link,
            &fixture.catalog,
            &fixture.store,
        )
        .expect("verified artifact graph");
    let expected_record = record_with_schema(&fixture, "marklab.cell_embedding_expected_cells");
    let row_link_record = record_with_schema(&fixture, "marklab.cell_embedding_row_link");
    let table = CellEmbeddingTable::from_rows(
        1_280,
        &fixture.expected,
        expected_record.id(),
        fixture.provenance_artifact_id,
        fixture.row_link.logical_digest(),
        vec![CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280])],
        1_000_000,
    )
    .expect("embedding table");
    let bindings = CellEmbeddingTablePhysicalBindings::new(
        expected_record.id(),
        fixture.provenance_artifact_id,
        row_link_record.id(),
        fixture.row_link.logical_digest(),
        table.qc_summary().logical_digest(),
    )
    .expect("physical bindings");
    let budgets = EmbeddingColumnarBudgets::new(
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
    );
    let mut bytes = Vec::new();
    write_cell_embedding_table_arrow(&mut bytes, &table, bindings, budgets)
        .expect("write Arrow table");
    let table_record = draft_record(
        "marklab.cell_embedding_table",
        "application/vnd.marklab.cell-embedding-table.v1+arrow",
        &bytes,
        vec![
            expected_record.id(),
            fixture.provenance_artifact_id,
            row_link_record.id(),
        ],
        Some(embedding_manifest(1, 1_280)),
    );

    let decoded = read_cell_embedding_table_arrow_bytes(
        &bytes,
        &table_record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        budgets,
    )
    .expect("read verified Arrow table");
    assert_eq!(decoded, table);

    let too_small = EmbeddingColumnarBudgets::new(
        bytes.len() as u64 - 1,
        budgets.maximum_retained_bytes(),
        budgets.maximum_row_group_bytes(),
        budgets.maximum_decoded_bytes(),
    );
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &bytes,
            row_link_record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            too_small,
        ),
        Err(EmbeddingColumnarError::FileByteBudgetExceeded { .. })
    ));
}

#[test]
fn verified_store_arrow_reader_matches_borrowed_bytes_and_checks_budget_first() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (expected_table, bytes, draft) = write_embedding_table(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    let record = fixture
        .store
        .publish(&draft, |writer| writer.write_all(&bytes))
        .expect("publish embedding table")
        .into_record();

    let decoded = read_cell_embedding_table_arrow_from_store(
        &fixture.store,
        &record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("read managed Arrow table");
    assert_eq!(decoded, expected_table);

    let too_small = EmbeddingColumnarBudgets::new(
        bytes.len() as u64 - 1,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
    );
    let unlocated = embedding_table_record(&fixture, &bytes);
    assert!(matches!(
        read_cell_embedding_table_arrow_from_store(
            &fixture.store,
            &unlocated,
            &fixture.expected,
            &fixture.row_link,
            verified,
            too_small,
        ),
        Err(VerifiedReaderError::Callback(
            EmbeddingColumnarError::FileByteBudgetExceeded { .. }
        ))
    ));
}

#[test]
fn verified_parquet_readers_match_the_domain_table_and_check_budget_first() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (expected_table, bytes, draft) = write_embedding_table_parquet_fixture(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );

    let borrowed = read_cell_embedding_table_parquet_bytes(
        &bytes,
        &draft,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("read borrowed Parquet table");
    assert_eq!(borrowed, expected_table);

    let mut different_same_length_content = bytes.clone();
    different_same_length_content[4] ^= 1;
    let forged_record = draft_record(
        "marklab.cell_embedding_table",
        "application/vnd.marklab.cell-embedding-table.v1+parquet",
        &different_same_length_content,
        draft.dependencies().to_vec(),
        Some(embedding_parquet_manifest(1, 1_280)),
    );
    assert!(matches!(
        read_cell_embedding_table_parquet_bytes(
            &bytes,
            &forged_record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::ArtifactBindingMismatch)
    ));

    let record = fixture
        .store
        .publish(&draft, |writer| writer.write_all(&bytes))
        .expect("publish embedding Parquet")
        .into_record();
    let managed = read_cell_embedding_table_parquet_from_store(
        &fixture.store,
        &record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("read managed Parquet table");
    assert_eq!(managed, expected_table);

    let too_small = EmbeddingColumnarBudgets::new(
        bytes.len() as u64 - 1,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
    );
    assert!(matches!(
        read_cell_embedding_table_parquet_bytes(
            &bytes,
            &draft,
            &fixture.expected,
            &fixture.row_link,
            verified,
            too_small,
        ),
        Err(EmbeddingColumnarError::FileByteBudgetExceeded { .. })
    ));
    assert!(matches!(
        read_cell_embedding_table_parquet_from_store(
            &fixture.store,
            &record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            too_small,
        ),
        Err(VerifiedReaderError::Callback(
            EmbeddingColumnarError::FileByteBudgetExceeded { .. }
        ))
    ));
}

#[test]
fn borrowed_and_managed_parquet_paths_reject_the_same_hostile_page_header() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (_, mut bytes, _) = write_embedding_table_parquet_fixture(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    assert_eq!(bytes[4] & 0x0f, 5, "page type is i32");
    bytes[4] = (bytes[4] & 0xf0) | 8;
    let expected_record = record_with_schema(&fixture, "marklab.cell_embedding_expected_cells");
    let row_link_record = record_with_schema(&fixture, "marklab.cell_embedding_row_link");
    let draft = draft_record(
        "marklab.cell_embedding_table",
        "application/vnd.marklab.cell-embedding-table.v1+parquet",
        &bytes,
        vec![
            expected_record.id(),
            fixture.provenance_artifact_id,
            row_link_record.id(),
        ],
        Some(embedding_parquet_manifest(1, 1_280)),
    );
    let record = fixture
        .store
        .publish(&draft, |writer| writer.write_all(&bytes))
        .expect("publish hostile Parquet")
        .into_record();
    let borrowed = read_cell_embedding_table_parquet_bytes(
        &bytes,
        &draft,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect_err("borrowed hostile page");
    let managed = match read_cell_embedding_table_parquet_from_store(
        &fixture.store,
        &record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    ) {
        Err(VerifiedReaderError::Callback(error)) => error,
        result => panic!("expected managed callback failure, observed {result:?}"),
    };
    assert_eq!(managed, borrowed);
    let borrowed_scan = scan_cell_embedding_table_parquet_bytes(
        &bytes,
        &draft,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect_err("borrowed hostile scan");
    let managed_scan = match scan_cell_embedding_table_parquet_from_store(
        &fixture.store,
        &record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    ) {
        Err(VerifiedReaderError::Callback(error)) => error,
        result => panic!("expected managed scan callback failure, observed {result:?}"),
    };
    assert_eq!(borrowed_scan, borrowed);
    assert_eq!(managed_scan, borrowed);
    assert!(matches!(
        managed,
        EmbeddingColumnarError::Parquet {
            reason: marklab::ParquetFailure::InvalidPageHeader,
        }
    ));
}

#[test]
fn parquet_reader_rejects_nonfinite_status_and_cell_drift_after_raw_preflight() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (_, canonical, _) = write_embedding_table_parquet_fixture(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );

    let value_pattern = 0.25_f32.to_le_bytes();
    let value_offset = canonical
        .windows(value_pattern.len())
        .position(|window| window == value_pattern)
        .expect("first plain float");
    let mut nonfinite = canonical.clone();
    nonfinite[value_offset..value_offset + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(matches!(
        read_cell_embedding_table_parquet_bytes(
            &nonfinite,
            &embedding_parquet_record(&fixture, &nonfinite),
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Parquet {
            reason: marklab::ParquetFailure::InvalidComponent,
        })
    ));

    let mut invalid_status = canonical.clone();
    assert_eq!(
        replace_all_same_length(&mut invalid_status, b"present", b"invalid"),
        1
    );
    let error = read_cell_embedding_table_parquet_bytes(
        &invalid_status,
        &embedding_parquet_record(&fixture, &invalid_status),
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect_err("invalid status");
    assert!(matches!(
        error,
        EmbeddingColumnarError::Parquet {
            reason: marklab::ParquetFailure::InvalidStatus,
        }
    ));
    assert!(!error.to_string().contains("invalid"));

    let mut wrong_cell = canonical;
    assert_eq!(
        replace_all_same_length(&mut wrong_cell, b"cell-a", b"cell-z"),
        1
    );
    assert!(matches!(
        read_cell_embedding_table_parquet_bytes(
            &wrong_cell,
            &embedding_parquet_record(&fixture, &wrong_cell),
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Parquet {
            reason: marklab::ParquetFailure::InvalidCellOrder,
        })
    ));

    let missing = build_fixture_with_status(
        "marklab.model_checkpoint",
        LicenseAvailability::Managed,
        FixtureEmbeddingStatus::MissingVector,
    );
    let missing_verified = verified_graph(&missing);
    let (_, mut hidden_nonzero, _) = write_embedding_table_parquet_fixture(
        &missing,
        CellEmbeddingRow::non_present(cell("cell-a"), EmbeddingStatus::MissingVector)
            .expect("missing row"),
    );
    let zero_run = hidden_nonzero
        .windows(64)
        .position(|window| window.iter().all(|byte| *byte == 0))
        .expect("plain zero vector run");
    hidden_nonzero[zero_run..zero_run + 4].copy_from_slice(&1.0_f32.to_le_bytes());
    assert!(matches!(
        read_cell_embedding_table_parquet_bytes(
            &hidden_nonzero,
            &embedding_parquet_record(&missing, &hidden_nonzero),
            &missing.expected,
            &missing.row_link,
            missing_verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Parquet {
            reason: marklab::ParquetFailure::InvalidComponent,
        })
    ));
}

#[test]
fn verified_store_and_borrowed_arrow_paths_reject_the_same_hostile_block() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (table, mut bytes, _) = write_embedding_table(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    let block_offset = first_footer_record_block_offset(&bytes);
    bytes[block_offset..block_offset + 8].copy_from_slice(&(-1_i64).to_le_bytes());
    let draft = embedding_table_record(&fixture, &bytes);
    let record = fixture
        .store
        .publish(&draft, |writer| writer.write_all(&bytes))
        .expect("publish hostile fixture")
        .into_record();
    let expected_record = record_with_schema(&fixture, "marklab.cell_embedding_expected_cells");
    let row_link_record = record_with_schema(&fixture, "marklab.cell_embedding_row_link");
    let bindings = CellEmbeddingTablePhysicalBindings::new(
        expected_record.id(),
        fixture.provenance_artifact_id,
        row_link_record.id(),
        fixture.row_link.logical_digest(),
        table.qc_summary().logical_digest(),
    )
    .expect("bindings");
    let borrowed_error = preflight_cell_embedding_table_arrow_bytes(
        &bytes,
        &fixture.expected,
        bindings,
        embedding_budgets(),
    )
    .expect_err("borrowed hostile block");
    let managed_error = match read_cell_embedding_table_arrow_from_store(
        &fixture.store,
        &record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    ) {
        Err(VerifiedReaderError::Callback(error)) => error,
        result => panic!("expected managed callback failure, observed {result:?}"),
    };
    assert_eq!(managed_error, borrowed_error);
    let borrowed_scan = scan_cell_embedding_table_arrow_bytes(
        &bytes,
        &draft,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect_err("borrowed hostile scan");
    let managed_scan = match scan_cell_embedding_table_arrow_from_store(
        &fixture.store,
        &record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    ) {
        Err(VerifiedReaderError::Callback(error)) => error,
        result => panic!("expected managed scan callback failure, observed {result:?}"),
    };
    assert_eq!(borrowed_scan, borrowed_error);
    assert_eq!(managed_scan, borrowed_error);
    assert!(matches!(
        managed_error,
        EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidBlock,
        }
    ));
}

#[test]
fn arrow_reader_rejects_nonfinite_negative_zero_status_link_and_logical_drift() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (_, canonical, _) = write_embedding_table(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    let value_offset = first_embedding_buffer_offset(&canonical, 5);

    let mut nonfinite = canonical.clone();
    nonfinite[value_offset..value_offset + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    let nonfinite_record = embedding_table_record(&fixture, &nonfinite);
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &nonfinite,
            &nonfinite_record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidComponent,
        })
    ));

    let mut negative_zero = canonical.clone();
    negative_zero[value_offset..value_offset + 4].copy_from_slice(&(-0.0_f32).to_le_bytes());
    let negative_zero_record = embedding_table_record(&fixture, &negative_zero);
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &negative_zero,
            &negative_zero_record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidComponent,
        })
    ));

    let mut invalid_status = canonical.clone();
    let status_offset = first_embedding_buffer_offset(&invalid_status, 8);
    invalid_status[status_offset..status_offset + 7].copy_from_slice(b"invalid");
    let invalid_status_record = embedding_table_record(&fixture, &invalid_status);
    let error = read_cell_embedding_table_arrow_bytes(
        &invalid_status,
        &invalid_status_record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect_err("invalid status");
    assert!(matches!(
        error,
        EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidStatus,
        }
    ));
    assert!(!error.to_string().contains("invalid"));

    let mut logical_drift = canonical.clone();
    let (table, _, _) = write_embedding_table(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    let old_digest = table.qc_summary().logical_digest().to_string();
    let replacement = "a".repeat(64);
    assert_ne!(old_digest, replacement);
    assert_eq!(
        replace_all_same_length(
            &mut logical_drift,
            old_digest.as_bytes(),
            replacement.as_bytes(),
        ),
        2
    );
    let logical_record = embedding_table_record(&fixture, &logical_drift);
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &logical_drift,
            &logical_record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::LogicalDigestMismatch,
        })
    ));

    let (_, status_mismatch, status_mismatch_record) = write_embedding_table(
        &fixture,
        CellEmbeddingRow::non_present(cell("cell-a"), EmbeddingStatus::QcRejected)
            .expect("rejected row"),
    );
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &status_mismatch,
            &status_mismatch_record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidStatus,
        })
    ));
}

#[test]
fn arrow_reader_rejects_hidden_nonzero_and_malformed_string_offsets() {
    let missing = build_fixture_with_status(
        "marklab.model_checkpoint",
        LicenseAvailability::Managed,
        FixtureEmbeddingStatus::MissingVector,
    );
    let missing_verified = verified_graph(&missing);
    let (expected_table, mut hidden_nonzero, _) = write_embedding_table(
        &missing,
        CellEmbeddingRow::non_present(cell("cell-a"), EmbeddingStatus::MissingVector)
            .expect("missing row"),
    );
    let canonical_record = embedding_table_record(&missing, &hidden_nonzero);
    let decoded = read_cell_embedding_table_arrow_bytes(
        &hidden_nonzero,
        &canonical_record,
        &missing.expected,
        &missing.row_link,
        missing_verified,
        embedding_budgets(),
    )
    .expect("canonical missing row");
    assert_eq!(decoded, expected_table);
    let value_offset = first_embedding_buffer_offset(&hidden_nonzero, 5);
    hidden_nonzero[value_offset..value_offset + 4].copy_from_slice(&1.0_f32.to_le_bytes());
    let hidden_record = embedding_table_record(&missing, &hidden_nonzero);
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &hidden_nonzero,
            &hidden_record,
            &missing.expected,
            &missing.row_link,
            missing_verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidComponent,
        })
    ));

    let present = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let present_verified = verified_graph(&present);
    let (_, mut malformed_offsets, _) = write_embedding_table(
        &present,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    let offsets_start = first_embedding_buffer_offset(&malformed_offsets, 1);
    malformed_offsets[offsets_start + 4..offsets_start + 8]
        .copy_from_slice(&i32::MAX.to_le_bytes());
    let malformed_record = embedding_table_record(&present, &malformed_offsets);
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &malformed_offsets,
            &malformed_record,
            &present.expected,
            &present.row_link,
            present_verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::StockDecode,
        })
    ));
}

#[test]
fn arrow_reader_rejects_unexpected_cell_identity_after_raw_preflight() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (_, mut bytes, _) = write_embedding_table(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    assert_eq!(replace_all_same_length(&mut bytes, b"cell-a", b"cell-z"), 1);
    let record = embedding_table_record(&fixture, &bytes);
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &bytes,
            &record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidCellOrder,
        })
    ));
}

#[test]
fn arrow_reader_rejects_record_binding_before_stock_decode() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (_, bytes, _) = write_embedding_table(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    let wrong_record = draft_record(
        "marklab.cell_embedding_table",
        "application/octet-stream",
        &bytes,
        Vec::new(),
        None,
    );
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &bytes,
            &wrong_record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::ArtifactBindingMismatch)
    ));
}

#[test]
fn artifact_graph_rejects_wrong_schema_and_catalog_only_availability() {
    let wrong_schema = build_fixture("marklab.wrong_checkpoint", LicenseAvailability::Managed);
    assert!(matches!(
        wrong_schema.provenance.validate_artifact_graph(
            wrong_schema.provenance_artifact_id,
            &wrong_schema.expected,
            &wrong_schema.identity_map,
            &wrong_schema.context,
            &wrong_schema.row_link,
            &wrong_schema.catalog,
            &wrong_schema.store,
        ),
        Err(EmbeddingArtifactGraphError::SchemaMismatch {
            role: CellEmbeddingArtifactRole::Checkpoint
        })
    ));

    let catalog_only = build_fixture("marklab.model_checkpoint", LicenseAvailability::CatalogOnly);
    let error = catalog_only
        .provenance
        .validate_artifact_graph(
            catalog_only.provenance_artifact_id,
            &catalog_only.expected,
            &catalog_only.identity_map,
            &catalog_only.context,
            &catalog_only.row_link,
            &catalog_only.catalog,
            &catalog_only.store,
        )
        .expect_err("catalog-only record must not be available");
    assert!(matches!(
        error,
        EmbeddingArtifactGraphError::Unavailable {
            role: CellEmbeddingArtifactRole::LicenseRecord,
            reason: ArtifactAvailabilityFailure::LocatorMissing,
        }
    ));
    assert!(!error.to_string().contains("privacy-sentinel"));

    let non_managed = build_fixture(
        "marklab.model_checkpoint",
        LicenseAvailability::NonManagedLocal,
    );
    let error = non_managed
        .provenance
        .validate_artifact_graph(
            non_managed.provenance_artifact_id,
            &non_managed.expected,
            &non_managed.identity_map,
            &non_managed.context,
            &non_managed.row_link,
            &non_managed.catalog,
            &non_managed.store,
        )
        .expect_err("non-managed local locator must not satisfy graph verification");
    assert!(matches!(
        error,
        EmbeddingArtifactGraphError::Unavailable {
            role: CellEmbeddingArtifactRole::LicenseRecord,
            reason: ArtifactAvailabilityFailure::LocatorMissing,
        }
    ));
    assert!(!error.to_string().contains("privacy-sentinel-license"));
}

#[test]
fn artifact_graph_rejects_row_link_values_from_a_different_expected_set() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let other_expected =
        ExpectedCellSet::new("other.v1", vec![cell("cell-b")]).expect("other expected set");
    let forged_link = CellEmbeddingRowLink::new(
        fixture.row_link.source_cells_artifact_id(),
        fixture.row_link.source_vectors_artifact_id(),
        fixture.row_link.expected_cells_artifact_id(),
        fixture.row_link.identity_map_artifact_id(),
        fixture.row_link.converter_artifact_id(),
        &other_expected,
        &hierarchy(&other_expected),
        vec![CellEmbeddingRowLinkEntry::present(cell("cell-b"), 0, 0)],
        4_096,
    )
    .expect("internally valid forged link");
    assert!(matches!(
        fixture.provenance.validate_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected,
            &fixture.identity_map,
            &fixture.context,
            &forged_link,
            &fixture.catalog,
            &fixture.store,
        ),
        Err(EmbeddingArtifactGraphError::LinkageMismatch)
    ));
}

#[test]
fn artifact_graph_rejects_provenance_record_structure_before_store_access() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let cases = [
        (
            provenance_record(
                &fixture,
                "application/json",
                fixture.provenance.direct_dependencies().to_vec(),
                BTreeMap::new(),
                None,
            ),
            EmbeddingArtifactGraphError::ContentKindMismatch {
                role: CellEmbeddingArtifactRole::Provenance,
            },
        ),
        (
            provenance_record(
                &fixture,
                "application/vnd.marklab.embedding-provenance.v1+json",
                fixture.provenance.direct_dependencies().to_vec(),
                BTreeMap::from([("extra".to_owned(), "value".to_owned())]),
                None,
            ),
            EmbeddingArtifactGraphError::SemanticMetadataMismatch {
                role: CellEmbeddingArtifactRole::Provenance,
            },
        ),
        (
            provenance_record(
                &fixture,
                "application/vnd.marklab.embedding-provenance.v1+json",
                fixture.provenance.direct_dependencies()[1..].to_vec(),
                BTreeMap::new(),
                None,
            ),
            EmbeddingArtifactGraphError::DependencyMismatch {
                role: CellEmbeddingArtifactRole::Provenance,
            },
        ),
        (
            provenance_record(
                &fixture,
                "application/vnd.marklab.embedding-provenance.v1+json",
                fixture.provenance.direct_dependencies().to_vec(),
                BTreeMap::new(),
                Some(row_link_manifest(1)),
            ),
            EmbeddingArtifactGraphError::UnexpectedTableManifest {
                role: CellEmbeddingArtifactRole::Provenance,
            },
        ),
    ];
    for (record, expected_error) in cases {
        let id = record.id();
        let catalog = replace_record(&fixture.catalog, fixture.provenance_artifact_id, record);
        let error = fixture
            .provenance
            .validate_artifact_graph(
                id,
                &fixture.expected,
                &fixture.identity_map,
                &fixture.context,
                &fixture.row_link,
                &catalog,
                &fixture.store,
            )
            .expect_err("structural drift must fail");
        assert_eq!(error, expected_error);
        assert!(!error.to_string().contains("privacy-sentinel"));
    }
}

#[test]
fn artifact_graph_rejects_canonical_payload_mismatches() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let encoded = fixture
        .provenance
        .to_canonical_json()
        .expect("provenance bytes");
    let changed = String::from_utf8(encoded)
        .expect("UTF-8 provenance")
        .replace("\"model_version\":\"1.0\"", "\"model_version\":\"1.1\"");
    let changed = CellEmbeddingProvenance::from_canonical_json(changed.as_bytes())
        .expect("changed canonical provenance");
    assert!(matches!(
        changed.validate_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected,
            &fixture.identity_map,
            &fixture.context,
            &fixture.row_link,
            &fixture.catalog,
            &fixture.store,
        ),
        Err(EmbeddingArtifactGraphError::PayloadIdentityMismatch {
            role: CellEmbeddingArtifactRole::Provenance,
        })
    ));

    let changed_map =
        CellIdentityMap::new(
            fixture.identity_map.source_cells_artifact_id(),
            fixture.identity_map.expected_cells_artifact_id(),
            &fixture.expected,
            vec![CellIdentityMapEntry::new("changed-source", cell("cell-a"))
                .expect("changed identity")],
        )
        .expect("changed identity map");
    assert!(matches!(
        fixture.provenance.validate_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected,
            &changed_map,
            &fixture.context,
            &fixture.row_link,
            &fixture.catalog,
            &fixture.store,
        ),
        Err(EmbeddingArtifactGraphError::PayloadIdentityMismatch {
            role: CellEmbeddingArtifactRole::IdentityMap,
        })
    ));
}

#[test]
fn artifact_graph_maps_corrupted_managed_bytes_to_redacted_integrity() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let license = record_with_schema(&fixture, "marklab.license_record");
    std::fs::write(
        managed_path(&fixture._root, license),
        b"corrupted privacy-sentinel",
    )
    .expect("corrupt managed fixture");
    let error = fixture
        .provenance
        .validate_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected,
            &fixture.identity_map,
            &fixture.context,
            &fixture.row_link,
            &fixture.catalog,
            &fixture.store,
        )
        .expect_err("corrupted managed bytes must fail");
    assert!(matches!(
        error,
        EmbeddingArtifactGraphError::Unavailable {
            role: CellEmbeddingArtifactRole::LicenseRecord,
            reason: ArtifactAvailabilityFailure::Integrity,
        }
    ));
    assert!(!error.to_string().contains("privacy-sentinel"));
}

#[path = "cellvit_embedding_artifact_graph/declared_binary_centroid.rs"]
mod declared_binary_centroid;

#[path = "cellvit_embedding_artifact_graph/declared_binary_centroid_workflow.rs"]
mod declared_binary_centroid_workflow;

#[path = "cellvit_embedding_artifact_graph/probability_cross_covariance.rs"]
mod probability_cross_covariance;

#[allow(dead_code)]
#[path = "support/declared_scalar.rs"]
mod declared_scalar_support;
