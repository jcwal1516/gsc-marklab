#![cfg(feature = "parquet")]

use std::{
    collections::BTreeMap,
    fs,
    io::{self, Cursor, Write},
    process::Command,
    str::FromStr,
};

use marklab::{
    preflight_cell_embedding_row_link_arrow_bytes, publish_cell_embedding_row_link_arrow,
    validate_cell_embedding_row_link_arrow_bytes,
    validate_cell_embedding_row_link_arrow_from_store, write_cell_embedding_row_link_arrow,
    ArrowIpcFailure, ArtifactDraft, ArtifactId, ArtifactKey, ArtifactLocator, ArtifactRecord,
    ArtifactRef, ArtifactSchema, CellEmbeddingRowLink, CellEmbeddingRowLinkEntry, CellId,
    CohortHierarchy, ContentDigest, EmbeddingColumnarBudgets, EmbeddingColumnarError,
    ExpectedCellSet, HierarchyId, HierarchyNode, LocalArtifactStore, PatientId,
    PublicationDisposition, ReplicationRole, SlideId, StoreId, TableColumn, TableColumnType,
    TableFormat, TableManifest, TableScalarType, VerifiedReaderError,
};
use tempfile::TempDir;

fn artifact_id(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

fn cell(value: &str) -> CellId {
    CellId::new(value).expect("cell ID")
}

fn fixture() -> (ExpectedCellSet, CellEmbeddingRowLink) {
    let expected = ExpectedCellSet::new(
        "all.v1",
        vec![
            cell("cell-a"),
            cell("cell-b"),
            cell("cell-c"),
            cell("cell-d"),
        ],
    )
    .expect("expected cells");
    let patient = HierarchyId::from(PatientId::new("patient").expect("patient ID"));
    let slide = HierarchyId::from(SlideId::new("slide").expect("slide ID"));
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
    nodes.extend(expected.cells().iter().cloned().map(|cell_id| {
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
            CellEmbeddingRowLinkEntry::present(cell("cell-a"), 2, 1),
            CellEmbeddingRowLinkEntry::missing_vector(cell("cell-b"), 0),
            CellEmbeddingRowLinkEntry::extraction_failed(cell("cell-c"), 3),
            CellEmbeddingRowLinkEntry::qc_rejected(cell("cell-d"), 1, 0),
        ],
        1024 * 1024,
    )
    .expect("row link");
    (expected, row_link)
}

fn fixture_with_rows(row_count: usize) -> (ExpectedCellSet, CellEmbeddingRowLink) {
    fixture_with_row_pattern(row_count, false)
}

fn fixture_with_mixed_rows(row_count: usize) -> (ExpectedCellSet, CellEmbeddingRowLink) {
    fixture_with_row_pattern(row_count, true)
}

fn fixture_with_row_pattern(
    row_count: usize,
    mixed: bool,
) -> (ExpectedCellSet, CellEmbeddingRowLink) {
    let cells = (0..row_count)
        .map(|index| cell(&format!("cell-{index:08}")))
        .collect::<Vec<_>>();
    let expected = ExpectedCellSet::new("all.v1", cells.clone()).expect("expected cells");
    let patient = HierarchyId::from(PatientId::new("patient-many").expect("patient ID"));
    let slide = HierarchyId::from(SlideId::new("slide-many").expect("slide ID"));
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
    let mut next_embedding_row = 0_u64;
    let entries = cells
        .into_iter()
        .enumerate()
        .map(|(index, cell)| {
            let source_cell_row = index as u64;
            if !mixed {
                return CellEmbeddingRowLinkEntry::present(cell, source_cell_row, source_cell_row);
            }
            match index % 4 {
                0 => {
                    let entry = CellEmbeddingRowLinkEntry::present(
                        cell,
                        source_cell_row,
                        next_embedding_row,
                    );
                    next_embedding_row += 1;
                    entry
                }
                1 => CellEmbeddingRowLinkEntry::missing_vector(cell, source_cell_row),
                2 => CellEmbeddingRowLinkEntry::extraction_failed(cell, source_cell_row),
                _ => {
                    let entry = CellEmbeddingRowLinkEntry::qc_rejected(
                        cell,
                        source_cell_row,
                        next_embedding_row,
                    );
                    next_embedding_row += 1;
                    entry
                }
            }
        })
        .collect();
    let row_link = CellEmbeddingRowLink::new(
        artifact_id(b"source-cells-many"),
        artifact_id(b"source-vectors-many"),
        artifact_id(b"expected-many"),
        artifact_id(b"identity-map-many"),
        artifact_id(b"converter-many"),
        &expected,
        &hierarchy,
        entries,
        16 * 1024 * 1024,
    )
    .expect("row link");
    (expected, row_link)
}

fn budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
    )
}

fn row_link_manifest(
    format: TableFormat,
    encoding: &str,
    row_count: u64,
    source_cell_nullable: bool,
    source_embedding_nullable: bool,
    primary_key: &str,
) -> TableManifest {
    TableManifest::new(
        format,
        encoding,
        row_count,
        vec![
            TableColumn::new(
                "cell_id",
                TableColumnType::Scalar(TableScalarType::Utf8),
                false,
            )
            .expect("cell column"),
            TableColumn::new(
                "source_cell_row",
                TableColumnType::Scalar(TableScalarType::U64),
                source_cell_nullable,
            )
            .expect("source-cell column"),
            TableColumn::new(
                "source_embedding_row",
                TableColumnType::Scalar(TableScalarType::U64),
                source_embedding_nullable,
            )
            .expect("source-embedding column"),
        ],
        vec![primary_key.to_owned()],
    )
    .expect("row-link manifest")
}

fn external_record(
    content: ArtifactRef,
    schema: ArtifactSchema,
    table: Option<TableManifest>,
    dependencies: Vec<ArtifactId>,
    semantic_metadata: BTreeMap<String, String>,
) -> ArtifactRecord {
    ArtifactRecord::new(
        content,
        schema,
        table,
        dependencies,
        semantic_metadata,
        vec![ArtifactLocator::new(
            StoreId::new("external-fixture").expect("external store ID"),
            ArtifactKey::new("fixtures/row-link.arrow").expect("external key"),
            None,
        )
        .expect("external locator")],
    )
    .expect("external record")
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

fn record_block(bytes: &[u8], index: usize) -> (usize, usize, usize) {
    let (footer_start, footer_length) = footer_bounds(bytes);
    let footer = arrow::ipc::root_as_footer(&bytes[footer_start..footer_start + footer_length])
        .expect("canonical footer");
    let block = footer.recordBatches().expect("record batches").get(index);
    (
        block.offset() as usize,
        block.metaDataLength() as usize,
        block.bodyLength() as usize,
    )
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

fn footer_block_location(bytes: &[u8], index: usize) -> (usize, [u8; 24]) {
    let (footer_start, footer_length) = footer_bounds(bytes);
    let footer_bytes = &bytes[footer_start..footer_start + footer_length];
    let footer = arrow::ipc::root_as_footer(footer_bytes).expect("canonical footer");
    let raw = footer.recordBatches().expect("record batches").get(index).0;
    (footer_start + find_unique(footer_bytes, &raw), raw)
}

fn alias_footer_dictionaries_to_record_batches(bytes: &mut [u8]) {
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
    let dictionaries_slot = vtable + 4 + 2 * size_of::<u16>();
    let record_batches_slot = vtable + 4 + 3 * size_of::<u16>();
    let record_batches_field = footer[record_batches_slot..record_batches_slot + 2].to_vec();
    footer[dictionaries_slot..dictionaries_slot + 2].copy_from_slice(&record_batches_field);
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

fn rewrite_with_footer_metadata(bytes: &[u8], key: &str, value: &str) -> Vec<u8> {
    let mut reader = arrow::ipc::reader::FileReader::try_new(Cursor::new(bytes), None)
        .expect("canonical reader");
    let schema = reader.schema();
    let batches = reader
        .by_ref()
        .collect::<Result<Vec<_>, _>>()
        .expect("canonical batches");
    let options =
        arrow::ipc::writer::IpcWriteOptions::try_new(64, false, arrow::ipc::MetadataVersion::V5)
            .expect("IPC options");
    let mut rewritten = Vec::new();
    let mut writer = arrow::ipc::writer::FileWriter::try_new_with_options(
        &mut rewritten,
        schema.as_ref(),
        options,
    )
    .expect("metadata writer");
    writer.write_metadata(key, value);
    for batch in &batches {
        writer.write(batch).expect("metadata batch");
    }
    writer.finish().expect("metadata finish");
    drop(writer);
    rewritten
}

fn record_message(bytes: &[u8], batch_index: usize) -> (usize, arrow::ipc::Message<'_>) {
    let (offset, metadata_length, _) = record_block(bytes, batch_index);
    let message_start = offset + 8;
    let message = arrow::ipc::root_as_message(&bytes[message_start..offset + metadata_length])
        .expect("canonical message");
    (message_start, message)
}

fn message_node_location(bytes: &[u8], batch_index: usize, node_index: usize) -> (usize, [u8; 16]) {
    let (message_start, message) = record_message(bytes, batch_index);
    let batch = message.header_as_record_batch().expect("record batch");
    let raw = batch.nodes().expect("nodes").get(node_index).0;
    let (offset, metadata_length, _) = record_block(bytes, batch_index);
    let message_bytes = &bytes[message_start..offset + metadata_length];
    (message_start + find_unique(message_bytes, &raw), raw)
}

fn message_buffer_location(
    bytes: &[u8],
    batch_index: usize,
    buffer_index: usize,
) -> (usize, [u8; 16]) {
    let (message_start, message) = record_message(bytes, batch_index);
    let batch = message.header_as_record_batch().expect("record batch");
    let raw = batch.buffers().expect("buffers").get(buffer_index).0;
    let (offset, metadata_length, _) = record_block(bytes, batch_index);
    let message_bytes = &bytes[message_start..offset + metadata_length];
    (message_start + find_unique(message_bytes, &raw), raw)
}

fn body_buffer_range(
    bytes: &[u8],
    batch_index: usize,
    buffer_index: usize,
) -> std::ops::Range<usize> {
    let (_, message) = record_message(bytes, batch_index);
    let batch = message.header_as_record_batch().expect("record batch");
    let buffer = batch.buffers().expect("buffers").get(buffer_index);
    let (block_offset, metadata_length, _) = record_block(bytes, batch_index);
    let start = block_offset + metadata_length + buffer.offset() as usize;
    start..start + buffer.length() as usize
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

#[derive(Default)]
struct FragmentingWriter {
    bytes: Vec<u8>,
    calls: usize,
}

impl Write for FragmentingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        const FRAGMENTS: [usize; 7] = [1, 3, 2, 7, 5, 11, 4];
        let maximum = FRAGMENTS[self.calls % FRAGMENTS.len()];
        self.calls += 1;
        let written = buffer.len().min(maximum);
        self.bytes.extend_from_slice(&buffer[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn canonical_row_link_arrow_writer_preflight_and_publication_are_exact() {
    let (expected, row_link) = fixture();
    let mut first = Vec::new();
    let first_summary =
        write_cell_embedding_row_link_arrow(&mut first, &row_link, budgets()).expect("write link");
    let mut second = Vec::new();
    let second_summary = write_cell_embedding_row_link_arrow(&mut second, &row_link, budgets())
        .expect("repeat link");
    assert_eq!(second, first);
    assert_eq!(second_summary, first_summary);
    assert_eq!(first_summary.encoded_byte_len(), first.len() as u64);
    assert_eq!(
        first_summary.content_digest(),
        ContentDigest::from_bytes(&first)
    );
    assert_eq!(
        first_summary.content_digest().to_string(),
        "0edeab689aa350263d3057098648a17c72bbf56066af2ed7e4a3bcab031f58a5"
    );
    assert_eq!(first_summary.row_count(), 4);

    let preflight =
        preflight_cell_embedding_row_link_arrow_bytes(&first, &expected, &row_link, budgets())
            .expect("preflight link");
    assert_eq!(preflight.row_count(), 4);
    assert_eq!(preflight.record_batch_count(), 1);
    assert_eq!(preflight.content_digest(), first_summary.content_digest());

    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("row-link-store").expect("store ID"),
    )
    .expect("store");
    let publication =
        publish_cell_embedding_row_link_arrow(&store, &row_link, budgets()).expect("publish link");
    assert_eq!(publication.disposition(), PublicationDisposition::Created);
    assert_eq!(publication.record().locations().len(), 1);
    assert_eq!(
        publication.record().table().expect("manifest").format(),
        TableFormat::ArrowIpcFile
    );
    assert_eq!(
        publication.record().dependencies(),
        row_link.direct_dependencies()
    );
    store.verify(publication.record()).expect("verify link");
    let repeated =
        publish_cell_embedding_row_link_arrow(&store, &row_link, budgets()).expect("reuse link");
    assert_eq!(
        repeated.disposition(),
        PublicationDisposition::AlreadyPresent
    );
    assert_eq!(repeated.record(), publication.record());

    let decoded = validate_cell_embedding_row_link_arrow_bytes(
        &first,
        publication.record(),
        &expected,
        &row_link,
        budgets(),
    )
    .expect("decode borrowed link");
    assert_eq!(decoded, preflight);
    let managed = validate_cell_embedding_row_link_arrow_from_store(
        &store,
        publication.record(),
        &expected,
        &row_link,
        budgets(),
    )
    .expect("decode managed link");
    assert_eq!(managed, preflight);
}

#[test]
fn canonical_row_link_writer_uses_exact_batch_boundaries() {
    for (rows, batches) in [(0, 0), (1, 1), (8_191, 1), (8_192, 1), (8_193, 2)] {
        let (expected, row_link) = fixture_with_rows(rows);
        let mut bytes = Vec::new();
        write_cell_embedding_row_link_arrow(&mut bytes, &row_link, budgets()).expect("write link");
        let preflight =
            preflight_cell_embedding_row_link_arrow_bytes(&bytes, &expected, &row_link, budgets())
                .unwrap_or_else(|error| panic!("preflight {rows} rows: {error:?}"));
        assert_eq!(preflight.row_count(), rows as u64);
        assert_eq!(preflight.record_batch_count(), batches);
    }
}

#[test]
fn canonical_row_link_writer_preserves_mixed_validity_across_batch_boundary() {
    for rows in [8_191, 8_192, 8_193] {
        let (expected, row_link) = fixture_with_mixed_rows(rows);
        let mut bytes = Vec::new();
        write_cell_embedding_row_link_arrow(&mut bytes, &row_link, budgets())
            .expect("write mixed boundary link");
        let preflight =
            preflight_cell_embedding_row_link_arrow_bytes(&bytes, &expected, &row_link, budgets())
                .unwrap_or_else(|error| panic!("mixed boundary preflight for {rows}: {error:?}"));
        assert_eq!(preflight.record_batch_count(), rows.div_ceil(8_192) as u32);

        for batch_index in 0..usize::try_from(preflight.record_batch_count()).unwrap() {
            let start = batch_index * 8_192;
            let end = (start + 8_192).min(rows);
            let mut expected_bitmap = vec![0_u8; (end - start).div_ceil(8)];
            for (local_row, entry) in row_link.entries()[start..end].iter().enumerate() {
                if entry.source_embedding_row().is_some() {
                    expected_bitmap[local_row / 8] |= 1 << (local_row % 8);
                }
            }
            let range = body_buffer_range(&bytes, batch_index, 5);
            assert_eq!(
                &bytes[range],
                expected_bitmap.as_slice(),
                "{rows} rows, batch {batch_index}"
            );
        }
    }
}

#[test]
fn row_link_arrow_preflight_rejects_framing_footer_and_file_budget_drift() {
    let (expected, row_link) = fixture();
    let mut bytes = Vec::new();
    write_cell_embedding_row_link_arrow(&mut bytes, &row_link, budgets()).expect("write link");

    let mut bad_magic = bytes.clone();
    bad_magic[0] = 0;
    assert!(matches!(
        preflight_cell_embedding_row_link_arrow_bytes(&bad_magic, &expected, &row_link, budgets(),),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidMagic,
        })
    ));

    let mut bad_padding = bytes.clone();
    bad_padding[6] = 1;
    assert!(matches!(
        preflight_cell_embedding_row_link_arrow_bytes(
            &bad_padding,
            &expected,
            &row_link,
            budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidMagic,
        })
    ));

    let mut negative_footer = bytes.clone();
    let footer_length_offset = negative_footer.len() - 10;
    negative_footer[footer_length_offset..footer_length_offset + 4]
        .copy_from_slice(&(-1_i32).to_le_bytes());
    assert!(matches!(
        preflight_cell_embedding_row_link_arrow_bytes(
            &negative_footer,
            &expected,
            &row_link,
            budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidFooterLength,
        })
    ));

    let (footer_start, _) = footer_bounds(&bytes);
    let mut bad_eos = bytes.clone();
    bad_eos[footer_start - 1] = 1;
    assert!(matches!(
        preflight_cell_embedding_row_link_arrow_bytes(&bad_eos, &expected, &row_link, budgets(),),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidFooterLength,
        })
    ));

    let too_small = EmbeddingColumnarBudgets::new(
        bytes.len() as u64 - 1,
        budgets().maximum_retained_bytes(),
        budgets().maximum_row_group_bytes(),
        budgets().maximum_decoded_bytes(),
    );
    assert!(matches!(
        preflight_cell_embedding_row_link_arrow_bytes(&bytes, &expected, &row_link, too_small,),
        Err(EmbeddingColumnarError::FileByteBudgetExceeded { .. })
    ));
}

#[test]
fn row_link_arrow_preflight_rejects_absent_empty_record_batch_vector() {
    let (expected, row_link) = fixture_with_rows(0);
    let mut bytes = Vec::new();
    write_cell_embedding_row_link_arrow(&mut bytes, &row_link, budgets())
        .expect("write empty link");
    clear_footer_record_batches(&mut bytes);
    assert!(matches!(
        preflight_cell_embedding_row_link_arrow_bytes(&bytes, &expected, &row_link, budgets(),),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidFooter,
        })
    ));
}

#[test]
fn row_link_arrow_preflight_rejects_block_node_buffer_schema_and_metadata_drift() {
    let (expected, row_link) = fixture();
    let mut canonical = Vec::new();
    write_cell_embedding_row_link_arrow(&mut canonical, &row_link, budgets()).expect("write link");

    let (block_offset, block_raw) = footer_block_location(&canonical, 0);
    let mut negative_block = canonical.clone();
    negative_block[block_offset..block_offset + 8].copy_from_slice(&(-1_i64).to_le_bytes());
    assert!(matches!(
        preflight_cell_embedding_row_link_arrow_bytes(
            &negative_block,
            &expected,
            &row_link,
            budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidBlock,
        })
    ));

    let original_block_offset =
        i64::from_le_bytes(block_raw[..8].try_into().expect("block offset"));
    let mut misaligned_block = canonical.clone();
    misaligned_block[block_offset..block_offset + 8]
        .copy_from_slice(&(original_block_offset + 1).to_le_bytes());
    assert!(matches!(
        preflight_cell_embedding_row_link_arrow_bytes(
            &misaligned_block,
            &expected,
            &row_link,
            budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidBlock,
        })
    ));

    let mut dictionary_footer = canonical.clone();
    alias_footer_dictionaries_to_record_batches(&mut dictionary_footer);
    assert!(matches!(
        preflight_cell_embedding_row_link_arrow_bytes(
            &dictionary_footer,
            &expected,
            &row_link,
            budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::DictionaryBatch,
        })
    ));

    let footer_metadata =
        rewrite_with_footer_metadata(&canonical, "private", "privacy-sentinel-footer-metadata");
    let error = preflight_cell_embedding_row_link_arrow_bytes(
        &footer_metadata,
        &expected,
        &row_link,
        budgets(),
    )
    .expect_err("footer custom metadata");
    assert!(matches!(
        error,
        EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::FileCustomMetadata,
        }
    ));
    assert!(!error.to_string().contains("privacy-sentinel"));

    let (node_offset, node_raw) = message_node_location(&canonical, 0, 2);
    let mut wrong_null_count = canonical.clone();
    wrong_null_count[node_offset + 8..node_offset + 16].copy_from_slice(&0_i64.to_le_bytes());
    assert!(matches!(
        preflight_cell_embedding_row_link_arrow_bytes(
            &wrong_null_count,
            &expected,
            &row_link,
            budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidFieldNodes,
        })
    ));
    assert_eq!(i64::from_le_bytes(node_raw[8..].try_into().unwrap()), 2);

    let (buffer_offset, buffer_raw) = message_buffer_location(&canonical, 0, 5);
    let original_buffer_offset =
        i64::from_le_bytes(buffer_raw[..8].try_into().expect("buffer offset"));
    let mut misaligned_buffer = canonical.clone();
    misaligned_buffer[buffer_offset..buffer_offset + 8]
        .copy_from_slice(&(original_buffer_offset + 1).to_le_bytes());
    assert!(matches!(
        preflight_cell_embedding_row_link_arrow_bytes(
            &misaligned_buffer,
            &expected,
            &row_link,
            budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidBuffers,
        })
    ));

    let mut giant_buffer = canonical.clone();
    giant_buffer[buffer_offset + 8..buffer_offset + 16].copy_from_slice(&i64::MAX.to_le_bytes());
    assert!(matches!(
        preflight_cell_embedding_row_link_arrow_bytes(
            &giant_buffer,
            &expected,
            &row_link,
            budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidBuffers,
        })
    ));

    let mut initial_schema_drift = canonical.clone();
    let schema_range = initial_schema_message_range(&initial_schema_drift);
    assert_eq!(
        replace_all_same_length(
            &mut initial_schema_drift[schema_range],
            b"source_embedding_row",
            b"source_embedding_roW",
        ),
        1
    );
    assert!(matches!(
        preflight_cell_embedding_row_link_arrow_bytes(
            &initial_schema_drift,
            &expected,
            &row_link,
            budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidSchema,
        })
    ));

    let mut metadata_drift = canonical;
    assert_eq!(
        replace_all_same_length(
            &mut metadata_drift,
            b"marklab.cell_embedding_row_link",
            b"marklab.cell_embedding_row_linK",
        ),
        2
    );
    let error = preflight_cell_embedding_row_link_arrow_bytes(
        &metadata_drift,
        &expected,
        &row_link,
        budgets(),
    )
    .expect_err("metadata drift");
    assert!(matches!(
        error,
        EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidApplicationMetadata,
        }
    ));
    assert!(!error.to_string().contains("cell_embedding_row_linK"));
}

#[test]
fn row_link_arrow_preflight_enforces_exact_nullable_bitmap_and_private_fillers() {
    for rows in [1, 7, 8, 9] {
        let (expected, row_link) = fixture_with_rows(rows);
        let mut bytes = Vec::new();
        write_cell_embedding_row_link_arrow(&mut bytes, &row_link, budgets())
            .expect("write all-present link");
        let bitmap = body_buffer_range(&bytes, 0, 5);
        let mut expected_bitmap = vec![u8::MAX; rows.div_ceil(8)];
        if rows % 8 != 0 {
            *expected_bitmap.last_mut().expect("tail bitmap") = u8::MAX >> (8 - rows % 8);
        }
        assert_eq!(
            &bytes[bitmap.clone()],
            expected_bitmap.as_slice(),
            "{rows} rows"
        );
        let (_, message) = record_message(&bytes, 0);
        assert_eq!(
            message
                .header_as_record_batch()
                .expect("record batch")
                .nodes()
                .expect("nodes")
                .get(2)
                .null_count(),
            0
        );
        preflight_cell_embedding_row_link_arrow_bytes(&bytes, &expected, &row_link, budgets())
            .expect("preflight all-present link");
    }

    let (expected, row_link) = fixture();
    let mut canonical = Vec::new();
    write_cell_embedding_row_link_arrow(&mut canonical, &row_link, budgets())
        .expect("write mixed link");
    let validity = body_buffer_range(&canonical, 0, 5);
    assert_eq!(&canonical[validity.clone()], &[0b0000_1001]);

    for mask in [0b0000_0001, 0b0000_0010, 0b1000_0000] {
        let mut drift = canonical.clone();
        drift[validity.start] ^= mask;
        assert!(matches!(
            preflight_cell_embedding_row_link_arrow_bytes(&drift, &expected, &row_link, budgets(),),
            Err(EmbeddingColumnarError::Arrow {
                reason: ArrowIpcFailure::InvalidBuffers,
            })
        ));
    }

    let source_cells = body_buffer_range(&canonical, 0, 4);
    let mut wrong_source_cell = canonical.clone();
    wrong_source_cell[source_cells.start..source_cells.start + 8]
        .copy_from_slice(&99_u64.to_le_bytes());
    assert!(matches!(
        preflight_cell_embedding_row_link_arrow_bytes(
            &wrong_source_cell,
            &expected,
            &row_link,
            budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidComponent,
        })
    ));

    let source_embeddings = body_buffer_range(&canonical, 0, 6);
    let mut hidden_nonzero = canonical.clone();
    let missing_value = source_embeddings.start + size_of::<u64>();
    hidden_nonzero[missing_value..missing_value + size_of::<u64>()]
        .copy_from_slice(&99_u64.to_le_bytes());
    assert!(matches!(
        preflight_cell_embedding_row_link_arrow_bytes(
            &hidden_nonzero,
            &expected,
            &row_link,
            budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidComponent,
        })
    ));

    let mut wrong_cell = canonical;
    assert_eq!(
        replace_all_same_length(&mut wrong_cell, b"cell-a", b"cell-z"),
        1
    );
    let error =
        preflight_cell_embedding_row_link_arrow_bytes(&wrong_cell, &expected, &row_link, budgets())
            .expect_err("wrong cell");
    assert!(matches!(
        error,
        EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidCellOrder,
        }
    ));
    assert!(!error.to_string().contains("cell-z"));
}

#[test]
fn row_link_arrow_writer_and_preflight_enforce_exact_resource_edges() {
    let (expected, row_link) = fixture();
    let generous = budgets();
    let mut bytes = Vec::new();
    write_cell_embedding_row_link_arrow(&mut bytes, &row_link, generous).expect("write link");

    let exact_file = EmbeddingColumnarBudgets::new(
        bytes.len() as u64,
        generous.maximum_retained_bytes(),
        generous.maximum_row_group_bytes(),
        generous.maximum_decoded_bytes(),
    );
    let mut exact_output = Vec::new();
    write_cell_embedding_row_link_arrow(&mut exact_output, &row_link, exact_file)
        .expect("exact writer file budget");
    assert_eq!(exact_output, bytes);
    preflight_cell_embedding_row_link_arrow_bytes(&bytes, &expected, &row_link, exact_file)
        .expect("exact preflight file budget");
    let one_file_byte_short = EmbeddingColumnarBudgets::new(
        bytes.len() as u64 - 1,
        generous.maximum_retained_bytes(),
        generous.maximum_row_group_bytes(),
        generous.maximum_decoded_bytes(),
    );
    assert!(matches!(
        write_cell_embedding_row_link_arrow(&mut Vec::new(), &row_link, one_file_byte_short),
        Err(EmbeddingColumnarError::FileByteBudgetExceeded { .. })
    ));

    let row_group_required = match preflight_cell_embedding_row_link_arrow_bytes(
        &bytes,
        &expected,
        &row_link,
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
    let exact_row_group = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        generous.maximum_retained_bytes(),
        row_group_required,
        generous.maximum_decoded_bytes(),
    );
    preflight_cell_embedding_row_link_arrow_bytes(&bytes, &expected, &row_link, exact_row_group)
        .expect("exact row-group budget");
    assert!(matches!(
        preflight_cell_embedding_row_link_arrow_bytes(
            &bytes,
            &expected,
            &row_link,
            EmbeddingColumnarBudgets::new(
                generous.maximum_file_bytes(),
                generous.maximum_retained_bytes(),
                row_group_required - 1,
                generous.maximum_decoded_bytes(),
            ),
        ),
        Err(EmbeddingColumnarError::RowGroupByteBudgetExceeded { .. })
    ));

    let decoded_required = match preflight_cell_embedding_row_link_arrow_bytes(
        &bytes,
        &expected,
        &row_link,
        EmbeddingColumnarBudgets::new(
            generous.maximum_file_bytes(),
            generous.maximum_retained_bytes(),
            generous.maximum_row_group_bytes(),
            0,
        ),
    ) {
        Err(EmbeddingColumnarError::DecodedByteBudgetExceeded {
            required,
            maximum: 0,
        }) => required,
        result => panic!("expected decoded failure, observed {result:?}"),
    };
    assert!(matches!(
        write_cell_embedding_row_link_arrow(
            &mut Vec::new(),
            &row_link,
            EmbeddingColumnarBudgets::new(
                generous.maximum_file_bytes(),
                generous.maximum_retained_bytes(),
                generous.maximum_row_group_bytes(),
                0,
            ),
        ),
        Err(EmbeddingColumnarError::DecodedByteBudgetExceeded {
            required,
            maximum: 0,
        }) if required == decoded_required
    ));
    let exact_decoded = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        generous.maximum_retained_bytes(),
        generous.maximum_row_group_bytes(),
        decoded_required,
    );
    write_cell_embedding_row_link_arrow(&mut Vec::new(), &row_link, exact_decoded)
        .expect("exact writer decoded budget");
    preflight_cell_embedding_row_link_arrow_bytes(&bytes, &expected, &row_link, exact_decoded)
        .expect("exact preflight decoded budget");

    let footer_retained_required = match preflight_cell_embedding_row_link_arrow_bytes(
        &bytes,
        &expected,
        &row_link,
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
        result => panic!("expected footer retained failure, observed {result:?}"),
    };
    let block_retained_required = match preflight_cell_embedding_row_link_arrow_bytes(
        &bytes,
        &expected,
        &row_link,
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
        result => panic!("expected block retained failure, observed {result:?}"),
    };
    let exact_retained = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        block_retained_required,
        generous.maximum_row_group_bytes(),
        generous.maximum_decoded_bytes(),
    );
    preflight_cell_embedding_row_link_arrow_bytes(&bytes, &expected, &row_link, exact_retained)
        .expect("exact retained budget");
    assert!(matches!(
        preflight_cell_embedding_row_link_arrow_bytes(
            &bytes,
            &expected,
            &row_link,
            EmbeddingColumnarBudgets::new(
                generous.maximum_file_bytes(),
                block_retained_required - 1,
                generous.maximum_row_group_bytes(),
                generous.maximum_decoded_bytes(),
            ),
        ),
        Err(EmbeddingColumnarError::RetainedByteBudgetExceeded { .. })
    ));

    let root = TempDir::new().expect("reader budget store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("row-link-reader-budget").expect("store ID"),
    )
    .expect("reader budget store");
    let publication = publish_cell_embedding_row_link_arrow(&store, &row_link, generous)
        .expect("publish reader budget link");
    let reader_retained_required = match validate_cell_embedding_row_link_arrow_bytes(
        &bytes,
        publication.record(),
        &expected,
        &row_link,
        exact_retained,
    ) {
        Err(EmbeddingColumnarError::RetainedByteBudgetExceeded { required, maximum })
            if maximum == block_retained_required && required > maximum =>
        {
            required
        }
        result => panic!("expected decoded-batch retained failure, observed {result:?}"),
    };
    let exact_reader_retained = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        reader_retained_required,
        generous.maximum_row_group_bytes(),
        generous.maximum_decoded_bytes(),
    );
    validate_cell_embedding_row_link_arrow_bytes(
        &bytes,
        publication.record(),
        &expected,
        &row_link,
        exact_reader_retained,
    )
    .expect("exact full-reader retained budget");
    assert!(matches!(
        validate_cell_embedding_row_link_arrow_bytes(
            &bytes,
            publication.record(),
            &expected,
            &row_link,
            EmbeddingColumnarBudgets::new(
                generous.maximum_file_bytes(),
                reader_retained_required - 1,
                generous.maximum_row_group_bytes(),
                generous.maximum_decoded_bytes(),
            ),
        ),
        Err(EmbeddingColumnarError::RetainedByteBudgetExceeded { required, maximum })
            if required == reader_retained_required && maximum == reader_retained_required - 1
    ));
}

#[test]
fn row_link_arrow_writer_charges_many_batch_retained_peak_at_exact_edge() {
    let (_, row_link) = fixture_with_rows(100_000);
    let zero_retained =
        EmbeddingColumnarBudgets::new(64 * 1024 * 1024, 0, 64 * 1024 * 1024, 64 * 1024 * 1024);
    let required =
        match write_cell_embedding_row_link_arrow(&mut Vec::new(), &row_link, zero_retained) {
            Err(EmbeddingColumnarError::RetainedByteBudgetExceeded {
                required,
                maximum: 0,
            }) => required,
            result => panic!("expected retained failure, observed {result:?}"),
        };
    let exact = EmbeddingColumnarBudgets::new(
        64 * 1024 * 1024,
        required,
        64 * 1024 * 1024,
        64 * 1024 * 1024,
    );
    write_cell_embedding_row_link_arrow(&mut Vec::new(), &row_link, exact)
        .expect("exact many-batch retained budget");
    assert!(matches!(
        write_cell_embedding_row_link_arrow(
            &mut Vec::new(),
            &row_link,
            EmbeddingColumnarBudgets::new(
                64 * 1024 * 1024,
                required - 1,
                64 * 1024 * 1024,
                64 * 1024 * 1024,
            ),
        ),
        Err(EmbeddingColumnarError::RetainedByteBudgetExceeded { .. })
    ));
}

#[test]
fn row_link_arrow_reader_rejects_every_record_binding_surface_without_leaking_values() {
    const ENCODING: &str = "marklab.arrow-ipc.embedding-row-link.v1";
    const KIND: &str = "application/vnd.marklab.embedding-row-link.v1+arrow";
    const SECRET: &str = "privacy-sentinel-row-link-record";

    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("binding-store").expect("store ID"),
    )
    .expect("store");
    let (expected, row_link) = fixture();
    let publication =
        publish_cell_embedding_row_link_arrow(&store, &row_link, budgets()).expect("publish link");
    let record = publication.record();
    let bytes =
        fs::read(root.path().join(record.locations()[0].key().as_str())).expect("published bytes");
    let digest = ContentDigest::from_bytes(&bytes);
    let byte_len = bytes.len() as u64;
    let correct_schema = record.schema().clone();
    let correct_table = record.table().cloned();
    let correct_dependencies = row_link.direct_dependencies().to_vec();

    let mut variants = vec![
        (
            "kind",
            external_record(
                ArtifactRef::new("application/octet-stream", digest, byte_len).expect("wrong kind"),
                correct_schema.clone(),
                correct_table.clone(),
                correct_dependencies.clone(),
                BTreeMap::new(),
            ),
        ),
        (
            "digest",
            external_record(
                ArtifactRef::new(KIND, ContentDigest::from_bytes(b"wrong digest"), byte_len)
                    .expect("wrong digest"),
                correct_schema.clone(),
                correct_table.clone(),
                correct_dependencies.clone(),
                BTreeMap::new(),
            ),
        ),
        (
            "length",
            external_record(
                ArtifactRef::new(KIND, digest, byte_len + 1).expect("wrong length"),
                correct_schema.clone(),
                correct_table.clone(),
                correct_dependencies.clone(),
                BTreeMap::new(),
            ),
        ),
        (
            "schema",
            external_record(
                ArtifactRef::new(KIND, digest, byte_len).expect("content"),
                ArtifactSchema::new("marklab.cell_embedding_row_linK", 1).expect("wrong schema"),
                correct_table.clone(),
                correct_dependencies.clone(),
                BTreeMap::new(),
            ),
        ),
        (
            "version",
            external_record(
                ArtifactRef::new(KIND, digest, byte_len).expect("content"),
                ArtifactSchema::new("marklab.cell_embedding_row_link", 2).expect("wrong version"),
                correct_table.clone(),
                correct_dependencies.clone(),
                BTreeMap::new(),
            ),
        ),
        (
            "missing table",
            external_record(
                ArtifactRef::new(KIND, digest, byte_len).expect("content"),
                correct_schema.clone(),
                None,
                correct_dependencies.clone(),
                BTreeMap::new(),
            ),
        ),
    ];
    for (label, table) in [
        (
            "format",
            row_link_manifest(
                TableFormat::ParquetFile,
                ENCODING,
                4,
                false,
                true,
                "cell_id",
            ),
        ),
        (
            "encoding",
            row_link_manifest(
                TableFormat::ArrowIpcFile,
                "marklab.arrow-ipc.embedding-row-link.v2",
                4,
                false,
                true,
                "cell_id",
            ),
        ),
        (
            "row count",
            row_link_manifest(
                TableFormat::ArrowIpcFile,
                ENCODING,
                5,
                false,
                true,
                "cell_id",
            ),
        ),
        (
            "source cell nullability",
            row_link_manifest(
                TableFormat::ArrowIpcFile,
                ENCODING,
                4,
                true,
                true,
                "cell_id",
            ),
        ),
        (
            "source embedding nullability",
            row_link_manifest(
                TableFormat::ArrowIpcFile,
                ENCODING,
                4,
                false,
                false,
                "cell_id",
            ),
        ),
        (
            "primary key",
            row_link_manifest(
                TableFormat::ArrowIpcFile,
                ENCODING,
                4,
                false,
                true,
                "source_cell_row",
            ),
        ),
    ] {
        variants.push((
            label,
            external_record(
                ArtifactRef::new(KIND, digest, byte_len).expect("content"),
                correct_schema.clone(),
                Some(table),
                correct_dependencies.clone(),
                BTreeMap::new(),
            ),
        ));
    }
    variants.push((
        "dependencies",
        external_record(
            ArtifactRef::new(KIND, digest, byte_len).expect("content"),
            correct_schema.clone(),
            correct_table.clone(),
            Vec::new(),
            BTreeMap::new(),
        ),
    ));
    variants.push((
        "semantic metadata",
        external_record(
            ArtifactRef::new(KIND, digest, byte_len).expect("content"),
            correct_schema,
            correct_table,
            correct_dependencies,
            BTreeMap::from([("private".to_owned(), SECRET.to_owned())]),
        ),
    ));

    for (label, variant) in variants {
        let error = validate_cell_embedding_row_link_arrow_bytes(
            &bytes,
            &variant,
            &expected,
            &row_link,
            budgets(),
        )
        .unwrap_err();
        assert_eq!(
            error,
            EmbeddingColumnarError::ArtifactBindingMismatch,
            "{label}"
        );
    }
    let metadata_error = validate_cell_embedding_row_link_arrow_bytes(
        &bytes,
        &external_record(
            ArtifactRef::new(KIND, digest, byte_len).expect("content"),
            record.schema().clone(),
            record.table().cloned(),
            row_link.direct_dependencies().to_vec(),
            BTreeMap::from([("private".to_owned(), SECRET.to_owned())]),
        ),
        &expected,
        &row_link,
        budgets(),
    )
    .expect_err("private metadata");
    assert!(!metadata_error.to_string().contains(SECRET));
}

#[test]
fn row_link_arrow_borrowed_and_managed_readers_reject_the_same_hostile_block() {
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("hostile-store").expect("store ID"),
    )
    .expect("store");
    let (expected, row_link) = fixture();
    let declaration =
        publish_cell_embedding_row_link_arrow(&store, &row_link, budgets()).expect("declaration");
    let canonical_record = declaration.record();
    let mut hostile = fs::read(
        root.path()
            .join(canonical_record.locations()[0].key().as_str()),
    )
    .expect("canonical bytes");
    let (block_offset, _) = footer_block_location(&hostile, 0);
    hostile[block_offset..block_offset + 8].copy_from_slice(&(-1_i64).to_le_bytes());

    let hostile_draft = ArtifactDraft::new(
        ArtifactRef::from_bytes(canonical_record.content().kind(), &hostile)
            .expect("hostile content"),
        canonical_record.schema().clone(),
        canonical_record.table().cloned(),
        canonical_record.dependencies().to_vec(),
        BTreeMap::new(),
    )
    .expect("hostile draft");
    let hostile_record = store
        .publish_new_send(&hostile_draft, |writer| writer.write_all(&hostile))
        .expect("publish hostile fixture")
        .into_record();

    let borrowed_error = validate_cell_embedding_row_link_arrow_bytes(
        &hostile,
        &hostile_record,
        &expected,
        &row_link,
        budgets(),
    )
    .expect_err("borrowed hostile block");
    let managed_error = match validate_cell_embedding_row_link_arrow_from_store(
        &store,
        &hostile_record,
        &expected,
        &row_link,
        budgets(),
    ) {
        Err(VerifiedReaderError::Callback(error)) => error,
        result => panic!("expected managed callback failure, observed {result:?}"),
    };
    assert_eq!(managed_error, borrowed_error);
    assert!(matches!(
        managed_error,
        EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidBlock,
        }
    ));
}

#[test]
fn row_link_arrow_managed_reader_prioritizes_store_integrity_failure() {
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("integrity-store").expect("store ID"),
    )
    .expect("store");
    let (expected, row_link) = fixture();
    let publication =
        publish_cell_embedding_row_link_arrow(&store, &row_link, budgets()).expect("publish link");
    let path = root
        .path()
        .join(publication.record().locations()[0].key().as_str());
    let mut bytes = fs::read(&path).expect("managed bytes");
    bytes[0] ^= 1;
    fs::write(path, bytes).expect("tamper managed fixture");

    assert!(matches!(
        validate_cell_embedding_row_link_arrow_from_store(
            &store,
            publication.record(),
            &expected,
            &row_link,
            budgets(),
        ),
        Err(VerifiedReaderError::Store(_))
    ));
}

#[test]
fn canonical_row_link_writer_honors_fragmenting_write_sinks() {
    for rows in [1, 7, 8, 9, 8_193] {
        let (expected, row_link) = fixture_with_rows(rows);
        let mut contiguous = Vec::new();
        let contiguous_summary =
            write_cell_embedding_row_link_arrow(&mut contiguous, &row_link, budgets())
                .expect("contiguous write");
        let mut fragmented = FragmentingWriter::default();
        let fragmented_summary =
            write_cell_embedding_row_link_arrow(&mut fragmented, &row_link, budgets())
                .unwrap_or_else(|error| panic!("fragmented write for {rows} rows: {error:?}"));
        assert_eq!(fragmented.bytes, contiguous, "{rows} rows");
        assert_eq!(fragmented_summary, contiguous_summary, "{rows} rows");
        preflight_cell_embedding_row_link_arrow_bytes(
            &fragmented.bytes,
            &expected,
            &row_link,
            budgets(),
        )
        .unwrap_or_else(|error| panic!("fragmented preflight for {rows} rows: {error:?}"));
    }

    let (expected, row_link) = fixture();
    let mut contiguous = Vec::new();
    write_cell_embedding_row_link_arrow(&mut contiguous, &row_link, budgets())
        .expect("mixed contiguous write");
    let mut fragmented = FragmentingWriter::default();
    write_cell_embedding_row_link_arrow(&mut fragmented, &row_link, budgets())
        .expect("mixed fragmented write");
    assert_eq!(fragmented.bytes, contiguous);
    preflight_cell_embedding_row_link_arrow_bytes(
        &fragmented.bytes,
        &expected,
        &row_link,
        budgets(),
    )
    .expect("mixed fragmented preflight");
}

#[test]
fn canonical_row_link_writer_is_deterministic_across_fresh_processes() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let executable = std::env::current_exe().expect("current test executable");
    let first_path = temporary.path().join("first.arrow");
    let second_path = temporary.path().join("second.arrow");
    for path in [&first_path, &second_path] {
        let status = Command::new(&executable)
            .arg("--exact")
            .arg("canonical_row_link_writer_child")
            .env("MARKLAB_ROW_LINK_ARROW_CHILD_OUTPUT", path)
            .status()
            .expect("run fresh writer process");
        assert!(status.success());
    }
    let first = fs::read(first_path).expect("first child output");
    let second = fs::read(second_path).expect("second child output");
    assert_eq!(first, second);
    assert_eq!(
        ContentDigest::from_bytes(&first).to_string(),
        "0edeab689aa350263d3057098648a17c72bbf56066af2ed7e4a3bcab031f58a5"
    );
}

#[test]
fn canonical_row_link_writer_child() {
    let Some(path) = std::env::var_os("MARKLAB_ROW_LINK_ARROW_CHILD_OUTPUT") else {
        return;
    };
    let (_, row_link) = fixture();
    let mut bytes = Vec::new();
    write_cell_embedding_row_link_arrow(&mut bytes, &row_link, budgets()).expect("child write");
    fs::write(path, bytes).expect("write child output");
}
