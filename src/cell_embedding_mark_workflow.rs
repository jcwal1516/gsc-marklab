use crate::workflow_node::{NodeAnalysis, TypedNode};
use marklab_embeddings::{CellEmbeddingArtifact, CellEmbeddingTable};
use marklab_project::{ArtifactId, ArtifactRef, ContentDigest, MarklabProject};
use marklab_workflow::{NodeError, NodeId, NodeSpec};
use thiserror::Error;

use crate::{
    cell_embedding_mark::{
        bind_declared_binary_cell_embedding_centroid, DeclaredBinaryCellEmbeddingCentroidBinding,
    },
    embeddings::{
        declared_binary_cell_embedding_centroid_discrepancy,
        DeclaredBinaryCellEmbeddingCentroidDiscrepancy,
        DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus,
    },
    marked_workflow::pattern_artifact,
    scalar_mark::DeclaredScalarPatternInput,
};

const NODE_KIND: &str = "declared_binary_cell_embedding_centroid";
const OUTPUT_KIND: &str =
    "application/vnd.marklab.internal.declared-binary-cell-centroid-cache;version=1";
const CONFIG_DOMAIN: &[u8] = b"marklab-declared-binary-cell-embedding-centroid-node-config-v1";
const EXECUTION_POLICY: &[u8] =
    b"serial-stored-row-component-order;f32-to-f64;mean-squared-component-difference;positive-zero";
const ADAPTER_REVISION: &str = "declared-binary-cell-embedding-centroid-node-v1";
const CODEC_REVISION: &str = "private-v1";
const CODEC_MAGIC: &[u8; 8] = b"MLCBCENT";
const CODEC_VERSION: u8 = 1;
const CODEC_LENGTH: usize = 18;

/// Exact centroid computation exposed as one provenance-aware project workflow node.
pub struct DeclaredBinaryCellEmbeddingCentroidNode<'node, 'pattern> {
    node: TypedNode<DeclaredBinaryCellEmbeddingCentroidAnalysis<'node, 'pattern>>,
}

struct DeclaredBinaryCellEmbeddingCentroidAnalysis<'node, 'pattern> {
    input: &'node DeclaredScalarPatternInput<'pattern>,
    table: &'node CellEmbeddingTable,
    artifact: CellEmbeddingArtifact,
    maximum_rows: usize,
    maximum_component_operations: u64,
    maximum_working_bytes: usize,
    semantic_inputs: Box<[ArtifactId]>,
    binding: DeclaredBinaryCellEmbeddingCentroidBinding,
}

impl<'node, 'pattern> DeclaredBinaryCellEmbeddingCentroidNode<'node, 'pattern> {
    /// Revalidate and catalog exact inputs before constructing the dependency-free node.
    ///
    /// The three scientific resource limits are part of cache identity. The embedding table,
    /// row-link, provenance, and declared mark evidence must already be exact catalog records;
    /// their managed bytes are reverified by `LocalScheduler::run_single_with_store` before every
    /// miss or hit.
    ///
    /// # Errors
    ///
    /// Returns a typed workflow input error when the target project, semantic records, centroid binding,
    /// ordinary references, limits, or node specification are invalid.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project: &mut MarklabProject,
        id: NodeId,
        input: &'node DeclaredScalarPatternInput<'pattern>,
        table: &'node CellEmbeddingTable,
        artifact: CellEmbeddingArtifact,
        maximum_rows: usize,
        maximum_component_operations: u64,
        maximum_working_bytes: usize,
    ) -> Result<Self, NodeError> {
        input
            .revalidate_project(project)
            .map_err(NodeError::input)?;
        let binding = bind_declared_binary_cell_embedding_centroid(
            input,
            table,
            artifact,
            maximum_rows,
            maximum_component_operations,
            maximum_working_bytes,
        )
        .map_err(NodeError::input)?;

        let mut semantic_inputs = input.semantic_artifact_ids().to_vec();
        for artifact_id in [
            artifact.embedding_artifact_id(),
            artifact.row_link_artifact_id(),
            artifact.provenance_artifact_id(),
        ] {
            if !semantic_inputs.contains(&artifact_id) {
                semantic_inputs.push(artifact_id);
            }
        }
        for artifact_id in &semantic_inputs {
            if project.artifact_record(*artifact_id).is_none() {
                return Err(NodeError::input(
                    CentroidNodeInputError::SemanticArtifactMissing,
                ));
            }
        }

        let pattern = pattern_artifact(input.pattern())?;
        let declared = input.declared_artifact_ref().map_err(NodeError::input)?;
        let spec = NodeSpec::new(id, NODE_KIND, 1, Vec::new()).map_err(NodeError::input)?;
        project
            .register_reference(pattern.clone())
            .map_err(NodeError::input)?;
        project
            .register_reference(declared.clone())
            .map_err(NodeError::input)?;

        let node_inputs = [pattern, declared];
        let node_semantic_inputs = semantic_inputs.into_boxed_slice();
        let node_configuration_digest = configuration_digest(
            maximum_rows,
            maximum_component_operations,
            maximum_working_bytes,
        );
        let node_implementation_identity = format!(
            "marklab/{};adapter={ADAPTER_REVISION};codec={CODEC_REVISION}",
            env!("CARGO_PKG_VERSION")
        );
        Ok(Self {
            node: TypedNode {
                spec,
                analysis: DeclaredBinaryCellEmbeddingCentroidAnalysis {
                    input,
                    table,
                    artifact,
                    maximum_rows,
                    maximum_component_operations,
                    maximum_working_bytes,
                    semantic_inputs: node_semantic_inputs,
                    binding,
                },
                inputs: node_inputs,
                configuration_digest: node_configuration_digest,
                execution_policy: std::borrow::Cow::Borrowed(EXECUTION_POLICY),
                implementation_identity: node_implementation_identity,
            },
        })
    }

    /// Return the exact dependency-free specification registered in the graph.
    pub fn spec(&self) -> &NodeSpec {
        self.node.spec()
    }
}

impl NodeAnalysis for DeclaredBinaryCellEmbeddingCentroidAnalysis<'_, '_> {
    type Output = DeclaredBinaryCellEmbeddingCentroidDiscrepancy;
    type Inputs = [ArtifactRef; 2];

    fn semantic_input_artifacts(&self) -> &[ArtifactId] {
        &self.semantic_inputs
    }

    fn verify_input_content(&self, inputs: &[ArtifactRef]) -> Result<(), NodeError> {
        let pattern = pattern_artifact(self.input.pattern())?;
        crate::workflow_node::verify_input(inputs, 0, &pattern)?;
        let declared = self
            .input
            .declared_artifact_ref()
            .map_err(NodeError::input)?;
        crate::workflow_node::verify_input(inputs, 1, &declared)?;
        let binding = bind_declared_binary_cell_embedding_centroid(
            self.input,
            self.table,
            self.artifact,
            self.maximum_rows,
            self.maximum_component_operations,
            self.maximum_working_bytes,
        )
        .map_err(NodeError::input)?;
        if binding != self.binding {
            return Err(NodeError::input(CentroidNodeInputError::BindingChanged));
        }
        Ok(())
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        declared_binary_cell_embedding_centroid_discrepancy(
            self.input,
            self.table,
            self.artifact,
            self.maximum_rows,
            self.maximum_component_operations,
            self.maximum_working_bytes,
        )
        .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        encode_output(&self.binding, output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        decode_output(&self.binding, bytes).map_err(NodeError::decode)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}

crate::workflow_node::impl_workflow_node!(DeclaredBinaryCellEmbeddingCentroidNode<'_, '_> => DeclaredBinaryCellEmbeddingCentroidDiscrepancy);

fn configuration_digest(
    maximum_rows: usize,
    maximum_component_operations: u64,
    maximum_working_bytes: usize,
) -> ContentDigest {
    let rows = (maximum_rows as u128).to_be_bytes();
    let operations = maximum_component_operations.to_be_bytes();
    let working = (maximum_working_bytes as u128).to_be_bytes();
    ContentDigest::from_framed([CONFIG_DOMAIN, &rows, &operations, &working])
}

fn encode_output(
    binding: &DeclaredBinaryCellEmbeddingCentroidBinding,
    output: &DeclaredBinaryCellEmbeddingCentroidDiscrepancy,
) -> Result<Box<[u8]>, CentroidCodecError> {
    if !binding.matches_result(output) {
        return Err(CentroidCodecError::BindingMismatch);
    }
    let (status, value) = match output.status() {
        DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus::InsufficientGroups => (0, 0.0),
        DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus::Available => (
            1,
            output
                .mean_squared_component_difference()
                .ok_or(CentroidCodecError::InvalidValue)?,
        ),
    };
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(CODEC_LENGTH)
        .map_err(|_| CentroidCodecError::AllocationFailed)?;
    bytes.extend_from_slice(CODEC_MAGIC);
    bytes.push(CODEC_VERSION);
    bytes.push(status);
    bytes.extend_from_slice(&value.to_bits().to_be_bytes());
    debug_assert_eq!(bytes.len(), CODEC_LENGTH);
    Ok(bytes.into_boxed_slice())
}

fn decode_output(
    binding: &DeclaredBinaryCellEmbeddingCentroidBinding,
    bytes: &[u8],
) -> Result<DeclaredBinaryCellEmbeddingCentroidDiscrepancy, CentroidCodecError> {
    if bytes.len() != CODEC_LENGTH {
        return Err(CentroidCodecError::InvalidLength);
    }
    if &bytes[..8] != CODEC_MAGIC {
        return Err(CentroidCodecError::InvalidMagic);
    }
    if bytes[8] != CODEC_VERSION {
        return Err(CentroidCodecError::UnsupportedVersion);
    }
    let value = f64::from_bits(u64::from_be_bytes(
        bytes[10..18]
            .try_into()
            .map_err(|_| CentroidCodecError::InvalidLength)?,
    ));
    let decoded_value = match bytes[9] {
        0 if binding.status()
            == DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus::InsufficientGroups
            && value.to_bits() == 0.0_f64.to_bits() =>
        {
            None
        }
        1 if binding.status()
            == DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus::Available =>
        {
            Some(value)
        }
        0 | 1 => return Err(CentroidCodecError::BindingMismatch),
        _ => return Err(CentroidCodecError::InvalidStatus),
    };
    binding
        .attach_value(decoded_value)
        .ok_or(CentroidCodecError::InvalidValue)
}

#[derive(Debug, Error)]
enum CentroidNodeInputError {
    #[error("required centroid workflow semantic artifact is absent")]
    SemanticArtifactMissing,
    #[error("centroid workflow input binding changed")]
    BindingChanged,
}

#[derive(Debug, Error)]
enum CentroidCodecError {
    #[error("centroid cache payload has the wrong length")]
    InvalidLength,
    #[error("centroid cache payload has the wrong magic")]
    InvalidMagic,
    #[error("centroid cache payload has an unsupported version")]
    UnsupportedVersion,
    #[error("centroid cache payload has an invalid status")]
    InvalidStatus,
    #[error("centroid cache payload disagrees with node-bound inputs")]
    BindingMismatch,
    #[error("centroid cache payload has an invalid numeric value")]
    InvalidValue,
    #[error("centroid cache payload allocation failed")]
    AllocationFailed,
}
