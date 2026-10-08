use marklab_cohort::{
    multisite_spatial_inference, noninferiority_test, tost_equivalence, MultisiteEffectModel,
    MultisiteInferenceSpec, NoninferiorityDirection, NoninferioritySpec, PatientEffect, SiteEffect,
    TostEquivalenceSpec,
};

#[test]
fn extreme_upper_tail_probabilities_match_analytical_references() {
    let scale = 3.0_f64.sqrt();
    let effects = [-scale, -scale, scale, scale]
        .into_iter()
        .enumerate()
        .map(|(index, effect)| PatientEffect {
            patient_id: format!("p-{index}"),
            effect,
        })
        .collect::<Vec<_>>();
    let equivalence = tost_equivalence(
        &effects,
        &TostEquivalenceSpec {
            lower_margin: -1_000_000.0,
            upper_margin: 1_000_000.0,
            alpha: 0.05,
            margin_rationale: "extreme-tail-reference".into(),
        },
    )
    .expect("TOST result");
    assert_relative(
        equivalence.p_lower,
        student_t_3_upper_tail(equivalence.t_lower),
        1e-14,
    );

    let noninferiority = noninferiority_test(
        &effects,
        &NoninferioritySpec {
            direction: NoninferiorityDirection::HigherIsBetter,
            margin: 1_000_000.0,
            alpha: 0.05,
            margin_rationale: "extreme-tail-reference".into(),
        },
    )
    .expect("noninferiority result");
    assert_relative(
        noninferiority.p_value,
        student_t_3_upper_tail(noninferiority.statistic),
        1e-14,
    );

    let heterogeneity_scale = 50.0_f64.sqrt();
    let sites = [-heterogeneity_scale, 0.0, heterogeneity_scale]
        .into_iter()
        .enumerate()
        .map(|(index, effect)| SiteEffect {
            site_id: format!("s-{index}"),
            effect,
            standard_error: 1.0,
            patient_count: 10,
        })
        .collect::<Vec<_>>();
    let multisite = multisite_spatial_inference(
        &sites,
        &MultisiteInferenceSpec {
            model: MultisiteEffectModel::FixedEffect,
            alpha: 0.05,
        },
    )
    .expect("multisite result");
    assert_relative(
        multisite.heterogeneity_p_value,
        (-multisite.heterogeneity_q / 2.0).exp(),
        1e-14,
    );
}

fn student_t_3_upper_tail(statistic: f64) -> f64 {
    let y = 3.0_f64.sqrt() / statistic;
    let y_squared = y * y;
    y.powi(3) * (2.0 / 3.0 + y_squared * (-4.0 / 5.0 + y_squared * 6.0 / 7.0))
        / std::f64::consts::PI
}

fn assert_relative(actual: f64, expected: f64, tolerance: f64) {
    let relative_error = ((actual - expected) / expected).abs();
    assert!(
        actual > 0.0 && relative_error < tolerance,
        "actual={actual:e}, expected={expected:e}, relative_error={relative_error:e}"
    );
}
