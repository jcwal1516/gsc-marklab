use std::io::{Cursor, Read, Seek, SeekFrom};

use marklab_project::ContentDigest;
use parquet::{
    file::metadata::{ParquetMetaData, ParquetMetaDataReader},
    format::{
        ColumnOrder, CompressionCodec, Encoding, FieldRepetitionType, FileMetaData, PageHeader,
        PageType, SchemaElement, Type,
    },
    thrift::TSerializable,
};

use crate::columnar::parquet::schema::{
    is_flat_group as is_group, is_required_utf8 as is_utf8, is_unsigned_u64 as is_u64,
};
use crate::{CellEmbeddingRowLink, ExpectedCellSet};

use super::super::super::{
    enforce_decoded_budget, enforce_retained_budget, enforce_row_group_budget,
    EmbeddingColumnarBudgets, EmbeddingColumnarError, ParquetFailure,
};
use super::{
    super::{
        compact::{is_canonical_compact, BoundedCompactProtocol, CompactLimits},
        preflight::{
            estimate_raw_preflight_bytes, estimate_stock_metadata_bytes,
            footer_compact_limits_with_metadata, read_level_varint,
        },
        profile::{
            CREATED_BY, MAXIMUM_APPLICATION_METADATA_BYTES, MAXIMUM_FOOTER_BYTES,
            MAXIMUM_PAGE_BYTES, MAXIMUM_PAGE_HEADER_BYTES, MAXIMUM_ROWS, PARQUET_MAGIC,
            ROW_GROUP_ROWS, TRAILER_BYTES,
        },
    },
    profile::{metadata_values, METADATA_KEYS, ROOT_NAME},
    writer::estimate_decoded_bytes,
};

mod file_metadata;
mod footer;
mod levels;
mod pages;
mod schema;

pub(super) use footer::{
    preflight_cell_embedding_row_link_parquet_reader,
    prepare_cell_embedding_row_link_parquet_reader,
};
#[cfg(test)]
use pages::page_limits;

/// Validated structural declaration for one canonical row-link Parquet file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellEmbeddingRowLinkParquetPreflight {
    row_count: u64,
    row_group_count: u32,
    encoded_byte_len: u64,
    content_digest: ContentDigest,
    pub(super) retained_preflight_bytes: usize,
}

pub(super) struct PreparedCellEmbeddingRowLinkParquet {
    pub(super) summary: CellEmbeddingRowLinkParquetPreflight,
    pub(super) metadata: ParquetMetaData,
}

impl CellEmbeddingRowLinkParquetPreflight {
    /// Declared canonical rows.
    pub fn row_count(self) -> u64 {
        self.row_count
    }

    /// Number of canonical Parquet row groups.
    pub fn row_group_count(self) -> u32 {
        self.row_group_count
    }

    /// Exact encoded Parquet byte length.
    pub fn encoded_byte_len(self) -> u64 {
        self.encoded_byte_len
    }

    /// SHA-256 of the exact preflighted bytes.
    pub fn content_digest(self) -> ContentDigest {
        self.content_digest
    }
}

/// Validate hostile borrowed row-link Parquet bytes before stock decoding.
pub fn preflight_cell_embedding_row_link_parquet_bytes(
    bytes: &[u8],
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingRowLinkParquetPreflight, EmbeddingColumnarError> {
    let encoded_byte_len =
        u64::try_from(bytes.len()).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(EmbeddingColumnarError::FileByteBudgetExceeded {
            observed: encoded_byte_len,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    preflight_cell_embedding_row_link_parquet_reader(
        &mut Cursor::new(bytes),
        encoded_byte_len,
        ContentDigest::from_bytes(bytes),
        expected,
        row_link,
        budgets,
    )
}

fn parquet_failure(reason: ParquetFailure) -> EmbeddingColumnarError {
    EmbeddingColumnarError::Parquet { reason }
}

#[cfg(test)]
mod tests;
