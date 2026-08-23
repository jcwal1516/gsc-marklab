#![cfg(feature = "parquet")]

use std::{fs, process::Command, str::FromStr};

use marklab::{
    preflight_cell_embedding_table_arrow_bytes, publish_cell_embedding_table_arrow,
    write_cell_embedding_table_arrow, ArrowIpcFailure, ArtifactId, CellEmbeddingRow,
    CellEmbeddingTable, CellEmbeddingTablePhysicalBindings, CellId, ContentDigest,
    EmbeddingColumnarBudgets, EmbeddingColumnarError, EmbeddingStatus, ExpectedCellSet,
    LocalArtifactStore, PublicationDisposition, StoreId, TableFormat,
};
use tempfile::TempDir;

fn artifact_id(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

fn cell(value: &str) -> CellId {
    CellId::new(value).expect("cell ID")
}

fn fixture() -> (
    ExpectedCellSet,
    CellEmbeddingTable,
    CellEmbeddingTablePhysicalBindings,
) {
    let expected = ExpectedCellSet::new("all.v1", vec![cell("cell-a"), cell("cell-b")])
        .expect("expected cells");
    let expected_id = artifact_id(b"expected");
    let provenance_id = artifact_id(b"provenance");
    let row_link_id = artifact_id(b"row-link");
    let row_link_digest = ContentDigest::from_bytes(b"row-link-logical");
    let table = CellEmbeddingTable::from_rows(
        3,
        &expected,
        expected_id,
        provenance_id,
        row_link_digest,
        vec![
            CellEmbeddingRow::present(cell("cell-a"), vec![1.0, -0.0, 3.5]),
            CellEmbeddingRow::non_present(cell("cell-b"), EmbeddingStatus::MissingVector)
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
    (expected, table, bindings)
}

fn fixture_with_rows(
    row_count: usize,
) -> (
    ExpectedCellSet,
    CellEmbeddingTable,
    CellEmbeddingTablePhysicalBindings,
) {
    let cells = (0..row_count)
        .map(|index| cell(&format!("cell-{index:08}")))
        .collect::<Vec<_>>();
    let expected = ExpectedCellSet::new("all.v1", cells.clone()).expect("expected cells");
    let expected_id = artifact_id(b"expected-boundary");
    let provenance_id = artifact_id(b"provenance-boundary");
    let row_link_id = artifact_id(b"row-link-boundary");
    let row_link_digest = ContentDigest::from_bytes(b"row-link-logical-boundary");
    let rows = cells
        .into_iter()
        .enumerate()
        .map(|(index, cell)| match index % 4 {
            0 => CellEmbeddingRow::present(cell, vec![index as f32, -0.0, 1.0]),
            1 => CellEmbeddingRow::non_present(cell, EmbeddingStatus::MissingVector)
                .expect("missing row"),
            2 => CellEmbeddingRow::non_present(cell, EmbeddingStatus::ExtractionFailed)
                .expect("failed row"),
            _ => CellEmbeddingRow::non_present(cell, EmbeddingStatus::QcRejected)
                .expect("rejected row"),
        })
        .collect();
    let table = CellEmbeddingTable::from_rows(
        3,
        &expected,
        expected_id,
        provenance_id,
        row_link_digest,
        rows,
        16 * 1024 * 1024,
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
    (expected, table, bindings)
}

fn budgets(file_bytes: u64) -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        file_bytes,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
    )
}

fn footer_bounds(bytes: &[u8]) -> (usize, usize) {
    let length_offset = bytes.len() - 10;
    let length = i32::from_le_bytes(
        bytes[length_offset..length_offset + 4]
            .try_into()
            .expect("footer length"),
    );
    assert!(length > 0);
    let length = length as usize;
    (bytes.len() - 10 - length, length)
}

fn clear_footer_record_batches(bytes: &mut [u8]) {
    let (footer_start, footer_length) = footer_bounds(bytes);
    let footer = &mut bytes[footer_start..footer_start + footer_length];
    let root_offset = u32::from_le_bytes(footer[..4].try_into().expect("root offset")) as usize;
    let vtable_distance = i32::from_le_bytes(
        footer[root_offset..root_offset + 4]
            .try_into()
            .expect("vtable distance"),
    );
    assert!(vtable_distance > 0);
    let vtable = root_offset - vtable_distance as usize;
    let record_batches_slot = vtable + 4 + 3 * size_of::<u16>();
    footer[record_batches_slot..record_batches_slot + 2].copy_from_slice(&0_u16.to_le_bytes());
}

fn find_unique(haystack: &[u8], needle: &[u8]) -> usize {
    let matches = haystack
        .windows(needle.len())
        .enumerate()
        .filter_map(|(index, window)| (window == needle).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "expected one physical declaration");
    matches[0]
}

fn first_record_block(bytes: &[u8]) -> (usize, usize, usize) {
    let (footer_start, footer_length) = footer_bounds(bytes);
    let footer_bytes = &bytes[footer_start..footer_start + footer_length];
    let footer = arrow::ipc::root_as_footer(footer_bytes).expect("canonical footer");
    let block = footer.recordBatches().expect("record batches").get(0);
    (
        block.offset() as usize,
        block.metaDataLength() as usize,
        block.bodyLength() as usize,
    )
}

fn first_footer_block_bytes(bytes: &[u8]) -> (usize, [u8; 24]) {
    let (footer_start, footer_length) = footer_bounds(bytes);
    let footer_bytes = &bytes[footer_start..footer_start + footer_length];
    let footer = arrow::ipc::root_as_footer(footer_bytes).expect("canonical footer");
    let raw = footer.recordBatches().expect("record batches").get(0).0;
    (footer_start + find_unique(footer_bytes, &raw), raw)
}

fn first_message_declarations(bytes: &[u8]) -> (usize, [u8; 16], usize, [u8; 16]) {
    let (offset, metadata_length, _) = first_record_block(bytes);
    let message_bytes = &bytes[offset + 8..offset + metadata_length];
    let message = arrow::ipc::root_as_message(message_bytes).expect("canonical message");
    let batch = message.header_as_record_batch().expect("record batch");
    let node = batch.nodes().expect("nodes").get(2).0;
    let buffer = batch.buffers().expect("buffers").get(5).0;
    (
        offset + 8 + find_unique(message_bytes, &node),
        node,
        offset + 8 + find_unique(message_bytes, &buffer),
        buffer,
    )
}

fn replace_all_same_length(bytes: &mut [u8], from: &[u8], to: &[u8]) -> usize {
    assert_eq!(from.len(), to.len());
    let offsets = bytes
        .windows(from.len())
        .enumerate()
        .filter_map(|(index, window)| (window == from).then_some(index))
        .collect::<Vec<_>>();
    for offset in &offsets {
        bytes[*offset..*offset + to.len()].copy_from_slice(to);
    }
    offsets.len()
}

fn initial_schema_message_range(bytes: &[u8]) -> std::ops::Range<usize> {
    let start = 64;
    let declared = i32::from_le_bytes(
        bytes[start + 4..start + 8]
            .try_into()
            .expect("schema message length"),
    );
    assert!(declared > 0);
    start..start + 8 + declared as usize
}

#[test]
fn canonical_arrow_writer_is_deterministic_and_preflights_exact_schema_and_batches() {
    let (expected, table, bindings) = fixture();
    let limits = budgets(8 * 1024 * 1024);
    let mut first = Vec::new();
    let first_summary = write_cell_embedding_table_arrow(&mut first, &table, bindings, limits)
        .expect("write Arrow");
    let mut second = Vec::new();
    let second_summary = write_cell_embedding_table_arrow(&mut second, &table, bindings, limits)
        .expect("repeat Arrow");
    assert_eq!(second, first);
    assert_eq!(second_summary, first_summary);
    assert_eq!(first_summary.encoded_byte_len(), first.len() as u64);
    assert_eq!(
        first_summary.content_digest(),
        ContentDigest::from_bytes(&first)
    );
    assert_eq!(
        first_summary.content_digest().to_string(),
        "44e1326b36546c793bb768f67afae9d6f5a6728ed13d47caece7514294b63570"
    );

    let preflight = preflight_cell_embedding_table_arrow_bytes(&first, &expected, bindings, limits)
        .expect("preflight Arrow");
    assert_eq!(preflight.row_count(), 2);
    assert_eq!(preflight.dimension(), 3);
    assert_eq!(preflight.record_batch_count(), 1);
    assert_eq!(preflight.encoded_byte_len(), first.len() as u64);
}

#[test]
fn canonical_arrow_publisher_creates_only_the_managed_record_and_is_idempotent() {
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("embedding-store").expect("store ID"),
    )
    .expect("store");
    let (_, table, bindings) = fixture();
    let limits = budgets(8 * 1024 * 1024);

    let first = publish_cell_embedding_table_arrow(&store, &table, bindings, limits)
        .expect("publish Arrow");
    assert_eq!(first.disposition(), PublicationDisposition::Created);
    assert_eq!(first.record().locations().len(), 1);
    assert_eq!(first.record().locations()[0].store_id(), store.store_id());
    assert_eq!(first.record().schema().id(), "marklab.cell_embedding_table");
    assert_eq!(
        first.record().content().kind(),
        "application/vnd.marklab.cell-embedding-table.v1+arrow"
    );
    assert_eq!(
        first.record().table().expect("table manifest").format(),
        TableFormat::ArrowIpcFile
    );
    store
        .verify(first.record())
        .expect("verify published Arrow");

    let second =
        publish_cell_embedding_table_arrow(&store, &table, bindings, limits).expect("reuse Arrow");
    assert_eq!(second.disposition(), PublicationDisposition::AlreadyPresent);
    assert_eq!(second.record(), first.record());
}

#[test]
fn arrow_preflight_rejects_magic_footer_and_file_budget_before_stock_decode() {
    let (expected, table, bindings) = fixture();
    let limits = budgets(8 * 1024 * 1024);
    let mut bytes = Vec::new();
    write_cell_embedding_table_arrow(&mut bytes, &table, bindings, limits).expect("write Arrow");

    let mut bad_magic = bytes.clone();
    bad_magic[0] = 0;
    assert!(matches!(
        preflight_cell_embedding_table_arrow_bytes(&bad_magic, &expected, bindings, limits),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidMagic,
        })
    ));

    let mut negative_footer = bytes.clone();
    let footer_length = negative_footer.len() - 10;
    negative_footer[footer_length..footer_length + 4].copy_from_slice(&(-1_i32).to_le_bytes());
    assert!(matches!(
        preflight_cell_embedding_table_arrow_bytes(&negative_footer, &expected, bindings, limits),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidFooterLength,
        })
    ));

    assert!(matches!(
        preflight_cell_embedding_table_arrow_bytes(
            &bytes,
            &expected,
            bindings,
            budgets(bytes.len() as u64 - 1)
        ),
        Err(EmbeddingColumnarError::FileByteBudgetExceeded { .. })
    ));
}

#[test]
fn canonical_arrow_writer_uses_exact_empty_and_8192_row_batch_boundaries() {
    for (row_count, expected_batches) in [(0, 0), (1, 1), (8_191, 1), (8_192, 1), (8_193, 2)] {
        let (expected, table, bindings) = fixture_with_rows(row_count);
        let limits = budgets(16 * 1024 * 1024);
        let mut bytes = Vec::new();
        write_cell_embedding_table_arrow(&mut bytes, &table, bindings, limits)
            .expect("write boundary Arrow");
        let preflight =
            preflight_cell_embedding_table_arrow_bytes(&bytes, &expected, bindings, limits)
                .expect("preflight boundary Arrow");
        assert_eq!(preflight.row_count(), row_count as u64);
        assert_eq!(preflight.record_batch_count(), expected_batches);
        assert_eq!(preflight.dimension(), 3);
    }
}

#[test]
fn arrow_preflight_rejects_absent_footer_record_batch_vector_for_empty_table() {
    let (expected, table, bindings) = fixture_with_rows(0);
    let limits = budgets(8 * 1024 * 1024);
    let mut bytes = Vec::new();
    write_cell_embedding_table_arrow(&mut bytes, &table, bindings, limits)
        .expect("write empty Arrow");
    clear_footer_record_batches(&mut bytes);
    assert!(matches!(
        preflight_cell_embedding_table_arrow_bytes(&bytes, &expected, bindings, limits),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidFooter,
        })
    ));
}

#[test]
fn arrow_preflight_rejects_block_node_buffer_and_metadata_drift() {
    let (expected, table, bindings) = fixture();
    let limits = budgets(8 * 1024 * 1024);
    let mut canonical = Vec::new();
    write_cell_embedding_table_arrow(&mut canonical, &table, bindings, limits)
        .expect("write Arrow");

    let (block_offset, block_raw) = first_footer_block_bytes(&canonical);
    let mut negative_block = canonical.clone();
    negative_block[block_offset..block_offset + 8].copy_from_slice(&(-1_i64).to_le_bytes());
    assert!(matches!(
        preflight_cell_embedding_table_arrow_bytes(&negative_block, &expected, bindings, limits),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidBlock,
        })
    ));

    let mut misaligned_block = canonical.clone();
    let original_offset = i64::from_le_bytes(block_raw[..8].try_into().expect("block offset"));
    misaligned_block[block_offset..block_offset + 8]
        .copy_from_slice(&(original_offset + 1).to_le_bytes());
    assert!(matches!(
        preflight_cell_embedding_table_arrow_bytes(&misaligned_block, &expected, bindings, limits),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidBlock,
        })
    ));

    let (node_offset, _, buffer_offset, buffer_raw) = first_message_declarations(&canonical);
    let mut wrong_node = canonical.clone();
    wrong_node[node_offset..node_offset + 8].copy_from_slice(&0_i64.to_le_bytes());
    assert!(matches!(
        preflight_cell_embedding_table_arrow_bytes(&wrong_node, &expected, bindings, limits),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidFieldNodes,
        })
    ));

    let mut misaligned_buffer = canonical.clone();
    let original_buffer_offset =
        i64::from_le_bytes(buffer_raw[..8].try_into().expect("buffer offset"));
    misaligned_buffer[buffer_offset..buffer_offset + 8]
        .copy_from_slice(&(original_buffer_offset + 1).to_le_bytes());
    assert!(matches!(
        preflight_cell_embedding_table_arrow_bytes(&misaligned_buffer, &expected, bindings, limits),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidBuffers,
        })
    ));

    let mut aligned_out_of_range_buffer = canonical.clone();
    aligned_out_of_range_buffer[buffer_offset..buffer_offset + 8]
        .copy_from_slice(&(original_buffer_offset + 64).to_le_bytes());
    assert!(matches!(
        preflight_cell_embedding_table_arrow_bytes(
            &aligned_out_of_range_buffer,
            &expected,
            bindings,
            limits,
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidBuffers,
        })
    ));

    let mut giant_buffer = canonical.clone();
    giant_buffer[buffer_offset + 8..buffer_offset + 16].copy_from_slice(&i64::MAX.to_le_bytes());
    assert!(matches!(
        preflight_cell_embedding_table_arrow_bytes(&giant_buffer, &expected, bindings, limits,),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidBuffers,
        })
    ));

    let mut metadata_drift = canonical;
    assert_eq!(
        replace_all_same_length(
            &mut metadata_drift,
            b"marklab.cell_embedding_table",
            b"marklab.cell_embedding_tablE",
        ),
        2
    );
    let error =
        preflight_cell_embedding_table_arrow_bytes(&metadata_drift, &expected, bindings, limits)
            .expect_err("metadata drift");
    assert!(matches!(
        error,
        EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidApplicationMetadata,
        }
    ));
    assert!(!error.to_string().contains("cell_embedding_tablE"));
}

#[test]
fn arrow_preflight_rejects_initial_schema_footer_disagreement() {
    let (expected, table, bindings) = fixture();
    let limits = budgets(8 * 1024 * 1024);
    let mut bytes = Vec::new();
    write_cell_embedding_table_arrow(&mut bytes, &table, bindings, limits).expect("write Arrow");
    let schema_range = initial_schema_message_range(&bytes);
    assert_eq!(
        replace_all_same_length(
            &mut bytes[schema_range],
            b"embedding_status",
            b"embedding_statuS",
        ),
        1
    );
    assert!(matches!(
        preflight_cell_embedding_table_arrow_bytes(&bytes, &expected, bindings, limits),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidSchema,
        })
    ));
}

#[test]
fn arrow_preflight_enforces_row_group_decoded_and_retained_budget_edges() {
    let (expected, table, bindings) = fixture();
    let generous = budgets(8 * 1024 * 1024);
    let mut bytes = Vec::new();
    write_cell_embedding_table_arrow(&mut bytes, &table, bindings, generous).expect("write Arrow");

    let row_group_required = match preflight_cell_embedding_table_arrow_bytes(
        &bytes,
        &expected,
        bindings,
        EmbeddingColumnarBudgets::new(
            generous.maximum_file_bytes(),
            generous.maximum_retained_bytes(),
            0,
            generous.maximum_decoded_bytes(),
        ),
    ) {
        Err(EmbeddingColumnarError::RowGroupByteBudgetExceeded {
            required,
            maximum: 0,
        }) => required,
        result => panic!("expected row-group failure, observed {result:?}"),
    };
    let row_group_exact = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        generous.maximum_retained_bytes(),
        row_group_required,
        generous.maximum_decoded_bytes(),
    );
    preflight_cell_embedding_table_arrow_bytes(&bytes, &expected, bindings, row_group_exact)
        .expect("exact row-group budget");
    assert!(matches!(
        preflight_cell_embedding_table_arrow_bytes(
            &bytes,
            &expected,
            bindings,
            EmbeddingColumnarBudgets::new(
                generous.maximum_file_bytes(),
                generous.maximum_retained_bytes(),
                row_group_required - 1,
                generous.maximum_decoded_bytes(),
            ),
        ),
        Err(EmbeddingColumnarError::RowGroupByteBudgetExceeded { .. })
    ));

    let component_bytes = table.row_count() as u64 * u64::from(table.dimension()) * 4;
    let decoded_required = match preflight_cell_embedding_table_arrow_bytes(
        &bytes,
        &expected,
        bindings,
        EmbeddingColumnarBudgets::new(
            generous.maximum_file_bytes(),
            generous.maximum_retained_bytes(),
            generous.maximum_row_group_bytes(),
            component_bytes,
        ),
    ) {
        Err(EmbeddingColumnarError::DecodedByteBudgetExceeded { required, .. }) => required,
        result => panic!("expected decoded failure, observed {result:?}"),
    };
    let decoded_exact = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        generous.maximum_retained_bytes(),
        generous.maximum_row_group_bytes(),
        decoded_required,
    );
    preflight_cell_embedding_table_arrow_bytes(&bytes, &expected, bindings, decoded_exact)
        .expect("exact decoded budget");

    let footer_retained_required = match preflight_cell_embedding_table_arrow_bytes(
        &bytes,
        &expected,
        bindings,
        EmbeddingColumnarBudgets::new(
            generous.maximum_file_bytes(),
            0,
            generous.maximum_row_group_bytes(),
            generous.maximum_decoded_bytes(),
        ),
    ) {
        Err(EmbeddingColumnarError::RetainedByteBudgetExceeded {
            required,
            maximum: 0,
        }) => required,
        result => panic!("expected retained failure, observed {result:?}"),
    };
    let retained_required = match preflight_cell_embedding_table_arrow_bytes(
        &bytes,
        &expected,
        bindings,
        EmbeddingColumnarBudgets::new(
            generous.maximum_file_bytes(),
            footer_retained_required,
            generous.maximum_row_group_bytes(),
            generous.maximum_decoded_bytes(),
        ),
    ) {
        Err(EmbeddingColumnarError::RetainedByteBudgetExceeded { required, maximum })
            if maximum == footer_retained_required && required > maximum =>
        {
            required
        }
        result => panic!("expected record-batch retained failure, observed {result:?}"),
    };
    let retained_exact = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        retained_required,
        generous.maximum_row_group_bytes(),
        generous.maximum_decoded_bytes(),
    );
    preflight_cell_embedding_table_arrow_bytes(&bytes, &expected, bindings, retained_exact)
        .expect("exact retained budget");
    assert!(matches!(
        preflight_cell_embedding_table_arrow_bytes(
            &bytes,
            &expected,
            bindings,
            EmbeddingColumnarBudgets::new(
                generous.maximum_file_bytes(),
                retained_required - 1,
                generous.maximum_row_group_bytes(),
                generous.maximum_decoded_bytes(),
            ),
        ),
        Err(EmbeddingColumnarError::RetainedByteBudgetExceeded { .. })
    ));
}

#[test]
fn canonical_arrow_writer_and_preflight_enforce_exact_budget_edges() {
    let (expected, table, bindings) = fixture();
    let generous = budgets(8 * 1024 * 1024);
    let mut bytes = Vec::new();
    write_cell_embedding_table_arrow(&mut bytes, &table, bindings, generous).expect("write Arrow");

    let exact_file = EmbeddingColumnarBudgets::new(
        bytes.len() as u64,
        generous.maximum_retained_bytes(),
        generous.maximum_row_group_bytes(),
        generous.maximum_decoded_bytes(),
    );
    let mut exact_output = Vec::new();
    write_cell_embedding_table_arrow(&mut exact_output, &table, bindings, exact_file)
        .expect("exact writer file budget");
    preflight_cell_embedding_table_arrow_bytes(&bytes, &expected, bindings, exact_file)
        .expect("exact reader file budget");

    let too_small = EmbeddingColumnarBudgets::new(
        bytes.len() as u64 - 1,
        generous.maximum_retained_bytes(),
        generous.maximum_row_group_bytes(),
        generous.maximum_decoded_bytes(),
    );
    assert!(matches!(
        write_cell_embedding_table_arrow(&mut Vec::new(), &table, bindings, too_small),
        Err(EmbeddingColumnarError::FileByteBudgetExceeded { .. })
    ));
    assert!(matches!(
        preflight_cell_embedding_table_arrow_bytes(&bytes, &expected, bindings, too_small),
        Err(EmbeddingColumnarError::FileByteBudgetExceeded { .. })
    ));
}

#[test]
fn canonical_arrow_writer_charges_every_decoded_buffer_at_exact_edge() {
    let (expected, table, bindings) = fixture();
    let generous = budgets(8 * 1024 * 1024);
    let mut bytes = Vec::new();
    write_cell_embedding_table_arrow(&mut bytes, &table, bindings, generous).expect("write Arrow");

    let component_bytes = table.row_count() as u64 * u64::from(table.dimension()) * 4;
    let component_only = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        generous.maximum_retained_bytes(),
        generous.maximum_row_group_bytes(),
        component_bytes,
    );
    let decoded_required = match preflight_cell_embedding_table_arrow_bytes(
        &bytes,
        &expected,
        bindings,
        component_only,
    ) {
        Err(EmbeddingColumnarError::DecodedByteBudgetExceeded { required, .. }) => required,
        result => panic!("expected preflight decoded failure, observed {result:?}"),
    };
    assert!(decoded_required > component_bytes);
    assert!(matches!(
        write_cell_embedding_table_arrow(&mut Vec::new(), &table, bindings, component_only),
        Err(EmbeddingColumnarError::DecodedByteBudgetExceeded {
            required,
            maximum,
        }) if required == decoded_required && maximum == component_bytes
    ));

    let exact = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        generous.maximum_retained_bytes(),
        generous.maximum_row_group_bytes(),
        decoded_required,
    );
    write_cell_embedding_table_arrow(&mut Vec::new(), &table, bindings, exact)
        .expect("exact writer decoded budget");
    preflight_cell_embedding_table_arrow_bytes(&bytes, &expected, bindings, exact)
        .expect("exact preflight decoded budget");
}

#[test]
fn canonical_arrow_writer_charges_many_batch_index_and_footer_peak_at_exact_edge() {
    let (_, table, bindings) = fixture_with_rows(100_000);
    let zero_retained =
        EmbeddingColumnarBudgets::new(64 * 1024 * 1024, 0, 64 * 1024 * 1024, 64 * 1024 * 1024);
    let required =
        match write_cell_embedding_table_arrow(&mut Vec::new(), &table, bindings, zero_retained) {
            Err(EmbeddingColumnarError::RetainedByteBudgetExceeded {
                required,
                maximum: 0,
            }) => required,
            result => panic!("expected retained budget failure, observed {result:?}"),
        };

    let exact = EmbeddingColumnarBudgets::new(
        64 * 1024 * 1024,
        required,
        64 * 1024 * 1024,
        64 * 1024 * 1024,
    );
    write_cell_embedding_table_arrow(&mut Vec::new(), &table, bindings, exact)
        .expect("exact many-batch retained budget");
    let one_short = EmbeddingColumnarBudgets::new(
        64 * 1024 * 1024,
        required - 1,
        64 * 1024 * 1024,
        64 * 1024 * 1024,
    );
    assert!(matches!(
        write_cell_embedding_table_arrow(&mut Vec::new(), &table, bindings, one_short),
        Err(EmbeddingColumnarError::RetainedByteBudgetExceeded { .. })
    ));
}

#[test]
fn arrow_preflight_rejects_noncanonical_header_padding_and_eos() {
    let (expected, table, bindings) = fixture();
    let limits = budgets(8 * 1024 * 1024);
    let mut bytes = Vec::new();
    write_cell_embedding_table_arrow(&mut bytes, &table, bindings, limits).expect("write Arrow");

    let mut bad_padding = bytes.clone();
    bad_padding[6] = 1;
    assert!(matches!(
        preflight_cell_embedding_table_arrow_bytes(&bad_padding, &expected, bindings, limits),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidMagic,
        })
    ));

    let footer_length_offset = bytes.len() - 10;
    let footer_length = i32::from_le_bytes(
        bytes[footer_length_offset..footer_length_offset + 4]
            .try_into()
            .expect("footer length"),
    ) as usize;
    let footer_start = bytes.len() - 10 - footer_length;
    let mut bad_eos = bytes;
    bad_eos[footer_start - 1] = 1;
    assert!(matches!(
        preflight_cell_embedding_table_arrow_bytes(&bad_eos, &expected, bindings, limits),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidFooterLength,
        })
    ));
}

#[test]
fn canonical_arrow_writer_is_deterministic_across_fresh_processes() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let executable = std::env::current_exe().expect("current test executable");
    let first_path = temporary.path().join("first.arrow");
    let second_path = temporary.path().join("second.arrow");
    for path in [&first_path, &second_path] {
        let status = Command::new(&executable)
            .arg("--exact")
            .arg("canonical_arrow_writer_child")
            .env("MARKLAB_ARROW_CHILD_OUTPUT", path)
            .status()
            .expect("run fresh writer process");
        assert!(status.success());
    }
    let first = fs::read(first_path).expect("first child output");
    let second = fs::read(second_path).expect("second child output");
    assert_eq!(first, second);
    assert_eq!(
        ContentDigest::from_bytes(&first).to_string(),
        "44e1326b36546c793bb768f67afae9d6f5a6728ed13d47caece7514294b63570"
    );
}

#[test]
fn canonical_arrow_writer_child() {
    let Some(path) = std::env::var_os("MARKLAB_ARROW_CHILD_OUTPUT") else {
        return;
    };
    let (_, table, bindings) = fixture();
    let mut bytes = Vec::new();
    write_cell_embedding_table_arrow(&mut bytes, &table, bindings, budgets(8 * 1024 * 1024))
        .expect("child Arrow write");
    fs::write(path, bytes).expect("write child output");
}
