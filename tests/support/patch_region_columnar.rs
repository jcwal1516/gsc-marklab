use std::str::FromStr;

use marklab::{
    ArtifactId, CohortHierarchy, ContentDigest, CoordinateFrame, CoordinateFrameId,
    CoordinateRegistry, CoordinateSpace, CoordinateUnit, EffectiveReceptiveField, ExpectedPatchSet,
    ExpectedRegionSet, FrameTransform, HierarchyId, HierarchyNode, ImageCoordinateConvention,
    PatchBoundaryPolicy, PatchEmbeddingContext, PatchFootprint, PatchFootprintSet, PatchId,
    PatchRegionAssessment, PatchRegionAssessmentBindings, PatchRegionDeclaration, PatchRegionLink,
    PatientId, PositiveRational, RegionId, ReplicationRole, SlideId, SpatialAxis, TransformId,
    TransformMatrix,
};

const DOMAIN_BUDGET: usize = 512 * 1024 * 1024;

fn artifact(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

pub(crate) fn patch_region_scale_link(row_count: usize) -> PatchRegionLink {
    let patient = HierarchyId::from(PatientId::new("scale-patient").expect("patient ID"));
    let slide_id = SlideId::new("scale-slide").expect("slide ID");
    let slide = HierarchyId::from(slide_id.clone());
    let patches = (0..row_count)
        .map(|index| PatchId::new(format!("patch-{index:08}")).expect("patch ID"))
        .collect::<Vec<_>>();
    let region = RegionId::new("region-00000000").expect("region ID");
    let mut nodes = Vec::with_capacity(row_count.checked_add(3).expect("node count"));
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
    nodes.extend(patches.iter().cloned().map(|patch| {
        HierarchyNode::new(
            HierarchyId::from(patch),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    nodes.push(HierarchyNode::new(
        HierarchyId::from(region.clone()),
        Some(slide.clone()),
        ReplicationRole::Structural,
    ));
    let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("hierarchy");
    let expected_patches = ExpectedPatchSet::new(
        &hierarchy,
        slide_id.clone(),
        "all_scale_patches.v1",
        patches.clone(),
        DOMAIN_BUDGET,
    )
    .expect("expected patches");
    let expected_regions = ExpectedRegionSet::new(
        &hierarchy,
        slide_id.clone(),
        "all_scale_regions.v1",
        vec![region.clone()],
        DOMAIN_BUDGET,
    )
    .expect("expected regions");
    let source_width = row_count
        .checked_mul(224)
        .and_then(|value| u64::try_from(value).ok())
        .expect("source width");
    let context = context(&hierarchy, slide_id, source_width.max(224));
    let footprints = PatchFootprintSet::new(
        &hierarchy,
        &expected_patches,
        artifact(b"scale-expected-patches"),
        &context,
        artifact(b"scale-patch-context"),
        patches
            .iter()
            .enumerate()
            .map(|(index, patch)| {
                PatchFootprint::new(
                    patch.clone(),
                    [
                        i64::try_from(index.checked_mul(224).expect("origin")).expect("origin"),
                        0,
                    ],
                )
            })
            .collect(),
        DOMAIN_BUDGET,
    )
    .expect("footprints");
    let bindings = PatchRegionAssessmentBindings::new(
        artifact(b"scale-expected-regions"),
        artifact(b"scale-patch-footprints"),
        artifact(b"scale-patch-region-converter"),
        ContentDigest::from_bytes(b"scale-patch-region-converter-content"),
    );
    let assessment = PatchRegionAssessment::new(
        &expected_patches,
        &expected_regions,
        &context,
        &footprints,
        &bindings,
        patches
            .iter()
            .cloned()
            .map(|patch| PatchRegionDeclaration::fully_contained(patch, region.clone()))
            .collect(),
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
    )
    .expect("assessment");
    let assessment_bytes = assessment.to_canonical_json().expect("assessment bytes");
    PatchRegionLink::from_exhaustive_assessment(
        &assessment,
        artifact(b"scale-patch-region-assessment"),
        ContentDigest::from_bytes(&assessment_bytes),
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
    )
    .expect("patch-region link")
}

fn context(
    hierarchy: &CohortHierarchy,
    slide_id: SlideId,
    source_width: u64,
) -> PatchEmbeddingContext {
    let image_id = CoordinateFrameId::new("scale-image-pixels").expect("image frame ID");
    let physical_id = CoordinateFrameId::new("scale-slide-micrometers").expect("physical frame ID");
    let transform_id = TransformId::new("scale-pixel-to-micrometer").expect("transform ID");
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
    .expect("registry");
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
    .expect("context")
}
