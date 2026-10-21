use crate::workflow_node::{NodeAnalysis, TypedNode};
use std::io::Write;

use marklab_project::{ArtifactRef, ContentDigest, MarklabProject};
use marklab_workflow::{NodeError, NodeId, NodeRun, NodeSpec};

use crate::{
    compare_marked_prepost,
    scalar_mark::{DeclaredMarkUse, DeclaredScalarIdentity, DeclaredScalarPatternInput},
    AnalysisConfig, AnalysisEngine, MarkedPatternResult, Pattern, PrePostResult, ResultDocument,
    ThreadSetting, RESULT_FORMAT_VERSION,
};

const PATTERN_KIND: &str = "application/vnd.marklab.compat-pattern+json";
const CONFIG_KIND: &str = "application/vnd.marklab.analysis-config+toml;version=0.2";
const RESULT_KIND: &str = "application/vnd.marklab.result+json;version=0.4";
const ADAPTER_REVISION: &str = "marked-analysis-v6";
const DECLARED_ADAPTER_REVISION: &str = "declared-marked-analysis-v6";
const PREPOST_ADAPTER_REVISION: &str = "marked-prepost-v1";

/// Marked analysis as one typed workflow node.
pub struct MarkedAnalysisNode<'a> {
    node: TypedNode<MarkedAnalysis<'a>>,
}

struct MarkedAnalysis<'a> {
    pattern: &'a Pattern,
    config: &'a AnalysisConfig,
}

impl<'a> MarkedAnalysisNode<'a> {
    /// Catalog immutable input references and construct the compatibility node.
    ///
    /// The cache policy records the requested thread setting. In this first
    /// in-memory slice, `auto` is intentionally the literal declared policy,
    /// not a snapshot of ambient host capacity.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        pattern: &'a Pattern,
        config: &'a AnalysisConfig,
    ) -> Result<Self, NodeError> {
        let pattern_artifact = pattern_artifact(pattern)?;
        let config_artifact = config_artifact(config)?;
        let spec = NodeSpec::new(id, "marked_analysis", 1, Vec::new()).map_err(NodeError::input)?;
        project
            .register_reference(pattern_artifact.clone())
            .map_err(NodeError::input)?;
        project
            .register_reference(config_artifact.clone())
            .map_err(NodeError::input)?;

        let node_config_digest = config_artifact.digest();
        let node_inputs = [pattern_artifact, config_artifact];
        let node_execution_policy = execution_policy(config);
        let node_implementation_identity = format!(
            "marklab/{};adapter={ADAPTER_REVISION};result={RESULT_FORMAT_VERSION};parallel={}",
            env!("CARGO_PKG_VERSION"),
            cfg!(feature = "parallel")
        );
        Ok(Self {
            node: TypedNode {
                spec,
                analysis: MarkedAnalysis { pattern, config },
                inputs: node_inputs,
                configuration_digest: node_config_digest,
                execution_policy: std::borrow::Cow::Owned(node_execution_policy),
                implementation_identity: node_implementation_identity,
            },
        })
    }

    /// Return the versioned dependency-free specification registered in the graph.
    pub fn spec(&self) -> &NodeSpec {
        self.node.spec()
    }
}

impl NodeAnalysis for MarkedAnalysis<'_> {
    type Output = MarkedPatternResult;
    type Inputs = [ArtifactRef; 2];

    fn verify_input_content(&self, inputs: &[ArtifactRef]) -> Result<(), NodeError> {
        let pattern = pattern_artifact(self.pattern)?;
        crate::workflow_node::verify_input(inputs, 0, &pattern)?;
        let config = config_artifact(self.config)?;
        crate::workflow_node::verify_input(inputs, 1, &config)
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        AnalysisEngine::new(self.config.clone())
            .and_then(|engine| engine.analyze_pattern(self.pattern))
            .map_err(NodeError::execution)
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
        RESULT_KIND
    }
}

crate::workflow_node::impl_workflow_node!(MarkedAnalysisNode<'_> => MarkedPatternResult);

/// Runtime output of a declared scalar-pattern workflow execution or replay.
#[derive(Clone, Debug, PartialEq)]
pub struct DeclaredMarkedAnalysisResult {
    /// Exact marked-pattern result decoded from the current result format.
    pub result: MarkedPatternResult,
    /// Runtime-only routing summary reattached from the cache-bound node input.
    pub mark_use: DeclaredMarkUse,
    /// Runtime-only declared row/frame identity reattached from the cache-bound node input.
    pub scalar_identity: DeclaredScalarIdentity,
}

/// Declared scalar-pattern analysis exposed as a provenance-aware workflow node.
pub struct DeclaredMarkedAnalysisNode<'input, 'pattern> {
    node: TypedNode<DeclaredMarkedAnalysis<'input, 'pattern>>,
}

struct DeclaredMarkedAnalysis<'input, 'pattern> {
    input: &'input DeclaredScalarPatternInput<'pattern>,
    config: &'input AnalysisConfig,
    mark_use: DeclaredMarkUse,
}

impl<'input, 'pattern> DeclaredMarkedAnalysisNode<'input, 'pattern> {
    /// Catalog the compatibility pattern, declared identity, and configuration references.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'input DeclaredScalarPatternInput<'pattern>,
        config: &'input AnalysisConfig,
    ) -> Result<Self, NodeError> {
        input
            .revalidate_project(project)
            .map_err(NodeError::input)?;
        let mark_use = input
            .mark_use_for_config(
                &config.analysis.mark_label,
                config.analysis.use_probabilistic_marks,
            )
            .map_err(NodeError::input)?;
        let pattern_artifact = pattern_artifact(input.pattern())?;
        let declared_artifact = input.declared_artifact_ref().map_err(NodeError::input)?;
        let config_artifact = config_artifact(config)?;
        let spec = NodeSpec::new(id, "declared_marked_analysis", 1, Vec::new())
            .map_err(NodeError::input)?;
        for artifact in [&pattern_artifact, &declared_artifact, &config_artifact] {
            project
                .register_reference(artifact.clone())
                .map_err(NodeError::input)?;
        }
        let config_digest = config_artifact.digest();

        let node_inputs = [pattern_artifact, declared_artifact, config_artifact];
        let node_execution_policy = execution_policy(config);
        let node_implementation_identity = format!(
                "marklab/{};adapter={DECLARED_ADAPTER_REVISION};result={RESULT_FORMAT_VERSION};parallel={}",
                env!("CARGO_PKG_VERSION"),
                cfg!(feature = "parallel")
            );
        Ok(Self {
            node: TypedNode {
                spec,
                analysis: DeclaredMarkedAnalysis {
                    input,
                    config,
                    mark_use,
                },
                inputs: node_inputs,
                configuration_digest: config_digest,
                execution_policy: std::borrow::Cow::Owned(node_execution_policy),
                implementation_identity: node_implementation_identity,
            },
        })
    }

    /// Return the versioned dependency-free specification registered in the graph.
    pub fn spec(&self) -> &NodeSpec {
        self.node.spec()
    }
}

impl NodeAnalysis for DeclaredMarkedAnalysis<'_, '_> {
    type Output = DeclaredMarkedAnalysisResult;
    type Inputs = [ArtifactRef; 3];

    fn semantic_input_artifacts(&self) -> &[marklab_project::ArtifactId] {
        self.input.semantic_artifact_ids()
    }

    fn verify_input_content(&self, inputs: &[ArtifactRef]) -> Result<(), NodeError> {
        let pattern = pattern_artifact(self.input.pattern())?;
        crate::workflow_node::verify_input(inputs, 0, &pattern)?;
        let declared = self
            .input
            .declared_artifact_ref()
            .map_err(NodeError::input)?;
        crate::workflow_node::verify_input(inputs, 1, &declared)?;
        let config = config_artifact(self.config)?;
        crate::workflow_node::verify_input(inputs, 2, &config)
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        AnalysisEngine::new(self.config.clone())
            .map_err(NodeError::execution)?
            .analyze_declared_scalar_pattern(self.input)
            .map(|run| DeclaredMarkedAnalysisResult {
                result: run.result,
                mark_use: run.mark_use,
                scalar_identity: run.scalar_identity,
            })
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        ResultDocument::marked(output.result.clone())
            .to_json_pretty()
            .map(String::into_bytes)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let json = std::str::from_utf8(bytes).map_err(NodeError::decode)?;
        ResultDocument::from_json(json)
            .and_then(ResultDocument::into_marked_pattern)
            .map(|result| DeclaredMarkedAnalysisResult {
                result,
                mark_use: self.mark_use.clone(),
                scalar_identity: self.input.scalar_identity().clone(),
            })
            .map_err(NodeError::decode)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

crate::workflow_node::impl_workflow_node!(DeclaredMarkedAnalysisNode<'_, '_> => DeclaredMarkedAnalysisResult);

/// Marked pre/post comparison as a typed two-input workflow node.
pub struct MarkedPrePostNode<'a> {
    node: TypedNode<MarkedPrePostAnalysis<'a>>,
}

struct MarkedPrePostAnalysis<'a> {
    pre: &'a MarkedPatternResult,
    post: &'a MarkedPatternResult,
}

impl<'a> MarkedPrePostNode<'a> {
    /// Bind two exact upstream marked-analysis runs as the pre/post comparison inputs.
    pub fn new(
        id: NodeId,
        pre_dependency: NodeId,
        pre: &'a NodeRun<MarkedPatternResult>,
        post_dependency: NodeId,
        post: &'a NodeRun<MarkedPatternResult>,
    ) -> Result<Self, NodeError> {
        let expected_pre = marked_result_artifact(&pre.output)?;
        pre.artifact
            .verify_identity(expected_pre.digest(), expected_pre.byte_len())
            .map_err(NodeError::input)?;
        if pre.artifact.kind() != RESULT_KIND {
            return Err(NodeError::input(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "pre input is not a current-format Marklab result artifact",
            )));
        }
        let expected_post = marked_result_artifact(&post.output)?;
        post.artifact
            .verify_identity(expected_post.digest(), expected_post.byte_len())
            .map_err(NodeError::input)?;
        if post.artifact.kind() != RESULT_KIND {
            return Err(NodeError::input(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "post input is not a current-format Marklab result artifact",
            )));
        }
        let spec = NodeSpec::new(
            id,
            "marked_prepost",
            1,
            vec![pre_dependency, post_dependency],
        )
        .map_err(NodeError::input)?;
        let node_pre = &pre.output;
        let node_post = &post.output;
        let node_inputs = [pre.artifact.clone(), post.artifact.clone()];
        let node_configuration_digest =
            ContentDigest::from_bytes(b"marklab-marked-prepost-configuration-v1");
        let node_execution_policy = b"deterministic-descriptive-comparison-v1".to_vec();
        let node_implementation_identity = format!(
            "marklab/{};adapter={PREPOST_ADAPTER_REVISION};result={RESULT_FORMAT_VERSION}",
            env!("CARGO_PKG_VERSION")
        );
        Ok(Self {
            node: TypedNode {
                spec,
                analysis: MarkedPrePostAnalysis {
                    pre: node_pre,
                    post: node_post,
                },
                inputs: node_inputs,
                configuration_digest: node_configuration_digest,
                execution_policy: std::borrow::Cow::Owned(node_execution_policy),
                implementation_identity: node_implementation_identity,
            },
        })
    }

    /// Return the versioned dependency-bearing specification registered in the graph.
    pub fn spec(&self) -> &NodeSpec {
        self.node.spec()
    }
}

impl NodeAnalysis for MarkedPrePostAnalysis<'_> {
    type Output = PrePostResult;
    type Inputs = [ArtifactRef; 2];

    fn verify_input_content(&self, inputs: &[ArtifactRef]) -> Result<(), NodeError> {
        let pre = marked_result_artifact(self.pre)?;
        crate::workflow_node::verify_input(inputs, 0, &pre)?;
        let post = marked_result_artifact(self.post)?;
        crate::workflow_node::verify_input(inputs, 1, &post)
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        Ok(compare_marked_prepost(self.pre, self.post))
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        ResultDocument::marked_prepost(output.clone())
            .to_json_pretty()
            .map(String::into_bytes)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let json = std::str::from_utf8(bytes).map_err(NodeError::decode)?;
        ResultDocument::from_json(json)
            .and_then(ResultDocument::into_marked_prepost)
            .map_err(NodeError::decode)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

crate::workflow_node::impl_workflow_node!(MarkedPrePostNode<'_> => PrePostResult);

pub(crate) fn pattern_artifact(pattern: &Pattern) -> Result<ArtifactRef, NodeError> {
    let mut writer = ContentDigest::builder();
    serde_json::to_writer(&mut writer, pattern).map_err(NodeError::input)?;
    writer.flush().map_err(NodeError::input)?;
    let (digest, byte_len) = writer.finish();
    ArtifactRef::new(PATTERN_KIND, digest, byte_len).map_err(NodeError::input)
}

fn config_artifact(config: &AnalysisConfig) -> Result<ArtifactRef, NodeError> {
    let encoded = toml::to_string(config).map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, encoded.as_bytes()).map_err(NodeError::input)
}

fn marked_result_artifact(result: &MarkedPatternResult) -> Result<ArtifactRef, NodeError> {
    let encoded = ResultDocument::marked(result.clone())
        .to_json_pretty()
        .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(RESULT_KIND, encoded.as_bytes()).map_err(NodeError::input)
}

fn execution_policy(config: &AnalysisConfig) -> Vec<u8> {
    let threads = match config.performance.threads {
        ThreadSetting::Auto => "auto".to_owned(),
        ThreadSetting::Count(count) => count.to_string(),
    };
    format!(
        "threads={threads};strict_repro={};memory_budget_mib={};k_chunk_modes={};permutations={};seed={};save_intermediates={}",
        config.performance.strict_repro,
        config.performance.memory_budget_mib,
        config.performance.k_chunk_modes,
        config.permutation.b,
        config.permutation.seed,
        config.performance.save_intermediates,
    )
    .into_bytes()
}

#[cfg(test)]
mod audit_cache_tests {
    use super::*;
    use crate::{
        workflow::{LocalScheduler, SchedulerLimits},
        PatternMeta,
    };

    #[test]
    fn corrected_marked_definition_invalidates_the_historical_cache_key() {
        let pattern = Pattern::from_arrays(
            vec![0., 1., 0., 1.],
            vec![0., 0., 1., 1.],
            vec![0, 1, 1, 0],
            PatternMeta {
                case_id: "cache-audit".into(),
                timepoint: "post".into(),
                protein: "mark".into(),
                slide_id: None,
                section_id: None,
                stain_batch: None,
                block_id: None,
                region_id: None,
            },
        )
        .unwrap();
        let config = AnalysisConfig::default();
        let mut project = MarklabProject::new();
        let mut node = MarkedAnalysisNode::new(
            &mut project,
            NodeId::new("analysis").unwrap(),
            &pattern,
            &config,
        )
        .unwrap();
        let scheduler = LocalScheduler::new(SchedulerLimits {
            max_inline_output_bytes: 1_000_000,
        })
        .unwrap();
        let corrected = scheduler.cache_key_for(&node).unwrap();
        node.node.implementation_identity = node
            .node
            .implementation_identity
            .replace("adapter=marked-analysis-v6;", "adapter=marked-analysis-v5;");
        assert_ne!(corrected, scheduler.cache_key_for(&node).unwrap());
    }
}
