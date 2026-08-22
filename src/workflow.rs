use std::io::Write;

use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};

use crate::{
    AnalysisConfig, AnalysisEngine, MarkedPatternResult, Pattern, ResultDocument, ThreadSetting,
    RESULT_FORMAT_VERSION,
};

const PATTERN_KIND: &str = "application/vnd.marklab.compat-pattern+json";
const CONFIG_KIND: &str = "application/vnd.marklab.analysis-config+toml;version=0.2";
const RESULT_KIND: &str = "application/vnd.marklab.result+json;version=0.3";
const ADAPTER_REVISION: &str = "marked-analysis-v1";

/// Existing marked analysis exposed as one typed workflow node.
pub struct MarkedAnalysisNode<'a> {
    spec: NodeSpec,
    pattern: &'a Pattern,
    config: &'a AnalysisConfig,
    inputs: [ArtifactRef; 2],
    config_digest: ContentDigest,
    execution_policy: Vec<u8>,
    implementation_identity: String,
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

        Ok(Self {
            spec,
            pattern,
            config,
            config_digest: config_artifact.digest(),
            inputs: [pattern_artifact, config_artifact],
            execution_policy: execution_policy(config),
            implementation_identity: format!(
                "marklab/{};adapter={ADAPTER_REVISION};result={RESULT_FORMAT_VERSION};parallel={}",
                env!("CARGO_PKG_VERSION"),
                cfg!(feature = "parallel")
            ),
        })
    }

    /// Return the versioned dependency-free specification registered in the graph.
    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for MarkedAnalysisNode<'_> {
    type Output = MarkedPatternResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.inputs
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let pattern = pattern_artifact(self.pattern)?;
        self.inputs[0]
            .verify_identity(pattern.digest(), pattern.byte_len())
            .map_err(NodeError::input)?;
        let config = config_artifact(self.config)?;
        self.inputs[1]
            .verify_identity(config.digest(), config.byte_len())
            .map_err(NodeError::input)
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.config_digest,
            execution_policy: &self.execution_policy,
            implementation_identity: &self.implementation_identity,
        }
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

fn pattern_artifact(pattern: &Pattern) -> Result<ArtifactRef, NodeError> {
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
