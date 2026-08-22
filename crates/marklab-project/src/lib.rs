#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Content identity and the minimal in-memory Marklab project state.
//!
//! Input artifacts are reference-only. The project stores bytes only for the
//! bounded result cache committed by the workflow scheduler.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    io::{self, Write},
};

use sha2::{Digest, Sha256};
use thiserror::Error;

/// Default maximum retained bytes for one inline cached result (16 MiB).
pub const DEFAULT_MAX_INLINE_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;

/// A SHA-256 content digest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentDigest([u8; 32]);

impl ContentDigest {
    /// Hash one contiguous byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        Self(hasher.finalize().into())
    }

    /// Hash an ordered sequence using an unambiguous 128-bit length prefix per field.
    pub fn from_framed<'a>(parts: impl IntoIterator<Item = &'a [u8]>) -> Self {
        let mut hasher = Sha256::new();
        for part in parts {
            hasher.update((part.len() as u128).to_be_bytes());
            hasher.update(part);
        }
        Self(hasher.finalize().into())
    }

    /// Create a streaming SHA-256 writer.
    pub fn builder() -> ContentDigestWriter {
        ContentDigestWriter::default()
    }

    /// Return the raw digest bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ContentDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Streaming content hasher with an exact byte count.
pub struct ContentDigestWriter {
    hasher: Sha256,
    byte_len: u64,
}

impl Default for ContentDigestWriter {
    fn default() -> Self {
        Self {
            hasher: Sha256::new(),
            byte_len: 0,
        }
    }
}

impl Write for ContentDigestWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let added = u64::try_from(buffer.len()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "content length exceeds u64")
        })?;
        self.byte_len = self.byte_len.checked_add(added).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "content length exceeds u64")
        })?;
        self.hasher.update(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl ContentDigestWriter {
    /// Finish hashing and return the digest and exact number of bytes written.
    pub fn finish(self) -> (ContentDigest, u64) {
        (ContentDigest(self.hasher.finalize().into()), self.byte_len)
    }
}

/// Immutable metadata that identifies artifact content without retaining it.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactRef {
    kind: String,
    digest: ContentDigest,
    byte_len: u64,
}

impl ArtifactRef {
    /// Build a reference from an already-computed digest and length.
    pub fn new(
        kind: impl Into<String>,
        digest: ContentDigest,
        byte_len: u64,
    ) -> Result<Self, ProjectError> {
        let kind = kind.into();
        validate_kind(&kind)?;
        Ok(Self {
            kind,
            digest,
            byte_len,
        })
    }

    /// Hash a bounded byte slice and build its reference.
    pub fn from_bytes(kind: impl Into<String>, bytes: &[u8]) -> Result<Self, ProjectError> {
        let byte_len = u64::try_from(bytes.len()).map_err(|_| ProjectError::ArtifactTooLarge)?;
        Self::new(kind, ContentDigest::from_bytes(bytes), byte_len)
    }

    /// Artifact media/schema kind.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// SHA-256 content identity.
    pub fn digest(&self) -> ContentDigest {
        self.digest
    }

    /// Exact encoded byte length.
    pub fn byte_len(&self) -> u64 {
        self.byte_len
    }

    /// Verify this reference against bounded bytes.
    pub fn verify_bytes(&self, bytes: &[u8]) -> Result<(), ProjectError> {
        let observed = Self::from_bytes(self.kind.clone(), bytes)?;
        self.verify_identity(observed.digest, observed.byte_len)
    }

    /// Verify this reference against a separately streamed digest and length.
    pub fn verify_identity(
        &self,
        digest: ContentDigest,
        byte_len: u64,
    ) -> Result<(), ProjectError> {
        if self.digest == digest && self.byte_len == byte_len {
            Ok(())
        } else {
            Err(ProjectError::DigestMismatch {
                kind: self.kind.clone(),
                expected: self.digest,
                observed: digest,
                expected_len: self.byte_len,
                observed_len: byte_len,
            })
        }
    }
}

/// A successful, cache-addressed node execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuccessfulRun {
    node_id: String,
    cache_key: ContentDigest,
    output: ArtifactRef,
}

impl SuccessfulRun {
    /// Node that produced the successful output.
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Deterministic key under which the output was committed.
    pub fn cache_key(&self) -> ContentDigest {
        self.cache_key
    }

    /// Content reference for the committed output.
    pub fn output(&self) -> &ArtifactRef {
        &self.output
    }
}

/// Minimal project state for the first workflow vertical slice.
pub struct MarklabProject {
    max_inline_artifact_bytes: usize,
    references: BTreeSet<ArtifactRef>,
    inline_artifacts: BTreeMap<ArtifactRef, Box<[u8]>>,
    successful_runs: BTreeMap<(String, ContentDigest), SuccessfulRun>,
}

impl Default for MarklabProject {
    fn default() -> Self {
        Self {
            max_inline_artifact_bytes: DEFAULT_MAX_INLINE_ARTIFACT_BYTES,
            references: BTreeSet::new(),
            inline_artifacts: BTreeMap::new(),
            successful_runs: BTreeMap::new(),
        }
    }
}

impl MarklabProject {
    /// Create an empty in-memory project.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an empty project with an explicit positive per-artifact inline limit.
    pub fn with_inline_artifact_limit(
        max_inline_artifact_bytes: usize,
    ) -> Result<Self, ProjectError> {
        if max_inline_artifact_bytes == 0 {
            return Err(ProjectError::InvalidInlineArtifactLimit);
        }
        Ok(Self {
            max_inline_artifact_bytes,
            ..Self::default()
        })
    }

    /// Maximum bytes this project will retain for one inline artifact.
    pub fn max_inline_artifact_bytes(&self) -> usize {
        self.max_inline_artifact_bytes
    }

    /// Catalog immutable artifact metadata without copying its content.
    pub fn register_reference(&mut self, artifact: ArtifactRef) -> Result<(), ProjectError> {
        if let Some(bytes) = self.inline_artifacts.get(&artifact) {
            artifact.verify_bytes(bytes)?;
        }
        self.references.insert(artifact);
        Ok(())
    }

    /// Whether this project catalogs the exact reference.
    pub fn contains(&self, artifact: &ArtifactRef) -> bool {
        self.references.contains(artifact)
    }

    /// Return the output reference for an exact successful node/cache key.
    pub fn cached_output(&self, node_id: &str, cache_key: ContentDigest) -> Option<&ArtifactRef> {
        self.successful_run(node_id, cache_key)
            .map(SuccessfulRun::output)
    }

    /// Return the successful execution record for an exact node/cache key.
    pub fn successful_run(
        &self,
        node_id: &str,
        cache_key: ContentDigest,
    ) -> Option<&SuccessfulRun> {
        self.successful_runs.get(&(node_id.to_owned(), cache_key))
    }

    /// Read a cached bounded artifact after rechecking its digest and length.
    pub fn read_inline_verified(&self, artifact: &ArtifactRef) -> Result<&[u8], ProjectError> {
        let bytes = self.inline_artifacts.get(artifact).ok_or_else(|| {
            ProjectError::MissingInlineArtifact {
                kind: artifact.kind.clone(),
                digest: artifact.digest,
            }
        })?;
        artifact.verify_bytes(bytes)?;
        Ok(bytes)
    }

    /// Atomically record a bounded encoded output and its successful execution.
    pub fn commit_success(
        &mut self,
        node_id: &str,
        cache_key: ContentDigest,
        output_kind: &str,
        encoded_output: Box<[u8]>,
    ) -> Result<ArtifactRef, ProjectError> {
        validate_node_id(node_id)?;
        if encoded_output.len() > self.max_inline_artifact_bytes {
            return Err(ProjectError::InlineArtifactTooLarge {
                encoded_bytes: encoded_output.len(),
                limit_bytes: self.max_inline_artifact_bytes,
            });
        }
        let output = ArtifactRef::from_bytes(output_kind, &encoded_output)?;
        let record_key = (node_id.to_owned(), cache_key);

        if let Some(existing) = self.successful_runs.get(&record_key) {
            if existing.output != output {
                return Err(ProjectError::ConflictingSuccess {
                    node_id: node_id.to_owned(),
                    cache_key,
                });
            }
            let existing_bytes = self.read_inline_verified(&output)?;
            if existing_bytes != encoded_output.as_ref() {
                return Err(ProjectError::DigestCollision { artifact: output });
            }
            return Ok(existing.output.clone());
        }

        if let Some(existing_bytes) = self.inline_artifacts.get(&output) {
            if existing_bytes.as_ref() != encoded_output.as_ref() {
                return Err(ProjectError::DigestCollision { artifact: output });
            }
        }

        let run = SuccessfulRun {
            node_id: node_id.to_owned(),
            cache_key,
            output: output.clone(),
        };
        self.references.insert(output.clone());
        self.inline_artifacts
            .entry(output.clone())
            .or_insert(encoded_output);
        self.successful_runs.insert(record_key, run);
        Ok(output)
    }

    /// Number of unique reference-only and inline artifacts.
    pub fn artifact_count(&self) -> usize {
        self.references.len()
    }

    /// Number of unique bounded artifacts whose bytes are retained inline.
    pub fn inline_artifact_count(&self) -> usize {
        self.inline_artifacts.len()
    }

    /// Number of successful node/cache-key records.
    pub fn successful_run_count(&self) -> usize {
        self.successful_runs.len()
    }
}

/// Project identity, integrity, and commit failures.
#[derive(Debug, Error)]
pub enum ProjectError {
    /// Artifact kind is empty, too long, or contains non-visible ASCII.
    #[error("artifact kind must be 1-255 visible ASCII bytes")]
    InvalidArtifactKind,
    /// Encoded artifact length cannot be represented as `u64`.
    #[error("artifact length exceeds u64")]
    ArtifactTooLarge,
    /// Project-level inline artifact limit is zero.
    #[error("project inline-artifact limit must be positive")]
    InvalidInlineArtifactLimit,
    /// Encoded output exceeds the project's per-artifact inline limit.
    #[error("encoded inline artifact has {encoded_bytes} bytes, exceeding limit {limit_bytes}")]
    InlineArtifactTooLarge {
        /// Actual encoded byte length.
        encoded_bytes: usize,
        /// Configured project limit.
        limit_bytes: usize,
    },
    /// Recomputed content identity differs from the recorded reference.
    #[error(
        "artifact digest mismatch for {kind}: expected {expected}/{expected_len} bytes, observed {observed}/{observed_len} bytes"
    )]
    DigestMismatch {
        /// Artifact media/schema kind.
        kind: String,
        /// Recorded SHA-256 digest.
        expected: ContentDigest,
        /// Recomputed SHA-256 digest.
        observed: ContentDigest,
        /// Recorded byte length.
        expected_len: u64,
        /// Recomputed byte length.
        observed_len: u64,
    },
    /// A success record names inline content that is not retained.
    #[error("inline artifact is missing for {kind} digest {digest}")]
    MissingInlineArtifact {
        /// Artifact media/schema kind.
        kind: String,
        /// Missing content digest.
        digest: ContentDigest,
    },
    /// The same node/cache key was already committed with another output.
    #[error("conflicting successful output for node {node_id} cache key {cache_key}")]
    ConflictingSuccess {
        /// Conflicting node identifier.
        node_id: String,
        /// Conflicting cache key.
        cache_key: ContentDigest,
    },
    /// Different bytes produced the same full artifact identity.
    #[error("distinct inline bytes share artifact identity {artifact:?}")]
    DigestCollision {
        /// Colliding artifact identity.
        artifact: ArtifactRef,
    },
    /// Successful-run node ID violates the shared ID syntax.
    #[error("successful-run node ID must be 1-128 ASCII alphanumeric, '.', '-', or '_'")]
    InvalidNodeId,
}

fn validate_kind(kind: &str) -> Result<(), ProjectError> {
    if kind.is_empty()
        || kind.len() > 255
        || !kind
            .as_bytes()
            .iter()
            .all(|byte| (0x21..=0x7e).contains(byte))
    {
        Err(ProjectError::InvalidArtifactKind)
    } else {
        Ok(())
    }
}

fn validate_node_id(node_id: &str) -> Result<(), ProjectError> {
    if node_id.is_empty()
        || node_id.len() > 128
        || !node_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        Err(ProjectError::InvalidNodeId)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn sha256_matches_the_standard_abc_vector_and_streaming_count() {
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(ContentDigest::from_bytes(b"abc").to_string(), expected);

        let mut writer = ContentDigest::builder();
        writer.write_all(b"a").expect("a");
        writer.write_all(b"bc").expect("bc");
        let (digest, byte_len) = writer.finish();
        assert_eq!(digest.to_string(), expected);
        assert_eq!(byte_len, 3);
    }

    #[test]
    fn project_keeps_inputs_reference_only_and_commits_bounded_output() {
        let input = ArtifactRef::from_bytes("application/test-input", b"input").expect("input");
        let mut project = MarklabProject::new();
        project.register_reference(input).expect("reference");
        assert_eq!(project.artifact_count(), 1);
        assert_eq!(project.inline_artifact_count(), 0);

        let key = ContentDigest::from_bytes(b"key");
        let output = project
            .commit_success(
                "node",
                key,
                "application/test-output",
                b"output".to_vec().into_boxed_slice(),
            )
            .expect("commit");
        assert_eq!(
            project.read_inline_verified(&output).expect("read"),
            b"output"
        );
        assert_eq!(project.artifact_count(), 2);
        assert_eq!(project.inline_artifact_count(), 1);
        assert_eq!(project.successful_run_count(), 1);
    }

    #[test]
    fn conflicting_success_is_rejected_without_mutating_committed_state() {
        let mut project = MarklabProject::new();
        let key = ContentDigest::from_bytes(b"key");
        project
            .commit_success(
                "node",
                key,
                "application/test-output",
                b"first".to_vec().into_boxed_slice(),
            )
            .expect("first commit");
        let before = (
            project.artifact_count(),
            project.inline_artifact_count(),
            project.successful_run_count(),
        );

        assert!(matches!(
            project.commit_success(
                "node",
                key,
                "application/test-output",
                b"second".to_vec().into_boxed_slice(),
            ),
            Err(ProjectError::ConflictingSuccess { .. })
        ));
        assert_eq!(
            (
                project.artifact_count(),
                project.inline_artifact_count(),
                project.successful_run_count(),
            ),
            before
        );
    }

    #[test]
    fn direct_commit_cannot_bypass_the_project_inline_limit() {
        let mut project = MarklabProject::with_inline_artifact_limit(4).expect("project");
        let before = (
            project.artifact_count(),
            project.inline_artifact_count(),
            project.successful_run_count(),
        );
        assert!(matches!(
            project.commit_success(
                "node",
                ContentDigest::from_bytes(b"key"),
                "application/test-output",
                b"12345".to_vec().into_boxed_slice(),
            ),
            Err(ProjectError::InlineArtifactTooLarge {
                encoded_bytes: 5,
                limit_bytes: 4
            })
        ));
        assert_eq!(
            (
                project.artifact_count(),
                project.inline_artifact_count(),
                project.successful_run_count(),
            ),
            before
        );
    }
}
