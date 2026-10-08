#![cfg(feature = "csv")]

use std::io::{Cursor, Read, Seek, SeekFrom};

use marklab::{
    ManifestFailure, MissingPromotionField, ReconciliationFailure, SourceBundleBudgets,
    SourceBundleError, SourceBundleReconciler,
};

const HEADER: &str = "cell_id,case_id,specimen_id,timepoint,fragment_id,roi_id,native_row,embedding_row,x_px,y_px,x_um,y_um,cell_type_id,cell_type_label,type_probability,nucleus_area_um2,nucleus_perimeter_um,eccentricity,solidity,circularity,qc_pass,block_500_id,split\r\n";

fn budgets(npy: usize, csv: usize, manifest: usize) -> SourceBundleBudgets {
    budgets_with_retained(npy, csv, manifest, 4 * 1024 * 1024)
}

fn budgets_with_retained(
    npy: usize,
    csv: usize,
    manifest: usize,
    retained: usize,
) -> SourceBundleBudgets {
    SourceBundleBudgets::new(
        u64::try_from(npy).expect("NPY bytes"),
        u64::try_from(csv).expect("CSV bytes"),
        u64::try_from(manifest).expect("manifest bytes"),
        retained,
        4 * 1024 * 1024,
    )
}

fn csv(prefix: &str, rows: usize) -> Vec<u8> {
    let mut bytes = HEADER.as_bytes().to_vec();
    for row in 0..rows {
        bytes.extend_from_slice(
            format!(
                "{prefix}-{row},case,synthetic,time,fragment,roi,{row},{row},1,2,3,4,1,label,0.5,10,5,0.2,0.8,0.7,True,block,train\r\n"
            )
            .as_bytes(),
        );
    }
    bytes
}

fn npy(rows: usize, seed: f32) -> Vec<u8> {
    let dictionary =
        format!("{{'descr': '<f4', 'fortran_order': False, 'shape': ({rows}, 1280), }}");
    let prefix_length = 10;
    let padding = (16 - ((prefix_length + dictionary.len() + 1) % 16)) % 16;
    let mut header = dictionary.into_bytes();
    header.resize(header.len() + padding, b' ');
    header.push(b'\n');
    let mut bytes = b"\x93NUMPY\x01\x00".to_vec();
    bytes.extend_from_slice(
        &u16::try_from(header.len())
            .expect("small header")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&header);
    for row in 0..rows {
        for column in 0..1_280 {
            let value = seed + row as f32 + column as f32 / 1_280.0;
            bytes.extend_from_slice(&value.to_bits().to_le_bytes());
        }
    }
    bytes
}

fn manifest(rows: usize, marker: &str) -> Vec<u8> {
    format!(
        concat!(
            "{{",
            "\"block_um\":500.0,",
            "\"coordinate_unit\":\"micrometers\",",
            "\"deduplicated_cells\":0,",
            "\"embedding_dtype\":\"float32\",",
            "\"embedding_width\":1280,",
            "\"n_cells\":{rows},",
            "\"row_alignment\":\"cells.csv embedding_row equals embeddings.npy row\",",
            "\"schema_name\":\"cellvit_he_bundle\",",
            "\"schema_version\":\"1.0\",",
            "\"sources\":[{{",
            "\"cells_json_sha256\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",",
            "\"cells_pt_sha256\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\",",
            "\"graph_position_scale\":0.662356930902925,",
            "\"mpp\":0.37744,",
            "\"roi_id\":\"roi-{marker}\",",
            "\"slide_path\":\"synthetic-{marker}\",",
            "\"specimen_id\":\"specimen-{marker}\",",
            "\"timepoint\":\"time-{marker}\",",
            "\"tumor_mask\":\"mask-{marker}\"",
            "}}]}}"
        ),
        rows = rows,
        marker = marker,
    )
    .into_bytes()
}

struct ChunkedCursor {
    inner: Cursor<Vec<u8>>,
    maximum_chunk: usize,
}

impl ChunkedCursor {
    fn new(bytes: Vec<u8>, maximum_chunk: usize) -> Self {
        Self {
            inner: Cursor::new(bytes),
            maximum_chunk,
        }
    }
}

impl Read for ChunkedCursor {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let length = buffer.len().min(self.maximum_chunk);
        self.inner.read(&mut buffer[..length])
    }
}

impl Seek for ChunkedCursor {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(position)
    }
}

#[test]
fn reconciliation_has_borrowed_chunked_and_order_independent_digest_parity() {
    let first = (npy(1, 1.0), csv("alpha", 1), manifest(1, "alpha"));
    let second = (npy(2, 2.0), csv("beta", 2), manifest(2, "beta"));
    let limits = budgets(
        first.0.len().max(second.0.len()),
        first.1.len().max(second.1.len()),
        first.2.len().max(second.2.len()),
    );
    let mut borrowed = SourceBundleReconciler::new(limits, 2);
    borrowed
        .push_bytes(&first.0, &first.1, &first.2)
        .expect("first bundle");
    borrowed
        .push_bytes(&second.0, &second.1, &second.2)
        .expect("second bundle");
    let borrowed = borrowed.finish().expect("borrowed report");

    let mut chunked = SourceBundleReconciler::new(limits, 2);
    for (npy, csv, manifest) in [second, first] {
        let mut npy = ChunkedCursor::new(npy, 1);
        let mut csv = ChunkedCursor::new(csv, 3);
        let mut manifest = ChunkedCursor::new(manifest, 2);
        chunked
            .push_readers(&mut npy, &mut csv, &mut manifest)
            .expect("chunked bundle");
    }
    let chunked = chunked.finish().expect("chunked report");

    assert_eq!(chunked, borrowed);
    assert_eq!(borrowed.bundle_count(), 2);
    assert_eq!(borrowed.row_count(), 3);
    assert_eq!(borrowed.dimension(), 1_280);
    assert_eq!(borrowed.present_count(), 3);
    assert_eq!(borrowed.qc_pass_count(), 3);
    assert_eq!(
        borrowed.aggregate_content_digest().to_string(),
        "3cf2aab1fb8c44beb9ea578865d656ce12e989246aa8db6a65b2af409df6c41b"
    );
    assert_eq!(
        borrowed.reconciliation_digest().to_string(),
        "fab1cc87fcb44fe0a7096641dde2f88fd91fc66bbfd7470e6357d339766c2c5b"
    );
    assert_eq!(
        borrowed.missing_promotion_fields(),
        &[
            MissingPromotionField::CanonicalIdentityMapping,
            MissingPromotionField::InputNormalizationAndRunConfiguration,
            MissingPromotionField::ReviewedSourceSnapshot,
            MissingPromotionField::LicenseRecord,
        ]
    );
    let debug = format!("{borrowed:?}");
    assert!(!debug.contains("synthetic-alpha"));
    assert!(!debug.contains("roi-alpha"));
}

#[test]
fn reconciliation_rejects_duplicate_bundles_and_bundle_limit_overflow() {
    let npy = npy(1, 1.0);
    let csv = csv("alpha", 1);
    let manifest = manifest(1, "alpha");
    let limits = budgets(npy.len(), csv.len(), manifest.len());
    let mut duplicate = SourceBundleReconciler::new(limits, 2);
    duplicate
        .push_bytes(&npy, &csv, &manifest)
        .expect("first bundle");
    assert!(matches!(
        duplicate.push_bytes(&npy, &csv, &manifest),
        Err(SourceBundleError::Reconciliation {
            reason: ReconciliationFailure::DuplicateBundle,
        })
    ));

    let mut limited = SourceBundleReconciler::new(limits, 0);
    assert!(matches!(
        limited.push_bytes(&npy, &csv, &manifest),
        Err(SourceBundleError::Reconciliation {
            reason: ReconciliationFailure::BundleLimitExceeded,
        })
    ));
}

#[test]
fn reconciliation_composes_live_accumulator_and_parser_retained_budgets() {
    let first = (npy(1, 1.0), csv("alpha", 1), manifest(1, "alpha"));
    let second = (npy(1, 2.0), csv("beta", 1), manifest(1, "beta"));
    let maximum_npy = first.0.len().max(second.0.len());
    let maximum_csv = first.1.len().max(second.1.len());
    let maximum_manifest = first.2.len().max(second.2.len());
    let mut retained = 64 * 1024;
    loop {
        let limits = budgets_with_retained(maximum_npy, maximum_csv, maximum_manifest, retained);
        let mut reconciler = SourceBundleReconciler::new(limits, 2);
        match reconciler.push_bytes(&first.0, &first.1, &first.2) {
            Ok(()) => break,
            Err(SourceBundleError::RetainedByteBudgetExceeded { required, .. })
                if required > retained =>
            {
                retained = required;
            }
            Err(error) => panic!("unexpected retained-budget probe failure: {error:?}"),
        }
    }
    let limits = budgets_with_retained(maximum_npy, maximum_csv, maximum_manifest, retained);
    let mut reconciler = SourceBundleReconciler::new(limits, 2);
    reconciler
        .push_bytes(&first.0, &first.1, &first.2)
        .expect("first bundle at exact parser budget");
    assert!(matches!(
        reconciler.push_bytes(&second.0, &second.1, &second.2),
        Err(SourceBundleError::RetainedByteBudgetExceeded {
            required,
            maximum,
        }) if required > maximum && maximum == retained
    ));
}

#[test]
fn reconciliation_manifest_is_strict_bounded_and_privacy_safe() {
    let npy = npy(1, 1.0);
    let csv = csv("alpha", 1);
    let valid = manifest(1, "privacy-sentinel");

    let cases = [
        (
            String::from_utf8(valid.clone())
                .expect("manifest text")
                .replace("\"n_cells\":1", "\"n_cells\":1,\"n_cells\":1")
                .into_bytes(),
            ManifestFailure::InvalidJson,
        ),
        (
            String::from_utf8(valid.clone())
                .expect("manifest text")
                .replace("\"embedding_width\":1280", "\"embedding_width\":1280.0")
                .into_bytes(),
            ManifestFailure::InvalidJson,
        ),
        (
            String::from_utf8(valid.clone())
                .expect("manifest text")
                .replace("\"n_cells\":1", "\"n_cells\":1,\"unknown\":0")
                .into_bytes(),
            ManifestFailure::InvalidJson,
        ),
        (
            String::from_utf8(valid.clone())
                .expect("manifest text")
                .replace("\"n_cells\":1", "\"n_cells\":2")
                .into_bytes(),
            ManifestFailure::RowCountMismatch,
        ),
        (
            String::from_utf8(valid.clone())
                .expect("manifest text")
                .replace("roi-privacy-sentinel", "roi-privacy-sentinel\\u0001")
                .into_bytes(),
            ManifestFailure::InvalidSensitiveString,
        ),
    ];

    for (manifest, reason) in cases {
        let limits = budgets(npy.len(), csv.len(), manifest.len());
        let mut reconciler = SourceBundleReconciler::new(limits, 1);
        let error = reconciler
            .push_bytes(&npy, &csv, &manifest)
            .expect_err("invalid manifest");
        assert!(matches!(
            error,
            SourceBundleError::Manifest { reason: observed } if observed == reason
        ));
        assert!(!error.to_string().contains("privacy-sentinel"));
    }

    let limits = SourceBundleBudgets::new(
        npy.len() as u64,
        csv.len() as u64,
        (valid.len() - 1) as u64,
        4 * 1024 * 1024,
        4 * 1024 * 1024,
    );
    let mut reconciler = SourceBundleReconciler::new(limits, 1);
    assert!(matches!(
        reconciler.push_bytes(&npy, &csv, &valid),
        Err(SourceBundleError::FileByteBudgetExceeded { .. })
    ));
}
