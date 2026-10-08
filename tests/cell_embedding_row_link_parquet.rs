#![cfg(feature = "parquet")]

use std::{
    fs,
    io::{self, Write},
    ops::Range,
    process::Command,
    str::FromStr,
};

use marklab::{
    preflight_cell_embedding_row_link_arrow_bytes, preflight_cell_embedding_row_link_parquet_bytes,
    publish_cell_embedding_row_link_parquet, validate_cell_embedding_row_link_parquet_bytes,
    validate_cell_embedding_row_link_parquet_from_store, write_cell_embedding_row_link_arrow,
    write_cell_embedding_row_link_parquet, ArtifactId, ArtifactKey, ArtifactLocator,
    ArtifactRecord, ArtifactRef, CellEmbeddingRowLink, CellEmbeddingRowLinkEntry, CellId,
    CohortHierarchy, ContentDigest, EmbeddingColumnarBudgets, EmbeddingColumnarError,
    ExpectedCellSet, HierarchyId, HierarchyNode, LocalArtifactStore, PatientId,
    PublicationDisposition, ReplicationRole, SlideId, StoreId, TableFormat, VerifiedReaderError,
};
use parquet::file::metadata::ParquetMetaDataReader;
use proptest::prelude::*;
use tempfile::TempDir;

fn artifact_id(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

fn cell(value: &str) -> CellId {
    CellId::new(value).expect("cell ID")
}

fn record_for_bytes(template: &ArtifactRecord, bytes: &[u8]) -> ArtifactRecord {
    ArtifactRecord::new(
        ArtifactRef::from_bytes(template.content().kind(), bytes).expect("content"),
        template.schema().clone(),
        template.table().cloned(),
        template.dependencies().to_vec(),
        template.semantic_metadata().clone(),
        template.locations().to_vec(),
    )
    .expect("record")
}

fn external_record_for_bytes(template: &ArtifactRecord, bytes: &[u8]) -> ArtifactRecord {
    ArtifactRecord::new(
        ArtifactRef::from_bytes(template.content().kind(), bytes).expect("content"),
        template.schema().clone(),
        template.table().cloned(),
        template.dependencies().to_vec(),
        template.semantic_metadata().clone(),
        vec![ArtifactLocator::new(
            StoreId::new("row-link-hostile-source").expect("source store ID"),
            ArtifactKey::new("fixtures/row-link-hostile.parquet").expect("source key"),
            None,
        )
        .expect("source locator")],
    )
    .expect("record")
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

fn column_chunk_range(bytes: &[u8], column_index: usize) -> Range<usize> {
    let mut file = tempfile::tempfile().expect("temporary Parquet file");
    file.write_all(bytes).expect("write temporary Parquet");
    let metadata = ParquetMetaDataReader::new()
        .parse_and_finish(&file)
        .expect("parse trusted fixture metadata");
    let (start, length) = metadata.row_group(0).column(column_index).byte_range();
    let start = usize::try_from(start).expect("column start");
    let length = usize::try_from(length).expect("column length");
    start..start.checked_add(length).expect("column end")
}

fn unique_pattern_offset(bytes: &[u8], range: Range<usize>, pattern: [u8; 8]) -> usize {
    let offsets = bytes[range.clone()]
        .windows(pattern.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == pattern).then_some(range.start + offset))
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 1, "unique PLAIN value in selected chunk");
    offsets[0]
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
    let cells = (0..row_count)
        .map(|index| cell(&format!("cell-{index:08}")))
        .collect::<Vec<_>>();
    let expected = ExpectedCellSet::new("many.v1", cells.clone()).expect("expected cells");
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
        .map(|(index, cell_id)| {
            let source_cell_row = index as u64;
            if index.is_multiple_of(2) {
                let entry = CellEmbeddingRowLinkEntry::present(
                    cell_id,
                    source_cell_row,
                    next_embedding_row,
                );
                next_embedding_row += 1;
                entry
            } else {
                CellEmbeddingRowLinkEntry::missing_vector(cell_id, source_cell_row)
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
        16 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
    )
}

fn scale_budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        64 * 1024 * 1024,
        128 * 1024 * 1024,
        128 * 1024 * 1024,
        64 * 1024 * 1024,
    )
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
fn canonical_row_link_parquet_writer_is_deterministic_and_preflights() {
    let (expected, row_link) = fixture();
    let mut first = Vec::new();
    let first_summary = write_cell_embedding_row_link_parquet(&mut first, &row_link, budgets())
        .expect("write row-link Parquet");
    let mut second = Vec::new();
    let second_summary = write_cell_embedding_row_link_parquet(&mut second, &row_link, budgets())
        .expect("repeat row-link Parquet");

    assert_eq!(second, first);
    assert_eq!(second_summary, first_summary);
    assert_eq!(&first[..4], b"PAR1");
    assert_eq!(&first[first.len() - 4..], b"PAR1");
    assert_eq!(first_summary.encoded_byte_len(), first.len() as u64);
    assert_eq!(
        first_summary.content_digest(),
        ContentDigest::from_bytes(&first)
    );
    assert_eq!(
        first_summary.content_digest().to_string(),
        "82729ed1e8853c51578286e059a4d35028c658785cc87e525016ed97049dafa2"
    );
    assert_eq!(first_summary.row_count(), 4);

    let preflight =
        preflight_cell_embedding_row_link_parquet_bytes(&first, &expected, &row_link, budgets())
            .expect("preflight row-link Parquet");
    assert_eq!(preflight.row_count(), 4);
    assert_eq!(preflight.row_group_count(), 1);
    assert_eq!(preflight.encoded_byte_len(), first.len() as u64);
    assert_eq!(preflight.content_digest(), first_summary.content_digest());
}

#[test]
fn canonical_row_link_parquet_writer_is_deterministic_across_fresh_processes() {
    let temporary = TempDir::new().expect("temporary directory");
    let executable = std::env::current_exe().expect("current test executable");
    let first_path = temporary.path().join("first.parquet");
    let second_path = temporary.path().join("second.parquet");
    for path in [&first_path, &second_path] {
        let status = Command::new(&executable)
            .arg("--exact")
            .arg("canonical_row_link_parquet_writer_child")
            .env("MARKLAB_ROW_LINK_PARQUET_CHILD_OUTPUT", path)
            .status()
            .expect("run fresh row-link Parquet writer");
        assert!(status.success());
    }
    let first = fs::read(first_path).expect("first child output");
    let second = fs::read(second_path).expect("second child output");
    assert_eq!(first, second);
    assert_eq!(
        ContentDigest::from_bytes(&first).to_string(),
        "82729ed1e8853c51578286e059a4d35028c658785cc87e525016ed97049dafa2"
    );
}

#[test]
fn canonical_row_link_parquet_writer_child() {
    let Some(path) = std::env::var_os("MARKLAB_ROW_LINK_PARQUET_CHILD_OUTPUT") else {
        return;
    };
    let (_, row_link) = fixture();
    let mut bytes = Vec::new();
    write_cell_embedding_row_link_parquet(&mut bytes, &row_link, budgets())
        .expect("child row-link Parquet write");
    fs::write(path, bytes).expect("write child output");
}

#[test]
fn canonical_row_link_parquet_publication_is_fresh_and_idempotent() {
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("row-link-parquet-store").expect("store ID"),
    )
    .expect("store");
    let (_, row_link) = fixture();

    let first = publish_cell_embedding_row_link_parquet(&store, &row_link, budgets())
        .expect("publish row-link Parquet");
    assert_eq!(first.disposition(), PublicationDisposition::Created);
    assert_eq!(first.record().locations().len(), 1);
    assert_eq!(first.record().locations()[0].store_id(), store.store_id());
    assert_eq!(
        first.record().content().kind(),
        "application/vnd.marklab.embedding-row-link.v1+parquet"
    );
    assert_eq!(
        first.record().table().expect("manifest").format(),
        TableFormat::ParquetFile
    );
    store.verify(first.record()).expect("verify row link");

    let second = publish_cell_embedding_row_link_parquet(&store, &row_link, budgets())
        .expect("reuse row-link Parquet");
    assert_eq!(second.disposition(), PublicationDisposition::AlreadyPresent);
    assert_eq!(second.record(), first.record());
}

#[test]
fn canonical_row_link_parquet_writer_honors_fragmenting_sinks() {
    for row_count in [0, 1, 8_193] {
        let (expected, row_link) = fixture_with_rows(row_count);
        let mut contiguous = Vec::new();
        let contiguous_summary =
            write_cell_embedding_row_link_parquet(&mut contiguous, &row_link, scale_budgets())
                .expect("contiguous Parquet");
        let mut fragmented = FragmentingWriter::default();
        let fragmented_summary =
            write_cell_embedding_row_link_parquet(&mut fragmented, &row_link, scale_budgets())
                .expect("fragmented Parquet");
        assert_eq!(fragmented.bytes, contiguous);
        assert_eq!(fragmented_summary, contiguous_summary);
        preflight_cell_embedding_row_link_parquet_bytes(
            &fragmented.bytes,
            &expected,
            &row_link,
            scale_budgets(),
        )
        .expect("fragmented preflight");
    }
}

#[test]
fn borrowed_and_managed_row_link_parquet_validation_have_exact_parity() {
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("row-link-parquet-validation").expect("store ID"),
    )
    .expect("store");
    let (expected, row_link) = fixture();
    let mut bytes = Vec::new();
    write_cell_embedding_row_link_parquet(&mut bytes, &row_link, budgets())
        .expect("write row-link Parquet");
    let publication = publish_cell_embedding_row_link_parquet(&store, &row_link, budgets())
        .expect("publish row-link Parquet");

    let borrowed = validate_cell_embedding_row_link_parquet_bytes(
        &bytes,
        publication.record(),
        &expected,
        &row_link,
        budgets(),
    )
    .expect("borrowed validation");
    let forged_record = ArtifactRecord::new(
        ArtifactRef::new(
            publication.record().content().kind(),
            ContentDigest::from_bytes(b"different-same-length-record-digest"),
            publication.record().content().byte_len(),
        )
        .expect("forged content"),
        publication.record().schema().clone(),
        publication.record().table().cloned(),
        publication.record().dependencies().to_vec(),
        publication.record().semantic_metadata().clone(),
        publication.record().locations().to_vec(),
    )
    .expect("forged record");
    assert!(matches!(
        validate_cell_embedding_row_link_parquet_bytes(
            &bytes,
            &forged_record,
            &expected,
            &row_link,
            budgets(),
        ),
        Err(EmbeddingColumnarError::ArtifactBindingMismatch)
    ));
    let managed = validate_cell_embedding_row_link_parquet_from_store(
        &store,
        publication.record(),
        &expected,
        &row_link,
        budgets(),
    )
    .expect("managed validation");
    assert_eq!(managed, borrowed);
}

#[test]
fn borrowed_and_managed_row_link_parquet_reject_the_same_hostile_page_header() {
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("row-link-parquet-hostile").expect("store ID"),
    )
    .expect("store");
    let (expected, row_link) = fixture();
    let canonical = publish_cell_embedding_row_link_parquet(&store, &row_link, budgets())
        .expect("publish canonical row-link Parquet");
    let mut hostile = Vec::new();
    write_cell_embedding_row_link_parquet(&mut hostile, &row_link, budgets())
        .expect("write row-link Parquet");
    assert_eq!(hostile[4] & 0x0f, 5, "page type is i32");
    hostile[4] = (hostile[4] & 0xf0) | 8;
    let hostile_draft = external_record_for_bytes(canonical.record(), &hostile);
    let hostile_record = store
        .publish(&hostile_draft, |writer| writer.write_all(&hostile))
        .expect("publish hostile row-link Parquet")
        .into_record();

    let borrowed = validate_cell_embedding_row_link_parquet_bytes(
        &hostile,
        &hostile_draft,
        &expected,
        &row_link,
        budgets(),
    )
    .expect_err("borrowed hostile page");
    let managed = match validate_cell_embedding_row_link_parquet_from_store(
        &store,
        &hostile_record,
        &expected,
        &row_link,
        budgets(),
    ) {
        Err(VerifiedReaderError::Callback(error)) => error,
        result => panic!("expected managed callback failure, observed {result:?}"),
    };

    assert_eq!(managed, borrowed);
    assert!(matches!(
        managed,
        EmbeddingColumnarError::Parquet {
            reason: marklab::ParquetFailure::InvalidPageHeader,
        }
    ));
}

#[test]
fn row_link_parquet_managed_reader_prioritizes_store_integrity_failure() {
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("row-link-parquet-integrity").expect("store ID"),
    )
    .expect("store");
    let (expected, row_link) = fixture();
    let publication = publish_cell_embedding_row_link_parquet(&store, &row_link, budgets())
        .expect("publish row link");
    let path = root
        .path()
        .join(publication.record().locations()[0].key().as_str());
    let mut bytes = fs::read(&path).expect("managed row-link bytes");
    bytes[0] ^= 1;
    fs::write(path, bytes).expect("tamper managed row link");

    assert!(matches!(
        validate_cell_embedding_row_link_parquet_from_store(
            &store,
            publication.record(),
            &expected,
            &row_link,
            budgets(),
        ),
        Err(VerifiedReaderError::Store(
            marklab::ArtifactStoreError::ContentIntegrity { .. }
        ))
    ));
}

#[test]
fn row_link_parquet_reader_rejects_cell_and_source_row_drift_after_preflight() {
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("row-link-parquet-drift").expect("store ID"),
    )
    .expect("store");
    let (expected, row_link) = fixture();
    let mut canonical = Vec::new();
    write_cell_embedding_row_link_parquet(&mut canonical, &row_link, budgets())
        .expect("write row link");
    let publication = publish_cell_embedding_row_link_parquet(&store, &row_link, budgets())
        .expect("publish row link");

    let mut wrong_cell = canonical.clone();
    assert_eq!(
        replace_all_same_length(&mut wrong_cell, b"cell-a", b"cell-z"),
        1
    );
    assert!(matches!(
        preflight_cell_embedding_row_link_parquet_bytes(
            &wrong_cell,
            &expected,
            &row_link,
            budgets(),
        ),
        Err(EmbeddingColumnarError::Parquet {
            reason: marklab::ParquetFailure::InvalidCellOrder,
        })
    ));
    assert!(matches!(
        validate_cell_embedding_row_link_parquet_bytes(
            &wrong_cell,
            &record_for_bytes(publication.record(), &wrong_cell),
            &expected,
            &row_link,
            budgets(),
        ),
        Err(EmbeddingColumnarError::Parquet {
            reason: marklab::ParquetFailure::InvalidCellOrder,
        })
    ));

    let source_offset = unique_pattern_offset(
        &canonical,
        column_chunk_range(&canonical, 1),
        2_u64.to_le_bytes(),
    );
    let mut wrong_source = canonical.clone();
    wrong_source[source_offset..source_offset + 8].copy_from_slice(&9_u64.to_le_bytes());
    assert!(matches!(
        validate_cell_embedding_row_link_parquet_bytes(
            &wrong_source,
            &record_for_bytes(publication.record(), &wrong_source),
            &expected,
            &row_link,
            budgets(),
        ),
        Err(EmbeddingColumnarError::Parquet {
            reason: marklab::ParquetFailure::InvalidComponent,
        })
    ));

    let embedding_offset = unique_pattern_offset(
        &canonical,
        column_chunk_range(&canonical, 2),
        1_u64.to_le_bytes(),
    );
    let mut wrong_embedding = canonical;
    wrong_embedding[embedding_offset..embedding_offset + 8].copy_from_slice(&9_u64.to_le_bytes());
    assert!(matches!(
        validate_cell_embedding_row_link_parquet_bytes(
            &wrong_embedding,
            &record_for_bytes(publication.record(), &wrong_embedding),
            &expected,
            &row_link,
            budgets(),
        ),
        Err(EmbeddingColumnarError::Parquet {
            reason: marklab::ParquetFailure::InvalidStatus,
        })
    ));
}

#[test]
fn row_link_parquet_uses_exact_empty_and_8192_row_group_boundaries() {
    for (row_count, expected_groups) in [(0, 0), (1, 1), (8_191, 1), (8_192, 1), (8_193, 2)] {
        let (expected, row_link) = fixture_with_rows(row_count);
        let mut bytes = Vec::new();
        let summary = write_cell_embedding_row_link_parquet(&mut bytes, &row_link, scale_budgets())
            .expect("write boundary row link");
        let preflight = preflight_cell_embedding_row_link_parquet_bytes(
            &bytes,
            &expected,
            &row_link,
            scale_budgets(),
        )
        .expect("preflight boundary row link");
        assert_eq!(summary.row_count(), row_count as u64);
        assert_eq!(preflight.row_group_count(), expected_groups);
        if row_count == 8_192 {
            assert_eq!(
                summary.content_digest().to_string(),
                "9af303ea0a54f5e993b678c9e81dd7dee0fc9fd852bd6926a84e09962be3dc52"
            );
        }
    }
}

#[test]
fn row_link_parquet_writer_and_preflight_share_the_exact_decoded_budget() {
    let (expected, row_link) = fixture();
    let zero_decoded =
        EmbeddingColumnarBudgets::new(8 * 1024 * 1024, 16 * 1024 * 1024, 8 * 1024 * 1024, 0);
    let writer_required =
        match write_cell_embedding_row_link_parquet(&mut Vec::new(), &row_link, zero_decoded) {
            Err(EmbeddingColumnarError::DecodedByteBudgetExceeded { required, .. }) => required,
            result => panic!("expected writer decoded budget failure, observed {result:?}"),
        };
    let mut bytes = Vec::new();
    write_cell_embedding_row_link_parquet(&mut bytes, &row_link, budgets())
        .expect("write row link");
    let preflight_required = match preflight_cell_embedding_row_link_parquet_bytes(
        &bytes,
        &expected,
        &row_link,
        zero_decoded,
    ) {
        Err(EmbeddingColumnarError::DecodedByteBudgetExceeded { required, .. }) => required,
        result => panic!("expected preflight decoded budget failure, observed {result:?}"),
    };
    assert_eq!(preflight_required, writer_required);

    let exact = EmbeddingColumnarBudgets::new(
        8 * 1024 * 1024,
        16 * 1024 * 1024,
        8 * 1024 * 1024,
        writer_required,
    );
    write_cell_embedding_row_link_parquet(&mut Vec::new(), &row_link, exact)
        .expect("exact writer decoded budget");
    preflight_cell_embedding_row_link_parquet_bytes(&bytes, &expected, &row_link, exact)
        .expect("exact preflight decoded budget");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn row_link_arrow_and_parquet_profiles_preserve_the_same_logical_rows(
        row_count in 0_usize..256,
    ) {
        let (expected, row_link) = fixture_with_rows(row_count);
        let mut arrow = Vec::new();
        let arrow_summary = write_cell_embedding_row_link_arrow(
            &mut arrow,
            &row_link,
            scale_budgets(),
        )
        .expect("write Arrow row link");
        let mut parquet = Vec::new();
        let parquet_summary = write_cell_embedding_row_link_parquet(
            &mut parquet,
            &row_link,
            scale_budgets(),
        )
        .expect("write Parquet row link");
        let arrow_preflight = preflight_cell_embedding_row_link_arrow_bytes(
            &arrow,
            &expected,
            &row_link,
            scale_budgets(),
        )
        .expect("preflight Arrow row link");
        let parquet_preflight = preflight_cell_embedding_row_link_parquet_bytes(
            &parquet,
            &expected,
            &row_link,
            scale_budgets(),
        )
        .expect("preflight Parquet row link");

        prop_assert_eq!(arrow_summary.row_count(), parquet_summary.row_count());
        prop_assert_eq!(arrow_preflight.row_count(), parquet_preflight.row_count());
        prop_assert_eq!(arrow_preflight.row_count(), row_link.row_count());
    }
}
