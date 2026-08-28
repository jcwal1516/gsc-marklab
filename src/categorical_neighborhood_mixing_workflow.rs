use std::io;

use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, MarklabProject, NodeError, NodeId, NodeSpec,
    WorkflowNode,
};
use serde::Serialize;

use crate::{
    categorical_neighborhood_mixing::{configuration_digest, entropy_nats},
    compartment_interface::analysis::measurement_status_name,
    workflow::pattern_artifact,
    CategoricalNeighborhoodMixingConfig, CategoricalNeighborhoodMixingResult,
    DeclaredScalarPatternInput, ObservationWindow2D, ScalarMarkId,
};

const NODE_KIND: &str = "categorical_neighborhood_mixing";
const WINDOW_KIND: &str = "application/vnd.marklab.observation-window-ref;version=1";
const CONFIG_KIND: &str =
    "application/vnd.marklab.categorical-neighborhood-mixing-config+json;version=1";
const RESULT_KIND: &str = "application/vnd.marklab.categorical-neighborhood-mixing+json;version=1";
const POLICY: &[u8] = b"serial;exact-undirected-physical-radius-graph;complete-categorical-rows;directed-mixing-matrix;fixed-count-random-label-expectation";

/// Cache-addressed multiclass categorical neighborhood-mixing node.
pub struct CategoricalNeighborhoodMixingAnalysisNode<'a> {
    spec: NodeSpec,
    input: &'a DeclaredScalarPatternInput<'a>,
    window: &'a ObservationWindow2D,
    mark_id: &'a ScalarMarkId,
    config: &'a CategoricalNeighborhoodMixingConfig,
    inputs: [ArtifactRef; 4],
    configuration_digest: ContentDigest,
    implementation_identity: String,
}

impl<'a> CategoricalNeighborhoodMixingAnalysisNode<'a> {
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'a DeclaredScalarPatternInput<'a>,
        window: &'a ObservationWindow2D,
        mark_id: &'a ScalarMarkId,
        config: &'a CategoricalNeighborhoodMixingConfig,
    ) -> Result<Self, NodeError> {
        input
            .revalidate_project(project)
            .map_err(NodeError::input)?;
        let pattern = pattern_artifact(input.pattern())?;
        let declared = input.declared_artifact_ref().map_err(NodeError::input)?;
        let window_ref = window_artifact(window)?;
        let config_ref = config_artifact(input, window, mark_id, config)?;
        let inputs = [pattern, declared, window_ref, config_ref.clone()];
        for artifact in &inputs {
            project
                .register_reference(artifact.clone())
                .map_err(NodeError::input)?;
        }
        Ok(Self {
            spec: NodeSpec::new(id, NODE_KIND, 1, Vec::new()).map_err(NodeError::input)?,
            input,
            window,
            mark_id,
            config,
            inputs,
            configuration_digest: config_ref.digest(),
            implementation_identity: format!(
                "marklab/{};adapter=categorical-neighborhood-mixing-node-v1",
                env!("CARGO_PKG_VERSION")
            ),
        })
    }

    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for CategoricalNeighborhoodMixingAnalysisNode<'_> {
    type Output = CategoricalNeighborhoodMixingResult;

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
            config_artifact(self.input, self.window, self.mark_id, self.config)?,
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
        crate::categorical_neighborhood_mixing(self.input, self.window, self.mark_id, self.config)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        validate(output, self.input, self.window, self.mark_id, self.config)
            .map_err(NodeError::encoding)?;
        crate::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output = crate::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        validate(&output, self.input, self.window, self.mark_id, self.config)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        RESULT_KIND
    }
}

#[derive(Serialize)]
struct WindowArtifact<'a> {
    logical_digest: String,
    coordinate_frame_id: &'a str,
}

fn window_artifact(window: &ObservationWindow2D) -> Result<ArtifactRef, NodeError> {
    let frame = window
        .coordinate_frame_id()
        .ok_or_else(|| NodeError::input(invalid("observation window is not frame-bound")))?;
    let bytes = serde_json::to_vec(&WindowArtifact {
        logical_digest: window.descriptor().logical_digest.to_string(),
        coordinate_frame_id: frame.as_str(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(WINDOW_KIND, &bytes).map_err(NodeError::input)
}

#[derive(Serialize)]
struct ConfigArtifact<'a> {
    mark_id: &'a str,
    radius_um: f64,
    limits: crate::CategoricalNeighborhoodMixingLimits,
    logical_digest: String,
}

fn config_artifact(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    config: &CategoricalNeighborhoodMixingConfig,
) -> Result<ArtifactRef, NodeError> {
    let digest = configuration_digest(input, window, mark_id, config).map_err(NodeError::input)?;
    let bytes = serde_json::to_vec(&ConfigArtifact {
        mark_id: mark_id.as_str(),
        radius_um: config.radius_um(),
        limits: config.limits(),
        logical_digest: digest.to_string(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(CONFIG_KIND, &bytes).map_err(NodeError::input)
}

fn validate(
    result: &CategoricalNeighborhoodMixingResult,
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    config: &CategoricalNeighborhoodMixingConfig,
) -> io::Result<()> {
    let table = input
        .mark_table()
        .ok_or_else(|| invalid("typed MarkTable is absent"))?;
    let levels = table
        .categorical_levels(mark_id)
        .ok_or_else(|| invalid("typed categorical levels are absent"))?;
    let values = table
        .categorical_values(mark_id)
        .ok_or_else(|| invalid("typed categorical rows are absent"))?;
    let expected_digest =
        configuration_digest(input, window, mark_id, config).map_err(invalid_owned)?;
    let classes = levels.len();
    let matrix_len = classes
        .checked_mul(classes)
        .ok_or_else(|| invalid("categorical matrix size overflow"))?;
    let mut cell_counts = vec![0_usize; classes];
    for value in values {
        cell_counts[*value as usize] += 1;
    }
    if result.case_id != input.pattern().meta.case_id
        || result.timepoint != input.pattern().meta.timepoint
        || result.mark_id != mark_id.as_str()
        || result.measurement_status
            != measurement_status_name(
                table
                    .measurement_status(mark_id)
                    .ok_or_else(|| invalid("categorical status is absent"))?,
            )
        || result.coordinate_frame_id != input.coordinate_frame_id().as_str()
        || result.window_digest != window.descriptor().logical_digest.to_string()
        || result.radius_um.to_bits() != config.radius_um().to_bits()
        || result.class_ids != levels
        || result.configuration_digest != expected_digest.to_string()
        || result.graph_digest.len() != 64
        || result.point_count != input.pattern().len()
        || result.cell_counts != cell_counts
        || result.directed_pair_counts.len() != matrix_len
        || result.observed_pair_fractions.len() != matrix_len
        || result.random_label_pair_expectations.len() != matrix_len
        || result.pair_fraction_excess.len() != matrix_len
        || result.class_summaries.len() != classes
        || result.directed_pair_visits > config.limits().maximum_pair_visits
        || result.estimated_storage_bytes > config.limits().maximum_retained_bytes
        || result.limits != config.limits()
    {
        return Err(invalid(
            "categorical neighborhood result does not match its request",
        ));
    }
    let visits = result
        .directed_pair_counts
        .iter()
        .try_fold(0_usize, |total, count| total.checked_add(*count));
    if visits != Some(result.directed_pair_visits)
        || result.directed_pair_visits == 0
        || !result.directed_pair_visits.is_multiple_of(2)
        || result.undirected_edge_count != result.directed_pair_visits / 2
    {
        return Err(invalid("categorical neighborhood edge counts disagree"));
    }
    let population_denominator = result
        .point_count
        .checked_mul(result.point_count - 1)
        .ok_or_else(|| invalid("categorical population size overflow"))?
        as f64;
    let mut cross_directed = 0_usize;
    let mut random_same = 0.0_f64;
    for source in 0..classes {
        for target in 0..classes {
            let index = source * classes + target;
            if result.directed_pair_counts[index]
                != result.directed_pair_counts[target * classes + source]
            {
                return Err(invalid("categorical mixing matrix is not symmetric"));
            }
            let observed =
                result.directed_pair_counts[index] as f64 / result.directed_pair_visits as f64;
            let numerator = if source == target {
                cell_counts[source].checked_mul(cell_counts[source].saturating_sub(1))
            } else {
                cell_counts[source].checked_mul(cell_counts[target])
            }
            .ok_or_else(|| invalid("categorical expectation overflow"))?;
            let expected = numerator as f64 / population_denominator;
            if result.observed_pair_fractions[index].to_bits() != observed.to_bits()
                || result.random_label_pair_expectations[index].to_bits() != expected.to_bits()
                || result.pair_fraction_excess[index].to_bits() != (observed - expected).to_bits()
                || !finite_unit(result.observed_pair_fractions[index])
                || !finite_unit(result.random_label_pair_expectations[index])
                || !result.pair_fraction_excess[index].is_finite()
            {
                return Err(invalid("categorical mixing fraction is inconsistent"));
            }
            if source == target {
                random_same += expected;
            } else {
                cross_directed = cross_directed
                    .checked_add(result.directed_pair_counts[index])
                    .ok_or_else(|| invalid("categorical cross-edge overflow"))?;
            }
        }
    }
    if !cross_directed.is_multiple_of(2)
        || result.cross_class_edge_count != cross_directed / 2
        || result.cross_class_edge_fraction.to_bits()
            != (result.cross_class_edge_count as f64 / result.undirected_edge_count as f64)
                .to_bits()
        || result.random_label_cross_edge_expectation.to_bits() != (1.0 - random_same).to_bits()
        || result.cross_edge_fraction_minus_expectation.to_bits()
            != (result.cross_class_edge_fraction - result.random_label_cross_edge_expectation)
                .to_bits()
        || !finite_unit(result.cross_class_edge_fraction)
        || !finite_unit(result.random_label_cross_edge_expectation)
    {
        return Err(invalid("categorical cross-edge summary is inconsistent"));
    }
    let mut zero_total = 0_usize;
    for (class, summary) in result.class_summaries.iter().enumerate() {
        let incidences = &result.directed_pair_counts[class * classes..(class + 1) * classes];
        let total = incidences.iter().sum::<usize>();
        let probabilities = (total > 0).then(|| {
            incidences
                .iter()
                .map(|count| *count as f64 / total as f64)
                .collect::<Vec<_>>()
        });
        let entropy = probabilities.as_ref().map(|values| entropy_nats(values));
        zero_total = zero_total
            .checked_add(summary.zero_neighbor_cell_count)
            .ok_or_else(|| invalid("categorical zero-neighbor overflow"))?;
        if summary.class_id != levels[class]
            || summary.cell_count != cell_counts[class]
            || summary.zero_neighbor_cell_count > summary.cell_count
            || summary.neighbor_incidences != incidences
            || summary.neighbor_probabilities != probabilities
            || summary.neighbor_label_entropy_nats != entropy
            || summary.normalized_neighbor_label_entropy
                != entropy.map(|value| value / (classes as f64).ln())
        {
            return Err(invalid("categorical class summary is inconsistent"));
        }
    }
    if zero_total != result.zero_neighbor_cell_count {
        return Err(invalid("categorical zero-neighbor counts disagree"));
    }
    Ok(())
}

fn finite_unit(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn invalid_owned(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
