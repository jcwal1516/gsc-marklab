use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
};

use super::support::*;

fn four_patch_expected(hierarchy: &CohortHierarchy, slide: &SlideId) -> ExpectedPatchSet {
    ExpectedPatchSet::new(
        hierarchy,
        slide.clone(),
        "all_patches.v1",
        ["patch-a", "patch-b", "patch-c", "patch-d"]
            .map(patch)
            .into(),
        RETAINED_BUDGET,
    )
    .expect("four expected patches")
}

fn footprints(
    hierarchy: &CohortHierarchy,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    origins: &[[i64; 2]],
) -> PatchFootprintSet {
    PatchFootprintSet::new(
        hierarchy,
        expected,
        artifact_id(b"expected-patches"),
        context,
        artifact_id(b"patch-context"),
        expected
            .ids()
            .iter()
            .cloned()
            .zip(origins.iter().copied())
            .map(|(id, origin)| PatchFootprint::new(id, origin))
            .collect(),
        RETAINED_BUDGET,
    )
    .expect("valid footprints")
}

fn frame(writer: &mut impl Write, value: &[u8]) {
    writer
        .write_all(&(value.len() as u128).to_be_bytes())
        .expect("reference length frame");
    writer.write_all(value).expect("reference value frame");
}

fn reference_overlap_digest(graph: &PatchOverlapGraph) -> ContentDigest {
    let mut writer = ContentDigest::builder();
    frame(&mut writer, b"marklab-patch-overlap-graph-logical-v1");
    frame(
        &mut writer,
        graph.expected_patches_artifact_id().digest().as_bytes(),
    );
    frame(
        &mut writer,
        graph.expected_patches_logical_digest().as_bytes(),
    );
    frame(
        &mut writer,
        graph.patch_footprints_artifact_id().digest().as_bytes(),
    );
    frame(
        &mut writer,
        graph.patch_footprints_logical_digest().as_bytes(),
    );
    frame(
        &mut writer,
        &u64::try_from(graph.edge_count())
            .expect("bounded edge count")
            .to_be_bytes(),
    );
    for edge in graph.edges() {
        frame(&mut writer, edge.left_patch_id().as_str().as_bytes());
        frame(&mut writer, edge.right_patch_id().as_str().as_bytes());
    }
    writer.finish().0
}

#[test]
fn overlap_graph_uses_positive_area_and_minimum_patch_components() {
    let (hierarchy, slide) = hierarchy();
    let (registry, image, physical, transform) = coordinate_registry();
    let context = patch_context_with_budget(
        &hierarchy,
        &slide,
        &registry,
        image,
        physical,
        transform,
        RETAINED_BUDGET,
    )
    .expect("context");
    let expected = four_patch_expected(&hierarchy, &slide);
    let footprints = footprints(
        &hierarchy,
        &expected,
        &context,
        &[[0, 0], [200, 0], [424, 0], [800, 544]],
    );

    let graph = PatchOverlapGraph::derive(
        &expected,
        &context,
        &footprints,
        artifact_id(b"patch-footprints"),
        RETAINED_BUDGET,
        RETAINED_BUDGET,
    )
    .expect("overlap graph");

    assert_eq!(graph.edge_count(), 1);
    assert_eq!(graph.component_count(), 3);
    assert_eq!(graph.edges()[0].left_patch_id(), &patch("patch-a"));
    assert_eq!(graph.edges()[0].right_patch_id(), &patch("patch-b"));
    assert_eq!(
        graph.component_id(&patch("patch-a")),
        Some(&patch("patch-a"))
    );
    assert_eq!(
        graph.component_id(&patch("patch-b")),
        Some(&patch("patch-a"))
    );
    assert_eq!(
        graph.component_id(&patch("patch-c")),
        Some(&patch("patch-c"))
    );
    assert_eq!(
        graph.component_id(&patch("patch-d")),
        Some(&patch("patch-d"))
    );
    assert_eq!(graph.component_id(&patch("absent")), None);
    let graph_debug = format!("{graph:?}");
    let edge_debug = format!("{:?}", &graph.edges()[0]);
    assert!(!graph_debug.contains("patch-a"));
    assert!(!edge_debug.contains("patch-a"));
    assert_eq!(graph.logical_digest(), reference_overlap_digest(&graph));
    assert_eq!(
        graph.logical_digest().to_string(),
        "b512cee465515aa5a077c0fc2f8d87869de8cdcbbb398182ed67fc0f5139d1f4"
    );

    let patch_text = expected
        .ids()
        .iter()
        .map(|id| id.as_str().len())
        .sum::<usize>();
    let edge_text = graph
        .edges()
        .iter()
        .map(|edge| edge.left_patch_id().as_str().len() + edge.right_patch_id().as_str().len())
        .sum::<usize>();
    let retained = size_of::<PatchOverlapGraph>()
        + expected.ids().len() * (size_of::<PatchId>() + size_of::<usize>())
        + patch_text
        + graph.edge_count() * size_of::<marklab::PatchOverlapEdge>()
        + edge_text;
    let scratch = expected.ids().len() * (size_of::<([i64; 2], usize)>() + size_of::<usize>());
    let materialized_peak = scratch
        + graph.edge_count() * size_of::<marklab::PatchOverlapEdge>()
        + edge_text
        + expected.ids().len() * size_of::<usize>();
    let working = retained.max(materialized_peak);
    PatchOverlapGraph::derive(
        &expected,
        &context,
        &footprints,
        artifact_id(b"patch-footprints"),
        retained,
        working,
    )
    .expect("independently computed exact overlap budgets");
    assert!(matches!(
        PatchOverlapGraph::derive(
            &expected,
            &context,
            &footprints,
            artifact_id(b"patch-footprints"),
            retained - 1,
            working,
        ),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, maximum })
            if required == retained && maximum == retained - 1
    ));
    assert!(matches!(
        PatchOverlapGraph::derive(
            &expected,
            &context,
            &footprints,
            artifact_id(b"patch-footprints"),
            retained,
            working - 1,
        ),
        Err(MultiscaleEmbeddingError::WorkingByteBudgetExceeded { required, maximum })
            if required == working && maximum == working - 1
    ));

    let (registry, image, physical, transform) = coordinate_registry();
    let reflected_context = PatchEmbeddingContext::new(
        &hierarchy,
        &registry,
        slide,
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
        PatchBoundaryPolicy::Reflect,
        RETAINED_BUDGET,
    )
    .expect("reflected context");
    let reflected_footprints = self::footprints(
        &hierarchy,
        &expected,
        &reflected_context,
        &[[-100, 0], [100, 0], [324, 0], [800, 544]],
    );
    let reflected = PatchOverlapGraph::derive(
        &expected,
        &reflected_context,
        &reflected_footprints,
        artifact_id(b"reflected-footprints"),
        RETAINED_BUDGET,
        RETAINED_BUDGET,
    )
    .expect("negative-origin Euclidean buckets");
    assert_eq!(reflected.edge_count(), 1);
    assert_eq!(reflected.edges()[0].left_patch_id(), &patch("patch-a"));
    assert_eq!(reflected.edges()[0].right_patch_id(), &patch("patch-b"));
}

#[test]
fn indexed_overlap_matches_brute_force_and_enforces_resource_budgets() {
    let patient = HierarchyId::from(PatientId::new("overlap-patient").expect("patient"));
    let slide = SlideId::new("overlap-slide").expect("slide");
    let slide_node = HierarchyId::from(slide.clone());
    let ids = (0..64)
        .map(|index| patch(&format!("patch-{index:03}")))
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
    let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("overlap hierarchy");
    let expected = ExpectedPatchSet::new(
        &hierarchy,
        slide.clone(),
        "deterministic_overlap_fixture.v1",
        ids,
        RETAINED_BUDGET,
    )
    .expect("expected patches");
    let (registry, image, physical, transform) = coordinate_registry();
    let context = patch_context_with_budget(
        &hierarchy,
        &slide,
        &registry,
        image,
        physical,
        transform,
        RETAINED_BUDGET,
    )
    .expect("context");

    let mut state = 0x4d59_5df4_d0f3_3173_u64;
    let origins = (0..expected.ids().len())
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let x = i64::try_from((state >> 16) % 801).expect("bounded x");
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let y = i64::try_from((state >> 16) % 545).expect("bounded y");
            [x, y]
        })
        .collect::<Vec<_>>();
    let footprints = footprints(&hierarchy, &expected, &context, &origins);

    let graph = PatchOverlapGraph::derive(
        &expected,
        &context,
        &footprints,
        artifact_id(b"random-footprints"),
        1 << 20,
        1 << 20,
    )
    .expect("indexed graph");

    let mut oracle_edges = BTreeSet::new();
    for left in 0..origins.len() {
        for right in left + 1..origins.len() {
            if origins[left][0] < origins[right][0] + 224
                && origins[right][0] < origins[left][0] + 224
                && origins[left][1] < origins[right][1] + 224
                && origins[right][1] < origins[left][1] + 224
            {
                oracle_edges.insert((
                    expected.ids()[left].as_str().to_owned(),
                    expected.ids()[right].as_str().to_owned(),
                ));
            }
        }
    }
    let indexed_edges = graph
        .edges()
        .iter()
        .map(|edge| {
            (
                edge.left_patch_id().as_str().to_owned(),
                edge.right_patch_id().as_str().to_owned(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(indexed_edges, oracle_edges);

    let mut component = expected
        .ids()
        .iter()
        .map(|id| (id.as_str().to_owned(), id.as_str().to_owned()))
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for (left, right) in &oracle_edges {
            let left_component = component[left].clone();
            let right_component = component[right].clone();
            let minimum = left_component.clone().min(right_component.clone());
            for value in component.values_mut() {
                if (*value == left_component || *value == right_component) && *value != minimum {
                    *value = minimum.clone();
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    for id in expected.ids() {
        assert_eq!(
            graph.component_id(id).map(PatchId::as_str),
            Some(component[id.as_str()].as_str())
        );
    }

    assert!(matches!(
        PatchOverlapGraph::derive(
            &expected,
            &context,
            &footprints,
            artifact_id(b"random-footprints"),
            1 << 20,
            0,
        ),
        Err(MultiscaleEmbeddingError::WorkingByteBudgetExceeded { .. })
    ));
    assert!(matches!(
        PatchOverlapGraph::derive(
            &expected,
            &context,
            &footprints,
            artifact_id(b"random-footprints"),
            0,
            1 << 20,
        ),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { .. })
    ));
}
