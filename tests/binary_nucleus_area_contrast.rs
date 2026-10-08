use marklab::{
    declared_binary_group_nucleus_area_contrast, BinaryMarkDeclaration,
    DeclaredBinaryGroupNucleusAreaContrastError, DeclaredBinaryGroupNucleusAreaContrastStatus,
    DeclaredScalarInputError, DeclaredScalarPatternInput, MarklabProject, MeasurementStatus,
    NucleusAreaUm2MarkDeclaration, ProbabilityMarkDeclaration, ScalarMarkId,
};

#[path = "support/declared_scalar.rs"]
#[allow(dead_code)]
mod support;
use support::*;

const ROW_COUNT: usize = 4;

fn binary_declaration(fixture: &mut Fixture) -> BinaryMarkDeclaration {
    let provenance_artifact_id = publish_record(
        fixture,
        b"declared-binary-nucleus-area-contrast-binary",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        binary_metadata(
            "mmr_loss",
            "MMR loss",
            MeasurementStatus::Measured,
            "independent",
        ),
    );
    BinaryMarkDeclaration::independent(
        ScalarMarkId::new("mmr_loss").expect("binary mark ID"),
        "MMR loss",
        MeasurementStatus::Measured,
        provenance_artifact_id,
    )
    .expect("binary declaration")
}

fn probability_declaration(fixture: &mut Fixture) -> ProbabilityMarkDeclaration {
    let provenance_artifact_id = publish_record(
        fixture,
        b"declared-binary-nucleus-area-contrast-probability",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        probability_metadata(
            "mmr_loss_probability",
            MeasurementStatus::ImportedPrediction,
        ),
    );
    ProbabilityMarkDeclaration::new(
        ScalarMarkId::new("mmr_loss_probability").expect("probability mark ID"),
        MeasurementStatus::ImportedPrediction,
        provenance_artifact_id,
    )
    .expect("probability declaration")
}

fn nucleus_area_declaration(fixture: &mut Fixture) -> NucleusAreaUm2MarkDeclaration {
    let provenance_artifact_id = publish_record(
        fixture,
        b"declared-binary-nucleus-area-contrast-nucleus",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        nucleus_area_um2_metadata(MeasurementStatus::Measured),
    );
    NucleusAreaUm2MarkDeclaration::new(MeasurementStatus::Measured, provenance_artifact_id)
        .expect("nucleus-area declaration")
}

fn declared_input<'a>(
    fixture: &'a Fixture,
    binary: BinaryMarkDeclaration,
    probability: Option<ProbabilityMarkDeclaration>,
) -> DeclaredScalarPatternInput<'a> {
    DeclaredScalarPatternInput::new(
        &fixture.project,
        &fixture.pattern,
        &fixture.cell_ids,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
        binary,
        probability,
        16 * 1024,
        cell_id_text_bytes(&fixture.cell_ids),
    )
    .expect("declared scalar input")
}

fn oracle_fixture() -> Fixture {
    let mut fixture = fixture();
    fixture.pattern.mark = vec![0, 0, 1, 1].into_boxed_slice();
    fixture.pattern.nucleus_area_um2 = Some(vec![10.0, 14.0, 20.0, 30.0].into_boxed_slice());
    fixture
}

#[test]
fn binary_nucleus_area_contrast_matches_oracle_identities_and_repeat_bits() {
    let mut fixture = oracle_fixture();
    let binary = binary_declaration(&mut fixture);
    let nucleus_area = nucleus_area_declaration(&mut fixture);
    let input = declared_input(&fixture, binary.clone(), None);
    let result = declared_binary_group_nucleus_area_contrast(
        &fixture.project,
        &input,
        nucleus_area.clone(),
        ROW_COUNT,
    )
    .expect("binary nucleus-area contrast");

    assert_eq!(
        result.status(),
        DeclaredBinaryGroupNucleusAreaContrastStatus::Available
    );
    assert_eq!(result.row_count(), 4);
    assert_eq!(result.marked_count(), 2);
    assert_eq!(result.unmarked_count(), 2);
    assert_eq!(result.marked_mean_nucleus_area_um2(), Some(25.0));
    assert_eq!(result.unmarked_mean_nucleus_area_um2(), Some(12.0));
    assert_eq!(
        result.marked_minus_unmarked_mean_nucleus_area_um2(),
        Some(13.0)
    );
    assert_eq!(result.scalar_identity().row_count(), 4);
    assert_eq!(
        result.scalar_identity().owning_slide_id(),
        &fixture.slide_id
    );
    assert_eq!(
        result.scalar_identity().coordinate_frame_id(),
        &fixture.frame_id
    );
    assert_eq!(result.binary_mark(), &binary);
    assert_eq!(result.probability_mark(), None);
    assert_eq!(result.nucleus_area_mark(), &nucleus_area);

    let repeated = declared_binary_group_nucleus_area_contrast(
        &fixture.project,
        &input,
        nucleus_area,
        ROW_COUNT,
    )
    .expect("repeated contrast");
    assert_eq!(repeated.status(), result.status());
    assert_eq!(
        repeated
            .marked_minus_unmarked_mean_nucleus_area_um2()
            .expect("difference")
            .to_bits(),
        result
            .marked_minus_unmarked_mean_nucleus_area_um2()
            .expect("difference")
            .to_bits()
    );
    assert_eq!(
        repeated.paired_values_logical_digest(),
        result.paired_values_logical_digest()
    );
}

#[test]
fn binary_assignments_change_paired_identity_and_value_but_probabilities_do_not_select_groups() {
    let mut baseline_fixture = oracle_fixture();
    baseline_fixture.pattern.mark_prob = Some(vec![0.01, 0.99, 0.99, 0.01].into_boxed_slice());
    let baseline_binary = binary_declaration(&mut baseline_fixture);
    let baseline_probability = probability_declaration(&mut baseline_fixture);
    let baseline_nucleus = nucleus_area_declaration(&mut baseline_fixture);
    let baseline_input = declared_input(
        &baseline_fixture,
        baseline_binary.clone(),
        Some(baseline_probability.clone()),
    );
    let baseline = declared_binary_group_nucleus_area_contrast(
        &baseline_fixture.project,
        &baseline_input,
        baseline_nucleus,
        ROW_COUNT,
    )
    .expect("baseline contrast");

    let mut swapped_fixture = oracle_fixture();
    swapped_fixture.pattern.mark = vec![0, 1, 0, 1].into_boxed_slice();
    swapped_fixture.pattern.mark_prob = Some(vec![0.01, 0.99, 0.99, 0.01].into_boxed_slice());
    let swapped_binary = binary_declaration(&mut swapped_fixture);
    let swapped_probability = probability_declaration(&mut swapped_fixture);
    let swapped_nucleus = nucleus_area_declaration(&mut swapped_fixture);
    let swapped_input = declared_input(&swapped_fixture, swapped_binary, Some(swapped_probability));
    let swapped = declared_binary_group_nucleus_area_contrast(
        &swapped_fixture.project,
        &swapped_input,
        swapped_nucleus,
        ROW_COUNT,
    )
    .expect("swapped contrast");
    assert_eq!(swapped.marked_count(), baseline.marked_count());
    assert_eq!(swapped.unmarked_count(), baseline.unmarked_count());
    assert_eq!(
        swapped.marked_minus_unmarked_mean_nucleus_area_um2(),
        Some(7.0)
    );
    assert_ne!(
        swapped.paired_values_logical_digest(),
        baseline.paired_values_logical_digest()
    );

    let mut changed_probability_fixture = oracle_fixture();
    changed_probability_fixture.pattern.mark_prob =
        Some(vec![0.95, 0.05, 0.05, 0.95].into_boxed_slice());
    let changed_binary = binary_declaration(&mut changed_probability_fixture);
    let changed_probability = probability_declaration(&mut changed_probability_fixture);
    let changed_nucleus = nucleus_area_declaration(&mut changed_probability_fixture);
    let changed_input = declared_input(
        &changed_probability_fixture,
        changed_binary.clone(),
        Some(changed_probability.clone()),
    );
    let changed = declared_binary_group_nucleus_area_contrast(
        &changed_probability_fixture.project,
        &changed_input,
        changed_nucleus,
        ROW_COUNT,
    )
    .expect("changed-probability contrast");
    assert_eq!(changed.binary_mark(), &baseline_binary);
    assert_eq!(changed.probability_mark(), Some(&baseline_probability));
    assert_eq!(changed.binary_mark(), &changed_binary);
    assert_eq!(changed.probability_mark(), Some(&changed_probability));
    assert_eq!(
        changed.marked_minus_unmarked_mean_nucleus_area_um2(),
        baseline.marked_minus_unmarked_mean_nucleus_area_um2()
    );
    assert_eq!(
        changed.paired_values_logical_digest(),
        baseline.paired_values_logical_digest()
    );
}

#[test]
fn binary_nucleus_area_contrast_canonicalizes_equal_group_means_to_positive_zero() {
    let mut fixture = fixture();
    fixture.pattern.mark = vec![0, 0, 1, 1].into_boxed_slice();
    fixture.pattern.nucleus_area_um2 = Some(vec![10.0, 20.0, 20.0, 10.0].into_boxed_slice());
    let binary = binary_declaration(&mut fixture);
    let nucleus = nucleus_area_declaration(&mut fixture);
    let input = declared_input(&fixture, binary, None);
    let result =
        declared_binary_group_nucleus_area_contrast(&fixture.project, &input, nucleus, ROW_COUNT)
            .expect("equal-means contrast");
    assert_eq!(result.marked_mean_nucleus_area_um2(), Some(15.0));
    assert_eq!(result.unmarked_mean_nucleus_area_um2(), Some(15.0));
    assert_eq!(
        result
            .marked_minus_unmarked_mean_nucleus_area_um2()
            .expect("zero difference")
            .to_bits(),
        0.0_f64.to_bits()
    );
}

#[test]
fn binary_nucleus_area_contrast_types_all_unmarked_and_all_marked_inputs() {
    for marked_value in [0_u8, 1_u8] {
        let mut fixture = fixture();
        fixture.pattern.mark = vec![marked_value; ROW_COUNT].into_boxed_slice();
        fixture.pattern.nucleus_area_um2 = Some(vec![10.0, 14.0, 20.0, 30.0].into_boxed_slice());
        let binary = binary_declaration(&mut fixture);
        let nucleus = nucleus_area_declaration(&mut fixture);
        let input = declared_input(&fixture, binary, None);
        let result = declared_binary_group_nucleus_area_contrast(
            &fixture.project,
            &input,
            nucleus,
            ROW_COUNT,
        )
        .expect("single-group contrast");
        assert_eq!(
            result.status(),
            DeclaredBinaryGroupNucleusAreaContrastStatus::InsufficientGroups
        );
        assert_eq!(result.row_count(), 4);
        assert_eq!(result.marked_count(), if marked_value == 1 { 4 } else { 0 });
        assert_eq!(
            result.unmarked_count(),
            if marked_value == 0 { 4 } else { 0 }
        );
        assert_eq!(
            result.marked_mean_nucleus_area_um2(),
            (marked_value == 1).then_some(18.5)
        );
        assert_eq!(
            result.unmarked_mean_nucleus_area_um2(),
            (marked_value == 0).then_some(18.5)
        );
        assert_eq!(result.marked_minus_unmarked_mean_nucleus_area_um2(), None);
    }
}

#[test]
fn binary_nucleus_area_contrast_rejects_missing_invalid_and_length_drift() {
    let mut missing_fixture = fixture();
    let missing_binary = binary_declaration(&mut missing_fixture);
    let missing_nucleus = nucleus_area_declaration(&mut missing_fixture);
    let missing_input = declared_input(&missing_fixture, missing_binary, None);
    assert!(matches!(
        declared_binary_group_nucleus_area_contrast(
            &missing_fixture.project,
            &missing_input,
            missing_nucleus,
            0,
        ),
        Err(DeclaredBinaryGroupNucleusAreaContrastError::NucleusAreaColumnRequired)
    ));

    for invalid in [f32::NAN, f32::INFINITY, 0.0, -1.0, -0.0] {
        let mut fixture = fixture();
        fixture.pattern.nucleus_area_um2 = Some(vec![10.0, invalid, 20.0, 30.0].into_boxed_slice());
        let binary = binary_declaration(&mut fixture);
        let nucleus = nucleus_area_declaration(&mut fixture);
        let input = declared_input(&fixture, binary, None);
        assert!(matches!(
            declared_binary_group_nucleus_area_contrast(
                &fixture.project,
                &input,
                nucleus,
                ROW_COUNT,
            ),
            Err(DeclaredBinaryGroupNucleusAreaContrastError::InvalidNucleusAreaValue { row: 1 })
        ));
    }

    for (areas, observed) in [(vec![10.0, 20.0, 30.0], 3), (vec![10.0; 5], 5)] {
        let mut fixture = fixture();
        fixture.pattern.nucleus_area_um2 = Some(areas.into_boxed_slice());
        let binary = binary_declaration(&mut fixture);
        let error = match DeclaredScalarPatternInput::new(
            &fixture.project,
            &fixture.pattern,
            &fixture.cell_ids,
            fixture.slide_id.clone(),
            fixture.frame_id.clone(),
            binary,
            None,
            16 * 1024,
            cell_id_text_bytes(&fixture.cell_ids),
        ) {
            Err(error) => error,
            Ok(_) => panic!("declared input accepted a drifted nucleus-area length"),
        };
        assert!(matches!(
            error,
            DeclaredScalarInputError::PatternColumnLengthMismatch {
                column: "nucleus_area_um2",
                expected: 4,
                observed: actual,
            } if actual == observed
        ));
    }

    let _typed_length = DeclaredBinaryGroupNucleusAreaContrastError::NucleusAreaLengthMismatch {
        expected: 4,
        observed: 3,
    };
    let _typed_overflow = DeclaredBinaryGroupNucleusAreaContrastError::SizeOverflow;
}

#[test]
fn binary_nucleus_area_contrast_revalidates_project_provenance_and_limit_precedence() {
    let mut fixture = oracle_fixture();
    let binary = binary_declaration(&mut fixture);
    let nucleus = nucleus_area_declaration(&mut fixture);
    let input = declared_input(&fixture, binary, None);

    assert!(matches!(
        declared_binary_group_nucleus_area_contrast(
            &MarklabProject::new(),
            &input,
            nucleus.clone(),
            0,
        ),
        Err(DeclaredBinaryGroupNucleusAreaContrastError::ScalarInput(
            DeclaredScalarInputError::CoordinateRegistryMissing
        ))
    ));
    assert!(matches!(
        declared_binary_group_nucleus_area_contrast(&fixture.project, &input, nucleus, 3),
        Err(
            DeclaredBinaryGroupNucleusAreaContrastError::RowCountBudgetExceeded {
                required: 4,
                maximum: 3,
            }
        )
    ));

    let mut invalid_fixture = oracle_fixture();
    invalid_fixture
        .pattern
        .nucleus_area_um2
        .as_mut()
        .expect("areas")[0] = f32::NAN;
    let invalid_binary = binary_declaration(&mut invalid_fixture);
    let invalid_nucleus = nucleus_area_declaration(&mut invalid_fixture);
    let invalid_input = declared_input(&invalid_fixture, invalid_binary, None);
    assert!(matches!(
        declared_binary_group_nucleus_area_contrast(
            &invalid_fixture.project,
            &invalid_input,
            invalid_nucleus,
            3,
        ),
        Err(
            DeclaredBinaryGroupNucleusAreaContrastError::RowCountBudgetExceeded {
                required: 4,
                maximum: 3,
            }
        )
    ));

    let mut missing_fixture = oracle_fixture();
    let missing_binary = binary_declaration(&mut missing_fixture);
    let missing_input = declared_input(&missing_fixture, missing_binary, None);
    let mut foreign_fixture = support::fixture();
    let absent_id = publish_record(
        &mut foreign_fixture,
        b"absent-binary-nucleus-area-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        nucleus_area_um2_metadata(MeasurementStatus::Measured),
    );
    let missing_nucleus =
        NucleusAreaUm2MarkDeclaration::new(MeasurementStatus::Measured, absent_id)
            .expect("missing provenance declaration");
    assert!(matches!(
        declared_binary_group_nucleus_area_contrast(
            &missing_fixture.project,
            &missing_input,
            missing_nucleus,
            0,
        ),
        Err(DeclaredBinaryGroupNucleusAreaContrastError::ScalarInput(
            DeclaredScalarInputError::ProvenanceRecordMissing { artifact }
        )) if artifact == absent_id
    ));

    let mut drift_fixture = oracle_fixture();
    let drift_binary = binary_declaration(&mut drift_fixture);
    let mut metadata = nucleus_area_um2_metadata(MeasurementStatus::Measured);
    metadata.insert("unit".into(), "square_millimeter".into());
    let drift_id = publish_record(
        &mut drift_fixture,
        b"drifted-binary-nucleus-area-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        metadata,
    );
    let drift_nucleus = NucleusAreaUm2MarkDeclaration::new(MeasurementStatus::Measured, drift_id)
        .expect("drifted provenance declaration");
    let drift_input = declared_input(&drift_fixture, drift_binary, None);
    assert!(matches!(
        declared_binary_group_nucleus_area_contrast(
            &drift_fixture.project,
            &drift_input,
            drift_nucleus,
            0,
        ),
        Err(DeclaredBinaryGroupNucleusAreaContrastError::ScalarInput(
            DeclaredScalarInputError::ProvenanceProfileMismatch { artifact }
        )) if artifact == drift_id
    ));
}
