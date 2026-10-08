use super::*;

const UNEQUAL_LINK_EDGE_COUNT: usize = 5;
const UNEQUAL_LINK_WORKING_BYTES: usize = UNEQUAL_LINK_EDGE_COUNT * size_of::<usize>();

struct NucleusAreaScalarFixture {
    project: MarklabProject,
    pattern: Pattern,
    cell_ids: Vec<CellId>,
    slide_id: SlideId,
    frame_id: CoordinateFrameId,
    binary_mark: BinaryMarkDeclaration,
    nucleus_area_mark: NucleusAreaUm2MarkDeclaration,
}

fn measurement_status_name(status: MeasurementStatus) -> &'static str {
    match status {
        MeasurementStatus::Measured => "measured",
        MeasurementStatus::ImportedPrediction => "imported_prediction",
        MeasurementStatus::MorphologyPrediction => "morphology_prediction",
        MeasurementStatus::DerivedSummary => "derived_summary",
    }
}

fn binary_mark_metadata() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("mark_id".into(), "mmr_loss".into()),
        ("mark_label".into(), "MMR loss".into()),
        (
            "measurement_status".into(),
            measurement_status_name(MeasurementStatus::Measured).into(),
        ),
        ("origin".into(), "independent".into()),
        ("unit".into(), "unitless".into()),
        ("value_kind".into(), "binary".into()),
    ])
}

fn nucleus_area_metadata() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("mark_id".into(), "nucleus_area_um2".into()),
        ("mark_label".into(), "Nucleus area".into()),
        (
            "measurement_status".into(),
            measurement_status_name(MeasurementStatus::Measured).into(),
        ),
        ("modality".into(), "morphology".into()),
        ("unit".into(), "square_micrometer".into()),
        ("value_kind".into(), "continuous".into()),
    ])
}

fn publish_scalar_record(
    embedding: &Fixture,
    label: &[u8],
    metadata: BTreeMap<String, String>,
) -> ArtifactRecord {
    let draft = ArtifactDraft::new(
        ArtifactRef::from_bytes("application/json", label).expect("scalar content"),
        ArtifactSchema::new("marklab.scalar_mark_provenance", 1).expect("scalar schema"),
        None,
        Vec::new(),
        metadata,
    )
    .expect("scalar artifact draft");
    embedding
        .store
        .publish_new_send(&draft, |writer| writer.write_all(label))
        .expect("publish scalar provenance")
        .into_record()
}

fn scalar_hierarchy(cell_ids: &[CellId], slide_id: &SlideId) -> CohortHierarchy {
    let patient = HierarchyId::from(
        PatientId::new(format!("{}-patient", slide_id.as_str())).expect("patient ID"),
    );
    let slide = HierarchyId::from(slide_id.clone());
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
    nodes.extend(cell_ids.iter().cloned().map(|cell_id| {
        HierarchyNode::new(
            HierarchyId::from(cell_id),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    CohortHierarchy::new(nodes, Vec::new()).expect("scalar hierarchy")
}

fn nucleus_area_scalar_fixture(
    embedding: &Fixture,
    slide_id: SlideId,
    marks: [u8; 3],
    areas: [f32; 3],
) -> NucleusAreaScalarFixture {
    let cell_ids = embedding.expected.cells().to_vec();
    let frame_id = CoordinateFrameId::new("nucleus-area-physical-xy").expect("frame ID");
    let hierarchy = scalar_hierarchy(&cell_ids, &slide_id);
    let registry = CoordinateRegistry::new(
        vec![CoordinateFrame::new(
            frame_id.clone(),
            vec![SpatialAxis::X, SpatialAxis::Y],
            CoordinateUnit::Micrometer,
            CoordinateSpace::Physical,
        )
        .expect("physical frame")],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("scalar coordinate registry");
    let mut project = MarklabProject::new();
    project
        .install_hierarchy(hierarchy)
        .expect("install scalar hierarchy");
    project
        .install_coordinate_registry(registry)
        .expect("install scalar registry");

    let binary_record = publish_scalar_record(
        embedding,
        format!("nucleus-area-binary-{}", slide_id.as_str()).as_bytes(),
        binary_mark_metadata(),
    );
    let nucleus_record = publish_scalar_record(
        embedding,
        format!("nucleus-area-{}", slide_id.as_str()).as_bytes(),
        nucleus_area_metadata(),
    );
    let binary_artifact_id = binary_record.id();
    let nucleus_artifact_id = nucleus_record.id();
    project
        .register_artifact(binary_record)
        .expect("register binary provenance");
    project
        .register_artifact(nucleus_record)
        .expect("register nucleus provenance");

    let binary_mark = BinaryMarkDeclaration::independent(
        ScalarMarkId::new("mmr_loss").expect("binary mark ID"),
        "MMR loss",
        MeasurementStatus::Measured,
        binary_artifact_id,
    )
    .expect("binary declaration");
    let nucleus_area_mark =
        NucleusAreaUm2MarkDeclaration::new(MeasurementStatus::Measured, nucleus_artifact_id)
            .expect("nucleus-area declaration");
    let mut pattern = Pattern::from_arrays(
        vec![0.0, 1.0, 2.0],
        vec![0.0, 0.0, 0.0],
        marks.to_vec(),
        PatternMeta {
            case_id: "synthetic-nucleus-area-case".into(),
            timepoint: "post".into(),
            protein: "MSH6".into(),
            slide_id: Some(slide_id.as_str().to_owned()),
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("scalar pattern");
    pattern.nucleus_area_um2 = Some(areas.to_vec().into_boxed_slice());
    NucleusAreaScalarFixture {
        project,
        pattern,
        cell_ids,
        slide_id,
        frame_id,
        binary_mark,
        nucleus_area_mark,
    }
}

fn nucleus_area_input(fixture: &NucleusAreaScalarFixture) -> DeclaredScalarPatternInput<'_> {
    DeclaredScalarPatternInput::new(
        &fixture.project,
        &fixture.pattern,
        &fixture.cell_ids,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
        fixture.binary_mark.clone(),
        None,
        16 * 1024,
        fixture
            .cell_ids
            .iter()
            .map(|cell_id| cell_id.as_str().len())
            .sum(),
    )
    .expect("declared scalar input")
}

fn run_nucleus_area(
    scalar: &NucleusAreaScalarFixture,
    input: &DeclaredScalarPatternInput<'_>,
    links: &LinkFixture,
    maximum_rows: usize,
    maximum_assignments: usize,
    maximum_edges: usize,
    maximum_working_bytes: usize,
) -> Result<
    marklab::ContainedPatchBinaryNucleusAreaContrast,
    ContainedPatchBinaryNucleusAreaContrastError,
> {
    contained_patch_binary_nucleus_area_contrast(
        &scalar.project,
        input,
        scalar.nucleus_area_mark.clone(),
        &links.link,
        links.graph(),
        links.receipt(),
        maximum_rows,
        maximum_assignments,
        maximum_edges,
        maximum_working_bytes,
    )
}

fn run_nucleus_area_exact(
    scalar: &NucleusAreaScalarFixture,
    input: &DeclaredScalarPatternInput<'_>,
    links: &LinkFixture,
) -> marklab::ContainedPatchBinaryNucleusAreaContrast {
    let edge_count = links.link.edge_count();
    run_nucleus_area(
        scalar,
        input,
        links,
        3,
        links.link.assignment_count(),
        edge_count,
        edge_count * size_of::<usize>(),
    )
    .expect("contained-patch nucleus-area contrast")
}

#[path = "nucleus_area/behavior.rs"]
mod behavior;
