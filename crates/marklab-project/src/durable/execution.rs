use std::{collections::BTreeMap, io, io::Read, str::FromStr};

use crate::{
    ArtifactDraft, ArtifactLocator, ArtifactRecord, ArtifactRef, ArtifactSchema, MarklabProject,
    StoreId, VerifiedReaderError,
};

use super::{
    api::{DurableCommitDisposition, DurableExecutionRequest, DurableReplay},
    layout::{atomic_write, remove_pending},
    state::{
        append_execution, execution_digest, head_for_execution, ledger_byte_len,
        monotonic_timestamp, prepare_execution_append, validate_execution, write_head,
    },
    wire::{
        canonical_pretty, ensure_size, parse_digest, ArtifactWire, ExecutionWire, OutputWire,
        PendingWire, EXECUTION_FORMAT, FORMAT_VERSION, OUTPUT_SCHEMA_ID, OUTPUT_SCHEMA_VERSION,
        PENDING_FORMAT, PENDING_PATH, STORE_DIRECTORY, STORE_ID,
    },
    DurableProject, DurableProjectError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CommitFault {
    None,
    AfterObjectPublication,
    AfterPendingIntent,
    AfterLedgerAppend,
    AfterHeadReplacement,
}

pub(super) fn restore_success(
    durable: &DurableProject,
    request: &DurableExecutionRequest,
    project: &mut MarklabProject,
) -> Result<Option<DurableReplay>, DurableProjectError> {
    let requested = request.identity_wire();
    let Some(entry) = durable
        .ledger
        .iter()
        .find(|entry| entry.identity.cache_key == requested.cache_key)
    else {
        return Ok(None);
    };
    if entry.identity != requested {
        return Err(DurableProjectError::ConflictingCacheIdentity {
            cache_key: request.cache_key,
        });
    }
    let (record, expected) = record_from_entry(entry)?;
    let bytes = read_verified_object(&durable.store, &record, durable.limits.maximum_object_bytes)?;
    let restored = project.commit_workflow_success(
        &request.node_id,
        request.node_spec_digest,
        request.cache_key,
        expected.kind(),
        bytes.into_boxed_slice(),
    )?;
    if restored != expected {
        return Err(DurableProjectError::RestoredOutputMismatch {
            cache_key: request.cache_key,
        });
    }
    Ok(Some(DurableReplay {
        sequence: entry.sequence,
        output: restored,
    }))
}

pub(super) fn commit_success_inner(
    durable: &mut DurableProject,
    request: &DurableExecutionRequest,
    output_kind: &str,
    encoded_output: &[u8],
    fault: CommitFault,
) -> Result<DurableCommitDisposition, DurableProjectError> {
    if encoded_output.len() > durable.limits.maximum_object_bytes {
        return Err(DurableProjectError::ObjectTooLarge {
            observed: encoded_output.len(),
            maximum: durable.limits.maximum_object_bytes,
        });
    }
    if let Some(existing) = durable
        .ledger
        .iter()
        .find(|entry| entry.identity.cache_key == request.cache_key.to_string())
    {
        if existing.identity != request.identity_wire() {
            return Err(DurableProjectError::ConflictingCacheIdentity {
                cache_key: request.cache_key,
            });
        }
        let (record, expected) = record_from_entry(existing)?;
        let bytes =
            read_verified_object(&durable.store, &record, durable.limits.maximum_object_bytes)?;
        expected.verify_bytes(&bytes)?;
        if bytes != encoded_output {
            return Err(DurableProjectError::RestoredOutputMismatch {
                cache_key: request.cache_key,
            });
        }
        return Ok(DurableCommitDisposition::AlreadyPresent);
    }

    let prospective_records = durable
        .ledger
        .len()
        .checked_add(1)
        .ok_or(DurableProjectError::LedgerSequenceOverflow)?;
    if prospective_records > durable.limits.maximum_ledger_records {
        return Err(DurableProjectError::TooManyLedgerRecords {
            observed: prospective_records,
            maximum: durable.limits.maximum_ledger_records,
        });
    }

    let content = ArtifactRef::from_bytes(output_kind, encoded_output)?;
    let draft = output_draft(request, content.clone())?;
    let publication = durable
        .store
        .publish_new_send(&draft, |writer| writer.write_all(encoded_output))?;
    let record = publication.into_record();
    if fault == CommitFault::AfterObjectPublication {
        return Err(DurableProjectError::InjectedFailure {
            stage: "after object publication",
        });
    }

    let sequence = u64::try_from(durable.ledger.len())
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or(DurableProjectError::LedgerSequenceOverflow)?;
    let previous_record_digest = durable
        .ledger
        .last()
        .map(execution_digest)
        .transpose()?
        .map(|digest| digest.to_string());
    let recorded_unix_ms = monotonic_timestamp(durable.ledger.last())?;
    let entry = ExecutionWire {
        format: EXECUTION_FORMAT.to_owned(),
        version: FORMAT_VERSION,
        sequence,
        previous_record_digest,
        recorded_unix_ms,
        identity: request.identity_wire(),
        output: OutputWire {
            artifact_id: record.id().to_string(),
            semantic_digest: record.semantic_digest().to_string(),
            content: ArtifactWire::from(record.content()),
        },
        terminal_disposition: "success".to_owned(),
    };
    validate_execution(&entry, durable.limits)?;
    let pending = PendingWire {
        format: PENDING_FORMAT.to_owned(),
        version: FORMAT_VERSION,
        execution: entry.clone(),
    };
    let pending_bytes = canonical_pretty(&pending, "pending execution")?;
    ensure_size(
        PENDING_PATH,
        pending_bytes.len(),
        durable.limits.maximum_control_bytes,
    )?;
    prepare_execution_append(&durable.root, &entry, durable.limits)?;
    atomic_write(&durable.root, PENDING_PATH, &pending_bytes)?;
    if fault == CommitFault::AfterPendingIntent {
        return Err(DurableProjectError::InjectedFailure {
            stage: "after pending intent",
        });
    }

    append_execution(&durable.root, &entry, durable.limits)?;
    durable.ledger.push(entry.clone());
    if fault == CommitFault::AfterLedgerAppend {
        return Err(DurableProjectError::InjectedFailure {
            stage: "after ledger append",
        });
    }

    durable.head = head_for_execution(
        durable.head.project_id.clone(),
        &entry,
        durable.ledger.len(),
        ledger_byte_len(&durable.ledger)?,
    )?;
    write_head(&durable.root, &durable.head, durable.limits)?;
    if fault == CommitFault::AfterHeadReplacement {
        return Err(DurableProjectError::InjectedFailure {
            stage: "after head replacement",
        });
    }
    remove_pending(&durable.root)?;
    Ok(DurableCommitDisposition::Appended)
}

fn output_draft(
    request: &DurableExecutionRequest,
    content: ArtifactRef,
) -> Result<ArtifactDraft, DurableProjectError> {
    let mut metadata = BTreeMap::new();
    metadata.insert("cache_key".to_owned(), request.cache_key.to_string());
    metadata.insert("node_id".to_owned(), request.node_id.clone());
    metadata.insert(
        "result_schema_id".to_owned(),
        request.result_schema.id().to_owned(),
    );
    metadata.insert(
        "result_schema_version".to_owned(),
        request.result_schema.version().to_string(),
    );
    Ok(ArtifactDraft::new(
        content,
        ArtifactSchema::new(OUTPUT_SCHEMA_ID, OUTPUT_SCHEMA_VERSION)?,
        None,
        Vec::new(),
        metadata,
    )?)
}

pub(super) fn record_from_entry(
    entry: &ExecutionWire,
) -> Result<(ArtifactRecord, ArtifactRef), DurableProjectError> {
    let runtime = super::NativeRuntimeProvenance::from_wire(entry.identity.runtime.clone())?;
    let request = DurableExecutionRequest::new(
        entry.identity.node.id.clone(),
        parse_digest(&entry.identity.node.spec_digest)?,
        entry
            .identity
            .inputs
            .iter()
            .map(ArtifactWire::to_artifact_ref)
            .collect::<Result<Vec<_>, _>>()?,
        parse_digest(&entry.identity.configuration_digest)?,
        parse_digest(&entry.identity.execution_policy_digest)?,
        usize::try_from(entry.identity.scheduler_output_limit_bytes).map_err(|_| {
            DurableProjectError::InvalidRequest {
                reason: "scheduler limit exceeds usize".to_owned(),
            }
        })?,
        parse_digest(&entry.identity.cache_key)?,
        ArtifactSchema::new(
            entry.identity.result_schema.id.clone(),
            entry.identity.result_schema.version,
        )?,
        runtime,
    )?;
    let content = entry.output.content.to_artifact_ref()?;
    let draft = output_draft(&request, content.clone())?;
    let claimed_id = crate::ArtifactId::from_str(&entry.output.artifact_id)
        .map_err(|_| DurableProjectError::InvalidArtifactIdentity)?;
    let claimed_semantic = parse_digest(&entry.output.semantic_digest)?;
    if draft.id() != claimed_id || draft.semantic_digest() != claimed_semantic {
        return Err(DurableProjectError::InvalidArtifactIdentity);
    }
    let record = draft.located_record(ArtifactLocator::managed(
        StoreId::new(STORE_ID).map_err(DurableProjectError::ArtifactRecord)?,
        draft.id(),
    ));
    Ok((record, content))
}

fn read_verified_object(
    store: &crate::LocalArtifactStore,
    record: &ArtifactRecord,
    maximum: usize,
) -> Result<Vec<u8>, DurableProjectError> {
    let declared = usize::try_from(record.content().byte_len()).map_err(|_| {
        DurableProjectError::ObjectTooLarge {
            observed: usize::MAX,
            maximum,
        }
    })?;
    if declared > maximum {
        return Err(DurableProjectError::ObjectTooLarge {
            observed: declared,
            maximum,
        });
    }
    store
        .with_verified_reader(record, |reader| {
            let capacity = declared.min(64 * 1024);
            let mut bytes = Vec::with_capacity(capacity);
            reader
                .take(u64::try_from(maximum).unwrap_or(u64::MAX).saturating_add(1))
                .read_to_end(&mut bytes)?;
            if bytes.len() > maximum {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "durable object exceeds materialization limit",
                ));
            }
            Ok(bytes)
        })
        .map_err(|error| match error {
            VerifiedReaderError::Store(source) => DurableProjectError::ArtifactStore(source),
            VerifiedReaderError::Callback(source) => DurableProjectError::Io {
                operation: "read verified durable object",
                path: STORE_DIRECTORY.to_owned(),
                source,
            },
        })
}
