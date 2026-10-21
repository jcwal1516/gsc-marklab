use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use marklab::{
    AnalysisConfig, AnalysisEngine, ArtifactStatus, ComponentMode, HeCell, IhcCell, LandmarkPair,
    MultimodalEngine, MultimodalInput, OutputSection, OutputWriter, Pattern, PatternMeta,
    RegistrationTransform, ResultDocument, ThreadSetting, TimingStage,
};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

// These benchmarks report production stage timings, not whole-analysis latency.
// Each measured stage builds one pair plan and evaluates observed labels plus
// 19 seeded null labelings, then assembles its global envelope.
fn bench_spatial_stages(c: &mut Criterion) {
    let full = std::env::var("MARKLAB_BENCH_PROFILE").as_deref() == Ok("full");
    let sizes: &[usize] = if full { &[256, 512, 1_024] } else { &[256] };
    let mut group = c.benchmark_group("spatial_stages");
    group.sample_size(10);

    for &n in sizes {
        let side = (n as f64).sqrt().ceil() as usize;
        let pattern = lattice_pattern(n);

        // Independent exhaustive geometry oracle, outside all timed iterations.
        // Covariance follows E[Mi Mj] - p E[Mi + Mj] + p^2 rather than
        // reproducing the production plan's centered-product accumulation.
        let mut pair_counts = [0_usize; 5];
        let mut both_marked = [0_usize; 5];
        let mut marked_endpoints = [0_usize; 5];
        let mut cross_counts = [0_usize; 5];
        for i in 0..n {
            for j in i + 1..n {
                let distance =
                    (pattern.x_um[i] - pattern.x_um[j]).hypot(pattern.y_um[i] - pattern.y_um[j]);
                if distance >= 5.0 {
                    continue;
                }
                let bin = distance.floor() as usize;
                pair_counts[bin] += 1;
                both_marked[bin] += usize::from(pattern.mark[i] == 1 && pattern.mark[j] == 1);
                marked_endpoints[bin] +=
                    usize::from(pattern.mark[i]) + usize::from(pattern.mark[j]);
                let is_abnormal = |index: usize| index.is_multiple_of(2);
                let is_lymphocyte =
                    |index: usize| !index.is_multiple_of(2) && index.is_multiple_of(5);
                cross_counts[bin] += usize::from(
                    (is_abnormal(i) && is_lymphocyte(j)) || (is_lymphocyte(i) && is_abnormal(j)),
                );
            }
        }
        let p = pattern.mark.iter().copied().map(f64::from).sum::<f64>() / n as f64;
        let covariance = std::array::from_fn::<_, 5, _>(|bin| {
            (pair_counts[bin] > 0).then(|| {
                (both_marked[bin] as f64 - p * marked_endpoints[bin] as f64
                    + p * p * pair_counts[bin] as f64)
                    / pair_counts[bin] as f64
            })
        });
        assert_eq!(pair_counts[0], 0);
        assert!(pair_counts[1..].iter().all(|&count| count > 0));
        assert!(cross_counts.iter().sum::<usize>() > 0);

        let mut config = base_config(n);
        config.validation.largest_interpretable_scale_fraction = 5.0 / side as f64;
        let marked = AnalysisEngine::new(config.clone()).expect("marked engine");
        group.bench_with_input(
            BenchmarkId::new("mark_pair_plan_plus_20_evaluations", n),
            &pattern,
            |b, pattern| {
                b.iter_custom(|iterations| {
                    let mut elapsed = Duration::ZERO;
                    for _ in 0..iterations {
                        let result = marked
                            .analyze_pattern(black_box(pattern))
                            .expect("marked analysis");
                        assert_eq!(result.mark_pair_covariance_curve.len(), 5);
                        for (bin, point) in result.mark_pair_covariance_curve.iter().enumerate() {
                            assert_eq!(
                                (point.r_min_um, point.r_max_um),
                                (bin as f64, (bin + 1) as f64)
                            );
                            assert_eq!(point.pair_count, pair_counts[bin]);
                            match (point.covariance, covariance[bin]) {
                                (Some(actual), Some(expected)) => {
                                    assert!((actual - expected).abs() < 1e-12)
                                }
                                (None, None) => {}
                                _ => {
                                    panic!("covariance availability differs from exhaustive oracle")
                                }
                            }
                        }
                        let summary = result
                            .mark_pair_covariance
                            .value()
                            .expect("available mark-pair inference");
                        assert_eq!(summary.n_permutations, 19);
                        assert_rank_probability(summary.p_global.expect("mark-pair p-value"));
                        elapsed += stage_duration(&result.timings, "mark_pair_covariance");
                        black_box(result);
                    }
                    elapsed
                });
            },
        );

        let input = multimodal_input(&pattern);
        configure_multimodal(&mut config);
        config.neighborhood.radius_um = 5.0; // Public engine chooses bin width radius / 5 = 1.
        config.neighborhood.k_nearest = 0;
        let multimodal = MultimodalEngine::new(config).expect("multimodal engine");
        group.bench_with_input(
            BenchmarkId::new("cross_interaction_plan_plus_20_evaluations", n),
            &input,
            |b, input| {
                b.iter_custom(|iterations| {
                    let mut elapsed = Duration::ZERO;
                    for _ in 0..iterations {
                        let run = multimodal
                            .analyze_run(black_box(input))
                            .expect("multimodal analysis");
                        assert_eq!(run.transform.apply(2.0, 3.0), (2.0, 3.0));
                        let curves = run
                            .result
                            .cross_interaction_curves
                            .value()
                            .expect("available cross-interaction curves");
                        assert_eq!(curves.len(), 1);
                        let curve = &curves[0];
                        assert_eq!(
                            (&*curve.label_a, &*curve.label_b),
                            ("lymphocyte", "mmr_abnormal")
                        );
                        assert_eq!(curve.points.len(), 5);
                        for (bin, point) in curve.points.iter().enumerate() {
                            assert_eq!(
                                (point.r_min_um, point.r_max_um),
                                (bin as f64, (bin + 1) as f64)
                            );
                            assert_eq!(point.count, cross_counts[bin]);
                            assert_eq!(
                                point.value,
                                (pair_counts[bin] > 0).then_some(cross_counts[bin] as f64)
                            );
                        }
                        assert_rank_probability(curve.p_global.expect("cross-interaction p-value"));
                        elapsed += stage_duration(&run.result.timings, "cross_interaction_curves");
                        black_box(run);
                    }
                    elapsed
                });
            },
        );
    }
    group.finish();
}

fn stage_duration(timings: &[TimingStage], name: &str) -> Duration {
    let stage = timings
        .iter()
        .find(|stage| stage.stage_name == name)
        .expect("production timing stage");
    assert!(stage.wall_ms.is_finite() && stage.wall_ms >= 0.0);
    Duration::from_secs_f64(stage.wall_ms / 1_000.0)
}

fn assert_rank_probability(p: f64) {
    assert!(p.is_finite() && (0.05..=1.0).contains(&p));
    assert!((p * 20.0 - (p * 20.0).round()).abs() < 1e-12);
}

fn lattice_pattern(n: usize) -> Pattern {
    let side = (n as f64).sqrt().ceil() as usize;
    let mut pattern = Pattern::from_arrays(
        (0..n).map(|i| (i % side) as f64).collect(),
        (0..n).map(|i| (i / side) as f64).collect(),
        (0..n).map(|i| u8::from(i % 7 == 0)).collect(),
        PatternMeta {
            case_id: "bench".into(),
            timepoint: "post".into(),
            protein: "MSH6".into(),
            slide_id: None,
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("unit-lattice pattern");
    pattern.window.area_um2 = (side * side) as f64;
    pattern.window.analysis_effective_length_um = side as f64;
    pattern.window.d_nn_mean_um = 1.0;
    pattern
}

fn base_config(n: usize) -> AnalysisConfig {
    let mut config = AnalysisConfig::default();
    config.analysis.analyze_components = ComponentMode::Pooled;
    config.validation.n_min = n;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.validation.area_min_um2 = 1.0;
    config.validation.k_shell_min = 1;
    config.spectrum.k_shells = 1;
    config.spectrum.low_k_shells = 1;
    config.spectrum.anisotropy_low_k_shells = 1;
    config.spectrum.fit_low_k_alpha = false;
    config.permutation.b = 19;
    config.permutation.seed = 123;
    config.permutation.stratified = false;
    // The public configuration requires B + 1 >= 2 / alpha.
    config.inference.family_wise_alpha = 0.10;
    config.periodogram.enabled = false;
    config.multiscale_residual.enabled = false;
    config.performance.threads = ThreadSetting::Count(1);
    config
}

fn multimodal_input(pattern: &Pattern) -> MultimodalInput {
    let mut input = MultimodalInput {
        he_cells: Vec::new(),
        ihc_cells: Vec::new(),
        landmarks: [
            (0.0, 0.0),
            (10.0, 0.0),
            (0.0, 10.0),
            (10.0, 10.0),
            (5.0, 2.0),
            (2.0, 5.0),
        ]
        .into_iter()
        .map(|(x, y)| LandmarkPair::new(x, y, x, y))
        .collect(),
        case_id: "bench".into(),
        timepoint: "post".into(),
        protein: "MSH6".into(),
    };
    for i in 0..pattern.len() {
        if i % 2 == 0 {
            input.ihc_cells.push(IhcCell {
                cell_id: format!("cell-{i}"),
                x_um: pattern.x_um[i],
                y_um: pattern.y_um[i],
                mmr_mark: Some(1),
                mmr_probability: Some(0.9),
            });
        } else {
            input.he_cells.push(HeCell {
                cell_id: format!("cell-{i}"),
                x_um: pattern.x_um[i],
                y_um: pattern.y_um[i],
                cell_type: Some(if i % 5 == 0 { "lymphocyte" } else { "tumor" }.into()),
                cell_type_probability: Some(0.9),
            });
        }
    }
    input
}

fn configure_multimodal(config: &mut AnalysisConfig) {
    // Identity rigid landmarks preserve exact lattice distances at bin edges.
    config.registration.transform = RegistrationTransform::Rigid;
    config.neighborhood.label_pairs = vec![["mmr_abnormal".into(), "lymphocyte".into()]];
    config.neighborhood.null_models.clear();
    config.neighborhood.territory_eps_um = 1.5;
    config.neighborhood.territory_min_cells = 2;
    config.neighborhood.territory_min_radius_um = 0.5;
}

// Whole marked workflow, including real anisotropy null permutations and the
// residual territory plan reused for observed labels and 19 null labelings.
// Smoke uses 256 cells; full uses 1,024, independently of the stage-size sweep.
fn bench_complete_marked_and_output(c: &mut Criterion) {
    let full = std::env::var("MARKLAB_BENCH_PROFILE").as_deref() == Ok("full");
    let n = if full { 1_024 } else { 256 };
    let side = (n as f64).sqrt() as usize;
    let mut pattern = lattice_pattern(n);
    for (i, mark) in pattern.mark.iter_mut().enumerate() {
        *mark = u8::from(i % side < side / 4);
    }
    let mut config = base_config(n);
    config.spectrum.k_shells = 8;
    config.spectrum.anisotropy_low_k_shells = 5;
    config.periodogram.enabled = true;
    config.multiscale_residual.enabled = true;
    config.multiscale_residual.territory_detection = true;
    let engine = AnalysisEngine::new(config).expect("complete marked engine");
    let expected = engine
        .analyze_pattern(&pattern)
        .expect("complete marked analysis");
    assert_eq!(expected.n_cells, n);
    assert_eq!(expected.n_marked, n / 4);
    assert_eq!(expected.p_hat, 0.25);
    assert!(!expected.spectrum_curve.is_empty());
    let anisotropy = expected.anisotropy.value().expect("available anisotropy");
    // This index is the major/minor eigenvalue ratio, with isotropy at one.
    assert!(anisotropy.index.is_finite() && anisotropy.index >= 1.0);
    assert_rank_probability(anisotropy.p_value.expect("anisotropy permutation p-value"));
    let residual = expected
        .multiscale_residual
        .value()
        .expect("available residual inference");
    let territories = expected
        .residual_territories
        .value()
        .expect("available residual territories");
    assert!(!territories.is_empty());
    assert_eq!(residual.territory_count, territories.len());
    assert_rank_probability(
        *residual
            .territory_count_p_value
            .value()
            .expect("territory null inference"),
    );
    let territory_count = territories.len();
    let curve_count = expected.spectrum_curve.len();
    let document = ResultDocument::marked(expected);
    let options = OutputSection {
        write_parquet_curves: cfg!(feature = "parquet"),
        ..OutputSection::default()
    };
    let mut group = c.benchmark_group("complete_marked_workflow");
    group.sample_size(10);
    group.bench_function(format!("analysis_n{n}_b19"), |b| {
        b.iter(|| black_box(engine.analyze_pattern(black_box(&pattern))).expect("marked analysis"));
    });
    // Only OutputWriter's transaction is timed: fixture analysis, directory
    // setup, artifact checks, JSON reading, and cleanup are outside the timer.
    group.bench_function(format!("output_transaction_n{n}"), |b| {
        b.iter_custom(|iterations| {
            let mut elapsed = Duration::ZERO;
            for _ in 0..iterations {
                let dir = tempfile::tempdir().expect("output benchmark directory");
                let out = dir.path().join("result");
                let started = Instant::now();
                let manifest = OutputWriter::write(black_box(&document), &out, &options)
                    .expect("write result transaction");
                elapsed += started.elapsed();
                assert!(matches!(&manifest.result, ArtifactStatus::Written { path } if path == &out.join("result.json")));
                for name in ["residual_territories", "figures"] {
                    assert!(matches!(manifest.artifacts.get(name), Some(ArtifactStatus::Written { path }) if path.is_file()));
                }
                assert!(out.join("run_manifest.json").is_file());
                if options.write_parquet_curves {
                    assert!(matches!(manifest.artifacts.get("parquet_curves"), Some(ArtifactStatus::Written { path }) if path.is_file()));
                }
                let written = ResultDocument::from_json(&std::fs::read_to_string(out.join("result.json")).expect("read result JSON"))
                    .expect("parse result JSON").into_marked_pattern().expect("marked result");
                assert_eq!((written.n_cells, written.n_marked, written.p_hat), (n, n / 4, 0.25));
                assert_eq!(written.spectrum_curve.len(), curve_count);
                assert_eq!(written.residual_territories.value().expect("written territories").len(), territory_count);
                black_box(manifest);
            }
            elapsed
        });
    });
    group.finish();
}

// Whole multimodal latency includes index, graph, enrichment, cross curves,
// territory detection, and profiles. Only the radius case gets the full
// 1,024..8,192-cell scaling sweep; kNN uses one size to avoid a Cartesian sweep.
fn bench_complete_multimodal(c: &mut Criterion) {
    let full = std::env::var("MARKLAB_BENCH_PROFILE").as_deref() == Ok("full");
    let radius_sizes: &[usize] = if full {
        &[1_024, 2_048, 4_096, 8_192]
    } else {
        &[256]
    };
    let knn_sizes: &[usize] = if full { &[1_024] } else { &[256] };
    let mut group = c.benchmark_group("complete_multimodal_workflow");
    group.sample_size(10);
    for (name, sizes, radius, k_nearest) in
        [("radius", radius_sizes, 1.5, 0), ("knn", knn_sizes, 0.5, 8)]
    {
        for &n in sizes {
            let pattern = lattice_pattern(n);
            let input = multimodal_input(&pattern);
            let mut config = base_config(n);
            configure_multimodal(&mut config);
            config.neighborhood.radius_um = radius;
            config.neighborhood.k_nearest = k_nearest;
            let engine = MultimodalEngine::new(config).expect("complete multimodal engine");
            let expected = engine
                .analyze_run(&input)
                .expect("complete multimodal analysis");
            assert_eq!(expected.graph.n_nodes, n);
            assert_eq!(expected.transform.apply(2.0, 3.0), (2.0, 3.0));
            if k_nearest == 0 {
                // At radius 1.5, exactly horizontal, vertical, and diagonal
                // unit-grid neighbors connect. Count those once analytically.
                let side = (n as f64).sqrt().ceil() as usize;
                let mut edge_count = 0;
                for i in 0..n {
                    let x = i % side;
                    edge_count += usize::from(x + 1 < side && i + 1 < n);
                    edge_count += usize::from(i + side < n);
                    edge_count += usize::from(x + 1 < side && i + side + 1 < n);
                    edge_count += usize::from(x > 0 && i + side - 1 < n);
                }
                assert_eq!(expected.graph.edges.len(), edge_count);
                assert!(expected
                    .graph
                    .edges
                    .iter()
                    .all(|edge| edge.distance_um <= radius));
            } else {
                // Unit spacing exceeds radius 0.5, so every edge must come
                // from the kNN query path and every node must have >= k edges.
                let mut degrees = vec![0; n];
                for edge in &expected.graph.edges {
                    assert!(edge.distance_um > radius);
                    degrees[edge.source] += 1;
                    degrees[edge.target] += 1;
                }
                assert!(degrees.iter().all(|&degree| degree >= k_nearest));
                assert!(expected.graph.edges.len() <= n * k_nearest);
            }
            let territories = expected
                .result
                .neighborhood_territories
                .value()
                .expect("available territories");
            assert!(!territories.is_empty());
            assert_eq!(
                territories
                    .iter()
                    .map(|territory| territory.supporting_abnormal_cells)
                    .sum::<usize>(),
                n / 2
            );
            let profiles = expected
                .result
                .territory_profiles
                .value()
                .expect("available profiles");
            assert_eq!(profiles.len(), territories.len());
            for profile in profiles {
                assert!(!profile.cell_type_fractions.is_empty());
                assert!(
                    (profile
                        .cell_type_fractions
                        .iter()
                        .map(|entry| entry.fraction)
                        .sum::<f64>()
                        - 1.0)
                        .abs()
                        < 1e-12
                );
            }
            group.bench_with_input(BenchmarkId::new(name, n), &input, |b, input| {
                b.iter(|| {
                    black_box(engine.analyze_run(black_box(input))).expect("multimodal analysis")
                });
            });
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_spatial_stages,
    bench_complete_marked_and_output,
    bench_complete_multimodal
);
criterion_main!(benches);
