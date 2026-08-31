use std::str::FromStr;

use marklab_data::{
    CellId, CohortHierarchy, HierarchyId, HierarchyNode, PatientId, ReplicationRole, SlideId,
};
use marklab_project::{ArtifactId, ContentDigest};
use thrift::protocol::TCompactOutputProtocol;

use crate::CellEmbeddingRowLinkEntry;

use super::super::writer::write_cell_embedding_row_link_parquet;
use super::*;

fn artifact_id(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

fn fixture() -> (
    ExpectedCellSet,
    CellEmbeddingRowLink,
    EmbeddingColumnarBudgets,
    Vec<u8>,
) {
    let (expected, row_link) = logical_fixture(["cell-a", "cell-b", "cell-c", "cell-d"]);
    let budgets = EmbeddingColumnarBudgets::new(
        8 * 1024 * 1024,
        16 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
    );
    let mut bytes = Vec::new();
    write_cell_embedding_row_link_parquet(&mut bytes, &row_link, budgets).expect("write row link");
    (expected, row_link, budgets, bytes)
}

fn logical_fixture(cell_names: [&str; 4]) -> (ExpectedCellSet, CellEmbeddingRowLink) {
    let cells = cell_names
        .into_iter()
        .map(|value| CellId::new(value).expect("cell"))
        .collect::<Vec<_>>();
    let expected =
        ExpectedCellSet::new("row-link-levels.v1", cells.clone()).expect("expected cells");
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
    nodes.extend(cells.iter().cloned().map(|cell| {
        HierarchyNode::new(
            HierarchyId::from(cell),
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
            CellEmbeddingRowLinkEntry::present(cells[0].clone(), 2, 1),
            CellEmbeddingRowLinkEntry::missing_vector(cells[1].clone(), 0),
            CellEmbeddingRowLinkEntry::extraction_failed(cells[2].clone(), 3),
            CellEmbeddingRowLinkEntry::qc_rejected(cells[3].clone(), 1, 0),
        ],
        1024 * 1024,
    )
    .expect("row link");
    (expected, row_link)
}

fn raw_footer(bytes: &[u8]) -> (usize, FileMetaData) {
    let footer_offset = bytes.len() - TRAILER_BYTES;
    let footer_length = u32::from_le_bytes(
        bytes[footer_offset..footer_offset + 4]
            .try_into()
            .expect("footer length"),
    ) as usize;
    let footer_start = footer_offset - footer_length;
    let mut protocol = BoundedCompactProtocol::new(
        &bytes[footer_start..footer_offset],
        footer_compact_limits_with_metadata(1, METADATA_KEYS.len()).expect("limits"),
    );
    let metadata = FileMetaData::read_from_in_protocol(&mut protocol).expect("footer metadata");
    (footer_start, metadata)
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

fn optional_definition_range(bytes: &[u8]) -> std::ops::Range<usize> {
    let footer_offset = bytes.len() - TRAILER_BYTES;
    let footer_length = u32::from_le_bytes(
        bytes[footer_offset..footer_offset + 4]
            .try_into()
            .expect("footer length"),
    ) as usize;
    let footer_start = footer_offset - footer_length;
    let mut footer_protocol = BoundedCompactProtocol::new(
        &bytes[footer_start..footer_offset],
        footer_compact_limits_with_metadata(1, METADATA_KEYS.len()).expect("limits"),
    );
    let metadata =
        FileMetaData::read_from_in_protocol(&mut footer_protocol).expect("footer metadata");
    let page_start = usize::try_from(
        metadata.row_groups[0].columns[2]
            .meta_data
            .as_ref()
            .expect("column")
            .data_page_offset,
    )
    .expect("page start");
    let mut page_protocol = BoundedCompactProtocol::new(&bytes[page_start..], page_limits());
    let header = PageHeader::read_from_in_protocol(&mut page_protocol).expect("page header");
    let header_bytes = page_protocol.consumed_bytes().expect("header bytes");
    let definitions = usize::try_from(
        header
            .data_page_header_v2
            .expect("page v2")
            .definition_levels_byte_length,
    )
    .expect("definition bytes");
    assert!(definitions > 0);
    let body_start = page_start + header_bytes;
    body_start..body_start + definitions
}

#[test]
fn preflight_rejects_row_link_schema_metadata_codec_encoding_and_auxiliary_drift() {
    let (expected, row_link, budgets, canonical) = fixture();
    let assert_rejected = |metadata: &FileMetaData, reason| {
        assert!(matches!(
            preflight_cell_embedding_row_link_parquet_bytes(
                &replace_footer(&canonical, metadata),
                &expected,
                &row_link,
                budgets,
            ),
            Err(EmbeddingColumnarError::Parquet { reason: observed }) if observed == reason
        ));
    };

    let (_, mut wrong_schema) = raw_footer(&canonical);
    wrong_schema.schema[0].name = "wrong_root".to_owned();
    assert_rejected(&wrong_schema, ParquetFailure::InvalidSchema);

    let (_, mut wrong_metadata) = raw_footer(&canonical);
    wrong_metadata.created_by = Some("wrong-writer".to_owned());
    assert_rejected(&wrong_metadata, ParquetFailure::InvalidMetadata);

    let (_, mut compressed) = raw_footer(&canonical);
    compressed.row_groups[0].columns[0]
        .meta_data
        .as_mut()
        .expect("column")
        .codec = CompressionCodec::SNAPPY;
    assert_rejected(&compressed, ParquetFailure::UnsupportedCompression);

    let (_, mut dictionary_encoding) = raw_footer(&canonical);
    dictionary_encoding.row_groups[0].columns[0]
        .meta_data
        .as_mut()
        .expect("column")
        .encodings
        .push(Encoding::RLE_DICTIONARY);
    assert_rejected(&dictionary_encoding, ParquetFailure::UnsupportedEncoding);

    let (_, mut dictionary_page) = raw_footer(&canonical);
    dictionary_page.row_groups[0].columns[0]
        .meta_data
        .as_mut()
        .expect("column")
        .dictionary_page_offset = Some(4);
    assert_rejected(&dictionary_page, ParquetFailure::ForbiddenAuxiliaryData);

    let (_, mut indexes) = raw_footer(&canonical);
    indexes.row_groups[0].columns[0].offset_index_offset = Some(4);
    indexes.row_groups[0].columns[0].offset_index_length = Some(8);
    assert_rejected(&indexes, ParquetFailure::ForbiddenAuxiliaryData);
}

#[test]
fn preflight_rejects_optional_levels_that_disagree_with_the_logical_row_link() {
    let (expected, row_link, budgets, mut bytes) = fixture();
    let levels = optional_definition_range(&bytes);
    bytes[levels.end - 1] ^= 1;
    assert!(matches!(
        preflight_cell_embedding_row_link_parquet_bytes(&bytes, &expected, &row_link, budgets,),
        Err(EmbeddingColumnarError::Parquet {
            reason: ParquetFailure::InvalidPage,
        })
    ));
}

#[test]
fn preflight_rejects_larger_physical_cell_ids_before_stock_decode_at_exact_budget() {
    let (expected, row_link, budgets, canonical) = fixture();
    let (_, longer_row_link) = logical_fixture([
        "cell-a-hostile-physical-payload",
        "cell-b-hostile-physical-payload",
        "cell-c-hostile-physical-payload",
        "cell-d-hostile-physical-payload",
    ]);
    let mut longer = Vec::new();
    write_cell_embedding_row_link_parquet(&mut longer, &longer_row_link, budgets)
        .expect("write longer row link");
    assert!(longer.len() > canonical.len());

    let (_, mut metadata) = raw_footer(&longer);
    let digest = metadata
        .key_value_metadata
        .as_mut()
        .expect("metadata")
        .iter_mut()
        .find(|entry| entry.key == "marklab.row_link_digest")
        .expect("row-link digest");
    digest.value = Some(row_link.logical_digest().to_string());
    let hostile = replace_footer(&longer, &metadata);
    let required = estimate_decoded_bytes(&row_link).expect("decoded bytes");
    let exact = EmbeddingColumnarBudgets::new(
        budgets.maximum_file_bytes(),
        budgets.maximum_retained_bytes(),
        budgets.maximum_row_group_bytes(),
        required,
    );
    assert!(matches!(
        preflight_cell_embedding_row_link_parquet_bytes(&hostile, &expected, &row_link, exact,),
        Err(EmbeddingColumnarError::Parquet {
            reason: ParquetFailure::InvalidCellOrder,
        })
    ));

    let one_short = EmbeddingColumnarBudgets::new(
        budgets.maximum_file_bytes(),
        budgets.maximum_retained_bytes(),
        budgets.maximum_row_group_bytes(),
        required - 1,
    );
    assert_eq!(
        preflight_cell_embedding_row_link_parquet_bytes(&hostile, &expected, &row_link, one_short,),
        Err(EmbeddingColumnarError::DecodedByteBudgetExceeded {
            required,
            maximum: required - 1,
        })
    );
}
