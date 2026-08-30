use super::{
    isotropic::{configuration_digest, IsotropicCategoricalCrossPairCorrelationConfig},
    isotropic_categorical_cross_pair_correlation,
    workflow::window_artifact,
    IsotropicCategoricalCrossPairCorrelationResult,
};
use crate::{
    workflow::pattern_artifact, DeclaredScalarPatternInput, ObservationWindow2D,
    PairCorrelationKernel, PairCorrelationPointStatus,
};
use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::Serialize;
use std::io;

const CONFIG_KIND: &str =
    "application/vnd.marklab.isotropic-categorical-cross-g-config+json;version=1";
const RESULT_KIND: &str =
    "application/vnd.marklab.isotropic-categorical-cross-g-result+json;version=1";
const POLICY: &[u8] = b"serial;retained-directed-pairs;exact-visible-arcs;epanechnikov;complete-row-random-labeling;erl";

/// Cache-addressed exact isotropic directed categorical cross-g node.
pub struct IsotropicCategoricalCrossPairCorrelationAnalysisNode<'a> {
    spec: NodeSpec,
    input: &'a DeclaredScalarPatternInput<'a>,
    window: &'a ObservationWindow2D,
    config: &'a IsotropicCategoricalCrossPairCorrelationConfig,
    inputs: Vec<ArtifactRef>,
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> IsotropicCategoricalCrossPairCorrelationAnalysisNode<'a> {
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'a DeclaredScalarPatternInput<'a>,
        window: &'a ObservationWindow2D,
        config: &'a IsotropicCategoricalCrossPairCorrelationConfig,
    ) -> Result<Self, NodeError> {
        Self::new_with_implementation_identity(
            project,
            id,
            input,
            window,
            config,
            format!(
                "marklab/{};adapter=isotropic-categorical-cross-g-node-v1",
                env!("CARGO_PKG_VERSION")
            ),
            Vec::new(),
        )
    }

    pub(crate) fn new_with_implementation_identity(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'a DeclaredScalarPatternInput<'a>,
        window: &'a ObservationWindow2D,
        config: &'a IsotropicCategoricalCrossPairCorrelationConfig,
        implementation_identity: String,
        source_artifacts: Vec<ArtifactRef>,
    ) -> Result<Self, NodeError> {
        input
            .revalidate_project(project)
            .map_err(NodeError::input)?;
        let config_ref = config_artifact(config)?;
        let mut inputs = vec![
            pattern_artifact(input.pattern())?,
            input.declared_artifact_ref().map_err(NodeError::input)?,
            window_artifact(window)?,
            config_ref.clone(),
        ];
        inputs.extend(source_artifacts);
        for artifact in &inputs {
            project
                .register_reference(artifact.clone())
                .map_err(NodeError::input)?;
        }
        Ok(Self {
            spec: NodeSpec::new(
                id,
                "isotropic_categorical_cross_pair_correlation",
                1,
                Vec::new(),
            )
            .map_err(NodeError::input)?,
            input,
            window,
            config,
            inputs,
            configuration_digest: config_ref.digest(),
            implementation_identity,
        })
    }
    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for IsotropicCategoricalCrossPairCorrelationAnalysisNode<'_> {
    type Output = IsotropicCategoricalCrossPairCorrelationResult;
    fn spec(&self) -> &NodeSpec {
        &self.spec
    }
    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.inputs
    }
    fn semantic_input_artifacts(&self) -> &[marklab_workflow::ArtifactId] {
        self.input.semantic_artifact_ids()
    }
    fn verify_input_content(&self) -> Result<(), NodeError> {
        let current = [
            pattern_artifact(self.input.pattern())?,
            self.input
                .declared_artifact_ref()
                .map_err(NodeError::input)?,
            window_artifact(self.window)?,
            config_artifact(self.config)?,
        ];
        for (bound, current) in self.inputs[..4].iter().zip(current) {
            bound
                .verify_identity(current.digest(), current.byte_len())
                .map_err(NodeError::input)?;
        }
        Ok(())
    }
    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.configuration_digest,
            execution_policy: POLICY,
            implementation_identity: &self.implementation_identity,
        }
    }
    fn execute(&self) -> Result<Self::Output, NodeError> {
        isotropic_categorical_cross_pair_correlation(self.input, self.window, self.config)
            .map_err(NodeError::execution)
    }
    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        encode_result(output, self.config).map_err(NodeError::encoding)
    }
    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output = serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        validate(&output, self.config).map_err(NodeError::decode)?;
        Ok(output)
    }
    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

pub(crate) fn encode_result(
    output: &IsotropicCategoricalCrossPairCorrelationResult,
    config: &IsotropicCategoricalCrossPairCorrelationConfig,
) -> io::Result<Box<[u8]>> {
    validate(output, config)?;
    serde_json::to_vec_pretty(output)
        .map(Vec::into_boxed_slice)
        .map_err(io::Error::other)
}

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    radii_um: &'a [f64],
    bandwidth_um: f64,
    source_level: &'a str,
    target_level: &'a str,
    permutations: usize,
    seed: u64,
    alpha: f64,
    categorical_limits: [usize; 5],
    isotropic_limits: crate::IsotropicSpatialLimits,
    logical_digest: String,
}

fn config_artifact(
    config: &IsotropicCategoricalCrossPairCorrelationConfig,
) -> Result<ArtifactRef, NodeError> {
    let base = config.base();
    let limits = base.limits();
    let bytes = serde_json::to_vec(&ConfigArtifact {
        radii_um: base.radii_um(),
        bandwidth_um: base.bandwidth_um(),
        source_level: base.source_level(),
        target_level: base.target_level(),
        permutations: base.permutations(),
        seed: base.seed(),
        alpha: base.alpha(),
        categorical_limits: [
            limits.maximum_points,
            limits.maximum_radii,
            limits.maximum_directed_pairs,
            limits.maximum_permutation_pair_evaluations,
            limits.maximum_retained_bytes,
        ],
        isotropic_limits: config.isotropic_limits(),
        logical_digest: configuration_digest(config).to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &bytes).map_err(NodeError::input)
}

fn validate(
    result: &IsotropicCategoricalCrossPairCorrelationResult,
    config: &IsotropicCategoricalCrossPairCorrelationConfig,
) -> io::Result<()> {
    let base = config.base();
    if result.edge_correction != "isotropic"
        || result.kernel != PairCorrelationKernel::Epanechnikov
        || result.bandwidth_um.to_bits() != base.bandwidth_um().to_bits()
        || result.source_level != base.source_level()
        || result.target_level != base.target_level()
        || result.configuration_digest != configuration_digest(config).to_string()
        || result.estimated_storage_bytes > config.isotropic_limits().maximum_retained_bytes
        || result.curve.len() != base.radii_um().len()
        || result.inference.permutations_completed != base.permutations()
        || result.inference.seed != base.seed()
        || result.inference.alpha.to_bits() != base.alpha().to_bits()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "isotropic cross-g identity differs",
        ));
    }
    for (point, radius) in result.curve.iter().zip(base.radii_um()) {
        let available = point.status == PairCorrelationPointStatus::Available;
        if point.radius_um.to_bits() != radius.to_bits()
            || point.visible_arc_evaluations != point.directed_source_target_pairs_in_support
            || point.cross_g.is_some() != available
            || !point.visible_arc_fraction_sum.is_finite()
            || !point.inverse_visible_arc_weighted_kernel_sum.is_finite()
            || point.cross_g.is_some_and(|value| !value.is_finite())
            || point.theoretical_cross_g != 1.0
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "isotropic cross-g curve differs",
            ));
        }
    }
    Ok(())
}
