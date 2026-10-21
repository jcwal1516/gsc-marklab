use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use marklab::{AnalysisConfig, AnalysisEngine, Pattern, PatternMeta, ThreadSetting};
use std::hint::black_box;

fn representative_pattern(n: usize) -> Pattern {
    let side = (n as f64).sqrt().ceil() as usize;
    let mut pattern = Pattern::from_arrays(
        (0..n).map(|i| (i % side) as f64).collect(),
        (0..n).map(|i| (i / side) as f64).collect(),
        (0..n).map(|i| u8::from(i % 11 == 0)).collect(),
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
    .expect("pattern");
    pattern.window.area_um2 = n as f64;
    pattern.window.analysis_effective_length_um = side as f64;
    pattern.window.d_nn_mean_um = 1.0;
    pattern
}

fn bench_marked_analysis_structure_factor_n10k_k1k(c: &mut Criterion) {
    let full = std::env::var("MARKLAB_BENCH_PROFILE").as_deref() == Ok("full");
    let n = if full { 10_000 } else { 1_000 };
    let pattern = representative_pattern(n);
    let mut config = AnalysisConfig::default();
    config.validation.n_min = n;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.validation.area_min_um2 = 1.0;
    config.validation.k_shell_min = 1;
    config.spectrum.k_shells = if full { 1_000 } else { 32 };
    config.spectrum.low_k_shells = 1;
    config.spectrum.anisotropy_low_k_shells = 1;
    config.permutation.b = 7;
    config.inference.family_wise_alpha = 0.25;
    config.permutation.stratified = false;
    config.periodogram.enabled = false;
    config.multiscale_residual.enabled = false;
    config.performance.threads = ThreadSetting::Count(1);
    let engine = AnalysisEngine::new(config).expect("engine");

    c.bench_function(
        &format!(
            "marked_analysis_structure_factor_n{n}_k{}",
            if full { 1_000 } else { 32 }
        ),
        |b| {
            b.iter(|| black_box(engine.analyze_pattern(black_box(&pattern))).expect("analysis"));
        },
    );
}

fn bench_spectrum_permutations(c: &mut Criterion) {
    let full = std::env::var("MARKLAB_BENCH_PROFILE").as_deref() == Ok("full");
    let chunk_sizes: &[usize] = if full { &[1, 64, 256, 4_096] } else { &[64] };
    let mut pattern = representative_pattern(1_024);
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 1_024;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.validation.p_min = 0.0001;
    config.validation.p_max = 0.9999;
    config.validation.area_min_um2 = 1.0;
    config.validation.k_shell_min = 1;
    config.spectrum.k_shells = 16;
    config.spectrum.low_k_shells = 2;
    config.spectrum.anisotropy_low_k_shells = 2;
    config.permutation.b = 99;
    config.permutation.stratified = false;
    config.periodogram.enabled = false;
    config.multiscale_residual.enabled = false;
    config.performance.threads = ThreadSetting::Count(1);
    config.performance.k_chunk_modes = 64;
    let mut group = c.benchmark_group("spectrum_permutations_n1024_b99");
    for (name, every, probabilistic) in [
        ("balanced_binary", 2, false),
        ("sparse_binary", 1_024, false),
        ("continuous", 2, true),
    ] {
        for (index, mark) in pattern.mark.iter_mut().enumerate() {
            *mark = u8::from(index % every == 0);
        }
        config.analysis.use_probabilistic_marks = probabilistic;
        pattern.mark_prob = probabilistic.then(|| {
            (0..pattern.len())
                .map(|index| (index % 17) as f32 / 16.0)
                .collect()
        });
        let engine = AnalysisEngine::new(config.clone()).expect("engine");
        let expected = engine.analyze_pattern(&pattern).expect("analysis");
        assert!(!expected.spectrum_curve.is_empty());
        // Keep a fixed 64-mode reference for every chunk size. This checks
        // that the sweep measures identical seeded scientific results.
        for &chunk_size in chunk_sizes {
            let mut chunk_config = config.clone();
            chunk_config.performance.k_chunk_modes = chunk_size;
            let chunk_engine = AnalysisEngine::new(chunk_config).expect("chunk-size engine");
            let actual = chunk_engine
                .analyze_pattern(&pattern)
                .expect("chunk-size analysis");
            assert_eq!(actual.spectrum, expected.spectrum);
            assert_eq!(actual.spectrum_curve, expected.spectrum_curve);
            group.bench_function(BenchmarkId::new(name, chunk_size), |b| {
                b.iter(|| {
                    let actual = chunk_engine
                        .analyze_pattern(black_box(&pattern))
                        .expect("analysis");
                    assert_eq!(actual.spectrum, expected.spectrum);
                    assert_eq!(actual.spectrum_curve, expected.spectrum_curve);
                    black_box(actual)
                })
            });
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_marked_analysis_structure_factor_n10k_k1k,
    bench_spectrum_permutations
);
criterion_main!(benches);
