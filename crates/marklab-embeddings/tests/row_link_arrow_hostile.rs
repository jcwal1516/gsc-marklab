#![cfg(feature = "parquet")]

use std::{mem::size_of, str::FromStr};

use arrow_ipc::{
    BodyCompression, BodyCompressionArgs, KeyValue, KeyValueArgs, Message, MessageArgs,
    MessageHeader, RecordBatch, RecordBatchArgs,
};
use flatbuffers::FlatBufferBuilder;
use marklab_data::{
    CellId, CohortHierarchy, HierarchyId, HierarchyNode, PatientId, ReplicationRole, SlideId,
};
use marklab_embeddings::{
    preflight_cell_embedding_row_link_arrow_bytes, write_cell_embedding_row_link_arrow,
    ArrowIpcFailure, CellEmbeddingRowLink, CellEmbeddingRowLinkEntry, EmbeddingColumnarBudgets,
    EmbeddingColumnarError, ExpectedCellSet,
};
use marklab_project::{ArtifactId, ContentDigest};

fn artifact_id(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

fn cell(value: &str) -> CellId {
    CellId::new(value).expect("cell")
}

fn fixture() -> (
    ExpectedCellSet,
    CellEmbeddingRowLink,
    EmbeddingColumnarBudgets,
) {
    let cells = vec![cell("cell-a"), cell("cell-b"), cell("cell-c")];
    let expected = ExpectedCellSet::new("hostile-message.v1", cells.clone()).expect("expected");
    let patient = HierarchyId::from(PatientId::new("patient").expect("patient"));
    let slide = HierarchyId::from(SlideId::new("slide").expect("slide"));
    let mut nodes = vec![
        HierarchyNode::new(patient.clone(), None, ReplicationRole::BiologicalUnit),
        HierarchyNode::new(
            slide.clone(),
            None,
            ReplicationRole::TechnicalReplicate {
                biological_source: patient,
            },
        ),
    ];
    nodes.extend(cells.iter().cloned().map(|cell_id| {
        HierarchyNode::new(
            HierarchyId::from(cell_id),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("hierarchy");
    let row_link = CellEmbeddingRowLink::new(
        artifact_id(b"source-cells"),
        artifact_id(b"source-vectors"),
        artifact_id(b"expected"),
        artifact_id(b"identity-map"),
        artifact_id(b"converter"),
        &expected,
        &hierarchy,
        vec![
            CellEmbeddingRowLinkEntry::present(cells[0].clone(), 0, 0),
            CellEmbeddingRowLinkEntry::missing_vector(cells[1].clone(), 1),
            CellEmbeddingRowLinkEntry::qc_rejected(cells[2].clone(), 2, 1),
        ],
        1024 * 1024,
    )
    .expect("row link");
    (
        expected,
        row_link,
        EmbeddingColumnarBudgets::new(
            8 * 1024 * 1024,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
        ),
    )
}

fn footer_bounds(bytes: &[u8]) -> (usize, usize) {
    let length_offset = bytes.len() - 10;
    let length = i32::from_le_bytes(
        bytes[length_offset..length_offset + size_of::<i32>()]
            .try_into()
            .expect("footer length"),
    );
    assert!(length > 0);
    let length = usize::try_from(length).expect("footer length");
    (length_offset - length, length)
}

fn first_record_block(bytes: &[u8]) -> (usize, usize, usize) {
    let (footer_start, footer_length) = footer_bounds(bytes);
    let footer = arrow_ipc::root_as_footer(&bytes[footer_start..footer_start + footer_length])
        .expect("footer");
    let block = footer.recordBatches().expect("batches").get(0);
    (
        usize::try_from(block.offset()).expect("block offset"),
        usize::try_from(block.metaDataLength()).expect("metadata length"),
        usize::try_from(block.bodyLength()).expect("body length"),
    )
}

fn rewrite_first_message(
    bytes: &[u8],
    compression: bool,
    variadic: bool,
    message_metadata_tables: usize,
) -> Vec<u8> {
    let (offset, metadata_length, body_length) = first_record_block(bytes);
    let original =
        arrow_ipc::root_as_message(&bytes[offset + 8..offset + metadata_length]).expect("message");
    let original_batch = original.header_as_record_batch().expect("record batch");
    let nodes = original_batch
        .nodes()
        .expect("nodes")
        .iter()
        .collect::<Vec<_>>();
    let buffers = original_batch
        .buffers()
        .expect("buffers")
        .iter()
        .collect::<Vec<_>>();

    let mut builder = FlatBufferBuilder::new();
    let nodes = builder.create_vector(&nodes);
    let buffers = builder.create_vector(&buffers);
    let compression =
        compression.then(|| BodyCompression::create(&mut builder, &BodyCompressionArgs::default()));
    let variadic_counts = variadic.then(|| builder.create_vector(&[1_i64]));
    let batch = RecordBatch::create(
        &mut builder,
        &RecordBatchArgs {
            length: original_batch.length(),
            nodes: Some(nodes),
            buffers: Some(buffers),
            compression,
            variadicBufferCounts: variadic_counts,
        },
    );
    let mut entries = Vec::new();
    for index in 0..message_metadata_tables {
        let key = builder.create_string(&format!("hostile-{index}"));
        let value = builder.create_string("value");
        entries.push(KeyValue::create(
            &mut builder,
            &KeyValueArgs {
                key: Some(key),
                value: Some(value),
            },
        ));
    }
    let custom_metadata = (!entries.is_empty()).then(|| builder.create_vector(&entries));
    let message = Message::create(
        &mut builder,
        &MessageArgs {
            version: original.version(),
            header_type: MessageHeader::RecordBatch,
            header: Some(batch.as_union_value()),
            bodyLength: original.bodyLength(),
            custom_metadata,
        },
    );
    builder.finish(message, None);
    let payload = builder.finished_data();
    let new_metadata_length = (payload.len() + 8).div_ceil(64) * 64;
    let declared_payload = new_metadata_length - 8;
    let old_body_start = offset + metadata_length;
    let old_body_end = old_body_start + body_length;
    let (footer_start, footer_length) = footer_bounds(bytes);
    let footer_bytes = &bytes[footer_start..footer_start + footer_length];
    let footer = arrow_ipc::root_as_footer(footer_bytes).expect("footer");
    let block_raw = footer.recordBatches().expect("batches").get(0).0;
    let block_offsets = footer_bytes
        .windows(block_raw.len())
        .enumerate()
        .filter_map(|(index, window)| (window == block_raw).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(block_offsets.len(), 1);
    let mut rewritten_footer = footer_bytes.to_vec();
    let metadata_field = block_offsets[0] + 8;
    rewritten_footer[metadata_field..metadata_field + size_of::<i32>()].copy_from_slice(
        &i32::try_from(new_metadata_length)
            .expect("metadata length")
            .to_le_bytes(),
    );

    let mut rewritten = Vec::new();
    rewritten.extend_from_slice(&bytes[..offset]);
    rewritten.extend_from_slice(&[0xff; 4]);
    rewritten.extend_from_slice(
        &i32::try_from(declared_payload)
            .expect("declared payload")
            .to_le_bytes(),
    );
    rewritten.extend_from_slice(payload);
    rewritten.resize(offset + new_metadata_length, 0);
    rewritten.extend_from_slice(&bytes[old_body_start..old_body_end]);
    rewritten.extend_from_slice(&bytes[old_body_end..footer_start]);
    rewritten.extend_from_slice(&rewritten_footer);
    rewritten.extend_from_slice(
        &i32::try_from(footer_length)
            .expect("footer length")
            .to_le_bytes(),
    );
    rewritten.extend_from_slice(b"ARROW1");
    rewritten
}

#[test]
fn public_row_link_preflight_rejects_compression_variadic_and_table_heavy_messages() {
    let (expected, row_link, budgets) = fixture();
    let mut canonical = Vec::new();
    write_cell_embedding_row_link_arrow(&mut canonical, &row_link, budgets)
        .expect("canonical row-link Arrow");
    preflight_cell_embedding_row_link_arrow_bytes(&canonical, &expected, &row_link, budgets)
        .expect("canonical preflight");

    for hostile in [
        rewrite_first_message(&canonical, true, false, 0),
        rewrite_first_message(&canonical, false, true, 0),
        rewrite_first_message(&canonical, false, false, 3),
    ] {
        assert!(matches!(
            preflight_cell_embedding_row_link_arrow_bytes(&hostile, &expected, &row_link, budgets,),
            Err(EmbeddingColumnarError::Arrow {
                reason: ArrowIpcFailure::InvalidMessage,
            })
        ));
    }
}
