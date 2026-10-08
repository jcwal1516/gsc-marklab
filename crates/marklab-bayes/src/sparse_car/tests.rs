use super::*;

fn chain() -> SparseCarSpec {
    SparseCarSpec {
        graph_definition: "declared synthetic three-region weighted chain; preserved weights"
            .into(),
        coordinate_frame: "synthetic_region_graph".into(),
        observations: vec![
            SparseCarObservation {
                region_id: "a".into(),
                prior_mean: 0.2,
                value: Some(1.2),
                noise_sd: 0.5,
            },
            SparseCarObservation {
                region_id: "b".into(),
                prior_mean: -0.1,
                value: None,
                noise_sd: 0.8,
            },
            SparseCarObservation {
                region_id: "c".into(),
                prior_mean: 0.3,
                value: Some(-0.4),
                noise_sd: 0.7,
            },
        ],
        edges: [
            ("a", "b", 0.7),
            ("b", "a", 0.7),
            ("b", "c", 1.3),
            ("c", "b", 1.3),
        ]
        .into_iter()
        .map(|(a, b, weight)| SpatialEdge {
            source_region: a.into(),
            target_region: b.into(),
            weight,
        })
        .collect(),
        rho: 0.6,
        tau: 1.4,
        draws: 4096,
        seed: 20260912,
        solve_tolerance: 1e-10,
        maximum_iterations: 100,
        maximum_work: 10_000_000,
        memory_budget_bytes: 4 * 1024 * 1024,
    }
}

// Independent dense Cholesky oracle; production must remain sparse and matrix-free.
fn dense_posterior(spec: &SparseCarSpec) -> (Vec<f64>, Vec<f64>) {
    let (mean, covariance) = dense_parameters(spec, true);
    let n = mean.len();
    (mean, (0..n).map(|i| covariance[i * n + i]).collect())
}

fn dense_parameters(spec: &SparseCarSpec, posterior: bool) -> (Vec<f64>, Vec<f64>) {
    let n = spec.observations.len();
    let mut precision = vec![0.0; n * n];
    let mut right = vec![0.0; n];
    for edge in &spec.edges {
        let i = spec
            .observations
            .iter()
            .position(|x| x.region_id == edge.source_region)
            .unwrap();
        let j = spec
            .observations
            .iter()
            .position(|x| x.region_id == edge.target_region)
            .unwrap();
        precision[i * n + i] += spec.tau * edge.weight;
        precision[i * n + j] -= spec.tau * spec.rho * edge.weight;
    }
    for (i, row) in spec.observations.iter().enumerate() {
        if let Some(y) = row.value.filter(|_| posterior) {
            precision[i * n + i] += row.noise_sd.powi(-2);
            right[i] = (y - row.prior_mean) * row.noise_sd.powi(-2);
        }
    }
    let lower = crate::linalg::cholesky(&precision, n).unwrap();
    let solve = |b: &[f64]| {
        let mut x = crate::linalg::solve_lower(&lower, n, b);
        crate::linalg::solve_upper_from_lower_transpose_in_place(&lower, n, &mut x);
        x
    };
    let mean = solve(&right)
        .iter()
        .zip(&spec.observations)
        .map(|(m, o)| m + o.prior_mean)
        .collect();
    let mut covariance = vec![0.0; n * n];
    for j in 0..n {
        let mut b = vec![0.0; n];
        b[j] = 1.0;
        for (i, value) in solve(&b).into_iter().enumerate() {
            covariance[i * n + j] = value;
        }
    }
    (mean, covariance)
}

#[test]
fn missing_region_posterior_matches_dense_oracle_and_replays_seed() {
    let spec = chain();
    let result = fit_sparse_car(&spec).expect("conditional posterior fit");
    let (mean, variance) = dense_posterior(&spec);
    for (i, region) in result.regions.iter().enumerate() {
        assert!((region.posterior_mean - mean[i]).abs() < 1e-8);
        assert!((region.posterior_sd - variance[i].sqrt()).abs() < 6.0 * region.sd_mcse + 1e-8);
        assert_eq!(region.observed, spec.observations[i].value.is_some());
        assert!(
            (region.predictive_sd.powi(2)
                - region.posterior_sd.powi(2)
                - spec.observations[i].noise_sd.powi(2))
            .abs()
                < 1e-10
        );
    }
    assert_eq!(result, fit_sparse_car(&spec).unwrap());
    assert_eq!(result.fit_state, "complete");
    assert!(result.diagnostics.maximum_relative_solution_error_bound <= spec.solve_tolerance);
    result.validate_for(&spec).unwrap();
    let mut permuted = spec.clone();
    permuted.observations.reverse();
    permuted.edges.reverse();
    assert_eq!(result, fit_sparse_car(&permuted).unwrap());
}

#[test]
fn predictive_checks_and_prespecified_sensitivity_match_dense_gaussians() {
    let mut baseline = chain();
    baseline.observations[1].value = Some(0.6);
    for scenario in 0..6 {
        let mut spec = baseline.clone();
        match scenario {
            1 => spec.rho = 0.0,
            2 => spec.rho = 0.9,
            3 => spec.tau *= 0.5,
            4 => spec.tau *= 2.0,
            5 => {
                spec.edges[0].weight *= 2.0;
                spec.edges[1].weight *= 2.0;
            }
            _ => {}
        }
        let result = fit_sparse_car(&spec).unwrap();
        for (posterior, ppc) in [
            (false, &result.prior_predictive),
            (true, &result.posterior_predictive),
        ] {
            let (mean, covariance) = dense_parameters(&spec, posterior);
            let n = mean.len();
            let residual: Vec<_> = mean
                .iter()
                .zip(&spec.observations)
                .map(|(m, o)| m - o.prior_mean)
                .collect();
            let expected_energy: f64 = (0..n)
                .map(|i| {
                    (residual[i].powi(2)
                        + covariance[i * n + i]
                        + spec.observations[i].noise_sd.powi(2))
                        / spec.observations[i].noise_sd.powi(2)
                })
                .sum();
            let expected_roughness = [(0, 1, spec.edges[0].weight), (1, 2, spec.edges[2].weight)]
                .iter()
                .map(|&(i, j, w)| {
                    w * ((residual[i] - residual[j]).powi(2)
                        + covariance[i * n + i]
                        + covariance[j * n + j]
                        - 2.0 * covariance[i * n + j]
                        + spec.observations[i].noise_sd.powi(2)
                        + spec.observations[j].noise_sd.powi(2))
                })
                .sum::<f64>();
            for (check, expected) in [
                (&ppc.residual_energy, expected_energy),
                (ppc.edge_roughness.as_ref().unwrap(), expected_roughness),
            ] {
                let mcse = check.replicate_sd / (spec.draws as f64).sqrt();
                assert!((check.replicate_mean-expected).abs() < 6.0*mcse+1e-8,"scenario={scenario} posterior={posterior} actual={} expected={expected} mcse={mcse}",check.replicate_mean);
                assert!(check.probability_mcse > 0.0);
            }
            if posterior {
                for (i, region) in result.regions.iter().enumerate() {
                    assert!((region.posterior_mean - mean[i]).abs() < 1e-8);
                    assert!(
                        (region.posterior_sd - covariance[i * n + i].sqrt()).abs()
                            < 6.0 * region.sd_mcse + 1e-8
                    );
                }
            }
        }
        println!("sensitivity {scenario}: middle_mean={:.6} middle_sd={:.6} prior_energy_tail={:.4} posterior_energy_tail={:.4}",result.regions[1].posterior_mean,result.regions[1].posterior_sd,result.prior_predictive.residual_energy.upper_tail_probability,result.posterior_predictive.residual_energy.upper_tail_probability);
    }
}

#[test]
fn prior_generative_field_calibration_and_coverage() {
    let mut spec = chain();
    spec.draws = 512;
    let (_, covariance) = dense_parameters(&spec, false);
    // This data generator factors a dense covariance; it does not use production perturbations.
    let lower = crate::linalg::cholesky(&covariance, 3).unwrap();
    let mut rng = ChaCha8Rng::seed_from_u64(0x7362_635f_6361_725f);
    let mut bins = [0_usize; 10];
    let cuts = [
        -1.281_551_565_544_600_4,
        -0.841_621_233_572_914_3,
        -0.524_400_512_708_040_9,
        -0.253_347_103_135_799_7,
        0.0,
        0.253_347_103_135_799_7,
        0.524_400_512_708_040_9,
        0.841_621_233_572_914_3,
        1.281_551_565_544_600_4,
    ];
    let mut covered = 0;
    let mut standardized_sum = 0.0;
    let mut standardized_squares = 0.0;
    let mut prior_tail_sum = 0.0;
    let mut posterior_tail_sum = 0.0;
    let replicates = 400;
    for replicate in 0..replicates {
        let z: Vec<f64> = (0..3).map(|_| rng.sample(StandardNormal)).collect();
        let field: Vec<f64> = (0..3)
            .map(|i| (0..=i).map(|j| lower[i * 3 + j] * z[j]).sum())
            .collect();
        for (i, row) in spec.observations.iter_mut().enumerate() {
            if row.value.is_some() {
                row.value = Some(
                    row.prior_mean + field[i] + row.noise_sd * rng.sample::<f64, _>(StandardNormal),
                );
            }
        }
        spec.seed = 20260912 + replicate as u64 * 7919;
        let result = fit_sparse_car(&spec).unwrap();
        // Middle region is unobserved: exercises genuine spatial posterior prediction.
        let region = &result.regions[1];
        let truth = spec.observations[1].prior_mean + field[1];
        let standardized = (truth - region.posterior_mean) / region.posterior_sd;
        bins[cuts.partition_point(|cut| *cut < standardized)] += 1;
        covered += usize::from(truth >= region.lower_95 && truth <= region.upper_95);
        standardized_sum += standardized;
        standardized_squares += standardized.powi(2);
        prior_tail_sum += result
            .prior_predictive
            .residual_energy
            .upper_tail_probability;
        posterior_tail_sum += result
            .posterior_predictive
            .residual_energy
            .upper_tail_probability;
    }
    let n = replicates as f64;
    let coverage = covered as f64 / n;
    let coverage_se = (0.95_f64 * 0.05 / n).sqrt();
    assert!(
        (coverage - 0.95).abs() <= 4.0 * coverage_se,
        "coverage={coverage}"
    );
    for count in bins {
        assert!(
            (count as f64 - n / 10.0).abs() <= 4.0 * (n * 0.1 * 0.9).sqrt(),
            "bins={bins:?}"
        );
    }
    assert!((standardized_sum / n).abs() < 4.0 / n.sqrt());
    assert!((standardized_squares / n - 1.0).abs() < 4.0 * (2.0 / n).sqrt());
    assert!((prior_tail_sum / n - 0.5).abs() < 4.0 / (12.0 * n).sqrt());
    println!("CAR calibration: {replicates} complete; rank_bins={bins:?}; coverage={covered}/{replicates} ({coverage:.4}, null_MCSE={coverage_se:.4}); standardized_bias={:.5}; standardized_MSE={:.5}; prior_tail_mean={:.5}; posterior_tail_mean={:.5}",standardized_sum/n,standardized_squares/n,prior_tail_sum/n,posterior_tail_sum/n);
}

#[test]
fn disconnected_prior_only_component_and_resource_failures_are_explicit() {
    let mut spec = chain();
    spec.draws = 512;
    let mut second = chain();
    for row in &mut second.observations {
        row.region_id = format!("other-{}", row.region_id);
        row.value = None;
    }
    for edge in &mut second.edges {
        edge.source_region = format!("other-{}", edge.source_region);
        edge.target_region = format!("other-{}", edge.target_region);
    }
    spec.observations.extend(second.observations);
    spec.edges.extend(second.edges);
    let result = fit_sparse_car(&spec).unwrap();
    assert_eq!(result.components, 2);
    assert_eq!(result.components_without_observations, 1);
    let (mean, variance) = dense_posterior(&spec);
    for (i, row) in result.regions.iter().enumerate() {
        assert!((row.posterior_mean - mean[i]).abs() < 1e-8);
        assert!((row.posterior_sd - variance[i].sqrt()).abs() < 6.0 * row.sd_mcse + 1e-8);
    }
    for boundary in [
        "memory",
        "work",
        "iterations",
        "island",
        "asymmetry",
        "intrinsic",
        "nonfinite",
    ] {
        let mut invalid = spec.clone();
        match boundary {
            "memory" => invalid.memory_budget_bytes = result.diagnostics.estimated_peak_bytes - 1,
            "work" => invalid.maximum_work = result.diagnostics.work_used - 1,
            "iterations" => invalid.maximum_iterations = 1,
            "island" => invalid
                .edges
                .retain(|e| e.source_region != "c" && e.target_region != "c"),
            "asymmetry" => {
                invalid.edges.pop();
            }
            "intrinsic" => invalid.rho = 1.0,
            "nonfinite" => invalid.observations[0].noise_sd = f64::NAN,
            _ => unreachable!(),
        }
        assert!(fit_sparse_car(&invalid).is_err(), "{boundary}");
    }
    let mut corrupt = result.clone();
    corrupt.regions[0].posterior_sd = f64::NAN;
    assert!(corrupt.validate_for(&spec).is_err());
    corrupt = result;
    corrupt.diagnostics.maximum_relative_solution_error_bound = 1e-3;
    assert!(corrupt.validate_for(&spec).is_err());
}

#[test]
fn larger_sparse_chain_fits_below_one_dense_matrix_memory() {
    let mut spec = chain();
    let n = 2048;
    spec.observations = (0..n)
        .map(|i| SparseCarObservation {
            region_id: format!("r{i:05}"),
            prior_mean: 0.0,
            value: Some((i as f64 * 0.02).sin()),
            noise_sd: 0.4,
        })
        .collect();
    spec.edges = (1..n)
        .flat_map(|i| [(i - 1, i), (i, i - 1)])
        .map(|(i, j)| SpatialEdge {
            source_region: format!("r{i:05}"),
            target_region: format!("r{j:05}"),
            weight: 1.0,
        })
        .collect();
    spec.draws = 256;
    spec.rho = 0.15;
    spec.solve_tolerance = 1e-8;
    spec.maximum_work = 500_000_000;
    spec.memory_budget_bytes = 16 * 1024 * 1024;
    let result = fit_sparse_car(&spec).unwrap();
    assert_eq!(result.regions.len(), n);
    assert_eq!(result.undirected_edges, n - 1);
    assert!(result.diagnostics.estimated_peak_bytes < spec.memory_budget_bytes);
    assert!(spec.memory_budget_bytes < n * n * 8);
    println!("sparse admission: {n} regions; {} undirected edges; estimated peak={} bytes; work={} visits; maximum iterations={}",result.undirected_edges,result.diagnostics.estimated_peak_bytes,result.diagnostics.work_used,result.diagnostics.maximum_iterations_used);
}
