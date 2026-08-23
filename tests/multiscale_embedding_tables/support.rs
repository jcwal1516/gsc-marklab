pub(super) use std::{mem::size_of, str::FromStr};

pub(super) use marklab::{
    ArtifactId, CohortHierarchy, ContentDigest, CoordinateFrame, CoordinateFrameId,
    CoordinateRegistry, CoordinateSpace, CoordinateUnit, EffectiveReceptiveField,
    EmbeddingEntityKind, EmbeddingStatus, ExpectedPatchSet, ExpectedRegionSet, ExpectedSlideSet,
    FrameTransform, HierarchyId, HierarchyNode, ImageCoordinateConvention,
    MultiscaleEmbeddingError, PatchBoundaryPolicy, PatchEmbeddingContext, PatchEmbeddingRow,
    PatchEmbeddingTable, PatchFootprint, PatchFootprintSet, PatchId, PatientId, PositiveRational,
    RegionEmbeddingRow, RegionEmbeddingTable, RegionId, ReplicationRole, SlideEmbeddingRow,
    SlideEmbeddingTable, SlideId, SpatialAxis, TransformId, TransformMatrix,
};

pub(super) const RETAINED_BUDGET: usize = 64 * 1024;

pub(super) fn artifact_id(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

pub(super) fn patch(value: &str) -> PatchId {
    PatchId::new(value).expect("patch ID")
}

pub(super) fn region(value: &str) -> RegionId {
    RegionId::new(value).expect("region ID")
}

pub(super) fn hierarchy() -> (CohortHierarchy, SlideId) {
    let patient = HierarchyId::from(PatientId::new("patient").expect("patient ID"));
    let slide_id = SlideId::new("slide").expect("slide ID");
    let slide = HierarchyId::from(slide_id.clone());
    let nodes = vec![
        HierarchyNode::new(patient.clone(), None, ReplicationRole::BiologicalUnit),
        HierarchyNode::new(
            slide.clone(),
            None,
            ReplicationRole::TechnicalReplicate {
                biological_source: patient,
            },
        ),
        HierarchyNode::new(
            HierarchyId::from(region("region-a")),
            Some(slide.clone()),
            ReplicationRole::Structural,
        ),
        HierarchyNode::new(
            HierarchyId::from(patch("patch-a")),
            Some(slide.clone()),
            ReplicationRole::Structural,
        ),
        HierarchyNode::new(
            HierarchyId::from(patch("patch-b")),
            Some(slide.clone()),
            ReplicationRole::Structural,
        ),
        HierarchyNode::new(
            HierarchyId::from(patch("patch-c")),
            Some(slide.clone()),
            ReplicationRole::Structural,
        ),
        HierarchyNode::new(
            HierarchyId::from(patch("patch-d")),
            Some(slide),
            ReplicationRole::Structural,
        ),
    ];
    (
        CohortHierarchy::new(nodes, Vec::new()).expect("valid multiscale hierarchy"),
        slide_id,
    )
}

pub(super) fn coordinate_registry() -> (
    CoordinateRegistry,
    CoordinateFrameId,
    CoordinateFrameId,
    TransformId,
) {
    coordinate_registry_with_matrix([0.5, 0.0, 1.25, 0.0, 0.25, -2.0])
}

pub(super) fn coordinate_registry_with_matrix(
    matrix: [f64; 6],
) -> (
    CoordinateRegistry,
    CoordinateFrameId,
    CoordinateFrameId,
    TransformId,
) {
    let image_id = CoordinateFrameId::new("image-pixels").expect("image frame ID");
    let physical_id = CoordinateFrameId::new("slide-micrometers").expect("physical frame ID");
    let transform_id = TransformId::new("pixel-to-micrometer").expect("transform ID");
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
        TransformMatrix::affine_2d(matrix).expect("affine matrix"),
        None,
    );
    (
        CoordinateRegistry::new(
            vec![image, physical],
            Vec::new(),
            vec![transform],
            Vec::new(),
        )
        .expect("coordinate registry"),
        image_id,
        physical_id,
        transform_id,
    )
}

pub(super) fn expected_patches(hierarchy: &CohortHierarchy, slide: &SlideId) -> ExpectedPatchSet {
    ExpectedPatchSet::new(
        hierarchy,
        slide.clone(),
        "all_patches.v1",
        vec![patch("patch-a"), patch("patch-b")],
        RETAINED_BUDGET,
    )
    .expect("expected patches")
}

pub(super) fn patch_context_with_budget(
    hierarchy: &CohortHierarchy,
    slide: &SlideId,
    registry: &CoordinateRegistry,
    image: CoordinateFrameId,
    physical: CoordinateFrameId,
    transform: TransformId,
    maximum_retained_bytes: usize,
) -> Result<PatchEmbeddingContext, MultiscaleEmbeddingError> {
    PatchEmbeddingContext::new(
        hierarchy,
        registry,
        slide.clone(),
        image,
        physical,
        transform,
        PositiveRational::new(1, 2).expect("x scale"),
        PositiveRational::new(1, 4).expect("y scale"),
        [1_024, 768],
        [224, 224],
        [192, 192],
        [32, 32],
        EffectiveReceptiveField::FullInput,
        PatchBoundaryPolicy::FullyContainedOnly,
        maximum_retained_bytes,
    )
}
