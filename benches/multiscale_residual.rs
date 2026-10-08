use criterion::{criterion_group, criterion_main, Criterion};
use marklab::{AnalysisConfig, AnalysisEngine, Pattern, PatternMeta, ThreadSetting};
use std::hint::black_box;

fn bench_marked_analysis_multiscale_residual_grid1024(c: &mut Criterion) {
    let full = std::env::var("MARKLAB_BENCH_PROFILE").as_deref() == Ok("full");
    let side = if full { 1024 } else { 64 };
    let n = if full { 1_000 } else { 250 };
    let mut x = (0..n)
        .map(|index| ((index * 37) % side) as f64)
        .collect::<Vec<_>>();
    let mut y = (0..n)
        .map(|index| ((index * 61) % side) as f64)
        .collect::<Vec<_>>();
    x[0] = 0.0;
    y[0] = 0.0;
    x[1] = (side - 1) as f64;
    y[1] = (side - 1) as f64;
    let mut pattern = Pattern::from_arrays(
        x,
        y,
        (0..n).map(|index| u8::from(index % 11 == 0)).collect(),
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
    pattern.window.area_um2 = (side * side) as f64;
    pattern.window.analysis_effective_length_um = side as f64;
    pattern.window.d_nn_mean_um = 1.0;

    let mut config = AnalysisConfig::default();
    config.validation.n_min = n;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.validation.area_min_um2 = 1.0;
    config.validation.k_shell_min = 1;
    config.spectrum.k_shells = 16;
    config.spectrum.low_k_shells = 1;
    config.spectrum.anisotropy_low_k_shells = 1;
    config.permutation.b = 7;
    config.permutation.stratified = false;
    config.inference.family_wise_alpha = 0.25;
    config.periodogram.enabled = false;
    config.multiscale_residual.enabled = true;
    config.multiscale_residual.territory_detection = false;
    config.performance.threads = ThreadSetting::Count(1);
    let engine = AnalysisEngine::new(config).expect("engine");

    c.bench_function(
        &format!("marked_analysis_multiscale_residual_grid{side}"),
        |b| {
            b.iter(|| black_box(engine.analyze_pattern(black_box(&pattern))).expect("analysis"));
        },
    );
}

criterion_group!(benches, bench_marked_analysis_multiscale_residual_grid1024);
criterion_main!(benches);
