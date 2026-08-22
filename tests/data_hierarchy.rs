use marklab::{MarklabProject, ProjectError};
use marklab_data::{
    BiologicalSourceError, BlockId, CellId, CohortHierarchy, CoreId, HierarchyError, HierarchyId,
    HierarchyKind, HierarchyNode, IdentityError, PatchId, PatientId, RegionId,
    RepeatedMeasureError, RepeatedMeasureLinkError, RepeatedMeasureSet, ReplicationRole,
    ReplicationRoleKind, SectionId, SiteId, SlideId, SpecimenId, TimepointId,
};

macro_rules! assert_id_contract {
    ($id_type:ty, $kind:expr) => {{
        assert!(matches!(
            <$id_type>::new(""),
            Err(IdentityError::Blank { kind }) if kind == $kind
        ));
        assert!(matches!(
            <$id_type>::new(" value"),
            Err(IdentityError::SurroundingWhitespace { kind }) if kind == $kind
        ));
        assert!(matches!(
            <$id_type>::new("value\n"),
            Err(IdentityError::ControlCharacter { kind }) if kind == $kind
        ));
        assert!(matches!(
            <$id_type>::new("x".repeat(256)),
            Err(IdentityError::TooLong { kind, byte_len: 256 }) if kind == $kind
        ));
    }};
}

fn site(value: &str) -> HierarchyId {
    SiteId::new(value).expect("site ID").into()
}

fn patient(value: &str) -> HierarchyId {
    PatientId::new(value).expect("patient ID").into()
}

fn timepoint(value: &str) -> HierarchyId {
    TimepointId::new(value).expect("timepoint ID").into()
}

fn specimen(value: &str) -> HierarchyId {
    SpecimenId::new(value).expect("specimen ID").into()
}

fn block(value: &str) -> HierarchyId {
    BlockId::new(value).expect("block ID").into()
}

fn slide(value: &str) -> HierarchyId {
    SlideId::new(value).expect("slide ID").into()
}

fn section(value: &str) -> HierarchyId {
    SectionId::new(value).expect("section ID").into()
}

fn core(value: &str) -> HierarchyId {
    CoreId::new(value).expect("core ID").into()
}

fn region(value: &str) -> HierarchyId {
    RegionId::new(value).expect("region ID").into()
}

fn cell(value: &str) -> HierarchyId {
    CellId::new(value).expect("cell ID").into()
}

fn structural(id: HierarchyId, parent: Option<HierarchyId>) -> HierarchyNode {
    HierarchyNode::new(id, parent, ReplicationRole::Structural)
}

fn biological(id: HierarchyId, parent: Option<HierarchyId>) -> HierarchyNode {
    HierarchyNode::new(id, parent, ReplicationRole::BiologicalUnit)
}

#[test]
fn typed_ids_are_opaque_validated_and_distinct_by_kind() {
    assert_id_contract!(PatientId, HierarchyKind::Patient);
    assert_id_contract!(SiteId, HierarchyKind::Site);
    assert_id_contract!(SpecimenId, HierarchyKind::Specimen);
    assert_id_contract!(TimepointId, HierarchyKind::Timepoint);
    assert_id_contract!(BlockId, HierarchyKind::Block);
    assert_id_contract!(SlideId, HierarchyKind::Slide);
    assert_id_contract!(SectionId, HierarchyKind::Section);
    assert_id_contract!(CoreId, HierarchyKind::Core);
    assert_id_contract!(RegionId, HierarchyKind::Region);
    assert_id_contract!(CellId, HierarchyKind::Cell);
    assert_id_contract!(PatchId, HierarchyKind::Patch);

    let path_like = "patient-01/slides/section.svs::cell-0007";
    let cell_id = CellId::new(path_like).expect("opaque path-like ID");
    assert_eq!(cell_id.as_str(), path_like);
    let exact_limit = "x".repeat(255);
    assert_eq!(
        PatientId::new(&exact_limit)
            .expect("255-byte ID is valid")
            .as_str(),
        exact_limit
    );
    assert_ne!(
        HierarchyId::from(PatientId::new("shared").expect("patient")),
        HierarchyId::from(CellId::new("shared").expect("cell")),
        "the same opaque text in different typed kinds is legal and distinct"
    );
}

#[test]
fn replication_roles_are_restricted_by_object_kind() {
    let patient_id = patient("patient");
    let specimen_id = specimen("specimen");
    let slide_id = slide("slide");

    let invalid = [
        (
            site("biological-site"),
            None,
            ReplicationRoleKind::BiologicalUnit,
        ),
        (
            timepoint("biological-timepoint"),
            Some(patient_id.clone()),
            ReplicationRoleKind::BiologicalUnit,
        ),
        (
            block("biological-block"),
            Some(specimen_id.clone()),
            ReplicationRoleKind::BiologicalUnit,
        ),
        (
            slide("biological-slide"),
            None,
            ReplicationRoleKind::BiologicalUnit,
        ),
        (
            section("biological-section"),
            Some(slide_id.clone()),
            ReplicationRoleKind::BiologicalUnit,
        ),
        (
            core("biological-core"),
            Some(slide_id.clone()),
            ReplicationRoleKind::BiologicalUnit,
        ),
        (
            region("biological-region"),
            Some(slide_id.clone()),
            ReplicationRoleKind::BiologicalUnit,
        ),
        (
            cell("biological-cell"),
            Some(slide_id.clone()),
            ReplicationRoleKind::BiologicalUnit,
        ),
        (
            HierarchyId::from(PatchId::new("biological-patch").expect("patch")),
            Some(slide_id.clone()),
            ReplicationRoleKind::BiologicalUnit,
        ),
    ];

    for (invalid_id, parent, role) in invalid {
        let nodes = vec![
            biological(patient_id.clone(), None),
            structural(specimen_id.clone(), Some(patient_id.clone())),
            structural(slide_id.clone(), None),
            HierarchyNode::new(invalid_id.clone(), parent, ReplicationRole::BiologicalUnit),
        ];
        assert!(matches!(
            CohortHierarchy::new(nodes, Vec::new()),
            Err(HierarchyError::UnsupportedReplicationRole { node, role: observed })
                if node == invalid_id && observed == role
        ));
    }

    let biological_specimen = specimen("biological-specimen");
    let subsample = core("subsample-core");
    let technical = region("technical-region");
    let inherited = cell("inherited-cell");
    let hierarchy = CohortHierarchy::new(
        vec![
            biological(patient_id.clone(), None),
            biological(biological_specimen.clone(), Some(patient_id.clone())),
            structural(slide_id.clone(), None),
            HierarchyNode::new(
                subsample.clone(),
                Some(slide_id),
                ReplicationRole::BiologicalSubsample {
                    biological_source: patient_id.clone(),
                },
            ),
            HierarchyNode::new(
                technical.clone(),
                Some(subsample),
                ReplicationRole::TechnicalReplicate {
                    biological_source: patient_id.clone(),
                },
            ),
            structural(inherited.clone(), Some(technical)),
        ],
        Vec::new(),
    )
    .expect("allowed role-kind combinations");
    assert_eq!(
        hierarchy.resolved_biological_unit(&biological_specimen),
        Some(&biological_specimen)
    );
    assert_eq!(
        hierarchy.resolved_biological_unit(&inherited),
        Some(&patient_id)
    );
}

#[test]
fn nested_biological_levels_support_explicit_patient_level_designs() {
    let patient_id = patient("nested-patient");
    let specimen_a = specimen("nested-specimen-a");
    let specimen_b = specimen("nested-specimen-b");
    let tma_slide = slide("nested-tma-slide");
    let core_a = core("nested-core-a");
    let core_b = core("nested-core-b");
    let region_a = region("nested-region-a");
    let region_b = region("nested-region-b");
    let pair = RepeatedMeasureSet::new(
        patient_id.clone(),
        vec![specimen_a.clone(), specimen_b.clone()],
    )
    .expect("patient-level specimen pair");

    let hierarchy = CohortHierarchy::new(
        vec![
            biological(patient_id.clone(), None),
            biological(specimen_a.clone(), Some(patient_id.clone())),
            biological(specimen_b.clone(), Some(patient_id.clone())),
            structural(tma_slide.clone(), None),
            HierarchyNode::new(
                core_a.clone(),
                Some(tma_slide.clone()),
                ReplicationRole::BiologicalSubsample {
                    biological_source: specimen_a.clone(),
                },
            ),
            HierarchyNode::new(
                core_b.clone(),
                Some(tma_slide),
                ReplicationRole::BiologicalSubsample {
                    biological_source: specimen_b.clone(),
                },
            ),
            structural(region_a.clone(), Some(core_a)),
            structural(region_b, Some(core_b)),
        ],
        vec![pair],
    )
    .expect("nested biological levels remain selectable");

    assert_eq!(
        hierarchy.resolved_biological_unit(&specimen_a),
        Some(&specimen_a),
        "nearest-unit resolution remains specific"
    );
    assert!(hierarchy.belongs_to_biological_unit(&specimen_a, &patient_id));
    assert!(hierarchy.belongs_to_biological_unit(&region_a, &patient_id));
    assert!(hierarchy.belongs_to_biological_unit(&region_a, &specimen_a));
    assert!(!hierarchy.belongs_to_biological_unit(&region_a, &specimen_b));
    assert!(!hierarchy.belongs_to_biological_unit(&region_a, &region_a));
    assert!(!hierarchy.belongs_to_biological_unit(&region_a, &patient("absent-patient")));
    assert_eq!(hierarchy.design_summary().pair_set_count(), 1);
    assert_eq!(
        hierarchy.design_summary().multicore_biological_unit_count(),
        1,
        "the patient is the only declared unit with at least two cores"
    );
    assert_eq!(
        hierarchy
            .design_summary()
            .multiregion_biological_unit_count(),
        1,
        "the patient is the only declared unit with at least two top-level regions"
    );
}

#[test]
fn duplicate_conflicting_missing_and_unsupported_parents_are_rejected() {
    let site_a = site("site-a");
    let site_b = site("site-b");
    let patient_id = patient("patient-1");
    let declaration = biological(patient_id.clone(), Some(site_a.clone()));

    assert!(matches!(
        CohortHierarchy::new(
            vec![
                structural(site_a.clone(), None),
                declaration.clone(),
                declaration.clone(),
            ],
            Vec::new(),
        ),
        Err(HierarchyError::DuplicateId { id }) if id == patient_id
    ));

    assert!(matches!(
        CohortHierarchy::new(
            vec![
                structural(site_a.clone(), None),
                structural(site_b.clone(), None),
                declaration,
                biological(patient_id.clone(), Some(site_b)),
            ],
            Vec::new(),
        ),
        Err(HierarchyError::ConflictingParent { id, .. }) if id == patient_id
    ));

    let missing_region = region("missing-region");
    let cell_id = cell("cell-with-missing-parent");
    assert!(matches!(
        CohortHierarchy::new(
            vec![
                biological(patient("unrelated-patient"), None),
                structural(cell_id.clone(), Some(missing_region.clone())),
            ],
            Vec::new(),
        ),
        Err(HierarchyError::MissingParent {
            child,
            parent: Some(parent),
        }) if child == cell_id && parent == missing_region
    ));

    let unsupported_slide = slide("slide-under-patient");
    let parent = patient("patient-parent");
    assert!(matches!(
        CohortHierarchy::new(
            vec![
                biological(parent.clone(), None),
                structural(unsupported_slide.clone(), Some(parent.clone())),
            ],
            Vec::new(),
        ),
        Err(HierarchyError::UnsupportedNesting {
            child,
            parent: observed_parent,
        }) if child == unsupported_slide && observed_parent == parent
    ));
}

#[test]
fn cycles_are_rejected_before_biological_resolution() {
    let region_a = region("region-a");
    let region_b = region("region-b");
    let error = CohortHierarchy::new(
        vec![
            biological(patient("patient-1"), None),
            structural(region_a.clone(), Some(region_b.clone())),
            structural(region_b.clone(), Some(region_a.clone())),
        ],
        Vec::new(),
    )
    .expect_err("cycle must fail");

    assert!(matches!(
        error,
        HierarchyError::Cycle { nodes }
            if nodes == vec![region_a, region_b]
    ));
}

#[test]
fn biological_sources_and_terminal_branches_are_explicit() {
    let donor = patient("donor");
    let missing_donor = patient("missing-donor");
    let tma_slide = slide("tma-slide");
    let tma_core = core("tma-core");

    assert!(matches!(
        CohortHierarchy::new(Vec::new(), Vec::new()),
        Err(HierarchyError::NoBiologicalUnit)
    ));
    assert!(matches!(
        CohortHierarchy::new(vec![structural(site("unused-site"), None)], Vec::new(),),
        Err(HierarchyError::NoBiologicalUnit)
    ));

    assert!(matches!(
        CohortHierarchy::new(
            vec![
                biological(donor.clone(), None),
                structural(tma_slide.clone(), None),
                HierarchyNode::new(
                    tma_core.clone(),
                    Some(tma_slide.clone()),
                    ReplicationRole::BiologicalSubsample {
                        biological_source: missing_donor.clone(),
                    },
                ),
            ],
            Vec::new(),
        ),
        Err(HierarchyError::InvalidBiologicalSource {
            node,
            biological_source,
            reason: BiologicalSourceError::Missing,
        }) if node == tma_core && biological_source == missing_donor
    ));

    let structural_source = patient("structural-source");
    assert!(matches!(
        CohortHierarchy::new(
            vec![
                biological(donor.clone(), None),
                structural(structural_source.clone(), None),
                HierarchyNode::new(
                    core("core-with-structural-source"),
                    Some(tma_slide.clone()),
                    ReplicationRole::TechnicalReplicate {
                        biological_source: structural_source.clone(),
                    },
                ),
                structural(tma_slide.clone(), None),
            ],
            Vec::new(),
        ),
        Err(HierarchyError::InvalidBiologicalSource {
            biological_source,
            reason: BiologicalSourceError::NotBiological,
            ..
        }) if biological_source == structural_source
    ));

    let self_sourced = specimen("self-sourced");
    assert!(matches!(
        CohortHierarchy::new(
            vec![
                biological(donor.clone(), None),
                HierarchyNode::new(
                    self_sourced.clone(),
                    Some(donor.clone()),
                    ReplicationRole::BiologicalSubsample {
                        biological_source: self_sourced.clone(),
                    },
                ),
            ],
            Vec::new(),
        ),
        Err(HierarchyError::InvalidBiologicalSource {
            node,
            reason: BiologicalSourceError::SelfReference,
            ..
        }) if node == self_sourced
    ));

    let unresolved_region = region("unresolved-region");
    assert!(matches!(
        CohortHierarchy::new(
            vec![
                biological(donor, None),
                structural(tma_slide.clone(), None),
                structural(unresolved_region.clone(), Some(tma_slide)),
            ],
            Vec::new(),
        ),
        Err(HierarchyError::UnresolvedBiologicalUnit { observation })
            if observation == unresolved_region
    ));
}

#[test]
fn paired_repeated_multisite_multicore_and_multiregion_are_factual() {
    let site_a = site("site-a");
    let site_b = site("site-b");
    let empty_site = site("empty-site");
    let patient_a = patient("patient-a");
    let patient_b = patient("patient-b");
    let a_pre_time = timepoint("patient-a/pre");
    let a_post_time = timepoint("patient-a/post");
    let b_t1_time = timepoint("patient-b/t1");
    let b_t2_time = timepoint("patient-b/t2");
    let b_t3_time = timepoint("patient-b/t3");
    let a_pre = specimen("patient-a/pre/specimen");
    let a_post = specimen("patient-a/post/specimen");
    let b_t1 = specimen("patient-b/t1/specimen");
    let b_t2 = specimen("patient-b/t2/specimen");
    let b_t3 = specimen("patient-b/t3/specimen");
    let tma_slide = slide("tma-slide");
    let a_core_1 = core("patient-a/core-1");
    let a_core_2 = core("patient-a/core-2");
    let b_core_1 = core("patient-b/core-1");
    let a_region_1 = region("patient-a/region-1");
    let a_region_2 = region("patient-a/region-2");
    let nested_region = region("patient-a/region-1/nested");

    let nodes = vec![
        structural(site_a.clone(), None),
        structural(site_b.clone(), None),
        structural(empty_site, None),
        biological(patient_a.clone(), Some(site_a)),
        biological(patient_b.clone(), Some(site_b)),
        structural(a_pre_time.clone(), Some(patient_a.clone())),
        structural(a_post_time.clone(), Some(patient_a.clone())),
        structural(b_t1_time.clone(), Some(patient_b.clone())),
        structural(b_t2_time.clone(), Some(patient_b.clone())),
        structural(b_t3_time.clone(), Some(patient_b.clone())),
        structural(a_pre.clone(), Some(a_pre_time)),
        structural(a_post.clone(), Some(a_post_time)),
        structural(b_t1.clone(), Some(b_t1_time)),
        structural(b_t2.clone(), Some(b_t2_time)),
        structural(b_t3.clone(), Some(b_t3_time)),
        structural(tma_slide.clone(), None),
        HierarchyNode::new(
            a_core_1.clone(),
            Some(tma_slide.clone()),
            ReplicationRole::BiologicalSubsample {
                biological_source: patient_a.clone(),
            },
        ),
        HierarchyNode::new(
            a_core_2.clone(),
            Some(tma_slide.clone()),
            ReplicationRole::BiologicalSubsample {
                biological_source: patient_a.clone(),
            },
        ),
        HierarchyNode::new(
            b_core_1,
            Some(tma_slide),
            ReplicationRole::BiologicalSubsample {
                biological_source: patient_b.clone(),
            },
        ),
        structural(a_region_1.clone(), Some(a_core_1)),
        structural(a_region_2, Some(a_core_2)),
        HierarchyNode::new(
            nested_region,
            Some(a_region_1),
            ReplicationRole::TechnicalReplicate {
                biological_source: patient_a.clone(),
            },
        ),
    ];
    let pair = RepeatedMeasureSet::new(patient_a.clone(), vec![a_pre.clone(), a_post.clone()])
        .expect("pair set");
    let repeated = RepeatedMeasureSet::new(
        patient_b.clone(),
        vec![b_t1.clone(), b_t2.clone(), b_t3.clone()],
    )
    .expect("repeated set");

    let hierarchy =
        CohortHierarchy::new(nodes, vec![pair, repeated]).expect("valid cohort hierarchy");
    let summary = hierarchy.design_summary();
    assert_eq!(summary.object_count(HierarchyKind::Site), 3);
    assert_eq!(summary.object_count(HierarchyKind::Patient), 2);
    assert_eq!(summary.object_count(HierarchyKind::Specimen), 5);
    assert_eq!(summary.object_count(HierarchyKind::Core), 3);
    assert_eq!(summary.object_count(HierarchyKind::Region), 3);
    assert_eq!(summary.biological_unit_count(), 2);
    assert_eq!(summary.biological_subsample_count(), 3);
    assert_eq!(summary.technical_replicate_count(), 1);
    assert_eq!(summary.active_site_count(), 2);
    assert!(summary.is_multisite());
    assert_eq!(summary.pair_set_count(), 1);
    assert_eq!(summary.repeated_set_count(), 1);
    assert_eq!(summary.multicore_biological_unit_count(), 1);
    assert_eq!(summary.multiregion_biological_unit_count(), 1);
    assert_eq!(hierarchy.resolved_biological_unit(&a_pre), Some(&patient_a));
    assert_eq!(hierarchy.resolved_biological_unit(&b_t3), Some(&patient_b));
}

#[test]
fn repeated_measure_sets_reject_ambiguous_membership() {
    let patient_a = patient("patient-a");
    let patient_b = patient("patient-b");
    let specimen_a = specimen("specimen-a");
    let specimen_b = specimen("specimen-b");
    let slide_a = slide("slide-a");

    assert!(matches!(
        RepeatedMeasureSet::new(patient_a.clone(), vec![specimen_a.clone()]),
        Err(RepeatedMeasureError::TooFewObservations { observed: 1 })
    ));
    assert!(matches!(
        RepeatedMeasureSet::new(
            patient_a.clone(),
            vec![specimen_a.clone(), specimen_a.clone()],
        ),
        Err(RepeatedMeasureError::DuplicateObservation { observation })
            if observation == specimen_a
    ));
    assert!(matches!(
        RepeatedMeasureSet::new(patient_a.clone(), vec![specimen_a.clone(), slide_a.clone()],),
        Err(RepeatedMeasureError::MixedObservationKinds { .. })
    ));

    let set = RepeatedMeasureSet::new(
        patient_a.clone(),
        vec![specimen_a.clone(), specimen_b.clone()],
    )
    .expect("shape-valid set");
    assert!(matches!(
        CohortHierarchy::new(
            vec![
                biological(patient_a.clone(), None),
                biological(patient_b.clone(), None),
                structural(specimen_a, Some(patient_b.clone())),
                structural(specimen_b, Some(patient_b)),
            ],
            vec![set],
        ),
        Err(HierarchyError::InvalidRepeatedMeasureSet { biological_unit, .. })
            if biological_unit == patient_a
    ));

    let patient_id = patient("patient-with-kind-specific-sets");
    let specimens = [
        specimen("set-a-1"),
        specimen("set-a-2"),
        specimen("set-b-1"),
        specimen("set-b-2"),
    ];
    let tma_slide = slide("kind-specific-tma");
    let cores = [core("core-set-1"), core("core-set-2")];
    let mut nodes = std::iter::once(biological(patient_id.clone(), None))
        .chain(
            specimens
                .iter()
                .cloned()
                .map(|id| structural(id, Some(patient_id.clone()))),
        )
        .collect::<Vec<_>>();
    nodes.push(structural(tma_slide.clone(), None));
    nodes.extend(cores.iter().cloned().map(|id| {
        HierarchyNode::new(
            id,
            Some(tma_slide.clone()),
            ReplicationRole::BiologicalSubsample {
                biological_source: patient_id.clone(),
            },
        )
    }));
    let specimen_set = RepeatedMeasureSet::new(
        patient_id.clone(),
        vec![specimens[0].clone(), specimens[1].clone()],
    )
    .expect("specimen pair");
    let core_set =
        RepeatedMeasureSet::new(patient_id.clone(), vec![cores[0].clone(), cores[1].clone()])
            .expect("core pair");
    let hierarchy = CohortHierarchy::new(nodes.clone(), vec![specimen_set.clone(), core_set])
        .expect("one set per observation kind");
    assert_eq!(hierarchy.design_summary().pair_set_count(), 2);

    let second_specimen_set = RepeatedMeasureSet::new(
        patient_id.clone(),
        vec![specimens[2].clone(), specimens[3].clone()],
    )
    .expect("second specimen pair");
    assert!(matches!(
        CohortHierarchy::new(nodes, vec![specimen_set, second_specimen_set]),
        Err(HierarchyError::InvalidRepeatedMeasureSet {
            biological_unit,
            reason: RepeatedMeasureLinkError::DuplicateSet,
        }) if biological_unit == patient_id
    ));
}

#[test]
fn repeated_measure_links_report_missing_nonbiological_and_reused_members() {
    let patient_a = patient("patient-a");
    let patient_b = patient("patient-b");
    let nonbiological_unit = specimen("not-a-biological-unit");
    let a1 = specimen("a-1");
    let a2 = specimen("a-2");
    let b1 = specimen("b-1");
    let missing = specimen("missing");
    let nodes = vec![
        biological(patient_a.clone(), None),
        biological(patient_b.clone(), None),
        structural(nonbiological_unit.clone(), Some(patient_a.clone())),
        structural(a1.clone(), Some(patient_a.clone())),
        structural(a2.clone(), Some(patient_a.clone())),
        structural(b1.clone(), Some(patient_b.clone())),
    ];

    let missing_unit = patient("missing-unit");
    let set = RepeatedMeasureSet::new(missing_unit.clone(), vec![a1.clone(), a2.clone()])
        .expect("shape-valid set");
    assert!(matches!(
        CohortHierarchy::new(nodes.clone(), vec![set]),
        Err(HierarchyError::InvalidRepeatedMeasureSet {
            biological_unit,
            reason: RepeatedMeasureLinkError::MissingBiologicalUnit,
        }) if biological_unit == missing_unit
    ));

    let set = RepeatedMeasureSet::new(nonbiological_unit.clone(), vec![a1.clone(), a2.clone()])
        .expect("shape-valid set");
    assert!(matches!(
        CohortHierarchy::new(nodes.clone(), vec![set]),
        Err(HierarchyError::InvalidRepeatedMeasureSet {
            biological_unit,
            reason: RepeatedMeasureLinkError::SourceNotBiological,
        }) if biological_unit == nonbiological_unit
    ));

    let set = RepeatedMeasureSet::new(patient_a.clone(), vec![a1.clone(), missing.clone()])
        .expect("shape-valid set");
    assert!(matches!(
        CohortHierarchy::new(nodes.clone(), vec![set]),
        Err(HierarchyError::InvalidRepeatedMeasureSet {
            reason: RepeatedMeasureLinkError::MissingObservation { observation },
            ..
        }) if observation == missing
    ));

    let first =
        RepeatedMeasureSet::new(patient_a.clone(), vec![a1.clone(), a2]).expect("first set");
    let reuses_a1 = RepeatedMeasureSet::new(patient_b.clone(), vec![a1.clone(), b1])
        .expect("second shape-valid set");
    assert!(matches!(
        CohortHierarchy::new(nodes, vec![first, reuses_a1]),
        Err(HierarchyError::InvalidRepeatedMeasureSet {
            biological_unit,
            reason: RepeatedMeasureLinkError::DuplicateObservation { observation },
        }) if biological_unit == patient_b && observation == a1
    ));
}

#[test]
fn nested_regions_do_not_inflate_multiregion_counts() {
    let patient_id = patient("patient");
    let slide_id = slide("slide");
    let top = region("top");
    let nested = region("top/nested");
    let hierarchy = CohortHierarchy::new(
        vec![
            biological(patient_id, None),
            structural(slide_id.clone(), None),
            HierarchyNode::new(
                top.clone(),
                Some(slide_id),
                ReplicationRole::BiologicalSubsample {
                    biological_source: patient("patient"),
                },
            ),
            structural(nested, Some(top)),
        ],
        Vec::new(),
    )
    .expect("one top-level and one nested region");

    assert_eq!(
        hierarchy
            .design_summary()
            .multiregion_biological_unit_count(),
        0
    );
}

#[test]
fn deep_hierarchies_and_cycle_tails_are_checked_iteratively() {
    const DEPTH: usize = 10_000;

    let patient_id = patient("deep-patient");
    let slide_id = slide("deep-slide");
    let core_id = core("deep-core");
    let mut nodes = vec![
        biological(patient_id.clone(), None),
        structural(slide_id.clone(), None),
        HierarchyNode::new(
            core_id.clone(),
            Some(slide_id),
            ReplicationRole::BiologicalSubsample {
                biological_source: patient_id.clone(),
            },
        ),
    ];
    let mut parent = core_id;
    for index in 0..DEPTH {
        let id = region(&format!("deep-region-{index}"));
        nodes.push(structural(id.clone(), Some(parent)));
        parent = id;
    }
    let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("deep acyclic hierarchy");
    assert_eq!(
        hierarchy.resolved_biological_unit(&parent),
        Some(&patient_id)
    );

    let cycle_ids = (0..DEPTH)
        .map(|index| region(&format!("cycle-region-{index}")))
        .collect::<Vec<_>>();
    let mut cycle_nodes = Vec::with_capacity(DEPTH + 1);
    cycle_nodes.push(biological(patient("cycle-patient"), None));
    for index in 0..DEPTH {
        let parent_index = if index + 1 < DEPTH {
            index + 1
        } else {
            DEPTH - 100
        };
        cycle_nodes.push(structural(
            cycle_ids[index].clone(),
            Some(cycle_ids[parent_index].clone()),
        ));
    }
    assert!(matches!(
        CohortHierarchy::new(cycle_nodes, Vec::new()),
        Err(HierarchyError::Cycle { nodes })
            if nodes.len() == 100 && nodes[0] == cycle_ids[DEPTH - 100]
    ));
}

#[test]
fn filename_like_ids_never_supply_missing_parentage() {
    let filename_like = cell("patient-01/slide-02.svs::region-3::cell-7");
    assert!(matches!(
        CohortHierarchy::new(
            vec![
                biological(patient("patient-01"), None),
                structural(filename_like.clone(), None),
            ],
            Vec::new(),
        ),
        Err(HierarchyError::MissingParent {
            child,
            parent: None,
        }) if child == filename_like
    ));
}

#[test]
fn project_installs_one_validated_hierarchy_without_overwrite() {
    let patient_a = patient("patient-a");
    let hierarchy = CohortHierarchy::new(vec![biological(patient_a.clone(), None)], Vec::new())
        .expect("hierarchy");
    let replacement =
        CohortHierarchy::new(vec![biological(patient("patient-b"), None)], Vec::new())
            .expect("replacement");
    let mut project = MarklabProject::new();

    project
        .install_hierarchy(hierarchy)
        .expect("first hierarchy install");
    assert!(matches!(
        project.install_hierarchy(replacement),
        Err(ProjectError::HierarchyAlreadyInstalled)
    ));
    let installed = project.hierarchy().expect("installed hierarchy");
    assert!(installed.contains(&patient_a));
    assert_eq!(installed.design_summary().biological_unit_count(), 1);
}

#[test]
fn supported_parent_chain_covers_regular_slide_sections_and_patches() {
    let patient_id = patient("patient");
    let specimen_id = specimen("specimen");
    let block_id = block("block");
    let slide_id = slide("slide");
    let section_id = section("section");
    let region_id = region("region");
    let patch_id: HierarchyId = PatchId::new("patch").expect("patch ID").into();
    let hierarchy = CohortHierarchy::new(
        vec![
            biological(patient_id.clone(), None),
            structural(specimen_id.clone(), Some(patient_id.clone())),
            structural(block_id.clone(), Some(specimen_id)),
            structural(slide_id.clone(), Some(block_id)),
            structural(section_id.clone(), Some(slide_id)),
            structural(region_id.clone(), Some(section_id)),
            structural(patch_id.clone(), Some(region_id)),
        ],
        Vec::new(),
    )
    .expect("regular hierarchy");

    assert_eq!(
        hierarchy.resolved_biological_unit(&patch_id),
        Some(&patient_id)
    );
}
