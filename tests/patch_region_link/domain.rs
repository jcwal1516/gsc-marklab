use super::support::*;

#[test]
fn exhaustive_assessment_derives_the_exact_sparse_declared_link() {
    let fixture = fixture();
    let assessment = PatchRegionAssessment::new(
        &fixture.expected_patches,
        &fixture.expected_regions,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        declarations(),
        BUDGET,
        BUDGET,
    )
    .expect("exhaustive assessment");
    assert_eq!(
        assessment.assessment_policy(),
        "expected_cartesian_exhaustive"
    );
    assert_eq!(assessment.assessed_pair_count(), 4);
    assert_eq!(assessment.nonzero_relation_count(), 3);
    assert_eq!(
        assessment.nonzero_relations_digest(),
        reference_relations_digest(&assessment)
    );
    assert_eq!(
        assessment.nonzero_relations_digest().to_string(),
        "e8380a102ecc8530cef24a9e823e278a8ceb6a0992f0f715ee46ce2d54c5cad6"
    );

    let link = PatchRegionLink::from_exhaustive_assessment(
        &assessment,
        artifact(b"patch-region-assessment"),
        ContentDigest::from_bytes(b"patch-region-assessment-content"),
        BUDGET,
        BUDGET,
    )
    .expect("patch-region link");
    assert_eq!(link.assessed_pair_count(), 4);
    assert_eq!(link.nonzero_relation_count(), 3);
    assert_eq!(link.nonzero_relations(), assessment.nonzero_relations());
    assert_eq!(
        link.nonzero_relations()[0].relation(),
        PatchRegionRelation::FullyContained
    );
    assert_eq!(link.nonzero_relations()[0].numerator(), 1);
    assert_eq!(link.nonzero_relations()[0].denominator(), 1);
    assert_eq!(
        link.nonzero_relations()[1].relation(),
        PatchRegionRelation::PartialOverlap
    );
    assert_eq!(link.nonzero_relations()[1].numerator(), 1);
    assert_eq!(link.nonzero_relations()[1].denominator(), 4);
    assert_eq!(link.logical_digest(), reference_link_digest(&link));
    assert_eq!(
        link.logical_digest().to_string(),
        "290ee111de5f60c016e1f28fe33cb8989a1a9f70b2b507e9fef5c01b031e5b28"
    );
    assert!(!format!("{assessment:?}").contains("patch-a"));
    assert!(!format!("{link:?}").contains("region-a"));
    assert_eq!(
        format!("{:?}", &link.nonzero_relations()[1]),
        "PatchRegionDeclaration { relation: PartialOverlap }"
    );
}

#[test]
fn relation_constructors_and_assessment_reject_invalid_or_noncanonical_rows() {
    assert!(
        PatchRegionDeclaration::partial_overlap(patch("patch-a"), region("region-a"), 0, 1,)
            .is_err()
    );
    assert!(
        PatchRegionDeclaration::partial_overlap(patch("patch-a"), region("region-a"), 1, 1,)
            .is_err()
    );
    assert!(
        PatchRegionDeclaration::partial_overlap(patch("patch-a"), region("region-a"), 3, 2,)
            .is_err()
    );
    assert!(
        PatchRegionDeclaration::partial_overlap(patch("patch-a"), region("region-a"), 2, 4,)
            .is_err()
    );

    let fixture = fixture();
    let cases = [
        vec![
            PatchRegionDeclaration::partial_overlap(patch("patch-b"), region("region-a"), 1, 2)
                .expect("half"),
            PatchRegionDeclaration::fully_contained(patch("patch-a"), region("region-a")),
        ],
        vec![
            PatchRegionDeclaration::fully_contained(patch("patch-a"), region("region-a")),
            PatchRegionDeclaration::fully_contained(patch("patch-a"), region("region-a")),
        ],
        vec![PatchRegionDeclaration::fully_contained(
            patch("patch-z"),
            region("region-a"),
        )],
        vec![PatchRegionDeclaration::fully_contained(
            patch("patch-a"),
            region("region-z"),
        )],
    ];
    for invalid in cases {
        assert!(matches!(
            PatchRegionAssessment::new(
                &fixture.expected_patches,
                &fixture.expected_regions,
                &fixture.context,
                &fixture.footprints,
                &fixture.bindings,
                invalid,
                BUDGET,
                BUDGET,
            ),
            Err(MultiscaleEmbeddingError::InvalidPatchRegionDeclarations { .. })
        ));
    }
}

#[test]
fn assessment_rejects_expected_set_or_slide_binding_drift() {
    let fixture = fixture();
    let drifted_patches = ExpectedPatchSet::new(
        &fixture.hierarchy,
        fixture.context.owning_slide_id().clone(),
        "different_patch_selection.v1",
        vec![patch("patch-a"), patch("patch-b")],
        BUDGET,
    )
    .expect("drifted expected patches");
    assert!(matches!(
        PatchRegionAssessment::new(
            &drifted_patches,
            &fixture.expected_regions,
            &fixture.context,
            &fixture.footprints,
            &fixture.bindings,
            declarations(),
            BUDGET,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::PatchRegionInputMismatch)
    ));

    let other_slide = SlideId::new("other-region-slide").expect("other slide");
    let other_region = region("other-region");
    let other_patient =
        HierarchyId::from(PatientId::new("other-region-patient").expect("other patient"));
    let other_hierarchy = CohortHierarchy::new(
        vec![
            HierarchyNode::new(other_patient.clone(), None, ReplicationRole::BiologicalUnit),
            HierarchyNode::new(
                HierarchyId::from(other_slide.clone()),
                None,
                ReplicationRole::TechnicalReplicate {
                    biological_source: other_patient,
                },
            ),
            HierarchyNode::new(
                HierarchyId::from(other_region.clone()),
                Some(HierarchyId::from(other_slide.clone())),
                ReplicationRole::Structural,
            ),
        ],
        Vec::new(),
    )
    .expect("other hierarchy");
    let other_regions = ExpectedRegionSet::new(
        &other_hierarchy,
        other_slide,
        "other_regions.v1",
        vec![other_region],
        BUDGET,
    )
    .expect("other expected regions");
    assert!(matches!(
        PatchRegionAssessment::new(
            &fixture.expected_patches,
            &other_regions,
            &fixture.context,
            &fixture.footprints,
            &fixture.bindings,
            Vec::new(),
            BUDGET,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::PatchRegionInputMismatch)
    ));
}

#[test]
fn logical_identity_changes_with_every_external_evidence_binding() {
    let fixture = fixture();
    let assessment = PatchRegionAssessment::new(
        &fixture.expected_patches,
        &fixture.expected_regions,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        declarations(),
        BUDGET,
        BUDGET,
    )
    .expect("assessment");
    let baseline = PatchRegionLink::from_exhaustive_assessment(
        &assessment,
        artifact(b"assessment-a"),
        ContentDigest::from_bytes(b"assessment-content-a"),
        BUDGET,
        BUDGET,
    )
    .expect("baseline link");
    let artifact_drift = PatchRegionLink::from_exhaustive_assessment(
        &assessment,
        artifact(b"assessment-b"),
        ContentDigest::from_bytes(b"assessment-content-a"),
        BUDGET,
        BUDGET,
    )
    .expect("artifact drift link");
    let content_drift = PatchRegionLink::from_exhaustive_assessment(
        &assessment,
        artifact(b"assessment-a"),
        ContentDigest::from_bytes(b"assessment-content-b"),
        BUDGET,
        BUDGET,
    )
    .expect("content drift link");
    assert_ne!(baseline.logical_digest(), artifact_drift.logical_digest());
    assert_ne!(baseline.logical_digest(), content_drift.logical_digest());
}
