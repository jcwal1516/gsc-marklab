use std::io;

use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::Serialize;

use crate::{
    workflow::pattern_artifact, BinaryCompartmentPartition2D, InhomogeneousSpatialPointStatus,
    Pattern,
};

use super::{
    compartment_identity::{piecewise_configuration_digest, piecewise_intensity_digest},
    compartment_types::{
        PiecewiseCompartmentRole, PiecewiseCompartmentSpatialConfig,
        PiecewiseCompartmentSpatialLimits, PiecewiseCompartmentSpatialResult,
    },
};

const NODE_KIND: &str = "piecewise_compartment_spatial_analysis";
const PARTITION_KIND: &str = "application/vnd.marklab.binary-compartment-partition-ref;version=1";
const CONFIG_KIND: &str =
    "application/vnd.marklab.piecewise-compartment-spatial-config+json;version=1";
const RESULT_KIND: &str =
    "application/vnd.marklab.piecewise-compartment-spatial-result+json;version=1";
const POLICY: &[u8] = b"serial;piecewise-constant-binary-compartment;leave-one-out-within-compartment;exact-compartment-area;standard-border-inverse-intensity-ratio;fixed-compartment-counts-uniform-exact-partition;erl";

/// Durable typed node for piecewise-compartment intensity-reweighted K/L.
pub struct PiecewiseCompartmentSpatialAnalysisNode<'a> {
    spec: NodeSpec,
    pattern: &'a Pattern,
    partition: &'a BinaryCompartmentPartition2D,
    config: &'a PiecewiseCompartmentSpatialConfig,
    inputs: [ArtifactRef; 3],
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> PiecewiseCompartmentSpatialAnalysisNode<'a> {
    /// Bind the exact pattern, oriented partition, configuration, and node identity.
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        pattern: &'a Pattern,
        partition: &'a BinaryCompartmentPartition2D,
        config: &'a PiecewiseCompartmentSpatialConfig,
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
                "marklab/{};adapter=piecewise-compartment-spatial-node-v1",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    /// Bound workflow specification.
    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for PiecewiseCompartmentSpatialAnalysisNode<'_> {
    type Output = PiecewiseCompartmentSpatialResult;

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
        super::analyze_piecewise_compartment_spatial_pattern(
            self.pattern,
            self.partition,
            self.config,
        )
        .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        encode_piecewise_compartment_result(output, self.pattern, self.partition, self.config)
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

pub(crate) fn encode_piecewise_compartment_result(
    output: &PiecewiseCompartmentSpatialResult,
    pattern: &Pattern,
    partition: &BinaryCompartmentPartition2D,
    config: &PiecewiseCompartmentSpatialConfig,
) -> io::Result<Box<[u8]>> {
    validate(output, pattern, partition, config)?;
    crate::exact_float_json::encode(output)
}

#[derive(Serialize)]
struct PartitionArtifact<'a> {
    logical_digest: String,
    negative_compartment_id: &'a str,
    positive_compartment_id: &'a str,
    observation_area_um2: f64,
    negative_area_um2: f64,
    positive_area_um2: f64,
    interface_segment_count: usize,
    validated_boundary_segment_count: usize,
}

fn partition_artifact(partition: &BinaryCompartmentPartition2D) -> Result<ArtifactRef, NodeError> {
    let descriptor = partition.descriptor();
    let bytes = serde_json::to_vec(&PartitionArtifact {
        logical_digest: descriptor.logical_digest.to_string(),
        negative_compartment_id: &descriptor.negative_compartment_id,
        positive_compartment_id: &descriptor.positive_compartment_id,
        observation_area_um2: descriptor.observation_area_um2,
        negative_area_um2: descriptor.negative_area_um2,
        positive_area_um2: descriptor.positive_area_um2,
        interface_segment_count: descriptor.interface_segment_count,
        validated_boundary_segment_count: descriptor.validated_boundary_segment_count,
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(PARTITION_KIND, &bytes).map_err(NodeError::input)
}

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    radii_um: &'a [f64],
    simulations: usize,
    seed: u64,
    alpha: f64,
    limits: PiecewiseCompartmentSpatialLimits,
    logical_digest: String,
}

fn config_artifact(config: &PiecewiseCompartmentSpatialConfig) -> Result<ArtifactRef, NodeError> {
    let bytes = serde_json::to_vec(&ConfigArtifact {
        radii_um: config.radii_um(),
        simulations: config.simulations(),
        seed: config.seed(),
        alpha: config.alpha(),
        limits: config.limits(),
        logical_digest: piecewise_configuration_digest(config).to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &bytes).map_err(NodeError::input)
}

fn validate(
    result: &PiecewiseCompartmentSpatialResult,
    pattern: &Pattern,
    partition: &BinaryCompartmentPartition2D,
    config: &PiecewiseCompartmentSpatialConfig,
) -> io::Result<()> {
    let descriptor = partition.descriptor();
    let window = partition.observation_window().descriptor();
    if result.case_id != pattern.meta.case_id
        || result.timepoint != pattern.meta.timepoint
        || result.window.logical_digest != window.logical_digest.to_string()
        || result.window.area_um2.to_bits() != window.area_um2.to_bits()
        || result.edge_correction != "standard_border_inverse_intensity_ratio"
        || result.configuration_digest != piecewise_configuration_digest(config).to_string()
        || result.limits != config.limits()
        || result.estimated_storage_bytes > config.limits().maximum_retained_bytes
        || result.compartment_queries != pattern.len()
        || result.compartment_queries > config.limits().maximum_compartment_queries
        || result.observed_pair_visits > result.total_pair_visits
        || result.total_pair_visits > config.limits().maximum_pair_visits
        || result.curve.len() != config.radii_um().len()
        || result.inference.null_model
            != "fixed_binary_compartment_counts_uniform_within_exact_partition"
        || result.inference.randomization_unit
            != "whole_location_pattern_conditioned_on_binary_compartment_counts"
        || result.inference.simulations_completed != config.simulations()
        || result.inference.seed != config.seed()
        || result.inference.alpha.to_bits() != config.alpha().to_bits()
        || result.inference.null_draws > config.limits().maximum_null_draws
    {
        return Err(invalid("result does not match its cache-bound request"));
    }
    validate_intensity(result, pattern, partition)?;
    validate_curve(result, config)?;
    if result.intensity.partition_digest != descriptor.logical_digest.to_string() {
        return Err(invalid("partition digest is inconsistent"));
    }
    Ok(())
}

fn validate_intensity(
    result: &PiecewiseCompartmentSpatialResult,
    pattern: &Pattern,
    partition: &BinaryCompartmentPartition2D,
) -> io::Result<()> {
    let summary = &result.intensity;
    let descriptor = partition.descriptor();
    if summary.estimator != "piecewise_constant_binary_compartment"
        || summary.cross_fit != "leave_one_out_within_compartment"
        || summary.boundary_correction != "exact_compartment_area"
        || summary.interface_event_policy != "reject"
        || summary.negative.role != PiecewiseCompartmentRole::Negative
        || summary.positive.role != PiecewiseCompartmentRole::Positive
        || summary.negative.compartment_id != descriptor.negative_compartment_id
        || summary.positive.compartment_id != descriptor.positive_compartment_id
        || summary.negative.area_um2.to_bits() != descriptor.negative_area_um2.to_bits()
        || summary.positive.area_um2.to_bits() != descriptor.positive_area_um2.to_bits()
        || summary.negative.event_count < 2
        || summary.positive.event_count < 2
        || summary.negative.event_count + summary.positive.event_count != pattern.len()
        || summary.negative.leave_one_out_training_count != summary.negative.event_count - 1
        || summary.positive.leave_one_out_training_count != summary.positive.event_count - 1
        || summary.negative.intensity_per_um2.to_bits()
            != ((summary.negative.event_count - 1) as f64 / summary.negative.area_um2).to_bits()
        || summary.positive.intensity_per_um2.to_bits()
            != ((summary.positive.event_count - 1) as f64 / summary.positive.area_um2).to_bits()
        || summary.point_values.len() != pattern.len()
    {
        return Err(invalid("piecewise intensity summary is inconsistent"));
    }
    for (row, point) in summary.point_values.iter().enumerate() {
        let signed = partition
            .signed_interface_distance_um(pattern.x_um[row], pattern.y_um[row])
            .map_err(|_| invalid("point cannot be assigned to its compartment"))?;
        let expected_role = if signed < 0.0 {
            PiecewiseCompartmentRole::Negative
        } else if signed > 0.0 {
            PiecewiseCompartmentRole::Positive
        } else {
            return Err(invalid("observed point lies on the compartment interface"));
        };
        let level = match expected_role {
            PiecewiseCompartmentRole::Negative => &summary.negative,
            PiecewiseCompartmentRole::Positive => &summary.positive,
        };
        if point.row != row
            || point.role != expected_role
            || point.compartment_id != level.compartment_id
            || point.observed_compartment_count != level.event_count
            || point.training_point_count != level.leave_one_out_training_count
            || point.intensity_per_um2.to_bits() != level.intensity_per_um2.to_bits()
        {
            return Err(invalid("piecewise intensity row is inconsistent"));
        }
    }
    let expected_digest = piecewise_intensity_digest(
        pattern,
        partition,
        summary.negative.event_count,
        summary.positive.event_count,
        summary.negative.intensity_per_um2,
        summary.positive.intensity_per_um2,
        &summary.point_values,
    );
    if summary.artifact_digest != expected_digest.to_string() {
        return Err(invalid(
            "piecewise intensity artifact digest is inconsistent",
        ));
    }
    Ok(())
}

fn validate_curve(
    result: &PiecewiseCompartmentSpatialResult,
    config: &PiecewiseCompartmentSpatialConfig,
) -> io::Result<()> {
    for (point, radius) in result.curve.iter().zip(config.radii_um()) {
        let available = point.status == InhomogeneousSpatialPointStatus::Available;
        if point.radius_um.to_bits() != radius.to_bits()
            || point.k.is_some() != available
            || point.l.is_some() != available
            || !point.inverse_intensity_pair_sum.is_finite()
            || point.inverse_intensity_pair_sum < 0.0
            || !point.eligible_center_inverse_intensity_sum.is_finite()
            || point.eligible_center_inverse_intensity_sum < 0.0
            || point
                .k
                .is_some_and(|value| !value.is_finite() || value < 0.0)
            || point
                .l
                .is_some_and(|value| !value.is_finite() || value < 0.0)
            || point.theoretical_k.to_bits() != (std::f64::consts::PI * radius * radius).to_bits()
            || point.theoretical_l.to_bits() != radius.to_bits()
            || point.lower_l.is_some_and(|value| !value.is_finite())
            || point.upper_l.is_some_and(|value| !value.is_finite())
            || point.inference_eligible != (point.lower_l.is_some() && point.upper_l.is_some())
        {
            return Err(invalid("K/L curve is inconsistent or non-finite"));
        }
        if let (Some(k), Some(l)) = (point.k, point.l) {
            let expected_l = (k / std::f64::consts::PI).sqrt();
            if l.to_bits().abs_diff(expected_l.to_bits()) > 1 {
                return Err(invalid("K and L disagree"));
            }
        }
    }
    let inference = [
        result.inference.p_global,
        result.inference.erl_depth,
        result.inference.critical_depth,
    ];
    if inference.iter().flatten().any(|value| !value.is_finite())
        || inference.iter().any(Option::is_some) != inference.iter().all(Option::is_some)
        || result.inference.eligible_radius_count
            != result
                .curve
                .iter()
                .filter(|point| point.inference_eligible)
                .count()
    {
        return Err(invalid("inference is inconsistent or non-finite"));
    }
    Ok(())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
