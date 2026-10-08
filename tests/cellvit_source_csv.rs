#![cfg(feature = "csv")]

use std::io::{Cursor, Read, Seek, SeekFrom};

use marklab::{
    CellVitCsvField, CellVitCsvSummary, ContentDigest, CsvFailure, SourceBundleBudgets,
    SourceBundleError,
};

const HEADER: [&str; 23] = [
    "cell_id",
    "case_id",
    "specimen_id",
    "timepoint",
    "fragment_id",
    "roi_id",
    "native_row",
    "embedding_row",
    "x_px",
    "y_px",
    "x_um",
    "y_um",
    "cell_type_id",
    "cell_type_label",
    "type_probability",
    "nucleus_area_um2",
    "nucleus_perimeter_um",
    "eccentricity",
    "solidity",
    "circularity",
    "qc_pass",
    "block_500_id",
    "split",
];

fn budgets(csv_bytes: usize, retained_bytes: usize) -> SourceBundleBudgets {
    SourceBundleBudgets::new(
        64 * 1024 * 1024,
        u64::try_from(csv_bytes).expect("small CSV"),
        64 * 1024,
        retained_bytes,
        64 * 1024 * 1024,
    )
}

fn row(index: u64, cell_id: &str) -> Vec<String> {
    [
        cell_id.to_owned(),
        "case-synthetic".to_owned(),
        "specimen-synthetic".to_owned(),
        "timepoint-1".to_owned(),
        "fragment-1".to_owned(),
        "roi-1".to_owned(),
        index.to_string(),
        index.to_string(),
        "12.5".to_owned(),
        "13".to_owned(),
        "4.25".to_owned(),
        "5".to_owned(),
        "2".to_owned(),
        "type,\"A\"".to_owned(),
        "0.75".to_owned(),
        "20.0".to_owned(),
        "18.5".to_owned(),
        "0.25".to_owned(),
        "1.0".to_owned(),
        "0".to_owned(),
        "True".to_owned(),
        "block-1".to_owned(),
        "train.v1".to_owned(),
    ]
    .into()
}

fn encode_field(value: &str) -> String {
    if value.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn csv_with_header(header: &[&str], rows: &[Vec<String>]) -> Vec<u8> {
    let mut encoded = String::new();
    encoded.push_str(&header.join(","));
    encoded.push_str("\r\n");
    for row in rows {
        encoded.push_str(
            &row.iter()
                .map(|value| encode_field(value))
                .collect::<Vec<_>>()
                .join(","),
        );
        encoded.push_str("\r\n");
    }
    encoded.into_bytes()
}

fn valid_csv() -> Vec<u8> {
    csv_with_header(&HEADER, &[row(0, "source,one"), row(1, "source-two")])
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
fn borrowed_and_chunked_csv_validation_have_aggregate_only_parity() {
    let bytes = valid_csv();
    let limits = budgets(bytes.len(), 512 * 1024);
    let borrowed = CellVitCsvSummary::from_bytes(&bytes, limits).expect("borrowed CSV");
    let mut reader = ChunkedCursor::new(bytes.clone(), 3);
    let streamed = CellVitCsvSummary::from_reader(&mut reader, limits).expect("streamed CSV");
    assert_eq!(streamed, borrowed);
    assert_eq!(borrowed.row_count(), 2);
    assert_eq!(borrowed.qc_pass_count(), 2);
    assert_eq!(borrowed.content_digest(), ContentDigest::from_bytes(&bytes));
    let debug = format!("{borrowed:?}");
    assert!(!debug.contains("source,one"));
    assert!(!debug.contains("case-synthetic"));
}

#[test]
fn csv_requires_the_exact_header_and_bounded_rfc4180_records() {
    let mut reordered = HEADER;
    reordered.swap(0, 1);
    let bytes = csv_with_header(&reordered, &[row(0, "source")]);
    assert!(matches!(
        CellVitCsvSummary::from_bytes(&bytes, budgets(bytes.len(), 512 * 1024)),
        Err(SourceBundleError::Csv {
            row: None,
            field: None,
            reason: CsvFailure::WrongHeader,
        })
    ));

    let mut bom = valid_csv();
    bom.splice(0..0, [0xef, 0xbb, 0xbf]);
    assert!(matches!(
        CellVitCsvSummary::from_bytes(&bom, budgets(bom.len(), 512 * 1024)),
        Err(SourceBundleError::Csv {
            reason: CsvFailure::WrongHeader,
            ..
        })
    ));

    let malformed = format!("{}\r\n\"unterminated,case\r\n", HEADER.join(","));
    assert!(matches!(
        CellVitCsvSummary::from_bytes(malformed.as_bytes(), budgets(malformed.len(), 512 * 1024)),
        Err(SourceBundleError::Csv {
            reason: CsvFailure::InvalidRecordSyntax,
            ..
        })
    ));

    let too_many_fields = format!("{}\r\n{}\r\n", HEADER.join(","), vec!["x"; 24].join(","));
    assert!(matches!(
        CellVitCsvSummary::from_bytes(
            too_many_fields.as_bytes(),
            budgets(too_many_fields.len(), 512 * 1024)
        ),
        Err(SourceBundleError::Csv {
            row: Some(0),
            reason: CsvFailure::WrongFieldCount,
            ..
        })
    ));

    let huge = csv_with_header(&HEADER, &[row(0, &"x".repeat(256 * 1024))]);
    assert!(matches!(
        CellVitCsvSummary::from_bytes(&huge, budgets(huge.len(), 2 * 1024 * 1024)),
        Err(SourceBundleError::CsvRecordByteBudgetExceeded { .. })
    ));

    let mut boundary = row(0, "");
    let fixed_record_bytes = boundary
        .iter()
        .map(|value| encode_field(value))
        .collect::<Vec<_>>()
        .join(",")
        .len();
    boundary[0] = "x".repeat(256 * 1024 - fixed_record_bytes);
    let exact = csv_with_header(&HEADER, &[boundary.clone()]);
    assert!(matches!(
        CellVitCsvSummary::from_bytes(&exact, budgets(exact.len(), 2 * 1024 * 1024)),
        Err(SourceBundleError::Csv {
            field: Some(CellVitCsvField::CellId),
            reason: CsvFailure::InvalidIdentifier,
            ..
        })
    ));
    boundary[0].push('x');
    let over = csv_with_header(&HEADER, &[boundary]);
    assert!(matches!(
        CellVitCsvSummary::from_bytes(&over, budgets(over.len(), 2 * 1024 * 1024)),
        Err(SourceBundleError::CsvRecordByteBudgetExceeded {
            record: 1,
            observed: 262_145,
            maximum: 262_144,
        })
    ));
    let valid = valid_csv();
    assert!(matches!(
        CellVitCsvSummary::from_bytes(&valid, budgets(valid.len() - 1, 512 * 1024)),
        Err(SourceBundleError::FileByteBudgetExceeded { .. })
    ));
    assert!(matches!(
        CellVitCsvSummary::from_bytes(&valid, budgets(valid.len(), 0)),
        Err(SourceBundleError::RetainedByteBudgetExceeded { .. })
    ));
}

#[test]
fn csv_retained_budget_has_exact_reject_and_accept_boundaries() {
    let bytes = valid_csv();
    let mut required = 256 * 1024;
    loop {
        match CellVitCsvSummary::from_bytes(&bytes, budgets(bytes.len(), required)) {
            Ok(_) => break,
            Err(SourceBundleError::RetainedByteBudgetExceeded { required: next, .. })
                if next > required =>
            {
                required = next
            }
            Err(other) => panic!("unexpected parser budget failure: {other:?}"),
        }
    }
    assert!(matches!(
        CellVitCsvSummary::from_bytes(&bytes, budgets(bytes.len(), required - 1)),
        Err(SourceBundleError::RetainedByteBudgetExceeded {
            required: observed,
            maximum,
        }) if observed == required && maximum == required - 1
    ));
    CellVitCsvSummary::from_bytes(&bytes, budgets(bytes.len(), required))
        .expect("exact retained parser budget");
}

#[test]
fn csv_rejects_invalid_fields_without_echoing_source_values() {
    let cases = [
        (0, "", CellVitCsvField::CellId, CsvFailure::EmptyField),
        (
            1,
            "case\nprivacy-sentinel",
            CellVitCsvField::CaseId,
            CsvFailure::InvalidIdentifier,
        ),
        (
            5,
            &"r".repeat(129),
            CellVitCsvField::RoiId,
            CsvFailure::InvalidIdentifier,
        ),
        (
            6,
            "01",
            CellVitCsvField::NativeRow,
            CsvFailure::InvalidUnsigned,
        ),
        (
            12,
            "4294967296",
            CellVitCsvField::CellTypeId,
            CsvFailure::InvalidUnsigned,
        ),
        (
            14,
            "1e-2",
            CellVitCsvField::TypeProbability,
            CsvFailure::InvalidDecimal,
        ),
        (
            17,
            "1.01",
            CellVitCsvField::Eccentricity,
            CsvFailure::OutOfRange,
        ),
        (
            20,
            "true",
            CellVitCsvField::QcPass,
            CsvFailure::InvalidQcPass,
        ),
        (
            21,
            &"b".repeat(33),
            CellVitCsvField::Block500Id,
            CsvFailure::InvalidIdentifier,
        ),
        (
            22,
            "bad split",
            CellVitCsvField::Split,
            CsvFailure::InvalidToken,
        ),
    ];
    for (index, value, field, reason) in cases {
        let mut changed = row(0, "source");
        changed[index] = value.to_owned();
        let bytes = csv_with_header(&HEADER, &[changed]);
        let error = CellVitCsvSummary::from_bytes(&bytes, budgets(bytes.len(), 512 * 1024))
            .expect_err("invalid field");
        assert!(matches!(
            error,
            SourceBundleError::Csv {
                row: Some(0),
                field: Some(observed_field),
                reason: observed_reason,
            } if observed_field == field && observed_reason == reason
        ));
        assert!(!error.to_string().contains("privacy-sentinel"));
    }
}

#[test]
fn csv_requires_unique_source_ids_and_both_exact_row_columns() {
    let duplicate = csv_with_header(&HEADER, &[row(0, "same"), row(1, "same")]);
    assert!(matches!(
        CellVitCsvSummary::from_bytes(&duplicate, budgets(duplicate.len(), 512 * 1024)),
        Err(SourceBundleError::Csv {
            reason: CsvFailure::DuplicateSourceCellId,
            ..
        })
    ));

    for (field_index, field) in [
        (6, CellVitCsvField::NativeRow),
        (7, CellVitCsvField::EmbeddingRow),
    ] {
        let mut changed = row(0, "source");
        changed[field_index] = "1".to_owned();
        let bytes = csv_with_header(&HEADER, &[changed]);
        assert!(matches!(
            CellVitCsvSummary::from_bytes(&bytes, budgets(bytes.len(), 512 * 1024)),
            Err(SourceBundleError::Csv {
                row: Some(0),
                field: Some(observed),
                reason: CsvFailure::SourceRowMismatch,
            }) if observed == field
        ));
    }
}
