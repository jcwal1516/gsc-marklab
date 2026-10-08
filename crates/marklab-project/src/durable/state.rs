use std::{
    collections::BTreeSet,
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use cap_std::fs::{Dir, OpenOptions};

use crate::{ArtifactSchema, ContentDigest};

use super::{
    api::DurableProjectLimits,
    layout::{
        atomic_write, create_empty_ledger, path_exists, read_bounded, validate_regular_metadata,
    },
    wire::{
        canonical_compact, canonical_pretty, decode_canonical_compact, decode_canonical_pretty,
        ensure_size, parse_digest, valid_digest, valid_token, ExecutionWire, LatestExecutionWire,
        LedgerHeadWire, ProjectHeadWire, StoreWire, EXECUTION_FORMAT, FORMAT_VERSION, HEAD_PATH,
        LEDGER_PATH, MAX_EXECUTION_INPUTS, PENDING_PATH, PROJECT_FORMAT, STORE_DIRECTORY, STORE_ID,
        STORE_POLICY,
    },
    DurableProjectError, NativeRuntimeProvenance,
};

pub(super) fn open_or_initialize_state(
    root: &Dir,
    root_path: &Path,
    limits: DurableProjectLimits,
) -> Result<(ProjectHeadWire, Vec<ExecutionWire>), DurableProjectError> {
    let head_exists = path_exists(root, HEAD_PATH)?;
    let ledger_exists = path_exists(root, LEDGER_PATH)?;
    match (head_exists, ledger_exists) {
        (false, false) => {
            create_empty_ledger(root)?;
            let head = empty_head(generate_project_id(root_path)?);
            write_head(root, &head, limits)?;
            Ok((head, Vec::new()))
        }
        (true, true) => {
            let head = read_head(root, limits)?;
            let (ledger, byte_len) = read_ledger(root, limits)?;
            if !path_exists(root, PENDING_PATH)? {
                validate_head_against_ledger(&head, &ledger, byte_len)?;
            }
            Ok((head, ledger))
        }
        _ => Err(DurableProjectError::IncompleteProjectState),
    }
}

fn empty_head(project_id: String) -> ProjectHeadWire {
    ProjectHeadWire {
        format: PROJECT_FORMAT.to_owned(),
        version: FORMAT_VERSION,
        project_id,
        store: StoreWire {
            id: STORE_ID.to_owned(),
            policy: STORE_POLICY.to_owned(),
            directory: STORE_DIRECTORY.to_owned(),
        },
        ledger: LedgerHeadWire {
            path: LEDGER_PATH.to_owned(),
            record_count: 0,
            byte_len: 0,
            tail_digest: None,
        },
        latest_execution: None,
        artifacts: Vec::new(),
    }
}

fn generate_project_id(root: &Path) -> Result<String, DurableProjectError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| DurableProjectError::ClockBeforeEpoch)?
        .as_nanos()
        .to_be_bytes();
    let process = std::process::id().to_be_bytes();
    Ok(ContentDigest::from_framed([
        b"marklab-project-id-v1".as_slice(),
        root.as_os_str().to_string_lossy().as_bytes(),
        now.as_slice(),
        process.as_slice(),
    ])
    .to_string())
}

fn read_head(
    root: &Dir,
    limits: DurableProjectLimits,
) -> Result<ProjectHeadWire, DurableProjectError> {
    let bytes = read_bounded(root, HEAD_PATH, limits.maximum_control_bytes)?;
    let head: ProjectHeadWire = decode_canonical_pretty(HEAD_PATH, &bytes)?;
    validate_head_shape(&head)?;
    Ok(head)
}

fn validate_head_shape(head: &ProjectHeadWire) -> Result<(), DurableProjectError> {
    if head.format != PROJECT_FORMAT {
        return Err(DurableProjectError::UnsupportedFormat {
            path: HEAD_PATH.to_owned(),
            observed: head.format.clone(),
        });
    }
    if head.version != FORMAT_VERSION {
        return Err(DurableProjectError::UnsupportedVersion {
            path: HEAD_PATH.to_owned(),
            observed: head.version,
        });
    }
    if !valid_digest(&head.project_id)
        || head.store.id != STORE_ID
        || head.store.policy != STORE_POLICY
        || head.store.directory != STORE_DIRECTORY
        || head.ledger.path != LEDGER_PATH
    {
        return Err(DurableProjectError::InvalidHead);
    }
    if let Some(tail) = &head.ledger.tail_digest {
        parse_digest(tail)?;
    }
    for artifact in &head.artifacts {
        artifact.to_artifact_ref()?;
    }
    Ok(())
}

fn read_ledger(
    root: &Dir,
    limits: DurableProjectLimits,
) -> Result<(Vec<ExecutionWire>, u64), DurableProjectError> {
    let bytes = read_bounded(root, LEDGER_PATH, limits.maximum_ledger_bytes)?;
    if bytes.is_empty() {
        return Ok((Vec::new(), 0));
    }
    if bytes.last() != Some(&b'\n') {
        return Err(DurableProjectError::TruncatedLedger);
    }
    let mut ledger = Vec::new();
    let mut previous_digest = None;
    let mut cache_keys = BTreeSet::new();
    let mut previous_time = 0;
    let records = &bytes[..bytes.len() - 1];
    for (index, line) in records.split(|byte| *byte == b'\n').enumerate() {
        if line.is_empty() {
            return Err(DurableProjectError::MalformedLedger {
                reason: "empty record".to_owned(),
            });
        }
        if line.len() > limits.maximum_record_bytes {
            return Err(DurableProjectError::StateTooLarge {
                path: LEDGER_PATH.to_owned(),
                observed: line.len(),
                maximum: limits.maximum_record_bytes,
            });
        }
        if index == limits.maximum_ledger_records {
            return Err(DurableProjectError::TooManyLedgerRecords {
                observed: index + 1,
                maximum: limits.maximum_ledger_records,
            });
        }
        let entry: ExecutionWire = decode_canonical_compact(LEDGER_PATH, line)?;
        validate_execution(&entry, limits)?;
        let expected_sequence = u64::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(DurableProjectError::LedgerSequenceOverflow)?;
        if entry.sequence != expected_sequence
            || entry.previous_record_digest != previous_digest
            || entry.recorded_unix_ms < previous_time
        {
            return Err(DurableProjectError::InvalidLedgerChain {
                sequence: entry.sequence,
            });
        }
        if !cache_keys.insert(entry.identity.cache_key.clone()) {
            return Err(DurableProjectError::DuplicateLedgerCacheKey {
                cache_key: parse_digest(&entry.identity.cache_key)?,
            });
        }
        previous_digest = Some(ContentDigest::from_bytes(line).to_string());
        previous_time = entry.recorded_unix_ms;
        ledger.push(entry);
    }
    Ok((
        ledger,
        u64::try_from(bytes.len()).map_err(|_| DurableProjectError::LedgerSequenceOverflow)?,
    ))
}

pub(super) fn validate_execution(
    entry: &ExecutionWire,
    limits: DurableProjectLimits,
) -> Result<(), DurableProjectError> {
    if entry.format != EXECUTION_FORMAT {
        return Err(DurableProjectError::UnsupportedFormat {
            path: LEDGER_PATH.to_owned(),
            observed: entry.format.clone(),
        });
    }
    if entry.version != FORMAT_VERSION {
        return Err(DurableProjectError::UnsupportedVersion {
            path: LEDGER_PATH.to_owned(),
            observed: entry.version,
        });
    }
    if entry.sequence == 0
        || entry.recorded_unix_ms == 0
        || entry.terminal_disposition != "success"
        || !valid_token(&entry.identity.node.id)
        || entry.identity.inputs.is_empty()
        || entry.identity.inputs.len() > MAX_EXECUTION_INPUTS
        || entry.identity.scheduler_output_limit_bytes == 0
    {
        return Err(DurableProjectError::MalformedLedger {
            reason: "invalid required execution field".to_owned(),
        });
    }
    if let Some(previous) = &entry.previous_record_digest {
        parse_digest(previous)?;
    }
    parse_digest(&entry.identity.node.spec_digest)?;
    parse_digest(&entry.identity.configuration_digest)?;
    parse_digest(&entry.identity.execution_policy_digest)?;
    parse_digest(&entry.identity.cache_key)?;
    for input in &entry.identity.inputs {
        input.to_artifact_ref()?;
    }
    ArtifactSchema::new(
        entry.identity.result_schema.id.clone(),
        entry.identity.result_schema.version,
    )?;
    NativeRuntimeProvenance::from_wire(entry.identity.runtime.clone())?;
    let content = entry.output.content.to_artifact_ref()?;
    let maximum = u64::try_from(limits.maximum_object_bytes).unwrap_or(u64::MAX);
    if content.byte_len() > maximum {
        return Err(DurableProjectError::ObjectTooLarge {
            observed: usize::try_from(content.byte_len()).unwrap_or(usize::MAX),
            maximum: limits.maximum_object_bytes,
        });
    }
    super::execution::record_from_entry(entry)?;
    Ok(())
}

pub(super) fn validate_head_against_ledger(
    head: &ProjectHeadWire,
    ledger: &[ExecutionWire],
    byte_len: u64,
) -> Result<(), DurableProjectError> {
    validate_head_shape(head)?;
    let count =
        u64::try_from(ledger.len()).map_err(|_| DurableProjectError::LedgerSequenceOverflow)?;
    let tail = ledger
        .last()
        .map(execution_digest)
        .transpose()?
        .map(|digest| digest.to_string());
    if head.ledger.record_count != count
        || head.ledger.byte_len != byte_len
        || head.ledger.tail_digest != tail
    {
        return Err(DurableProjectError::HeadLedgerMismatch);
    }
    match (ledger.last(), &head.latest_execution) {
        (None, None) if head.artifacts.is_empty() => Ok(()),
        (Some(entry), Some(latest)) => {
            let expected =
                head_for_execution(head.project_id.clone(), entry, ledger.len(), byte_len)?;
            if latest == expected.latest_execution.as_ref().expect("latest")
                && head.artifacts == expected.artifacts
            {
                Ok(())
            } else {
                Err(DurableProjectError::HeadLedgerMismatch)
            }
        }
        _ => Err(DurableProjectError::HeadLedgerMismatch),
    }
}

pub(super) fn head_for_execution(
    project_id: String,
    entry: &ExecutionWire,
    record_count: usize,
    byte_len: u64,
) -> Result<ProjectHeadWire, DurableProjectError> {
    let count =
        u64::try_from(record_count).map_err(|_| DurableProjectError::LedgerSequenceOverflow)?;
    let mut artifacts = entry.identity.inputs.clone();
    artifacts.push(entry.output.content.clone());
    Ok(ProjectHeadWire {
        format: PROJECT_FORMAT.to_owned(),
        version: FORMAT_VERSION,
        project_id,
        store: StoreWire {
            id: STORE_ID.to_owned(),
            policy: STORE_POLICY.to_owned(),
            directory: STORE_DIRECTORY.to_owned(),
        },
        ledger: LedgerHeadWire {
            path: LEDGER_PATH.to_owned(),
            record_count: count,
            byte_len,
            tail_digest: Some(execution_digest(entry)?.to_string()),
        },
        latest_execution: Some(LatestExecutionWire {
            sequence: entry.sequence,
            node_id: entry.identity.node.id.clone(),
            cache_key: entry.identity.cache_key.clone(),
            output_artifact_id: entry.output.artifact_id.clone(),
        }),
        artifacts,
    })
}

pub(super) fn append_execution(
    root: &Dir,
    entry: &ExecutionWire,
    limits: DurableProjectLimits,
) -> Result<(), DurableProjectError> {
    let encoded = prepare_execution_append(root, entry, limits)?;
    let mut options = OpenOptions::new();
    options.write(true).append(true);
    let mut file =
        root.open_with(LEDGER_PATH, &options)
            .map_err(|source| DurableProjectError::Io {
                operation: "open execution ledger for append",
                path: LEDGER_PATH.to_owned(),
                source,
            })?;
    file.write_all(&encoded)
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.sync_all())
        .map_err(|source| DurableProjectError::Io {
            operation: "append and sync execution ledger",
            path: LEDGER_PATH.to_owned(),
            source,
        })
}

pub(super) fn prepare_execution_append(
    root: &Dir,
    entry: &ExecutionWire,
    limits: DurableProjectLimits,
) -> Result<Vec<u8>, DurableProjectError> {
    let encoded = canonical_compact(entry, "execution record")?;
    ensure_size(LEDGER_PATH, encoded.len(), limits.maximum_record_bytes)?;
    let current = root
        .symlink_metadata(LEDGER_PATH)
        .map_err(|source| DurableProjectError::Io {
            operation: "inspect execution ledger",
            path: LEDGER_PATH.to_owned(),
            source,
        })?;
    validate_regular_metadata(LEDGER_PATH, current)?;
    let current_len = usize::try_from(
        root.metadata(LEDGER_PATH)
            .map_err(|source| DurableProjectError::Io {
                operation: "measure execution ledger",
                path: LEDGER_PATH.to_owned(),
                source,
            })?
            .len(),
    )
    .unwrap_or(usize::MAX);
    let appended = encoded.len().saturating_add(1);
    if current_len.saturating_add(appended) > limits.maximum_ledger_bytes {
        return Err(DurableProjectError::StateTooLarge {
            path: LEDGER_PATH.to_owned(),
            observed: current_len.saturating_add(appended),
            maximum: limits.maximum_ledger_bytes,
        });
    }
    Ok(encoded)
}

pub(super) fn ledger_byte_len(ledger: &[ExecutionWire]) -> Result<u64, DurableProjectError> {
    ledger.iter().try_fold(0_u64, |total, entry| {
        let encoded = canonical_compact(entry, "execution record")?;
        let length = u64::try_from(encoded.len())
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(DurableProjectError::LedgerSequenceOverflow)?;
        total
            .checked_add(length)
            .ok_or(DurableProjectError::LedgerSequenceOverflow)
    })
}

pub(super) fn execution_digest(
    entry: &ExecutionWire,
) -> Result<ContentDigest, DurableProjectError> {
    Ok(ContentDigest::from_bytes(&canonical_compact(
        entry,
        "execution record",
    )?))
}

pub(super) fn monotonic_timestamp(
    previous: Option<&ExecutionWire>,
) -> Result<u64, DurableProjectError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| DurableProjectError::ClockBeforeEpoch)?
        .as_millis();
    let now = u64::try_from(millis).map_err(|_| DurableProjectError::ClockOverflow)?;
    Ok(previous.map_or(now, |entry| now.max(entry.recorded_unix_ms)))
}

pub(super) fn write_head(
    root: &Dir,
    head: &ProjectHeadWire,
    limits: DurableProjectLimits,
) -> Result<(), DurableProjectError> {
    validate_head_shape(head)?;
    let bytes = canonical_pretty(head, "project head")?;
    ensure_size(HEAD_PATH, bytes.len(), limits.maximum_control_bytes)?;
    atomic_write(root, HEAD_PATH, &bytes)
}
