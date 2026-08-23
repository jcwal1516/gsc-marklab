use std::{
    fmt,
    io::{ErrorKind, SeekFrom, Write},
};

use marklab_project::{ArtifactReadSeek, ContentDigest, ContentDigestWriter};

use super::{
    NpyFailure, SourceBundleBudgets, SourceBundleError, SourceFileKind, SourceIoFailure,
    SourceIoOperation,
};

const MAGIC: &[u8; 6] = b"\x93NUMPY";
const MAX_HEADER_BYTES: u64 = 64 * 1024;
const MAX_ROWS: u64 = 100_000_000;
const MAX_DIMENSION: u64 = 65_536;
const CELLVIT_DIMENSION: u64 = 1_280;
const STREAM_BUFFER_BYTES: usize = 64 * 1024;

#[cfg(feature = "csv")]
pub(crate) const NPY_STREAM_BUFFER_BYTES: usize = STREAM_BUFFER_BYTES;

/// Accepted NPY wire versions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NpyVersion {
    /// NPY version 1.0 with a two-byte header-length field.
    V1,
    /// NPY version 2.0 with a four-byte header-length field.
    V2,
}

impl NpyVersion {
    fn prefix_length(self) -> usize {
        match self {
            Self::V1 => 10,
            Self::V2 => 12,
        }
    }
}

/// Aggregate-only facts from one fully validated raw CellViT NPY matrix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellVitNpySummary {
    version: NpyVersion,
    row_count: u64,
    dimension: u32,
    encoded_byte_len: u64,
    content_digest: ContentDigest,
}

impl CellVitNpySummary {
    /// Inspect and validate a bounded seekable reader without retaining its vector payload.
    pub fn from_reader(
        reader: &mut dyn ArtifactReadSeek,
        budgets: SourceBundleBudgets,
    ) -> Result<Self, SourceBundleError> {
        visit_reader(reader, budgets, &mut |_row, _column, _value| Ok(()))
    }

    /// Accepted NPY wire version.
    pub fn version(self) -> NpyVersion {
        self.version
    }

    /// Exact source vector row count.
    pub fn row_count(self) -> u64 {
        self.row_count
    }

    /// Frozen raw CellViT vector width.
    pub fn dimension(self) -> u32 {
        self.dimension
    }

    /// Rows with finite source vectors; every accepted source row is present.
    pub fn present_count(self) -> u64 {
        self.row_count
    }

    /// Exact encoded NPY byte length.
    pub fn encoded_byte_len(self) -> u64 {
        self.encoded_byte_len
    }

    /// Digest of the exact encoded NPY bytes.
    pub fn content_digest(self) -> ContentDigest {
        self.content_digest
    }
}

#[cfg(feature = "csv")]
pub(crate) fn reader_shape(
    reader: &mut dyn ArtifactReadSeek,
    budgets: SourceBundleBudgets,
) -> Result<(u64, u32), SourceBundleError> {
    let file_bytes = reader
        .seek(SeekFrom::End(0))
        .map_err(|error| io_error(SourceIoOperation::InspectLength, &error))?;
    enforce_file_budget(file_bytes, budgets)?;
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|error| io_error(SourceIoOperation::Seek, &error))?;
    let (layout, _encoded_header) = read_layout(reader, file_bytes, budgets)?;
    Ok((layout.rows, layout.dimension))
}

pub(crate) fn visit_reader(
    reader: &mut dyn ArtifactReadSeek,
    budgets: SourceBundleBudgets,
    visitor: &mut dyn FnMut(u64, u32, f32) -> Result<(), SourceBundleError>,
) -> Result<CellVitNpySummary, SourceBundleError> {
    let file_bytes = reader
        .seek(SeekFrom::End(0))
        .map_err(|error| io_error(SourceIoOperation::InspectLength, &error))?;
    enforce_file_budget(file_bytes, budgets)?;
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|error| io_error(SourceIoOperation::Seek, &error))?;
    let (layout, encoded_header) = read_layout(reader, file_bytes, budgets)?;
    let peak_parser_bytes = encoded_header
        .len()
        .checked_add(STREAM_BUFFER_BYTES)
        .ok_or(SourceBundleError::SizeOverflow)?;
    enforce_retained(peak_parser_bytes, budgets)?;
    let mut digest = ContentDigest::builder();
    digest
        .write_all(&encoded_header)
        .map_err(|_| SourceBundleError::SizeOverflow)?;
    validate_streamed_payload(reader, &layout, &mut digest, visitor)?;
    let (content_digest, observed) = digest.finish();
    if observed != file_bytes {
        return Err(npy_error(NpyFailure::PayloadLengthMismatch));
    }
    Ok(layout.summary(content_digest, file_bytes))
}

/// Borrowed, structurally validated, finite raw CellViT NPY matrix.
#[derive(Clone, Copy)]
pub struct CellVitNpyMatrix<'a> {
    payload: &'a [u8],
    layout: Layout,
    summary: CellVitNpySummary,
}

impl fmt::Debug for CellVitNpyMatrix<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CellVitNpyMatrix")
            .field("summary", &self.summary)
            .finish_non_exhaustive()
    }
}

impl<'a> CellVitNpyMatrix<'a> {
    /// Validate one complete borrowed NPY file without allocating its vector payload.
    pub fn from_bytes(
        bytes: &'a [u8],
        budgets: SourceBundleBudgets,
    ) -> Result<Self, SourceBundleError> {
        let file_bytes = u64::try_from(bytes.len()).map_err(|_| SourceBundleError::SizeOverflow)?;
        enforce_file_budget(file_bytes, budgets)?;
        let layout = parse_layout(bytes, file_bytes, budgets)?;
        let offset =
            usize::try_from(layout.data_offset).map_err(|_| SourceBundleError::SizeOverflow)?;
        let payload = bytes
            .get(offset..)
            .ok_or_else(|| npy_error(NpyFailure::PayloadLengthMismatch))?;
        validate_borrowed_payload(payload, &layout)?;
        let summary = layout.summary(ContentDigest::from_bytes(bytes), file_bytes);
        Ok(Self {
            payload,
            layout,
            summary,
        })
    }

    /// Exact source vector row count.
    pub fn row_count(self) -> u64 {
        self.layout.rows
    }

    /// Frozen raw CellViT vector width.
    pub fn dimension(self) -> u32 {
        self.layout.dimension
    }

    /// Aggregate-only validated matrix facts.
    pub fn summary(self) -> CellVitNpySummary {
        self.summary
    }

    /// Decode one finite little-endian component without exposing an aligned typed cast.
    pub fn value(self, row: u64, column: u32) -> Result<f32, SourceBundleError> {
        if row >= self.layout.rows || column >= self.layout.dimension {
            return Err(SourceBundleError::SourceValueOutOfBounds);
        }
        let index = row
            .checked_mul(u64::from(self.layout.dimension))
            .and_then(|value| value.checked_add(u64::from(column)))
            .and_then(|value| value.checked_mul(4))
            .ok_or(SourceBundleError::SizeOverflow)?;
        let index = usize::try_from(index).map_err(|_| SourceBundleError::SizeOverflow)?;
        let bytes: [u8; 4] = self
            .payload
            .get(index..index + 4)
            .ok_or(SourceBundleError::SourceValueOutOfBounds)?
            .try_into()
            .map_err(|_| SourceBundleError::SourceValueOutOfBounds)?;
        Ok(f32::from_bits(u32::from_le_bytes(bytes)))
    }
}

#[derive(Clone, Copy, Debug)]
struct Layout {
    version: NpyVersion,
    rows: u64,
    dimension: u32,
    data_offset: u64,
    payload_bytes: u64,
}

impl Layout {
    fn summary(self, content_digest: ContentDigest, encoded_byte_len: u64) -> CellVitNpySummary {
        CellVitNpySummary {
            version: self.version,
            row_count: self.rows,
            dimension: self.dimension,
            encoded_byte_len,
            content_digest,
        }
    }
}

fn enforce_file_budget(
    file_bytes: u64,
    budgets: SourceBundleBudgets,
) -> Result<(), SourceBundleError> {
    if file_bytes > budgets.maximum_npy_file_bytes() {
        return Err(SourceBundleError::FileByteBudgetExceeded {
            file: SourceFileKind::Npy,
            observed: file_bytes,
            maximum: budgets.maximum_npy_file_bytes(),
        });
    }
    Ok(())
}

fn parse_layout(
    bytes: &[u8],
    file_bytes: u64,
    budgets: SourceBundleBudgets,
) -> Result<Layout, SourceBundleError> {
    if bytes.get(..MAGIC.len()) != Some(MAGIC) {
        return Err(npy_error(NpyFailure::InvalidMagic));
    }
    let version = parse_version(bytes.get(6..8))?;
    let prefix_length = version.prefix_length();
    let header_length = match version {
        NpyVersion::V1 => {
            let value: [u8; 2] = bytes
                .get(8..10)
                .ok_or_else(|| npy_error(NpyFailure::InvalidHeaderLength))?
                .try_into()
                .map_err(|_| npy_error(NpyFailure::InvalidHeaderLength))?;
            u64::from(u16::from_le_bytes(value))
        }
        NpyVersion::V2 => {
            let value: [u8; 4] = bytes
                .get(8..12)
                .ok_or_else(|| npy_error(NpyFailure::InvalidHeaderLength))?
                .try_into()
                .map_err(|_| npy_error(NpyFailure::InvalidHeaderLength))?;
            u64::from(u32::from_le_bytes(value))
        }
    };
    parse_layout_parts(
        version,
        prefix_length,
        header_length,
        bytes,
        file_bytes,
        budgets,
    )
}

fn read_layout(
    reader: &mut dyn ArtifactReadSeek,
    file_bytes: u64,
    budgets: SourceBundleBudgets,
) -> Result<(Layout, Vec<u8>), SourceBundleError> {
    if file_bytes < 6 {
        return Err(npy_error(NpyFailure::InvalidMagic));
    }
    let mut initial = [0_u8; 8];
    read_exact(reader, &mut initial[..6])?;
    if initial[..6] != *MAGIC {
        return Err(npy_error(NpyFailure::InvalidMagic));
    }
    if file_bytes < 8 {
        return Err(npy_error(NpyFailure::UnsupportedVersion));
    }
    read_exact(reader, &mut initial[6..8])?;
    let version = parse_version(Some(&initial[6..8]))?;
    let length_bytes = match version {
        NpyVersion::V1 => 2,
        NpyVersion::V2 => 4,
    };
    let mut encoded_header = Vec::new();
    if file_bytes
        < u64::try_from(version.prefix_length()).map_err(|_| SourceBundleError::SizeOverflow)?
    {
        return Err(npy_error(NpyFailure::InvalidHeaderLength));
    }
    enforce_retained(version.prefix_length(), budgets)?;
    encoded_header
        .try_reserve_exact(version.prefix_length())
        .map_err(|_| SourceBundleError::AllocationFailed {
            requested: version.prefix_length(),
        })?;
    encoded_header.extend_from_slice(&initial);
    encoded_header.resize(8 + length_bytes, 0);
    read_exact(reader, &mut encoded_header[8..])?;
    let header_length = match version {
        NpyVersion::V1 => u64::from(u16::from_le_bytes(
            encoded_header[8..10]
                .try_into()
                .map_err(|_| npy_error(NpyFailure::InvalidHeaderLength))?,
        )),
        NpyVersion::V2 => u64::from(u32::from_le_bytes(
            encoded_header[8..12]
                .try_into()
                .map_err(|_| npy_error(NpyFailure::InvalidHeaderLength))?,
        )),
    };
    enforce_header_budget(header_length)?;
    let header_usize =
        usize::try_from(header_length).map_err(|_| SourceBundleError::SizeOverflow)?;
    let total = version
        .prefix_length()
        .checked_add(header_usize)
        .ok_or(SourceBundleError::SizeOverflow)?;
    enforce_retained(total, budgets)?;
    if u64::try_from(total).map_err(|_| SourceBundleError::SizeOverflow)? > file_bytes {
        return Err(npy_error(NpyFailure::InvalidHeaderLength));
    }
    encoded_header
        .try_reserve_exact(header_usize)
        .map_err(|_| SourceBundleError::AllocationFailed { requested: total })?;
    encoded_header.resize(total, 0);
    read_exact(reader, &mut encoded_header[version.prefix_length()..])?;
    let layout = parse_layout_parts(
        version,
        version.prefix_length(),
        header_length,
        &encoded_header,
        file_bytes,
        budgets,
    )?;
    Ok((layout, encoded_header))
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

fn parse_layout_parts(
    version: NpyVersion,
    prefix_length: usize,
    header_length: u64,
    encoded_header: &[u8],
    file_bytes: u64,
    budgets: SourceBundleBudgets,
) -> Result<Layout, SourceBundleError> {
    enforce_header_budget(header_length)?;
    if header_length == 0 {
        return Err(npy_error(NpyFailure::InvalidHeaderLength));
    }
    let data_offset = u64::try_from(prefix_length)
        .map_err(|_| SourceBundleError::SizeOverflow)?
        .checked_add(header_length)
        .ok_or(SourceBundleError::SizeOverflow)?;
    if data_offset % 16 != 0 {
        return Err(npy_error(NpyFailure::InvalidHeaderAlignment));
    }
    let header_start = prefix_length;
    let header_end = usize::try_from(data_offset).map_err(|_| SourceBundleError::SizeOverflow)?;
    let header = encoded_header
        .get(header_start..header_end)
        .ok_or_else(|| npy_error(NpyFailure::InvalidHeaderLength))?;
    let shape = HeaderParser::parse(header)?;
    if shape.rows > MAX_ROWS {
        return Err(npy_error(NpyFailure::RowLimitExceeded));
    }
    if shape.dimension == 0 || shape.dimension > MAX_DIMENSION {
        return Err(npy_error(NpyFailure::InvalidShape));
    }
    if shape.dimension != CELLVIT_DIMENSION {
        return Err(npy_error(NpyFailure::WrongDimension));
    }
    let payload_bytes = shape
        .rows
        .checked_mul(shape.dimension)
        .and_then(|value| value.checked_mul(4))
        .ok_or(SourceBundleError::SizeOverflow)?;
    let maximum_decoded = budgets.maximum_decoded_bytes();
    if payload_bytes > maximum_decoded {
        return Err(SourceBundleError::DecodedByteBudgetExceeded {
            required: payload_bytes,
            maximum: maximum_decoded,
        });
    }
    if data_offset.checked_add(payload_bytes) != Some(file_bytes) {
        return Err(npy_error(NpyFailure::PayloadLengthMismatch));
    }
    Ok(Layout {
        version,
        rows: shape.rows,
        dimension: u32::try_from(shape.dimension).map_err(|_| SourceBundleError::SizeOverflow)?,
        data_offset,
        payload_bytes,
    })
}

fn enforce_header_budget(header_length: u64) -> Result<(), SourceBundleError> {
    if header_length > MAX_HEADER_BYTES {
        return Err(SourceBundleError::HeaderByteBudgetExceeded {
            observed: header_length,
            maximum: MAX_HEADER_BYTES,
        });
    }
    Ok(())
}

fn parse_version(bytes: Option<&[u8]>) -> Result<NpyVersion, SourceBundleError> {
    match bytes {
        Some([1, 0]) => Ok(NpyVersion::V1),
        Some([2, 0]) => Ok(NpyVersion::V2),
        _ => Err(npy_error(NpyFailure::UnsupportedVersion)),
    }
}

fn validate_borrowed_payload(payload: &[u8], layout: &Layout) -> Result<(), SourceBundleError> {
    if u64::try_from(payload.len()).map_err(|_| SourceBundleError::SizeOverflow)?
        != layout.payload_bytes
    {
        return Err(npy_error(NpyFailure::PayloadLengthMismatch));
    }
    for (index, bytes) in payload.chunks_exact(4).enumerate() {
        let bits = u32::from_le_bytes(
            bytes
                .try_into()
                .map_err(|_| npy_error(NpyFailure::PayloadLengthMismatch))?,
        );
        validate_component(
            u64::try_from(index).map_err(|_| SourceBundleError::SizeOverflow)?,
            bits,
            layout.dimension,
        )?;
    }
    Ok(())
}

fn validate_streamed_payload(
    reader: &mut dyn ArtifactReadSeek,
    layout: &Layout,
    digest: &mut ContentDigestWriter,
    visitor: &mut dyn FnMut(u64, u32, f32) -> Result<(), SourceBundleError>,
) -> Result<(), SourceBundleError> {
    let mut remaining = layout.payload_bytes;
    let mut component_index = 0_u64;
    let mut carry = [0_u8; 4];
    let mut carry_len = 0_usize;
    let mut buffer = [0_u8; STREAM_BUFFER_BYTES];
    while remaining != 0 {
        let requested = usize::try_from(remaining.min(STREAM_BUFFER_BYTES as u64))
            .map_err(|_| SourceBundleError::SizeOverflow)?;
        let read = reader
            .read(&mut buffer[..requested])
            .map_err(|error| io_error(SourceIoOperation::Read, &error))?;
        if read == 0 {
            return Err(npy_error(NpyFailure::PayloadLengthMismatch));
        }
        digest
            .write_all(&buffer[..read])
            .map_err(|_| SourceBundleError::SizeOverflow)?;
        remaining = remaining
            .checked_sub(u64::try_from(read).map_err(|_| SourceBundleError::SizeOverflow)?)
            .ok_or(SourceBundleError::SizeOverflow)?;
        let mut offset = 0_usize;
        if carry_len != 0 {
            let needed = 4 - carry_len;
            let copied = needed.min(read);
            carry[carry_len..carry_len + copied].copy_from_slice(&buffer[..copied]);
            carry_len += copied;
            offset += copied;
            if carry_len != 4 {
                continue;
            }
            let value =
                validate_component(component_index, u32::from_le_bytes(carry), layout.dimension)?;
            visit_component(visitor, component_index, layout.dimension, value)?;
            component_index += 1;
        }
        let body = &buffer[offset..read];
        let complete_bytes = body.len() / 4 * 4;
        for bytes in body[..complete_bytes].chunks_exact(4) {
            let bits = u32::from_le_bytes(
                bytes
                    .try_into()
                    .map_err(|_| npy_error(NpyFailure::PayloadLengthMismatch))?,
            );
            let value = validate_component(component_index, bits, layout.dimension)?;
            visit_component(visitor, component_index, layout.dimension, value)?;
            component_index += 1;
        }
        let remainder = &body[complete_bytes..];
        carry[..remainder.len()].copy_from_slice(remainder);
        carry_len = remainder.len();
    }
    if carry_len != 0
        || component_index
            != layout
                .rows
                .checked_mul(u64::from(layout.dimension))
                .ok_or(SourceBundleError::SizeOverflow)?
    {
        return Err(npy_error(NpyFailure::PayloadLengthMismatch));
    }
    Ok(())
}

fn visit_component(
    visitor: &mut dyn FnMut(u64, u32, f32) -> Result<(), SourceBundleError>,
    index: u64,
    dimension: u32,
    value: f32,
) -> Result<(), SourceBundleError> {
    visitor(
        index / u64::from(dimension),
        u32::try_from(index % u64::from(dimension)).map_err(|_| SourceBundleError::SizeOverflow)?,
        value,
    )
}

fn validate_component(index: u64, bits: u32, dimension: u32) -> Result<f32, SourceBundleError> {
    let value = f32::from_bits(bits);
    if !value.is_finite() {
        return Err(SourceBundleError::NonFiniteSourceComponent {
            row: index / u64::from(dimension),
            column: u32::try_from(index % u64::from(dimension))
                .map_err(|_| SourceBundleError::SizeOverflow)?,
        });
    }
    Ok(value)
}

#[derive(Clone, Copy)]
struct Shape {
    rows: u64,
    dimension: u64,
}

struct HeaderParser<'a> {
    bytes: &'a [u8],
    offset: usize,
    seen_descr: bool,
    seen_fortran: bool,
    seen_shape: bool,
    shape: Option<Shape>,
}

impl<'a> HeaderParser<'a> {
    fn parse(header: &'a [u8]) -> Result<Shape, SourceBundleError> {
        if header.last() != Some(&b'\n') || !header.iter().all(u8::is_ascii) {
            return Err(npy_error(NpyFailure::InvalidHeaderEncoding));
        }
        let mut parser = Self {
            bytes: &header[..header.len() - 1],
            offset: 0,
            seen_descr: false,
            seen_fortran: false,
            seen_shape: false,
            shape: None,
        };
        parser.whitespace();
        parser.expect(b'{')?;
        parser.whitespace();
        if parser.peek() == Some(b'}') {
            return Err(npy_error(NpyFailure::InvalidHeaderGrammar));
        }
        loop {
            let key = parser.quoted()?;
            parser.whitespace();
            parser.expect(b':')?;
            parser.whitespace();
            match key {
                b"descr" => {
                    parser.mark_key(Key::Descr)?;
                    if parser.quoted()? != b"<f4" {
                        return Err(npy_error(NpyFailure::UnsupportedDescriptor));
                    }
                }
                b"fortran_order" => {
                    parser.mark_key(Key::Fortran)?;
                    if parser.take_keyword(b"False") {
                    } else if parser.take_keyword(b"True") {
                        return Err(npy_error(NpyFailure::FortranOrder));
                    } else {
                        return Err(npy_error(NpyFailure::InvalidHeaderGrammar));
                    }
                }
                b"shape" => {
                    parser.mark_key(Key::Shape)?;
                    parser.shape = Some(parser.shape()?);
                }
                _ => return Err(npy_error(NpyFailure::UnknownHeaderKey)),
            }
            parser.whitespace();
            match parser.peek() {
                Some(b',') => {
                    parser.offset += 1;
                    parser.whitespace();
                    if parser.peek() == Some(b'}') {
                        parser.offset += 1;
                        break;
                    }
                }
                Some(b'}') => {
                    parser.offset += 1;
                    break;
                }
                _ => return Err(npy_error(NpyFailure::InvalidHeaderGrammar)),
            }
        }
        if parser.bytes[parser.offset..]
            .iter()
            .any(|byte| *byte != b' ')
        {
            return Err(npy_error(NpyFailure::InvalidPadding));
        }
        if !parser.seen_descr || !parser.seen_fortran || !parser.seen_shape {
            return Err(npy_error(NpyFailure::InvalidHeaderGrammar));
        }
        parser
            .shape
            .ok_or_else(|| npy_error(NpyFailure::InvalidShape))
    }

    fn mark_key(&mut self, key: Key) -> Result<(), SourceBundleError> {
        let seen = match key {
            Key::Descr => &mut self.seen_descr,
            Key::Fortran => &mut self.seen_fortran,
            Key::Shape => &mut self.seen_shape,
        };
        if *seen {
            return Err(npy_error(NpyFailure::DuplicateHeaderKey));
        }
        *seen = true;
        Ok(())
    }

    fn shape(&mut self) -> Result<Shape, SourceBundleError> {
        self.expect(b'(')?;
        self.whitespace();
        let rows = self.unsigned()?;
        self.whitespace();
        self.expect(b',')?;
        self.whitespace();
        let dimension = self.unsigned()?;
        self.whitespace();
        if self.peek() == Some(b',') {
            self.offset += 1;
            self.whitespace();
        }
        self.expect(b')')?;
        Ok(Shape { rows, dimension })
    }

    fn unsigned(&mut self) -> Result<u64, SourceBundleError> {
        let start = self.offset;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.offset += 1;
        }
        let value = &self.bytes[start..self.offset];
        if value.is_empty() || (value.len() > 1 && value[0] == b'0') {
            return Err(npy_error(NpyFailure::InvalidShape));
        }
        let mut parsed = 0_u64;
        for digit in value {
            parsed = parsed
                .checked_mul(10)
                .and_then(|number| number.checked_add(u64::from(*digit - b'0')))
                .ok_or_else(|| npy_error(NpyFailure::InvalidShape))?;
        }
        Ok(parsed)
    }

    fn quoted(&mut self) -> Result<&'a [u8], SourceBundleError> {
        self.expect(b'\'')?;
        let start = self.offset;
        while let Some(byte) = self.peek() {
            if byte == b'\'' {
                let value = &self.bytes[start..self.offset];
                self.offset += 1;
                return Ok(value);
            }
            if byte == b'\\' || byte.is_ascii_control() {
                return Err(npy_error(NpyFailure::InvalidHeaderGrammar));
            }
            self.offset += 1;
        }
        Err(npy_error(NpyFailure::InvalidHeaderGrammar))
    }

    fn take_keyword(&mut self, keyword: &[u8]) -> bool {
        if self.bytes.get(self.offset..self.offset + keyword.len()) == Some(keyword) {
            self.offset += keyword.len();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expected: u8) -> Result<(), SourceBundleError> {
        if self.peek() != Some(expected) {
            return Err(npy_error(NpyFailure::InvalidHeaderGrammar));
        }
        self.offset += 1;
        Ok(())
    }

    fn whitespace(&mut self) {
        while self
            .peek()
            .is_some_and(|byte| byte.is_ascii_whitespace() && byte != b'\n')
        {
            self.offset += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.offset).copied()
    }
}

enum Key {
    Descr,
    Fortran,
    Shape,
}

fn read_exact(
    reader: &mut dyn ArtifactReadSeek,
    bytes: &mut [u8],
) -> Result<(), SourceBundleError> {
    reader
        .read_exact(bytes)
        .map_err(|error| io_error(SourceIoOperation::Read, &error))
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

fn npy_error(reason: NpyFailure) -> SourceBundleError {
    SourceBundleError::Npy { reason }
}
