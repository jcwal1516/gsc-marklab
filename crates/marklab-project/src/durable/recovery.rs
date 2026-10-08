use cap_std::fs::Dir;

use crate::LocalArtifactStore;

use super::{
    api::{DurableProjectLimits, DurableRecoveryAction},
    execution::record_from_entry,
    layout::{path_exists, read_bounded, remove_pending},
    state::{
        append_execution, execution_digest, head_for_execution, ledger_byte_len,
        validate_execution, write_head,
    },
    wire::{
        decode_canonical_pretty, ExecutionWire, PendingWire, ProjectHeadWire, FORMAT_VERSION,
        PENDING_FORMAT, PENDING_PATH,
    },
    DurableProjectError,
};

pub(super) fn recover_pending(
    root: &Dir,
    store: &LocalArtifactStore,
    limits: DurableProjectLimits,
    head: &mut ProjectHeadWire,
    ledger: &mut Vec<ExecutionWire>,
) -> Result<DurableRecoveryAction, DurableProjectError> {
    if !path_exists(root, PENDING_PATH)? {
        return Ok(DurableRecoveryAction::None);
    }
    let bytes = read_bounded(root, PENDING_PATH, limits.maximum_control_bytes)?;
    let pending: PendingWire = decode_canonical_pretty(PENDING_PATH, &bytes)?;
    if pending.format != PENDING_FORMAT || pending.version != FORMAT_VERSION {
        return Err(DurableProjectError::UnsupportedVersion {
            path: PENDING_PATH.to_owned(),
            observed: pending.version,
        });
    }
    validate_execution(&pending.execution, limits)?;
    let (record, _) = record_from_entry(&pending.execution)?;
    store.verify(&record)?;

    let prior_count = ledger.len();
    let expected_next = u64::try_from(prior_count)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or(DurableProjectError::LedgerSequenceOverflow)?;
    let expected_previous = ledger
        .last()
        .map(execution_digest)
        .transpose()?
        .map(|digest| digest.to_string());
    if pending.execution.sequence == expected_next
        && pending.execution.previous_record_digest == expected_previous
    {
        let prospective_records = prior_count
            .checked_add(1)
            .ok_or(DurableProjectError::LedgerSequenceOverflow)?;
        if prospective_records > limits.maximum_ledger_records {
            return Err(DurableProjectError::TooManyLedgerRecords {
                observed: prospective_records,
                maximum: limits.maximum_ledger_records,
            });
        }
        append_execution(root, &pending.execution, limits)?;
        ledger.push(pending.execution.clone());
        *head = head_for_execution(
            head.project_id.clone(),
            &pending.execution,
            ledger.len(),
            ledger_byte_len(ledger)?,
        )?;
        write_head(root, head, limits)?;
        remove_pending(root)?;
        return Ok(DurableRecoveryAction::CompletedPendingExecution);
    }

    if ledger.last() == Some(&pending.execution) {
        let target = head_for_execution(
            head.project_id.clone(),
            &pending.execution,
            ledger.len(),
            ledger_byte_len(ledger)?,
        )?;
        let action = if *head == target {
            DurableRecoveryAction::ClearedCommittedIntent
        } else {
            *head = target;
            write_head(root, head, limits)?;
            DurableRecoveryAction::AdvancedProjectHead
        };
        remove_pending(root)?;
        return Ok(action);
    }
    Err(DurableProjectError::ConflictingPendingExecution)
}
