#![cfg(feature = "csv")]

use std::{
    env, fs,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use marklab::{SourceBundleBudgets, SourceBundleReconciler};

const STAGE_MARKER: &[u8] = b"marklab-c04-authorized-reconciliation-v1\n";

#[test]
#[ignore = "requires the explicitly staged authorized CellViT corpus"]
fn reconciles_all_authorized_bundles() {
    let root = PathBuf::from(
        env::var_os("MARKLAB_AUTHORIZED_CELLVIT_ROOT")
            .expect("MARKLAB_AUTHORIZED_CELLVIT_ROOT is required"),
    );
    assert_eq!(
        fs::read(root.join(".marklab-c04-stage-owner")).expect("read stage ownership marker"),
        STAGE_MARKER
    );
    let mut bundle_directories = Vec::new();
    collect_bundle_directories(&root, &mut bundle_directories);
    bundle_directories.sort_unstable();
    assert_eq!(bundle_directories.len(), 32);

    let budgets = SourceBundleBudgets::new(
        64 * 1024 * 1024,
        64 * 1024 * 1024,
        64 * 1024,
        16 * 1024 * 1024,
        64 * 1024 * 1024,
    );
    let mut reconciler = SourceBundleReconciler::new(budgets, 32);
    for directory in bundle_directories {
        let mut npy =
            BufReader::new(File::open(directory.join("embeddings.npy")).expect("open NPY"));
        let mut csv = BufReader::new(File::open(directory.join("cells.csv")).expect("open CSV"));
        let mut manifest = BufReader::new(
            File::open(directory.join("bundle_manifest.json")).expect("open manifest"),
        );
        reconciler
            .push_readers(&mut npy, &mut csv, &mut manifest)
            .expect("reconcile authorized bundle");
    }
    let report = reconciler.finish().expect("finish reconciliation");
    assert_eq!(report.bundle_count(), 32);
    assert_eq!(report.row_count(), 60_191);
    assert_eq!(report.dimension(), 1_280);
    assert_eq!(report.present_count(), 60_191);
    assert_eq!(report.qc_pass_count(), 60_191);
    assert_eq!(
        report.aggregate_content_digest().to_string(),
        "75335c9aca2ab167a823cfbcae6d6783482b1cffea71903ac03f61b8775113b6"
    );
    assert_eq!(
        report.reconciliation_digest().to_string(),
        "e5aeb0a426a7f9f2b38d538c73dd867818a589e3ccbe1b1b955b4273b65996a8"
    );
    assert_eq!(report.missing_promotion_fields().len(), 4);
}

fn collect_bundle_directories(directory: &Path, bundles: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("read staged directory") {
        let entry = entry.expect("read staged entry");
        let file_type = entry.file_type().expect("inspect staged entry");
        if file_type.is_dir() {
            collect_bundle_directories(&entry.path(), bundles);
        } else if file_type.is_file() && entry.file_name() == "bundle_manifest.json" {
            bundles.push(entry.path().parent().expect("manifest parent").to_owned());
        }
    }
}
