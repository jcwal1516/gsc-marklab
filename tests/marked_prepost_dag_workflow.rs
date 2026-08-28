use std::{fs, path::Path, process::Command};

use marklab::{
    compare_marked_prepost, execute_marked_prepost_dag, plan_marked_prepost_dag, AnalysisConfig,
    ArtifactRef, CacheStatus, ContentDigest, DurableProjectLimits, MarkedPrePostDagError,
    MarkedPrePostDagLimits, MarkedPrePostDagTarget, NativeRuntimeProvenance, Pattern, PatternMeta,
    SchedulerLimits, ThreadSetting,
};

const CHILD_ROOT: &str = "MARKLAB_MARKED_DAG_CHILD_ROOT";
const CHILD_OUTPUT: &str = "MARKLAB_MARKED_DAG_CHILD_OUTPUT";
const CHILD_TARGET: &str = "MARKLAB_MARKED_DAG_CHILD_TARGET";

#[test]
fn bounded_marked_dag_resumes_then_replays_across_processes() {
    let directory = tempfile::tempdir().expect("temporary DAG root");
    let executable = std::env::current_exe().expect("current test executable");
    let mut receipts = Vec::new();
    for (index, target) in ["roots", "comparison", "comparison"].iter().enumerate() {
        let output = directory.path().join(format!("receipt-{index}.json"));
        let status = Command::new(&executable)
            .arg("--exact")
            .arg("bounded_marked_dag_child")
            .env(CHILD_ROOT, directory.path().join("project"))
            .env(CHILD_OUTPUT, &output)
            .env(CHILD_TARGET, target)
            .status()
            .expect("fresh DAG process");
        assert!(status.success());
        receipts.push(
            serde_json::from_slice::<serde_json::Value>(&fs::read(output).expect("receipt"))
                .expect("receipt JSON"),
        );
    }

    assert_eq!(
        receipts[0]["cache"],
        serde_json::json!(["miss", "miss", null])
    );
    assert_eq!(
        receipts[0]["execution_counts"],
        serde_json::json!([1, 1, 0])
    );
    assert_eq!(
        receipts[1]["cache"],
        serde_json::json!(["hit", "hit", "miss"])
    );
    assert_eq!(
        receipts[1]["execution_counts"],
        serde_json::json!([1, 1, 1])
    );
    assert_eq!(
        receipts[2]["cache"],
        serde_json::json!(["hit", "hit", "hit"])
    );
    assert_eq!(
        receipts[2]["execution_counts"],
        serde_json::json!([1, 1, 1])
    );
    assert_eq!(receipts[0]["root_parallelism"], 2);
    assert_eq!(receipts[0]["executed_waves"], 1);
    assert_eq!(receipts[1]["executed_waves"], 2);
    assert_eq!(
        receipts[1]["comparison_digest"],
        receipts[2]["comparison_digest"]
    );
}

#[test]
fn bounded_marked_dag_child() {
    let Some(root) = std::env::var_os(CHILD_ROOT) else {
        return;
    };
    let output = std::env::var_os(CHILD_OUTPUT).expect(CHILD_OUTPUT);
    let target = match std::env::var(CHILD_TARGET).expect(CHILD_TARGET).as_str() {
        "roots" => MarkedPrePostDagTarget::RootAnalyses,
        "comparison" => MarkedPrePostDagTarget::Comparison,
        value => panic!("unsupported target {value}"),
    };
    let (pre, post, config) = fixture();
    let limits = MarkedPrePostDagLimits::new(2, 2, 128, 8).expect("DAG limits");
    let plan = plan_marked_prepost_dag(&pre, &post, &config, limits).expect("resource plan");
    assert_eq!(plan.root_parallelism(), 2);
    assert_eq!(plan.wave_count(), 2);
    let run = execute_marked_prepost_dag(
        Path::new(&root),
        &pre,
        &post,
        &config,
        target,
        limits,
        DurableProjectLimits::new(64 * 1024, 1 << 20, 16, 64 * 1024, 1 << 20)
            .expect("durable limits"),
        SchedulerLimits {
            max_inline_output_bytes: 1 << 20,
        },
        runtime(),
    )
    .expect("DAG execution");
    if let Some(comparison) = &run.comparison {
        assert_eq!(
            comparison.output,
            compare_marked_prepost(&run.pre.output, &run.post.output)
        );
    }
    let comparison_digest = run.comparison.as_ref().map(|comparison| {
        ContentDigest::from_bytes(format!("{:?}", comparison.output).as_bytes()).to_string()
    });
    let receipt = serde_json::json!({
        "cache": [
            cache_name(run.pre.cache_status),
            cache_name(run.post.cache_status),
            run.comparison.as_ref().map(|value| cache_name(value.cache_status)),
        ],
        "execution_counts": run.durable_execution_counts,
        "root_parallelism": run.plan.root_parallelism(),
        "executed_waves": run.executed_wave_count,
        "comparison_digest": comparison_digest,
    });
    fs::write(output, serde_json::to_vec(&receipt).expect("receipt JSON")).expect("write receipt");
}

#[test]
fn resource_plan_serializes_roots_when_parallel_budget_is_one() {
    let (pre, post, config) = fixture();
    let plan = plan_marked_prepost_dag(
        &pre,
        &post,
        &config,
        MarkedPrePostDagLimits::new(1, 1, 64, 8).expect("limits"),
    )
    .expect("serial plan");
    assert_eq!(plan.root_parallelism(), 1);
    assert_eq!(plan.wave_count(), 3);
}

#[test]
fn resource_plan_rejects_unbounded_threads_and_one_root_over_budget() {
    let (pre, post, mut config) = fixture();
    config.performance.threads = ThreadSetting::Auto;
    assert!(matches!(
        plan_marked_prepost_dag(
            &pre,
            &post,
            &config,
            MarkedPrePostDagLimits::new(2, 2, 128, 8).expect("limits"),
        ),
        Err(MarkedPrePostDagError::UnboundedThreadSetting)
    ));
    config.performance.threads = ThreadSetting::Count(2);
    config.performance.memory_budget_mib = 65;
    assert!(matches!(
        plan_marked_prepost_dag(
            &pre,
            &post,
            &config,
            MarkedPrePostDagLimits::new(2, 1, 64, 8).expect("limits"),
        ),
        Err(MarkedPrePostDagError::RootResourceLimitExceeded)
    ));
    config.performance.threads = ThreadSetting::Count(1);
    config.performance.memory_budget_mib = 64;
    assert!(matches!(
        plan_marked_prepost_dag(
            &pre,
            &post,
            &config,
            MarkedPrePostDagLimits::new(2, 2, 128, 7).expect("limits"),
        ),
        Err(MarkedPrePostDagError::PatternRowLimitExceeded {
            observed: 8,
            maximum: 7,
        })
    ));
}

fn fixture() -> (Pattern, Pattern, AnalysisConfig) {
    let pattern = |timepoint: &str, marks| {
        let mut pattern = Pattern::from_arrays(
            vec![0.0, 1.0, 0.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
            marks,
            PatternMeta {
                case_id: "dag-case".into(),
                timepoint: timepoint.into(),
                protein: "MSH6".into(),
                slide_id: None,
                section_id: None,
                stain_batch: None,
                block_id: None,
                region_id: None,
            },
        )
        .expect("pattern");
        pattern.window.area_um2 = 4.0;
        pattern.window.analysis_effective_length_um = 2.0;
        pattern.window.d_nn_mean_um = 1.0;
        pattern
    };
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 4;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.validation.area_min_um2 = 1.0;
    config.validation.k_shell_min = 1;
    config.spectrum.k_shells = 4;
    config.spectrum.low_k_shells = 1;
    config.spectrum.anisotropy_low_k_shells = 1;
    config.spectrum.fit_low_k_alpha = false;
    config.periodogram.enabled = false;
    config.multiscale_residual.enabled = false;
    config.permutation.b = 19;
    config.permutation.stratified = false;
    config.performance.threads = ThreadSetting::Count(1);
    config.performance.memory_budget_mib = 64;
    config.performance.strict_repro = true;
    (
        pattern("pre", vec![1, 0, 0, 1]),
        pattern("post", vec![1, 1, 0, 0]),
        config,
    )
}

fn cache_name(status: CacheStatus) -> &'static str {
    match status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    }
}

fn runtime() -> NativeRuntimeProvenance {
    NativeRuntimeProvenance::new(
        "0.1.0-test",
        None,
        None,
        "rustc 1.96.0-test",
        vec!["marked-prepost-dag".into()],
        ArtifactRef::from_bytes(
            "application/vnd.marklab.executable",
            b"marked-prepost-dag-test-v1",
        )
        .expect("executable"),
    )
    .expect("runtime")
}
