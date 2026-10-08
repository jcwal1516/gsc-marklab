use std::{
    collections::BTreeMap,
    fs, io,
    sync::{Arc, Barrier},
    thread,
};

use marklab_project::{
    ArtifactKey, ArtifactLocator, ArtifactRecord, ArtifactRecordError, ArtifactRef, ArtifactSchema,
    ArtifactStoreError, LocalArtifactStore, PublicationDisposition, StoreId,
};
use tempfile::TempDir;

fn source_record(bytes: &[u8], suffix: &str) -> ArtifactRecord {
    ArtifactRecord::new(
        ArtifactRef::from_bytes("application/vnd.marklab.object", bytes).expect("content"),
        ArtifactSchema::new("marklab.test.object", 1).expect("schema"),
        None,
        Vec::new(),
        BTreeMap::new(),
        vec![ArtifactLocator::new(
            StoreId::new("source").expect("source store"),
            ArtifactKey::new(format!("incoming/{suffix}")).expect("source key"),
            None,
        )
        .expect("source locator")],
    )
    .expect("record")
}

fn open_store(root: &TempDir, id: &str) -> LocalArtifactStore {
    LocalArtifactStore::open(root.path(), StoreId::new(id).expect("store ID")).expect("store")
}

fn managed_path(root: &TempDir, record: &ArtifactRecord) -> std::path::PathBuf {
    let id = record.id().to_string();
    root.path()
        .join("objects")
        .join("sha256")
        .join(&id[..2])
        .join(id)
}

#[test]
fn streaming_publication_is_verified_idempotent_and_store_relative() {
    let root = TempDir::new().expect("temp root");
    let store = open_store(&root, "local");
    let expected = b"immutable artifact bytes";
    let record = source_record(expected, "artifact");

    let first = store
        .publish(&record, |writer| writer.write_all(expected))
        .expect("first publication");
    assert_eq!(first.disposition(), PublicationDisposition::Created);
    assert_eq!(first.record().id(), record.id());
    store.verify(first.record()).expect("verify first");

    let second = store
        .publish(&record, |_| {
            panic!("idempotent publication must not rewrite")
        })
        .expect("idempotent publication");
    assert_eq!(second.disposition(), PublicationDisposition::AlreadyPresent);
    assert_eq!(second.record(), first.record());
    assert_eq!(
        fs::read(managed_path(&root, &record)).expect("stored bytes"),
        expected
    );
}

#[test]
fn digest_callback_and_immutable_conflict_failures_expose_no_partial_final_content() {
    let root = TempDir::new().expect("temp root");
    let store = open_store(&root, "local");
    let expected = b"complete content";
    let record = source_record(expected, "failure");
    let final_path = managed_path(&root, &record);

    assert!(matches!(
        store.publish(&record, |writer| writer.write_all(b"wrong")),
        Err(ArtifactStoreError::ContentIntegrity { .. })
    ));
    assert!(!final_path.exists());

    assert!(matches!(
        store.publish(&record, |writer| writer
            .write_all(b"content exceeding expected length")),
        Err(ArtifactStoreError::WriteCallback { .. })
    ));
    assert!(!final_path.exists());

    assert!(matches!(
        store.publish(&record, |writer| {
            writer.write_all(b"part")?;
            Err(io::Error::other("injected callback failure"))
        }),
        Err(ArtifactStoreError::WriteCallback { .. })
    ));
    assert!(!final_path.exists());

    fs::create_dir_all(final_path.parent().expect("final parent")).expect("parent");
    fs::write(&final_path, b"hostile existing content").expect("conflict fixture");
    assert!(matches!(
        store.publish(&record, |_| panic!("conflict must be checked first")),
        Err(ArtifactStoreError::ImmutableConflict { .. })
    ));
    assert_eq!(
        fs::read(&final_path).expect("unchanged conflict"),
        b"hostile existing content"
    );
}

#[test]
fn missing_truncated_appended_and_wrong_store_objects_are_integrity_errors() {
    let root = TempDir::new().expect("temp root");
    let store = open_store(&root, "local");
    let expected = b"verify me";
    let record = source_record(expected, "verification");
    let published = store
        .publish(&record, |writer| writer.write_all(expected))
        .expect("publication");
    let final_path = managed_path(&root, &record);

    fs::write(&final_path, b"verify no").expect("same-length mutation");
    assert!(matches!(
        store.verify(published.record()),
        Err(ArtifactStoreError::ContentIntegrity { .. })
    ));
    fs::write(&final_path, b"short").expect("truncate/mutate");
    assert!(matches!(
        store.verify(published.record()),
        Err(ArtifactStoreError::ContentIntegrity { .. })
    ));
    fs::write(&final_path, b"verify me and appended").expect("append");
    assert!(matches!(
        store.verify(published.record()),
        Err(ArtifactStoreError::ContentIntegrity { .. })
    ));
    fs::remove_file(&final_path).expect("remove fixture");
    assert!(matches!(
        store.verify(published.record()),
        Err(ArtifactStoreError::MissingObject { artifact, .. }) if artifact == record.id()
    ));

    let other_root = TempDir::new().expect("other root");
    let other = open_store(&other_root, "other");
    assert!(matches!(
        other.verify(published.record()),
        Err(ArtifactStoreError::LocatorNotFound { .. })
    ));
}

#[test]
fn same_store_locator_conflict_is_rejected_before_writing_or_invoking_callback() {
    let root = TempDir::new().expect("temp root");
    let store = open_store(&root, "local");
    let bytes = b"must not be written";
    let conflicting = ArtifactRecord::new(
        ArtifactRef::from_bytes("application/vnd.marklab.object", bytes).expect("content"),
        ArtifactSchema::new("marklab.test.object", 1).expect("schema"),
        None,
        Vec::new(),
        BTreeMap::new(),
        vec![ArtifactLocator::new(
            StoreId::new("local").expect("store"),
            ArtifactKey::new("external/already-bound").expect("key"),
            None,
        )
        .expect("locator")],
    )
    .expect("record");
    assert!(matches!(
        store.publish(&conflicting, |_| panic!("callback must not run")),
        Err(ArtifactStoreError::InvalidRecord(
            ArtifactRecordError::ConflictingStoreLocator { .. }
        ))
    ));
    assert!(!managed_path(&root, &conflicting).exists());
}

#[test]
fn concurrent_publication_never_overwrites_and_both_callers_verify_same_content() {
    let root = TempDir::new().expect("temp root");
    let store = Arc::new(open_store(&root, "local"));
    let bytes = Arc::<[u8]>::from(b"concurrent immutable bytes".as_slice());
    let record = Arc::new(source_record(&bytes, "concurrent"));
    let barrier = Arc::new(Barrier::new(2));

    let handles = (0..2)
        .map(|_| {
            let store = Arc::clone(&store);
            let record = Arc::clone(&record);
            let bytes = Arc::clone(&bytes);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                store
                    .publish(&record, |writer| writer.write_all(&bytes))
                    .expect("concurrent publication")
            })
        })
        .collect::<Vec<_>>();
    let mut dispositions = handles
        .into_iter()
        .map(|handle| handle.join().expect("publication thread").disposition())
        .collect::<Vec<_>>();
    dispositions.sort_unstable();
    assert_eq!(
        dispositions,
        [
            PublicationDisposition::Created,
            PublicationDisposition::AlreadyPresent
        ]
    );
    assert_eq!(
        fs::read(managed_path(&root, &record)).expect("final bytes"),
        bytes.as_ref()
    );
}

#[test]
fn recovery_quarantines_only_recognized_regular_staging_files() {
    let root = TempDir::new().expect("temp root");
    let store = open_store(&root, "local");
    let record = source_record(b"abandoned", "abandoned");
    let staging = root.path().join(".marklab-staging");
    let recognized = format!("marklab-1-1-{}.part", record.id());
    fs::write(staging.join(&recognized), b"abandoned").expect("recognized fixture");
    fs::write(staging.join("operator-note"), b"do not delete").expect("unknown fixture");

    let report = store.recover_staging().expect("recovery");
    assert_eq!(report.quarantined(), std::slice::from_ref(&recognized));
    assert_eq!(report.issues().len(), 1);
    assert!(!staging.join(&recognized).exists());
    assert!(root
        .path()
        .join(".marklab-quarantine")
        .join(recognized)
        .is_file());
    assert!(staging.join("operator-note").is_file());
}

#[cfg(unix)]
#[test]
fn symlink_root_intermediate_and_leaf_are_rejected_without_following() {
    use std::os::unix::fs::symlink;

    let real = TempDir::new().expect("real root");
    let link_parent = TempDir::new().expect("link parent");
    let root_link = link_parent.path().join("root-link");
    symlink(real.path(), &root_link).expect("root symlink");
    assert!(matches!(
        LocalArtifactStore::open(&root_link, StoreId::new("local").expect("store")),
        Err(ArtifactStoreError::SymlinkBoundary { .. })
    ));

    let store = open_store(&real, "local");
    fs::create_dir_all(real.path().join("external/real")).expect("external directory");
    fs::write(real.path().join("external/real/object"), b"linked").expect("target");
    symlink("real", real.path().join("external/intermediate")).expect("intermediate symlink");
    let intermediate = ArtifactRecord::new(
        ArtifactRef::from_bytes("application/vnd.marklab.object", b"linked").expect("content"),
        ArtifactSchema::new("marklab.test.object", 1).expect("schema"),
        None,
        Vec::new(),
        BTreeMap::new(),
        vec![ArtifactLocator::new(
            StoreId::new("local").expect("store"),
            ArtifactKey::new("external/intermediate/object").expect("key"),
            None,
        )
        .expect("locator")],
    )
    .expect("intermediate record");
    assert!(matches!(
        store.verify(&intermediate),
        Err(ArtifactStoreError::SymlinkBoundary { .. })
    ));

    symlink("real/object", real.path().join("external/leaf")).expect("leaf symlink");
    let leaf = ArtifactRecord::new(
        ArtifactRef::from_bytes("application/vnd.marklab.object", b"linked").expect("content"),
        ArtifactSchema::new("marklab.test.object", 1).expect("schema"),
        None,
        Vec::new(),
        BTreeMap::new(),
        vec![ArtifactLocator::new(
            StoreId::new("local").expect("store"),
            ArtifactKey::new("external/leaf").expect("key"),
            None,
        )
        .expect("locator")],
    )
    .expect("leaf record");
    assert!(matches!(
        store.verify(&leaf),
        Err(ArtifactStoreError::SymlinkBoundary { .. })
    ));
}

#[test]
fn directories_are_not_artifact_files() {
    let root = TempDir::new().expect("temp root");
    let store = open_store(&root, "local");
    fs::create_dir_all(root.path().join("external/directory")).expect("directory fixture");
    let record = ArtifactRecord::new(
        ArtifactRef::from_bytes("application/vnd.marklab.object", b"").expect("content"),
        ArtifactSchema::new("marklab.test.object", 1).expect("schema"),
        None,
        Vec::new(),
        BTreeMap::new(),
        vec![ArtifactLocator::new(
            StoreId::new("local").expect("store"),
            ArtifactKey::new("external/directory").expect("key"),
            None,
        )
        .expect("locator")],
    )
    .expect("record");
    assert!(matches!(
        store.verify(&record),
        Err(ArtifactStoreError::UnsupportedFileType { .. })
    ));
}

#[cfg(unix)]
#[test]
fn fifos_and_sockets_are_rejected_and_never_consumed_as_artifacts() {
    use std::{os::unix::net::UnixListener, process::Command};

    let root = TempDir::new().expect("temp root");
    let store = open_store(&root, "local");
    fs::create_dir_all(root.path().join("external")).expect("external directory");
    let fifo_path = root.path().join("external/fifo");
    let status = Command::new("mkfifo")
        .arg(&fifo_path)
        .status()
        .expect("run mkfifo");
    assert!(status.success());
    let _listener = UnixListener::bind(root.path().join("external/socket")).expect("socket");

    for key in ["external/fifo", "external/socket"] {
        let record = ArtifactRecord::new(
            ArtifactRef::from_bytes("application/vnd.marklab.object", b"").expect("content"),
            ArtifactSchema::new("marklab.test.object", 1).expect("schema"),
            None,
            Vec::new(),
            BTreeMap::new(),
            vec![ArtifactLocator::new(
                StoreId::new("local").expect("store"),
                ArtifactKey::new(key).expect("key"),
                None,
            )
            .expect("locator")],
        )
        .expect("record");
        assert!(matches!(
            store.verify(&record),
            Err(ArtifactStoreError::UnsupportedFileType { .. })
        ));
    }
}

#[cfg(unix)]
#[test]
fn recovery_reports_recognized_symlink_and_fifo_without_following_or_deleting() {
    use std::{os::unix::fs::symlink, process::Command};

    let root = TempDir::new().expect("temp root");
    let store = open_store(&root, "local");
    let record = source_record(b"recovery types", "recovery-types");
    let staging = root.path().join(".marklab-staging");
    fs::write(root.path().join("operator-target"), b"retain").expect("target");
    let symlink_name = format!("marklab-2-1-{}.part", record.id());
    symlink("../operator-target", staging.join(&symlink_name)).expect("staging symlink");
    let fifo_name = format!("marklab-2-2-{}.part", record.id());
    let status = Command::new("mkfifo")
        .arg(staging.join(&fifo_name))
        .status()
        .expect("run mkfifo");
    assert!(status.success());

    let report = store.recover_staging().expect("recovery report");
    assert!(report.quarantined().is_empty());
    assert_eq!(report.issues().len(), 2);
    assert!(staging.join(symlink_name).symlink_metadata().is_ok());
    assert!(staging.join(fifo_name).symlink_metadata().is_ok());
    assert_eq!(
        fs::read(root.path().join("operator-target")).expect("target"),
        b"retain"
    );
}
