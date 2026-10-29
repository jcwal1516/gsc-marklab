use super::durable;

use super::{
    topology, CliError, Codec, ContentDigest, Cow, NodeContract, NodeError, Source, SourceNode,
    WitnessPersistenceBottleneckStabilityProjectArgs, WitnessPersistenceBottleneckStabilityResult,
    WitnessPersistenceProjectArgs, WitnessPersistenceResult,
    WitnessPersistenceStabilityProjectArgs, WitnessPersistenceStabilityResult,
    MAXIMUM_RESULT_BYTES, MAXIMUM_WITNESS_RESULT_BYTES,
};

pub(super) fn run_witness_persistence(args: WitnessPersistenceProjectArgs) -> Result<(), CliError> {
    let WitnessPersistenceProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    const LABEL: &str = "witness persistence input";
    let source = Source::identify(
        input_path,
        "application/vnd.marklab.source.witness-persistence+json;version=1",
    )?;
    let prepared = topology::prepare_witness_persistence(source.path())?;
    source.ensure_unchanged(LABEL)?;
    let contract = NodeContract {
        id: "gudhi-witness-persistence",
        kind: "topological_witness_approximation",
        version: 1,
        configuration_digest: ContentDigest::from_framed([
            b"marklab-project-gudhi-witness-persistence-node-v1".as_slice(),
            prepared.request_bytes.as_slice(),
        ]),
        execution_policy: Cow::Borrowed(b"bounded-pinned-gudhi-subprocess-v1"),
        implementation_identity: "marklab-project-gudhi-witness-persistence-node-v1",
        output_kind: "application/vnd.marklab.gudhi-witness-persistence-result+json;version=1",
        codec: Codec::ExactFloatJson,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| topology::execute_witness_persistence(prepared).map_err(NodeError::execution),
        |prepared, output: &WitnessPersistenceResult| {
            output
                .validate(&prepared.request, &prepared.request_bytes)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        ("marklab.gudhi_witness_persistence_result", 1),
        MAXIMUM_WITNESS_RESULT_BYTES,
    )?;
    durable::publish(
        "witness-persistence",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

pub(super) fn run_witness_persistence_stability(
    args: WitnessPersistenceStabilityProjectArgs,
) -> Result<(), CliError> {
    let WitnessPersistenceStabilityProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    const LABEL: &str = "witness persistence stability input";
    let source = Source::identify(
        input_path,
        "application/vnd.marklab.source.witness-persistence-stability+json;version=1",
    )?;
    let prepared = topology::prepare_witness_persistence_stability(source.path())?;
    source.ensure_unchanged(LABEL)?;
    let mut frames = vec![b"marklab-project-witness-persistence-stability-node-v1".as_slice()];
    frames.extend(prepared.runs.iter().map(|run| run.request_bytes.as_slice()));
    let contract = NodeContract {
        id: "gudhi-witness-persistence-stability",
        kind: "topological_witness_coordinate_stability",
        version: 1,
        configuration_digest: ContentDigest::from_framed(frames),
        execution_policy: Cow::Borrowed(b"bounded-gudhi-witness-persistence-stability-v1"),
        implementation_identity: "marklab-project-witness-persistence-stability-node-v1",
        output_kind:
            "application/vnd.marklab.gudhi-witness-persistence-stability-result+json;version=1",
        codec: Codec::ExactFloatJson,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| {
            topology::execute_witness_persistence_stability(prepared).map_err(NodeError::execution)
        },
        |prepared, output: &WitnessPersistenceStabilityResult| {
            topology::validate_witness_persistence_stability_result(prepared, output)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        ("marklab.gudhi_witness_persistence_stability_result", 1),
        MAXIMUM_WITNESS_RESULT_BYTES,
    )?;
    durable::publish(
        "witness-persistence-stability",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

pub(super) fn run_witness_persistence_bottleneck_stability(
    args: WitnessPersistenceBottleneckStabilityProjectArgs,
) -> Result<(), CliError> {
    let WitnessPersistenceBottleneckStabilityProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    const LABEL: &str = "witness bottleneck stability input";
    let source = Source::identify(
        input_path,
        "application/vnd.marklab.source.witness-persistence-bottleneck-stability+json;version=1",
    )?;
    let prepared = topology::prepare_witness_persistence_bottleneck_stability(source.path())?;
    source.ensure_unchanged(LABEL)?;
    let mut frames =
        vec![b"marklab-project-witness-persistence-bottleneck-stability-node-v1".as_slice()];
    frames.extend(
        prepared
            .stability
            .runs
            .iter()
            .map(|run| run.request_bytes.as_slice()),
    );
    frames.push(prepared.backend.environment_lock_sha256.as_bytes());
    frames.push(prepared.backend.worker_sha256.as_bytes());
    let contract = NodeContract {
        id: "gudhi-witness-persistence-bottleneck-stability",
        kind: "topological_witness_coordinate_bottleneck_stability",
        version: 1,
        configuration_digest: ContentDigest::from_framed(frames),
        execution_policy: Cow::Borrowed(
            b"bounded-gudhi-witness-persistence-bottleneck-stability-v1",
        ),
        implementation_identity: "marklab-project-witness-persistence-bottleneck-stability-node-v1",
        output_kind: "application/vnd.marklab.gudhi-witness-persistence-bottleneck-stability-result+json;version=1",
        codec: Codec::ExactFloatJson,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| {
            topology::execute_witness_persistence_bottleneck_stability(prepared)
                .map_err(NodeError::execution)
        },
        |prepared, output: &WitnessPersistenceBottleneckStabilityResult| {
            topology::validate_witness_persistence_bottleneck_stability_result(prepared, output)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        (
            "marklab.gudhi_witness_persistence_bottleneck_stability_result",
            1,
        ),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "witness-persistence-bottleneck-stability",
        &output_path,
        &run.output,
        run.cache_status,
    )
}
