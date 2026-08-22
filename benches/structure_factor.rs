use criterion::{criterion_group, criterion_main, Criterion};
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
    pattern.window.l_eff_um = side as f64;
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

criterion_group!(benches, bench_marked_analysis_structure_factor_n10k_k1k);
criterion_main!(benches);
