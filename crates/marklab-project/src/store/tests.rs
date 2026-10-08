use std::{collections::BTreeMap, fs, path::PathBuf};

use tempfile::TempDir;

use super::*;
use crate::{ArtifactKey, ArtifactLocator, ArtifactRef, ArtifactSchema};

fn record(bytes: &[u8]) -> ArtifactRecord {
    ArtifactRecord::new(
        ArtifactRef::from_bytes("application/vnd.marklab.fault-test", bytes).expect("content"),
        ArtifactSchema::new("marklab.test.fault", 1).expect("schema"),
        None,
        Vec::new(),
        BTreeMap::new(),
        vec![ArtifactLocator::new(
            StoreId::new("source").expect("source store"),
            ArtifactKey::new("incoming/fault-test").expect("source key"),
            None,
        )
        .expect("source locator")],
    )
    .expect("record")
}

fn final_path(root: &TempDir, record: &ArtifactRecord) -> PathBuf {
    let id = record.id().to_string();
    root.path().join("objects/sha256").join(&id[..2]).join(id)
}

#[test]
fn failure_after_file_sync_removes_staging_without_publishing() {
    let root = TempDir::new().expect("root");
    let store = LocalArtifactStore::open(root.path(), StoreId::new("local").expect("store ID"))
        .expect("store");
    let bytes = b"prepublication fault";
    let record = record(bytes);
    assert!(matches!(
        store.publish_with_fault(
            &record,
            |writer| writer.write_all(bytes),
            PublishFault::AfterFileSync,
        ),
        Err(ArtifactStoreError::Io {
            operation: "injected after file sync",
            ..
        })
    ));
    assert!(!final_path(&root, &record).exists());
    assert_eq!(
        fs::read_dir(root.path().join(STAGING_DIRECTORY))
            .expect("staging")
            .count(),
        0
    );
}

#[test]
fn create_new_collision_never_deletes_a_staging_file_owned_by_another_writer() {
    let root = TempDir::new().expect("root");
    let store = LocalArtifactStore::open(root.path(), StoreId::new("local").expect("store ID"))
        .expect("store");
    let bytes = b"collision content";
    let record = record(bytes);
    assert!(matches!(
        store.publish_with_fault(
            &record,
            |writer| writer.write_all(bytes),
            PublishFault::StagingCreateCollision,
        ),
        Err(ArtifactStoreError::Io {
            operation: "create staging file",
            ..
        })
    ));
    assert!(!final_path(&root, &record).exists());
    let staging_entries = fs::read_dir(root.path().join(STAGING_DIRECTORY))
        .expect("staging")
        .collect::<Result<Vec<_>, _>>()
        .expect("staging entries");
    assert_eq!(staging_entries.len(), 1);
    assert_eq!(
        fs::read(staging_entries[0].path()).expect("colliding writer bytes"),
        b"owned by another writer"
    );
}

#[test]
fn recovery_coordination_lock_excludes_active_cross_instance_publication() {
    use std::{fs::TryLockError, sync::mpsc, thread};

    let root = TempDir::new().expect("root");
    let publisher = LocalArtifactStore::open(root.path(), StoreId::new("local").expect("store ID"))
        .expect("publisher store");
    let recovery = LocalArtifactStore::open(root.path(), StoreId::new("local").expect("store ID"))
        .expect("recovery store");
    let bytes = b"coordinated publication".to_vec();
    let record = record(&bytes);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let publisher_thread = thread::spawn(move || {
        publisher.publish(&record, |writer| {
            entered_tx.send(()).expect("entered signal");
            release_rx.recv().expect("release signal");
            writer.write_all(&bytes)
        })
    });
    entered_rx.recv().expect("publisher entered callback");

    let coordination = recovery
        .open_coordination_file()
        .expect("coordination file");
    assert!(matches!(
        coordination.try_lock(),
        Err(TryLockError::WouldBlock)
    ));
    release_tx.send(()).expect("release publisher");
    publisher_thread
        .join()
        .expect("publisher thread")
        .expect("publication");
    coordination
        .try_lock()
        .expect("exclusive lock after publication");
}

#[test]
fn recovery_reports_quarantine_target_across_every_partial_move_phase() {
    for (fault, staging_remains, completed_move) in [
        (RecoveryFault::AfterQuarantineLink, true, false),
        (RecoveryFault::AfterQuarantineSync, true, false),
        (RecoveryFault::AfterStagingRemove, false, true),
    ] {
        let root = TempDir::new().expect("root");
        let store = LocalArtifactStore::open(root.path(), StoreId::new("local").expect("store ID"))
            .expect("store");
        let record = record(b"partial recovery");
        let name = format!("marklab-3-1-{}.part", record.id());
        let staging_path = root.path().join(STAGING_DIRECTORY).join(&name);
        fs::write(&staging_path, b"partial recovery").expect("staging fixture");

        let report = store
            .recover_staging_with_fault(fault)
            .expect("recovery report");
        assert_eq!(report.issues().len(), 1);
        let target = report.issues()[0]
            .quarantine_target()
            .expect("reported quarantine target");
        assert!(root
            .path()
            .join(QUARANTINE_DIRECTORY)
            .join(target)
            .is_file());
        assert_eq!(staging_path.exists(), staging_remains);
        assert_eq!(!report.quarantined().is_empty(), completed_move);
    }
}

#[test]
fn failure_after_publication_leaves_durable_final_and_recoverable_staging() {
    let root = TempDir::new().expect("root");
    let store = LocalArtifactStore::open(root.path(), StoreId::new("local").expect("store ID"))
        .expect("store");
    let bytes = b"postpublication fault";
    let record = record(bytes);
    assert!(matches!(
        store.publish_with_fault(
            &record,
            |writer| writer.write_all(bytes),
            PublishFault::AfterPublication,
        ),
        Err(ArtifactStoreError::PublishedButCleanupFailed {
            operation: "injected after publication",
            ..
        })
    ));
    assert_eq!(fs::read(final_path(&root, &record)).expect("final"), bytes);
    assert_eq!(
        fs::read_dir(root.path().join(STAGING_DIRECTORY))
            .expect("staging")
            .count(),
        1
    );
    drop(store);

    let reopened = LocalArtifactStore::open(root.path(), StoreId::new("local").expect("store ID"))
        .expect("reopened store");
    let report = reopened.recover_staging().expect("recovery");
    assert_eq!(report.quarantined().len(), 1);
    assert!(report.issues().is_empty());
    let mut located = record.clone();
    located
        .merge_locations(&[ArtifactLocator::managed(
            StoreId::new("local").expect("store ID"),
            record.id(),
        )])
        .expect("managed locator");
    reopened.verify(&located).expect("durable final verifies");
}
