use marklab_cohort::{
    noninferiority_test, NoninferiorityDirection, NoninferioritySpec, PatientEffect,
};

#[test]
fn both_directions_match_r_4_5_2_student_t_reference() {
    let effects = [-0.1, 0.0, 0.1, 0.0]
        .into_iter()
        .enumerate()
        .map(|(index, effect)| PatientEffect {
            patient_id: format!("p-{}", index + 1),
            effect,
        })
        .collect::<Vec<_>>();
    for (direction, expected_bound) in [
        (
            NoninferiorityDirection::HigherIsBetter,
            -0.096_075_659_909_800_98,
        ),
        (
            NoninferiorityDirection::LowerIsBetter,
            0.096_075_659_909_800_98,
        ),
    ] {
        let result = noninferiority_test(
            &effects,
            &NoninferioritySpec {
                direction,
                margin: 0.2,
                alpha: 0.05,
                margin_rationale: "protocol-ni-margin-v1".into(),
            },
        )
        .expect("noninferiority result");
        assert!((result.standard_error - 0.040_824_829_046_386_304).abs() < 1e-15);
        assert!((result.statistic - 4.898_979_485_566_356).abs() < 1e-14);
        assert!((result.p_value - 0.008_138_301_729_714_28).abs() < 1e-14);
        assert!((result.confidence_bound - expected_bound).abs() < 1e-14);
        assert!(result.noninferior);
    }
}
