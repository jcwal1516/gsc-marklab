use marklab_numerics::{minimize_bfgs, NumericsError};

#[test]
fn bfgs_recovers_coupled_positive_quadratic() {
    let fit = minimize_bfgs(&[0.0, 0.0], 1000, 1e-8, |x, g| {
        let a = x[0] - 3.0;
        let b = x[1] + 2.0;
        g[0] = 4.0 * a + b;
        g[1] = a + 2.0 * b;
        Ok(2.0 * a * a + a * b + b * b)
    })
    .unwrap();
    assert!((fit[0] - 3.0).abs() < 1e-8);
    assert!((fit[1] + 2.0).abs() < 1e-8);
}

#[test]
fn bfgs_propagates_resource_errors_and_rejects_nonfinite_evaluations() {
    let error = minimize_bfgs(&[0.0], 100, 1e-8, |_, _| {
        Err(NumericsError::Resource("deadline".into()))
    })
    .unwrap_err();
    assert!(matches!(error, NumericsError::Resource(_)));
    assert!(minimize_bfgs(&[0.0], 100, 1e-8, |_, _| Ok(f64::NAN)).is_err());
    assert!(minimize_bfgs(&[], 100, 1e-8, |_, _| Ok(0.0)).is_err());
}

#[test]
fn bfgs_does_not_accept_unconverged_exhausted_iterations() {
    assert!(minimize_bfgs(&[1.0, 1.0], 1, 1e-8, |x, g| {
        g[0] = 2.0 * x[0];
        g[1] = 200.0 * x[1];
        Ok(x[0] * x[0] + 100.0 * x[1] * x[1])
    })
    .is_err());
}

#[test]
fn bfgs_matches_planted_minima_across_positive_definite_scales() {
    for n in [1, 2, 8, 32] {
        for scale in [0.001, 1.0, 1000.0] {
            let matrix = (0..n * n)
                .map(|i| ((i + 1) as f64 * 0.31).sin())
                .collect::<Vec<_>>();
            let target = (0..n).map(|i| (i as f64 * 0.73).cos()).collect::<Vec<_>>();
            let mut projected = vec![0.0; n];
            let fit = minimize_bfgs(&vec![0.0; n], 1000, 1e-8, |x, g| {
                for i in 0..n {
                    projected[i] = (0..n).map(|j| matrix[i * n + j] * (x[j] - target[j])).sum();
                }
                let mut value = 0.0;
                for i in 0..n {
                    let delta = x[i] - target[i];
                    value += 0.5 * scale * (delta * delta + projected[i] * projected[i]);
                    g[i] = scale
                        * (delta
                            + (0..n)
                                .map(|j| matrix[j * n + i] * projected[j])
                                .sum::<f64>());
                }
                Ok(value)
            })
            .unwrap();
            let error = fit
                .iter()
                .zip(&target)
                .map(|(a, b)| (a - b) * (a - b))
                .sum::<f64>()
                .sqrt();
            // The Hessian is scale*(I+M'M), so ||error||₂ <= ||gradient||₂ / scale.
            assert!(
                error <= (n as f64).sqrt() * 1.01e-8 / scale,
                "n={n}, scale={scale}, error={error}"
            );
        }
    }
}
