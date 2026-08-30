use std::io::{Read, Seek, SeekFrom};

use parquet::format::{ColumnChunk, ColumnMetaData, CompressionCodec, Encoding};

use crate::columnar::{
    multiscale::SpatialParquetFailure,
    parquet::{
        compact::CompactLimits,
        profile::{MAXIMUM_FOOTER_BYTES, MAXIMUM_PAGE_HEADER_BYTES, PARQUET_MAGIC},
    },
    MultiscaleColumnarError,
};

pub(super) fn validate_footer_length(
    footer_len: usize,
    footer_length_offset: usize,
) -> Result<usize, MultiscaleColumnarError> {
    if footer_len == 0 || footer_len > MAXIMUM_FOOTER_BYTES {
        return Err(parquet_failure(SpatialParquetFailure::InvalidFooterLength));
    }
    let footer_start = footer_length_offset
        .checked_sub(footer_len)
        .ok_or_else(|| parquet_failure(SpatialParquetFailure::InvalidFooterLength))?;
    if footer_start < PARQUET_MAGIC.len() {
        return Err(parquet_failure(SpatialParquetFailure::InvalidFooterLength));
    }
    Ok(footer_start)
}

pub(super) fn validate_column_features(
    column: &ColumnChunk,
    metadata: &ColumnMetaData,
) -> Result<(), MultiscaleColumnarError> {
    if column.file_path.is_some()
        || column.file_offset != 0
        || column.offset_index_offset.is_some()
        || column.offset_index_length.is_some()
        || column.column_index_offset.is_some()
        || column.column_index_length.is_some()
        || column.crypto_metadata.is_some()
        || column.encrypted_column_metadata.is_some()
        || metadata.codec != CompressionCodec::UNCOMPRESSED
        || metadata.encodings != [Encoding::PLAIN, Encoding::RLE]
        || metadata.key_value_metadata.is_some()
        || metadata.index_page_offset.is_some()
        || metadata.dictionary_page_offset.is_some()
        || metadata.statistics.is_some()
        || metadata.bloom_filter_offset.is_some()
        || metadata.bloom_filter_length.is_some()
        || metadata.size_statistics.is_some()
        || metadata.geospatial_statistics.is_some()
    {
        return Err(parquet_failure(SpatialParquetFailure::ForbiddenFeature));
    }
    Ok(())
}

pub(super) fn absolute_slice(bytes: &[u8], base: usize, start: usize, end: usize) -> Option<&[u8]> {
    bytes.get(start.checked_sub(base)?..end.checked_sub(base)?)
}

pub(super) fn page_limits() -> CompactLimits {
    CompactLimits {
        maximum_depth: 8,
        maximum_fields: 64,
        maximum_collection_elements: 16,
        maximum_total_elements: 64,
        maximum_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
        maximum_total_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
    }
}

pub(super) fn read_exact_at<R: Read + Seek + ?Sized>(
    reader: &mut R,
    offset: u64,
    buffer: &mut [u8],
) -> Result<(), MultiscaleColumnarError> {
    reader
        .seek(SeekFrom::Start(offset))
        .and_then(|_| reader.read_exact(buffer))
        .map_err(|_| parquet_failure(SpatialParquetFailure::ArtifactRead))
}

pub(super) fn parquet_failure(reason: SpatialParquetFailure) -> MultiscaleColumnarError {
    MultiscaleColumnarError::Parquet { reason }
}
