use super::{
    bayes, cache_status_name, invalid, source_artifact, ArtifactRef, CacheKeyMaterial, CliError,
    ContentDigest, MarkedPatternResult, MarkedPrePostNode, MarkedPrepostProjectArgs, NodeError,
    NodeId, NodeSpec, Path, PathBuf, ResultDocument, Session, WorkflowGraph, WorkflowNode,
    MARKED_RESULT_KIND, MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES,
};

pub(super) fn run_marked_prepost(args: MarkedPrepostProjectArgs) -> Result<(), CliError> {
    let MarkedPrepostProjectArgs {
        project: project_path,
        pre: pre_path,
        post: post_path,
        out: output_path,
    } = args;
    let pre_path = result_document_path(pre_path);
    let post_path = result_document_path(post_path);
    let pre_source = source_artifact(&pre_path, MARKED_RESULT_KIND)?;
    let post_source = source_artifact(&post_path, MARKED_RESULT_KIND)?;
    let pre = MarkedResultImportNode::new("marked-pre-import", pre_path, pre_source.clone())?;
    let post = MarkedResultImportNode::new("marked-post-import", post_path, post_source.clone())?;

    let mut session = Session::open(
        &project_path,
        &[pre_source, post_source],
        MAXIMUM_RESULT_BYTES,
    )?;
    let import_graph =
        WorkflowGraph::new([pre.spec().clone(), post.spec().clone()]).map_err(invalid)?;
    let import_schema = ("marklab.marked_pattern_result", 1);
    let pre_run = session.run(&import_graph, &pre, import_schema)?;
    let post_run = session.run(&import_graph, &post, import_schema)?;
    let comparison = MarkedPrePostNode::new(
        NodeId::new("marked-prepost").map_err(invalid)?,
        pre.spec().id().clone(),
        &pre_run,
        post.spec().id().clone(),
        &post_run,
    )
    .map_err(invalid)?;
    let graph = WorkflowGraph::new([
        pre.spec().clone(),
        post.spec().clone(),
        comparison.spec().clone(),
    ])
    .map_err(invalid)?;
    let comparison_run = session.run(&graph, &comparison, ("marklab.marked_prepost_result", 1))?;

    bayes::publish_json(
        &output_path,
        &ResultDocument::marked_prepost(comparison_run.output),
    )?;
    eprintln!(
        "project marked-prepost cache_status: pre={} post={} comparison={}",
        cache_status_name(pre_run.cache_status),
        cache_status_name(post_run.cache_status),
        cache_status_name(comparison_run.cache_status)
    );
    Ok(())
}

fn result_document_path(path: PathBuf) -> PathBuf {
    if path.is_dir() {
        path.join("result.json")
    } else {
        path
    }
}

struct MarkedResultImportNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    output: MarkedPatternResult,
}

impl MarkedResultImportNode {
    fn new(node_id: &str, input_path: PathBuf, input: ArtifactRef) -> Result<Self, CliError> {
        let output = read_marked_result(&input_path)?;
        let observed = source_artifact(&input_path, MARKED_RESULT_KIND)?;
        if observed != input {
            return Err(CliError::Input(format!(
                "marked result changed while its durable import was prepared: {}",
                input_path.display()
            )));
        }
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(node_id).map_err(|error| CliError::Input(error.to_string()))?,
                "marked_result_import",
                1,
                Vec::new(),
            )
            .map_err(|error| CliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            output,
        })
    }
}

impl WorkflowNode for MarkedResultImportNode {
    type Output = MarkedPatternResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed =
            source_artifact(&self.input_path, MARKED_RESULT_KIND).map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(CliError::Input(format!(
                "marked result no longer matches the one recorded in the project: {}",
                self.input_path.display()
            ))));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_bytes(
                b"marklab-marked-result-import-configuration-v1",
            ),
            execution_policy: b"bounded-typed-result-import-v1",
            implementation_identity: "marklab-project-marked-result-import-v2",
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        Ok(self.output.clone())
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        ResultDocument::marked(output.clone())
            .to_json_pretty()
            .map(String::into_bytes)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let json = std::str::from_utf8(bytes).map_err(NodeError::decode)?;
        ResultDocument::from_json(json)
            .and_then(ResultDocument::into_marked_pattern)
            .map_err(NodeError::decode)
    }

    fn output_kind(&self) -> &'static str {
        MARKED_RESULT_KIND
    }
}

fn read_marked_result(path: &Path) -> Result<MarkedPatternResult, CliError> {
    let bytes = crate::input_file::read_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        &format!(
            "marked result must be a regular file within {MAXIMUM_INPUT_BYTES} bytes: {}",
            path.display()
        ),
    )?;
    let json = std::str::from_utf8(&bytes)
        .map_err(|error| CliError::Input(format!("invalid result UTF-8: {error}")))?;
    ResultDocument::from_json(json)
        .and_then(ResultDocument::into_marked_pattern)
        .map_err(|error| CliError::Input(error.to_string()))
}
