use std::io;

use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::Serialize;

use crate::{
    workflow::pattern_artifact, BinaryCompartmentPartition2D, PairCorrelationKernel, Pattern,
};

use super::{
    compartment_identity::piecewise_g_configuration_digest,
    compartment_workflow::{partition_artifact, validate_piecewise_intensity},
    g_workflow::validate_g_curve,
    PiecewiseCompartmentPairCorrelationConfig, PiecewiseCompartmentPairCorrelationResult,
    PiecewiseCompartmentSpatialLimits,
};

const NODE_KIND: &str = "piecewise_compartment_pair_correlation";
const CONFIG_KIND: &str =
    "application/vnd.marklab.piecewise-compartment-pair-correlation-config+json;version=1";
const RESULT_KIND: &str =
    "application/vnd.marklab.piecewise-compartment-pair-correlation-result+json;version=1";
const POLICY: &[u8] = b"serial;piecewise-constant-binary-compartment;leave-one-out-within-compartment;exact-compartment-area;epanechnikov;standard-border-radius-plus-pair-bandwidth-inverse-intensity-ratio;fixed-compartment-counts-uniform-exact-partition;erl";

/// Durable typed node for piecewise-compartment intensity-reweighted pair correlation.
pub struct PiecewiseCompartmentPairCorrelationAnalysisNode<'a> {
    spec: NodeSpec,
    pattern: &'a Pattern,
    partition: &'a BinaryCompartmentPartition2D,
    config: &'a PiecewiseCompartmentPairCorrelationConfig,
    inputs: [ArtifactRef; 3],
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> PiecewiseCompartmentPairCorrelationAnalysisNode<'a> {
    /// Bind the exact pattern, oriented partition, estimator, kernel, and limits.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        pattern: &'a Pattern,
        partition: &'a BinaryCompartmentPartition2D,
        config: &'a PiecewiseCompartmentPairCorrelationConfig,
    ) -> Result<Self, NodeError> {
        let pattern_ref = pattern_artifact(pattern)?;
        let partition_ref = partition_artifact(partition)?;
        let config_ref = config_artifact(config)?;
        let inputs = [pattern_ref, partition_ref, config_ref.clone()];
        for artifact in &inputs {
            project
                .register_reference(artifact.clone())
                .map_err(NodeError::input)?;
        }
        Ok(Self {
            spec: NodeSpec::new(id, NODE_KIND, 1, Vec::new()).map_err(NodeError::input)?,
            pattern,
            partition,
            config,
            inputs,
            configuration_digest: config_ref.digest(),
            implementation_identity: format!(
                "marklab/{};adapter=piecewise-compartment-pair-correlation-node-v1",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    /// Bound workflow specification.
    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for PiecewiseCompartmentPairCorrelationAnalysisNode<'_> {
    type Output = PiecewiseCompartmentPairCorrelationResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.inputs
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let current = [
            pattern_artifact(self.pattern)?,
            partition_artifact(self.partition)?,
            config_artifact(self.config)?,
        ];
        for (bound, current) in self.inputs.iter().zip(current) {
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
        super::analyze_piecewise_compartment_pair_correlation(
            self.pattern,
            self.partition,
            self.config,
        )
        .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        encode_piecewise_compartment_pair_correlation_result(
            output,
            self.pattern,
            self.partition,
            self.config,
        )
        .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output = crate::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        validate(&output, self.pattern, self.partition, self.config).map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

pub(crate) fn encode_piecewise_compartment_pair_correlation_result(
    output: &PiecewiseCompartmentPairCorrelationResult,
    pattern: &Pattern,
    partition: &BinaryCompartmentPartition2D,
    config: &PiecewiseCompartmentPairCorrelationConfig,
) -> io::Result<Box<[u8]>> {
    validate(output, pattern, partition, config)?;
    crate::exact_float_json::encode(output)
}

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    radii_um: &'a [f64],
    pair_bandwidth_um: f64,
    simulations: usize,
    seed: u64,
    alpha: f64,
    limits: PiecewiseCompartmentSpatialLimits,
    logical_digest: String,
}

fn config_artifact(
    config: &PiecewiseCompartmentPairCorrelationConfig,
) -> Result<ArtifactRef, NodeError> {
    let intensity = config.intensity_config();
    let bytes = serde_json::to_vec(&ConfigArtifact {
        radii_um: intensity.radii_um(),
        pair_bandwidth_um: config.pair_bandwidth_um(),
        simulations: intensity.simulations(),
        seed: intensity.seed(),
        alpha: intensity.alpha(),
        limits: intensity.limits(),
        logical_digest: piecewise_g_configuration_digest(config).to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &bytes).map_err(NodeError::input)
}

fn validate(
    result: &PiecewiseCompartmentPairCorrelationResult,
    pattern: &Pattern,
    partition: &BinaryCompartmentPartition2D,
    config: &PiecewiseCompartmentPairCorrelationConfig,
) -> io::Result<()> {
    let base = config.intensity_config();
    let window = partition.observation_window().descriptor();
    if result.case_id != pattern.meta.case_id
        || result.timepoint != pattern.meta.timepoint
        || result.window.logical_digest != window.logical_digest.to_string()
        || result.window.area_um2.to_bits() != window.area_um2.to_bits()
        || result.kernel != PairCorrelationKernel::Epanechnikov
        || result.pair_bandwidth_um.to_bits() != config.pair_bandwidth_um().to_bits()
        || result.edge_correction
            != "standard_border_radius_plus_pair_bandwidth_inverse_intensity_ratio"
        || result.configuration_digest != piecewise_g_configuration_digest(config).to_string()
        || result.limits != base.limits()
        || result.estimated_storage_bytes > base.limits().maximum_retained_bytes
        || result.compartment_queries != pattern.len()
        || result.compartment_queries > base.limits().maximum_compartment_queries
        || result.observed_pair_visits > result.total_pair_visits
        || result.total_pair_visits > base.limits().maximum_pair_visits
        || result.curve.len() != base.radii_um().len()
        || result.inference.null_model
            != "fixed_binary_compartment_counts_uniform_within_exact_partition"
        || result.inference.randomization_unit
            != "whole_location_pattern_conditioned_on_binary_compartment_counts"
        || result.inference.simulations_completed != base.simulations()
        || result.inference.seed != base.seed()
        || result.inference.alpha.to_bits() != base.alpha().to_bits()
        || result.inference.null_draws > base.limits().maximum_null_draws
    {
        return Err(invalid("result does not match its cache-bound request"));
    }
    validate_piecewise_intensity(&result.intensity, pattern, partition)?;
    validate_g_curve(&result.curve, &result.inference, base.radii_um())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
