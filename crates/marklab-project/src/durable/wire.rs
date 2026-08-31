use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{ArtifactRef, ArtifactSchema, ContentDigest};

use super::DurableProjectError;

pub(super) const HEAD_PATH: &str = "project.json";
pub(super) const LEDGER_PATH: &str = "executions.jsonl";
pub(super) const PENDING_PATH: &str = "pending-execution.json";
pub(super) const LOCK_PATH: &str = ".marklab-project.lock";
pub(super) const STORE_DIRECTORY: &str = "artifacts";
pub(super) const STORE_ID: &str = "project_objects";
pub(super) const PROJECT_FORMAT: &str = "marklab.project";
pub(super) const EXECUTION_FORMAT: &str = "marklab.execution";
pub(super) const PENDING_FORMAT: &str = "marklab.pending_execution";
pub(super) const FORMAT_VERSION: u32 = 1;
pub(super) const STORE_POLICY: &str = "local_content_addressed_sha256_v1";
pub(super) const OUTPUT_SCHEMA_ID: &str = "marklab.workflow_node_output";
pub(super) const OUTPUT_SCHEMA_VERSION: u32 = 1;
pub(super) const MAX_EXECUTION_INPUTS: usize = 64;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ArtifactWire {
    pub(super) kind: String,
    pub(super) digest: String,
    pub(super) byte_len: u64,
}

impl From<&ArtifactRef> for ArtifactWire {
    fn from(value: &ArtifactRef) -> Self {
        Self {
            kind: value.kind().to_owned(),
            digest: value.digest().to_string(),
            byte_len: value.byte_len(),
        }
    }
}

impl ArtifactWire {
    pub(super) fn to_artifact_ref(&self) -> Result<ArtifactRef, DurableProjectError> {
        Ok(ArtifactRef::new(
            self.kind.clone(),
            parse_digest(&self.digest)?,
            self.byte_len,
        )?)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GitWire {
    pub(super) availability: String,
    pub(super) sha: Option<String>,
    pub(super) dirty: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NativeRuntimeWire {
    pub(super) backend: String,
    pub(super) crate_version: String,
    pub(super) git: GitWire,
    pub(super) rustc_version: String,
    pub(super) features: Vec<String>,
    pub(super) executable: ArtifactWire,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SchemaWire {
    pub(super) id: String,
    pub(super) version: u32,
}

impl From<&ArtifactSchema> for SchemaWire {
    fn from(value: &ArtifactSchema) -> Self {
        Self {
            id: value.id().to_owned(),
            version: value.version(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NodeWire {
    pub(super) id: String,
    pub(super) spec_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExecutionIdentityWire {
    pub(super) node: NodeWire,
    pub(super) inputs: Vec<ArtifactWire>,
    pub(super) configuration_digest: String,
    pub(super) execution_policy_digest: String,
    pub(super) scheduler_output_limit_bytes: u64,
    pub(super) runtime: NativeRuntimeWire,
    pub(super) cache_key: String,
    pub(super) result_schema: SchemaWire,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OutputWire {
    pub(super) artifact_id: String,
    pub(super) semantic_digest: String,
    pub(super) content: ArtifactWire,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExecutionWire {
    pub(super) format: String,
    pub(super) version: u32,
    pub(super) sequence: u64,
    pub(super) previous_record_digest: Option<String>,
    pub(super) recorded_unix_ms: u64,
    pub(super) identity: ExecutionIdentityWire,
    pub(super) output: OutputWire,
    pub(super) terminal_disposition: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PendingWire {
    pub(super) format: String,
    pub(super) version: u32,
    pub(super) execution: ExecutionWire,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoreWire {
    pub(super) id: String,
    pub(super) policy: String,
    pub(super) directory: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LedgerHeadWire {
    pub(super) path: String,
    pub(super) record_count: u64,
    pub(super) byte_len: u64,
    pub(super) tail_digest: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestExecutionWire {
    pub(super) sequence: u64,
    pub(super) node_id: String,
    pub(super) cache_key: String,
    pub(super) output_artifact_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProjectHeadWire {
    pub(super) format: String,
    pub(super) version: u32,
    pub(super) project_id: String,
    pub(super) store: StoreWire,
    pub(super) ledger: LedgerHeadWire,
    pub(super) latest_execution: Option<LatestExecutionWire>,
    pub(super) artifacts: Vec<ArtifactWire>,
}

pub(super) fn canonical_compact<T: Serialize>(
    value: &T,
    context: &'static str,
) -> Result<Vec<u8>, DurableProjectError> {
    serde_json::to_vec(value).map_err(|source| DurableProjectError::Json {
        path: context.to_owned(),
        reason: source.to_string(),
    })
}

pub(super) fn canonical_pretty<T: Serialize>(
    value: &T,
    context: &'static str,
) -> Result<Vec<u8>, DurableProjectError> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|source| DurableProjectError::Json {
            path: context.to_owned(),
            reason: source.to_string(),
        })?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn decode_canonical_compact<T>(
    path: &str,
    bytes: &[u8],
) -> Result<T, DurableProjectError>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    let value: T = serde_json::from_slice(bytes).map_err(|source| DurableProjectError::Json {
        path: path.to_owned(),
        reason: source.to_string(),
    })?;
    if canonical_compact(&value, "decoded compact state")? != bytes {
        return Err(DurableProjectError::NonCanonicalState {
            path: path.to_owned(),
        });
    }
    Ok(value)
}

pub(super) fn decode_canonical_pretty<T>(path: &str, bytes: &[u8]) -> Result<T, DurableProjectError>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    let value: T = serde_json::from_slice(bytes).map_err(|source| DurableProjectError::Json {
        path: path.to_owned(),
        reason: source.to_string(),
    })?;
    if canonical_pretty(&value, "decoded pretty state")? != bytes {
        return Err(DurableProjectError::NonCanonicalState {
            path: path.to_owned(),
        });
    }
    Ok(value)
}

pub(super) fn ensure_size(
    path: &str,
    observed: usize,
    maximum: usize,
) -> Result<(), DurableProjectError> {
    if observed > maximum {
        Err(DurableProjectError::StateTooLarge {
            path: path.to_owned(),
            observed,
            maximum,
        })
    } else {
        Ok(())
    }
}

pub(super) fn parse_digest(value: &str) -> Result<ContentDigest, DurableProjectError> {
    ContentDigest::from_str(value).map_err(|_| DurableProjectError::InvalidDigest {
        value: value.to_owned(),
    })
}

pub(super) fn valid_digest(value: &str) -> bool {
    ContentDigest::from_str(value).is_ok()
}

pub(super) fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}
