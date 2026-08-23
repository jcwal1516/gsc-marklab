pub(super) use std::{io::Write, mem::size_of, str::FromStr};

pub(super) use marklab::{
    ArtifactId, CellId, CellPatchAnchor, CellPatchAssignment, CellPatchAssignmentMode,
    CellPatchAssignmentStatus, CellPatchContributor, CellPatchEdge, CellPatchLink,
    CellPatchLinkBindings, CohortHierarchy, ContentDigest, CoordinateFrame, CoordinateFrameId,
    CoordinateRegistry, CoordinateSpace, CoordinateUnit, DeclaredCellPatchAssignment,
    EffectiveReceptiveField, ExpectedCellSet, ExpectedPatchSet, FrameTransform, HierarchyId,
    HierarchyNode, ImageCoordinateConvention, MultiscaleEmbeddingError, PatchBoundaryPolicy,
    PatchEmbeddingContext, PatchFootprint, PatchFootprintSet, PatchId, PatientId, PositiveRational,
    ReplicationRole, SlideId, SpatialAxis, TransformId, TransformMatrix,
};

pub(super) const BUDGET: usize = 1 << 20;

pub(super) fn artifact(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

pub(super) fn cell(value: &str) -> CellId {
    CellId::new(value).expect("cell ID")
}

pub(super) fn patch(value: &str) -> PatchId {
    PatchId::new(value).expect("patch ID")
}

pub(super) struct LinkFixture {
    pub(super) hierarchy: CohortHierarchy,
    pub(super) expected_cells: ExpectedCellSet,
    pub(super) expected_patches: ExpectedPatchSet,
    pub(super) context: PatchEmbeddingContext,
    pub(super) footprints: PatchFootprintSet,
    pub(super) bindings: CellPatchLinkBindings,
}

pub(super) fn fixture() -> LinkFixture {
    fixture_with_footprints(
        PatchBoundaryPolicy::FullyContainedOnly,
        [[0, 0], [50, 0], [300, 0]],
    )
}

pub(super) fn fixture_with_footprints(
    boundary_policy: PatchBoundaryPolicy,
    origins: [[i64; 2]; 3],
) -> LinkFixture {
    let patient = HierarchyId::from(PatientId::new("link-patient").expect("patient"));
    let slide = SlideId::new("link-slide").expect("slide");
    let slide_node = HierarchyId::from(slide.clone());
    let cell_ids = ["cell-a", "cell-b", "cell-c", "cell-d"].map(cell);
    let patch_ids = ["patch-a", "patch-b", "patch-c"].map(patch);
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
    nodes.extend(cell_ids.iter().cloned().map(|id| {
        HierarchyNode::new(
            HierarchyId::from(id),
            Some(slide_node.clone()),
            ReplicationRole::Structural,
        )
    }));
    nodes.extend(patch_ids.iter().cloned().map(|id| {
        HierarchyNode::new(
            HierarchyId::from(id),
            Some(slide_node.clone()),
            ReplicationRole::Structural,
        )
    }));
    let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("link hierarchy");
    let expected_cells =
        ExpectedCellSet::new("all_cells.v1", cell_ids.into()).expect("expected cells");
    let expected_patches = ExpectedPatchSet::new(
        &hierarchy,
        slide.clone(),
        "all_patches.v1",
        patch_ids.into(),
        BUDGET,
    )
    .expect("expected patches");
    let image_frame_id = CoordinateFrameId::new("link-image-pixels").expect("image frame");
    let physical_frame_id =
        CoordinateFrameId::new("link-slide-micrometers").expect("physical frame");
    let transform_id = TransformId::new("link-pixel-to-micrometer").expect("transform");
    let image = CoordinateFrame::new(
        image_frame_id.clone(),
        vec![SpatialAxis::X, SpatialAxis::Y],
        CoordinateUnit::Pixel,
        CoordinateSpace::Image(ImageCoordinateConvention::PixelCenterAtInteger),
    )
    .expect("image frame");
    let physical = CoordinateFrame::new(
        physical_frame_id.clone(),
        vec![SpatialAxis::X, SpatialAxis::Y],
        CoordinateUnit::Micrometer,
        CoordinateSpace::Physical,
    )
    .expect("physical frame");
    let transform = FrameTransform::new(
        transform_id.clone(),
        image_frame_id.clone(),
        physical_frame_id.clone(),
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
        image_frame_id.clone(),
        physical_frame_id,
        transform_id,
        PositiveRational::new(1, 2).expect("x scale"),
        PositiveRational::new(1, 2).expect("y scale"),
        [1_024, 768],
        [224, 224],
        [192, 192],
        [32, 32],
        EffectiveReceptiveField::FullInput,
        boundary_policy,
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
            PatchFootprint::new(patch("patch-a"), origins[0]),
            PatchFootprint::new(patch("patch-b"), origins[1]),
            PatchFootprint::new(patch("patch-c"), origins[2]),
        ],
        BUDGET,
    )
    .expect("footprints");
    let bindings = CellPatchLinkBindings::new(
        artifact(b"expected-cells"),
        artifact(b"patch-footprints"),
        artifact(b"cell-patch-producer"),
        ContentDigest::from_bytes(b"cell-patch-producer-content"),
        image_frame_id.clone(),
    );
    LinkFixture {
        hierarchy,
        expected_cells,
        expected_patches,
        context,
        footprints,
        bindings,
    }
}

pub(super) fn anchor(cell_id: &str, xy: [f64; 2]) -> CellPatchAnchor {
    CellPatchAnchor::new(cell(cell_id), xy).expect("finite anchor")
}

fn frame(writer: &mut impl Write, value: &[u8]) {
    writer
        .write_all(&(value.len() as u128).to_be_bytes())
        .expect("reference length frame");
    writer.write_all(value).expect("reference value frame");
}

pub(super) fn reference_link_digest(link: &CellPatchLink) -> ContentDigest {
    let mut writer = ContentDigest::builder();
    frame(&mut writer, b"marklab-cell-patch-link-logical-v1");
    frame(
        &mut writer,
        match link.mode() {
            CellPatchAssignmentMode::ContainedShared => b"contained_shared",
            CellPatchAssignmentMode::DeclaredWeightedInterpolation => {
                b"declared_weighted_interpolation"
            }
        },
    );
    frame(
        &mut writer,
        link.expected_cells_artifact_id().digest().as_bytes(),
    );
    frame(&mut writer, link.expected_cells_logical_digest().as_bytes());
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
    frame(&mut writer, link.producer_artifact_id().digest().as_bytes());
    frame(&mut writer, link.producer_content_digest().as_bytes());
    frame(
        &mut writer,
        &u64::try_from(link.assignment_count())
            .expect("bounded assignments")
            .to_be_bytes(),
    );
    for (row, assignment) in link.assignments().iter().enumerate() {
        frame(&mut writer, assignment.cell_id().as_str().as_bytes());
        frame(
            &mut writer,
            &assignment.anchor_px()[0].to_bits().to_be_bytes(),
        );
        frame(
            &mut writer,
            &assignment.anchor_px()[1].to_bits().to_be_bytes(),
        );
        frame(
            &mut writer,
            match assignment.status() {
                CellPatchAssignmentStatus::Assigned => b"assigned",
                CellPatchAssignmentStatus::OutsideSampledSupport => b"outside_sampled_support",
                CellPatchAssignmentStatus::InterpolationUnavailable => b"interpolation_unavailable",
            },
        );
        frame(&mut writer, &assignment.edge_count().to_be_bytes());
        for edge in link
            .edges_for_assignment(row)
            .expect("canonical edge range")
        {
            frame(&mut writer, edge.patch_id().as_str().as_bytes());
            if let Some(weight) = edge.weight() {
                frame(&mut writer, b"weight_present");
                frame(&mut writer, &weight.numerator().to_be_bytes());
                frame(&mut writer, &weight.denominator().to_be_bytes());
            } else {
                frame(&mut writer, b"weight_absent");
            }
        }
    }
    writer.finish().0
}
