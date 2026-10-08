#![no_main]

use std::{str::FromStr, sync::OnceLock};

use libfuzzer_sys::fuzz_target;
use marklab::{
    ArtifactId, CellId, CohortHierarchy, ContentDigest, CoordinateFrame, CoordinateFrameId,
    CoordinateRegistry, CoordinateSpace, CoordinateUnit, FrameTransform, HierarchyId,
    HierarchyNode, ImageCoordinateConvention, PatchId, PatientId, RegionId, ReplicationRole,
    SlideId, SpatialAxis, TransformId, TransformMatrix,
};
use marklab_embeddings::{
    preflight_cell_embedding_row_link_arrow_bytes, preflight_cell_embedding_row_link_parquet_bytes,
    preflight_cell_embedding_table_arrow_bytes, preflight_cell_embedding_table_parquet_bytes,
    preflight_cell_patch_assignment_table_arrow_bytes,
    preflight_cell_patch_assignment_table_parquet_bytes,
    preflight_cell_patch_edge_table_arrow_bytes, preflight_cell_patch_edge_table_parquet_bytes,
    preflight_patch_embedding_table_arrow_bytes, preflight_patch_embedding_table_parquet_bytes,
    preflight_patch_footprint_set_arrow_bytes, preflight_patch_footprint_set_parquet_bytes,
    preflight_patch_overlap_graph_arrow_bytes, preflight_patch_overlap_graph_parquet_bytes,
    preflight_patch_region_link_arrow_bytes, preflight_patch_region_link_parquet_bytes,
    preflight_region_embedding_table_arrow_bytes, preflight_region_embedding_table_parquet_bytes,
    preflight_slide_embedding_table_arrow_bytes, preflight_slide_embedding_table_parquet_bytes,
    write_cell_embedding_row_link_arrow, write_cell_embedding_row_link_parquet,
    write_cell_embedding_table_arrow, write_cell_embedding_table_parquet,
    write_cell_patch_assignment_table_arrow, write_cell_patch_assignment_table_parquet,
    write_cell_patch_edge_table_arrow, write_cell_patch_edge_table_parquet,
    write_patch_embedding_table_arrow, write_patch_embedding_table_parquet,
    write_patch_footprint_set_arrow, write_patch_footprint_set_parquet,
    write_patch_overlap_graph_arrow, write_patch_overlap_graph_parquet,
    write_patch_region_link_arrow, write_patch_region_link_parquet,
    write_region_embedding_table_arrow, write_region_embedding_table_parquet,
    write_slide_embedding_table_arrow, write_slide_embedding_table_parquet,
    CellEmbeddingProvenance, CellEmbeddingRow, CellEmbeddingRowLink, CellEmbeddingRowLinkEntry,
    CellEmbeddingTable, CellEmbeddingTablePhysicalBindings, CellPatchAnchor, CellPatchLink,
    CellPatchLinkBindings, CellVitCsvSummary, CellVitNpyMatrix, EffectiveReceptiveField,
    EmbeddingColumnarBudgets, EmbeddingStatus, ExpectedCellSet, ExpectedPatchSet,
    ExpectedRegionSet, ExpectedSlideSet, MultiscaleArtifactBinding,
    MultiscaleEmbeddingDerivationContract, MultiscaleEmbeddingExecutionProvenance,
    MultiscaleEmbeddingProvenance, MultiscaleEmbeddingSupport, PatchBoundaryPolicy,
    PatchEmbeddingContext, PatchEmbeddingRow, PatchEmbeddingTable, PatchFootprint,
    PatchFootprintSet, PatchOverlapGraph, PatchRegionAssessment, PatchRegionAssessmentBindings,
    PatchRegionDeclaration, PatchRegionLink, PositiveRational, RegionEmbeddingRow,
    RegionEmbeddingTable, SlideEmbeddingRow, SlideEmbeddingTable, SourceBundleBudgets,
};

const MULTISCALE_BUDGET: usize = 2 * 1024 * 1024;
const MULTISCALE_PHYSICAL_FAMILIES: usize = 8;

struct ArrowSeed {
    bytes: Vec<u8>,
    parquet_bytes: Vec<u8>,
    expected: ExpectedCellSet,
    bindings: CellEmbeddingTablePhysicalBindings,
}

struct RowLinkArrowSeed {
    bytes: Vec<u8>,
    parquet_bytes: Vec<u8>,
    expected: ExpectedCellSet,
    row_link: CellEmbeddingRowLink,
}

struct MultiscaleSeed {
    hierarchy: CohortHierarchy,
    registry: CoordinateRegistry,
    expected_cells: ExpectedCellSet,
    expected_patches: ExpectedPatchSet,
    expected_regions: ExpectedRegionSet,
    expected_slides: ExpectedSlideSet,
    context: PatchEmbeddingContext,
    footprints: PatchFootprintSet,
    overlap: PatchOverlapGraph,
    patch_table: PatchEmbeddingTable,
    region_table: RegionEmbeddingTable,
    slide_table: SlideEmbeddingTable,
    cell_link: CellPatchLink,
    patch_region_link: PatchRegionLink,
    cell_link_bindings: CellPatchLinkBindings,
    patch_region_bindings: PatchRegionAssessmentBindings,
    context_json: Vec<u8>,
    support_json: Vec<u8>,
    derivation_json: Vec<u8>,
    provenance_json: Vec<u8>,
    arrow: [Vec<u8>; MULTISCALE_PHYSICAL_FAMILIES],
    parquet: [Vec<u8>; MULTISCALE_PHYSICAL_FAMILIES],
}

fn artifact_id(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

fn patch_id(value: &str) -> PatchId {
    PatchId::new(value).expect("patch ID")
}

fn region_id(value: &str) -> RegionId {
    RegionId::new(value).expect("region ID")
}

fn multiscale_budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        MULTISCALE_BUDGET as u64,
        MULTISCALE_BUDGET,
        MULTISCALE_BUDGET,
        MULTISCALE_BUDGET as u64,
    )
}

fn multiscale_seed() -> &'static MultiscaleSeed {
    static SEED: OnceLock<MultiscaleSeed> = OnceLock::new();
    SEED.get_or_init(|| {
        let patient = HierarchyId::from(PatientId::new("fuzz-ms-patient").expect("patient ID"));
        let slide_id = SlideId::new("fuzz-ms-slide").expect("slide ID");
        let slide = HierarchyId::from(slide_id.clone());
        let patches = [patch_id("fuzz-ms-patch-a"), patch_id("fuzz-ms-patch-b")];
        let regions = [region_id("fuzz-ms-region-a")];
        let cells = [
            CellId::new("fuzz-ms-cell-a").expect("cell ID"),
            CellId::new("fuzz-ms-cell-b").expect("cell ID"),
        ];
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
        nodes.extend(patches.iter().cloned().map(|id| {
            HierarchyNode::new(
                HierarchyId::from(id),
                Some(slide.clone()),
                ReplicationRole::Structural,
            )
        }));
        nodes.extend(regions.iter().cloned().map(|id| {
            HierarchyNode::new(
                HierarchyId::from(id),
                Some(slide.clone()),
                ReplicationRole::Structural,
            )
        }));
        nodes.extend(cells.iter().cloned().map(|id| {
            HierarchyNode::new(
                HierarchyId::from(id),
                Some(slide.clone()),
                ReplicationRole::Structural,
            )
        }));
        let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("multiscale hierarchy");

        let expected_patches = ExpectedPatchSet::new(
            &hierarchy,
            slide_id.clone(),
            "fuzz_patches.v1",
            patches.to_vec(),
            MULTISCALE_BUDGET,
        )
        .expect("expected patches");
        let expected_regions = ExpectedRegionSet::new(
            &hierarchy,
            slide_id.clone(),
            "fuzz_regions.v1",
            regions.to_vec(),
            MULTISCALE_BUDGET,
        )
        .expect("expected regions");
        let expected_slides = ExpectedSlideSet::new(
            &hierarchy,
            slide_id.clone(),
            "fuzz_slide.v1",
            vec![slide_id.clone()],
            MULTISCALE_BUDGET,
        )
        .expect("expected slide");
        let expected_cells =
            ExpectedCellSet::new("fuzz_cells.v1", cells.to_vec()).expect("expected cells");

        let image_id = CoordinateFrameId::new("fuzz-ms-image").expect("image frame ID");
        let physical_id = CoordinateFrameId::new("fuzz-ms-physical").expect("physical frame ID");
        let transform_id = TransformId::new("fuzz-ms-transform").expect("transform ID");
        let image = CoordinateFrame::new(
            image_id.clone(),
            vec![SpatialAxis::X, SpatialAxis::Y],
            CoordinateUnit::Pixel,
            CoordinateSpace::Image(ImageCoordinateConvention::PixelCenterAtInteger),
        )
        .expect("image frame");
        let physical = CoordinateFrame::new(
            physical_id.clone(),
            vec![SpatialAxis::X, SpatialAxis::Y],
            CoordinateUnit::Micrometer,
            CoordinateSpace::Physical,
        )
        .expect("physical frame");
        let transform = FrameTransform::new(
            transform_id.clone(),
            image_id.clone(),
            physical_id.clone(),
            TransformMatrix::affine_2d([0.5, 0.0, 0.0, 0.0, 0.5, 0.0]).expect("transform matrix"),
            None,
        );
        let registry = CoordinateRegistry::new(
            vec![image, physical],
            Vec::new(),
            vec![transform],
            Vec::new(),
        )
        .expect("coordinate registry");
        let context = PatchEmbeddingContext::new(
            &hierarchy,
            &registry,
            slide_id.clone(),
            image_id.clone(),
            physical_id,
            transform_id,
            PositiveRational::new(1, 2).expect("x scale"),
            PositiveRational::new(1, 2).expect("y scale"),
            [1_024, 512],
            [224, 224],
            [192, 192],
            [32, 32],
            EffectiveReceptiveField::FullInput,
            PatchBoundaryPolicy::FullyContainedOnly,
            MULTISCALE_BUDGET,
        )
        .expect("patch context");
        let expected_patches_artifact_id = artifact_id(b"fuzz-ms-expected-patches");
        let context_artifact_id = artifact_id(b"fuzz-ms-context");
        let footprints = PatchFootprintSet::new(
            &hierarchy,
            &expected_patches,
            expected_patches_artifact_id,
            &context,
            context_artifact_id,
            vec![
                PatchFootprint::new(patches[0].clone(), [0, 0]),
                PatchFootprint::new(patches[1].clone(), [128, 0]),
            ],
            MULTISCALE_BUDGET,
        )
        .expect("patch footprints");
        let footprints_artifact_id = artifact_id(b"fuzz-ms-footprints");
        let overlap = PatchOverlapGraph::derive(
            &expected_patches,
            &context,
            &footprints,
            footprints_artifact_id,
            MULTISCALE_BUDGET,
            MULTISCALE_BUDGET,
        )
        .expect("patch overlap");

        let cell_link_bindings = CellPatchLinkBindings::new(
            artifact_id(b"fuzz-ms-expected-cells"),
            footprints_artifact_id,
            artifact_id(b"fuzz-ms-cell-link-producer"),
            ContentDigest::from_bytes(b"fuzz-ms-cell-link-producer-content"),
            image_id,
        );
        let cell_link = CellPatchLink::derive_contained_shared(
            &hierarchy,
            &expected_cells,
            &expected_patches,
            &context,
            &footprints,
            &cell_link_bindings,
            vec![
                CellPatchAnchor::new(cells[0].clone(), [64.0, 64.0]).expect("cell anchor"),
                CellPatchAnchor::new(cells[1].clone(), [160.0, 64.0]).expect("cell anchor"),
            ],
            64,
            MULTISCALE_BUDGET,
            MULTISCALE_BUDGET,
        )
        .expect("cell-patch link");

        let patch_region_bindings = PatchRegionAssessmentBindings::new(
            artifact_id(b"fuzz-ms-expected-regions"),
            footprints_artifact_id,
            artifact_id(b"fuzz-ms-patch-region-converter"),
            ContentDigest::from_bytes(b"fuzz-ms-patch-region-converter-content"),
        );
        let assessment = PatchRegionAssessment::new(
            &expected_patches,
            &expected_regions,
            &context,
            &footprints,
            &patch_region_bindings,
            vec![
                PatchRegionDeclaration::fully_contained(patches[0].clone(), regions[0].clone()),
                PatchRegionDeclaration::partial_overlap(
                    patches[1].clone(),
                    regions[0].clone(),
                    1,
                    2,
                )
                .expect("partial relation"),
            ],
            MULTISCALE_BUDGET,
            MULTISCALE_BUDGET,
        )
        .expect("patch-region assessment");
        let assessment_json = assessment.to_canonical_json().expect("assessment JSON");
        let patch_region_link = PatchRegionLink::from_exhaustive_assessment(
            &assessment,
            artifact_id(b"fuzz-ms-patch-region-assessment"),
            ContentDigest::from_bytes(&assessment_json),
            MULTISCALE_BUDGET,
            MULTISCALE_BUDGET,
        )
        .expect("patch-region link");

        let patch_support_id = artifact_id(b"fuzz-ms-patch-support");
        let patch_provenance_id = artifact_id(b"fuzz-ms-patch-provenance");
        let patch_table = PatchEmbeddingTable::from_rows(
            3,
            &expected_patches,
            expected_patches_artifact_id,
            patch_support_id,
            ContentDigest::from_bytes(b"fuzz-ms-patch-support-logical"),
            patch_provenance_id,
            ContentDigest::from_bytes(b"fuzz-ms-patch-provenance-logical"),
            vec![
                PatchEmbeddingRow::present(patches[0].clone(), vec![1.0, 0.0, -2.0]),
                PatchEmbeddingRow::non_present(patches[1].clone(), EmbeddingStatus::MissingVector)
                    .expect("missing patch row"),
            ],
            MULTISCALE_BUDGET,
        )
        .expect("patch table");
        let region_table = RegionEmbeddingTable::from_rows(
            3,
            &expected_regions,
            artifact_id(b"fuzz-ms-expected-regions"),
            artifact_id(b"fuzz-ms-region-support"),
            ContentDigest::from_bytes(b"fuzz-ms-region-support-logical"),
            artifact_id(b"fuzz-ms-region-provenance"),
            ContentDigest::from_bytes(b"fuzz-ms-region-provenance-logical"),
            vec![RegionEmbeddingRow::present(
                regions[0].clone(),
                vec![1.0, 2.0, 3.0],
            )],
            MULTISCALE_BUDGET,
        )
        .expect("region table");
        let slide_table = SlideEmbeddingTable::from_rows(
            3,
            &expected_slides,
            artifact_id(b"fuzz-ms-expected-slides"),
            artifact_id(b"fuzz-ms-slide-support"),
            ContentDigest::from_bytes(b"fuzz-ms-slide-support-logical"),
            artifact_id(b"fuzz-ms-slide-provenance"),
            ContentDigest::from_bytes(b"fuzz-ms-slide-provenance-logical"),
            vec![SlideEmbeddingRow::present(
                slide_id.clone(),
                vec![1.0, 2.0, 3.0],
            )],
            MULTISCALE_BUDGET,
        )
        .expect("slide table");

        let patch_support = MultiscaleEmbeddingSupport::patch(
            slide_id.clone(),
            MultiscaleArtifactBinding::new(context_artifact_id, context.logical_digest()),
            MultiscaleArtifactBinding::new(footprints_artifact_id, footprints.logical_digest()),
            MultiscaleArtifactBinding::new(
                artifact_id(b"fuzz-ms-overlap"),
                overlap.logical_digest(),
            ),
            MULTISCALE_BUDGET,
        )
        .expect("patch support");
        let slide_support = MultiscaleEmbeddingSupport::slide_from_patches(
            slide_id.clone(),
            MultiscaleArtifactBinding::new(patch_support_id, patch_support.logical_digest()),
            MultiscaleArtifactBinding::new(
                artifact_id(b"fuzz-ms-source-patch-table"),
                patch_table.logical_digest(),
            ),
            MULTISCALE_BUDGET,
        )
        .expect("slide support");
        let derivation =
            MultiscaleEmbeddingDerivationContract::arithmetic_mean("1.0.0", MULTISCALE_BUDGET)
                .expect("slide derivation");
        let provenance = MultiscaleEmbeddingProvenance::derived_slide_from_patches(
            slide_id,
            &slide_support,
            &derivation,
            3,
            MultiscaleEmbeddingExecutionProvenance::new(
                artifact_id(b"fuzz-ms-run-config"),
                artifact_id(b"fuzz-ms-environment"),
                artifact_id(b"fuzz-ms-provenance-converter"),
                "fuzz_converter",
                "1.0.0",
                MULTISCALE_BUDGET,
            )
            .expect("execution provenance"),
            artifact_id(b"fuzz-ms-source-patch-table"),
            artifact_id(b"fuzz-ms-expected-slides"),
            artifact_id(b"fuzz-ms-slide-support-record"),
            artifact_id(b"fuzz-ms-derivation-record"),
            MULTISCALE_BUDGET,
        )
        .expect("slide provenance");

        let mut arrow: [Vec<u8>; MULTISCALE_PHYSICAL_FAMILIES] =
            std::array::from_fn(|_| Vec::new());
        let mut parquet: [Vec<u8>; MULTISCALE_PHYSICAL_FAMILIES] =
            std::array::from_fn(|_| Vec::new());
        let budgets = multiscale_budgets();
        write_patch_footprint_set_arrow(
            &mut arrow[0],
            &expected_patches,
            &context,
            &footprints,
            budgets,
        )
        .expect("footprint Arrow seed");
        write_patch_footprint_set_parquet(
            &mut parquet[0],
            &expected_patches,
            &context,
            &footprints,
            budgets,
        )
        .expect("footprint Parquet seed");
        write_patch_overlap_graph_arrow(
            &mut arrow[1],
            &expected_patches,
            &context,
            &footprints,
            &overlap,
            budgets,
        )
        .expect("overlap Arrow seed");
        write_patch_overlap_graph_parquet(
            &mut parquet[1],
            &expected_patches,
            &context,
            &footprints,
            &overlap,
            budgets,
        )
        .expect("overlap Parquet seed");
        write_cell_patch_assignment_table_arrow(&mut arrow[2], &cell_link, budgets)
            .expect("assignment Arrow seed");
        write_cell_patch_assignment_table_parquet(&mut parquet[2], &cell_link, budgets)
            .expect("assignment Parquet seed");
        write_cell_patch_edge_table_arrow(&mut arrow[3], &cell_link, budgets)
            .expect("edge Arrow seed");
        write_cell_patch_edge_table_parquet(&mut parquet[3], &cell_link, budgets)
            .expect("edge Parquet seed");
        write_patch_region_link_arrow(&mut arrow[4], &patch_region_link, budgets)
            .expect("patch-region Arrow seed");
        write_patch_region_link_parquet(&mut parquet[4], &patch_region_link, budgets)
            .expect("patch-region Parquet seed");
        write_patch_embedding_table_arrow(&mut arrow[5], &patch_table, budgets)
            .expect("patch matrix Arrow seed");
        write_patch_embedding_table_parquet(&mut parquet[5], &patch_table, budgets)
            .expect("patch matrix Parquet seed");
        write_region_embedding_table_arrow(&mut arrow[6], &region_table, budgets)
            .expect("region matrix Arrow seed");
        write_region_embedding_table_parquet(&mut parquet[6], &region_table, budgets)
            .expect("region matrix Parquet seed");
        write_slide_embedding_table_arrow(&mut arrow[7], &slide_table, budgets)
            .expect("slide matrix Arrow seed");
        write_slide_embedding_table_parquet(&mut parquet[7], &slide_table, budgets)
            .expect("slide matrix Parquet seed");

        MultiscaleSeed {
            hierarchy,
            registry,
            expected_cells,
            expected_patches,
            expected_regions,
            expected_slides,
            context_json: context.to_canonical_json().expect("context JSON"),
            support_json: patch_support.to_canonical_json().expect("support JSON"),
            derivation_json: derivation.to_canonical_json().expect("derivation JSON"),
            provenance_json: provenance.to_canonical_json().expect("provenance JSON"),
            context,
            footprints,
            overlap,
            patch_table,
            region_table,
            slide_table,
            cell_link,
            patch_region_link,
            cell_link_bindings,
            patch_region_bindings,
            arrow,
            parquet,
        }
    })
}

fn arrow_seed() -> &'static ArrowSeed {
    static SEED: OnceLock<ArrowSeed> = OnceLock::new();
    SEED.get_or_init(|| {
        let first_cell = marklab::CellId::new("fuzz-cell-a").expect("cell ID");
        let second_cell = marklab::CellId::new("fuzz-cell-b").expect("cell ID");
        let expected =
            ExpectedCellSet::new("all.v1", vec![first_cell.clone(), second_cell.clone()])
                .expect("expected cells");
        let expected_id = artifact_id(b"fuzz-expected");
        let provenance_id = artifact_id(b"fuzz-provenance");
        let row_link_id = artifact_id(b"fuzz-row-link");
        let row_link_digest = ContentDigest::from_bytes(b"fuzz-row-link-logical");
        let table = CellEmbeddingTable::from_rows(
            3,
            &expected,
            expected_id,
            provenance_id,
            row_link_digest,
            vec![
                CellEmbeddingRow::present(first_cell, vec![1.0, 0.0, -2.5]),
                CellEmbeddingRow::non_present(second_cell, EmbeddingStatus::MissingVector)
                    .expect("missing row"),
            ],
            1024,
        )
        .expect("seed table");
        let bindings = CellEmbeddingTablePhysicalBindings::new(
            expected_id,
            provenance_id,
            row_link_id,
            row_link_digest,
            table.qc_summary().logical_digest(),
        )
        .expect("bindings");
        let budgets = EmbeddingColumnarBudgets::new(
            2 * 1024 * 1024,
            16 * 1024 * 1024,
            16 * 1024 * 1024,
            2 * 1024 * 1024,
        );
        let mut bytes = Vec::new();
        write_cell_embedding_table_arrow(&mut bytes, &table, bindings, budgets)
            .expect("canonical seed");
        let mut parquet_bytes = Vec::new();
        write_cell_embedding_table_parquet(&mut parquet_bytes, &table, bindings, budgets)
            .expect("canonical Parquet seed");
        ArrowSeed {
            bytes,
            parquet_bytes,
            expected,
            bindings,
        }
    })
}

fn row_link_arrow_seed() -> &'static RowLinkArrowSeed {
    static SEED: OnceLock<RowLinkArrowSeed> = OnceLock::new();
    SEED.get_or_init(|| {
        let first_cell = CellId::new("fuzz-link-a").expect("cell ID");
        let second_cell = CellId::new("fuzz-link-b").expect("cell ID");
        let expected =
            ExpectedCellSet::new("all.v1", vec![first_cell.clone(), second_cell.clone()])
                .expect("expected cells");
        let patient = HierarchyId::from(PatientId::new("fuzz-patient").expect("patient ID"));
        let slide = HierarchyId::from(SlideId::new("fuzz-slide").expect("slide ID"));
        let hierarchy = CohortHierarchy::new(
            vec![
                HierarchyNode::new(patient.clone(), None, ReplicationRole::BiologicalUnit),
                HierarchyNode::new(
                    slide.clone(),
                    None,
                    ReplicationRole::TechnicalReplicate {
                        biological_source: patient,
                    },
                ),
                HierarchyNode::new(
                    HierarchyId::from(first_cell.clone()),
                    Some(slide.clone()),
                    ReplicationRole::Structural,
                ),
                HierarchyNode::new(
                    HierarchyId::from(second_cell.clone()),
                    Some(slide),
                    ReplicationRole::Structural,
                ),
            ],
            Vec::new(),
        )
        .expect("hierarchy");
        let row_link = CellEmbeddingRowLink::new(
            artifact_id(b"fuzz-link-source-cells"),
            artifact_id(b"fuzz-link-source-vectors"),
            artifact_id(b"fuzz-link-expected"),
            artifact_id(b"fuzz-link-identity"),
            artifact_id(b"fuzz-link-converter"),
            &expected,
            &hierarchy,
            vec![
                CellEmbeddingRowLinkEntry::present(first_cell, 1, 0),
                CellEmbeddingRowLinkEntry::missing_vector(second_cell, 0),
            ],
            1024,
        )
        .expect("row link");
        let budgets = EmbeddingColumnarBudgets::new(
            2 * 1024 * 1024,
            16 * 1024 * 1024,
            16 * 1024 * 1024,
            2 * 1024 * 1024,
        );
        let mut bytes = Vec::new();
        write_cell_embedding_row_link_arrow(&mut bytes, &row_link, budgets)
            .expect("canonical row-link seed");
        let mut parquet_bytes = Vec::new();
        write_cell_embedding_row_link_parquet(&mut parquet_bytes, &row_link, budgets)
            .expect("canonical row-link Parquet seed");
        RowLinkArrowSeed {
            bytes,
            parquet_bytes,
            expected,
            row_link,
        }
    })
}

fn fuzz_parquet(input: &[u8]) {
    let seed = arrow_seed();
    let mut mutated;
    let candidate = if input.first().is_some_and(|selector| selector & 1 == 0) {
        &input[1..]
    } else {
        mutated = seed.parquet_bytes.clone();
        for mutation in input.get(1..).unwrap_or_default().chunks_exact(3) {
            let raw_offset = usize::from(u16::from_le_bytes([mutation[0], mutation[1]]));
            let length = mutated.len();
            if let Some(byte) = mutated.get_mut(raw_offset % length) {
                *byte ^= mutation[2];
            }
        }
        mutated.as_slice()
    };
    let budgets = EmbeddingColumnarBudgets::new(
        2 * 1024 * 1024,
        16 * 1024 * 1024,
        16 * 1024 * 1024,
        2 * 1024 * 1024,
    );
    let _ = preflight_cell_embedding_table_parquet_bytes(
        candidate,
        &seed.expected,
        3,
        seed.bindings,
        budgets,
    );
}

fn fuzz_arrow(input: &[u8]) {
    let seed = arrow_seed();
    let mut mutated;
    let candidate = if input.first().is_some_and(|selector| selector & 1 == 0) {
        &input[1..]
    } else {
        mutated = seed.bytes.clone();
        for mutation in input.get(1..).unwrap_or_default().chunks_exact(3) {
            let raw_offset = usize::from(u16::from_le_bytes([mutation[0], mutation[1]]));
            let length = mutated.len();
            if let Some(byte) = mutated.get_mut(raw_offset % length) {
                *byte ^= mutation[2];
            }
        }
        mutated.as_slice()
    };
    let budgets = EmbeddingColumnarBudgets::new(
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        2 * 1024 * 1024,
    );
    let _ = preflight_cell_embedding_table_arrow_bytes(
        candidate,
        &seed.expected,
        seed.bindings,
        budgets,
    );
}

fn fuzz_row_link_arrow(input: &[u8]) {
    let seed = row_link_arrow_seed();
    let mut mutated;
    let candidate = if input.first().is_some_and(|selector| selector & 1 == 0) {
        &input[1..]
    } else {
        mutated = seed.bytes.clone();
        for mutation in input.get(1..).unwrap_or_default().chunks_exact(3) {
            let raw_offset = usize::from(u16::from_le_bytes([mutation[0], mutation[1]]));
            let length = mutated.len();
            if let Some(byte) = mutated.get_mut(raw_offset % length) {
                *byte ^= mutation[2];
            }
        }
        mutated.as_slice()
    };
    let budgets = EmbeddingColumnarBudgets::new(
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        2 * 1024 * 1024,
    );
    let _ = preflight_cell_embedding_row_link_arrow_bytes(
        candidate,
        &seed.expected,
        &seed.row_link,
        budgets,
    );
}

fn fuzz_row_link_parquet(input: &[u8]) {
    let seed = row_link_arrow_seed();
    let mut mutated;
    let candidate = if input.first().is_some_and(|selector| selector & 1 == 0) {
        &input[1..]
    } else {
        mutated = seed.parquet_bytes.clone();
        for mutation in input.get(1..).unwrap_or_default().chunks_exact(3) {
            let raw_offset = usize::from(u16::from_le_bytes([mutation[0], mutation[1]]));
            let length = mutated.len();
            if let Some(byte) = mutated.get_mut(raw_offset % length) {
                *byte ^= mutation[2];
            }
        }
        mutated.as_slice()
    };
    let budgets = EmbeddingColumnarBudgets::new(
        2 * 1024 * 1024,
        16 * 1024 * 1024,
        16 * 1024 * 1024,
        2 * 1024 * 1024,
    );
    let _ = preflight_cell_embedding_row_link_parquet_bytes(
        candidate,
        &seed.expected,
        &seed.row_link,
        budgets,
    );
}

fn with_seeded_bytes(input: &[u8], seed: &[u8], use_candidate: impl FnOnce(&[u8])) {
    if input.first().is_some_and(|selector| selector & 1 == 0) {
        use_candidate(input.get(1..).unwrap_or_default());
        return;
    }
    let mut mutated = seed.to_vec();
    for mutation in input.get(1..).unwrap_or_default().chunks_exact(3).take(256) {
        let raw_offset = usize::from(u16::from_le_bytes([mutation[0], mutation[1]]));
        let length = mutated.len();
        if let Some(byte) = mutated.get_mut(raw_offset % length) {
            *byte ^= mutation[2];
        }
    }
    use_candidate(&mutated);
}

fn fuzz_patch_context(input: &[u8]) {
    let seed = multiscale_seed();
    with_seeded_bytes(input, &seed.context_json, |candidate| {
        let _ = PatchEmbeddingContext::from_canonical_json(
            candidate,
            &seed.hierarchy,
            &seed.registry,
            MULTISCALE_BUDGET,
            MULTISCALE_BUDGET,
            MULTISCALE_BUDGET,
        );
    });
}

fn fuzz_footprints_and_overlap(input: &[u8]) {
    let seed = multiscale_seed();
    let x0 = i64::from(input.first().copied().unwrap_or(0)) * 3;
    let x1 = i64::from(input.get(1).copied().unwrap_or(64)) * 3;
    let y0 = i64::from(input.get(2).copied().unwrap_or(0)) % 256;
    let y1 = i64::from(input.get(3).copied().unwrap_or(0)) % 256;
    let rows = vec![
        PatchFootprint::new(seed.expected_patches.ids()[0].clone(), [x0, y0]),
        PatchFootprint::new(seed.expected_patches.ids()[1].clone(), [x1, y1]),
    ];
    if let Ok(footprints) = PatchFootprintSet::new(
        &seed.hierarchy,
        &seed.expected_patches,
        artifact_id(b"fuzz-ms-expected-patches"),
        &seed.context,
        artifact_id(b"fuzz-ms-context"),
        rows,
        MULTISCALE_BUDGET,
    ) {
        let budget_selector = input.get(4).copied().unwrap_or(0);
        let maximum_retained = if budget_selector & 1 == 0 {
            MULTISCALE_BUDGET
        } else {
            usize::from(budget_selector)
        };
        let _ = PatchOverlapGraph::derive(
            &seed.expected_patches,
            &seed.context,
            &footprints,
            artifact_id(b"fuzz-ms-dynamic-footprints"),
            maximum_retained,
            MULTISCALE_BUDGET,
        );
    }
}

fn finite_vector(input: &[u8], dimension: usize, offset: usize) -> Vec<f32> {
    (0..dimension)
        .map(|column| {
            let byte = input.get(offset + column).copied().unwrap_or(0);
            f32::from(i8::from_ne_bytes([byte])) / 16.0
        })
        .collect()
}

fn fuzz_typed_tables(input: &[u8]) {
    let seed = multiscale_seed();
    let dimension_u32 = u32::from(input.first().copied().unwrap_or(2) % 8) + 1;
    let dimension = usize::try_from(dimension_u32).unwrap_or(1);
    let first_patch = seed.expected_patches.ids()[0].clone();
    let second_patch = seed.expected_patches.ids()[1].clone();
    let second_status = match input.get(1).copied().unwrap_or(0) % 4 {
        0 => None,
        1 => Some(EmbeddingStatus::MissingVector),
        2 => Some(EmbeddingStatus::ExtractionFailed),
        _ => Some(EmbeddingStatus::QcRejected),
    };
    let second_row = match second_status {
        None => {
            PatchEmbeddingRow::present(second_patch, finite_vector(input, dimension, dimension + 2))
        }
        Some(status) => {
            let Ok(row) = PatchEmbeddingRow::non_present(second_patch, status) else {
                return;
            };
            row
        }
    };
    let _ = PatchEmbeddingTable::from_rows(
        dimension_u32,
        &seed.expected_patches,
        artifact_id(b"fuzz-ms-dynamic-patch-expected"),
        artifact_id(b"fuzz-ms-dynamic-patch-support"),
        ContentDigest::from_bytes(b"fuzz-ms-dynamic-patch-support-logical"),
        artifact_id(b"fuzz-ms-dynamic-patch-provenance"),
        ContentDigest::from_bytes(b"fuzz-ms-dynamic-patch-provenance-logical"),
        vec![
            PatchEmbeddingRow::present(first_patch, finite_vector(input, dimension, 2)),
            second_row,
        ],
        MULTISCALE_BUDGET,
    );
    let _ = RegionEmbeddingTable::from_rows(
        dimension_u32,
        &seed.expected_regions,
        artifact_id(b"fuzz-ms-dynamic-region-expected"),
        artifact_id(b"fuzz-ms-dynamic-region-support"),
        ContentDigest::from_bytes(b"fuzz-ms-dynamic-region-support-logical"),
        artifact_id(b"fuzz-ms-dynamic-region-provenance"),
        ContentDigest::from_bytes(b"fuzz-ms-dynamic-region-provenance-logical"),
        vec![RegionEmbeddingRow::present(
            seed.expected_regions.ids()[0].clone(),
            finite_vector(input, dimension, 3),
        )],
        MULTISCALE_BUDGET,
    );
    let _ = SlideEmbeddingTable::from_rows(
        dimension_u32,
        &seed.expected_slides,
        artifact_id(b"fuzz-ms-dynamic-slide-expected"),
        artifact_id(b"fuzz-ms-dynamic-slide-support"),
        ContentDigest::from_bytes(b"fuzz-ms-dynamic-slide-support-logical"),
        artifact_id(b"fuzz-ms-dynamic-slide-provenance"),
        ContentDigest::from_bytes(b"fuzz-ms-dynamic-slide-provenance-logical"),
        vec![SlideEmbeddingRow::present(
            seed.expected_slides.ids()[0].clone(),
            finite_vector(input, dimension, 4),
        )],
        MULTISCALE_BUDGET,
    );
}

fn fuzz_multiscale_links(input: &[u8]) {
    let seed = multiscale_seed();
    let anchor = |cell_index: usize, x_index: usize, y_index: usize| {
        CellPatchAnchor::new(
            seed.expected_cells.cells()[cell_index].clone(),
            [
                f64::from(input.get(x_index).copied().unwrap_or(64)),
                f64::from(input.get(y_index).copied().unwrap_or(64)),
            ],
        )
    };
    let (Ok(first_anchor), Ok(second_anchor)) = (anchor(0, 0, 1), anchor(1, 2, 3)) else {
        return;
    };
    let _ = CellPatchLink::derive_contained_shared(
        &seed.hierarchy,
        &seed.expected_cells,
        &seed.expected_patches,
        &seed.context,
        &seed.footprints,
        &seed.cell_link_bindings,
        vec![first_anchor, second_anchor],
        usize::from(input.get(4).copied().unwrap_or(32)),
        MULTISCALE_BUDGET,
        MULTISCALE_BUDGET,
    );

    let mut declarations = vec![PatchRegionDeclaration::fully_contained(
        seed.expected_patches.ids()[0].clone(),
        seed.expected_regions.ids()[0].clone(),
    )];
    if input.get(5).copied().unwrap_or(0) & 1 != 0 {
        if let Ok(declaration) = PatchRegionDeclaration::partial_overlap(
            seed.expected_patches.ids()[1].clone(),
            seed.expected_regions.ids()[0].clone(),
            1,
            2,
        ) {
            declarations.push(declaration);
        }
    }
    if let Ok(assessment) = PatchRegionAssessment::new(
        &seed.expected_patches,
        &seed.expected_regions,
        &seed.context,
        &seed.footprints,
        &seed.patch_region_bindings,
        declarations,
        MULTISCALE_BUDGET,
        MULTISCALE_BUDGET,
    ) {
        let _ = PatchRegionLink::from_exhaustive_assessment(
            &assessment,
            artifact_id(b"fuzz-ms-dynamic-assessment"),
            ContentDigest::from_bytes(input),
            MULTISCALE_BUDGET,
            MULTISCALE_BUDGET,
        );
    }
}

fn fuzz_multiscale_records(input: &[u8]) {
    let seed = multiscale_seed();
    let (selector, mutations) = input.split_first().unwrap_or((&0, &[]));
    match selector % 3 {
        0 => with_seeded_bytes(mutations, &seed.support_json, |candidate| {
            let _ = MultiscaleEmbeddingSupport::from_canonical_json(
                candidate,
                MULTISCALE_BUDGET,
                MULTISCALE_BUDGET,
                MULTISCALE_BUDGET,
            );
        }),
        1 => with_seeded_bytes(mutations, &seed.derivation_json, |candidate| {
            let _ = MultiscaleEmbeddingDerivationContract::from_canonical_json(
                candidate,
                MULTISCALE_BUDGET,
                MULTISCALE_BUDGET,
                MULTISCALE_BUDGET,
            );
        }),
        _ => with_seeded_bytes(mutations, &seed.provenance_json, |candidate| {
            let _ = MultiscaleEmbeddingProvenance::from_canonical_json(
                candidate,
                MULTISCALE_BUDGET,
                MULTISCALE_BUDGET,
                MULTISCALE_BUDGET,
            );
        }),
    }
}

fn fuzz_multiscale_arrow(input: &[u8]) {
    let seed = multiscale_seed();
    let (selector, mutations) = input.split_first().unwrap_or((&0, &[]));
    let family = usize::from(*selector) % MULTISCALE_PHYSICAL_FAMILIES;
    with_seeded_bytes(mutations, &seed.arrow[family], |candidate| {
        let budgets = multiscale_budgets();
        match family {
            0 => {
                let _ = preflight_patch_footprint_set_arrow_bytes(
                    candidate,
                    &seed.expected_patches,
                    &seed.context,
                    &seed.footprints,
                    budgets,
                );
            }
            1 => {
                let _ = preflight_patch_overlap_graph_arrow_bytes(
                    candidate,
                    &seed.expected_patches,
                    &seed.context,
                    &seed.footprints,
                    &seed.overlap,
                    budgets,
                );
            }
            2 => {
                let _ = preflight_cell_patch_assignment_table_arrow_bytes(
                    candidate,
                    &seed.cell_link,
                    budgets,
                );
            }
            3 => {
                let _ = preflight_cell_patch_edge_table_arrow_bytes(
                    candidate,
                    &seed.cell_link,
                    budgets,
                );
            }
            4 => {
                let _ = preflight_patch_region_link_arrow_bytes(
                    candidate,
                    &seed.patch_region_link,
                    budgets,
                );
            }
            5 => {
                let _ = preflight_patch_embedding_table_arrow_bytes(
                    candidate,
                    &seed.patch_table,
                    budgets,
                );
            }
            6 => {
                let _ = preflight_region_embedding_table_arrow_bytes(
                    candidate,
                    &seed.region_table,
                    budgets,
                );
            }
            _ => {
                let _ = preflight_slide_embedding_table_arrow_bytes(
                    candidate,
                    &seed.slide_table,
                    budgets,
                );
            }
        }
    });
}

fn fuzz_multiscale_parquet(input: &[u8]) {
    let seed = multiscale_seed();
    let (selector, mutations) = input.split_first().unwrap_or((&0, &[]));
    let family = usize::from(*selector) % MULTISCALE_PHYSICAL_FAMILIES;
    with_seeded_bytes(mutations, &seed.parquet[family], |candidate| {
        let budgets = multiscale_budgets();
        match family {
            0 => {
                let _ = preflight_patch_footprint_set_parquet_bytes(
                    candidate,
                    &seed.expected_patches,
                    &seed.context,
                    &seed.footprints,
                    budgets,
                );
            }
            1 => {
                let _ = preflight_patch_overlap_graph_parquet_bytes(
                    candidate,
                    &seed.expected_patches,
                    &seed.context,
                    &seed.footprints,
                    &seed.overlap,
                    budgets,
                );
            }
            2 => {
                let _ = preflight_cell_patch_assignment_table_parquet_bytes(
                    candidate,
                    &seed.cell_link,
                    budgets,
                );
            }
            3 => {
                let _ = preflight_cell_patch_edge_table_parquet_bytes(
                    candidate,
                    &seed.cell_link,
                    budgets,
                );
            }
            4 => {
                let _ = preflight_patch_region_link_parquet_bytes(
                    candidate,
                    &seed.patch_region_link,
                    budgets,
                );
            }
            5 => {
                let _ = preflight_patch_embedding_table_parquet_bytes(
                    candidate,
                    &seed.patch_table,
                    budgets,
                );
            }
            6 => {
                let _ = preflight_region_embedding_table_parquet_bytes(
                    candidate,
                    &seed.region_table,
                    budgets,
                );
            }
            _ => {
                let _ = preflight_slide_embedding_table_parquet_bytes(
                    candidate,
                    &seed.slide_table,
                    budgets,
                );
            }
        }
    });
}

fuzz_target!(|bytes: &[u8]| {
    let Some((selector, input)) = bytes.split_first() else {
        return;
    };
    let source_budgets = SourceBundleBudgets::new(
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        64 * 1024,
        2 * 1024 * 1024,
        2 * 1024 * 1024,
    );
    match selector % 14 {
        0 => {
            let _ = CellVitNpyMatrix::from_bytes(input, source_budgets);
        }
        1 => {
            let _ = CellVitCsvSummary::from_bytes(input, source_budgets);
        }
        2 => {
            let _ = CellEmbeddingProvenance::from_canonical_json(input);
        }
        3 => fuzz_arrow(input),
        4 => fuzz_row_link_arrow(input),
        5 => fuzz_parquet(input),
        6 => fuzz_row_link_parquet(input),
        7 => fuzz_patch_context(input),
        8 => fuzz_footprints_and_overlap(input),
        9 => fuzz_typed_tables(input),
        10 => fuzz_multiscale_links(input),
        11 => fuzz_multiscale_records(input),
        12 => fuzz_multiscale_arrow(input),
        _ => fuzz_multiscale_parquet(input),
    }
});
