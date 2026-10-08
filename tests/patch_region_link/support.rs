pub(super) use std::{io::Write, mem::size_of, str::FromStr};

pub(super) use marklab::{
    ArtifactId, CohortHierarchy, ContentDigest, CoordinateFrame, CoordinateFrameId,
    CoordinateRegistry, CoordinateSpace, CoordinateUnit, EffectiveReceptiveField, ExpectedPatchSet,
    ExpectedRegionSet, FrameTransform, HierarchyId, HierarchyNode, ImageCoordinateConvention,
    MultiscaleEmbeddingError, PatchBoundaryPolicy, PatchEmbeddingContext, PatchFootprint,
    PatchFootprintSet, PatchId, PatchRegionAssessment, PatchRegionAssessmentBindings,
    PatchRegionDeclaration, PatchRegionLink, PatchRegionRelation, PatientId, PositiveRational,
    RegionId, ReplicationRole, SlideId, SpatialAxis, TransformId, TransformMatrix,
};

pub(super) const BUDGET: usize = 1 << 20;

pub(super) fn artifact(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

pub(super) fn patch(value: &str) -> PatchId {
    PatchId::new(value).expect("patch ID")
}

pub(super) fn region(value: &str) -> RegionId {
    RegionId::new(value).expect("region ID")
}

pub(super) struct Fixture {
    pub(super) hierarchy: CohortHierarchy,
    pub(super) expected_patches: ExpectedPatchSet,
    pub(super) expected_regions: ExpectedRegionSet,
    pub(super) context: PatchEmbeddingContext,
    pub(super) footprints: PatchFootprintSet,
    pub(super) bindings: PatchRegionAssessmentBindings,
}

pub(super) fn fixture() -> Fixture {
    let patient = HierarchyId::from(PatientId::new("region-link-patient").expect("patient"));
    let slide = SlideId::new("region-link-slide").expect("slide");
    let slide_node = HierarchyId::from(slide.clone());
    let patch_ids = ["patch-a", "patch-b"].map(patch);
    let region_ids = ["region-a", "region-b"].map(region);
    let mut nodes = vec![
        HierarchyNode::new(patient.clone(), None, ReplicationRole::BiologicalUnit),
        HierarchyNode::new(
            slide_node.clone(),
            None,
            ReplicationRole::TechnicalReplicate {
                biological_source: patient,
            },
        ),
    ];
    nodes.extend(patch_ids.iter().cloned().map(|id| {
        HierarchyNode::new(
            HierarchyId::from(id),
            Some(slide_node.clone()),
            ReplicationRole::Structural,
        )
    }));
    nodes.extend(region_ids.iter().cloned().map(|id| {
        HierarchyNode::new(
            HierarchyId::from(id),
            Some(slide_node.clone()),
            ReplicationRole::Structural,
        )
    }));
    let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("link hierarchy");
    let expected_patches = ExpectedPatchSet::new(
        &hierarchy,
        slide.clone(),
        "all_patches.v1",
        patch_ids.into(),
        BUDGET,
    )
    .expect("expected patches");
    let expected_regions = ExpectedRegionSet::new(
        &hierarchy,
        slide.clone(),
        "all_regions.v1",
        region_ids.into(),
        BUDGET,
    )
    .expect("expected regions");
    let image_id = CoordinateFrameId::new("region-link-image").expect("image frame");
    let physical_id = CoordinateFrameId::new("region-link-physical").expect("physical frame");
    let transform_id = TransformId::new("region-link-transform").expect("transform");
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
    let context = PatchEmbeddingContext::new(
        &hierarchy,
        &registry,
        slide,
        image_id,
        physical_id,
        transform_id,
        PositiveRational::new(1, 2).expect("x scale"),
        PositiveRational::new(1, 2).expect("y scale"),
        [1_024, 768],
        [224, 224],
        [192, 192],
        [32, 32],
        EffectiveReceptiveField::FullInput,
        PatchBoundaryPolicy::FullyContainedOnly,
        BUDGET,
    )
    .expect("context");
    let footprints = PatchFootprintSet::new(
        &hierarchy,
        &expected_patches,
        artifact(b"expected-patches"),
        &context,
        artifact(b"patch-context"),
        vec![
            PatchFootprint::new(patch("patch-a"), [0, 0]),
            PatchFootprint::new(patch("patch-b"), [200, 0]),
        ],
        BUDGET,
    )
    .expect("footprints");
    let bindings = PatchRegionAssessmentBindings::new(
        artifact(b"expected-regions"),
        artifact(b"patch-footprints"),
        artifact(b"patch-region-converter"),
        ContentDigest::from_bytes(b"patch-region-converter-content"),
    );
    Fixture {
        hierarchy,
        expected_patches,
        expected_regions,
        context,
        footprints,
        bindings,
    }
}

pub(super) fn declarations() -> Vec<PatchRegionDeclaration> {
    vec![
        PatchRegionDeclaration::fully_contained(patch("patch-a"), region("region-a")),
        PatchRegionDeclaration::partial_overlap(patch("patch-a"), region("region-b"), 1, 4)
            .expect("quarter overlap"),
        PatchRegionDeclaration::partial_overlap(patch("patch-b"), region("region-a"), 1, 2)
            .expect("half overlap"),
    ]
}

fn frame(writer: &mut impl Write, value: &[u8]) {
    writer
        .write_all(&(value.len() as u128).to_be_bytes())
        .expect("reference length frame");
    writer.write_all(value).expect("reference value frame");
}

pub(super) fn reference_relations_digest(assessment: &PatchRegionAssessment) -> ContentDigest {
    let mut writer = ContentDigest::builder();
    frame(&mut writer, b"marklab-patch-region-assessment-relations-v1");
    frame(
        &mut writer,
        &u64::try_from(assessment.nonzero_relation_count())
            .expect("bounded rows")
            .to_be_bytes(),
    );
    for row in assessment.nonzero_relations() {
        frame(&mut writer, row.patch_id().as_str().as_bytes());
        frame(&mut writer, row.region_id().as_str().as_bytes());
        frame(
            &mut writer,
            match row.relation() {
                PatchRegionRelation::FullyContained => b"fully_contained",
                PatchRegionRelation::PartialOverlap => b"partial_overlap",
            },
        );
        frame(&mut writer, &row.numerator().to_be_bytes());
        frame(&mut writer, &row.denominator().to_be_bytes());
    }
    writer.finish().0
}

pub(super) fn reference_link_digest(link: &PatchRegionLink) -> ContentDigest {
    let mut writer = ContentDigest::builder();
    frame(&mut writer, b"marklab-patch-region-link-logical-v1");
    frame(
        &mut writer,
        link.expected_patches_artifact_id().digest().as_bytes(),
    );
    frame(
        &mut writer,
        link.expected_patches_logical_digest().as_bytes(),
    );
    frame(
        &mut writer,
        link.expected_regions_artifact_id().digest().as_bytes(),
    );
    frame(
        &mut writer,
        link.expected_regions_logical_digest().as_bytes(),
    );
    frame(
        &mut writer,
        link.patch_context_artifact_id().digest().as_bytes(),
    );
    frame(&mut writer, link.patch_context_logical_digest().as_bytes());
    frame(
        &mut writer,
        link.patch_footprints_artifact_id().digest().as_bytes(),
    );
    frame(
        &mut writer,
        link.patch_footprints_logical_digest().as_bytes(),
    );
    frame(
        &mut writer,
        link.converter_artifact_id().digest().as_bytes(),
    );
    frame(&mut writer, link.converter_content_digest().as_bytes());
    frame(
        &mut writer,
        link.assessment_artifact_id().digest().as_bytes(),
    );
    frame(&mut writer, link.assessment_content_digest().as_bytes());
    frame(&mut writer, b"expected_cartesian_exhaustive");
    frame(&mut writer, &link.assessed_pair_count().to_be_bytes());
    frame(
        &mut writer,
        &u64::try_from(link.nonzero_relation_count())
            .expect("bounded rows")
            .to_be_bytes(),
    );
    for row in link.nonzero_relations() {
        frame(&mut writer, row.patch_id().as_str().as_bytes());
        frame(&mut writer, row.region_id().as_str().as_bytes());
        frame(
            &mut writer,
            match row.relation() {
                PatchRegionRelation::FullyContained => b"fully_contained",
                PatchRegionRelation::PartialOverlap => b"partial_overlap",
            },
        );
        frame(&mut writer, &row.numerator().to_be_bytes());
        frame(&mut writer, &row.denominator().to_be_bytes());
    }
    writer.finish().0
}
