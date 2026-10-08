use super::support::*;

fn assessment() -> PatchRegionAssessment {
    let fixture = fixture();
    PatchRegionAssessment::new(
        &fixture.expected_patches,
        &fixture.expected_regions,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        declarations(),
        BUDGET,
        BUDGET,
    )
    .expect("assessment")
}

fn retained_assessment_bytes(assessment: &PatchRegionAssessment) -> usize {
    let assessment_retained = size_of::<PatchRegionAssessment>()
        + assessment.owning_slide_id().as_str().len()
        + assessment.nonzero_relation_count() * size_of::<PatchRegionDeclaration>()
        + assessment
            .nonzero_relations()
            .iter()
            .map(|row| row.patch_id().as_str().len() + row.region_id().as_str().len())
            .sum::<usize>();
    assessment_retained
}

fn input_bytes(rows: &Vec<PatchRegionDeclaration>) -> usize {
    rows.capacity() * size_of::<PatchRegionDeclaration>()
        + rows
            .iter()
            .map(|row| row.patch_id().as_str().len() + row.region_id().as_str().len())
            .sum::<usize>()
}

fn retained_link_bytes(link: &PatchRegionLink) -> usize {
    size_of::<PatchRegionLink>()
        + link.owning_slide_id().as_str().len()
        + link.nonzero_relation_count() * size_of::<PatchRegionDeclaration>()
        + link
            .nonzero_relations()
            .iter()
            .map(|row| row.patch_id().as_str().len() + row.region_id().as_str().len())
            .sum::<usize>()
}

#[test]
fn assessment_enforces_exact_retained_and_peak_working_boundaries() {
    let fixture = fixture();
    let declarations = declarations();
    let assessment_retained = retained_assessment_bytes(&assessment());
    let assessment_input = input_bytes(&declarations);
    let assessment_working = assessment_input + assessment_retained;
    PatchRegionAssessment::new(
        &fixture.expected_patches,
        &fixture.expected_regions,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        declarations.clone(),
        assessment_retained,
        assessment_working,
    )
    .expect("exact assessment budgets");
    assert!(matches!(
        PatchRegionAssessment::new(
            &fixture.expected_patches,
            &fixture.expected_regions,
            &fixture.context,
            &fixture.footprints,
            &fixture.bindings,
            declarations.clone(),
            assessment_retained - 1,
            assessment_working,
        ),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, maximum })
            if required == assessment_retained && maximum == assessment_retained - 1
    ));
    assert!(matches!(
        PatchRegionAssessment::new(
            &fixture.expected_patches,
            &fixture.expected_regions,
            &fixture.context,
            &fixture.footprints,
            &fixture.bindings,
            declarations,
            assessment_retained,
            assessment_working - 1,
        ),
        Err(MultiscaleEmbeddingError::WorkingByteBudgetExceeded { required, maximum })
            if required == assessment_working && maximum == assessment_working - 1
    ));
}

#[test]
fn assessment_charges_input_vector_capacity_before_allocating_output() {
    let fixture = fixture();
    let mut overallocated = Vec::with_capacity(32);
    overallocated.extend(declarations());
    assert!(overallocated.capacity() > overallocated.len());
    let required_input = input_bytes(&overallocated);
    let length_only = overallocated.len() * size_of::<PatchRegionDeclaration>()
        + overallocated
            .iter()
            .map(|row| row.patch_id().as_str().len() + row.region_id().as_str().len())
            .sum::<usize>();
    assert!(required_input > length_only);
    assert!(matches!(
        PatchRegionAssessment::new(
            &fixture.expected_patches,
            &fixture.expected_regions,
            &fixture.context,
            &fixture.footprints,
            &fixture.bindings,
            overallocated,
            BUDGET,
            required_input - 1,
        ),
        Err(MultiscaleEmbeddingError::WorkingByteBudgetExceeded { required, maximum })
            if required == required_input && maximum == required_input - 1
    ));
}

#[test]
fn link_enforces_exact_retained_and_working_boundaries() {
    let assessment = assessment();
    let link = PatchRegionLink::from_exhaustive_assessment(
        &assessment,
        artifact(b"patch-region-assessment"),
        ContentDigest::from_bytes(b"patch-region-assessment-content"),
        BUDGET,
        BUDGET,
    )
    .expect("measured link");
    let link_retained = retained_link_bytes(&link);
    let link_working = retained_assessment_bytes(&assessment) + link_retained;
    PatchRegionLink::from_exhaustive_assessment(
        &assessment,
        artifact(b"patch-region-assessment"),
        ContentDigest::from_bytes(b"patch-region-assessment-content"),
        link_retained,
        link_working,
    )
    .expect("exact link budgets");
    assert!(matches!(
        PatchRegionLink::from_exhaustive_assessment(
            &assessment,
            artifact(b"patch-region-assessment"),
            ContentDigest::from_bytes(b"patch-region-assessment-content"),
            link_retained - 1,
            link_working,
        ),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, maximum })
            if required == link_retained && maximum == link_retained - 1
    ));
    assert!(matches!(
        PatchRegionLink::from_exhaustive_assessment(
            &assessment,
            artifact(b"patch-region-assessment"),
            ContentDigest::from_bytes(b"patch-region-assessment-content"),
            link_retained,
            link_working - 1,
        ),
        Err(MultiscaleEmbeddingError::WorkingByteBudgetExceeded { required, maximum })
            if required == link_working && maximum == link_working - 1
    ));
}

#[test]
fn assessment_and_link_reject_aliased_artifact_roles() {
    let fixture = fixture();
    let aliased = PatchRegionAssessmentBindings::new(
        fixture.footprints.expected_patches_artifact_id(),
        fixture.bindings.patch_footprints_artifact_id(),
        fixture.bindings.converter_artifact_id(),
        fixture.bindings.converter_content_digest(),
    );
    assert!(matches!(
        PatchRegionAssessment::new(
            &fixture.expected_patches,
            &fixture.expected_regions,
            &fixture.context,
            &fixture.footprints,
            &aliased,
            declarations(),
            BUDGET,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::DuplicatePatchRegionArtifactDependency)
    ));

    let assessment = assessment();
    assert!(matches!(
        PatchRegionLink::from_exhaustive_assessment(
            &assessment,
            fixture.footprints.expected_patches_artifact_id(),
            ContentDigest::from_bytes(b"aliased-assessment-content"),
            BUDGET,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::DuplicatePatchRegionArtifactDependency)
    ));
}

#[test]
fn empty_exhaustive_assessment_derives_an_empty_link() {
    let fixture = fixture();
    let empty_regions = ExpectedRegionSet::new(
        &fixture.hierarchy,
        fixture.context.owning_slide_id().clone(),
        "empty_regions.v1",
        Vec::new(),
        BUDGET,
    )
    .expect("empty expected regions");
    let empty = PatchRegionAssessment::new(
        &fixture.expected_patches,
        &empty_regions,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        Vec::new(),
        BUDGET,
        BUDGET,
    )
    .expect("empty Cartesian assessment");
    assert_eq!(empty.assessed_pair_count(), 0);
    assert_eq!(empty.nonzero_relation_count(), 0);
    let empty_link = PatchRegionLink::from_exhaustive_assessment(
        &empty,
        artifact(b"empty-assessment"),
        ContentDigest::from_bytes(b"empty-assessment-content"),
        BUDGET,
        BUDGET,
    )
    .expect("empty sparse link");
    assert_eq!(empty_link.assessed_pair_count(), 0);
    assert!(empty_link.nonzero_relations().is_empty());
}
