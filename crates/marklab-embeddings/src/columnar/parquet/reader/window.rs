use std::io::Cursor;

use bytes::Bytes;
use parquet::{
    errors::{ParquetError, Result as ParquetResult},
    file::reader::{ChunkReader, Length},
};

#[derive(Clone)]
pub(in crate::columnar::parquet) struct RowGroupWindow {
    base: u64,
    end: u64,
    bytes: Bytes,
}

impl RowGroupWindow {
    pub(in crate::columnar::parquet) fn new(base: u64, end: u64, bytes: Bytes) -> Self {
        Self { base, end, bytes }
    }
}

impl Length for RowGroupWindow {
    fn len(&self) -> u64 {
        self.end
    }
}

impl ChunkReader for RowGroupWindow {
    type T = Cursor<Bytes>;

    fn get_read(&self, start: u64) -> ParquetResult<Self::T> {
        let relative = checked_relative(self.base, self.end, start, 0)?;
        Ok(Cursor::new(self.bytes.slice(relative..)))
    }

    fn get_bytes(&self, start: u64, length: usize) -> ParquetResult<Bytes> {
        let relative = checked_relative(self.base, self.end, start, length)?;
        let end = relative
            .checked_add(length)
            .ok_or_else(bounded_range_error)?;
        Ok(self.bytes.slice(relative..end))
    }
}

fn checked_relative(base: u64, end: u64, start: u64, length: usize) -> ParquetResult<usize> {
    let requested_end = start
        .checked_add(u64::try_from(length).map_err(|_| bounded_range_error())?)
        .ok_or_else(bounded_range_error)?;
    if start < base || requested_end > end {
        return Err(bounded_range_error());
    }
    usize::try_from(start - base).map_err(|_| bounded_range_error())
}

fn bounded_range_error() -> ParquetError {
    ParquetError::General("bounded Parquet row-group range rejected".to_owned())
}

#[cfg(test)]
mod tests {
    use std::{io::Read, str::FromStr, sync::Arc};

    use arrow::array::{Array, RecordBatchReader, StringArray};
    use marklab_data::CellId;
    use marklab_project::{ArtifactId, ContentDigest};
    use parquet::arrow::arrow_reader::{
        ArrowReaderMetadata, ArrowReaderOptions, ParquetRecordBatchReaderBuilder,
    };

    use crate::{CellEmbeddingTable, ExpectedCellSet};

    use super::super::{
        super::{
            preflight::prepare_cell_embedding_table_parquet_reader,
            profile::{embedding_schema, ROW_GROUP_ROWS},
            writer::write_cell_embedding_table_parquet,
        },
        resources::validated_group_range,
    };
    use super::*;
    use crate::columnar::{CellEmbeddingTablePhysicalBindings, EmbeddingColumnarBudgets};

    fn artifact_id(label: &[u8]) -> ArtifactId {
        ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
    }

    fn window() -> RowGroupWindow {
        RowGroupWindow {
            base: 10,
            end: 14,
            bytes: Bytes::from_static(&[1, 2, 3, 4]),
        }
    }

    #[test]
    fn row_group_window_rejects_every_out_of_range_translation() {
        let window = window();
        assert!(window.get_read(9).is_err());
        assert!(window.get_read(15).is_err());
        assert!(window.get_bytes(9, 1).is_err());
        assert!(window.get_bytes(13, 2).is_err());
        assert!(window.get_bytes(u64::MAX, 2).is_err());

        let mut at_end = window.get_read(14).expect("exact empty end");
        let mut empty = Vec::new();
        at_end.read_to_end(&mut empty).expect("read empty end");
        assert!(empty.is_empty());
        assert_eq!(
            window.get_bytes(11, 2).expect("translated bytes"),
            Bytes::from_static(&[2, 3])
        );
    }

    #[test]
    fn cached_metadata_decodes_only_the_selected_middle_row_group_window() {
        let row_count = 16_385_usize;
        let cells = (0..row_count)
            .map(|index| CellId::new(format!("cell-{index:08}")).expect("cell"))
            .collect::<Vec<_>>();
        let expected = ExpectedCellSet::new("middle-window.v1", cells).expect("expected");
        let expected_id = artifact_id(b"middle-expected");
        let provenance_id = artifact_id(b"middle-provenance");
        let row_link_id = artifact_id(b"middle-row-link");
        let row_link_digest = ContentDigest::from_bytes(b"middle-row-link-logical");
        let values = (0..row_count).map(|index| index as f32).collect();
        let table = CellEmbeddingTable::from_present_values(
            1,
            &expected,
            expected_id,
            provenance_id,
            row_link_digest,
            values,
            8 * 1024 * 1024,
        )
        .expect("table");
        let bindings = CellEmbeddingTablePhysicalBindings::new(
            expected_id,
            provenance_id,
            row_link_id,
            row_link_digest,
            table.qc_summary().logical_digest(),
        )
        .expect("bindings");
        let budgets = EmbeddingColumnarBudgets::new(
            64 * 1024 * 1024,
            128 * 1024 * 1024,
            64 * 1024 * 1024,
            64 * 1024 * 1024,
        );
        let mut bytes = Vec::new();
        write_cell_embedding_table_parquet(&mut bytes, &table, bindings, budgets)
            .expect("write Parquet");
        let mut source = Cursor::new(bytes.as_slice());
        let prepared = prepare_cell_embedding_table_parquet_reader(
            &mut source,
            u64::try_from(bytes.len()).expect("file length"),
            ContentDigest::from_bytes(&bytes),
            &expected,
            1,
            bindings,
            budgets,
        )
        .expect("prepare reader");
        assert_eq!(prepared.metadata.num_row_groups(), 3);

        let metadata = Arc::new(prepared.metadata);
        let expected_schema = embedding_schema(1).expect("schema");
        let reader_metadata = ArrowReaderMetadata::try_new(
            Arc::clone(&metadata),
            ArrowReaderOptions::new().with_schema(Arc::new(expected_schema.clone())),
        )
        .expect("cached reader metadata");
        let (start, length) = validated_group_range(metadata.row_group(1)).expect("middle range");
        let start_index = usize::try_from(start).expect("middle start");
        let end_index = start_index.checked_add(length).expect("middle end");
        let window = RowGroupWindow::new(
            start,
            start
                .checked_add(u64::try_from(length).expect("middle length"))
                .expect("absolute middle end"),
            Bytes::copy_from_slice(&bytes[start_index..end_index]),
        );
        let mut reader =
            ParquetRecordBatchReaderBuilder::new_with_metadata(window, reader_metadata)
                .with_row_groups(vec![1])
                .with_batch_size(ROW_GROUP_ROWS)
                .build()
                .expect("middle-only reader");
        assert_eq!(reader.schema().as_ref(), &expected_schema);
        let mut observed = 0_usize;
        let mut first = None;
        let mut last = None;
        for batch in &mut reader {
            let batch = batch.expect("middle batch");
            let ids = batch
                .column(0)
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("cell IDs");
            if !ids.is_empty() {
                first.get_or_insert_with(|| ids.value(0).to_owned());
                last = Some(ids.value(ids.len() - 1).to_owned());
            }
            observed = observed.checked_add(batch.num_rows()).expect("row count");
        }
        assert_eq!(observed, ROW_GROUP_ROWS);
        assert_eq!(first.as_deref(), Some("cell-00008192"));
        assert_eq!(last.as_deref(), Some("cell-00016383"));
    }
}
