use marklab_numerics::{minimize_lbfgs, NumericsError};

#[test]
fn bounded_lbfgs_converges_and_propagates_caller_errors() {
    let targets = [-2.0, -1.0, 0.5, 3.0, 8.0, 13.0];
    let scales = [0.2, 0.5, 1.0, 2.0, 5.0, 20.0];
    let fitted = minimize_lbfgs(
        &[0.0; 6],
        2_000,
        1e-9,
        1e-14,
        10,
        50,
        |parameters, gradient| {
            let mut value = 0.0;
            for index in 0..parameters.len() {
                let difference = parameters[index] - targets[index];
                value += 0.5 * scales[index] * difference * difference;
                gradient[index] = scales[index] * difference;
            }
            Ok(value)
        },
    )
    .unwrap();
    for (actual, expected) in fitted.iter().zip(targets) {
        assert!((actual - expected).abs() < 1e-7);
    }

    let boundary = minimize_lbfgs(
        &[1.0; 200],
        20,
        1e-9,
        1e-14,
        10,
        50,
        |parameters, gradient| {
            let mut value = 0.0;
            for (parameter, derivative) in parameters.iter().zip(gradient) {
                value += 0.5 * parameter * parameter;
                *derivative = *parameter;
            }
            Ok(value)
        },
    )
    .unwrap();
    assert!(boundary.iter().all(|value| value.abs() < 1e-9));

    let propagated = minimize_lbfgs(&[0.0], 10, 1e-7, 1e-12, 10, 50, |_, _| {
        Err(NumericsError::Resource("deadline".into()))
    });
    assert!(matches!(propagated, Err(NumericsError::Resource(_))));

    for invalid in [
        minimize_lbfgs(&[], 10, 1e-7, 1e-12, 10, 50, |_, _| Ok(0.0)),
        minimize_lbfgs(&[0.0; 201], 10, 1e-7, 1e-12, 10, 50, |_, _| Ok(0.0)),
        minimize_lbfgs(&[0.0], 2_001, 1e-7, 1e-12, 10, 50, |_, _| Ok(0.0)),
        minimize_lbfgs(&[0.0], 10, 1e-7, 1e-12, 11, 50, |_, _| Ok(0.0)),
        minimize_lbfgs(&[0.0], 10, 1e-7, 1e-12, 10, 51, |_, _| Ok(0.0)),
    ] {
        assert!(matches!(invalid, Err(NumericsError::Invalid(_))));
    }
}
