use std::{io::Write, str::FromStr};

use marklab::{
    publish_cell_patch_assignment_table_arrow, publish_cell_patch_assignment_table_parquet,
    publish_cell_patch_edge_table_arrow, publish_cell_patch_edge_table_parquet,
    publish_patch_embedding_table_arrow, publish_patch_embedding_table_parquet,
    validate_cell_patch_assignment_table_arrow_from_store,
    validate_cell_patch_assignment_table_parquet_from_store,
    validate_cell_patch_edge_table_arrow_from_store,
    validate_cell_patch_edge_table_parquet_from_store,
    validate_patch_embedding_table_arrow_from_store,
    validate_patch_embedding_table_parquet_from_store, ArtifactId, ArtifactRecord, CellId,
    CellPatchAnchor, CellPatchLink, CellPatchLinkBindings, CohortHierarchy, ContentDigest,
    ContentDigestWriter, CoordinateFrame, CoordinateFrameId, CoordinateRegistry, CoordinateSpace,
    CoordinateUnit, EffectiveReceptiveField, EmbeddingColumnarBudgets, ExpectedCellSet,
    ExpectedPatchSet, FrameTransform, HierarchyId, HierarchyNode, ImageCoordinateConvention,
    LocalArtifactStore, PatchBoundaryPolicy, PatchEmbeddingContext, PatchEmbeddingRow,
    PatchEmbeddingTable, PatchFootprint, PatchFootprintSet, PatchId, PatientId, PositiveRational,
    ReplicationRole, SlideId, SpatialAxis, StoreId, TransformId, TransformMatrix,
};
use tempfile::TempDir;

const DIMENSION: u32 = 1_024;
const GRID_COLUMNS: usize = 400;
const PATCH_EXTENT: u64 = 16;
const CELLS_PER_PATCH: usize = 10;
const DOMAIN_BUDGET: usize = 2 * 1024 * 1024 * 1024;
const COLUMNAR_BUDGET: usize = 2 * 1024 * 1024 * 1024;
const NUMERIC_DOMAIN: &[u8] = b"marklab-patch-embedding-benchmark-numerics-v1";
const LINK_DOMAIN: &[u8] = b"marklab-patch-embedding-benchmark-links-v1";
#[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
const DHAT_PEAK_LIMIT: usize = 320 * 1024 * 1024;

const SMOKE_TABLE_LOGICAL: &str =
    "6aa1ef12bf4955b4f58460b01f3a35975ddf147a57803a42d83359c9d4eb11c0";
const SMOKE_LINK_LOGICAL: &str = "c447edd055d8f6e44cecd739a31ac157c1c3b95c690eaf64469ab53806fc9a47";
const SMOKE_NUMERIC_DIGEST: &str =
    "ccb9ab3ea4ca78f26c39b7b97028757feace6b746065d8e11abbdc74df91ce65";
const SMOKE_LINK_CHECKSUM: &str =
    "b7472ac57b761db386a4cbd19470bc7b1006fbf69a1fbcf4869db5cc5ebe0ce9";
const FULL_TABLE_LOGICAL: &str = "113b3d67889d901bc987fd99a20e0fd94c9c1fd5d1f5179bd5b8ae3f995e9033";
const FULL_LINK_LOGICAL: &str = "1972def442ed8c5d980f86f2f6a28b58f54a8fcec979fd2e579ad8fe281eefee";
const FULL_NUMERIC_DIGEST: &str =
    "9520cc35c365025f630dd0d31d7bf0b77111ff347062b070a11064aeca8acc65";
const FULL_LINK_CHECKSUM: &str = "c081e62e6c108de5645df53d4f7c87c3f4a2671cc35ecab34e200c056f22a6cd";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BenchmarkProfile {
    Smoke,
    Full,
}

impl BenchmarkProfile {
    fn selected() -> Self {
        if std::env::var("MARKLAB_BENCH_PROFILE").as_deref() == Ok("full")
            || std::env::args().any(|argument| argument.contains("100k_x_1024_1m_links"))
        {
            Self::Full
        } else {
            Self::Smoke
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Smoke => "10k_x_1024_100k_links",
            Self::Full => "100k_x_1024_1m_links",
        }
    }

    fn patch_count(self) -> usize {
        match self {
            Self::Smoke => 10_000,
            Self::Full => 100_000,
        }
    }

    fn assignment_count(self) -> usize {
        self.patch_count()
            .checked_mul(CELLS_PER_PATCH)
            .expect("benchmark assignment count")
    }

    fn candidate_checks(self) -> usize {
        let patches = self.patch_count();
        CELLS_PER_PATCH
            .checked_mul(
                patches
                    .checked_mul(4)
                    .and_then(|value| value.checked_sub(2 * (patches / GRID_COLUMNS)))
                    .and_then(|value| value.checked_sub(799))
                    .expect("benchmark candidate formula"),
            )
            .expect("benchmark candidate checks")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PatchEmbeddingBenchmarkOutcome {
    patch_count: u64,
    value_count: u64,
    assignment_count: u64,
    edge_count: u64,
    table_logical_digest: ContentDigest,
    link_logical_digest: ContentDigest,
    numeric_digest: ContentDigest,
    link_checksum: ContentDigest,
}

impl PatchEmbeddingBenchmarkOutcome {
    pub fn table_logical_digest(self) -> ContentDigest {
        self.table_logical_digest
    }

    pub fn link_logical_digest(self) -> ContentDigest {
        self.link_logical_digest
    }

    pub fn numeric_digest(self) -> ContentDigest {
        self.numeric_digest
    }

    pub fn link_checksum(self) -> ContentDigest {
        self.link_checksum
    }
}

pub struct PreparedPatchEmbeddingBenchmark {
    profile: BenchmarkProfile,
    table: PatchEmbeddingTable,
    link: CellPatchLink,
    expected: PatchEmbeddingBenchmarkOutcome,
    _root: TempDir,
    _store: LocalArtifactStore,
    _physical_records: Vec<ArtifactRecord>,
    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    columnar_budgets: EmbeddingColumnarBudgets,
    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    dhat_hierarchy: CohortHierarchy,
    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    dhat_expected_cells: ExpectedCellSet,
    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    dhat_expected_patches: ExpectedPatchSet,
    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    dhat_context: PatchEmbeddingContext,
    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    dhat_footprints: PatchFootprintSet,
    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    dhat_bindings: CellPatchLinkBindings,
    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    dhat_anchors: Vec<CellPatchAnchor>,
}

impl PreparedPatchEmbeddingBenchmark {
    pub fn selected() -> Self {
        Self::new(BenchmarkProfile::selected())
    }

    fn new(profile: BenchmarkProfile) -> Self {
        let fixture = Fixture::new(profile);
        let observed = workload_outcome(profile, &fixture.table, &fixture.link);
        assert_eq!(
            observed.numeric_digest,
            reference_numeric_checksum(profile),
            "table traversal differs from the independent generator traversal"
        );
        assert_eq!(
            observed.link_checksum,
            reference_link_checksum(profile),
            "link traversal differs from the independent profile mapping"
        );
        let expected = expected_outcome(profile);
        assert_eq!(
            observed, expected,
            "replace only the reviewed profile goldens; observed={observed:?}"
        );

        let columnar_budgets = columnar_budgets();
        let root = TempDir::new().expect("patch benchmark store root");
        let store = LocalArtifactStore::open(
            root.path(),
            StoreId::new(format!("patch-benchmark-{}", profile.name())).expect("benchmark store"),
        )
        .expect("benchmark store");
        let physical_records =
            publish_physical(&store, &fixture.table, &fixture.link, columnar_budgets);
        validate_physical(
            &store,
            &physical_records,
            &fixture.table,
            &fixture.link,
            columnar_budgets,
        );

        Self {
            profile,
            table: fixture.table,
            link: fixture.link,
            expected,
            _root: root,
            _store: store,
            _physical_records: physical_records,
            #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
            columnar_budgets,
            #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
            dhat_hierarchy: fixture.hierarchy,
            #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
            dhat_expected_cells: fixture.expected_cells,
            #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
            dhat_expected_patches: fixture.expected_patches,
            #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
            dhat_context: fixture.context,
            #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
            dhat_footprints: fixture.footprints,
            #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
            dhat_bindings: fixture.bindings,
            #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
            dhat_anchors: fixture.dhat_anchors,
        }
    }

    pub fn name(&self) -> &'static str {
        self.profile.name()
    }

    pub fn patch_count(&self) -> u64 {
        u64::try_from(self.profile.patch_count()).expect("patch count")
    }

    pub fn dimension(&self) -> u32 {
        DIMENSION
    }

    pub fn value_count(&self) -> u64 {
        self.patch_count()
            .checked_mul(u64::from(DIMENSION))
            .expect("value count")
    }

    pub fn assignment_count(&self) -> u64 {
        u64::try_from(self.profile.assignment_count()).expect("assignment count")
    }

    pub fn edge_count(&self) -> u64 {
        self.assignment_count()
    }

    pub fn expected_outcome(&self) -> PatchEmbeddingBenchmarkOutcome {
        self.expected
    }

    pub fn run(&self) -> PatchEmbeddingBenchmarkOutcome {
        workload_outcome(self.profile, &self.table, &self.link)
    }

    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    pub fn run_with_fresh_publication(&mut self) -> PatchEmbeddingBenchmarkOutcome {
        let anchors = std::mem::take(&mut self.dhat_anchors);
        let link = CellPatchLink::derive_contained_shared(
            &self.dhat_hierarchy,
            &self.dhat_expected_cells,
            &self.dhat_expected_patches,
            &self.dhat_context,
            &self.dhat_footprints,
            &self.dhat_bindings,
            anchors,
            self.profile.candidate_checks(),
            DOMAIN_BUDGET,
            DOMAIN_BUDGET,
        )
        .expect("DHAT contained-shared link");
        let output_root = TempDir::new().expect("DHAT patch output root");
        let output_store = LocalArtifactStore::open(
            output_root.path(),
            StoreId::new("patch-benchmark-dhat").expect("DHAT store ID"),
        )
        .expect("DHAT output store");
        let records = publish_physical(&output_store, &self.table, &link, self.columnar_budgets);
        let outcome = workload_outcome(self.profile, &self.table, &link);
        assert_eq!(outcome, self.expected);
        drop(records);
        drop(link);
        drop(output_store);
        drop(output_root);
        outcome
    }

    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    pub fn dhat_peak_limit(&self) -> usize {
        DHAT_PEAK_LIMIT
    }

    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    pub fn forbidden_edge_vector_bytes(&self) -> usize {
        self.profile
            .assignment_count()
            .checked_mul(DIMENSION as usize)
            .and_then(|value| value.checked_mul(size_of::<f32>()))
            .expect("forbidden copied-vector bytes")
    }
}

#[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
use std::mem::size_of;

struct Fixture {
    table: PatchEmbeddingTable,
    link: CellPatchLink,
    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    hierarchy: CohortHierarchy,
    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    expected_cells: ExpectedCellSet,
    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    expected_patches: ExpectedPatchSet,
    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    context: PatchEmbeddingContext,
    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    footprints: PatchFootprintSet,
    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    bindings: CellPatchLinkBindings,
    #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
    dhat_anchors: Vec<CellPatchAnchor>,
}

impl Fixture {
    fn new(profile: BenchmarkProfile) -> Self {
        let patch_ids = (0..profile.patch_count()).map(patch_id).collect::<Vec<_>>();
        let cell_ids = (0..profile.assignment_count())
            .map(cell_id)
            .collect::<Vec<_>>();
        let patient =
            HierarchyId::from(PatientId::new("patch-benchmark-patient").expect("patient"));
        let slide_id = SlideId::new("patch-benchmark-slide").expect("slide");
        let slide = HierarchyId::from(slide_id.clone());
        let mut nodes = Vec::with_capacity(
            patch_ids
                .len()
                .checked_add(cell_ids.len())
                .and_then(|value| value.checked_add(2))
                .expect("hierarchy node count"),
        );
        nodes.push(HierarchyNode::new(
            patient.clone(),
            None,
            ReplicationRole::BiologicalUnit,
        ));
        nodes.push(HierarchyNode::new(
            slide.clone(),
            None,
            ReplicationRole::TechnicalReplicate {
                biological_source: patient,
            },
        ));
        nodes.extend(patch_ids.iter().cloned().map(|patch| {
            HierarchyNode::new(
                HierarchyId::from(patch),
                Some(slide.clone()),
                ReplicationRole::Structural,
            )
        }));
        nodes.extend(cell_ids.iter().cloned().map(|cell| {
            HierarchyNode::new(
                HierarchyId::from(cell),
                Some(slide.clone()),
                ReplicationRole::Structural,
            )
        }));
        let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("benchmark hierarchy");
        let expected_patches = ExpectedPatchSet::new(
            &hierarchy,
            slide_id.clone(),
            "patch_benchmark_patches.v1",
            patch_ids.clone(),
            DOMAIN_BUDGET,
        )
        .expect("benchmark expected patches");
        let expected_cells = ExpectedCellSet::new("patch_benchmark_cells.v1", cell_ids.clone())
            .expect("benchmark expected cells");
        let (context, image_frame_id) =
            benchmark_context(&hierarchy, slide_id, profile.patch_count());
        let expected_patches_artifact_id = artifact_id(b"patch-benchmark-expected-patches");
        let context_artifact_id = artifact_id(b"patch-benchmark-context");
        let footprints = PatchFootprintSet::new(
            &hierarchy,
            &expected_patches,
            expected_patches_artifact_id,
            &context,
            context_artifact_id,
            patch_ids
                .iter()
                .enumerate()
                .map(|(row, patch)| PatchFootprint::new(patch.clone(), patch_origin(row)))
                .collect(),
            DOMAIN_BUDGET,
        )
        .expect("benchmark footprints");
        let table = PatchEmbeddingTable::from_rows(
            DIMENSION,
            &expected_patches,
            expected_patches_artifact_id,
            artifact_id(b"patch-benchmark-support"),
            ContentDigest::from_bytes(b"patch-benchmark-support-logical"),
            artifact_id(b"patch-benchmark-provenance"),
            ContentDigest::from_bytes(b"patch-benchmark-provenance-logical"),
            patch_ids
                .iter()
                .enumerate()
                .map(|(row, patch)| {
                    PatchEmbeddingRow::present(
                        patch.clone(),
                        (0..DIMENSION as usize)
                            .map(|column| deterministic_value(row, column))
                            .collect(),
                    )
                })
                .collect(),
            DOMAIN_BUDGET,
        )
        .expect("benchmark patch table");
        let bindings = CellPatchLinkBindings::new(
            artifact_id(b"patch-benchmark-expected-cells"),
            artifact_id(b"patch-benchmark-footprints"),
            artifact_id(b"patch-benchmark-producer"),
            ContentDigest::from_bytes(b"patch-benchmark-producer-content"),
            image_frame_id,
        );
        let anchors = cell_ids
            .iter()
            .enumerate()
            .map(|(assignment, cell)| {
                let patch_row = assignment / CELLS_PER_PATCH;
                let origin = patch_origin(patch_row);
                CellPatchAnchor::new(
                    cell.clone(),
                    [origin[0] as f64 + 8.0, origin[1] as f64 + 8.0],
                )
                .expect("benchmark anchor")
            })
            .collect::<Vec<_>>();
        #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
        let dhat_anchors = anchors.clone();
        let link = CellPatchLink::derive_contained_shared(
            &hierarchy,
            &expected_cells,
            &expected_patches,
            &context,
            &footprints,
            &bindings,
            anchors,
            profile.candidate_checks(),
            DOMAIN_BUDGET,
            DOMAIN_BUDGET,
        )
        .expect("benchmark contained-shared link");
        assert_eq!(link.assignment_count(), profile.assignment_count());
        assert_eq!(link.edge_count(), profile.assignment_count());
        assert!(link
            .assignments()
            .iter()
            .all(|assignment| assignment.edge_count() == 1));
        assert!(link.edges().iter().all(|edge| edge.weight().is_none()));

        Self {
            table,
            link,
            #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
            hierarchy,
            #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
            expected_cells,
            #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
            expected_patches,
            #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
            context,
            #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
            footprints,
            #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
            bindings,
            #[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
            dhat_anchors,
        }
    }
}

fn benchmark_context(
    hierarchy: &CohortHierarchy,
    slide_id: SlideId,
    patch_count: usize,
) -> (PatchEmbeddingContext, CoordinateFrameId) {
    let image = CoordinateFrameId::new("patch-benchmark-image-pixels").expect("image frame");
    let physical =
        CoordinateFrameId::new("patch-benchmark-slide-micrometers").expect("physical frame");
    let transform = TransformId::new("patch-benchmark-pixel-to-micrometer").expect("transform");
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
            TransformMatrix::affine_2d([1.0, 0.0, 0.0, 0.0, 1.0, 0.0])
                .expect("benchmark transform"),
            None,
        )],
        Vec::new(),
    )
    .expect("benchmark coordinate registry");
    let grid_rows = patch_count.div_ceil(GRID_COLUMNS);
    let source_image_px = [
        u64::try_from(GRID_COLUMNS).expect("grid columns") * PATCH_EXTENT,
        u64::try_from(grid_rows).expect("grid rows") * PATCH_EXTENT,
    ];
    let context = PatchEmbeddingContext::new(
        hierarchy,
        &registry,
        slide_id,
        image.clone(),
        physical,
        transform,
        PositiveRational::new(1, 1).expect("x scale"),
        PositiveRational::new(1, 1).expect("y scale"),
        source_image_px,
        [16, 16],
        [16, 16],
        [0, 0],
        EffectiveReceptiveField::FullInput,
        PatchBoundaryPolicy::FullyContainedOnly,
        DOMAIN_BUDGET,
    )
    .expect("benchmark patch context");
    (context, image)
}

fn patch_origin(row: usize) -> [i64; 2] {
    [
        i64::try_from((row % GRID_COLUMNS) * PATCH_EXTENT as usize).expect("patch x"),
        i64::try_from((row / GRID_COLUMNS) * PATCH_EXTENT as usize).expect("patch y"),
    ]
}

fn patch_id(row: usize) -> PatchId {
    PatchId::new(format!("patch-{row:06}")).expect("benchmark patch ID")
}

fn cell_id(assignment: usize) -> CellId {
    CellId::new(format!("cell-{assignment:07}")).expect("benchmark cell ID")
}

fn deterministic_value(row: usize, column: usize) -> f32 {
    let mixed = (row as u64)
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .wrapping_add((column as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9));
    let raw = ((mixed >> 24) & 0xffff) as i32 - 32_768;
    if raw == 0 {
        0.125
    } else {
        raw as f32 / 4096.0
    }
}

fn workload_outcome(
    profile: BenchmarkProfile,
    table: &PatchEmbeddingTable,
    link: &CellPatchLink,
) -> PatchEmbeddingBenchmarkOutcome {
    assert_eq!(table.row_count(), profile.patch_count());
    assert_eq!(table.dimension(), DIMENSION);
    assert_eq!(link.assignment_count(), profile.assignment_count());
    assert_eq!(link.edge_count(), profile.assignment_count());
    let (sum, sum_of_squares) = table_arithmetic(table);
    PatchEmbeddingBenchmarkOutcome {
        patch_count: u64::try_from(table.row_count()).expect("patch count"),
        value_count: u64::try_from(table.row_count())
            .expect("patch count")
            .checked_mul(u64::from(table.dimension()))
            .expect("value count"),
        assignment_count: u64::try_from(link.assignment_count()).expect("assignment count"),
        edge_count: u64::try_from(link.edge_count()).expect("edge count"),
        table_logical_digest: table.logical_digest(),
        link_logical_digest: link.logical_digest(),
        numeric_digest: numeric_checksum(table, link, sum, sum_of_squares),
        link_checksum: link_checksum(link),
    }
}

fn table_arithmetic(table: &PatchEmbeddingTable) -> (f64, f64) {
    let mut sum = 0.0_f64;
    let mut sum_of_squares = 0.0_f64;
    for start in (0..table.row_count()).step_by(8_192) {
        let count = (table.row_count() - start).min(8_192);
        let block = table.block(start, count).expect("benchmark table block");
        for row in block.rows() {
            let vector = row.vector().expect("all benchmark rows are present");
            for value in vector {
                let promoted = f64::from(*value);
                sum += promoted;
                sum_of_squares += promoted * promoted;
            }
        }
    }
    (sum, sum_of_squares)
}

fn numeric_checksum(
    table: &PatchEmbeddingTable,
    link: &CellPatchLink,
    sum: f64,
    sum_of_squares: f64,
) -> ContentDigest {
    let qc = table.qc_summary();
    let mut digest = FramedChecksum::new(NUMERIC_DOMAIN);
    digest.u64(qc.row_count());
    digest.u64(
        qc.row_count()
            .checked_mul(u64::from(table.dimension()))
            .expect("numeric value count"),
    );
    digest.u64(u64::try_from(link.assignment_count()).expect("assignment count"));
    digest.u64(u64::try_from(link.edge_count()).expect("edge count"));
    digest.u64(qc.row_count());
    digest.u64(qc.present_count());
    digest.u64(qc.missing_vector_count());
    digest.u64(qc.extraction_failed_count());
    digest.u64(qc.qc_rejected_count());
    digest.u64(qc.all_zero_present_count());
    digest.u32(table.dimension());
    digest.u64(sum.to_bits());
    digest.u64(sum_of_squares.to_bits());
    digest.finish()
}

fn link_checksum(link: &CellPatchLink) -> ContentDigest {
    let mut digest = FramedChecksum::new(LINK_DOMAIN);
    digest.u64(u64::try_from(link.assignment_count()).expect("assignment count"));
    digest.u64(u64::try_from(link.edge_count()).expect("edge count"));
    for (row, assignment) in link.assignments().iter().enumerate() {
        digest.u64(u64::try_from(row).expect("assignment row"));
        digest.u64(assignment.edge_start());
        digest.u64(assignment.edge_count());
    }
    for edge in link.edges() {
        digest.u64(edge.assignment_row());
        digest.text(edge.patch_id().as_str());
    }
    digest.finish()
}

fn reference_numeric_checksum(profile: BenchmarkProfile) -> ContentDigest {
    let mut sum = 0.0_f64;
    let mut sum_of_squares = 0.0_f64;
    for row in 0..profile.patch_count() {
        for column in 0..DIMENSION as usize {
            let promoted = f64::from(deterministic_value(row, column));
            sum += promoted;
            sum_of_squares += promoted * promoted;
        }
    }
    let patch_count = u64::try_from(profile.patch_count()).expect("patch count");
    let assignment_count = u64::try_from(profile.assignment_count()).expect("assignment count");
    let mut digest = FramedChecksum::new(NUMERIC_DOMAIN);
    digest.u64(patch_count);
    digest.u64(
        patch_count
            .checked_mul(u64::from(DIMENSION))
            .expect("reference value count"),
    );
    digest.u64(assignment_count);
    digest.u64(assignment_count);
    digest.u64(patch_count);
    digest.u64(patch_count);
    digest.u64(0);
    digest.u64(0);
    digest.u64(0);
    digest.u64(0);
    digest.u32(DIMENSION);
    digest.u64(sum.to_bits());
    digest.u64(sum_of_squares.to_bits());
    digest.finish()
}

fn reference_link_checksum(profile: BenchmarkProfile) -> ContentDigest {
    let assignment_count = profile.assignment_count();
    let mut digest = FramedChecksum::new(LINK_DOMAIN);
    digest.u64(u64::try_from(assignment_count).expect("assignment count"));
    digest.u64(u64::try_from(assignment_count).expect("edge count"));
    for assignment in 0..assignment_count {
        let row = u64::try_from(assignment).expect("assignment row");
        digest.u64(row);
        digest.u64(row);
        digest.u64(1);
    }
    for assignment in 0..assignment_count {
        digest.u64(u64::try_from(assignment).expect("edge assignment row"));
        digest.text(patch_id(assignment / CELLS_PER_PATCH).as_str());
    }
    digest.finish()
}

struct FramedChecksum(ContentDigestWriter);

impl FramedChecksum {
    fn new(domain: &[u8]) -> Self {
        let mut checksum = Self(ContentDigestWriter::default());
        checksum.bytes(domain);
        checksum
    }

    fn bytes(&mut self, value: &[u8]) {
        self.0
            .write_all(&(value.len() as u128).to_be_bytes())
            .expect("checksum length");
        self.0.write_all(value).expect("checksum value");
    }

    fn text(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_be_bytes());
    }

    fn finish(self) -> ContentDigest {
        self.0.finish().0
    }
}

fn expected_outcome(profile: BenchmarkProfile) -> PatchEmbeddingBenchmarkOutcome {
    let (table, link, numeric, link_checksum) = match profile {
        BenchmarkProfile::Smoke => (
            SMOKE_TABLE_LOGICAL,
            SMOKE_LINK_LOGICAL,
            SMOKE_NUMERIC_DIGEST,
            SMOKE_LINK_CHECKSUM,
        ),
        BenchmarkProfile::Full => (
            FULL_TABLE_LOGICAL,
            FULL_LINK_LOGICAL,
            FULL_NUMERIC_DIGEST,
            FULL_LINK_CHECKSUM,
        ),
    };
    let patch_count = u64::try_from(profile.patch_count()).expect("expected patch count");
    let assignment_count =
        u64::try_from(profile.assignment_count()).expect("expected assignment count");
    PatchEmbeddingBenchmarkOutcome {
        patch_count,
        value_count: patch_count
            .checked_mul(u64::from(DIMENSION))
            .expect("expected value count"),
        assignment_count,
        edge_count: assignment_count,
        table_logical_digest: ContentDigest::from_str(table).expect("table golden"),
        link_logical_digest: ContentDigest::from_str(link).expect("link golden"),
        numeric_digest: ContentDigest::from_str(numeric).expect("numeric golden"),
        link_checksum: ContentDigest::from_str(link_checksum).expect("link checksum golden"),
    }
}

fn artifact_id(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

fn columnar_budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        COLUMNAR_BUDGET as u64,
        COLUMNAR_BUDGET,
        COLUMNAR_BUDGET,
        COLUMNAR_BUDGET as u64,
    )
}

fn publish_physical(
    store: &LocalArtifactStore,
    table: &PatchEmbeddingTable,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Vec<ArtifactRecord> {
    vec![
        publish_patch_embedding_table_arrow(store, table, budgets)
            .expect("publish patch Arrow")
            .into_record(),
        publish_patch_embedding_table_parquet(store, table, budgets)
            .expect("publish patch Parquet")
            .into_record(),
        publish_cell_patch_assignment_table_arrow(store, link, budgets)
            .expect("publish assignment Arrow")
            .into_record(),
        publish_cell_patch_edge_table_arrow(store, link, budgets)
            .expect("publish edge Arrow")
            .into_record(),
        publish_cell_patch_assignment_table_parquet(store, link, budgets)
            .expect("publish assignment Parquet")
            .into_record(),
        publish_cell_patch_edge_table_parquet(store, link, budgets)
            .expect("publish edge Parquet")
            .into_record(),
    ]
}

fn validate_physical(
    store: &LocalArtifactStore,
    records: &[ArtifactRecord],
    table: &PatchEmbeddingTable,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) {
    validate_patch_embedding_table_arrow_from_store(store, &records[0], table, budgets)
        .expect("validate patch Arrow");
    validate_patch_embedding_table_parquet_from_store(store, &records[1], table, budgets)
        .expect("validate patch Parquet");
    validate_cell_patch_assignment_table_arrow_from_store(store, &records[2], link, budgets)
        .expect("validate assignment Arrow");
    validate_cell_patch_edge_table_arrow_from_store(store, &records[3], link, budgets)
        .expect("validate edge Arrow");
    validate_cell_patch_assignment_table_parquet_from_store(store, &records[4], link, budgets)
        .expect("validate assignment Parquet");
    validate_cell_patch_edge_table_parquet_from_store(store, &records[5], link, budgets)
        .expect("validate edge Parquet");
}
