use super::support::*;

#[test]
fn expected_sets_are_typed_canonical_and_slide_owned() {
    let (hierarchy, slide) = hierarchy();
    let patches = expected_patches(&hierarchy, &slide);
    let regions = ExpectedRegionSet::new(
        &hierarchy,
        slide.clone(),
        "all_regions.v1",
        vec![region("region-a")],
        RETAINED_BUDGET,
    )
    .expect("expected regions");
    let slides = ExpectedSlideSet::new(
        &hierarchy,
        slide.clone(),
        "owning_slide.v1",
        vec![slide.clone()],
        RETAINED_BUDGET,
    )
    .expect("expected slide");

    assert_eq!(patches.owning_slide_id(), &slide);
    assert_eq!(patches.ids(), &[patch("patch-a"), patch("patch-b")]);
    assert_eq!(regions.ids(), &[region("region-a")]);
    assert_eq!(slides.ids(), std::slice::from_ref(&slide));
    assert_eq!(
        patches.logical_digest().to_string(),
        "4b1842f6627482860caba7e80792ba7529cba75231f627e44f775b97d65ddf34"
    );
    assert_eq!(
        regions.logical_digest().to_string(),
        "a6275d3e3192c20dbed9165c870f5605a0a5fead9100dd8ce2e8ef6ee49672b8"
    );
    assert_eq!(
        slides.logical_digest().to_string(),
        "46551dcb0bc417f3527de09fd846edfc8fe9a4f46be457e986bce2c48a08deb6"
    );

    let encoded = patches.to_canonical_json().expect("canonical patch set");
    assert_eq!(
        encoded,
        concat!(
            "{\"format\":\"marklab.expected_patch_set\",\"version\":1,",
            "\"owning_slide_id\":\"slide\",\"selection_rule\":\"all_patches.v1\",",
            "\"ids\":[\"patch-a\",\"patch-b\"]}\n"
        )
        .as_bytes()
    );
    assert_eq!(
        regions.to_canonical_json().expect("canonical region set"),
        concat!(
            "{\"format\":\"marklab.expected_region_set\",\"version\":1,",
            "\"owning_slide_id\":\"slide\",\"selection_rule\":\"all_regions.v1\",",
            "\"ids\":[\"region-a\"]}\n"
        )
        .as_bytes()
    );
    assert_eq!(
        slides.to_canonical_json().expect("canonical slide set"),
        concat!(
            "{\"format\":\"marklab.expected_slide_set\",\"version\":1,",
            "\"owning_slide_id\":\"slide\",\"selection_rule\":\"owning_slide.v1\",",
            "\"ids\":[\"slide\"]}\n"
        )
        .as_bytes()
    );
    assert!(encoded.ends_with(b"\n"));
    assert_eq!(
        ExpectedPatchSet::from_canonical_json(
            &encoded,
            &hierarchy,
            encoded.len(),
            RETAINED_BUDGET,
            RETAINED_BUDGET,
        )
        .expect("patch-set round trip"),
        patches
    );
    assert!(ExpectedPatchSet::from_canonical_json(
        &encoded,
        &hierarchy,
        encoded.len() - 1,
        RETAINED_BUDGET,
        RETAINED_BUDGET,
    )
    .is_err());

    assert!(ExpectedPatchSet::new(
        &hierarchy,
        slide.clone(),
        "all_patches.v1",
        vec![patch("patch-b"), patch("patch-a")],
        RETAINED_BUDGET,
    )
    .is_err());
    assert!(ExpectedSlideSet::new(
        &hierarchy,
        slide.clone(),
        "owning_slide.v1",
        Vec::new(),
        RETAINED_BUDGET,
    )
    .is_err());

    let empty_patches = ExpectedPatchSet::new(
        &hierarchy,
        slide.clone(),
        "no_patches.v1",
        Vec::new(),
        RETAINED_BUDGET,
    )
    .expect("empty patch set is structurally valid");
    let empty_regions = ExpectedRegionSet::new(
        &hierarchy,
        slide,
        "no_regions.v1",
        Vec::new(),
        RETAINED_BUDGET,
    )
    .expect("empty region set is structurally valid");
    assert!(empty_patches.ids().is_empty());
    assert!(empty_regions.ids().is_empty());
}

#[test]
fn expected_sets_reject_cross_slide_ancestry_and_multiple_slide_rows() {
    let patient = HierarchyId::from(PatientId::new("two-slide-patient").expect("patient"));
    let slide_a = SlideId::new("slide-a").expect("slide A");
    let slide_b = SlideId::new("slide-b").expect("slide B");
    let hierarchy = CohortHierarchy::new(
        vec![
            HierarchyNode::new(patient.clone(), None, ReplicationRole::BiologicalUnit),
            HierarchyNode::new(
                HierarchyId::from(slide_a.clone()),
                None,
                ReplicationRole::TechnicalReplicate {
                    biological_source: patient.clone(),
                },
            ),
            HierarchyNode::new(
                HierarchyId::from(slide_b.clone()),
                None,
                ReplicationRole::TechnicalReplicate {
                    biological_source: patient,
                },
            ),
            HierarchyNode::new(
                HierarchyId::from(patch("patch-a")),
                Some(HierarchyId::from(slide_a.clone())),
                ReplicationRole::Structural,
            ),
            HierarchyNode::new(
                HierarchyId::from(patch("patch-b")),
                Some(HierarchyId::from(slide_b.clone())),
                ReplicationRole::Structural,
            ),
        ],
        Vec::new(),
    )
    .expect("two-slide hierarchy");
    assert!(ExpectedPatchSet::new(
        &hierarchy,
        slide_a.clone(),
        "cross_slide.v1",
        vec![patch("patch-a"), patch("patch-b")],
        RETAINED_BUDGET,
    )
    .is_err());
    assert!(ExpectedSlideSet::new(
        &hierarchy,
        slide_a.clone(),
        "multiple_slides.v1",
        vec![slide_a, slide_b],
        RETAINED_BUDGET,
    )
    .is_err());
}
