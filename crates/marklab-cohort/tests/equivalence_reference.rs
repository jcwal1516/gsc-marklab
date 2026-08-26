use marklab_cohort::{tost_equivalence, PatientEffect, TostEquivalenceSpec};

#[test]
fn tost_matches_r_4_5_2_student_t_reference() {
    let effects = [-0.1, 0.0, 0.1, 0.0]
        .into_iter()
        .enumerate()
        .map(|(index, effect)| PatientEffect {
            patient_id: format!("p-{index}"),
            effect,
        })
        .collect::<Vec<_>>();
    let result = tost_equivalence(
        &effects,
        &TostEquivalenceSpec {
            lower_margin: -0.2,
            upper_margin: 0.2,
            alpha: 0.05,
            margin_rationale: "protocol-margin-v1".into(),
        },
    )
    .expect("TOST result");

    assert!((result.standard_error - 0.040_824_829_046_386_304).abs() < 1e-15);
    assert!((result.t_lower - 4.898_979_485_566_356).abs() < 1e-14);
    assert!((result.t_upper + 4.898_979_485_566_356).abs() < 1e-14);
    assert!((result.p_lower - 0.008_138_301_729_714_28).abs() < 1e-14);
    assert!((result.p_upper - 0.008_138_301_729_714_28).abs() < 1e-14);
    assert!((result.confidence_interval.lower + 0.096_075_659_909_800_98).abs() < 1e-14);
    assert!((result.confidence_interval.upper - 0.096_075_659_909_800_98).abs() < 1e-14);
    assert!(result.equivalent);
}
