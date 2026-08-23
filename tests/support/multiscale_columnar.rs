use std::{io, io::Write, str::FromStr};

use marklab::{
    ArtifactId, CohortHierarchy, ContentDigest, CoordinateFrame, CoordinateFrameId,
    CoordinateRegistry, CoordinateSpace, CoordinateUnit, EffectiveReceptiveField, ExpectedPatchSet,
    FrameTransform, HierarchyId, HierarchyNode, ImageCoordinateConvention, PatchBoundaryPolicy,
    PatchEmbeddingContext, PatchFootprint, PatchFootprintSet, PatchId, PatchOverlapGraph,
    PatientId, PositiveRational, ReplicationRole, SlideId, SpatialAxis, TransformId,
    TransformMatrix,
};

const DOMAIN_BUDGET: usize = 256 * 1024 * 1024;

#[derive(Default)]
pub(crate) struct FragmentingWriter {
    pub(crate) bytes: Vec<u8>,
    calls: usize,
}

impl Write for FragmentingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        const FRAGMENTS: [usize; 7] = [1, 3, 2, 7, 5, 11, 4];
        let maximum = FRAGMENTS[self.calls % FRAGMENTS.len()];
        self.calls += 1;
        let written = buffer.len().min(maximum);
        self.bytes.extend_from_slice(&buffer[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) struct SpatialFixture {
    pub(crate) expected: ExpectedPatchSet,
    pub(crate) expected_artifact_id: ArtifactId,
    pub(crate) context: PatchEmbeddingContext,
    pub(crate) context_artifact_id: ArtifactId,
    pub(crate) footprints: PatchFootprintSet,
    pub(crate) overlap: PatchOverlapGraph,
}

pub(crate) fn artifact(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

pub(crate) fn fixture(row_count: usize) -> SpatialFixture {
    let origins = (0..row_count)
        .map(|index| i64::try_from(index).expect("index") * 192)
        .collect();
    fixture_with_origins(origins, PatchBoundaryPolicy::FullyContainedOnly)
}

pub(crate) fn reflecting_fixture() -> SpatialFixture {
    fixture_with_origins(vec![-32, 160], PatchBoundaryPolicy::Reflect)
}

fn fixture_with_origins(origins: Vec<i64>, boundary_policy: PatchBoundaryPolicy) -> SpatialFixture {
    let row_count = origins.len();
    let patient = HierarchyId::from(PatientId::new("physical-patient").expect("patient ID"));
    let slide_id = SlideId::new("physical-slide").expect("slide ID");
    let slide = HierarchyId::from(slide_id.clone());
    let patch_ids = (0..row_count)
        .map(|index| PatchId::new(format!("patch-{index:08}")).expect("patch ID"))
        .collect::<Vec<_>>();
    let mut hierarchy_nodes = vec![
        HierarchyNode::new(patient.clone(), None, ReplicationRole::BiologicalUnit),
        HierarchyNode::new(
            slide.clone(),
            None,
            ReplicationRole::TechnicalReplicate {
                biological_source: patient,
            },
        ),
    ];
    hierarchy_nodes.extend(patch_ids.iter().cloned().map(|patch_id| {
        HierarchyNode::new(
            HierarchyId::from(patch_id),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    let hierarchy = CohortHierarchy::new(hierarchy_nodes, Vec::new()).expect("hierarchy");

    let expected_artifact_id = artifact(b"physical-expected-patches");
    let expected = ExpectedPatchSet::new(
        &hierarchy,
        slide_id.clone(),
        "all_patches.v1",
        patch_ids.clone(),
        DOMAIN_BUDGET,
    )
    .expect("expected patches");
    let context_artifact_id = artifact(b"physical-patch-context");
    let context = context(&hierarchy, slide_id, &origins, boundary_policy);
    let footprint_rows = patch_ids
        .into_iter()
        .zip(origins)
        .map(|(patch_id, origin_x)| PatchFootprint::new(patch_id, [origin_x, 0]))
        .collect();
    let footprints = PatchFootprintSet::new(
        &hierarchy,
        &expected,
        expected_artifact_id,
        &context,
        context_artifact_id,
        footprint_rows,
        DOMAIN_BUDGET,
    )
    .expect("footprints");
    let footprint_artifact_id = artifact(b"physical-patch-footprints");
    let overlap = PatchOverlapGraph::derive(
        &expected,
        &context,
        &footprints,
        footprint_artifact_id,
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
    )
    .expect("overlap graph");
    SpatialFixture {
        expected,
        expected_artifact_id,
        context,
        context_artifact_id,
        footprints,
        overlap,
    }
}

fn context(
    hierarchy: &CohortHierarchy,
    slide_id: SlideId,
    origins: &[i64],
    boundary_policy: PatchBoundaryPolicy,
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
    let source_width = origins
        .iter()
        .try_fold(224_i64, |maximum, origin| {
            origin
                .checked_add(224)
                .map(|end| maximum.max(end))
                .ok_or("source width overflow")
        })
        .and_then(|width| u64::try_from(width).map_err(|_| "negative source width"))
        .expect("source image width");
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
        boundary_policy,
        DOMAIN_BUDGET,
    )
    .expect("patch context")
}
