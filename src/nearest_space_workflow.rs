use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::Serialize;

use crate::{
    nearest_space::{configuration_digest, NearestSpaceConfig, NearestSpaceResult},
    workflow::pattern_artifact,
    NearestSpaceResultDocument, ObservationWindow2D, Pattern,
};

const NODE_KIND: &str = "nearest_space_analysis";
const WINDOW_KIND: &str = "application/vnd.marklab.observation-window-2d+json;version=1";
const CONFIG_KIND: &str = "application/vnd.marklab.nearest-space-config+json;version=1";
const RESULT_KIND: &str = "application/vnd.marklab.nearest-space-result+json;version=1";
const EXECUTION_POLICY: &[u8] =
    b"serial;exact-rstar;fixed-cell-centred-probes;standard-border;whole-pattern-conditional-csr;erl";
const ADAPTER_REVISION: &str = "nearest-space-analysis-node-v1";

/// Complete exact F/G/J workflow exposed as one cache-addressed typed node.
pub struct NearestSpaceAnalysisNode<'a> {
    spec: NodeSpec,
    pattern: &'a Pattern,
    window: &'a ObservationWindow2D,
    config: &'a NearestSpaceConfig,
    inputs: Vec<ArtifactRef>,
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> NearestSpaceAnalysisNode<'a> {
    /// Catalog exact pattern, window, and configuration identities.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        pattern: &'a Pattern,
        window: &'a ObservationWindow2D,
        config: &'a NearestSpaceConfig,
    ) -> Result<Self, NodeError> {
        Self::new_with_implementation_identity(
            project,
            id,
            pattern,
            window,
            config,
            format!(
                "marklab/{};adapter={ADAPTER_REVISION}",
                env!("CARGO_PKG_VERSION")
            ),
            Vec::new(),
        )
    }

    pub(crate) fn new_with_implementation_identity(
        project: &mut MarklabProject,
        id: NodeId,
        pattern: &'a Pattern,
        window: &'a ObservationWindow2D,
        config: &'a NearestSpaceConfig,
        implementation_identity: String,
        source_artifacts: Vec<ArtifactRef>,
    ) -> Result<Self, NodeError> {
        let pattern_ref = pattern_artifact(pattern)?;
        let window_ref = window_artifact(window)?;
        let config_ref = config_artifact(config)?;
        let spec = NodeSpec::new(id, NODE_KIND, 1, Vec::new()).map_err(NodeError::input)?;
        let mut inputs = vec![pattern_ref, window_ref, config_ref.clone()];
        inputs.extend(source_artifacts);
        for artifact in &inputs {
            project
                .register_reference(artifact.clone())
                .map_err(NodeError::input)?;
        }
        Ok(Self {
            spec,
            pattern,
            window,
            config,
            inputs,
            configuration_digest: config_ref.digest(),
            implementation_identity,
        })
    }

    /// Return the dependency-free graph specification.
    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for NearestSpaceAnalysisNode<'_> {
    type Output = NearestSpaceResult;

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
        let window = window_artifact(self.window)?;
        self.inputs[1]
            .verify_identity(window.digest(), window.byte_len())
            .map_err(NodeError::input)?;
        let config = config_artifact(self.config)?;
        self.inputs[2]
            .verify_identity(config.digest(), config.byte_len())
            .map_err(NodeError::input)
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.configuration_digest,
            execution_policy: EXECUTION_POLICY,
            implementation_identity: &self.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        crate::analyze_nearest_space_pattern(self.pattern, self.window, self.config)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        NearestSpaceResultDocument::new(output.clone())
            .and_then(|document| document.to_json_pretty())
            .map(String::into_bytes)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let text = std::str::from_utf8(bytes).map_err(NodeError::decode)?;
        NearestSpaceResultDocument::from_json(text)
            .map(NearestSpaceResultDocument::into_analysis)
            .map_err(NodeError::decode)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

#[derive(Serialize)]
struct WindowArtifact {
    kind: &'static str,
    logical_digest: String,
    area_um2: f64,
    perimeter_um: f64,
    bounds_um: [f64; 4],
    component_count: usize,
    hole_count: usize,
    ring_count: usize,
    vertex_count: usize,
}

fn window_artifact(window: &ObservationWindow2D) -> Result<ArtifactRef, NodeError> {
    let descriptor = window.descriptor();
    let encoded = serde_json::to_vec(&WindowArtifact {
        kind: "marklab.observation_window_2d",
        logical_digest: descriptor.logical_digest.to_string(),
        area_um2: descriptor.area_um2,
        perimeter_um: descriptor.perimeter_um,
        bounds_um: descriptor.bounds_um,
        component_count: descriptor.component_count,
        hole_count: descriptor.hole_count,
        ring_count: descriptor.ring_count,
        vertex_count: descriptor.vertex_count,
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(WINDOW_KIND, &encoded).map_err(NodeError::input)
}

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    radii_um: &'a [f64],
    probe_grid: [usize; 2],
    simulations: usize,
    seed: u64,
    alpha: f64,
    j_denominator_epsilon: f64,
    limits: crate::NearestSpaceLimits,
    logical_digest: String,
}

fn config_artifact(config: &NearestSpaceConfig) -> Result<ArtifactRef, NodeError> {
    let encoded = serde_json::to_vec(&ConfigArtifact {
        radii_um: config.radii_um(),
        probe_grid: config.probe_grid(),
        simulations: config.simulations(),
        seed: config.seed(),
        alpha: config.alpha(),
        j_denominator_epsilon: config.j_denominator_epsilon(),
        limits: config.limits(),
        logical_digest: configuration_digest(config).to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &encoded).map_err(NodeError::input)
}
