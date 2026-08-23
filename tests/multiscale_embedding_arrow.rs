#![cfg(feature = "parquet")]

#[path = "support/multiscale_columnar.rs"]
mod support;

use std::{fs, process::Command};

use marklab::{
    preflight_patch_footprint_set_arrow_bytes, preflight_patch_overlap_graph_arrow_bytes,
    publish_patch_footprint_set_arrow, publish_patch_overlap_graph_arrow,
    validate_patch_footprint_set_arrow_bytes, validate_patch_footprint_set_arrow_from_store,
    validate_patch_overlap_graph_arrow_bytes, validate_patch_overlap_graph_arrow_from_store,
    write_patch_footprint_set_arrow, write_patch_overlap_graph_arrow, ContentDigest,
    EmbeddingColumnarBudgets, LocalArtifactStore, MultiscaleColumnarError, PatchOverlapGraph,
    PublicationDisposition, SpatialArrowFailure, StoreId, TableFormat,
};
use tempfile::TempDir;

use support::{fixture, reflecting_fixture, FragmentingWriter};

fn budgets(file_bytes: u64) -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        file_bytes,
        64 * 1024 * 1024,
        32 * 1024 * 1024,
        32 * 1024 * 1024,
    )
}

fn arrow_footer_bounds(bytes: &[u8]) -> (usize, usize) {
    let length_offset = bytes.len() - 10;
    let length = i32::from_le_bytes(
        bytes[length_offset..length_offset + 4]
            .try_into()
            .expect("footer length"),
    );
    assert!(length > 0);
    (bytes.len() - 10 - length as usize, length as usize)
}

fn first_arrow_validity_byte(bytes: &[u8]) -> usize {
    let (footer_start, footer_length) = arrow_footer_bounds(bytes);
    let footer = arrow::ipc::root_as_footer(&bytes[footer_start..footer_start + footer_length])
        .expect("Arrow footer");
    let block = footer.recordBatches().expect("record batches").get(0);
    let block_offset = usize::try_from(block.offset()).expect("block offset");
    let metadata_length = usize::try_from(block.metaDataLength()).expect("metadata length");
    let message =
        arrow::ipc::root_as_message(&bytes[block_offset + 8..block_offset + metadata_length])
            .expect("record message");
    let batch = message.header_as_record_batch().expect("record batch");
    let validity_offset = usize::try_from(batch.buffers().expect("buffers").get(0).offset())
        .expect("validity offset");
    block_offset + metadata_length + validity_offset
}

fn first_arrow_block_bytes(bytes: &[u8]) -> (usize, usize) {
    let (footer_start, footer_length) = arrow_footer_bounds(bytes);
    let footer = arrow::ipc::root_as_footer(&bytes[footer_start..footer_start + footer_length])
        .expect("Arrow footer");
    let block = footer.recordBatches().expect("record batches").get(0);
    (
        usize::try_from(block.metaDataLength()).expect("metadata length"),
        usize::try_from(block.bodyLength()).expect("body length"),
    )
}

fn arrow_reader_requirements(bytes: &[u8]) -> (usize, usize, u64) {
    const READER_WORKSPACE: usize = 64 * 1024;

    let (footer_start, footer_length) = arrow_footer_bounds(bytes);
    let footer = arrow::ipc::root_as_footer(&bytes[footer_start..footer_start + footer_length])
        .expect("Arrow footer");
    let blocks = footer.recordBatches().expect("record batches");
    let mut maximum_group = 0_usize;
    let mut maximum_batch_decoded = 0_usize;
    let mut aggregate_decoded = 0_u64;
    for block in blocks.iter() {
        let offset = usize::try_from(block.offset()).expect("block offset");
        let metadata = usize::try_from(block.metaDataLength()).expect("metadata length");
        let body = usize::try_from(block.bodyLength()).expect("body length");
        maximum_group = maximum_group.max(metadata + body);
        let message = arrow::ipc::root_as_message(&bytes[offset + 8..offset + metadata])
            .expect("record message");
        let batch = message.header_as_record_batch().expect("record batch");
        let decoded = batch
            .buffers()
            .expect("buffers")
            .iter()
            .map(|buffer| usize::try_from(buffer.length()).expect("buffer length"))
            .sum::<usize>();
        maximum_batch_decoded = maximum_batch_decoded.max(decoded);
        aggregate_decoded += u64::try_from(decoded).expect("decoded bytes");
    }
    let retained_preflight = footer_length * 2 + maximum_group + READER_WORKSPACE;
    let retained_full = retained_preflight + maximum_batch_decoded + READER_WORKSPACE;
    (maximum_group, retained_full, aggregate_decoded)
}

fn first_arrow_block_raw(bytes: &[u8]) -> (usize, [u8; 24]) {
    let (footer_start, footer_length) = arrow_footer_bounds(bytes);
    let footer_bytes = &bytes[footer_start..footer_start + footer_length];
    let footer = arrow::ipc::root_as_footer(footer_bytes).expect("Arrow footer");
    let blocks = footer.recordBatches().expect("record batches");
    let block = blocks.get(0);
    let relative = (block as *const arrow::ipc::Block as usize)
        .checked_sub(footer_bytes.as_ptr() as usize)
        .expect("block inside footer");
    (footer_start + relative, block.0)
}

fn first_arrow_node_and_buffer(bytes: &[u8]) -> (usize, [u8; 16], usize, [u8; 16]) {
    let (footer_start, footer_length) = arrow_footer_bounds(bytes);
    let footer = arrow::ipc::root_as_footer(&bytes[footer_start..footer_start + footer_length])
        .expect("Arrow footer");
    let block = footer.recordBatches().expect("record batches").get(0);
    let offset = usize::try_from(block.offset()).expect("block offset");
    let metadata = usize::try_from(block.metaDataLength()).expect("metadata length");
    let message_bytes = &bytes[offset + 8..offset + metadata];
    let message = arrow::ipc::root_as_message(message_bytes).expect("record message");
    let batch = message.header_as_record_batch().expect("record batch");
    let nodes = batch.nodes().expect("nodes");
    let node = nodes.get(0);
    let buffers = batch.buffers().expect("buffers");
    let buffer = buffers.get(0);
    let message_base = message_bytes.as_ptr() as usize;
    let node_relative = (node as *const arrow::ipc::FieldNode as usize)
        .checked_sub(message_base)
        .expect("node inside message");
    let buffer_relative = (buffer as *const arrow::ipc::Buffer as usize)
        .checked_sub(message_base)
        .expect("buffer inside message");
    (
        offset + 8 + node_relative,
        node.0,
        offset + 8 + buffer_relative,
        buffer.0,
    )
}

#[test]
fn canonical_spatial_arrow_writers_are_deterministic_and_preflight_exact_rows() {
    let fixture = fixture(3);
    let limits = budgets(8 * 1024 * 1024);
    let mut footprint_first = Vec::new();
    let footprint_summary = write_patch_footprint_set_arrow(
        &mut footprint_first,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("write footprint Arrow");
    let mut footprint_second = Vec::new();
    write_patch_footprint_set_arrow(
        &mut footprint_second,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("repeat footprint Arrow");
    assert_eq!(footprint_second, footprint_first);
    let mut fragmented = FragmentingWriter::default();
    write_patch_footprint_set_arrow(
        &mut fragmented,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("fragmented footprint Arrow");
    assert_eq!(fragmented.bytes, footprint_first);
    assert_eq!(footprint_summary.row_count(), 3);
    assert_eq!(
        footprint_summary.encoded_byte_len(),
        footprint_first.len() as u64
    );
    assert_eq!(
        footprint_summary.content_digest(),
        ContentDigest::from_bytes(&footprint_first)
    );
    assert_eq!(
        footprint_summary.content_digest().to_string(),
        "ab4e2b3599066f7b3fedabc8033ce64a4dbccb3f53ff6112ff6204adf47775de"
    );
    let footprint_preflight = preflight_patch_footprint_set_arrow_bytes(
        &footprint_first,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("preflight footprint Arrow");
    assert_eq!(footprint_preflight.row_count(), 3);
    assert_eq!(footprint_preflight.record_batch_count(), 1);

    let mut overlap_first = Vec::new();
    let overlap_summary = write_patch_overlap_graph_arrow(
        &mut overlap_first,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &fixture.overlap,
        limits,
    )
    .expect("write overlap Arrow");
    let mut overlap_second = Vec::new();
    write_patch_overlap_graph_arrow(
        &mut overlap_second,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &fixture.overlap,
        limits,
    )
    .expect("repeat overlap Arrow");
    assert_eq!(overlap_second, overlap_first);
    assert_eq!(overlap_summary.row_count(), 2);
    assert_eq!(
        overlap_summary.content_digest(),
        ContentDigest::from_bytes(&overlap_first)
    );
    assert_eq!(
        overlap_summary.content_digest().to_string(),
        "cf05c10ba0c280c98009935b49eefb573aee5b81dc20817affa8b86745cd3b16"
    );
    let overlap_preflight = preflight_patch_overlap_graph_arrow_bytes(
        &overlap_first,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &fixture.overlap,
        limits,
    )
    .expect("preflight overlap Arrow");
    assert_eq!(overlap_preflight.row_count(), 2);
    assert_eq!(overlap_preflight.record_batch_count(), 1);
}

#[test]
fn spatial_arrow_uses_exact_empty_and_public_batch_boundaries() {
    for (rows, batches) in [(0, 0), (1, 1), (8_192, 1), (8_193, 2)] {
        let fixture = fixture(rows);
        let limits = budgets(32 * 1024 * 1024);
        let mut encoded = Vec::new();
        write_patch_footprint_set_arrow(
            &mut encoded,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            limits,
        )
        .expect("write boundary Arrow");
        let preflight = preflight_patch_footprint_set_arrow_bytes(
            &encoded,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            limits,
        )
        .expect("preflight boundary Arrow");
        assert_eq!(preflight.row_count(), rows as u64);
        assert_eq!(preflight.record_batch_count(), batches);
    }
}

#[test]
fn spatial_arrow_preserves_negative_reflect_origins_and_empty_overlap_graphs() {
    let reflecting = reflecting_fixture();
    let limits = budgets(8 * 1024 * 1024);
    let mut footprints = Vec::new();
    write_patch_footprint_set_arrow(
        &mut footprints,
        &reflecting.expected,
        &reflecting.context,
        &reflecting.footprints,
        limits,
    )
    .expect("write reflecting footprints");
    preflight_patch_footprint_set_arrow_bytes(
        &footprints,
        &reflecting.expected,
        &reflecting.context,
        &reflecting.footprints,
        limits,
    )
    .expect("preflight negative origin");

    let isolated = fixture(1);
    let mut overlap = Vec::new();
    write_patch_overlap_graph_arrow(
        &mut overlap,
        &isolated.expected,
        &isolated.context,
        &isolated.footprints,
        &isolated.overlap,
        limits,
    )
    .expect("write empty overlap graph");
    let preflight = preflight_patch_overlap_graph_arrow_bytes(
        &overlap,
        &isolated.expected,
        &isolated.context,
        &isolated.footprints,
        &isolated.overlap,
        limits,
    )
    .expect("preflight empty overlap graph");
    assert_eq!(preflight.row_count(), 0);
    assert_eq!(preflight.record_batch_count(), 0);
}

#[test]
fn spatial_arrow_publication_has_exact_profiles_and_dependencies() {
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("spatial-arrow").expect("store ID"),
    )
    .expect("store");
    let fixture = fixture(3);
    let limits = budgets(8 * 1024 * 1024);

    let footprints = publish_patch_footprint_set_arrow(
        &store,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("publish footprints");
    assert_eq!(footprints.disposition(), PublicationDisposition::Created);
    assert_eq!(
        footprints.record().schema().id(),
        "marklab.patch_footprint_table"
    );
    assert_eq!(
        footprints.record().content().kind(),
        "application/vnd.marklab.patch-footprint-table.v1+arrow"
    );
    assert_eq!(
        footprints.record().table().expect("manifest").format(),
        TableFormat::ArrowIpcFile
    );
    let mut expected_dependencies = vec![fixture.expected_artifact_id, fixture.context_artifact_id];
    expected_dependencies.sort_unstable();
    assert_eq!(footprints.record().dependencies(), expected_dependencies);
    let mut footprint_bytes = Vec::new();
    write_patch_footprint_set_arrow(
        &mut footprint_bytes,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("write footprint bytes");
    validate_patch_footprint_set_arrow_bytes(
        &footprint_bytes,
        footprints.record(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("validate borrowed footprints");
    validate_patch_footprint_set_arrow_from_store(
        &store,
        footprints.record(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("validate managed footprints");

    let physical_overlap = PatchOverlapGraph::derive(
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        footprints.record().id(),
        8 * 1024 * 1024,
        8 * 1024 * 1024,
    )
    .expect("physical overlap graph");

    let overlap = publish_patch_overlap_graph_arrow(
        &store,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &physical_overlap,
        limits,
    )
    .expect("publish overlap");
    assert_eq!(overlap.disposition(), PublicationDisposition::Created);
    assert_eq!(
        overlap.record().schema().id(),
        "marklab.patch_overlap_edge_table"
    );
    assert_eq!(
        overlap.record().content().kind(),
        "application/vnd.marklab.patch-overlap-edge-table.v1+arrow"
    );
    let mut overlap_dependencies = vec![
        fixture.expected_artifact_id,
        fixture.context_artifact_id,
        footprints.record().id(),
    ];
    overlap_dependencies.sort_unstable();
    assert_eq!(overlap.record().dependencies(), overlap_dependencies);
    let mut overlap_bytes = Vec::new();
    write_patch_overlap_graph_arrow(
        &mut overlap_bytes,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &physical_overlap,
        limits,
    )
    .expect("write overlap bytes");
    validate_patch_overlap_graph_arrow_bytes(
        &overlap_bytes,
        overlap.record(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &physical_overlap,
        limits,
    )
    .expect("validate borrowed overlap");
    validate_patch_overlap_graph_arrow_from_store(
        &store,
        overlap.record(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &physical_overlap,
        limits,
    )
    .expect("validate managed overlap");
}

#[test]
fn spatial_arrow_fully_decodes_three_batches_for_both_profiles() {
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("three-batch-arrow").expect("store ID"),
    )
    .expect("store");
    let fixture = fixture(16_386);
    let limits = budgets(64 * 1024 * 1024);
    let footprint = publish_patch_footprint_set_arrow(
        &store,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("publish three-batch footprints");
    let footprint_summary = validate_patch_footprint_set_arrow_from_store(
        &store,
        footprint.record(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("decode three-batch footprints");
    assert_eq!(footprint_summary.record_batch_count(), 3);

    let overlap = PatchOverlapGraph::derive(
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        footprint.record().id(),
        64 * 1024 * 1024,
        64 * 1024 * 1024,
    )
    .expect("derive three-batch overlap");
    let publication = publish_patch_overlap_graph_arrow(
        &store,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &overlap,
        limits,
    )
    .expect("publish three-batch overlap");
    let overlap_summary = validate_patch_overlap_graph_arrow_from_store(
        &store,
        publication.record(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &overlap,
        limits,
    )
    .expect("decode three-batch overlap");
    assert_eq!(overlap_summary.record_batch_count(), 3);
}

#[test]
fn spatial_arrow_file_budget_is_enforced_before_encoding_or_preflight() {
    let fixture = fixture(3);
    let generous = budgets(8 * 1024 * 1024);
    let mut bytes = Vec::new();
    write_patch_footprint_set_arrow(
        &mut bytes,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        generous,
    )
    .expect("write Arrow");
    let short = budgets(bytes.len() as u64 - 1);
    assert!(write_patch_footprint_set_arrow(
        &mut Vec::new(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        short,
    )
    .is_err());
    assert!(preflight_patch_footprint_set_arrow_bytes(
        &bytes,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        short,
    )
    .is_err());
}

#[test]
fn spatial_arrow_raw_preflight_rejects_header_and_canonical_row_drift() {
    let fixture = fixture(3);
    let limits = budgets(8 * 1024 * 1024);
    let mut canonical = Vec::new();
    write_patch_footprint_set_arrow(
        &mut canonical,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("write Arrow");

    let mut bad_padding = canonical.clone();
    bad_padding[6] = 1;
    assert!(matches!(
        preflight_patch_footprint_set_arrow_bytes(
            &bad_padding,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            limits,
        ),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidMagic,
        })
    ));

    let mut bad_footer_length = canonical.clone();
    let footer_length_offset = bad_footer_length.len() - 10;
    bad_footer_length[footer_length_offset..footer_length_offset + 4]
        .copy_from_slice(&(-1_i32).to_le_bytes());
    assert!(matches!(
        preflight_patch_footprint_set_arrow_bytes(
            &bad_footer_length,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            limits,
        ),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidFooterLength,
        })
    ));

    let mut wrong_metadata = canonical.clone();
    let from = b"marklab.arrow-ipc.patch-footprint-table.v1";
    let to = b"marklab.arrow-ipc.patch-footprint-table.x1";
    assert_eq!(from.len(), to.len());
    let metadata_offsets = wrong_metadata
        .windows(from.len())
        .enumerate()
        .filter_map(|(index, window)| (window == from).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(metadata_offsets.len(), 2, "schema and footer metadata");
    for offset in metadata_offsets {
        wrong_metadata[offset..offset + to.len()].copy_from_slice(to);
    }
    assert!(matches!(
        preflight_patch_footprint_set_arrow_bytes(
            &wrong_metadata,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            limits,
        ),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidApplicationMetadata,
        })
    ));

    let mut wrong_validity = canonical.clone();
    let validity = first_arrow_validity_byte(&wrong_validity);
    wrong_validity[validity] = 0x7f;
    assert!(matches!(
        preflight_patch_footprint_set_arrow_bytes(
            &wrong_validity,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            limits,
        ),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidBuffers,
        })
    ));

    let (block_at, block_raw) = first_arrow_block_raw(&canonical);
    let mut negative_block = canonical.clone();
    negative_block[block_at..block_at + 8].copy_from_slice(&(-1_i64).to_le_bytes());
    assert!(matches!(
        preflight_patch_footprint_set_arrow_bytes(
            &negative_block,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            limits,
        ),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidBlock,
        })
    ));
    let mut misaligned_block = canonical.clone();
    let original_offset = i64::from_le_bytes(block_raw[..8].try_into().expect("block offset"));
    misaligned_block[block_at..block_at + 8].copy_from_slice(&(original_offset + 1).to_le_bytes());
    assert!(matches!(
        preflight_patch_footprint_set_arrow_bytes(
            &misaligned_block,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            limits,
        ),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidBlock,
        })
    ));

    let (node_at, node_raw, buffer_at, _) = first_arrow_node_and_buffer(&canonical);
    let mut wrong_node = canonical.clone();
    wrong_node[node_at + 8..node_at + 16].copy_from_slice(&1_i64.to_le_bytes());
    assert_ne!(&wrong_node[node_at..node_at + 16], &node_raw);
    assert!(matches!(
        preflight_patch_footprint_set_arrow_bytes(
            &wrong_node,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            limits,
        ),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidFieldNodes,
        })
    ));
    let mut negative_buffer = canonical.clone();
    negative_buffer[buffer_at + 8..buffer_at + 16].copy_from_slice(&(-1_i64).to_le_bytes());
    assert!(matches!(
        preflight_patch_footprint_set_arrow_bytes(
            &negative_buffer,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            limits,
        ),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidBuffers,
        })
    ));

    let needle = b"patch-00000000";
    let offsets = canonical
        .windows(needle.len())
        .enumerate()
        .filter_map(|(index, window)| (window == needle).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 1, "one physical patch row");
    let mut wrong_id = canonical;
    wrong_id[offsets[0]] = b'P';
    assert!(matches!(
        preflight_patch_footprint_set_arrow_bytes(
            &wrong_id,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            limits,
        ),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidCanonicalRows,
        })
    ));

    let mut overlap_bytes = Vec::new();
    write_patch_overlap_graph_arrow(
        &mut overlap_bytes,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &fixture.overlap,
        limits,
    )
    .expect("write overlap Arrow");
    let endpoint = b"patch-00000001";
    let endpoint_at = overlap_bytes
        .windows(endpoint.len())
        .position(|window| window == endpoint)
        .expect("overlap endpoint");
    overlap_bytes[endpoint_at] = b'P';
    assert!(matches!(
        preflight_patch_overlap_graph_arrow_bytes(
            &overlap_bytes,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            &fixture.overlap,
            limits,
        ),
        Err(MultiscaleColumnarError::Arrow {
            reason: SpatialArrowFailure::InvalidCanonicalRows,
        })
    ));
}

#[test]
fn spatial_arrow_decoded_row_group_and_retained_budgets_have_exact_edges() {
    let fixture = fixture(3);
    let generous = budgets(8 * 1024 * 1024);
    let discover = EmbeddingColumnarBudgets::new(8 * 1024 * 1024, 0, 0, 0);
    let decoded_required = match write_patch_footprint_set_arrow(
        &mut Vec::new(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        discover,
    ) {
        Err(MultiscaleColumnarError::DecodedByteBudgetExceeded {
            required,
            maximum: 0,
        }) => required,
        observed => panic!("expected decoded budget failure, got {observed:?}"),
    };
    assert_eq!(decoded_required, 109, "independent footprint buffer oracle");
    let decoded_exact = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        generous.maximum_retained_bytes(),
        generous.maximum_row_group_bytes(),
        decoded_required,
    );
    write_patch_footprint_set_arrow(
        &mut Vec::new(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        decoded_exact,
    )
    .expect("exact decoded budget");
    let decoded_short = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        generous.maximum_retained_bytes(),
        generous.maximum_row_group_bytes(),
        decoded_required - 1,
    );
    assert!(matches!(
        write_patch_footprint_set_arrow(
            &mut Vec::new(),
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            decoded_short,
        ),
        Err(MultiscaleColumnarError::DecodedByteBudgetExceeded { .. })
    ));

    let no_group = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        generous.maximum_retained_bytes(),
        0,
        generous.maximum_decoded_bytes(),
    );
    let group_required = match write_patch_footprint_set_arrow(
        &mut Vec::new(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        no_group,
    ) {
        Err(MultiscaleColumnarError::RowGroupByteBudgetExceeded {
            required,
            maximum: 0,
        }) => required,
        observed => panic!("expected row-group budget failure, got {observed:?}"),
    };
    let no_retained = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        0,
        group_required,
        generous.maximum_decoded_bytes(),
    );
    let retained_required = match write_patch_footprint_set_arrow(
        &mut Vec::new(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        no_retained,
    ) {
        Err(MultiscaleColumnarError::RetainedByteBudgetExceeded {
            required,
            maximum: 0,
        }) => required,
        observed => panic!("expected retained budget failure, got {observed:?}"),
    };
    let exact = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        retained_required,
        group_required,
        generous.maximum_decoded_bytes(),
    );
    write_patch_footprint_set_arrow(
        &mut Vec::new(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        exact,
    )
    .expect("exact physical budgets");
    let group_short = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        retained_required,
        group_required - 1,
        generous.maximum_decoded_bytes(),
    );
    assert!(matches!(
        write_patch_footprint_set_arrow(
            &mut Vec::new(),
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            group_short,
        ),
        Err(MultiscaleColumnarError::RowGroupByteBudgetExceeded { .. })
    ));
    let retained_short = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        retained_required - 1,
        group_required,
        generous.maximum_decoded_bytes(),
    );
    assert!(matches!(
        write_patch_footprint_set_arrow(
            &mut Vec::new(),
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            retained_short,
        ),
        Err(MultiscaleColumnarError::RetainedByteBudgetExceeded { .. })
    ));

    let mut overlap_bytes = Vec::new();
    write_patch_overlap_graph_arrow(
        &mut overlap_bytes,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &fixture.overlap,
        generous,
    )
    .expect("write overlap budget fixture");
    let overlap_decoded = 82_u64;
    let overlap_exact = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        generous.maximum_retained_bytes(),
        generous.maximum_row_group_bytes(),
        overlap_decoded,
    );
    preflight_patch_overlap_graph_arrow_bytes(
        &overlap_bytes,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &fixture.overlap,
        overlap_exact,
    )
    .expect("exact overlap decoded budget");
    let overlap_short = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        generous.maximum_retained_bytes(),
        generous.maximum_row_group_bytes(),
        overlap_decoded - 1,
    );
    assert!(matches!(
        preflight_patch_overlap_graph_arrow_bytes(
            &overlap_bytes,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            &fixture.overlap,
            overlap_short,
        ),
        Err(MultiscaleColumnarError::DecodedByteBudgetExceeded { .. })
    ));

    let mut footprint_bytes = Vec::new();
    write_patch_footprint_set_arrow(
        &mut footprint_bytes,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        generous,
    )
    .expect("write reader budget fixture");
    let (message_bytes, body_bytes) = first_arrow_block_bytes(&footprint_bytes);
    let reader_group = message_bytes + body_bytes;
    let (_, footer_bytes) = arrow_footer_bounds(&footprint_bytes);
    let reader_retained = footer_bytes * 2 + reader_group + 64 * 1024;
    let reader_exact = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        reader_retained,
        reader_group,
        decoded_required,
    );
    preflight_patch_footprint_set_arrow_bytes(
        &footprint_bytes,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        reader_exact,
    )
    .expect("exact reader budgets");
    let reader_group_short = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        reader_retained,
        reader_group - 1,
        decoded_required,
    );
    assert!(matches!(
        preflight_patch_footprint_set_arrow_bytes(
            &footprint_bytes,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            reader_group_short,
        ),
        Err(MultiscaleColumnarError::RowGroupByteBudgetExceeded { .. })
    ));
    let reader_retained_short = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        reader_retained - 1,
        reader_group,
        decoded_required,
    );
    assert!(matches!(
        preflight_patch_footprint_set_arrow_bytes(
            &footprint_bytes,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            reader_retained_short,
        ),
        Err(MultiscaleColumnarError::RetainedByteBudgetExceeded { .. })
    ));
}

#[test]
fn spatial_arrow_full_readers_have_exact_group_and_retained_edges() {
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("spatial-arrow-reader-budgets").expect("store ID"),
    )
    .expect("store");
    let fixture = fixture(3);
    let generous = budgets(8 * 1024 * 1024);

    let footprint_publication = publish_patch_footprint_set_arrow(
        &store,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        generous,
    )
    .expect("publish footprint budget fixture");
    let mut footprint_bytes = Vec::new();
    write_patch_footprint_set_arrow(
        &mut footprint_bytes,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        generous,
    )
    .expect("write footprint budget fixture");
    let (footprint_group, footprint_retained, footprint_decoded) =
        arrow_reader_requirements(&footprint_bytes);
    let footprint_exact = EmbeddingColumnarBudgets::new(
        footprint_bytes.len() as u64,
        footprint_retained,
        footprint_group,
        footprint_decoded,
    );
    validate_patch_footprint_set_arrow_bytes(
        &footprint_bytes,
        footprint_publication.record(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        footprint_exact,
    )
    .expect("exact footprint full-reader budgets");
    for (budgets, expected_error) in [
        (
            EmbeddingColumnarBudgets::new(
                footprint_bytes.len() as u64,
                footprint_retained,
                footprint_group - 1,
                footprint_decoded,
            ),
            "group",
        ),
        (
            EmbeddingColumnarBudgets::new(
                footprint_bytes.len() as u64,
                footprint_retained - 1,
                footprint_group,
                footprint_decoded,
            ),
            "retained",
        ),
    ] {
        let error = validate_patch_footprint_set_arrow_bytes(
            &footprint_bytes,
            footprint_publication.record(),
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            budgets,
        )
        .expect_err("one-short footprint reader budget");
        assert!(
            matches!(
                (&error, expected_error),
                (
                    MultiscaleColumnarError::RowGroupByteBudgetExceeded { .. },
                    "group"
                ) | (
                    MultiscaleColumnarError::RetainedByteBudgetExceeded { .. },
                    "retained"
                )
            ),
            "unexpected {expected_error} error: {error:?}"
        );
    }

    let overlap = PatchOverlapGraph::derive(
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        footprint_publication.record().id(),
        8 * 1024 * 1024,
        8 * 1024 * 1024,
    )
    .expect("physical overlap graph");
    let overlap_publication = publish_patch_overlap_graph_arrow(
        &store,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &overlap,
        generous,
    )
    .expect("publish overlap budget fixture");
    let mut overlap_bytes = Vec::new();
    write_patch_overlap_graph_arrow(
        &mut overlap_bytes,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &overlap,
        generous,
    )
    .expect("write overlap budget fixture");
    let (overlap_group, overlap_retained, overlap_decoded) =
        arrow_reader_requirements(&overlap_bytes);
    let overlap_exact = EmbeddingColumnarBudgets::new(
        overlap_bytes.len() as u64,
        overlap_retained,
        overlap_group,
        overlap_decoded,
    );
    validate_patch_overlap_graph_arrow_bytes(
        &overlap_bytes,
        overlap_publication.record(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &overlap,
        overlap_exact,
    )
    .expect("exact overlap full-reader budgets");
    for (budgets, expected_error) in [
        (
            EmbeddingColumnarBudgets::new(
                overlap_bytes.len() as u64,
                overlap_retained,
                overlap_group - 1,
                overlap_decoded,
            ),
            "group",
        ),
        (
            EmbeddingColumnarBudgets::new(
                overlap_bytes.len() as u64,
                overlap_retained - 1,
                overlap_group,
                overlap_decoded,
            ),
            "retained",
        ),
    ] {
        let error = validate_patch_overlap_graph_arrow_bytes(
            &overlap_bytes,
            overlap_publication.record(),
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            &overlap,
            budgets,
        )
        .expect_err("one-short overlap reader budget");
        assert!(
            matches!(
                (&error, expected_error),
                (
                    MultiscaleColumnarError::RowGroupByteBudgetExceeded { .. },
                    "group"
                ) | (
                    MultiscaleColumnarError::RetainedByteBudgetExceeded { .. },
                    "retained"
                )
            ),
            "unexpected {expected_error} error: {error:?}"
        );
    }
}

#[test]
fn spatial_arrow_is_deterministic_across_fresh_processes() {
    let temporary = TempDir::new().expect("temporary directory");
    let executable = std::env::current_exe().expect("test executable");
    let first = temporary.path().join("first.arrow");
    let second = temporary.path().join("second.arrow");
    for path in [&first, &second] {
        let status = Command::new(&executable)
            .arg("--exact")
            .arg("spatial_arrow_child")
            .env("MARKLAB_SPATIAL_ARROW_CHILD", path)
            .status()
            .expect("fresh child");
        assert!(status.success());
    }
    assert_eq!(
        fs::read(first).expect("first"),
        fs::read(second).expect("second")
    );

    let overlap_first = temporary.path().join("first-overlap.arrow");
    let overlap_second = temporary.path().join("second-overlap.arrow");
    for path in [&overlap_first, &overlap_second] {
        let status = Command::new(&executable)
            .arg("--exact")
            .arg("spatial_arrow_child")
            .env("MARKLAB_SPATIAL_ARROW_OVERLAP_CHILD", path)
            .status()
            .expect("fresh overlap child");
        assert!(status.success());
    }
    assert_eq!(
        fs::read(overlap_first).expect("first overlap"),
        fs::read(overlap_second).expect("second overlap")
    );
}

#[test]
fn spatial_arrow_child() {
    let footprint_path = std::env::var_os("MARKLAB_SPATIAL_ARROW_CHILD");
    let overlap_path = std::env::var_os("MARKLAB_SPATIAL_ARROW_OVERLAP_CHILD");
    let Some(path) = footprint_path.or_else(|| overlap_path.clone()) else {
        return;
    };
    let fixture = fixture(3);
    let mut bytes = Vec::new();
    if overlap_path.is_some() {
        write_patch_overlap_graph_arrow(
            &mut bytes,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            &fixture.overlap,
            budgets(8 * 1024 * 1024),
        )
        .expect("child overlap write");
    } else {
        write_patch_footprint_set_arrow(
            &mut bytes,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            budgets(8 * 1024 * 1024),
        )
        .expect("child footprint write");
    }
    fs::write(path, bytes).expect("child output");
}
