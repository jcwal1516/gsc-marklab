use std::str::FromStr;

use marklab::{
    ArtifactId, BinaryMarkDeclaration, CellId, ContentDigest, DeclaredScalarInputError,
    DeclaredScalarPatternInput, MeasurementStatus, ProbabilityMarkDeclaration,
    ProbabilityThresholdComparator, ScalarMarkId,
};

#[path = "support/declared_scalar.rs"]
mod support;
use support::*;

const ROW_LIMIT: usize = 4;

fn missing_artifact(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

fn independent_binary(fixture: &mut Fixture, status: MeasurementStatus) -> BinaryMarkDeclaration {
    let provenance = publish_record(
        fixture,
        b"independent-binary-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        binary_metadata("mmr_loss", "MMR loss", status, "independent"),
    );
    BinaryMarkDeclaration::independent(
        ScalarMarkId::new("mmr_loss").expect("mark ID"),
        "MMR loss",
        status,
        provenance,
    )
    .expect("binary declaration")
}

fn probability_declaration(
    fixture: &mut Fixture,
    status: MeasurementStatus,
) -> ProbabilityMarkDeclaration {
    let provenance = publish_record(
        fixture,
        b"probability-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        probability_metadata("mmr_loss_probability", status),
    );
    ProbabilityMarkDeclaration::new(
        ScalarMarkId::new("mmr_loss_probability").expect("probability ID"),
        status,
        provenance,
    )
    .expect("probability declaration")
}

fn thresholded_declarations(
    fixture: &mut Fixture,
    status: MeasurementStatus,
    comparator: ProbabilityThresholdComparator,
    threshold: f32,
) -> (BinaryMarkDeclaration, ProbabilityMarkDeclaration) {
    let probability = probability_declaration(fixture, status);
    let comparator_name = match comparator {
        ProbabilityThresholdComparator::GreaterThan => "greater_than",
        ProbabilityThresholdComparator::GreaterThanOrEqual => "greater_than_or_equal",
    };
    let threshold_provenance = publish_record(
        fixture,
        b"threshold-provenance",
        THRESHOLD_SCHEMA,
        1,
        None,
        vec![probability.provenance_artifact_id()],
        threshold_metadata(
            "mmr_loss",
            probability.mark_id().as_str(),
            comparator_name,
            threshold,
        ),
    );
    let binary_provenance = publish_record(
        fixture,
        b"thresholded-binary-provenance",
        MARK_SCHEMA,
        1,
        None,
        vec![probability.provenance_artifact_id(), threshold_provenance],
        binary_metadata("mmr_loss", "MMR loss", status, "thresholded"),
    );
    let binary = BinaryMarkDeclaration::thresholded(
        ScalarMarkId::new("mmr_loss").expect("mark ID"),
        "MMR loss",
        status,
        binary_provenance,
        probability.mark_id().clone(),
        comparator,
        threshold,
        Some(threshold_provenance),
    )
    .expect("thresholded declaration");
    (binary, probability)
}

fn construct<'a>(
    fixture: &'a Fixture,
    cell_ids: &'a [CellId],
    binary: BinaryMarkDeclaration,
    probability: Option<ProbabilityMarkDeclaration>,
    row_limit: usize,
    text_limit: usize,
) -> Result<DeclaredScalarPatternInput<'a>, DeclaredScalarInputError> {
    DeclaredScalarPatternInput::new(
        &fixture.project,
        &fixture.pattern,
        cell_ids,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
        binary,
        probability,
        row_limit,
        text_limit,
    )
}

#[test]
fn independent_binary_input_binds_rows_frame_status_and_provenance() {
    let mut fixture = fixture();
    let binary = independent_binary(&mut fixture, MeasurementStatus::Measured);
    let text_bytes = cell_id_text_bytes(&fixture.cell_ids);
    let input = construct(
        &fixture,
        &fixture.cell_ids,
        binary.clone(),
        None,
        ROW_LIMIT,
        text_bytes,
    )
    .expect("declared scalar input");

    assert_eq!(input.pattern(), &fixture.pattern);
    assert_eq!(input.cell_ids(), fixture.cell_ids);
    assert_eq!(input.owning_slide_id(), &fixture.slide_id);
    assert_eq!(input.coordinate_frame_id(), &fixture.frame_id);
    assert_eq!(input.binary_mark(), &binary);
    assert_eq!(input.probability_mark(), None);
}

#[test]
fn cell_identity_and_resource_edges_are_strict_and_precede_traversal() {
    let mut fixture = fixture();
    let binary = independent_binary(&mut fixture, MeasurementStatus::Measured);
    let text_bytes = cell_id_text_bytes(&fixture.cell_ids);

    assert!(matches!(
        construct(
            &fixture,
            &fixture.cell_ids,
            binary.clone(),
            None,
            0,
            text_bytes,
        ),
        Err(DeclaredScalarInputError::InvalidResourceLimit)
    ));
    assert!(matches!(
        construct(
            &fixture,
            &fixture.cell_ids,
            binary.clone(),
            None,
            ROW_LIMIT,
            0,
        ),
        Err(DeclaredScalarInputError::InvalidResourceLimit)
    ));

    assert!(matches!(
        construct(
            &fixture,
            &fixture.cell_ids[..3],
            binary.clone(),
            None,
            ROW_LIMIT,
            text_bytes,
        ),
        Err(DeclaredScalarInputError::CellIdCountMismatch {
            expected: 4,
            observed: 3
        })
    ));
    let mut duplicated = fixture.cell_ids.clone();
    duplicated[2] = duplicated[1].clone();
    assert!(matches!(
        construct(
            &fixture,
            &duplicated,
            binary.clone(),
            None,
            ROW_LIMIT,
            text_bytes,
        ),
        Err(DeclaredScalarInputError::NonCanonicalCellIds { row: 2 })
    ));
    let mut reversed = fixture.cell_ids.clone();
    reversed.swap(1, 2);
    assert!(matches!(
        construct(
            &fixture,
            &reversed,
            binary.clone(),
            None,
            ROW_LIMIT,
            text_bytes,
        ),
        Err(DeclaredScalarInputError::NonCanonicalCellIds { row: 2 })
    ));
    let mut foreign = fixture.cell_ids[..3].to_vec();
    foreign.push(CellId::new("foreign-cell").expect("foreign cell"));
    assert!(matches!(
        construct(
            &fixture,
            &foreign,
            binary.clone(),
            None,
            ROW_LIMIT,
            cell_id_text_bytes(&foreign),
        ),
        Err(DeclaredScalarInputError::CellHierarchyMismatch { row: 3 })
    ));
    let mut project_without_hierarchy = marklab::MarklabProject::new();
    project_without_hierarchy
        .install_coordinate_registry(
            fixture
                .project
                .coordinate_registry()
                .expect("fixture registry")
                .clone(),
        )
        .expect("install registry without hierarchy");
    assert!(matches!(
        DeclaredScalarPatternInput::new(
            &project_without_hierarchy,
            &fixture.pattern,
            &fixture.cell_ids,
            fixture.slide_id.clone(),
            fixture.frame_id.clone(),
            binary.clone(),
            None,
            ROW_LIMIT,
            text_bytes,
        ),
        Err(DeclaredScalarInputError::CohortHierarchyMissing)
    ));
    assert!(matches!(
        DeclaredScalarPatternInput::new(
            &fixture.project,
            &fixture.pattern,
            &fixture.cell_ids,
            marklab::SlideId::new("foreign-slide").expect("foreign slide"),
            fixture.frame_id.clone(),
            binary.clone(),
            None,
            ROW_LIMIT,
            text_bytes,
        ),
        Err(DeclaredScalarInputError::OwningSlideMismatch)
    ));
    assert!(matches!(
        construct(
            &fixture,
            &fixture.cell_ids,
            binary.clone(),
            None,
            ROW_LIMIT - 1,
            text_bytes,
        ),
        Err(DeclaredScalarInputError::RowCountExceeded {
            observed: 4,
            maximum: 3
        })
    ));
    assert!(matches!(
        construct(
            &fixture,
            &fixture.cell_ids,
            binary.clone(),
            None,
            ROW_LIMIT,
            text_bytes - 1,
        ),
        Err(DeclaredScalarInputError::CellIdTextBudgetExceeded {
            required,
            maximum
        }) if required == text_bytes && maximum == text_bytes - 1
    ));
    construct(
        &fixture,
        &fixture.cell_ids,
        binary,
        None,
        ROW_LIMIT,
        text_bytes,
    )
    .expect("exact resource edges");
}

#[test]
fn coordinate_frame_must_be_installed_physical_xy_micrometres() {
    for profile in [
        None,
        Some(FrameProfile::PhysicalYxMicrometer),
        Some(FrameProfile::PhysicalXyzMicrometer),
        Some(FrameProfile::PhysicalXyMillimeter),
        Some(FrameProfile::ImageXyPixel),
    ] {
        let mut fixture = fixture_with_frame(profile);
        let binary = independent_binary(&mut fixture, MeasurementStatus::Measured);
        assert!(matches!(
            construct(
                &fixture,
                &fixture.cell_ids,
                binary,
                None,
                ROW_LIMIT,
                cell_id_text_bytes(&fixture.cell_ids),
            ),
            Err(DeclaredScalarInputError::CoordinateRegistryMissing)
                | Err(DeclaredScalarInputError::CoordinateFrameMismatch)
        ));
    }
}

#[test]
fn probability_values_are_dense_declared_finite_and_normalized() {
    let probabilities = [0.1_f32, 0.8, 0.7, 0.2];

    let mut undeclared = fixture();
    undeclared.pattern.mark_prob = Some(probabilities.into());
    let binary = independent_binary(&mut undeclared, MeasurementStatus::Measured);
    assert!(matches!(
        construct(
            &undeclared,
            &undeclared.cell_ids,
            binary,
            None,
            ROW_LIMIT,
            cell_id_text_bytes(&undeclared.cell_ids),
        ),
        Err(DeclaredScalarInputError::ProbabilityDeclarationMismatch)
    ));

    let mut missing = fixture();
    let binary = independent_binary(&mut missing, MeasurementStatus::Measured);
    let probability = probability_declaration(&mut missing, MeasurementStatus::ImportedPrediction);
    assert!(matches!(
        construct(
            &missing,
            &missing.cell_ids,
            binary,
            Some(probability),
            ROW_LIMIT,
            cell_id_text_bytes(&missing.cell_ids),
        ),
        Err(DeclaredScalarInputError::ProbabilityDeclarationMismatch)
    ));

    for values in [
        vec![0.1, 0.8, 0.7],
        vec![0.1, f32::NAN, 0.7, 0.2],
        vec![0.1, 1.01, 0.7, 0.2],
    ] {
        let mut invalid = fixture();
        invalid.pattern.mark_prob = Some(values.into_boxed_slice());
        let binary = independent_binary(&mut invalid, MeasurementStatus::Measured);
        let probability =
            probability_declaration(&mut invalid, MeasurementStatus::ImportedPrediction);
        assert!(matches!(
            construct(
                &invalid,
                &invalid.cell_ids,
                binary,
                Some(probability),
                ROW_LIMIT,
                cell_id_text_bytes(&invalid.cell_ids),
            ),
            Err(DeclaredScalarInputError::ProbabilityLengthMismatch { .. })
                | Err(DeclaredScalarInputError::InvalidProbability { .. })
        ));
    }

    let mut valid = fixture();
    valid.pattern.mark_prob = Some(probabilities.into());
    let binary = independent_binary(&mut valid, MeasurementStatus::Measured);
    let probability = probability_declaration(&mut valid, MeasurementStatus::ImportedPrediction);
    construct(
        &valid,
        &valid.cell_ids,
        binary,
        Some(probability),
        ROW_LIMIT,
        cell_id_text_bytes(&valid.cell_ids),
    )
    .expect("valid dense probability");
}

#[test]
fn required_pattern_columns_are_aligned_finite_and_binary() {
    let mut wrong_length = fixture();
    wrong_length.pattern.valid = vec![1; 3].into_boxed_slice();
    let binary = independent_binary(&mut wrong_length, MeasurementStatus::Measured);
    assert!(matches!(
        construct(
            &wrong_length,
            &wrong_length.cell_ids,
            binary,
            None,
            ROW_LIMIT,
            cell_id_text_bytes(&wrong_length.cell_ids),
        ),
        Err(DeclaredScalarInputError::PatternColumnLengthMismatch {
            column: "valid",
            expected: 4,
            observed: 3,
        })
    ));

    let mut missing_row = fixture();
    missing_row.pattern.valid[3] = 0;
    let binary = independent_binary(&mut missing_row, MeasurementStatus::Measured);
    assert!(matches!(
        construct(
            &missing_row,
            &missing_row.cell_ids,
            binary,
            None,
            ROW_LIMIT,
            cell_id_text_bytes(&missing_row.cell_ids),
        ),
        Err(DeclaredScalarInputError::UnsupportedMissingScalarRow { row: 3 })
    ));

    let mut nonfinite = fixture();
    nonfinite.pattern.x_um[1] = f64::NAN;
    let binary = independent_binary(&mut nonfinite, MeasurementStatus::Measured);
    assert!(matches!(
        construct(
            &nonfinite,
            &nonfinite.cell_ids,
            binary,
            None,
            ROW_LIMIT,
            cell_id_text_bytes(&nonfinite.cell_ids),
        ),
        Err(DeclaredScalarInputError::NonFiniteCoordinate { row: 1 })
    ));

    let mut nonbinary = fixture();
    nonbinary.pattern.mark[2] = 2;
    let binary = independent_binary(&mut nonbinary, MeasurementStatus::Measured);
    assert!(matches!(
        construct(
            &nonbinary,
            &nonbinary.cell_ids,
            binary,
            None,
            ROW_LIMIT,
            cell_id_text_bytes(&nonbinary.cell_ids),
        ),
        Err(DeclaredScalarInputError::InvalidBinaryMark { row: 2 })
    ));
}

#[test]
fn declaration_types_reject_invalid_ids_labels_status_and_thresholds() {
    assert!(matches!(
        ScalarMarkId::new("bad mark id"),
        Err(DeclaredScalarInputError::InvalidMarkId)
    ));
    let id = ScalarMarkId::new("mmr_loss").expect("mark ID");
    assert!(matches!(
        BinaryMarkDeclaration::independent(
            id.clone(),
            "  ",
            MeasurementStatus::Measured,
            missing_artifact(b"label-provenance"),
        ),
        Err(DeclaredScalarInputError::InvalidMarkLabel)
    ));
    assert!(matches!(
        BinaryMarkDeclaration::independent(
            id.clone(),
            "MMR loss",
            MeasurementStatus::DerivedSummary,
            missing_artifact(b"derived-provenance"),
        ),
        Err(DeclaredScalarInputError::UnsupportedPerCellMeasurementStatus)
    ));
    assert!(matches!(
        ProbabilityMarkDeclaration::new(
            ScalarMarkId::new("probability").expect("probability ID"),
            MeasurementStatus::DerivedSummary,
            missing_artifact(b"derived-probability"),
        ),
        Err(DeclaredScalarInputError::UnsupportedPerCellMeasurementStatus)
    ));
    for threshold in [f32::NAN, -0.01, 1.01] {
        assert!(matches!(
            BinaryMarkDeclaration::thresholded(
                id.clone(),
                "MMR loss",
                MeasurementStatus::ImportedPrediction,
                missing_artifact(b"binary-provenance"),
                ScalarMarkId::new("probability").expect("probability ID"),
                ProbabilityThresholdComparator::GreaterThanOrEqual,
                threshold,
                Some(missing_artifact(b"threshold-provenance")),
            ),
            Err(DeclaredScalarInputError::InvalidThreshold)
        ));
    }
    assert!(matches!(
        BinaryMarkDeclaration::thresholded(
            id,
            "MMR loss",
            MeasurementStatus::ImportedPrediction,
            missing_artifact(b"binary-provenance"),
            ScalarMarkId::new("probability").expect("probability ID"),
            ProbabilityThresholdComparator::GreaterThanOrEqual,
            0.5,
            None,
        ),
        Err(DeclaredScalarInputError::ThresholdProvenanceMissing)
    ));
}

#[test]
fn scalar_provenance_records_require_exact_schema_metadata_and_no_table() {
    for (schema, version, table, metadata) in [
        (
            "marklab.wrong_scalar_provenance",
            1,
            None,
            binary_metadata(
                "mmr_loss",
                "MMR loss",
                MeasurementStatus::Measured,
                "independent",
            ),
        ),
        (
            MARK_SCHEMA,
            2,
            None,
            binary_metadata(
                "mmr_loss",
                "MMR loss",
                MeasurementStatus::Measured,
                "independent",
            ),
        ),
        (
            MARK_SCHEMA,
            1,
            Some(one_column_table()),
            binary_metadata(
                "mmr_loss",
                "MMR loss",
                MeasurementStatus::Measured,
                "independent",
            ),
        ),
        (
            MARK_SCHEMA,
            1,
            None,
            binary_metadata(
                "different_mark",
                "MMR loss",
                MeasurementStatus::Measured,
                "independent",
            ),
        ),
    ] {
        let mut fixture = fixture();
        let provenance = publish_record(
            &mut fixture,
            b"drifted-binary-provenance",
            schema,
            version,
            table,
            Vec::new(),
            metadata,
        );
        let binary = BinaryMarkDeclaration::independent(
            ScalarMarkId::new("mmr_loss").expect("mark ID"),
            "MMR loss",
            MeasurementStatus::Measured,
            provenance,
        )
        .expect("shape-valid declaration");
        assert!(matches!(
            construct(
                &fixture,
                &fixture.cell_ids,
                binary,
                None,
                ROW_LIMIT,
                cell_id_text_bytes(&fixture.cell_ids),
            ),
            Err(DeclaredScalarInputError::ProvenanceProfileMismatch { artifact })
                if artifact == provenance
        ));
    }

    let missing = fixture();
    let provenance = missing_artifact(b"missing-scalar-provenance");
    let binary = BinaryMarkDeclaration::independent(
        ScalarMarkId::new("mmr_loss").expect("mark ID"),
        "MMR loss",
        MeasurementStatus::Measured,
        provenance,
    )
    .expect("shape-valid declaration");
    assert!(matches!(
        construct(
            &missing,
            &missing.cell_ids,
            binary,
            None,
            ROW_LIMIT,
            cell_id_text_bytes(&missing.cell_ids),
        ),
        Err(DeclaredScalarInputError::ProvenanceRecordMissing { artifact })
            if artifact == provenance
    ));
}

#[test]
fn thresholded_binary_requires_exact_source_status_dependencies_and_rows() {
    let mut valid = fixture();
    valid.pattern.mark_prob = Some(vec![0.1, 0.8, 0.7, 0.2].into_boxed_slice());
    let (binary, probability) = thresholded_declarations(
        &mut valid,
        MeasurementStatus::ImportedPrediction,
        ProbabilityThresholdComparator::GreaterThanOrEqual,
        0.5,
    );
    let exact = construct(
        &valid,
        &valid.cell_ids,
        binary.clone(),
        Some(probability.clone()),
        ROW_LIMIT,
        cell_id_text_bytes(&valid.cell_ids),
    )
    .expect("valid threshold binding");

    let mut wrong_rows = fixture();
    wrong_rows.pattern.mark_prob = Some(vec![0.1, 0.2, 0.7, 0.2].into_boxed_slice());
    let (binary, probability) = thresholded_declarations(
        &mut wrong_rows,
        MeasurementStatus::ImportedPrediction,
        ProbabilityThresholdComparator::GreaterThanOrEqual,
        0.5,
    );
    assert!(matches!(
        construct(
            &wrong_rows,
            &wrong_rows.cell_ids,
            binary,
            Some(probability),
            ROW_LIMIT,
            cell_id_text_bytes(&wrong_rows.cell_ids),
        ),
        Err(DeclaredScalarInputError::ThresholdBindingMismatch { row: 1 })
    ));

    let mut status_drift = fixture();
    status_drift.pattern.mark_prob = Some(vec![0.1, 0.8, 0.7, 0.2].into_boxed_slice());
    let (binary, _) = thresholded_declarations(
        &mut status_drift,
        MeasurementStatus::ImportedPrediction,
        ProbabilityThresholdComparator::GreaterThanOrEqual,
        0.5,
    );
    let probability = probability_declaration(&mut status_drift, MeasurementStatus::Measured);
    assert!(matches!(
        construct(
            &status_drift,
            &status_drift.cell_ids,
            binary,
            Some(probability),
            ROW_LIMIT,
            cell_id_text_bytes(&status_drift.cell_ids),
        ),
        Err(DeclaredScalarInputError::ThresholdSourceMismatch)
    ));

    let aliased_probability = ProbabilityMarkDeclaration::new(
        ScalarMarkId::new("mmr_loss_probability").expect("probability ID"),
        MeasurementStatus::ImportedPrediction,
        exact.binary_mark().provenance_artifact_id(),
    )
    .expect("aliased declaration");
    assert!(matches!(
        construct(
            &valid,
            &valid.cell_ids,
            exact.binary_mark().clone(),
            Some(aliased_probability),
            ROW_LIMIT,
            cell_id_text_bytes(&valid.cell_ids),
        ),
        Err(DeclaredScalarInputError::DuplicateArtifactRole)
    ));
}

#[test]
fn threshold_provenance_dependencies_are_exact_for_both_records() {
    for drift_binary_dependencies in [true, false] {
        let mut fixture = fixture();
        fixture.pattern.mark_prob = Some(vec![0.1, 0.8, 0.7, 0.2].into_boxed_slice());
        let status = MeasurementStatus::ImportedPrediction;
        let probability = probability_declaration(&mut fixture, status);
        let probability_provenance = probability.provenance_artifact_id();
        let threshold = 0.5_f32;
        let threshold_provenance = publish_record(
            &mut fixture,
            b"dependency-drift-threshold",
            THRESHOLD_SCHEMA,
            1,
            None,
            if drift_binary_dependencies {
                vec![probability_provenance]
            } else {
                Vec::new()
            },
            threshold_metadata(
                "mmr_loss",
                probability.mark_id().as_str(),
                "greater_than_or_equal",
                threshold,
            ),
        );
        let binary_provenance = publish_record(
            &mut fixture,
            b"dependency-drift-binary",
            MARK_SCHEMA,
            1,
            None,
            if drift_binary_dependencies {
                vec![probability_provenance]
            } else {
                vec![probability_provenance, threshold_provenance]
            },
            binary_metadata("mmr_loss", "MMR loss", status, "thresholded"),
        );
        let binary = BinaryMarkDeclaration::thresholded(
            ScalarMarkId::new("mmr_loss").expect("binary ID"),
            "MMR loss",
            status,
            binary_provenance,
            probability.mark_id().clone(),
            ProbabilityThresholdComparator::GreaterThanOrEqual,
            threshold,
            Some(threshold_provenance),
        )
        .expect("shape-valid threshold declaration");
        let expected_artifact = if drift_binary_dependencies {
            binary_provenance
        } else {
            threshold_provenance
        };

        assert!(matches!(
            construct(
                &fixture,
                &fixture.cell_ids,
                binary,
                Some(probability),
                ROW_LIMIT,
                cell_id_text_bytes(&fixture.cell_ids),
            ),
            Err(DeclaredScalarInputError::ProvenanceProfileMismatch { artifact })
                if artifact == expected_artifact
        ));
    }
}
