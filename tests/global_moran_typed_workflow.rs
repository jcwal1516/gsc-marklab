#![allow(dead_code)]

use approx::assert_abs_diff_eq;
use marklab::{
    global_geary_permutation, global_moran_permutation, BinaryMarkDeclaration,
    DeclaredScalarPatternInput, GlobalGearyAlternative, GlobalGearyDesign, GlobalGearyLimits,
    GlobalMoranAlternative, GlobalMoranDesign, GlobalMoranError, GlobalMoranLimits,
    GlobalMoranWeightPolicy, HistologicCompartmentMarkDeclaration, MarkTable, MeasurementStatus,
    MissingnessPolicy, NucleusAreaUm2MarkDeclaration, ObservationWindow2D, ObservationWindowError,
    ObservationWindowLimits, ScalarMarkColumn, ScalarMarkId, ScalarMarkModality, ScalarMarkUnit,
};

#[path = "support/declared_scalar.rs"]
mod support;
use support::*;

#[test]
fn observation_window_rejects_noncanonical_physical_frame_axes() {
    let fixture = fixture_with_frame(Some(FrameProfile::PhysicalYxMicrometer));
    let window = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[1,0],[1,1],[0,1],[0,0]]]]}"#,
        ObservationWindowLimits::default(),
    )
    .expect("window");
    assert_eq!(
        window.with_coordinate_frame(
            fixture.project.coordinate_registry().expect("registry"),
            fixture.frame_id.clone(),
        ),
        Err(ObservationWindowError::InvalidCoordinateFrame {
            frame: fixture.frame_id,
        })
    );
}

#[test]
fn typed_frame_mark_and_compartment_design_drive_global_moran_inference() {
    let mut fixture = fixture();
    fixture.pattern.nucleus_area_um2 = Some(vec![1.0, 2.0, 8.0, 9.0].into_boxed_slice());
    fixture.pattern.categorical_strata.insert(
        "histologic_compartment".into(),
        vec![0_u32, 0, 1, 1].into_boxed_slice(),
    );
    let binary_provenance = publish_record(
        &mut fixture,
        b"moran-binary-provenance",
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
    let area_provenance = publish_record(
        &mut fixture,
        b"moran-area-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        nucleus_area_um2_metadata(MeasurementStatus::Measured),
    );
    let compartment_provenance = publish_record(
        &mut fixture,
        b"moran-compartment-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        histologic_compartment_metadata(MeasurementStatus::Measured, &["tumor", "stroma"]),
    );
    let binary = BinaryMarkDeclaration::independent(
        ScalarMarkId::new("mmr_loss").expect("binary mark ID"),
        "MMR loss",
        MeasurementStatus::Measured,
        binary_provenance,
    )
    .expect("binary declaration");
    let area = NucleusAreaUm2MarkDeclaration::new(MeasurementStatus::Measured, area_provenance)
        .expect("area declaration");
    let compartment = HistologicCompartmentMarkDeclaration::new(
        vec!["tumor".into(), "stroma".into()],
        MeasurementStatus::Measured,
        compartment_provenance,
    )
    .expect("compartment declaration");
    assert!(matches!(
        ScalarMarkColumn::histologic_compartment(
            compartment.clone(),
            ScalarMarkModality::Histology,
            ScalarMarkUnit::Categorical,
            MissingnessPolicy::NotPermitted,
            vec![0_u32, 0, 1, 2],
        ),
        Err(marklab::DeclaredScalarInputError::InvalidCategoricalValue {
            row: 3,
            code: 2,
            level_count: 2,
        })
    ));
    assert!(matches!(
        ScalarMarkColumn::histologic_compartment(
            compartment.clone(),
            ScalarMarkModality::Morphology,
            ScalarMarkUnit::Categorical,
            MissingnessPolicy::NotPermitted,
            vec![0_u32, 0, 1, 1],
        ),
        Err(marklab::DeclaredScalarInputError::UnitMismatch)
    ));
    let table = MarkTable::new(
        fixture.cell_ids.clone(),
        vec![
            ScalarMarkColumn::binary(
                binary,
                ScalarMarkModality::Immunohistochemistry,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                fixture.pattern.mark.clone(),
            )
            .expect("binary column"),
            ScalarMarkColumn::continuous(
                area,
                ScalarMarkModality::Morphology,
                ScalarMarkUnit::SquareMicrometer,
                MissingnessPolicy::NotPermitted,
                fixture
                    .pattern
                    .nucleus_area_um2
                    .clone()
                    .expect("area values"),
            )
            .expect("area column"),
            ScalarMarkColumn::histologic_compartment(
                compartment,
                ScalarMarkModality::Histology,
                ScalarMarkUnit::Categorical,
                MissingnessPolicy::NotPermitted,
                fixture.pattern.categorical_strata["histologic_compartment"].clone(),
            )
            .expect("compartment column"),
        ],
        4,
        fixture.cell_ids.iter().map(|id| id.as_str().len()).sum(),
    )
    .expect("mark table");
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let window = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
        ObservationWindowLimits::default(),
    )
    .expect("window")
    .with_coordinate_frame(
        fixture.project.coordinate_registry().expect("registry"),
        fixture.frame_id.clone(),
    )
    .expect("framed window");
    let result = global_moran_permutation(
        &input,
        &window,
        &ScalarMarkId::new("nucleus_area_um2").expect("area mark ID"),
        1.1,
        GlobalMoranWeightPolicy::BinarySymmetric,
        &GlobalMoranDesign::histologic_compartment_random_labeling(
            31,
            20260826,
            GlobalMoranAlternative::Greater,
        )
        .expect("design"),
        GlobalMoranLimits::new(4, 6, 6 * 31).expect("limits"),
    )
    .expect("global Moran workflow");

    assert_abs_diff_eq!(result.statistic, 0.4, epsilon = 1e-12);
    assert_abs_diff_eq!(result.null_expectation, -1.0 / 3.0, epsilon = 1e-12);
    assert_eq!(result.directed_edge_count, 6);
    assert_eq!(result.stratum_count, 2);
    assert_eq!(
        result
            .conditioning_mark_id
            .as_ref()
            .expect("conditioning mark")
            .as_str(),
        "histologic_compartment"
    );
    assert_eq!(
        result.conditioning_measurement_status,
        Some(MeasurementStatus::Measured)
    );
    assert_eq!(result.permutations_completed, 31);
    assert_eq!(result.coordinate_frame_id, fixture.frame_id);
    assert_eq!(result.mark_id.as_str(), "nucleus_area_um2");
    assert_eq!(result.measurement_status, MeasurementStatus::Measured);
    assert!(result.p_value > 0.0 && result.p_value <= 1.0);

    let replay = global_moran_permutation(
        &input,
        &window,
        &ScalarMarkId::new("nucleus_area_um2").expect("area mark ID"),
        1.1,
        GlobalMoranWeightPolicy::BinarySymmetric,
        &GlobalMoranDesign::histologic_compartment_random_labeling(
            31,
            20260826,
            GlobalMoranAlternative::Greater,
        )
        .expect("design"),
        GlobalMoranLimits::new(4, 6, 6 * 31).expect("limits"),
    )
    .expect("deterministic replay");
    assert_eq!(result, replay);

    let row_standardized = global_moran_permutation(
        &input,
        &window,
        &ScalarMarkId::new("nucleus_area_um2").expect("area mark ID"),
        1.1,
        GlobalMoranWeightPolicy::RowStandardized,
        &GlobalMoranDesign::histologic_compartment_random_labeling(
            31,
            20260826,
            GlobalMoranAlternative::Greater,
        )
        .expect("design"),
        GlobalMoranLimits::new(4, 6, 6 * 31).expect("limits"),
    )
    .expect("row-standardized workflow");
    assert_abs_diff_eq!(row_standardized.statistic, 0.54, epsilon = 1e-12);

    let geary = global_geary_permutation(
        &input,
        &window,
        &ScalarMarkId::new("nucleus_area_um2").expect("area mark ID"),
        1.1,
        GlobalMoranWeightPolicy::BinarySymmetric,
        &GlobalGearyDesign::histologic_compartment_random_labeling(
            31,
            20260826,
            GlobalGearyAlternative::Less,
        )
        .expect("Geary design"),
        GlobalGearyLimits::new(4, 6, 6 * 31).expect("Geary limits"),
    )
    .expect("global Geary workflow");
    assert_abs_diff_eq!(geary.statistic, 0.38, epsilon = 1e-12);
    assert_eq!(geary.null_expectation, 1.0);
    assert_eq!(geary.weights_digest, result.weights_digest);
    assert_eq!(geary.stratum_count, 2);
    assert_eq!(geary.permutations_completed, 31);
    assert!(geary.p_value > 0.0 && geary.p_value <= 1.0);

    let geary_row_standardized = global_geary_permutation(
        &input,
        &window,
        &ScalarMarkId::new("nucleus_area_um2").expect("area mark ID"),
        1.1,
        GlobalMoranWeightPolicy::RowStandardized,
        &GlobalGearyDesign::histologic_compartment_random_labeling(
            31,
            20260826,
            GlobalGearyAlternative::Less,
        )
        .expect("Geary design"),
        GlobalGearyLimits::new(4, 6, 6 * 31).expect("Geary limits"),
    )
    .expect("row-standardized Geary workflow");
    assert_abs_diff_eq!(geary_row_standardized.statistic, 0.2925, epsilon = 1e-12);

    let unbound = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
        ObservationWindowLimits::default(),
    )
    .expect("unbound window");
    assert_eq!(
        global_moran_permutation(
            &input,
            &unbound,
            &ScalarMarkId::new("nucleus_area_um2").expect("area mark ID"),
            1.1,
            GlobalMoranWeightPolicy::BinarySymmetric,
            &GlobalMoranDesign::random_labeling(7, 1, GlobalMoranAlternative::TwoSided)
                .expect("design"),
            GlobalMoranLimits::new(4, 6, 6 * 7).expect("limits"),
        ),
        Err(GlobalMoranError::UnboundObservationWindow)
    );

    let wrong_frame = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
        ObservationWindowLimits::default(),
    )
    .expect("window")
    .with_coordinate_frame(
        fixture.project.coordinate_registry().expect("registry"),
        fixture.alternate_frame_id.clone(),
    )
    .expect("alternate framed window");
    assert!(matches!(
        global_moran_permutation(
            &input,
            &wrong_frame,
            &ScalarMarkId::new("nucleus_area_um2").expect("area mark ID"),
            1.1,
            GlobalMoranWeightPolicy::BinarySymmetric,
            &GlobalMoranDesign::random_labeling(7, 1, GlobalMoranAlternative::TwoSided)
                .expect("design"),
            GlobalMoranLimits::new(4, 6, 6 * 7).expect("limits"),
        ),
        Err(GlobalMoranError::CoordinateFrameMismatch { .. })
    ));
    assert!(matches!(
        global_moran_permutation(
            &input,
            &window,
            &ScalarMarkId::new("nucleus_area_um2").expect("area mark ID"),
            0.5,
            GlobalMoranWeightPolicy::BinarySymmetric,
            &GlobalMoranDesign::random_labeling(7, 1, GlobalMoranAlternative::TwoSided)
                .expect("design"),
            GlobalMoranLimits::new(4, 6, 6 * 7).expect("limits"),
        ),
        Err(GlobalMoranError::IsolatedPoint { row: 0 })
    ));
    assert_eq!(
        global_moran_permutation(
            &input,
            &window,
            &ScalarMarkId::new("nucleus_area_um2").expect("area mark ID"),
            1.1,
            GlobalMoranWeightPolicy::BinarySymmetric,
            &GlobalMoranDesign::random_labeling(31, 1, GlobalMoranAlternative::TwoSided)
                .expect("design"),
            GlobalMoranLimits::new(4, 6, 6 * 31 - 1).expect("limits"),
        ),
        Err(GlobalMoranError::PermutationWorkExceeded {
            observed: 6 * 31,
            maximum: 6 * 31 - 1,
        })
    );
}
