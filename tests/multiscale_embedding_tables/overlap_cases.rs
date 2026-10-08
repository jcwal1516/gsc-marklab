use std::collections::BTreeSet;

use super::support::*;

struct Fixture {
    hierarchy: CohortHierarchy,
    slide: SlideId,
    expected: ExpectedPatchSet,
    context: PatchEmbeddingContext,
    footprints: PatchFootprintSet,
}

fn fixture(origins: &[[i64; 2]], boundary_policy: PatchBoundaryPolicy) -> Fixture {
    let patient = HierarchyId::from(PatientId::new("overlap-case-patient").expect("patient"));
    let slide = SlideId::new("overlap-case-slide").expect("slide");
    let slide_node = HierarchyId::from(slide.clone());
    let ids = (0..origins.len())
        .map(|index| patch(&format!("case-patch-{index:03}")))
        .collect::<Vec<_>>();
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
    nodes.extend(ids.iter().cloned().map(|id| {
        HierarchyNode::new(
            HierarchyId::from(id),
            Some(slide_node.clone()),
            ReplicationRole::Structural,
        )
    }));
    let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("case hierarchy");
    let expected = ExpectedPatchSet::new(
        &hierarchy,
        slide.clone(),
        "overlap_case_fixture.v1",
        ids,
        1 << 20,
    )
    .expect("case expected patches");
    let (registry, image, physical, transform) = coordinate_registry();
    let context = PatchEmbeddingContext::new(
        &hierarchy,
        &registry,
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
        boundary_policy,
        1 << 20,
    )
    .expect("case context");
    let footprints = PatchFootprintSet::new(
        &hierarchy,
        &expected,
        artifact_id(b"overlap-case-expected"),
        &context,
        artifact_id(b"overlap-case-context"),
        expected
            .ids()
            .iter()
            .cloned()
            .zip(origins.iter().copied())
            .map(|(id, origin)| PatchFootprint::new(id, origin))
            .collect(),
        1 << 20,
    )
    .expect("case footprints");
    Fixture {
        hierarchy,
        slide,
        expected,
        context,
        footprints,
    }
}

fn root(parents: &mut [usize], mut index: usize) -> usize {
    while parents[index] != index {
        index = parents[index];
    }
    index
}

fn brute_force_oracle(
    ids: &[PatchId],
    origins: &[[i64; 2]],
) -> (Vec<(String, String)>, Vec<usize>, usize) {
    let mut parents = (0..origins.len()).collect::<Vec<_>>();
    let mut edges = Vec::new();
    for left in 0..origins.len() {
        for right in left + 1..origins.len() {
            let overlaps = (0..2).all(|axis| {
                origins[left][axis] < origins[right][axis] + 224
                    && origins[right][axis] < origins[left][axis] + 224
            });
            if !overlaps {
                continue;
            }
            edges.push((
                ids[left].as_str().to_owned(),
                ids[right].as_str().to_owned(),
            ));
            let left_root = root(&mut parents, left);
            let right_root = root(&mut parents, right);
            if left_root != right_root {
                let (minimum, maximum) = if left_root < right_root {
                    (left_root, right_root)
                } else {
                    (right_root, left_root)
                };
                parents[maximum] = minimum;
            }
        }
    }
    let component_indices = (0..parents.len())
        .map(|index| root(&mut parents, index))
        .collect::<Vec<_>>();
    let component_count = component_indices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .len();
    (edges, component_indices, component_count)
}

#[test]
fn table_driven_overlap_cases_match_the_checked_brute_force_oracle() {
    struct Case {
        name: &'static str,
        origins: &'static [[i64; 2]],
        boundary_policy: PatchBoundaryPolicy,
        edge_count: usize,
        component_count: usize,
    }

    let cases = [
        Case {
            name: "empty",
            origins: &[],
            boundary_policy: PatchBoundaryPolicy::FullyContainedOnly,
            edge_count: 0,
            component_count: 0,
        },
        Case {
            name: "singleton",
            origins: &[[0, 0]],
            boundary_policy: PatchBoundaryPolicy::FullyContainedOnly,
            edge_count: 0,
            component_count: 1,
        },
        Case {
            name: "identical-origin dense clique",
            origins: &[[0, 0], [0, 0], [0, 0], [0, 0]],
            boundary_policy: PatchBoundaryPolicy::FullyContainedOnly,
            edge_count: 6,
            component_count: 1,
        },
        Case {
            name: "half-open edge and corner contacts",
            origins: &[[0, 0], [224, 0], [0, 224], [224, 224]],
            boundary_policy: PatchBoundaryPolicy::FullyContainedOnly,
            edge_count: 0,
            component_count: 4,
        },
        Case {
            name: "adjacent-bucket nonedge",
            origins: &[[0, 0], [447, 0]],
            boundary_policy: PatchBoundaryPolicy::FullyContainedOnly,
            edge_count: 0,
            component_count: 2,
        },
        Case {
            name: "late bridge merges earlier components",
            origins: &[[0, 0], [400, 0], [200, 0], [800, 544]],
            boundary_policy: PatchBoundaryPolicy::FullyContainedOnly,
            edge_count: 2,
            component_count: 2,
        },
        Case {
            name: "negative Euclidean bucket boundaries",
            origins: &[[-223, 0], [0, 0], [224, 0], [447, 0]],
            boundary_policy: PatchBoundaryPolicy::Reflect,
            edge_count: 2,
            component_count: 2,
        },
    ];

    for case in cases {
        let fixture = fixture(case.origins, case.boundary_policy);
        let graph = PatchOverlapGraph::derive(
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            artifact_id(b"overlap-case-footprints"),
            1 << 20,
            1 << 20,
        )
        .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        let (oracle_edges, component_indices, oracle_component_count) =
            brute_force_oracle(fixture.expected.ids(), case.origins);
        let actual_edges = graph
            .edges()
            .iter()
            .map(|edge| {
                assert!(
                    edge.left_patch_id() < edge.right_patch_id(),
                    "{}",
                    case.name
                );
                (
                    edge.left_patch_id().as_str().to_owned(),
                    edge.right_patch_id().as_str().to_owned(),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(graph.edge_count(), case.edge_count, "{}", case.name);
        assert_eq!(actual_edges, oracle_edges, "{}", case.name);
        assert!(graph.edges().windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            actual_edges.iter().collect::<BTreeSet<_>>().len(),
            actual_edges.len(),
            "{}",
            case.name
        );
        assert_eq!(
            graph.component_count(),
            case.component_count,
            "{}",
            case.name
        );
        assert_eq!(
            graph.component_count(),
            oracle_component_count,
            "{}",
            case.name
        );
        for (index, id) in fixture.expected.ids().iter().enumerate() {
            assert_eq!(
                graph.component_id(id),
                Some(&fixture.expected.ids()[component_indices[index]]),
                "{}",
                case.name
            );
        }
    }
}

#[test]
fn overlap_derivation_rejects_binding_drift_without_exposing_ids() {
    let baseline = fixture(&[[0, 0]], PatchBoundaryPolicy::FullyContainedOnly);
    let drifted_expected = ExpectedPatchSet::new(
        &baseline.hierarchy,
        baseline.slide.clone(),
        "different_selection.v1",
        baseline.expected.ids().to_vec(),
        1 << 20,
    )
    .expect("drifted expected set");
    let error = PatchOverlapGraph::derive(
        &drifted_expected,
        &baseline.context,
        &baseline.footprints,
        artifact_id(b"overlap-case-footprints"),
        1 << 20,
        1 << 20,
    )
    .expect_err("expected binding drift must fail");
    assert_eq!(error, MultiscaleEmbeddingError::OverlapInputMismatch);
    assert!(!error.to_string().contains("case-patch-000"));

    let reflected = fixture(&[[0, 0]], PatchBoundaryPolicy::Reflect);
    let error = PatchOverlapGraph::derive(
        &baseline.expected,
        &reflected.context,
        &baseline.footprints,
        artifact_id(b"overlap-case-footprints"),
        1 << 20,
        1 << 20,
    )
    .expect_err("context binding drift must fail");
    assert_eq!(error, MultiscaleEmbeddingError::OverlapInputMismatch);
    assert!(!format!("{error:?}").contains("case-patch-000"));
}
