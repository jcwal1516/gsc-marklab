use std::io::Cursor;

use marklab_project::ContentDigest;
use parquet::file::metadata::ParquetMetaData;

use crate::ExpectedCellSet;

use super::super::{
    enforce_file_budget, CellEmbeddingTablePhysicalBindings, EmbeddingColumnarBudgets,
    EmbeddingColumnarError, ParquetFailure,
};

mod file_metadata;
mod footer;
mod levels;
mod pages;
mod schema;

pub(super) use footer::{
    declared_table_logical_digest_parquet_reader, estimate_raw_preflight_bytes,
    estimate_stock_metadata_bytes, footer_compact_limits_with_metadata,
    preflight_cell_embedding_table_parquet_reader, prepare_cell_embedding_table_parquet_reader,
};
pub(super) use levels::read_level_varint;

/// Validated structural declaration for one canonical embedding Parquet file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellEmbeddingParquetPreflight {
    row_count: u64,
    dimension: u32,
    row_group_count: u32,
    encoded_byte_len: u64,
    content_digest: ContentDigest,
    pub(super) retained_preflight_bytes: usize,
}

pub(super) struct PreparedCellEmbeddingParquet {
    pub(super) summary: CellEmbeddingParquetPreflight,
    pub(super) metadata: ParquetMetaData,
}

impl CellEmbeddingParquetPreflight {
    /// Declared canonical rows.
    pub fn row_count(self) -> u64 {
        self.row_count
    }

    /// Declared embedding width supplied by the exact table manifest.
    pub fn dimension(self) -> u32 {
        self.dimension
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

/// Validate hostile borrowed Parquet bytes before any stock Parquet decoder sees them.
pub fn preflight_cell_embedding_table_parquet_bytes(
    bytes: &[u8],
    expected: &ExpectedCellSet,
    dimension: u32,
    bindings: CellEmbeddingTablePhysicalBindings,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingParquetPreflight, EmbeddingColumnarError> {
    let encoded_byte_len =
        u64::try_from(bytes.len()).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded_byte_len, budgets)?;
    preflight_cell_embedding_table_parquet_reader(
        &mut Cursor::new(bytes),
        encoded_byte_len,
        ContentDigest::from_bytes(bytes),
        expected,
        dimension,
        bindings,
        budgets,
    )
}

fn parquet_failure(reason: ParquetFailure) -> EmbeddingColumnarError {
    EmbeddingColumnarError::Parquet { reason }
}

#[cfg(test)]
mod tests;
