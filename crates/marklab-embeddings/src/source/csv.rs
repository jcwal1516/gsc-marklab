use std::io::{Cursor, ErrorKind, SeekFrom, Write};

use marklab_project::{ArtifactReadSeek, ContentDigest};

use super::{
    CellVitCsvField, CsvFailure, SourceBundleBudgets, SourceBundleError, SourceFileKind,
    SourceIoFailure, SourceIoOperation,
};

const MAX_CSV_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CSV_RECORD_BYTES: u64 = 256 * 1024;
const PREFLIGHT_BUFFER_BYTES: usize = 64 * 1024;
const CSV_READER_BUFFER_BYTES: usize = 8 * 1024;
const CSV_FIELD_END_CAPACITY: usize = 32;
const CSV_PARSER_ALLOCATION_OVERHEAD: usize = 4 * 1024;
const PARSER_RETAINED_BYTES: usize = MAX_CSV_RECORD_BYTES as usize
    + CSV_READER_BUFFER_BYTES
    + CSV_FIELD_END_CAPACITY * size_of::<usize>()
    + CSV_PARSER_ALLOCATION_OVERHEAD;

const HEADER: [&[u8]; 23] = [
    b"cell_id",
    b"case_id",
    b"specimen_id",
    b"timepoint",
    b"fragment_id",
    b"roi_id",
    b"native_row",
    b"embedding_row",
    b"x_px",
    b"y_px",
    b"x_um",
    b"y_um",
    b"cell_type_id",
    b"cell_type_label",
    b"type_probability",
    b"nucleus_area_um2",
    b"nucleus_perimeter_um",
    b"eccentricity",
    b"solidity",
    b"circularity",
    b"qc_pass",
    b"block_500_id",
    b"split",
];

const ALL_FIELDS: [CellVitCsvField; 23] = [
    CellVitCsvField::CellId,
    CellVitCsvField::CaseId,
    CellVitCsvField::SpecimenId,
    CellVitCsvField::Timepoint,
    CellVitCsvField::FragmentId,
    CellVitCsvField::RoiId,
    CellVitCsvField::NativeRow,
    CellVitCsvField::EmbeddingRow,
    CellVitCsvField::XPx,
    CellVitCsvField::YPx,
    CellVitCsvField::XUm,
    CellVitCsvField::YUm,
    CellVitCsvField::CellTypeId,
    CellVitCsvField::CellTypeLabel,
    CellVitCsvField::TypeProbability,
    CellVitCsvField::NucleusAreaUm2,
    CellVitCsvField::NucleusPerimeterUm,
    CellVitCsvField::Eccentricity,
    CellVitCsvField::Solidity,
    CellVitCsvField::Circularity,
    CellVitCsvField::QcPass,
    CellVitCsvField::Block500Id,
    CellVitCsvField::Split,
];

const IDENTIFIER_FIELDS: [CellVitCsvField; 7] = [
    CellVitCsvField::CellId,
    CellVitCsvField::CaseId,
    CellVitCsvField::SpecimenId,
    CellVitCsvField::Timepoint,
    CellVitCsvField::FragmentId,
    CellVitCsvField::RoiId,
    CellVitCsvField::CellTypeLabel,
];

const NONNEGATIVE_FIELDS: [CellVitCsvField; 6] = [
    CellVitCsvField::XPx,
    CellVitCsvField::YPx,
    CellVitCsvField::XUm,
    CellVitCsvField::YUm,
    CellVitCsvField::NucleusAreaUm2,
    CellVitCsvField::NucleusPerimeterUm,
];

const UNIT_INTERVAL_FIELDS: [CellVitCsvField; 4] = [
    CellVitCsvField::TypeProbability,
    CellVitCsvField::Eccentricity,
    CellVitCsvField::Solidity,
    CellVitCsvField::Circularity,
];

/// Aggregate-only facts from one fully validated frozen CellViT source CSV.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellVitCsvSummary {
    row_count: u64,
    qc_pass_count: u64,
    encoded_byte_len: u64,
    content_digest: ContentDigest,
}

impl CellVitCsvSummary {
    /// Validate one complete borrowed CSV file through the bounded reader path.
    pub fn from_bytes(
        bytes: &[u8],
        budgets: SourceBundleBudgets,
    ) -> Result<Self, SourceBundleError> {
        let mut reader = Cursor::new(bytes);
        Self::from_reader(&mut reader, budgets)
    }

    /// Validate a bounded seekable CSV reader without retaining source identifiers.
    pub fn from_reader(
        reader: &mut dyn ArtifactReadSeek,
        budgets: SourceBundleBudgets,
    ) -> Result<Self, SourceBundleError> {
        let parsed = parse_reader(reader, budgets)?;
        debug_assert!(parsed.rows.iter().all(|row| {
            row.native_row == row.embedding_row && row.native_row < parsed.summary.row_count
        }));
        Ok(parsed.summary)
    }

    /// Exact validated source data-row count.
    pub fn row_count(self) -> u64 {
        self.row_count
    }

    /// Rows whose frozen source QC predicate is exact `True`.
    pub fn qc_pass_count(self) -> u64 {
        self.qc_pass_count
    }

    /// Exact encoded CSV byte length.
    pub fn encoded_byte_len(self) -> u64 {
        self.encoded_byte_len
    }

    /// Digest of the exact encoded CSV bytes.
    pub fn content_digest(self) -> ContentDigest {
        self.content_digest
    }
}

pub(crate) struct ParsedCsv {
    pub(crate) rows: Vec<SourceRow>,
    pub(crate) summary: CellVitCsvSummary,
    pub(crate) retained_bytes: usize,
}

pub(crate) struct SourceRow {
    pub(crate) source_cell_id: String,
    pub(crate) native_row: u64,
    pub(crate) embedding_row: u64,
    pub(crate) canonical_row: usize,
}

pub(crate) fn parse_reader(
    reader: &mut dyn ArtifactReadSeek,
    budgets: SourceBundleBudgets,
) -> Result<ParsedCsv, SourceBundleError> {
    let file_bytes = reader
        .seek(SeekFrom::End(0))
        .map_err(|error| io_error(SourceIoOperation::InspectLength, &error))?;
    let maximum_file_bytes = budgets.maximum_csv_file_bytes().min(MAX_CSV_FILE_BYTES);
    if file_bytes > maximum_file_bytes {
        return Err(SourceBundleError::FileByteBudgetExceeded {
            file: SourceFileKind::Csv,
            observed: file_bytes,
            maximum: maximum_file_bytes,
        });
    }
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|error| io_error(SourceIoOperation::Seek, &error))?;
    enforce_retained(PREFLIGHT_BUFFER_BYTES, budgets)?;
    let preflight = preflight(reader, file_bytes)?;
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|error| io_error(SourceIoOperation::Seek, &error))?;

    let data_rows = preflight
        .record_count
        .checked_sub(1)
        .ok_or_else(|| csv_error(None, None, CsvFailure::WrongHeader))?;
    if data_rows == 0 {
        return Err(csv_error(None, None, CsvFailure::EmptySource));
    }
    let data_rows_usize =
        usize::try_from(data_rows).map_err(|_| SourceBundleError::SizeOverflow)?;
    let row_bytes = data_rows_usize
        .checked_mul(size_of::<SourceRow>())
        .ok_or(SourceBundleError::SizeOverflow)?;
    let mut retained_bytes = PARSER_RETAINED_BYTES
        .checked_add(row_bytes)
        .ok_or(SourceBundleError::SizeOverflow)?;
    enforce_retained(retained_bytes, budgets)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(data_rows_usize)
        .map_err(|_| SourceBundleError::AllocationFailed {
            requested: retained_bytes,
        })?;

    let mut csv = ::csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .buffer_capacity(CSV_READER_BUFFER_BYTES)
        .from_reader(reader);
    let mut record = ::csv::ByteRecord::with_capacity(MAX_CSV_RECORD_BYTES as usize, HEADER.len());
    if !csv
        .read_byte_record(&mut record)
        .map_err(csv_library_error)?
        || record.len() != HEADER.len()
        || !record
            .iter()
            .zip(HEADER)
            .all(|(actual, expected)| actual == expected)
    {
        return Err(csv_error(None, None, CsvFailure::WrongHeader));
    }
    let mut data_row = 0_u64;
    while csv
        .read_byte_record(&mut record)
        .map_err(csv_library_error)?
    {
        if record.len() != HEADER.len() {
            return Err(csv_error(Some(data_row), None, CsvFailure::WrongFieldCount));
        }
        validate_record(&record, data_row)?;
        let source = text(
            field(&record, CellVitCsvField::CellId),
            data_row,
            CellVitCsvField::CellId,
        )?;
        retained_bytes = retained_bytes
            .checked_add(source.len())
            .ok_or(SourceBundleError::SizeOverflow)?;
        enforce_retained(retained_bytes, budgets)?;
        let mut source_cell_id = String::new();
        source_cell_id
            .try_reserve_exact(source.len())
            .map_err(|_| SourceBundleError::AllocationFailed {
                requested: retained_bytes,
            })?;
        source_cell_id.push_str(source);
        rows.push(SourceRow {
            source_cell_id,
            native_row: data_row,
            embedding_row: data_row,
            canonical_row: usize::MAX,
        });
        data_row = data_row
            .checked_add(1)
            .ok_or(SourceBundleError::SizeOverflow)?;
    }
    if data_row != data_rows {
        return Err(csv_error(None, None, CsvFailure::InvalidRecordSyntax));
    }
    rows.sort_unstable_by(|left, right| left.source_cell_id.cmp(&right.source_cell_id));
    if let Some(duplicate) = rows
        .windows(2)
        .find(|pair| pair[0].source_cell_id == pair[1].source_cell_id)
    {
        return Err(csv_error(
            Some(duplicate[1].native_row),
            Some(CellVitCsvField::CellId),
            CsvFailure::DuplicateSourceCellId,
        ));
    }
    rows.sort_unstable_by_key(|row| row.native_row);
    Ok(ParsedCsv {
        rows,
        retained_bytes,
        summary: CellVitCsvSummary {
            row_count: data_rows,
            qc_pass_count: data_rows,
            encoded_byte_len: file_bytes,
            content_digest: preflight.content_digest,
        },
    })
}

fn validate_record(record: &::csv::ByteRecord, row: u64) -> Result<(), SourceBundleError> {
    for field_name in ALL_FIELDS {
        if field(record, field_name).is_empty() {
            return Err(csv_error(
                Some(row),
                Some(field_name),
                CsvFailure::EmptyField,
            ));
        }
    }
    for field_name in IDENTIFIER_FIELDS {
        validate_identifier(field(record, field_name), 128, row, field_name)?;
    }
    validate_identifier(
        field(record, CellVitCsvField::Block500Id),
        32,
        row,
        CellVitCsvField::Block500Id,
    )?;
    validate_token(
        field(record, CellVitCsvField::Split),
        row,
        CellVitCsvField::Split,
    )?;
    let native = parse_unsigned(
        field(record, CellVitCsvField::NativeRow),
        u64::MAX,
        row,
        CellVitCsvField::NativeRow,
    )?;
    let embedding = parse_unsigned(
        field(record, CellVitCsvField::EmbeddingRow),
        u64::MAX,
        row,
        CellVitCsvField::EmbeddingRow,
    )?;
    parse_unsigned(
        field(record, CellVitCsvField::CellTypeId),
        u64::from(u32::MAX),
        row,
        CellVitCsvField::CellTypeId,
    )?;
    if native != row {
        return Err(csv_error(
            Some(row),
            Some(CellVitCsvField::NativeRow),
            CsvFailure::SourceRowMismatch,
        ));
    }
    if embedding != row {
        return Err(csv_error(
            Some(row),
            Some(CellVitCsvField::EmbeddingRow),
            CsvFailure::SourceRowMismatch,
        ));
    }
    for field_name in NONNEGATIVE_FIELDS {
        parse_decimal(field(record, field_name), row, field_name)?;
    }
    for field_name in UNIT_INTERVAL_FIELDS {
        if parse_decimal(field(record, field_name), row, field_name)? > 1.0 {
            return Err(csv_error(
                Some(row),
                Some(field_name),
                CsvFailure::OutOfRange,
            ));
        }
    }
    if field(record, CellVitCsvField::QcPass) != b"True" {
        return Err(csv_error(
            Some(row),
            Some(CellVitCsvField::QcPass),
            CsvFailure::InvalidQcPass,
        ));
    }
    Ok(())
}

fn field(record: &::csv::ByteRecord, field: CellVitCsvField) -> &[u8] {
    record.get(field.index()).unwrap_or_default()
}

fn validate_identifier(
    value: &[u8],
    maximum: usize,
    row: u64,
    field: CellVitCsvField,
) -> Result<(), SourceBundleError> {
    let value = text(value, row, field)?;
    if value.len() > maximum || value.chars().any(char::is_control) {
        return Err(csv_error(
            Some(row),
            Some(field),
            CsvFailure::InvalidIdentifier,
        ));
    }
    Ok(())
}

fn validate_token(value: &[u8], row: u64, field: CellVitCsvField) -> Result<(), SourceBundleError> {
    if value.len() > 32
        || !value
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(csv_error(Some(row), Some(field), CsvFailure::InvalidToken));
    }
    Ok(())
}

fn parse_unsigned(
    value: &[u8],
    maximum: u64,
    row: u64,
    field: CellVitCsvField,
) -> Result<u64, SourceBundleError> {
    if value.is_empty()
        || !value.iter().all(u8::is_ascii_digit)
        || (value.len() > 1 && value[0] == b'0')
    {
        return Err(csv_error(
            Some(row),
            Some(field),
            CsvFailure::InvalidUnsigned,
        ));
    }
    let mut parsed = 0_u64;
    for digit in value {
        parsed = parsed
            .checked_mul(10)
            .and_then(|number| number.checked_add(u64::from(*digit - b'0')))
            .ok_or_else(|| csv_error(Some(row), Some(field), CsvFailure::InvalidUnsigned))?;
    }
    if parsed > maximum {
        return Err(csv_error(
            Some(row),
            Some(field),
            CsvFailure::InvalidUnsigned,
        ));
    }
    Ok(parsed)
}

fn parse_decimal(value: &[u8], row: u64, field: CellVitCsvField) -> Result<f64, SourceBundleError> {
    let valid_length = !value.is_empty() && value.len() <= 32;
    let mut parts = value.split(|byte| *byte == b'.');
    let integer = parts.next().unwrap_or_default();
    let fraction = parts.next();
    let valid_integer = !integer.is_empty()
        && integer.iter().all(u8::is_ascii_digit)
        && (integer.len() == 1 || integer[0] != b'0');
    let valid_fraction =
        fraction.is_none_or(|digits| !digits.is_empty() && digits.iter().all(u8::is_ascii_digit));
    if !valid_length || !valid_integer || !valid_fraction || parts.next().is_some() {
        return Err(csv_error(
            Some(row),
            Some(field),
            CsvFailure::InvalidDecimal,
        ));
    }
    let value = text(value, row, field)?
        .parse::<f64>()
        .map_err(|_| csv_error(Some(row), Some(field), CsvFailure::InvalidDecimal))?;
    if !value.is_finite() || value < 0.0 {
        return Err(csv_error(Some(row), Some(field), CsvFailure::OutOfRange));
    }
    Ok(value)
}

fn text(value: &[u8], row: u64, field: CellVitCsvField) -> Result<&str, SourceBundleError> {
    std::str::from_utf8(value)
        .map_err(|_| csv_error(Some(row), Some(field), CsvFailure::InvalidUtf8))
}

struct Preflight {
    record_count: u64,
    content_digest: ContentDigest,
}

fn preflight(
    reader: &mut dyn ArtifactReadSeek,
    file_bytes: u64,
) -> Result<Preflight, SourceBundleError> {
    let mut parser = RecordPreflight::default();
    let mut digest = ContentDigest::builder();
    let mut remaining = file_bytes;
    let mut buffer = [0_u8; PREFLIGHT_BUFFER_BYTES];
    while remaining != 0 {
        let requested = usize::try_from(remaining.min(PREFLIGHT_BUFFER_BYTES as u64))
            .map_err(|_| SourceBundleError::SizeOverflow)?;
        let read = reader
            .read(&mut buffer[..requested])
            .map_err(|error| io_error(SourceIoOperation::Read, &error))?;
        if read == 0 {
            return Err(SourceBundleError::Io {
                operation: SourceIoOperation::Read,
                reason: SourceIoFailure::UnexpectedEnd,
            });
        }
        digest
            .write_all(&buffer[..read])
            .map_err(|_| SourceBundleError::SizeOverflow)?;
        for byte in &buffer[..read] {
            parser.push(*byte)?;
        }
        remaining = remaining
            .checked_sub(u64::try_from(read).map_err(|_| SourceBundleError::SizeOverflow)?)
            .ok_or(SourceBundleError::SizeOverflow)?;
    }
    parser.finish()?;
    let (content_digest, observed) = digest.finish();
    if observed != file_bytes {
        return Err(csv_error(None, None, CsvFailure::InvalidRecordSyntax));
    }
    Ok(Preflight {
        record_count: parser.record_count,
        content_digest,
    })
}

#[derive(Clone, Copy, Default)]
enum RecordState {
    #[default]
    StartField,
    InUnquoted,
    InQuoted,
    AfterQuote,
}

struct RecordPreflight {
    state: RecordState,
    record_count: u64,
    record_bytes: u64,
    field_count: u32,
    has_content: bool,
    pending_cr: bool,
    prefix: [u8; 3],
    total_bytes: u64,
}

impl Default for RecordPreflight {
    fn default() -> Self {
        Self {
            state: RecordState::StartField,
            record_count: 0,
            record_bytes: 0,
            field_count: 1,
            has_content: false,
            pending_cr: false,
            prefix: [0; 3],
            total_bytes: 0,
        }
    }
}

impl RecordPreflight {
    fn push(&mut self, byte: u8) -> Result<(), SourceBundleError> {
        if self.total_bytes < 3 {
            let index =
                usize::try_from(self.total_bytes).map_err(|_| SourceBundleError::SizeOverflow)?;
            self.prefix[index] = byte;
        }
        self.total_bytes = self
            .total_bytes
            .checked_add(1)
            .ok_or(SourceBundleError::SizeOverflow)?;
        if self.total_bytes == 3 && self.prefix == [0xef, 0xbb, 0xbf] {
            return Err(csv_error(None, None, CsvFailure::WrongHeader));
        }
        if self.pending_cr {
            self.pending_cr = false;
            if byte == b'\n' {
                return self.finish_record();
            }
            self.finish_record()?;
        }
        let is_record_terminator =
            !matches!(self.state, RecordState::InQuoted) && matches!(byte, b'\r' | b'\n');
        if !is_record_terminator {
            self.add_byte()?;
        }
        match (self.state, byte) {
            (RecordState::InQuoted, b'"') => self.state = RecordState::AfterQuote,
            (RecordState::InQuoted, _) => self.has_content = true,
            (RecordState::AfterQuote, b'"') => {
                self.state = RecordState::InQuoted;
                self.has_content = true;
            }
            (RecordState::AfterQuote, b',') => {
                self.add_field()?;
                self.state = RecordState::StartField;
                self.has_content = true;
            }
            (RecordState::AfterQuote, b'\r') => self.pending_cr = true,
            (RecordState::AfterQuote, b'\n') => return self.finish_record(),
            (RecordState::AfterQuote, _) => return Err(record_syntax()),
            (RecordState::StartField, b'"') => {
                self.state = RecordState::InQuoted;
                self.has_content = true;
            }
            (RecordState::StartField, b',') => {
                self.add_field()?;
                self.has_content = true;
            }
            (RecordState::StartField, b'\r') => self.pending_cr = true,
            (RecordState::StartField, b'\n') => return self.finish_record(),
            (RecordState::StartField, _) => {
                self.state = RecordState::InUnquoted;
                self.has_content = true;
            }
            (RecordState::InUnquoted, b'"') => return Err(record_syntax()),
            (RecordState::InUnquoted, b',') => {
                self.add_field()?;
                self.state = RecordState::StartField;
                self.has_content = true;
            }
            (RecordState::InUnquoted, b'\r') => self.pending_cr = true,
            (RecordState::InUnquoted, b'\n') => return self.finish_record(),
            (RecordState::InUnquoted, _) => self.has_content = true,
        }
        Ok(())
    }

    fn add_byte(&mut self) -> Result<(), SourceBundleError> {
        self.record_bytes = self
            .record_bytes
            .checked_add(1)
            .ok_or(SourceBundleError::SizeOverflow)?;
        if self.record_bytes > MAX_CSV_RECORD_BYTES {
            return Err(SourceBundleError::CsvRecordByteBudgetExceeded {
                record: self.record_count,
                observed: self.record_bytes,
                maximum: MAX_CSV_RECORD_BYTES,
            });
        }
        Ok(())
    }

    fn add_field(&mut self) -> Result<(), SourceBundleError> {
        self.field_count = self
            .field_count
            .checked_add(1)
            .ok_or(SourceBundleError::SizeOverflow)?;
        if self.field_count > HEADER.len() as u32 {
            return Err(field_count_error(self.record_count));
        }
        Ok(())
    }

    fn finish_record(&mut self) -> Result<(), SourceBundleError> {
        if matches!(self.state, RecordState::InQuoted) || !self.has_content {
            return Err(record_syntax());
        }
        if self.field_count != HEADER.len() as u32 {
            return Err(field_count_error(self.record_count));
        }
        self.record_count = self
            .record_count
            .checked_add(1)
            .ok_or(SourceBundleError::SizeOverflow)?;
        self.state = RecordState::StartField;
        self.record_bytes = 0;
        self.field_count = 1;
        self.has_content = false;
        self.pending_cr = false;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), SourceBundleError> {
        if self.pending_cr {
            self.pending_cr = false;
            self.finish_record()?;
        } else if self.record_bytes != 0 {
            self.finish_record()?;
        }
        Ok(())
    }
}

fn field_count_error(record: u64) -> SourceBundleError {
    if record == 0 {
        csv_error(None, None, CsvFailure::WrongHeader)
    } else {
        csv_error(Some(record - 1), None, CsvFailure::WrongFieldCount)
    }
}

fn enforce_retained(
    required: usize,
    budgets: SourceBundleBudgets,
) -> Result<(), SourceBundleError> {
    if required > budgets.maximum_retained_bytes() {
        return Err(SourceBundleError::RetainedByteBudgetExceeded {
            required,
            maximum: budgets.maximum_retained_bytes(),
        });
    }
    Ok(())
}

fn csv_library_error(error: ::csv::Error) -> SourceBundleError {
    match error.into_kind() {
        ::csv::ErrorKind::Io(error) => io_error(SourceIoOperation::Read, &error),
        _ => record_syntax(),
    }
}

fn io_error(operation: SourceIoOperation, error: &std::io::Error) -> SourceBundleError {
    SourceBundleError::Io {
        operation,
        reason: if error.kind() == ErrorKind::UnexpectedEof {
            SourceIoFailure::UnexpectedEnd
        } else {
            SourceIoFailure::Other
        },
    }
}

fn csv_error(
    row: Option<u64>,
    field: Option<CellVitCsvField>,
    reason: CsvFailure,
) -> SourceBundleError {
    SourceBundleError::Csv { row, field, reason }
}

fn record_syntax() -> SourceBundleError {
    csv_error(None, None, CsvFailure::InvalidRecordSyntax)
}
