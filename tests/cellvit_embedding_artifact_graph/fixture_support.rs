use super::*;

pub(super) struct Fixture {
    pub(super) _root: TempDir,
    pub(super) store: LocalArtifactStore,
    pub(super) catalog: ArtifactCatalog,
    pub(super) provenance: CellEmbeddingProvenance,
    pub(super) provenance_artifact_id: marklab::ArtifactId,
    pub(super) expected: ExpectedCellSet,
    pub(super) identity_map: CellIdentityMap,
    pub(super) context: EmbeddingSpatialContext,
    pub(super) row_link: CellEmbeddingRowLink,
    pub(super) dimension: u32,
}

#[derive(Clone, Copy)]
pub(super) enum LicenseAvailability {
    Managed,
    CatalogOnly,
    NonManagedLocal,
}

#[derive(Clone, Copy)]
pub(super) enum FixtureEmbeddingStatus {
    Present,
    MissingVector,
    ExtractionFailed,
    QcRejected,
}

impl FixtureEmbeddingStatus {
    pub(super) fn domain_status(self) -> EmbeddingStatus {
        match self {
            Self::Present => EmbeddingStatus::Present,
            Self::MissingVector => EmbeddingStatus::MissingVector,
            Self::ExtractionFailed => EmbeddingStatus::ExtractionFailed,
            Self::QcRejected => EmbeddingStatus::QcRejected,
        }
    }

    pub(super) fn table_row(self, cell_id: CellId, dimension: u32) -> CellEmbeddingRow {
        match self {
            Self::Present => CellEmbeddingRow::present(cell_id, vec![0.25; dimension as usize]),
            status => CellEmbeddingRow::non_present(cell_id, status.domain_status())
                .expect("non-present fixture row"),
        }
    }
}

pub(super) fn cell(value: &str) -> CellId {
    CellId::new(value).expect("cell ID")
}

pub(super) fn draft_record(
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

pub(super) fn publish_record(
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

pub(super) fn row_link_manifest(row_count: u64) -> TableManifest {
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

pub(super) fn embedding_manifest(row_count: u64, dimension: u32) -> TableManifest {
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

pub(super) fn embedding_parquet_manifest(row_count: u64, dimension: u32) -> TableManifest {
    TableManifest::new(
        TableFormat::ParquetFile,
        "marklab.parquet.embedding-table.v1",
        row_count,
        embedding_manifest(row_count, dimension).columns().to_vec(),
        vec!["cell_id".to_owned()],
    )
    .expect("embedding Parquet manifest")
}

pub(super) fn context() -> EmbeddingSpatialContext {
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

pub(super) fn hierarchy(expected: &ExpectedCellSet) -> CohortHierarchy {
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

pub(super) fn build_fixture(
    checkpoint_schema: &str,
    license_availability: LicenseAvailability,
) -> Fixture {
    build_fixture_with_status(
        checkpoint_schema,
        license_availability,
        FixtureEmbeddingStatus::Present,
    )
}

pub(super) fn build_fixture_with_status(
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

pub(super) fn build_fixture_with_rows(
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

pub(super) fn build_fixture_with_rows_and_sources(
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

pub(super) fn replace_record(
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

pub(super) fn provenance_record(
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

pub(super) fn record_with_schema<'a>(fixture: &'a Fixture, schema: &str) -> &'a ArtifactRecord {
    fixture
        .catalog
        .iter()
        .find_map(|(_id, record)| (record.schema().id() == schema).then_some(record))
        .expect("record with schema")
}

pub(super) fn managed_path(root: &TempDir, record: &ArtifactRecord) -> std::path::PathBuf {
    let id = record.id().to_string();
    root.path().join("objects/sha256").join(&id[..2]).join(id)
}

pub(super) fn verified_graph(fixture: &Fixture) -> VerifiedCellEmbeddingArtifactGraph {
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

pub(super) fn embedding_budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
    )
}

#[cfg(feature = "csv")]
pub(super) fn all_present_source_csv(row_count: usize) -> Vec<u8> {
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
pub(super) fn all_present_source_npy(rows: &[Vec<f32>]) -> Vec<u8> {
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

pub(super) fn write_embedding_table(
    fixture: &Fixture,
    row: CellEmbeddingRow,
) -> (CellEmbeddingTable, Vec<u8>, ArtifactRecord) {
    write_embedding_rows_arrow(fixture, vec![row], embedding_budgets())
}

pub(super) fn write_embedding_rows_arrow(
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

pub(super) fn embedding_table_record(fixture: &Fixture, bytes: &[u8]) -> ArtifactRecord {
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

pub(super) fn embedding_parquet_record(fixture: &Fixture, bytes: &[u8]) -> ArtifactRecord {
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

pub(super) fn write_embedding_table_parquet_fixture(
    fixture: &Fixture,
    row: CellEmbeddingRow,
) -> (CellEmbeddingTable, Vec<u8>, ArtifactRecord) {
    write_embedding_rows_parquet(fixture, vec![row], embedding_budgets())
}

pub(super) fn write_embedding_rows_parquet(
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

pub(super) fn exact_retained_limit<T: std::fmt::Debug>(
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
