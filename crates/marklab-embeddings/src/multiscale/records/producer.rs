use std::{fmt, mem::size_of};

#[cfg(feature = "parquet")]
use std::io::Read;

use marklab_project::ArtifactId;

mod wire;
use wire::{parse, ProducerWireRef};

use super::codec::{
    preflight_json_strings, require_decoded, require_retained, valid_token,
    MAX_RAW_SMALL_JSON_STRING_BYTES, MAX_SMALL_RECORD_BYTES,
};
#[cfg(feature = "parquet")]
use crate::multiscale::json::{compare_canonical_json_reader, CanonicalJsonReaderError};
use crate::multiscale::{
    cell_patch::CellPatchAssignmentMode,
    error::MultiscaleEmbeddingError,
    json::{canonical_json_len, encode_canonical_json, matches_canonical_json},
};

const FORMAT: &str = "marklab.cell_patch_link_producer";
const VERSION: u32 = 1;
const CONTAINMENT_ALGORITHM: &str = "all_half_open_anchor_containment";

#[derive(Clone, Eq, PartialEq)]
enum ProducerAlgorithm {
    Containment,
    Interpolation(Box<str>),
}

impl ProducerAlgorithm {
    fn as_str(&self) -> &str {
        match self {
            Self::Containment => CONTAINMENT_ALGORITHM,
            Self::Interpolation(value) => value,
        }
    }
}

/// Strict canonical producer descriptor for one vector-free cell-patch link.
#[derive(Clone, Eq, PartialEq)]
pub struct CellPatchLinkProducer {
    assignment_mode: CellPatchAssignmentMode,
    algorithm: ProducerAlgorithm,
    algorithm_version: Box<str>,
    role_artifact_ids: [ArtifactId; 4],
    encoded_len: usize,
}

impl fmt::Debug for CellPatchLinkProducer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CellPatchLinkProducer")
            .field("assignment_mode", &self.assignment_mode)
            .finish_non_exhaustive()
    }
}

impl CellPatchLinkProducer {
    /// Describe deterministic all-containing half-open anchor containment.
    #[allow(clippy::too_many_arguments)]
    pub fn contained_shared(
        algorithm_version: impl Into<String>,
        source_coordinates_artifact_id: ArtifactId,
        run_config_artifact_id: ArtifactId,
        environment_artifact_id: ArtifactId,
        converter_artifact_id: ArtifactId,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        let algorithm_version = algorithm_version.into();
        if !valid_token(&algorithm_version) {
            return Err(MultiscaleEmbeddingError::InvalidCellPatchLinkProducer);
        }
        let required = retained_bytes(0, algorithm_version.capacity())?;
        require_retained(required, maximum_retained_bytes)?;
        Self::finish(
            CellPatchAssignmentMode::ContainedShared,
            ProducerAlgorithm::Containment,
            algorithm_version.into_boxed_str(),
            [
                source_coordinates_artifact_id,
                run_config_artifact_id,
                environment_artifact_id,
                converter_artifact_id,
            ],
        )
    }

    /// Preserve one explicit interpolation producer token without inferring its algorithm.
    #[allow(clippy::too_many_arguments)]
    pub fn declared_weighted_interpolation(
        algorithm: impl Into<String>,
        algorithm_version: impl Into<String>,
        source_coordinates_artifact_id: ArtifactId,
        run_config_artifact_id: ArtifactId,
        environment_artifact_id: ArtifactId,
        converter_artifact_id: ArtifactId,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        let algorithm = algorithm.into();
        let algorithm_version = algorithm_version.into();
        if !valid_token(&algorithm)
            || algorithm == CONTAINMENT_ALGORITHM
            || !valid_token(&algorithm_version)
        {
            return Err(MultiscaleEmbeddingError::InvalidCellPatchLinkProducer);
        }
        let required = retained_bytes(algorithm.capacity(), algorithm_version.capacity())?;
        require_retained(required, maximum_retained_bytes)?;
        Self::finish(
            CellPatchAssignmentMode::DeclaredWeightedInterpolation,
            ProducerAlgorithm::Interpolation(algorithm.into_boxed_str()),
            algorithm_version.into_boxed_str(),
            [
                source_coordinates_artifact_id,
                run_config_artifact_id,
                environment_artifact_id,
                converter_artifact_id,
            ],
        )
    }

    /// Decode and revalidate one exact canonical producer fixed point under caller budgets.
    pub fn from_canonical_json(
        bytes: &[u8],
        maximum_encoded_bytes: usize,
        maximum_decoded_bytes: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        let effective_maximum = maximum_encoded_bytes.min(MAX_SMALL_RECORD_BYTES);
        if bytes.len() > effective_maximum {
            return Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded {
                observed: bytes.len(),
                maximum: effective_maximum,
            });
        }
        preflight_json_strings(bytes, MAX_RAW_SMALL_JSON_STRING_BYTES)?;
        let decoded_required = size_of::<wire::Parsed<'_>>()
            .checked_add(bytes.len())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        require_decoded(decoded_required, maximum_decoded_bytes)?;
        let parsed = parse(bytes)?;
        if parsed.format != FORMAT
            || parsed.version != VERSION
            || !valid_token(parsed.algorithm_version)
        {
            return Err(MultiscaleEmbeddingError::InvalidCellPatchLinkProducer);
        }
        let producer = match parsed.assignment_mode {
            "contained_shared" if parsed.algorithm == CONTAINMENT_ALGORITHM => {
                require_retained(
                    retained_bytes(0, parsed.algorithm_version.len())?,
                    maximum_retained_bytes,
                )?;
                Self::finish(
                    CellPatchAssignmentMode::ContainedShared,
                    ProducerAlgorithm::Containment,
                    parsed.algorithm_version.into(),
                    parsed.role_artifact_ids,
                )?
            }
            "declared_weighted_interpolation"
                if valid_token(parsed.algorithm) && parsed.algorithm != CONTAINMENT_ALGORITHM =>
            {
                require_retained(
                    retained_bytes(parsed.algorithm.len(), parsed.algorithm_version.len())?,
                    maximum_retained_bytes,
                )?;
                Self::finish(
                    CellPatchAssignmentMode::DeclaredWeightedInterpolation,
                    ProducerAlgorithm::Interpolation(parsed.algorithm.into()),
                    parsed.algorithm_version.into(),
                    parsed.role_artifact_ids,
                )?
            }
            _ => return Err(MultiscaleEmbeddingError::InvalidCellPatchLinkProducer),
        };
        if !matches_canonical_json(&producer.wire(), bytes) {
            return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
        }
        Ok(producer)
    }

    /// Encode the exact canonical JSON document with one final newline.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, MultiscaleEmbeddingError> {
        encode_canonical_json(&self.wire(), self.encoded_len, MAX_SMALL_RECORD_BYTES)
    }

    #[cfg(feature = "parquet")]
    pub(in crate::multiscale) fn compare_canonical_json_reader<R: Read + ?Sized>(
        &self,
        reader: &mut R,
    ) -> Result<(), CanonicalJsonReaderError> {
        compare_canonical_json_reader(
            &self.wire(),
            self.encoded_len,
            MAX_SMALL_RECORD_BYTES,
            reader,
        )
    }

    /// Closed assignment mode described by this producer.
    pub fn assignment_mode(&self) -> CellPatchAssignmentMode {
        self.assignment_mode
    }

    /// Exact algorithm token; containment uses its frozen literal.
    pub fn algorithm(&self) -> &str {
        self.algorithm.as_str()
    }

    /// Explicit algorithm version token.
    pub fn algorithm_version(&self) -> &str {
        &self.algorithm_version
    }

    /// Opaque source-coordinate artifact identity.
    pub fn source_coordinates_artifact_id(&self) -> ArtifactId {
        self.role_artifact_ids[0]
    }

    /// Run-configuration artifact identity.
    pub fn run_config_artifact_id(&self) -> ArtifactId {
        self.role_artifact_ids[1]
    }

    /// Execution-environment artifact identity.
    pub fn environment_artifact_id(&self) -> ArtifactId {
        self.role_artifact_ids[2]
    }

    /// Converter artifact identity.
    pub fn converter_artifact_id(&self) -> ArtifactId {
        self.role_artifact_ids[3]
    }

    /// Sorted distinct dependency IDs for the C-03 record boundary.
    pub fn direct_dependencies(&self) -> impl ExactSizeIterator<Item = ArtifactId> {
        let mut dependencies = self.role_artifact_ids;
        dependencies.sort_unstable();
        dependencies.into_iter()
    }

    fn finish(
        assignment_mode: CellPatchAssignmentMode,
        algorithm: ProducerAlgorithm,
        algorithm_version: Box<str>,
        role_artifact_ids: [ArtifactId; 4],
    ) -> Result<Self, MultiscaleEmbeddingError> {
        validate_dependencies(role_artifact_ids)?;
        let mut producer = Self {
            assignment_mode,
            algorithm,
            algorithm_version,
            role_artifact_ids,
            encoded_len: 0,
        };
        let encoded_len = canonical_json_len(&producer.wire())?;
        if encoded_len > MAX_SMALL_RECORD_BYTES {
            return Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded {
                observed: encoded_len,
                maximum: MAX_SMALL_RECORD_BYTES,
            });
        }
        producer.encoded_len = encoded_len;
        Ok(producer)
    }

    fn wire(&self) -> ProducerWireRef<'_> {
        ProducerWireRef::new(self)
    }
}

fn retained_bytes(
    algorithm_bytes: usize,
    version_bytes: usize,
) -> Result<usize, MultiscaleEmbeddingError> {
    size_of::<CellPatchLinkProducer>()
        .checked_add(algorithm_bytes)
        .and_then(|value| value.checked_add(version_bytes))
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)
}

fn validate_dependencies(
    mut dependencies: [ArtifactId; 4],
) -> Result<(), MultiscaleEmbeddingError> {
    dependencies.sort_unstable();
    if dependencies.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(MultiscaleEmbeddingError::DuplicateCellPatchProducerArtifactDependency);
    }
    Ok(())
}
