use std::{
    collections::BTreeMap,
    error::Error,
    fmt, fs,
    sync::atomic::{AtomicBool, Ordering},
};

use marklab_project::{
    ArtifactDraft, ArtifactKey, ArtifactLocator, ArtifactRecord, ArtifactRef, ArtifactSchema,
    ArtifactStoreError, LocalArtifactStore, PublicationDisposition, StoreId, VerifiedReaderError,
};
use tempfile::TempDir;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CallbackError;

impl fmt::Display for CallbackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("callback rejected artifact")
    }
}

impl Error for CallbackError {}

fn record(bytes: &[u8], suffix: &str) -> ArtifactRecord {
    ArtifactRecord::new(
        ArtifactRef::from_bytes("application/vnd.marklab.verified-io-test", bytes)
            .expect("content"),
        ArtifactSchema::new("marklab.test.verified_io", 1).expect("schema"),
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

fn draft(bytes: &[u8]) -> ArtifactDraft {
    ArtifactDraft::new(
        ArtifactRef::from_bytes("application/vnd.marklab.fresh-test", bytes).expect("content"),
        ArtifactSchema::new("marklab.test.fresh", 1).expect("schema"),
        None,
        Vec::new(),
        BTreeMap::new(),
    )
    .expect("draft")
}

fn open_store(root: &TempDir) -> LocalArtifactStore {
    LocalArtifactStore::open(root.path(), StoreId::new("local").expect("store ID")).expect("store")
}

fn managed_path(root: &TempDir, record: &ArtifactRecord) -> std::path::PathBuf {
    let id = record.id().to_string();
    root.path().join("objects/sha256").join(&id[..2]).join(id)
}

fn assert_send<T: Send + ?Sized>(_: &T) {}

#[test]
fn verified_reader_uses_the_published_descriptor_and_returns_callback_value() {
    let root = TempDir::new().expect("root");
    let store = open_store(&root);
    let bytes = b"verified reader bytes";
    let publication = store
        .publish(&record(bytes, "read"), |writer| writer.write_all(bytes))
        .expect("publish");

    let observed = store
        .with_verified_reader(publication.record(), |reader| {
            let mut observed = Vec::new();
            reader.read_to_end(&mut observed)?;
            Ok::<_, std::io::Error>(observed)
        })
        .expect("verified read");

    assert_eq!(observed, bytes);
}

#[test]
fn verified_reader_keeps_callback_errors_distinct() {
    let root = TempDir::new().expect("root");
    let store = open_store(&root);
    let bytes = b"callback error bytes";
    let publication = store
        .publish(&record(bytes, "callback-error"), |writer| {
            writer.write_all(bytes)
        })
        .expect("publish");

    let error = store
        .with_verified_reader(publication.record(), |_reader| Err::<(), _>(CallbackError))
        .expect_err("callback should fail");

    assert!(matches!(
        error,
        VerifiedReaderError::Callback(CallbackError)
    ));
}

#[test]
fn pre_read_integrity_failure_never_invokes_callback() {
    let root = TempDir::new().expect("root");
    let store = open_store(&root);
    let bytes = b"pre-read identity";
    let publication = store
        .publish(&record(bytes, "pre-read-integrity"), |writer| {
            writer.write_all(bytes)
        })
        .expect("publish");
    fs::write(managed_path(&root, publication.record()), b"wrong").expect("mutate artifact");
    let invoked = AtomicBool::new(false);

    let error = store
        .with_verified_reader(publication.record(), |_reader| {
            invoked.store(true, Ordering::SeqCst);
            Ok::<_, CallbackError>(())
        })
        .expect_err("pre-read verification should fail");

    assert!(matches!(
        error,
        VerifiedReaderError::Store(ArtifactStoreError::ContentIntegrity { .. })
    ));
    assert!(!invoked.load(Ordering::SeqCst));
}

#[test]
fn post_read_integrity_failure_overrides_callback_failure() {
    let root = TempDir::new().expect("root");
    let store = open_store(&root);
    let bytes = b"original immutable bytes";
    let publication = store
        .publish(&record(bytes, "mutated-error"), |writer| {
            writer.write_all(bytes)
        })
        .expect("publish");
    let path = managed_path(&root, publication.record());

    let error = store
        .with_verified_reader(publication.record(), |_reader| {
            fs::write(&path, b"mutated").expect("mutate opened artifact");
            Err::<(), _>(CallbackError)
        })
        .expect_err("post-read verification should fail");

    assert!(matches!(
        error,
        VerifiedReaderError::Store(ArtifactStoreError::ContentIntegrity { .. })
    ));
}

#[test]
fn post_read_integrity_failure_overrides_callback_success() {
    let root = TempDir::new().expect("root");
    let store = open_store(&root);
    let bytes = b"original successful bytes";
    let publication = store
        .publish(&record(bytes, "mutated-success"), |writer| {
            writer.write_all(bytes)
        })
        .expect("publish");
    let path = managed_path(&root, publication.record());

    let error = store
        .with_verified_reader(publication.record(), |_reader| {
            fs::write(&path, b"changed").expect("mutate opened artifact");
            Ok::<_, CallbackError>(())
        })
        .expect_err("post-read verification should fail");

    assert!(matches!(
        error,
        VerifiedReaderError::Store(ArtifactStoreError::ContentIntegrity { .. })
    ));
}

#[test]
fn publish_send_is_send_capable_idempotent_and_keeps_publish_compatible() {
    let root = TempDir::new().expect("root");
    let store = open_store(&root);
    let send_bytes = b"send writer bytes";
    let send_record = record(send_bytes, "send-writer");

    let first = store
        .publish_send(&send_record, |writer| {
            assert_send(writer);
            writer.write_all(send_bytes)
        })
        .expect("send publish");
    assert_eq!(first.disposition(), PublicationDisposition::Created);

    let invoked = AtomicBool::new(false);
    let second = store
        .publish_send(&send_record, |_writer| {
            invoked.store(true, Ordering::SeqCst);
            Ok(())
        })
        .expect("idempotent send publish");
    assert_eq!(second.disposition(), PublicationDisposition::AlreadyPresent);
    assert!(!invoked.load(Ordering::SeqCst));

    let legacy_bytes = b"legacy writer remains supported";
    let legacy = store
        .publish(&record(legacy_bytes, "legacy-writer"), |writer| {
            writer.write_all(legacy_bytes)
        })
        .expect("legacy publish");
    assert_eq!(legacy.disposition(), PublicationDisposition::Created);
}

#[test]
fn publish_send_callback_failure_removes_staging_and_never_publishes() {
    let root = TempDir::new().expect("root");
    let store = open_store(&root);
    let bytes = b"send callback must roll back";
    let record = record(bytes, "send-callback-failure");

    let error = store
        .publish_send(&record, |writer| {
            writer.write_all(&bytes[..5])?;
            Err(std::io::Error::other("injected send callback failure"))
        })
        .expect_err("send callback should fail");

    assert!(matches!(error, ArtifactStoreError::WriteCallback { .. }));
    assert!(!managed_path(&root, &record).exists());
    assert_eq!(
        fs::read_dir(root.path().join(".marklab-staging"))
            .expect("staging directory")
            .count(),
        0
    );
}

#[test]
fn publish_new_send_returns_only_a_truthful_managed_record_and_is_idempotent() {
    let root = TempDir::new().expect("root");
    let store = open_store(&root);
    let bytes = b"fresh canonical bytes";
    let draft = draft(bytes);

    let first = store
        .publish_new_send(&draft, |writer| {
            assert_send(writer);
            writer.write_all(bytes)
        })
        .expect("publish fresh artifact");
    assert_eq!(first.disposition(), PublicationDisposition::Created);
    assert_eq!(first.record().id(), draft.id());
    assert_eq!(first.record().locations().len(), 1);
    assert_eq!(first.record().locations()[0].store_id(), store.store_id());
    store.verify(first.record()).expect("verify fresh record");

    let invoked = AtomicBool::new(false);
    let second = store
        .publish_new_send(&draft, |_writer| {
            invoked.store(true, Ordering::SeqCst);
            Ok(())
        })
        .expect("reuse fresh artifact");
    assert_eq!(second.disposition(), PublicationDisposition::AlreadyPresent);
    assert!(!invoked.load(Ordering::SeqCst));
    assert_eq!(second.record(), first.record());
}

#[test]
fn publish_new_send_integrity_failure_returns_no_record_and_removes_staging() {
    let root = TempDir::new().expect("root");
    let store = open_store(&root);
    let bytes = b"fresh expected bytes";
    let draft = draft(bytes);

    let error = store
        .publish_new_send(&draft, |writer| writer.write_all(b"fresh altered!bytes"))
        .expect_err("integrity mismatch");
    assert!(matches!(error, ArtifactStoreError::ContentIntegrity { .. }));
    let id = draft.id().to_string();
    assert!(!root
        .path()
        .join("objects/sha256")
        .join(&id[..2])
        .join(id)
        .exists());
    assert_eq!(
        fs::read_dir(root.path().join(".marklab-staging"))
            .expect("staging directory")
            .count(),
        0
    );
}
