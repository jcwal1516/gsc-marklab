use std::str::FromStr;

use marklab::{
    ArtifactId, CellId, CellPatchAnchor, CellPatchContributor, CellPatchLink,
    CellPatchLinkBindings, CohortHierarchy, ContentDigest, CoordinateFrame, CoordinateFrameId,
    CoordinateRegistry, CoordinateSpace, CoordinateUnit, DeclaredCellPatchAssignment,
    EffectiveReceptiveField, ExpectedCellSet, ExpectedPatchSet, FrameTransform, HierarchyId,
    HierarchyNode, ImageCoordinateConvention, PatchBoundaryPolicy, PatchEmbeddingContext,
    PatchFootprint, PatchFootprintSet, PatchId, PatientId, PositiveRational, ReplicationRole,
    SlideId, SpatialAxis, TransformId, TransformMatrix,
};

const DOMAIN_BUDGET: usize = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CellPatchFixtureMode {
    Contained,
    Interpolation,
}

pub(crate) struct CellPatchFixture {
    pub(crate) link: CellPatchLink,
}

fn artifact(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

pub(crate) fn cell_patch_fixture(mode: CellPatchFixtureMode) -> CellPatchFixture {
    let patient = HierarchyId::from(PatientId::new("physical-patient").expect("patient ID"));
    let slide_id = SlideId::new("physical-slide").expect("slide ID");
    let slide = HierarchyId::from(slide_id.clone());
    let cells = (0..3)
        .map(|index| CellId::new(format!("cell-{index:08}")).expect("cell ID"))
        .collect::<Vec<_>>();
    let patches = (0..3)
        .map(|index| PatchId::new(format!("patch-{index:08}")).expect("patch ID"))
        .collect::<Vec<_>>();
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
    nodes.extend(cells.iter().cloned().map(|cell| {
        HierarchyNode::new(
            HierarchyId::from(cell),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    nodes.extend(patches.iter().cloned().map(|patch| {
        HierarchyNode::new(
            HierarchyId::from(patch),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("hierarchy");
    let expected_cells =
        ExpectedCellSet::new("all_cells.v1", cells.clone()).expect("expected cells");
    let expected_patches = ExpectedPatchSet::new(
        &hierarchy,
        slide_id.clone(),
        "all_patches.v1",
        patches.clone(),
        DOMAIN_BUDGET,
    )
    .expect("expected patches");
    let context = context(&hierarchy, slide_id);
    let footprints = PatchFootprintSet::new(
        &hierarchy,
        &expected_patches,
        artifact(b"cell-patch-physical-expected-patches"),
        &context,
        artifact(b"cell-patch-physical-context"),
        patches
            .iter()
            .cloned()
            .zip([0, 192, 384])
            .map(|(patch, origin_x)| PatchFootprint::new(patch, [origin_x, 0]))
            .collect(),
        DOMAIN_BUDGET,
    )
    .expect("footprints");
    let bindings = CellPatchLinkBindings::new(
        artifact(b"cell-patch-physical-expected-cells"),
        artifact(b"cell-patch-physical-footprints"),
        artifact(b"cell-patch-physical-producer"),
        ContentDigest::from_bytes(b"cell-patch-physical-producer-content"),
        CoordinateFrameId::new("physical-image-pixels").expect("image frame ID"),
    );
    let link = match mode {
        CellPatchFixtureMode::Contained => CellPatchLink::derive_contained_shared(
            &hierarchy,
            &expected_cells,
            &expected_patches,
            &context,
            &footprints,
            &bindings,
            vec![
                CellPatchAnchor::new(cells[0].clone(), [200.0, 10.0]).expect("anchor"),
                CellPatchAnchor::new(cells[1].clone(), [10.0, 10.0]).expect("anchor"),
                CellPatchAnchor::new(cells[2].clone(), [1_000.0, 10.0]).expect("anchor"),
            ],
            DOMAIN_BUDGET,
            DOMAIN_BUDGET,
            DOMAIN_BUDGET,
        ),
        CellPatchFixtureMode::Interpolation => CellPatchLink::from_declared_weighted_interpolation(
            &hierarchy,
            &expected_cells,
            &expected_patches,
            &context,
            &footprints,
            &bindings,
            vec![
                DeclaredCellPatchAssignment::new(
                    CellPatchAnchor::new(cells[0].clone(), [200.0, 10.0]).expect("anchor"),
                    vec![
                        CellPatchContributor::new(patches[0].clone(), 1, 2).expect("contributor"),
                        CellPatchContributor::new(patches[1].clone(), 1, 2).expect("contributor"),
                    ],
                ),
                DeclaredCellPatchAssignment::new(
                    CellPatchAnchor::new(cells[1].clone(), [10.0, 10.0]).expect("anchor"),
                    vec![CellPatchContributor::new(patches[2].clone(), 1, 1).expect("contributor")],
                ),
                DeclaredCellPatchAssignment::new(
                    CellPatchAnchor::new(cells[2].clone(), [1_000.0, 10.0]).expect("anchor"),
                    Vec::new(),
                ),
            ],
            DOMAIN_BUDGET,
            DOMAIN_BUDGET,
        ),
    }
    .expect("cell-patch link");
    CellPatchFixture { link }
}

#[allow(dead_code)]
pub(crate) fn interpolation_scale_fixture(row_count: usize) -> CellPatchFixture {
    assert!(row_count > 0);
    let patient = HierarchyId::from(PatientId::new("scale-patient").expect("patient ID"));
    let slide_id = SlideId::new("scale-slide").expect("slide ID");
    let slide = HierarchyId::from(slide_id.clone());
    let cells = (0..row_count)
        .map(|index| CellId::new(format!("cell-{index:08}")).expect("cell ID"))
        .collect::<Vec<_>>();
    let patches = (0..row_count)
        .map(|index| PatchId::new(format!("patch-{index:08}")).expect("patch ID"))
        .collect::<Vec<_>>();
    let mut nodes = Vec::with_capacity(
        row_count
            .checked_mul(2)
            .and_then(|value| value.checked_add(2))
            .expect("node count"),
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
    nodes.extend(cells.iter().cloned().map(|cell| {
        HierarchyNode::new(
            HierarchyId::from(cell),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    nodes.extend(patches.iter().cloned().map(|patch| {
        HierarchyNode::new(
            HierarchyId::from(patch),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("hierarchy");
    let expected_cells =
        ExpectedCellSet::new("all_scale_cells.v1", cells.clone()).expect("expected cells");
    let expected_patches = ExpectedPatchSet::new(
        &hierarchy,
        slide_id.clone(),
        "all_scale_patches.v1",
        patches.clone(),
        DOMAIN_BUDGET,
    )
    .expect("expected patches");
    let source_width = row_count
        .checked_mul(224)
        .and_then(|value| u64::try_from(value).ok())
        .expect("source width");
    let context = context_with_source_width(&hierarchy, slide_id, source_width);
    let footprints = PatchFootprintSet::new(
        &hierarchy,
        &expected_patches,
        artifact(b"cell-patch-scale-expected-patches"),
        &context,
        artifact(b"cell-patch-scale-context"),
        patches
            .iter()
            .enumerate()
            .map(|(index, patch)| {
                PatchFootprint::new(
                    patch.clone(),
                    [i64::try_from(index * 224).expect("patch origin"), 0],
                )
            })
            .collect(),
        DOMAIN_BUDGET,
    )
    .expect("footprints");
    let bindings = CellPatchLinkBindings::new(
        artifact(b"cell-patch-scale-expected-cells"),
        artifact(b"cell-patch-scale-footprints"),
        artifact(b"cell-patch-scale-producer"),
        ContentDigest::from_bytes(b"cell-patch-scale-producer-content"),
        CoordinateFrameId::new("physical-image-pixels").expect("image frame ID"),
    );
    let declarations = cells
        .iter()
        .zip(&patches)
        .enumerate()
        .map(|(index, (cell, patch))| {
            DeclaredCellPatchAssignment::new(
                CellPatchAnchor::new(cell.clone(), [index as f64 * 224.0 + 10.0, 10.0])
                    .expect("anchor"),
                vec![CellPatchContributor::new(patch.clone(), 1, 1).expect("contributor")],
            )
        })
        .collect();
    let link = CellPatchLink::from_declared_weighted_interpolation(
        &hierarchy,
        &expected_cells,
        &expected_patches,
        &context,
        &footprints,
        &bindings,
        declarations,
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
    )
    .expect("cell-patch scale link");
    CellPatchFixture { link }
}

fn context(hierarchy: &CohortHierarchy, slide_id: SlideId) -> PatchEmbeddingContext {
    context_with_source_width(hierarchy, slide_id, 608)
}

fn context_with_source_width(
    hierarchy: &CohortHierarchy,
    slide_id: SlideId,
    source_width: u64,
) -> PatchEmbeddingContext {
    let image_id = CoordinateFrameId::new("physical-image-pixels").expect("image frame ID");
    let physical_id =
        CoordinateFrameId::new("physical-slide-micrometers").expect("physical frame ID");
    let transform_id = TransformId::new("physical-pixel-to-micrometer").expect("transform ID");
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
        TransformMatrix::affine_2d([0.5, 0.0, 0.0, 0.0, 0.5, 0.0]).expect("matrix"),
        None,
    );
    let registry = CoordinateRegistry::new(
        vec![image, physical],
        Vec::new(),
        vec![transform],
        Vec::new(),
    )
    .expect("coordinate registry");
    PatchEmbeddingContext::new(
        hierarchy,
        &registry,
        slide_id,
        image_id,
        physical_id,
        transform_id,
        PositiveRational::new(1, 2).expect("x scale"),
        PositiveRational::new(1, 2).expect("y scale"),
        [source_width, 224],
        [224, 224],
        [192, 192],
        [32, 32],
        EffectiveReceptiveField::FullInput,
        PatchBoundaryPolicy::FullyContainedOnly,
        DOMAIN_BUDGET,
    )
    .expect("patch context")
}
