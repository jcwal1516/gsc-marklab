use criterion::{criterion_group, criterion_main, Criterion};
use marklab_bayes::{fit_grouped_conformal, GroupedConformalSpec};
use std::hint::black_box;

fn bench_grouped_conformal(c: &mut Criterion) {
    let cases = [("30x2", "small"), ("300x8", "representative")];
    for (name, fixture) in cases {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("crates/marklab-bayes/tests/fixtures/grouped_conformal");
        let raw = std::fs::read_to_string(root.join(format!("{fixture}.spec.json")))
            .expect("workspace benchmark fixture");
        let oracle = std::fs::read_to_string(root.join(format!("{fixture}.oracle.json")))
            .expect("workspace oracle");
        let spec: GroupedConformalSpec = serde_json::from_str(&raw).expect("declared fixture");
        let oracle: serde_json::Value =
            serde_json::from_str(&oracle).expect("frozen Python oracle");
        let fit = fit_grouped_conformal(spec.clone()).expect("converged native fixture");
        assert_eq!(
            fit.coverage.overall.covered,
            oracle["coverage"]["overall"]["covered"].as_u64().unwrap() as u32
        );
        assert_eq!(
            fit.predictions.len(),
            oracle["predictions"].as_array().unwrap().len()
        );
        assert!(
            (fit.nonconformity_threshold - oracle["nonconformity_threshold"].as_f64().unwrap())
                .abs()
                <= 2e-7
        );
        c.bench_function(&format!("grouped_conformal_{name}_owned_input"), |b| {
            b.iter(|| {
                black_box(fit_grouped_conformal(black_box(spec.clone())).expect("native fit"))
            })
        });
    }
}
criterion_group!(benches, bench_grouped_conformal);
criterion_main!(benches);
