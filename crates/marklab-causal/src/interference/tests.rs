use super::{estimation::exposure, *};

#[test]
fn independent_cluster_randomization_states_form_cartesian_product() {
    let result = randomized_binary_interference(spec(false)).unwrap();
    assert_eq!(result.assignment_states, 4);
    assert_eq!(result.exposure_probabilities.len(), 4);
    assert_eq!(
        result.exposure_probabilities[0]
            .probabilities
            .treated_neighbor_untreated,
        0.5
    );
}

#[test]
fn post_treatment_baseline_covariate_is_rejected() {
    let error = randomized_binary_interference(spec(true)).unwrap_err();
    assert!(error.to_string().contains("post-treatment"));
}

fn spec(post_treatment_covariate: bool) -> RandomizedInterferenceSpec {
    let mut units = vec![
        unit("a1", "a", true),
        unit("a2", "a", false),
        unit("b1", "b", true),
        unit("b2", "b", false),
    ];
    if post_treatment_covariate {
        units[0].baseline_covariates[0].measurement_time = 0.5;
    }
    RandomizedInterferenceSpec {
        design_provenance: "synthetic".into(),
        graph_provenance: "prespecified".into(),
        units,
        graph_edges: vec![
            InterferenceEdge {
                left_unit: "a1".into(),
                right_unit: "a2".into(),
            },
            InterferenceEdge {
                left_unit: "b1".into(),
                right_unit: "b2".into(),
            },
        ],
        cluster_assignments: vec![
            ClusterAssignment {
                cluster_id: "a".into(),
                treated_units: 1,
            },
            ClusterAssignment {
                cluster_id: "b".into(),
                treated_units: 1,
            },
        ],
        test_exposure_high: exposure(true, false),
        test_exposure_low: exposure(false, true),
        permutations: 10,
        seed: 7,
        maximum_assignment_states: 4,
        maximum_unit_assignment_evaluations: 10_000,
    }
}

fn unit(id: &str, cluster: &str, treatment: bool) -> CausalUnit {
    CausalUnit {
        unit_id: id.into(),
        cluster_id: cluster.into(),
        treatment,
        treatment_time: 0.0,
        outcome: f64::from(treatment),
        outcome_time: 1.0,
        eligible: true,
        baseline_covariates: vec![BaselineCovariate {
            name: "x".into(),
            value: 0.0,
            measurement_time: -1.0,
        }],
    }
}
