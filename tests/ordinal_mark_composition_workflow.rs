#![allow(dead_code)]

#[path = "support/declared_scalar.rs"]
mod support;

use std::collections::BTreeMap;

use marklab::{
    execute_algorithm_with_store, ordinal_class_composition, ArtifactRef, ArtifactSchema,
    BinaryMarkDeclaration, CacheStatus, DeclaredScalarPatternInput, DurableProject,
    DurableProjectLimits, LocalScheduler, MarkTable, MeasurementStatus, MissingnessPolicy,
    NativeRuntimeProvenance, NodeId, OrdinalClassCompositionAnalysisNode,
    OrdinalClassCompositionConfig, OrdinalClassCompositionError, OrdinalClassCompositionLimits,
    OrdinalMarkDeclaration, ScalarMarkColumn, ScalarMarkId, ScalarMarkModality, ScalarMarkUnit,
    SchedulerLimits, WorkflowGraph,
};
use support::{binary_metadata, fixture, publish_record, MARK_SCHEMA};

const PYTHON_ORACLE: &str = include_str!("fixtures/ordinal_composition/python_oracle.json");

fn ordinal_metadata(mark_id: &str, levels: &[&str]) -> BTreeMap<String, String> {
    let digest = marklab::ContentDigest::from_framed(levels.iter().map(|level| level.as_bytes()));
    BTreeMap::from([
        ("levels_count".into(), levels.len().to_string()),
        ("levels_digest".into(), digest.to_string()),
        ("mark_id".into(), mark_id.into()),
        ("mark_label".into(), "Ordinal IHC intensity".into()),
        ("measurement_status".into(), "measured".into()),
        ("modality".into(), "immunohistochemistry".into()),
        ("unit".into(), "ordinal".into()),
        ("value_kind".into(), "ordinal".into()),
    ])
}

fn make_table(fixture: &mut support::Fixture, values: Vec<u32>) -> (MarkTable, ScalarMarkId) {
    let binary_provenance = publish_record(
        fixture,
        b"ordinal-binary",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        binary_metadata(
            "mmr_loss",
            "MMR loss",
            MeasurementStatus::Measured,
            "independent",
        ),
    );
    let levels = ["negative", "weak", "moderate", "strong"];
    let mark_id = ScalarMarkId::new("ihc_intensity_grade").expect("ordinal mark ID");
    let ordinal_provenance = publish_record(
        fixture,
        b"ordinal-mark",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        ordinal_metadata(mark_id.as_str(), &levels),
    );
    let table = MarkTable::new(
        fixture.cell_ids.clone(),
        vec![
            ScalarMarkColumn::binary(
                BinaryMarkDeclaration::independent(
                    ScalarMarkId::new("mmr_loss").expect("binary mark ID"),
                    "MMR loss",
                    MeasurementStatus::Measured,
                    binary_provenance,
                )
                .expect("binary declaration"),
                ScalarMarkModality::Immunohistochemistry,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                fixture.pattern.mark.clone(),
            )
            .expect("binary column"),
            ScalarMarkColumn::ordinal(
                OrdinalMarkDeclaration::new(
                    mark_id.clone(),
                    "Ordinal IHC intensity",
                    levels.iter().map(|level| (*level).into()).collect(),
                    MeasurementStatus::Measured,
                    ordinal_provenance,
                )
                .expect("ordinal declaration"),
                ScalarMarkModality::Immunohistochemistry,
                ScalarMarkUnit::Ordinal,
                MissingnessPolicy::NotPermitted,
                values,
            )
            .expect("ordinal column"),
        ],
        4,
        fixture.cell_ids.iter().map(|id| id.as_str().len()).sum(),
    )
    .expect("ordinal MarkTable");
    (table, mark_id)
}

fn limits(bytes: usize) -> OrdinalClassCompositionLimits {
    OrdinalClassCompositionLimits::new(4, 4, bytes).expect("ordinal limits")
}

fn runtime() -> NativeRuntimeProvenance {
    NativeRuntimeProvenance::new(
        "0.0.0-test",
        None,
        None,
        "rustc 1.96.0-test",
        vec!["test".into()],
        ArtifactRef::from_bytes("application/vnd.marklab.executable", b"ordinal-test")
            .expect("executable"),
    )
    .expect("runtime")
}

fn run_once(path: &std::path::Path, maximum_bytes: usize) -> (CacheStatus, usize) {
    let mut fixture = fixture();
    let (table, mark_id) = make_table(&mut fixture, vec![0, 1, 1, 3]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared ordinal input");
    let config = OrdinalClassCompositionConfig::new(limits(maximum_bytes));
    let node = OrdinalClassCompositionAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("ordinal-class-composition").expect("node ID"),
        &input,
        &mark_id,
        &config,
    )
    .expect("ordinal node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1 << 20,
    })
    .expect("scheduler");
    let mut durable = DurableProject::open_or_create(
        path,
        DurableProjectLimits::new(64 * 1024, 1 << 20, 64, 64 * 1024, 1 << 20)
            .expect("durable limits"),
    )
    .expect("durable project");
    let run = execute_algorithm_with_store(
        &mut durable,
        &mut fixture.project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.ordinal_class_composition", 1).expect("schema"),
        runtime(),
        &fixture.store,
    )
    .expect("ordinal run");
    (run.cache_status, durable.execution_count())
}

#[test]
fn ordered_rows_produce_counts_cdf_entropy_and_median_interval_without_interval_arithmetic() {
    let mut fixture = fixture();
    let (table, mark_id) = make_table(&mut fixture, vec![0, 1, 1, 3]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared ordinal input");
    let result = ordinal_class_composition(
        &input,
        &mark_id,
        &OrdinalClassCompositionConfig::new(limits(1 << 20)),
    )
    .expect("ordinal composition");
    let oracle: serde_json::Value = serde_json::from_str(PYTHON_ORACLE).expect("Python oracle");
    assert_eq!(
        serde_json::to_value(&result.level_ids).unwrap(),
        oracle["levels"]
    );
    assert_eq!(
        serde_json::to_value(&result.counts).unwrap(),
        oracle["counts"]
    );
    assert_eq!(
        serde_json::to_value(&result.proportions).unwrap(),
        oracle["proportions"]
    );
    assert_eq!(
        serde_json::to_value(&result.cumulative_proportions).unwrap(),
        oracle["cumulative_proportions"]
    );
    assert_eq!(result.lower_median_level, oracle["lower_median_level"]);
    assert_eq!(result.upper_median_level, oracle["upper_median_level"]);
    assert!((result.entropy_nats - oracle["entropy_nats"].as_f64().unwrap()).abs() < 1e-15);
    assert_eq!(
        result.normalized_entropy,
        oracle["normalized_entropy"].as_f64().unwrap()
    );
    assert_eq!(
        result.effective_level_count,
        oracle["effective_level_count"].as_f64().unwrap()
    );

    assert!(matches!(
        ScalarMarkColumn::ordinal(
            OrdinalMarkDeclaration::new(
                ScalarMarkId::new("bad_ordinal").expect("mark ID"),
                "Bad ordinal",
                vec!["low".into(), "high".into()],
                MeasurementStatus::Measured,
                "0000000000000000000000000000000000000000000000000000000000000000"
                    .parse()
                    .expect("artifact ID"),
            )
            .expect("declaration"),
            ScalarMarkModality::Immunohistochemistry,
            ScalarMarkUnit::Ordinal,
            MissingnessPolicy::NotPermitted,
            vec![0, 2, 0, 1],
        ),
        Err(marklab::DeclaredScalarInputError::InvalidOrdinalValue { row: 1, .. })
    ));
    assert!(matches!(
        OrdinalMarkDeclaration::new(
            ScalarMarkId::new("duplicate_ordinal").expect("mark ID"),
            "Duplicate ordinal",
            vec!["low".into(), "low".into()],
            MeasurementStatus::Measured,
            "0000000000000000000000000000000000000000000000000000000000000000"
                .parse()
                .expect("artifact ID"),
        ),
        Err(marklab::DeclaredScalarInputError::InvalidOrdinalLevels)
    ));
    assert!(matches!(
        ordinal_class_composition(
            &input,
            &mark_id,
            &OrdinalClassCompositionConfig::new(
                OrdinalClassCompositionLimits::new(3, 4, 1 << 20).expect("point limits"),
            ),
        ),
        Err(OrdinalClassCompositionError::PointLimitExceeded {
            observed: 4,
            maximum: 3,
        })
    ));
    assert!(matches!(
        ordinal_class_composition(
            &input,
            &mark_id,
            &OrdinalClassCompositionConfig::new(
                OrdinalClassCompositionLimits::new(4, 3, 1 << 20).expect("level limits"),
            ),
        ),
        Err(OrdinalClassCompositionError::LevelLimitExceeded {
            observed: 4,
            maximum: 3,
        })
    ));
    assert!(matches!(
        ordinal_class_composition(
            &input,
            &mark_id,
            &OrdinalClassCompositionConfig::new(limits(result.estimated_storage_bytes - 1)),
        ),
        Err(OrdinalClassCompositionError::RetainedByteLimitExceeded { required, maximum })
            if required == maximum + 1
    ));
}

#[test]
fn ordinal_composition_reopens_as_a_hit_and_limit_changes_identity() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("project");
    assert_eq!(run_once(&path, 1 << 20), (CacheStatus::Miss, 1));
    assert_eq!(run_once(&path, 1 << 20), (CacheStatus::Hit, 1));
    assert_eq!(run_once(&path, (1 << 20) - 1), (CacheStatus::Miss, 2));
}
