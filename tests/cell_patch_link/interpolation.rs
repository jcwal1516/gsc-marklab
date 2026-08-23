use super::support::*;

fn contributor(patch_id: &str, numerator: u64, denominator: u64) -> CellPatchContributor {
    CellPatchContributor::new(patch(patch_id), numerator, denominator)
        .expect("positive contributor")
}

fn declaration(
    cell_id: &str,
    xy: [f64; 2],
    contributors: Vec<CellPatchContributor>,
) -> DeclaredCellPatchAssignment {
    DeclaredCellPatchAssignment::new(anchor(cell_id, xy), contributors)
}

fn valid_declarations() -> Vec<DeclaredCellPatchAssignment> {
    vec![
        declaration(
            "cell-a",
            [60.0, 10.0],
            vec![
                contributor("patch-a", 2, 4),
                contributor("patch-b", 1, 4),
                contributor("patch-c", 1, 4),
            ],
        ),
        declaration("cell-b", [-0.0, 0.0], Vec::new()),
        declaration("cell-c", [224.0, 10.0], vec![contributor("patch-c", 1, 1)]),
        declaration(
            "cell-d",
            [900.0, 700.0],
            vec![contributor("patch-a", 2, 3), contributor("patch-b", 1, 3)],
        ),
    ]
}

fn overallocated_declarations() -> (Vec<DeclaredCellPatchAssignment>, usize) {
    let mut first = Vec::with_capacity(8);
    first.extend([
        contributor("patch-a", 2, 4),
        contributor("patch-b", 1, 4),
        contributor("patch-c", 1, 4),
    ]);
    let mut declarations = Vec::with_capacity(8);
    declarations.extend([
        declaration("cell-a", [60.0, 10.0], first),
        declaration("cell-b", [0.0, 0.0], Vec::new()),
        declaration("cell-c", [224.0, 10.0], vec![contributor("patch-c", 1, 1)]),
        declaration(
            "cell-d",
            [900.0, 700.0],
            vec![contributor("patch-a", 2, 3), contributor("patch-b", 1, 3)],
        ),
    ]);
    let cell_text = declarations
        .iter()
        .map(|declaration| declaration.anchor().cell_id().as_str().len())
        .sum::<usize>();
    let patch_text = declarations
        .iter()
        .flat_map(DeclaredCellPatchAssignment::contributors)
        .map(|contributor| contributor.patch_id().as_str().len())
        .sum::<usize>();
    let input_bytes = declarations.capacity() * size_of::<DeclaredCellPatchAssignment>()
        + 11 * size_of::<CellPatchContributor>()
        + cell_text
        + patch_text;
    (declarations, input_bytes)
}

#[test]
fn declared_interpolation_preserves_canonical_group_weights_and_zero_states() {
    let fixture = fixture();
    let link = CellPatchLink::from_declared_weighted_interpolation(
        &fixture.hierarchy,
        &fixture.expected_cells,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        valid_declarations(),
        BUDGET,
        BUDGET,
    )
    .expect("declared interpolation link");

    assert_eq!(
        link.mode(),
        CellPatchAssignmentMode::DeclaredWeightedInterpolation
    );
    assert_eq!(link.assignment_count(), 4);
    assert_eq!(link.edge_count(), 6);
    assert_eq!(
        link.assignments()[1].status(),
        CellPatchAssignmentStatus::InterpolationUnavailable
    );
    assert_eq!(link.assignments()[1].edge_count(), 0);
    let first = link
        .edges_for_assignment(0)
        .expect("first interpolation group");
    assert_eq!(first.len(), 3);
    assert_eq!(first[0].weight().expect("weight").numerator(), 2);
    assert_eq!(first[0].weight().expect("weight").denominator(), 4);
    assert_eq!(first[1].weight().expect("weight").numerator(), 1);
    assert_eq!(first[2].weight().expect("weight").numerator(), 1);
    assert_eq!(link.assignments()[1].anchor_px()[0].to_bits(), 0);
    assert!(!format!("{:?}", first[0].weight().expect("weight")).contains("numerator"));
    assert_eq!(link.logical_digest(), reference_link_digest(&link));
    assert_eq!(
        link.logical_digest().to_string(),
        "c7317ffb9ec145963dd2d311eec5ea97a7f4254176226a9d6b61f903d8d4bb06"
    );
}

#[test]
fn declared_interpolation_rejects_noncanonical_or_nonunit_groups() {
    let fixture = fixture();
    let cases = [
        vec![contributor("patch-a", 2, 4), contributor("patch-b", 2, 4)],
        vec![contributor("patch-a", 1, 2), contributor("patch-b", 1, 3)],
        vec![contributor("patch-a", 1, 4), contributor("patch-b", 1, 4)],
        vec![contributor("patch-b", 1, 2), contributor("patch-a", 1, 2)],
        vec![contributor("patch-a", 1, 2), contributor("patch-a", 1, 2)],
        vec![contributor("patch-z", 1, 1)],
        vec![
            contributor("patch-a", 1, 5),
            contributor("patch-b", 1, 5),
            contributor("patch-c", 1, 5),
            contributor("patch-a", 1, 5),
            contributor("patch-b", 1, 5),
        ],
    ];
    for invalid in cases {
        let result = CellPatchLink::from_declared_weighted_interpolation(
            &fixture.hierarchy,
            &fixture.expected_cells,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            &fixture.bindings,
            vec![
                declaration("cell-a", [60.0, 10.0], invalid),
                declaration("cell-b", [0.0, 0.0], Vec::new()),
                declaration("cell-c", [224.0, 10.0], Vec::new()),
                declaration("cell-d", [900.0, 700.0], Vec::new()),
            ],
            BUDGET,
            BUDGET,
        );
        assert!(matches!(
            result,
            Err(MultiscaleEmbeddingError::InvalidCellPatchContributors { row: 0 })
        ));
    }

    assert!(CellPatchContributor::new(patch("patch-a"), 0, 1).is_err());
    assert!(CellPatchContributor::new(patch("patch-a"), 1, 0).is_err());
}

#[test]
fn declared_interpolation_charges_nested_input_and_output_exactly() {
    let fixture = fixture();
    let declarations = valid_declarations();
    let measured = CellPatchLink::from_declared_weighted_interpolation(
        &fixture.hierarchy,
        &fixture.expected_cells,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        declarations.clone(),
        BUDGET,
        BUDGET,
    )
    .expect("measured interpolation link");
    let input_bytes = declarations.capacity() * size_of::<DeclaredCellPatchAssignment>()
        + declarations
            .iter()
            .map(|declaration| {
                declaration.anchor().cell_id().as_str().len()
                    + std::mem::size_of_val(declaration.contributors())
                    + declaration
                        .contributors()
                        .iter()
                        .map(|contributor| contributor.patch_id().as_str().len())
                        .sum::<usize>()
            })
            .sum::<usize>();
    let retained = size_of::<CellPatchLink>()
        + measured.owning_slide_id().as_str().len()
        + measured.assignment_count() * size_of::<CellPatchAssignment>()
        + measured
            .assignments()
            .iter()
            .map(|assignment| assignment.cell_id().as_str().len())
            .sum::<usize>()
        + measured.edge_count() * size_of::<CellPatchEdge>()
        + measured
            .edges()
            .iter()
            .map(|edge| edge.patch_id().as_str().len())
            .sum::<usize>();
    let working = input_bytes + retained;
    CellPatchLink::from_declared_weighted_interpolation(
        &fixture.hierarchy,
        &fixture.expected_cells,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        declarations.clone(),
        retained,
        working,
    )
    .expect("independently computed exact interpolation budgets");
    assert!(matches!(
        CellPatchLink::from_declared_weighted_interpolation(
            &fixture.hierarchy,
            &fixture.expected_cells,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            &fixture.bindings,
            declarations.clone(),
            retained - 1,
            working,
        ),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, maximum })
            if required == retained && maximum == retained - 1
    ));
    assert!(matches!(
        CellPatchLink::from_declared_weighted_interpolation(
            &fixture.hierarchy,
            &fixture.expected_cells,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            &fixture.bindings,
            declarations,
            retained,
            working - 1,
        ),
        Err(MultiscaleEmbeddingError::WorkingByteBudgetExceeded { required, maximum })
            if required == working && maximum == working - 1
    ));

    let (_, overallocated_input) = overallocated_declarations();
    let overallocated_working = overallocated_input + retained;
    CellPatchLink::from_declared_weighted_interpolation(
        &fixture.hierarchy,
        &fixture.expected_cells,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        overallocated_declarations().0,
        retained,
        overallocated_working,
    )
    .expect("outer and nested caller capacities are charged exactly");
    assert!(matches!(
        CellPatchLink::from_declared_weighted_interpolation(
            &fixture.hierarchy,
            &fixture.expected_cells,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            &fixture.bindings,
            overallocated_declarations().0,
            retained,
            overallocated_working - 1,
        ),
        Err(MultiscaleEmbeddingError::WorkingByteBudgetExceeded { required, maximum })
            if required == overallocated_working && maximum == overallocated_working - 1
    ));
}
