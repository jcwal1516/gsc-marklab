use std::{io::Cursor, io::Write};

use arrow::{
    datatypes::Schema,
    ipc::{
        reader::FileReader,
        writer::{FileWriter, IpcWriteOptions},
        MetadataVersion,
    },
    record_batch::RecordBatch,
};
use parquet::{
    file::metadata::{ParquetMetaData, ParquetMetaDataReader},
    format::{ColumnOrder, FileMetaData, TypeDefinedOrder},
    schema::types,
    thrift::{TCompactOutputProtocol, TSerializable},
};

pub fn arrow_footer_bounds(bytes: &[u8]) -> (usize, usize) {
    let length_offset = bytes.len() - 10;
    let length = usize::try_from(i32::from_le_bytes(
        bytes[length_offset..length_offset + 4]
            .try_into()
            .expect("footer length"),
    ))
    .expect("positive footer length");
    (length_offset - length, length)
}

pub fn first_arrow_block(bytes: &[u8]) -> (usize, usize, usize) {
    let (footer_start, footer_length) = arrow_footer_bounds(bytes);
    let footer = arrow::ipc::root_as_footer(&bytes[footer_start..footer_start + footer_length])
        .expect("Arrow footer");
    let block = footer.recordBatches().expect("record batches").get(0);
    (
        usize::try_from(block.offset()).expect("block offset"),
        usize::try_from(block.metaDataLength()).expect("metadata length"),
        usize::try_from(block.bodyLength()).expect("body length"),
    )
}

pub fn arrow_node_location(bytes: &[u8], index: usize) -> usize {
    let (offset, metadata, _) = first_arrow_block(bytes);
    let message_bytes = &bytes[offset + 8..offset + metadata];
    let message = arrow::ipc::root_as_message(message_bytes).expect("record message");
    let node = message
        .header_as_record_batch()
        .expect("record batch")
        .nodes()
        .expect("nodes")
        .get(index);
    offset + 8 + (node as *const arrow::ipc::FieldNode as usize - message_bytes.as_ptr() as usize)
}

pub fn arrow_buffer_location(bytes: &[u8], index: usize) -> (usize, std::ops::Range<usize>) {
    let (offset, metadata, _) = first_arrow_block(bytes);
    let message_bytes = &bytes[offset + 8..offset + metadata];
    let message = arrow::ipc::root_as_message(message_bytes).expect("record message");
    let buffer = message
        .header_as_record_batch()
        .expect("record batch")
        .buffers()
        .expect("buffers")
        .get(index);
    let descriptor = offset
        + 8
        + (buffer as *const arrow::ipc::Buffer as usize - message_bytes.as_ptr() as usize);
    let start = offset + metadata + usize::try_from(buffer.offset()).expect("buffer offset");
    let end = start + usize::try_from(buffer.length()).expect("buffer length");
    (descriptor, start..end)
}

pub fn first_arrow_block_descriptor(bytes: &[u8]) -> usize {
    let (footer_start, footer_length) = arrow_footer_bounds(bytes);
    let footer_bytes = &bytes[footer_start..footer_start + footer_length];
    let footer = arrow::ipc::root_as_footer(footer_bytes).expect("Arrow footer");
    let block = footer.recordBatches().expect("record batches").get(0);
    footer_start + (block as *const arrow::ipc::Block as usize - footer_bytes.as_ptr() as usize)
}

pub fn swapped_arrow_columns(bytes: &[u8], left: usize, right: usize) -> Vec<u8> {
    let mut reader = FileReader::try_new(Cursor::new(bytes), None).expect("trusted Arrow reader");
    let original_schema = reader.schema();
    let original_batch = reader
        .next()
        .expect("record batch")
        .expect("trusted record batch");
    assert!(reader.next().is_none());
    let mut fields = original_schema
        .fields()
        .iter()
        .map(|field| field.as_ref().clone())
        .collect::<Vec<_>>();
    let mut columns = original_batch.columns().to_vec();
    fields.swap(left, right);
    columns.swap(left, right);
    let schema = Schema::new_with_metadata(fields, original_schema.metadata().clone());
    let batch =
        RecordBatch::try_new(std::sync::Arc::new(schema.clone()), columns).expect("swapped batch");
    let options = IpcWriteOptions::try_new(64, false, MetadataVersion::V5).expect("IPC options");
    let mut output = Vec::new();
    let mut writer =
        FileWriter::try_new_with_options(&mut output, &schema, options).expect("Arrow writer");
    writer.write(&batch).expect("swapped batch write");
    writer.finish().expect("finish swapped Arrow");
    output
}

pub fn rewrite_parquet_footer(bytes: &[u8], mutate: impl FnOnce(&mut FileMetaData)) -> Vec<u8> {
    let mut raw = raw_parquet_metadata(&trusted_parquet_metadata(bytes));
    mutate(&mut raw);
    let mut footer = Vec::new();
    {
        let mut protocol = TCompactOutputProtocol::new(&mut footer);
        raw.write_to_out_protocol(&mut protocol)
            .expect("rewrite footer");
    }
    let original_footer_start = bytes.len()
        - 8
        - u32::from_le_bytes(
            bytes[bytes.len() - 8..bytes.len() - 4]
                .try_into()
                .expect("footer length"),
        ) as usize;
    let mut output = bytes[..original_footer_start].to_vec();
    output.extend_from_slice(&footer);
    output.extend_from_slice(
        &u32::try_from(footer.len())
            .expect("footer length")
            .to_le_bytes(),
    );
    output.extend_from_slice(b"PAR1");
    output
}

pub fn trusted_parquet_metadata(bytes: &[u8]) -> ParquetMetaData {
    let mut file = tempfile::tempfile().expect("temporary Parquet file");
    file.write_all(bytes).expect("trusted Parquet bytes");
    ParquetMetaDataReader::new()
        .parse_and_finish(&file)
        .expect("trusted Parquet metadata")
}

fn raw_parquet_metadata(metadata: &ParquetMetaData) -> FileMetaData {
    let file = metadata.file_metadata();
    let columns = file.schema_descr().num_columns();
    FileMetaData {
        version: file.version(),
        schema: types::to_thrift(file.schema()).expect("Thrift schema"),
        num_rows: file.num_rows(),
        row_groups: metadata
            .row_groups()
            .iter()
            .map(|group| group.to_thrift())
            .collect(),
        key_value_metadata: file.key_value_metadata().cloned(),
        created_by: file.created_by().map(str::to_owned),
        column_orders: Some(
            (0..columns)
                .map(|_| ColumnOrder::TYPEORDER(TypeDefinedOrder {}))
                .collect(),
        ),
        encryption_algorithm: None,
        footer_signing_key_metadata: None,
    }
}
