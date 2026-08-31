use std::str::FromStr;

use marklab_data::CellId;
use marklab_project::{ArtifactId, ContentDigest};
use parquet::{
    format::{FileMetaData, PageHeader},
    thrift::TSerializable,
};
use thrift::protocol::TCompactOutputProtocol;

use crate::{CellEmbeddingRow, CellEmbeddingTable, EmbeddingStatus};

use super::super::{
    compact::{BoundedCompactProtocol, CompactLimits},
    profile::{
        MAXIMUM_APPLICATION_METADATA_BYTES, MAXIMUM_FOOTER_BYTES, MAXIMUM_PAGE_HEADER_BYTES,
        PARQUET_MAGIC, TRAILER_BYTES,
    },
    writer::write_cell_embedding_table_parquet,
};
use super::*;

fn artifact_id(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

fn fixture() -> (
    ExpectedCellSet,
    CellEmbeddingTable,
    CellEmbeddingTablePhysicalBindings,
    EmbeddingColumnarBudgets,
    Vec<u8>,
) {
    let cells = vec![
        CellId::new("cell-a").expect("cell"),
        CellId::new("cell-b").expect("cell"),
    ];
    let expected = ExpectedCellSet::new("levels.v1", cells.clone()).expect("expected");
    let expected_id = artifact_id(b"levels-expected");
    let provenance_id = artifact_id(b"levels-provenance");
    let row_link_id = artifact_id(b"levels-row-link");
    let row_link_digest = ContentDigest::from_bytes(b"levels-row-link-logical");
    let table = CellEmbeddingTable::from_rows(
        3,
        &expected,
        expected_id,
        provenance_id,
        row_link_digest,
        vec![
            CellEmbeddingRow::present(cells[0].clone(), vec![1.0, 2.0, 3.0]),
            CellEmbeddingRow::non_present(cells[1].clone(), EmbeddingStatus::MissingVector)
                .expect("missing row"),
        ],
        1024 * 1024,
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
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
    );
    let mut bytes = Vec::new();
    write_cell_embedding_table_parquet(&mut bytes, &table, bindings, budgets)
        .expect("write Parquet");
    (expected, table, bindings, budgets, bytes)
}

fn first_embedding_level_range(bytes: &[u8]) -> std::ops::Range<usize> {
    let footer_length_offset = bytes.len() - TRAILER_BYTES;
    let footer_length = u32::from_le_bytes(
        bytes[footer_length_offset..footer_length_offset + 4]
            .try_into()
            .expect("footer length"),
    ) as usize;
    let footer_start = footer_length_offset - footer_length;
    let mut footer_protocol = BoundedCompactProtocol::new(
        &bytes[footer_start..footer_length_offset],
        CompactLimits {
            maximum_depth: 16,
            maximum_fields: 1024,
            maximum_collection_elements: 64,
            maximum_total_elements: 1024,
            maximum_string_bytes: MAXIMUM_APPLICATION_METADATA_BYTES,
            maximum_total_string_bytes: MAXIMUM_APPLICATION_METADATA_BYTES,
        },
    );
    let metadata =
        FileMetaData::read_from_in_protocol(&mut footer_protocol).expect("footer metadata");
    let column = metadata.row_groups[0].columns[1]
        .meta_data
        .as_ref()
        .expect("column metadata");
    let page_start = usize::try_from(column.data_page_offset).expect("page offset");
    let mut page_protocol = BoundedCompactProtocol::new(
        &bytes[page_start..],
        CompactLimits {
            maximum_depth: 8,
            maximum_fields: 64,
            maximum_collection_elements: 16,
            maximum_total_elements: 64,
            maximum_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
            maximum_total_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
        },
    );
    let header = PageHeader::read_from_in_protocol(&mut page_protocol).expect("page header");
    let header_bytes = page_protocol.consumed_bytes().expect("header length");
    let page = header.data_page_header_v2.expect("data page v2");
    let repetition_bytes =
        usize::try_from(page.repetition_levels_byte_length).expect("repetition bytes");
    assert!(repetition_bytes > 0);
    let body_start = page_start + header_bytes;
    body_start..body_start + repetition_bytes
}

fn raw_footer(bytes: &[u8]) -> (usize, FileMetaData) {
    let length_offset = bytes.len() - TRAILER_BYTES;
    let length = u32::from_le_bytes(
        bytes[length_offset..length_offset + 4]
            .try_into()
            .expect("footer length"),
    ) as usize;
    let start = length_offset - length;
    let mut protocol = BoundedCompactProtocol::new(
        &bytes[start..length_offset],
        CompactLimits {
            maximum_depth: 16,
            maximum_fields: 4096,
            maximum_collection_elements: 128,
            maximum_total_elements: 4096,
            maximum_string_bytes: MAXIMUM_APPLICATION_METADATA_BYTES,
            maximum_total_string_bytes: MAXIMUM_FOOTER_BYTES,
        },
    );
    let metadata = FileMetaData::read_from_in_protocol(&mut protocol).expect("footer metadata");
    (start, metadata)
}

fn first_page(bytes: &[u8], column_index: usize) -> (usize, usize, PageHeader) {
    let (_, metadata) = raw_footer(bytes);
    let page_start = usize::try_from(
        metadata.row_groups[0].columns[column_index]
            .meta_data
            .as_ref()
            .expect("column")
            .data_page_offset,
    )
    .expect("page start");
    let mut protocol = BoundedCompactProtocol::new(
        &bytes[page_start..],
        CompactLimits {
            maximum_depth: 8,
            maximum_fields: 64,
            maximum_collection_elements: 16,
            maximum_total_elements: 64,
            maximum_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
            maximum_total_string_bytes: MAXIMUM_PAGE_HEADER_BYTES,
        },
    );
    let header = PageHeader::read_from_in_protocol(&mut protocol).expect("page header");
    (
        page_start,
        protocol.consumed_bytes().expect("header bytes"),
        header,
    )
}

fn replace_footer(bytes: &[u8], metadata: &FileMetaData) -> Vec<u8> {
    let (footer_start, _) = raw_footer(bytes);
    let mut footer = Vec::new();
    let mut protocol = TCompactOutputProtocol::new(&mut footer);
    metadata
        .write_to_out_protocol(&mut protocol)
        .expect("serialize footer");
    let mut rewritten = bytes[..footer_start].to_vec();
    rewritten.extend_from_slice(&footer);
    rewritten.extend_from_slice(&(footer.len() as u32).to_le_bytes());
    rewritten.extend_from_slice(PARQUET_MAGIC);
    rewritten
}

#[test]
fn preflight_rejects_corrupted_fixed_list_level_streams() {
    let (expected, table, bindings, budgets, mut bytes) = fixture();
    let levels = first_embedding_level_range(&bytes);
    let last = levels.end - 1;
    bytes[last] ^= 1;
    assert!(matches!(
        preflight_cell_embedding_table_parquet_bytes(
            &bytes,
            &expected,
            table.dimension(),
            bindings,
            budgets,
        ),
        Err(EmbeddingColumnarError::Parquet {
            reason: ParquetFailure::InvalidPage,
        })
    ));
}

#[test]
fn preflight_rejects_schema_metadata_auxiliary_and_chunk_range_drift() {
    let (expected, table, bindings, budgets, canonical) = fixture();
    let assert_rejected = |bytes: &[u8], reason| {
        assert!(matches!(
            preflight_cell_embedding_table_parquet_bytes(
                bytes,
                &expected,
                table.dimension(),
                bindings,
                budgets,
            ),
            Err(EmbeddingColumnarError::Parquet { reason: observed }) if observed == reason
        ));
    };

    let (_, mut wrong_schema) = raw_footer(&canonical);
    wrong_schema.schema[0].name = "wrong_root".to_owned();
    assert_rejected(
        &replace_footer(&canonical, &wrong_schema),
        ParquetFailure::InvalidSchema,
    );

    let (_, mut wrong_metadata) = raw_footer(&canonical);
    wrong_metadata
        .key_value_metadata
        .as_mut()
        .expect("metadata")[0]
        .key = "marklab.invalid_metadata".to_owned();
    assert_rejected(
        &replace_footer(&canonical, &wrong_metadata),
        ParquetFailure::InvalidMetadata,
    );

    let (_, mut auxiliary) = raw_footer(&canonical);
    auxiliary.row_groups[0].columns[0]
        .meta_data
        .as_mut()
        .expect("column")
        .dictionary_page_offset = Some(4);
    assert_rejected(
        &replace_footer(&canonical, &auxiliary),
        ParquetFailure::ForbiddenAuxiliaryData,
    );

    let (_, mut gap) = raw_footer(&canonical);
    gap.row_groups[0].columns[1]
        .meta_data
        .as_mut()
        .expect("column")
        .data_page_offset += 1;
    assert_rejected(
        &replace_footer(&canonical, &gap),
        ParquetFailure::InvalidColumnChunk,
    );
}

#[test]
fn preflight_rejects_hostile_plain_lengths_and_page_declarations() {
    let (expected, table, bindings, budgets, canonical) = fixture();
    let (page_start, header_bytes, header) = first_page(&canonical, 0);
    let body_start = page_start + header_bytes;

    let mut bad_length = canonical.clone();
    bad_length[body_start..body_start + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        preflight_cell_embedding_table_parquet_bytes(
            &bad_length,
            &expected,
            table.dimension(),
            bindings,
            budgets,
        ),
        Err(EmbeddingColumnarError::Parquet {
            reason: ParquetFailure::InvalidPage,
        })
    ));

    let mut wrong_values = header;
    wrong_values
        .data_page_header_v2
        .as_mut()
        .expect("data page")
        .num_values -= 1;
    let mut encoded_header = Vec::new();
    let mut protocol = TCompactOutputProtocol::new(&mut encoded_header);
    wrong_values
        .write_to_out_protocol(&mut protocol)
        .expect("serialize page header");
    assert_eq!(encoded_header.len(), header_bytes);
    let mut wrong_values_bytes = canonical;
    wrong_values_bytes[page_start..body_start].copy_from_slice(&encoded_header);
    assert!(matches!(
        preflight_cell_embedding_table_parquet_bytes(
            &wrong_values_bytes,
            &expected,
            table.dimension(),
            bindings,
            budgets,
        ),
        Err(EmbeddingColumnarError::Parquet {
            reason: ParquetFailure::InvalidPage,
        })
    ));
}
