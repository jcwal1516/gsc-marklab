use marklab_bayes::{fit_partial_transport, PartialTransportSpec, TransportMass};
use serde_json::Value;

fn spec(name: &str) -> PartialTransportSpec {
    let v: Value = serde_json::from_slice(
        &std::fs::read(format!(
            "{}/tests/fixtures/partial_transport/{name}.spec.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    let support = |key: &str| {
        v[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| TransportMass {
                id: r["id"].as_str().unwrap().into(),
                mass: r["mass"].as_f64().unwrap(),
            })
            .collect()
    };
    PartialTransportSpec {
        source: support("source"),
        target: support("target"),
        costs_row_major: v["costs_row_major"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_f64().unwrap())
            .collect(),
        transported_mass: v["transported_mass"].as_f64().unwrap(),
        epsilon: v["epsilon"].as_f64().unwrap(),
        timeout_seconds: 120,
    }
}
fn compare(a: &Value, b: &Value, path: &str) {
    match a {
        Value::Object(fields) => {
            assert_eq!(fields.len(), b.as_object().unwrap().len(), "{path}");
            for (k, v) in fields {
                compare(v, &b[k], &format!("{path}.{k}"));
            }
        }
        Value::Array(rows) => {
            assert_eq!(rows.len(), b.as_array().unwrap().len(), "{path}");
            for (i, v) in rows.iter().enumerate() {
                compare(v, &b[i], &format!("{path}[{i}]"));
            }
        }
        Value::Number(n) if n.is_f64() => {
            let x = n.as_f64().unwrap();
            let y = b.as_f64().unwrap();
            if path.ends_with(".cost") {
                assert_eq!(x.to_bits(), y.to_bits(), "{path}");
            } else {
                assert!(
                    y.is_finite() && (x - y).abs() <= 1e-6 * (1. + x.abs()),
                    "{path}: {x} != {y}"
                );
            }
        }
        _ => assert_eq!(a, b, "{path}"),
    }
}
#[test]
fn complete_partial_plan_matches_independent_slsqp_oracles() {
    for name in [
        "forced",
        "inactive",
        "binding",
        "balanced",
        "representative",
        "demanding",
    ] {
        let reference: Value = serde_json::from_slice(
            &std::fs::read(format!(
                "{}/tests/fixtures/partial_transport/{name}.oracle.json",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
        )
        .unwrap();
        let result = serde_json::to_value(fit_partial_transport(spec(name)).unwrap()).unwrap();
        for key in [
            "plan",
            "source_marginals",
            "target_marginals",
            "transported_mass",
            "unmatched_source_mass",
            "unmatched_target_mass",
            "transport_cost",
            "entropy",
            "regularized_objective",
            "maximum_constraint_violation",
            "constraint_status",
        ] {
            compare(&reference[key], &result[key], &format!("{name}.{key}"));
        }
        assert!(result["maximum_constraint_violation"].as_f64().unwrap() <= 1e-8);
        assert_eq!(result["backend"]["name"], "marklab-rust");
        assert_eq!(result["version"], 2);
    }
}
#[test]
fn forced_and_uniform_plans_have_analytic_mass_and_entropy() {
    let forced = fit_partial_transport(spec("forced")).unwrap();
    assert!((forced.plan[0].mass - 1.5).abs() < 1e-12);
    assert!((forced.transport_cost - 6.).abs() < 1e-12);
    assert!((forced.regularized_objective - (6. + 0.5 * 1.5 * (1.5_f64.ln() - 1.))).abs() < 1e-12);
    let uniform = fit_partial_transport(spec("maximum_null")).unwrap();
    assert_eq!(uniform.plan.len(), 4096);
    assert!(uniform
        .plan
        .iter()
        .all(|x| (x.mass - 1. / 128.).abs() < 1e-12));
    assert!((uniform.transported_mass - 32.).abs() < 1e-8);
}
#[test]
fn zero_capacities_remain_zero_and_no_support_is_dropped() {
    let result = fit_partial_transport(spec("zero_capacity")).unwrap();
    assert_eq!(result.plan.len(), 9);
    assert_eq!(result.source_marginals[0], 0.);
    assert_eq!(result.target_marginals[1], 0.);
    assert!(result.plan[0..3].iter().all(|r| r.mass == 0.));
    assert!(result.maximum_constraint_violation <= 1e-8);
    // All positive-support capacities are inactive here. The independent Gibbs formula
    // therefore gives the unique minimizer, despite the failed SLSQP reference.
    let weights = [(-0.5_f64).exp(), (-1.5_f64).exp(), (-2.0_f64).exp(), 1.];
    let normalizer = weights.iter().sum::<f64>();
    for (index, weight) in [3, 5, 6, 8].into_iter().zip(weights) {
        assert!((result.plan[index].mass - 1.5 * weight / normalizer).abs() < 1e-12);
    }
}
#[test]
fn partial_transport_rejects_invalid_and_excess_requested_mass() {
    for case in 0..6 {
        let mut s = spec("forced");
        match case {
            0 => s.transported_mass = 4.,
            1 => s.epsilon = 0.,
            2 => s.source[0].mass = 0.,
            3 => s.costs_row_major[0] = f64::NAN,
            4 => s.source[0].id = " bad ".into(),
            _ => s.timeout_seconds = 0,
        };
        assert!(fit_partial_transport(s).is_err());
    }
}

#[test]
fn dense_identity_expansion_is_bounded_before_constructing_the_plan() {
    let mut s = spec("maximum_null");
    s.source[0].id = "s".repeat(300_000);
    let error = fit_partial_transport(s).unwrap_err().to_string();
    assert!(error.contains("plan identity expansion"), "{error}");
}

#[test]
fn support_permutation_and_constant_cost_shift_preserve_the_unique_plan() {
    let original = spec("binding");
    let baseline = fit_partial_transport(original.clone()).unwrap();
    let mut transformed = original.clone();
    transformed.source.reverse();
    transformed.target.reverse();
    let n = original.source.len();
    let m = original.target.len();
    transformed.costs_row_major = (0..n)
        .flat_map(|i| (0..m).map(move |j| (n - 1 - i) * m + (m - 1 - j)))
        .map(|index| original.costs_row_major[index] + 2.)
        .collect();
    let result = fit_partial_transport(transformed).unwrap();
    for (a, b) in baseline.plan.iter().zip(result.plan.iter().rev()) {
        assert_eq!(a.source_id, b.source_id);
        assert_eq!(a.target_id, b.target_id);
        assert!((a.mass - b.mass).abs() < 1e-10);
    }
    assert!(
        (result.regularized_objective
            - baseline.regularized_objective
            - 2. * original.transported_mass)
            .abs()
            < 1e-10
    );
}

#[test]
fn frozen_legacy_python_results_remain_readable_and_validated() {
    for name in [
        "forced",
        "binding",
        "balanced",
        "representative",
        "demanding",
        "maximum_null",
    ] {
        let root = format!(
            "{}/tests/fixtures/partial_transport",
            env!("CARGO_MANIFEST_DIR")
        );
        let raw = std::fs::read(format!("{root}/{name}.request.json")).unwrap();
        let request: Value = serde_json::from_slice(&raw).unwrap();
        let request = marklab_bayes::PartialTransportWorkerRequest::new(
            spec(name),
            request["backend"]["environment_lock_sha256"]
                .as_str()
                .unwrap()
                .into(),
            request["backend"]["worker_sha256"].as_str().unwrap().into(),
        )
        .unwrap();
        let result: marklab_bayes::PartialTransportWorkerResult =
            serde_json::from_slice(&std::fs::read(format!("{root}/{name}.oracle.json")).unwrap())
                .unwrap();
        result
            .validate(&request, &marklab_bayes::sha256_hex(&raw))
            .unwrap();
    }
}
