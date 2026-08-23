use std::io::{Cursor, Read, Seek, SeekFrom};

use marklab::{
    CellVitNpyMatrix, CellVitNpySummary, NpyFailure, NpyVersion, SourceBundleBudgets,
    SourceBundleError,
};

fn budgets(npy_bytes: usize, decoded_bytes: usize) -> SourceBundleBudgets {
    SourceBundleBudgets::new(
        u64::try_from(npy_bytes).expect("small NPY"),
        64 * 1024 * 1024,
        64 * 1024,
        4 * 1024 * 1024,
        u64::try_from(decoded_bytes).expect("small decoded payload"),
    )
}

fn npy(version: NpyVersion, dictionary: &str, values: &[f32]) -> Vec<u8> {
    let (version_bytes, length_bytes) = match version {
        NpyVersion::V1 => ([1, 0], 2),
        NpyVersion::V2 => ([2, 0], 4),
    };
    let prefix_length = 6 + version_bytes.len() + length_bytes;
    let padding = (16 - ((prefix_length + dictionary.len() + 1) % 16)) % 16;
    let mut header = dictionary.as_bytes().to_vec();
    header.resize(header.len() + padding, b' ');
    header.push(b'\n');
    let mut bytes = b"\x93NUMPY".to_vec();
    bytes.extend_from_slice(&version_bytes);
    match version {
        NpyVersion::V1 => bytes.extend_from_slice(
            &u16::try_from(header.len())
                .expect("v1 header")
                .to_le_bytes(),
        ),
        NpyVersion::V2 => bytes.extend_from_slice(
            &u32::try_from(header.len())
                .expect("v2 header")
                .to_le_bytes(),
        ),
    }
    bytes.extend_from_slice(&header);
    for value in values {
        bytes.extend_from_slice(&value.to_bits().to_le_bytes());
    }
    bytes
}

fn valid(version: NpyVersion) -> Vec<u8> {
    let mut values = vec![0.0; 2 * 1_280];
    values[0] = 1.25;
    values[1_280 + 17] = -3.5;
    npy(
        version,
        "{'descr': '<f4', 'fortran_order': False, 'shape': (2, 1280), }",
        &values,
    )
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
fn borrowed_and_chunked_npy_parsers_have_exact_summary_parity() {
    for version in [NpyVersion::V1, NpyVersion::V2] {
        let bytes = valid(version);
        let limits = budgets(bytes.len(), 2 * 1_280 * 4);
        let matrix = CellVitNpyMatrix::from_bytes(&bytes, limits).expect("borrowed matrix");
        assert_eq!(matrix.row_count(), 2);
        assert_eq!(matrix.dimension(), 1_280);
        assert_eq!(matrix.value(0, 0).expect("first value"), 1.25);
        assert_eq!(matrix.value(1, 17).expect("second-row value"), -3.5);
        let borrowed = matrix.summary();
        let debug = format!("{matrix:?}");
        assert!(debug.contains("summary"));
        assert!(!debug.contains("payload"));
        assert!(debug.len() < 384);

        let mut reader = ChunkedCursor::new(bytes, 7);
        let streamed = CellVitNpySummary::from_reader(&mut reader, limits).expect("streamed NPY");
        assert_eq!(streamed, borrowed);
        assert_eq!(streamed.version(), version);
        assert_eq!(streamed.present_count(), 2);
    }
}

#[test]
fn streamed_npy_precharges_header_and_payload_buffer_before_validation() {
    let bytes = valid(NpyVersion::V2);
    let header_length = u32::from_le_bytes(bytes[8..12].try_into().expect("header length"));
    let required = 12 + usize::try_from(header_length).expect("header bytes") + 64 * 1024;
    let limits = SourceBundleBudgets::new(
        bytes.len() as u64,
        64 * 1024 * 1024,
        64 * 1024,
        required - 1,
        (2 * 1_280 * 4) as u64,
    );
    let mut reader = Cursor::new(&bytes);
    assert_eq!(
        CellVitNpySummary::from_reader(&mut reader, limits),
        Err(SourceBundleError::RetainedByteBudgetExceeded {
            required,
            maximum: required - 1,
        })
    );
    let limits = SourceBundleBudgets::new(
        bytes.len() as u64,
        64 * 1024 * 1024,
        64 * 1024,
        required,
        (2 * 1_280 * 4) as u64,
    );
    let mut reader = Cursor::new(bytes);
    CellVitNpySummary::from_reader(&mut reader, limits).expect("exact retained parser budget");
}

#[test]
fn borrowed_and_reader_npy_errors_match_at_truncation_boundaries() {
    let full = valid(NpyVersion::V1);
    let header_length = usize::from(u16::from_le_bytes(
        full[8..10].try_into().expect("header length"),
    ));
    let data_offset = 10 + header_length;
    for length in [
        0,
        5,
        6,
        7,
        8,
        9,
        10,
        data_offset - 1,
        data_offset,
        full.len() - 1,
    ] {
        let bytes = &full[..length];
        let limits = budgets(bytes.len(), 2 * 1_280 * 4);
        let borrowed = CellVitNpyMatrix::from_bytes(bytes, limits).expect_err("truncated bytes");
        let mut reader = ChunkedCursor::new(bytes.to_vec(), 1);
        let streamed =
            CellVitNpySummary::from_reader(&mut reader, limits).expect_err("truncated reader");
        assert_eq!(streamed, borrowed, "parity failed at byte length {length}");
    }
}

#[test]
fn npy_header_accepts_only_the_closed_cellvit_profile() {
    let accepted = npy(
        NpyVersion::V1,
        "{ 'shape' : (1,1280), 'descr' : '<f4', 'fortran_order' : False, }",
        &vec![0.0; 1_280],
    );
    CellVitNpyMatrix::from_bytes(&accepted, budgets(accepted.len(), 1_280 * 4))
        .expect("key order and bounded whitespace are irrelevant");

    let cases = [
        (
            "{'descr': '>f4', 'fortran_order': False, 'shape': (1, 1280), }",
            NpyFailure::UnsupportedDescriptor,
        ),
        (
            "{'descr': '=f4', 'fortran_order': False, 'shape': (1, 1280), }",
            NpyFailure::UnsupportedDescriptor,
        ),
        (
            "{'descr': '<f8', 'fortran_order': False, 'shape': (1, 1280), }",
            NpyFailure::UnsupportedDescriptor,
        ),
        (
            "{'descr': '|O', 'fortran_order': False, 'shape': (1, 1280), }",
            NpyFailure::UnsupportedDescriptor,
        ),
        (
            "{'descr': '<f4', 'fortran_order': True, 'shape': (1, 1280), }",
            NpyFailure::FortranOrder,
        ),
        (
            "{'descr': '<f4', 'fortran_order': False, 'shape': (1,), }",
            NpyFailure::InvalidShape,
        ),
        (
            "{'descr': '<f4', 'fortran_order': False, 'shape': (1, 1024), }",
            NpyFailure::WrongDimension,
        ),
        (
            "{'descr': '<f4', 'descr': '<f4', 'fortran_order': False, 'shape': (1, 1280), }",
            NpyFailure::DuplicateHeaderKey,
        ),
        (
            "{'descr': '<f4', 'fortran_order': False, 'shape': (1, 1280), 'extra': 'x', }",
            NpyFailure::UnknownHeaderKey,
        ),
        (
            "{'descr': '<f4', 'fortran_order': False, 'shape': (01, 1280), }",
            NpyFailure::InvalidShape,
        ),
        (
            "{'descr': '<f4', 'fortran_order': False, 'shape': (1, 0), }",
            NpyFailure::InvalidShape,
        ),
        (
            "{'descr': '<f4', 'fortran_order': False, 'shape': (100000001, 1280), }",
            NpyFailure::RowLimitExceeded,
        ),
        (
            "{'descr': '<f4', 'fortran_order': False, 'shape': (18446744073709551616, 1280), }",
            NpyFailure::InvalidShape,
        ),
        (
            "{'descr': '<f4', 'fortran_order': False, 'shape': (1, 1280), # comment }",
            NpyFailure::InvalidHeaderGrammar,
        ),
    ];
    for (dictionary, reason) in cases {
        let bytes = npy(NpyVersion::V1, dictionary, &vec![0.0; 1_280]);
        assert!(matches!(
            CellVitNpyMatrix::from_bytes(&bytes, budgets(bytes.len(), 1_280 * 4)),
            Err(SourceBundleError::Npy { reason: observed }) if observed == reason
        ));
    }
}

#[test]
fn npy_rejects_magic_version_padding_length_budget_and_nonfinite_failures() {
    let valid = valid(NpyVersion::V1);
    let mut bad_magic = valid.clone();
    bad_magic[0] = 0;
    assert!(matches!(
        CellVitNpyMatrix::from_bytes(&bad_magic, budgets(bad_magic.len(), 2 * 1_280 * 4)),
        Err(SourceBundleError::Npy {
            reason: NpyFailure::InvalidMagic
        })
    ));

    let mut bad_version = valid.clone();
    bad_version[6..8].copy_from_slice(&[3, 0]);
    assert!(matches!(
        CellVitNpyMatrix::from_bytes(&bad_version, budgets(bad_version.len(), 2 * 1_280 * 4)),
        Err(SourceBundleError::Npy {
            reason: NpyFailure::UnsupportedVersion
        })
    ));

    let mut missing_newline = valid.clone();
    let newline = missing_newline
        .iter()
        .position(|byte| *byte == b'\n')
        .expect("header newline");
    missing_newline[newline] = b' ';
    assert!(matches!(
        CellVitNpyMatrix::from_bytes(
            &missing_newline,
            budgets(missing_newline.len(), 2 * 1_280 * 4)
        ),
        Err(SourceBundleError::Npy {
            reason: NpyFailure::InvalidHeaderEncoding
        })
    ));

    let mut non_ascii = valid.clone();
    non_ascii[12] = 0xff;
    assert!(matches!(
        CellVitNpyMatrix::from_bytes(&non_ascii, budgets(non_ascii.len(), 2 * 1_280 * 4)),
        Err(SourceBundleError::Npy {
            reason: NpyFailure::InvalidHeaderEncoding
        })
    ));

    let mut misaligned = valid.clone();
    let header_length = u16::from_le_bytes(misaligned[8..10].try_into().expect("header length"));
    misaligned[8..10].copy_from_slice(&(header_length - 1).to_le_bytes());
    assert!(matches!(
        CellVitNpyMatrix::from_bytes(&misaligned, budgets(misaligned.len(), 2 * 1_280 * 4)),
        Err(SourceBundleError::Npy {
            reason: NpyFailure::InvalidHeaderAlignment
        })
    ));

    let mut oversized_header = b"\x93NUMPY\x02\x00".to_vec();
    oversized_header.extend_from_slice(&(64_u32 * 1024 + 1).to_le_bytes());
    assert!(matches!(
        CellVitNpyMatrix::from_bytes(
            &oversized_header,
            budgets(oversized_header.len(), 2 * 1_280 * 4)
        ),
        Err(SourceBundleError::HeaderByteBudgetExceeded { .. })
    ));

    let mut bad_padding = valid.clone();
    let newline = bad_padding
        .iter()
        .position(|byte| *byte == b'\n')
        .expect("header newline");
    bad_padding[newline - 1] = b'x';
    assert!(matches!(
        CellVitNpyMatrix::from_bytes(&bad_padding, budgets(bad_padding.len(), 2 * 1_280 * 4)),
        Err(SourceBundleError::Npy { .. })
    ));

    let mut trailing = valid.clone();
    trailing.push(0);
    assert!(matches!(
        CellVitNpyMatrix::from_bytes(&trailing, budgets(trailing.len(), 2 * 1_280 * 4)),
        Err(SourceBundleError::Npy {
            reason: NpyFailure::PayloadLengthMismatch
        })
    ));
    let truncated = &valid[..valid.len() - 1];
    assert!(matches!(
        CellVitNpyMatrix::from_bytes(truncated, budgets(truncated.len(), 2 * 1_280 * 4)),
        Err(SourceBundleError::Npy {
            reason: NpyFailure::PayloadLengthMismatch
        })
    ));
    assert!(matches!(
        CellVitNpyMatrix::from_bytes(&valid, budgets(valid.len() - 1, 2 * 1_280 * 4)),
        Err(SourceBundleError::FileByteBudgetExceeded { .. })
    ));
    assert!(matches!(
        CellVitNpyMatrix::from_bytes(&valid, budgets(valid.len(), 2 * 1_280 * 4 - 1)),
        Err(SourceBundleError::DecodedByteBudgetExceeded { .. })
    ));

    let mut nonfinite = vec![0.0; 1_280];
    nonfinite[23] = f32::INFINITY;
    let nonfinite = npy(
        NpyVersion::V1,
        "{'descr': '<f4', 'fortran_order': False, 'shape': (1, 1280), }",
        &nonfinite,
    );
    assert!(matches!(
        CellVitNpyMatrix::from_bytes(&nonfinite, budgets(nonfinite.len(), 1_280 * 4)),
        Err(SourceBundleError::NonFiniteSourceComponent { row: 0, column: 23 })
    ));
}
