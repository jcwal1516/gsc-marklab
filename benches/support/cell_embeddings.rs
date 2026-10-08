use std::{collections::BTreeMap, io::Write, mem::size_of, str::FromStr};

use marklab::{
    import_cellvit_he_bundle_bytes, publish_cell_embedding_row_link_arrow,
    publish_cell_embedding_table_arrow, publish_cell_embedding_table_parquet,
    read_cell_embedding_table_arrow_from_store, read_cell_embedding_table_parquet_from_store,
    scan_cell_embedding_table_arrow_from_store, scan_cell_embedding_table_parquet_from_store,
    ArtifactCatalog, ArtifactId, ArtifactKey, ArtifactLocator, ArtifactRecord, ArtifactRef,
    ArtifactSchema, CanonicalDecimal, CellEmbeddingExecutionProvenance,
    CellEmbeddingInputArtifacts, CellEmbeddingModelProvenance, CellEmbeddingProvenance,
    CellEmbeddingRowLink, CellEmbeddingRowLinkEntry, CellEmbeddingTable,
    CellEmbeddingTablePhysicalBindings, CellEmbeddingTensorContract, CellId, CellIdentityMap,
    CellIdentityMapEntry, CellVitHeArtifactBindings, CellVitHeImportRequest, CohortHierarchy,
    ContentDigest, ContentDigestWriter, CoordinateFrame, CoordinateFrameId, CoordinateRegistry,
    CoordinateSpace, CoordinateUnit, EmbeddingColumnarBudgets, EmbeddingQcSummary,
    EmbeddingSpatialContext, FrameTransform, HierarchyId, HierarchyNode, ImageCoordinateConvention,
    LocalArtifactStore, PatchBoundaryPolicy, PatientId, PositiveRational, ReplicationRole, SlideId,
    SourceBundleBudgets, SpatialAxis, StoreId, TransformId, TransformMatrix,
    VerifiedCellEmbeddingArtifactGraph,
};
use tempfile::TempDir;

const SMOKE_ROWS: usize = 10_000;
const SMOKE_DIMENSION: u32 = 1_280;
const FULL_ROWS: usize = 1_000_000;
const FULL_DIMENSION: u32 = 256;
const RANDOM_ACCESS_COUNT: usize = 4_096;
const COVARIANCE_DIMENSION: usize = 16;
const KERNEL_DIMENSION: usize = 64;
#[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
const HARNESS_ALLOWANCE_BYTES: usize = 64 * 1024 * 1024;
const SMOKE_RETAINED_BYTES: usize = 512 * 1024 * 1024;
const FULL_RETAINED_BYTES: usize = 1_536 * 1024 * 1024;
const NUMERIC_DIGEST_DOMAIN: &[u8] = b"marklab-cell-embedding-benchmark-numerics-v1";
const LCG_SEED: u64 = 0x4d4c_4330_3453_4341;
const LCG_MULTIPLIER: u64 = 6_364_136_223_846_793_005;
const LCG_INCREMENT: u64 = 1_442_695_040_888_963_407;
const SMOKE_LOGICAL_DIGEST: &str =
    "df74ee3588f3c5dbf4fa81ffc285dd9f84daf8bb1101e7294fba6536785931de";
const SMOKE_NUMERIC_DIGEST: &str =
    "0396c2d78ff98c7307e7dcf383d8579cf1a06c2501f9fc67c91a285308de08b4";
const FULL_LOGICAL_DIGEST: &str =
    "d5dc753238330e9be60fdee94c6c24d7151f26660527689b15e8895e983c5633";
const FULL_NUMERIC_DIGEST: &str =
    "133dc720d9775d8d2a3c4140a36529a750ab844acd6970eb6ca2ff86d8e7d49a";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BenchmarkProfile {
    Smoke,
    Full,
}

impl BenchmarkProfile {
    fn selected() -> Self {
        if std::env::var("MARKLAB_BENCH_PROFILE").as_deref() == Ok("full")
            || std::env::args().any(|argument| argument.contains("1m_x_256"))
        {
            Self::Full
        } else {
            Self::Smoke
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Smoke => "10k_x_1280",
            Self::Full => "1m_x_256",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkloadOutcome {
    row_count: u64,
    dimension: u32,
    logical_digest: ContentDigest,
    numeric_digest: ContentDigest,
}

impl WorkloadOutcome {
    pub fn logical_digest(self) -> ContentDigest {
        self.logical_digest
    }

    pub fn numeric_digest(self) -> ContentDigest {
        self.numeric_digest
    }
}

enum PreparedInner {
    Smoke(Box<SmokeFixture>),
    Full(Box<CellEmbeddingTable>),
}

pub struct PreparedEmbeddingBenchmark {
    profile: BenchmarkProfile,
    inner: PreparedInner,
    expected: WorkloadOutcome,
    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    declared_retained_bytes: usize,
}

impl PreparedEmbeddingBenchmark {
    pub fn selected() -> Self {
        match BenchmarkProfile::selected() {
            BenchmarkProfile::Smoke => Self::smoke(),
            BenchmarkProfile::Full => Self::full(),
        }
    }

    pub fn smoke() -> Self {
        let fixture = Box::new(SmokeFixture::new());
        Self {
            profile: BenchmarkProfile::Smoke,
            inner: PreparedInner::Smoke(fixture),
            expected: expected_outcome(BenchmarkProfile::Smoke),
            #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
            declared_retained_bytes: SMOKE_RETAINED_BYTES,
        }
    }

    fn full() -> Self {
        let table = Box::new(build_domain_table(
            FULL_ROWS,
            FULL_DIMENSION,
            FULL_RETAINED_BYTES,
        ));
        Self {
            profile: BenchmarkProfile::Full,
            inner: PreparedInner::Full(table),
            expected: expected_outcome(BenchmarkProfile::Full),
            #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
            declared_retained_bytes: FULL_RETAINED_BYTES,
        }
    }

    pub fn name(&self) -> &'static str {
        self.profile.name()
    }

    pub fn row_count(&self) -> usize {
        match &self.inner {
            PreparedInner::Smoke(fixture) => fixture.expected.cells().len(),
            PreparedInner::Full(table) => table.row_count(),
        }
    }

    pub fn dimension(&self) -> u32 {
        match &self.inner {
            PreparedInner::Smoke(_) => SMOKE_DIMENSION,
            PreparedInner::Full(table) => table.dimension(),
        }
    }

    pub fn value_count(&self) -> u64 {
        u64::try_from(self.row_count())
            .expect("benchmark rows fit u64")
            .checked_mul(u64::from(self.dimension()))
            .expect("benchmark value count")
    }

    pub fn expected_outcome(&self) -> WorkloadOutcome {
        self.expected
    }

    pub fn run(&self) -> WorkloadOutcome {
        match &self.inner {
            PreparedInner::Smoke(fixture) => fixture.run(),
            PreparedInner::Full(table) => run_domain_workload(table),
        }
    }

    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    pub fn run_with_physical_publication(&self) -> WorkloadOutcome {
        match &self.inner {
            PreparedInner::Smoke(fixture) => fixture.run_with_physical_publication(),
            PreparedInner::Full(_) => {
                panic!("DHAT physical publication is defined only for the smoke profile")
            }
        }
    }

    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    pub fn dhat_peak_limit(&self) -> usize {
        self.declared_retained_bytes
            .checked_add(HARNESS_ALLOWANCE_BYTES)
            .expect("DHAT peak limit")
    }
}

fn expected_outcome(profile: BenchmarkProfile) -> WorkloadOutcome {
    let (row_count, dimension, logical_digest, numeric_digest) = match profile {
        BenchmarkProfile::Smoke => (
            SMOKE_ROWS,
            SMOKE_DIMENSION,
            SMOKE_LOGICAL_DIGEST,
            SMOKE_NUMERIC_DIGEST,
        ),
        BenchmarkProfile::Full => (
            FULL_ROWS,
            FULL_DIMENSION,
            FULL_LOGICAL_DIGEST,
            FULL_NUMERIC_DIGEST,
        ),
    };
    WorkloadOutcome {
        row_count: u64::try_from(row_count).expect("benchmark expected row count"),
        dimension,
        logical_digest: ContentDigest::from_str(logical_digest)
            .expect("benchmark expected logical digest"),
        numeric_digest: ContentDigest::from_str(numeric_digest)
            .expect("benchmark expected numeric digest"),
    }
}

struct SmokeFixture {
    _root: TempDir,
    store: LocalArtifactStore,
    expected: marklab::ExpectedCellSet,
    identity_map: CellIdentityMap,
    hierarchy: CohortHierarchy,
    row_link: CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    source_bindings: CellVitHeArtifactBindings,
    source_npy: Vec<u8>,
    source_csv: Vec<u8>,
    source_budgets: SourceBundleBudgets,
    columnar_budgets: EmbeddingColumnarBudgets,
    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    physical_bindings: CellEmbeddingTablePhysicalBindings,
    arrow_record: ArtifactRecord,
    parquet_record: ArtifactRecord,
}

impl SmokeFixture {
    fn new() -> Self {
        let source_csv = source_csv(SMOKE_ROWS);
        let source_npy = source_npy(SMOKE_ROWS, SMOKE_DIMENSION);
        let source_budgets = source_budgets(&source_npy, &source_csv);
        let columnar_budgets = columnar_budgets();
        let root = TempDir::new().expect("embedding benchmark store root");
        let store = LocalArtifactStore::open(
            root.path(),
            StoreId::new("embedding-benchmark").expect("benchmark store ID"),
        )
        .expect("benchmark store");
        let cells = benchmark_cells(SMOKE_ROWS);
        let expected = marklab::ExpectedCellSet::new("benchmark-all.v1", cells.clone())
            .expect("benchmark expected cells");
        let hierarchy = benchmark_hierarchy(&expected);

        let records = publish_graph_records(
            &store,
            &expected,
            &hierarchy,
            &cells,
            &source_csv,
            &source_npy,
            columnar_budgets,
        );
        let graph = records
            .provenance
            .validate_artifact_graph(
                records.provenance_record.id(),
                &expected,
                &records.identity_map,
                &records.context,
                &records.row_link,
                &records.catalog,
                &store,
            )
            .expect("benchmark verified graph");
        let source_bindings = CellVitHeArtifactBindings::from_records(
            records.source_cells_record.clone(),
            records.source_vectors_record.clone(),
            records.expected_record.clone(),
            records.identity_record.clone(),
            records.converter_record.clone(),
        )
        .expect("benchmark source bindings");
        let request = CellVitHeImportRequest::new(
            &expected,
            &records.identity_map,
            &hierarchy,
            &source_bindings,
            source_budgets,
        );
        let imported = import_cellvit_he_bundle_bytes(&source_npy, &source_csv, request)
            .expect("benchmark source import")
            .finalize(&expected, &graph)
            .expect("benchmark source finalization");
        assert_eq!(imported.row_link(), &records.row_link);
        let physical_bindings = CellEmbeddingTablePhysicalBindings::new(
            records.expected_record.id(),
            records.provenance_record.id(),
            records.row_link_record.id(),
            records.row_link.logical_digest(),
            imported.table().qc_summary().logical_digest(),
        )
        .expect("benchmark physical bindings");
        let arrow_record = publish_cell_embedding_table_arrow(
            &store,
            imported.table(),
            physical_bindings,
            columnar_budgets,
        )
        .expect("publish benchmark Arrow table")
        .into_record();
        let parquet_record = publish_cell_embedding_table_parquet(
            &store,
            imported.table(),
            physical_bindings,
            columnar_budgets,
        )
        .expect("publish benchmark Parquet table")
        .into_record();

        Self {
            _root: root,
            store,
            expected,
            identity_map: records.identity_map,
            hierarchy,
            row_link: records.row_link,
            graph,
            source_bindings,
            source_npy,
            source_csv,
            source_budgets,
            columnar_budgets,
            #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
            physical_bindings,
            arrow_record,
            parquet_record,
        }
    }

    fn run(&self) -> WorkloadOutcome {
        let request = CellVitHeImportRequest::new(
            &self.expected,
            &self.identity_map,
            &self.hierarchy,
            &self.source_bindings,
            self.source_budgets,
        );
        let candidate = import_cellvit_he_bundle_bytes(&self.source_npy, &self.source_csv, request)
            .expect("benchmark source import");
        assert_eq!(candidate.row_link(), &self.row_link);
        let imported = candidate
            .finalize(&self.expected, &self.graph)
            .expect("benchmark source finalization");
        let table = imported.table();
        let outcome = run_domain_workload(table);
        self.validate_physical_round_trip(
            &self.store,
            &self.arrow_record,
            &self.parquet_record,
            table,
        );
        outcome
    }

    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    fn run_with_physical_publication(&self) -> WorkloadOutcome {
        let request = CellVitHeImportRequest::new(
            &self.expected,
            &self.identity_map,
            &self.hierarchy,
            &self.source_bindings,
            self.source_budgets,
        );
        let candidate = import_cellvit_he_bundle_bytes(&self.source_npy, &self.source_csv, request)
            .expect("benchmark source import");
        assert_eq!(candidate.row_link(), &self.row_link);
        let imported = candidate
            .finalize(&self.expected, &self.graph)
            .expect("benchmark source finalization");
        let table = imported.table();
        let outcome = run_domain_workload(table);

        let output_root = TempDir::new().expect("DHAT embedding output root");
        let output_store = LocalArtifactStore::open(
            output_root.path(),
            StoreId::new("embedding-benchmark-dhat").expect("DHAT output store ID"),
        )
        .expect("DHAT output store");
        let arrow_record = publish_cell_embedding_table_arrow(
            &output_store,
            table,
            self.physical_bindings,
            self.columnar_budgets,
        )
        .expect("publish DHAT Arrow table")
        .into_record();
        let parquet_record = publish_cell_embedding_table_parquet(
            &output_store,
            table,
            self.physical_bindings,
            self.columnar_budgets,
        )
        .expect("publish DHAT Parquet table")
        .into_record();
        self.validate_physical_round_trip(&output_store, &arrow_record, &parquet_record, table);
        outcome
    }

    fn validate_physical_round_trip(
        &self,
        store: &LocalArtifactStore,
        arrow_record: &ArtifactRecord,
        parquet_record: &ArtifactRecord,
        table: &CellEmbeddingTable,
    ) {
        let qc = table.qc_summary();

        let arrow_scan = scan_cell_embedding_table_arrow_from_store(
            store,
            arrow_record,
            &self.expected,
            &self.row_link,
            self.graph,
            self.columnar_budgets,
        )
        .expect("benchmark Arrow scan");
        assert_eq!(arrow_scan, qc);
        let arrow_table = read_cell_embedding_table_arrow_from_store(
            store,
            arrow_record,
            &self.expected,
            &self.row_link,
            self.graph,
            self.columnar_budgets,
        )
        .expect("benchmark Arrow read");
        assert_eq!(&arrow_table, table);
        drop(arrow_table);

        let parquet_scan = scan_cell_embedding_table_parquet_from_store(
            store,
            parquet_record,
            &self.expected,
            &self.row_link,
            self.graph,
            self.columnar_budgets,
        )
        .expect("benchmark Parquet scan");
        assert_eq!(parquet_scan, qc);
        let parquet_table = read_cell_embedding_table_parquet_from_store(
            store,
            parquet_record,
            &self.expected,
            &self.row_link,
            self.graph,
            self.columnar_budgets,
        )
        .expect("benchmark Parquet read");
        assert_eq!(&parquet_table, table);
    }
}

struct PublishedGraphRecords {
    source_cells_record: ArtifactRecord,
    source_vectors_record: ArtifactRecord,
    expected_record: ArtifactRecord,
    identity_record: ArtifactRecord,
    converter_record: ArtifactRecord,
    row_link_record: ArtifactRecord,
    provenance_record: ArtifactRecord,
    identity_map: CellIdentityMap,
    context: EmbeddingSpatialContext,
    row_link: CellEmbeddingRowLink,
    provenance: CellEmbeddingProvenance,
    catalog: ArtifactCatalog,
}

#[allow(clippy::too_many_arguments)]
fn publish_graph_records(
    store: &LocalArtifactStore,
    expected: &marklab::ExpectedCellSet,
    hierarchy: &CohortHierarchy,
    cells: &[CellId],
    source_csv: &[u8],
    source_npy: &[u8],
    budgets: EmbeddingColumnarBudgets,
) -> PublishedGraphRecords {
    let checkpoint = publish_record(
        store,
        "marklab.model_checkpoint",
        "application/octet-stream",
        b"benchmark-checkpoint",
        Vec::new(),
        None,
    );
    let source_snapshot = publish_record(
        store,
        "marklab.source_snapshot",
        "application/octet-stream",
        b"benchmark-source-snapshot",
        Vec::new(),
        None,
    );
    let license = publish_record(
        store,
        "marklab.license_record",
        "text/plain",
        b"benchmark-license",
        Vec::new(),
        None,
    );
    let preprocessing = publish_simple_record(store, "marklab.embedding_preprocessing");
    let run_config = publish_simple_record(store, "marklab.embedding_run_config");
    let environment = publish_simple_record(store, "marklab.execution_environment");
    let converter = publish_simple_record(store, "marklab.converter_manifest");
    let source_cells = publish_record(
        store,
        "marklab.cell_embedding_source_cells",
        "text/csv;profile=marklab-cellvit-he-bundle-v1",
        source_csv,
        Vec::new(),
        None,
    );
    let source_vectors = publish_record(
        store,
        "marklab.cell_embedding_source_npy",
        "application/x-npy;profile=marklab-cellvit-he-f4-v1",
        source_npy,
        Vec::new(),
        None,
    );
    let expected_bytes = expected.to_bytes().expect("benchmark expected bytes");
    let expected_record = publish_record(
        store,
        "marklab.cell_embedding_expected_cells",
        "application/vnd.marklab.embedding-expected-cells.v1",
        &expected_bytes,
        Vec::new(),
        None,
    );
    let identity_map = CellIdentityMap::new(
        source_cells.id(),
        expected_record.id(),
        expected,
        cells
            .iter()
            .enumerate()
            .map(|(row, cell_id)| {
                CellIdentityMapEntry::new(format!("source-{row:08}"), cell_id.clone())
                    .expect("benchmark identity entry")
            })
            .collect(),
    )
    .expect("benchmark identity map");
    let identity_bytes = identity_map.to_bytes().expect("benchmark identity bytes");
    let identity_record = publish_record(
        store,
        "marklab.cell_embedding_identity_map",
        "application/vnd.marklab.embedding-identity-map.v1",
        &identity_bytes,
        vec![source_cells.id(), expected_record.id()],
        None,
    );
    let context = benchmark_context();
    let context_bytes = context
        .to_canonical_json()
        .expect("benchmark context bytes");
    let context_record = publish_record(
        store,
        "marklab.cell_embedding_spatial_context",
        "application/vnd.marklab.embedding-spatial-context.v1+json",
        &context_bytes,
        Vec::new(),
        None,
    );
    let row_link = CellEmbeddingRowLink::new(
        source_cells.id(),
        source_vectors.id(),
        expected_record.id(),
        identity_record.id(),
        converter.id(),
        expected,
        hierarchy,
        cells
            .iter()
            .enumerate()
            .map(|(row, cell_id)| {
                let row = u64::try_from(row).expect("benchmark source row");
                CellEmbeddingRowLinkEntry::present(cell_id.clone(), row, row)
            })
            .collect(),
        SMOKE_RETAINED_BYTES,
    )
    .expect("benchmark row link");
    let row_link_record = publish_cell_embedding_row_link_arrow(store, &row_link, budgets)
        .expect("publish benchmark row link")
        .into_record();
    let provenance = benchmark_provenance(
        &checkpoint,
        &source_snapshot,
        &license,
        &preprocessing,
        &run_config,
        &environment,
        &converter,
        &source_cells,
        &source_vectors,
        &expected_record,
        &identity_record,
        &context_record,
        &row_link_record,
    );
    let provenance_bytes = provenance
        .to_canonical_json()
        .expect("benchmark provenance bytes");
    let provenance_record = publish_record(
        store,
        "marklab.cell_embedding_provenance",
        "application/vnd.marklab.embedding-provenance.v1+json",
        &provenance_bytes,
        provenance.direct_dependencies().to_vec(),
        None,
    );
    let catalog = ArtifactCatalog::from_records([
        checkpoint,
        source_snapshot,
        license,
        preprocessing,
        run_config,
        environment,
        converter.clone(),
        source_cells.clone(),
        source_vectors.clone(),
        expected_record.clone(),
        identity_record.clone(),
        context_record,
        row_link_record.clone(),
        provenance_record.clone(),
    ])
    .expect("benchmark artifact catalog");
    PublishedGraphRecords {
        source_cells_record: source_cells,
        source_vectors_record: source_vectors,
        expected_record,
        identity_record,
        converter_record: converter,
        row_link_record,
        provenance_record,
        identity_map,
        context,
        row_link,
        provenance,
        catalog,
    }
}

#[allow(clippy::too_many_arguments)]
fn benchmark_provenance(
    checkpoint: &ArtifactRecord,
    source_snapshot: &ArtifactRecord,
    license: &ArtifactRecord,
    preprocessing: &ArtifactRecord,
    run_config: &ArtifactRecord,
    environment: &ArtifactRecord,
    converter: &ArtifactRecord,
    source_cells: &ArtifactRecord,
    source_vectors: &ArtifactRecord,
    expected: &ArtifactRecord,
    identity: &ArtifactRecord,
    context: &ArtifactRecord,
    row_link: &ArtifactRecord,
) -> CellEmbeddingProvenance {
    let model = CellEmbeddingModelProvenance::new(
        "cellvit_sam_h",
        "1.0",
        "sam_h",
        checkpoint.id(),
        checkpoint.content().digest(),
        source_snapshot.id(),
        license.id(),
        "Apache-2.0",
        "doi:10.0000-benchmark",
        "z4",
        32,
    )
    .expect("benchmark model provenance");
    let decimal = |value| CanonicalDecimal::new(value).expect("benchmark decimal");
    let tensor = CellEmbeddingTensorContract::rgb_he_raw(
        SMOKE_DIMENSION,
        [decimal("0.485"), decimal("0.456"), decimal("0.406")],
        [decimal("0.229"), decimal("0.224"), decimal("0.225")],
    )
    .expect("benchmark tensor provenance");
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
    .expect("benchmark execution provenance");
    let inputs = CellEmbeddingInputArtifacts::new(
        source_cells.id(),
        source_vectors.id(),
        expected.id(),
        identity.id(),
        context.id(),
        row_link.id(),
    );
    CellEmbeddingProvenance::new(model, tensor, execution, inputs).expect("benchmark provenance")
}

fn benchmark_context() -> EmbeddingSpatialContext {
    let image = CoordinateFrameId::new("benchmark-image-pixels").expect("image frame");
    let physical = CoordinateFrameId::new("benchmark-slide-micrometers").expect("physical frame");
    let transform = TransformId::new("benchmark-pixel-to-micrometer").expect("transform");
    let registry = CoordinateRegistry::new(
        vec![
            CoordinateFrame::new(
                image.clone(),
                vec![SpatialAxis::X, SpatialAxis::Y],
                CoordinateUnit::Pixel,
                CoordinateSpace::Image(ImageCoordinateConvention::PixelCenterAtInteger),
            )
            .expect("image frame"),
            CoordinateFrame::new(
                physical.clone(),
                vec![SpatialAxis::X, SpatialAxis::Y],
                CoordinateUnit::Micrometer,
                CoordinateSpace::Physical,
            )
            .expect("physical frame"),
        ],
        Vec::new(),
        vec![FrameTransform::new(
            transform.clone(),
            image.clone(),
            physical.clone(),
            TransformMatrix::affine_2d([0.5, 0.0, 0.0, 0.0, 0.5, 0.0])
                .expect("benchmark transform"),
            None,
        )],
        Vec::new(),
    )
    .expect("benchmark coordinate registry");
    EmbeddingSpatialContext::new(
        &registry,
        image,
        physical,
        transform,
        PositiveRational::new(1, 2).expect("benchmark mpp x"),
        PositiveRational::new(1, 2).expect("benchmark mpp y"),
        [1_024, 1_024],
        [64, 64],
        [16, 16],
        PatchBoundaryPolicy::FullyContainedOnly,
    )
    .expect("benchmark spatial context")
}

fn benchmark_hierarchy(expected: &marklab::ExpectedCellSet) -> CohortHierarchy {
    let patient = HierarchyId::from(PatientId::new("benchmark-patient").expect("patient"));
    let slide = HierarchyId::from(SlideId::new("benchmark-slide").expect("slide"));
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
    CohortHierarchy::new(nodes, Vec::new()).expect("benchmark hierarchy")
}

fn build_domain_table(
    row_count: usize,
    dimension: u32,
    maximum_retained_bytes: usize,
) -> CellEmbeddingTable {
    let cells = benchmark_cells(row_count);
    let expected = marklab::ExpectedCellSet::new("benchmark-domain.v1", cells)
        .expect("benchmark domain expected cells");
    let component_count = row_count
        .checked_mul(usize::try_from(dimension).expect("benchmark dimension"))
        .expect("benchmark component count");
    let mut values = Vec::new();
    values
        .try_reserve_exact(component_count)
        .expect("reserve benchmark values");
    for row in 0..row_count {
        for column in 0..dimension {
            values.push(deterministic_value(row, column));
        }
    }
    CellEmbeddingTable::from_present_values(
        dimension,
        &expected,
        artifact_id(b"benchmark-domain-expected"),
        artifact_id(b"benchmark-domain-provenance"),
        ContentDigest::from_bytes(b"benchmark-domain-row-link"),
        values,
        maximum_retained_bytes,
    )
    .expect("benchmark domain table")
}

fn run_domain_workload(table: &CellEmbeddingTable) -> WorkloadOutcome {
    let qc = table.scan_qc(8_192).expect("benchmark QC scan");
    assert_eq!(qc, table.qc_summary());
    let rows = table.row_count();
    assert!(rows > 0);
    let dimension = usize::try_from(table.dimension()).expect("benchmark dimension");
    let mut means = vec![0.0_f64; dimension];
    let mut sum_squares = 0.0_f64;
    for row in 0..rows {
        let vector = table
            .row(row)
            .expect("benchmark sequential row")
            .vector()
            .expect("benchmark present vector");
        for (column, value) in vector.iter().copied().enumerate() {
            let value = f64::from(value);
            means[column] += value;
            sum_squares += value * value;
        }
    }
    let row_count_f64 = f64::from(u32::try_from(rows).expect("benchmark rows fit exact f64"));
    for mean in &mut means {
        *mean /= row_count_f64;
    }
    let frobenius_norm = sum_squares.sqrt();

    let covariance_dimension = dimension.min(COVARIANCE_DIMENSION);
    let mut covariance = vec![0.0_f64; covariance_dimension * covariance_dimension];
    for row in 0..rows {
        let vector = table
            .row(row)
            .expect("benchmark covariance row")
            .vector()
            .expect("benchmark present vector");
        for left in 0..covariance_dimension {
            let left_delta = f64::from(vector[left]) - means[left];
            for right in 0..covariance_dimension {
                covariance[left * covariance_dimension + right] +=
                    left_delta * (f64::from(vector[right]) - means[right]);
            }
        }
    }
    for value in &mut covariance {
        *value /= row_count_f64;
    }

    let indices = random_indices(rows);
    let mut random_access_sum = 0.0_f64;
    for &row in &indices {
        let vector = table
            .row(row)
            .expect("benchmark random row")
            .vector()
            .expect("benchmark present vector");
        for value in vector {
            random_access_sum += f64::from(*value);
        }
    }

    let kernel_dimension = rows.min(KERNEL_DIMENSION);
    let mut kernel = vec![0.0_f64; kernel_dimension * kernel_dimension];
    for left in 0..kernel_dimension {
        let left_vector = table
            .row(indices[left])
            .expect("benchmark kernel left row")
            .vector()
            .expect("benchmark present vector");
        for right in 0..kernel_dimension {
            let right_vector = table
                .row(indices[right])
                .expect("benchmark kernel right row")
                .vector()
                .expect("benchmark present vector");
            let mut dot = 0.0_f64;
            for column in 0..dimension {
                dot += f64::from(left_vector[column]) * f64::from(right_vector[column]);
            }
            kernel[left * kernel_dimension + right] = dot;
        }
    }
    let numeric_digest = numeric_digest(
        qc,
        &means,
        sum_squares,
        frobenius_norm,
        random_access_sum,
        covariance_dimension,
        &covariance,
        kernel_dimension,
        &kernel,
    );
    WorkloadOutcome {
        row_count: u64::try_from(rows).expect("benchmark row count"),
        dimension: table.dimension(),
        logical_digest: qc.logical_digest(),
        numeric_digest,
    }
}

#[allow(clippy::too_many_arguments)]
fn numeric_digest(
    qc: EmbeddingQcSummary,
    means: &[f64],
    sum_squares: f64,
    frobenius_norm: f64,
    random_access_sum: f64,
    covariance_dimension: usize,
    covariance: &[f64],
    kernel_dimension: usize,
    kernel: &[f64],
) -> ContentDigest {
    let mut writer = ContentDigest::builder();
    framed_field(&mut writer, NUMERIC_DIGEST_DOMAIN);
    for count in [
        qc.row_count(),
        qc.present_count(),
        qc.missing_vector_count(),
        qc.extraction_failed_count(),
        qc.qc_rejected_count(),
        qc.all_zero_present_count(),
    ] {
        framed_field(&mut writer, &count.to_be_bytes());
    }
    framed_field(&mut writer, &qc.dimension().to_be_bytes());
    framed_field(
        &mut writer,
        &u64::try_from(RANDOM_ACCESS_COUNT)
            .expect("random access count")
            .to_be_bytes(),
    );
    framed_field(
        &mut writer,
        &u32::try_from(covariance_dimension)
            .expect("covariance dimension")
            .to_be_bytes(),
    );
    framed_field(
        &mut writer,
        &u32::try_from(kernel_dimension)
            .expect("kernel dimension")
            .to_be_bytes(),
    );
    for value in means
        .iter()
        .copied()
        .chain([sum_squares, frobenius_norm, random_access_sum])
        .chain(covariance.iter().copied())
        .chain(kernel.iter().copied())
    {
        framed_field(&mut writer, &value.to_bits().to_be_bytes());
    }
    writer.finish().0
}

fn framed_field(writer: &mut ContentDigestWriter, bytes: &[u8]) {
    writer
        .write_all(
            &u128::try_from(bytes.len())
                .expect("benchmark digest field length")
                .to_be_bytes(),
        )
        .expect("benchmark digest length");
    writer.write_all(bytes).expect("benchmark digest field");
}

fn random_indices(row_count: usize) -> Vec<usize> {
    let mut state = LCG_SEED;
    let row_count = u64::try_from(row_count).expect("benchmark row count");
    let mut indices = Vec::with_capacity(RANDOM_ACCESS_COUNT);
    for _ in 0..RANDOM_ACCESS_COUNT {
        state = state
            .wrapping_mul(LCG_MULTIPLIER)
            .wrapping_add(LCG_INCREMENT);
        indices.push(usize::try_from(state % row_count).expect("benchmark random index"));
    }
    indices
}

fn deterministic_value(row: usize, column: u32) -> f32 {
    let mixed = u64::try_from(row)
        .expect("benchmark row")
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .wrapping_add(u64::from(column).wrapping_mul(0xbf58_476d_1ce4_e5b9));
    let signed =
        i32::from(u16::try_from((mixed >> 24) & 0xffff).expect("benchmark value bits")) - 32_768;
    if signed == 0 {
        0.125
    } else {
        f32::from(i16::try_from(signed).expect("benchmark signed value")) / 4_096.0
    }
}

fn source_csv(row_count: usize) -> Vec<u8> {
    let mut bytes = concat!(
        "cell_id,case_id,specimen_id,timepoint,fragment_id,roi_id,native_row,",
        "embedding_row,x_px,y_px,x_um,y_um,cell_type_id,cell_type_label,",
        "type_probability,nucleus_area_um2,nucleus_perimeter_um,eccentricity,",
        "solidity,circularity,qc_pass,block_500_id,split\r\n"
    )
    .as_bytes()
    .to_vec();
    for row in 0..row_count {
        writeln!(
            bytes,
            "source-{row:08},case,synthetic,time,fragment,roi,{row},{row},1,2,3,4,1,label,0.5,10,5,0.2,0.8,0.7,True,block,train\r"
        )
        .expect("benchmark CSV row");
    }
    bytes
}

fn source_npy(row_count: usize, dimension: u32) -> Vec<u8> {
    let dictionary = format!(
        "{{'descr': '<f4', 'fortran_order': False, 'shape': ({row_count}, {dimension}), }}"
    );
    let padding = (16 - ((10 + dictionary.len() + 1) % 16)) % 16;
    let mut header = dictionary.into_bytes();
    header.resize(header.len() + padding, b' ');
    header.push(b'\n');
    let component_count = row_count
        .checked_mul(usize::try_from(dimension).expect("benchmark source dimension"))
        .expect("benchmark source component count");
    let payload_bytes = component_count
        .checked_mul(size_of::<f32>())
        .expect("benchmark source payload bytes");
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(10 + header.len() + payload_bytes)
        .expect("reserve benchmark NPY");
    bytes.extend_from_slice(b"\x93NUMPY\x01\x00");
    bytes.extend_from_slice(
        &u16::try_from(header.len())
            .expect("benchmark NPY header")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&header);
    for row in 0..row_count {
        for column in 0..dimension {
            bytes.extend_from_slice(&deterministic_value(row, column).to_bits().to_le_bytes());
        }
    }
    bytes
}

fn source_budgets(npy: &[u8], csv: &[u8]) -> SourceBundleBudgets {
    SourceBundleBudgets::new(
        u64::try_from(npy.len()).expect("benchmark NPY bytes"),
        u64::try_from(csv.len()).expect("benchmark CSV bytes"),
        64 * 1024,
        SMOKE_RETAINED_BYTES,
        u64::try_from(SMOKE_ROWS)
            .expect("benchmark source rows")
            .checked_mul(u64::from(SMOKE_DIMENSION))
            .and_then(|count| count.checked_mul(4))
            .expect("benchmark decoded bytes"),
    )
}

fn columnar_budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        256 * 1024 * 1024,
        SMOKE_RETAINED_BYTES,
        512 * 1024 * 1024,
        256 * 1024 * 1024,
    )
}

fn benchmark_cells(row_count: usize) -> Vec<CellId> {
    (0..row_count)
        .map(|row| CellId::new(format!("cell-{row:08}")).expect("benchmark cell"))
        .collect()
}

fn artifact_id(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string())
        .expect("benchmark artifact ID")
}

fn publish_simple_record(store: &LocalArtifactStore, schema: &str) -> ArtifactRecord {
    publish_record(
        store,
        schema,
        "application/json",
        schema.as_bytes(),
        Vec::new(),
        None,
    )
}

fn publish_record(
    store: &LocalArtifactStore,
    schema: &str,
    kind: &str,
    bytes: &[u8],
    dependencies: Vec<ArtifactId>,
    table: Option<marklab::TableManifest>,
) -> ArtifactRecord {
    let draft = ArtifactRecord::new(
        ArtifactRef::from_bytes(kind, bytes).expect("benchmark artifact content"),
        ArtifactSchema::new(schema, 1).expect("benchmark artifact schema"),
        table,
        dependencies,
        BTreeMap::new(),
        vec![ArtifactLocator::new(
            StoreId::new("benchmark-source").expect("benchmark source store"),
            ArtifactKey::new(format!("fixtures/benchmark/{schema}")).expect("benchmark source key"),
            None,
        )
        .expect("benchmark source locator")],
    )
    .expect("benchmark artifact record");
    store
        .publish(&draft, |writer| writer.write_all(bytes))
        .expect("publish benchmark artifact")
        .into_record()
}
