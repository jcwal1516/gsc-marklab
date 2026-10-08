#![cfg(feature = "parquet")]

#[path = "support/multiscale_matrix_columnar.rs"]
mod support;

use std::{
    fs,
    io::{self, Write},
    process::Command,
};

use marklab::{
    preflight_patch_embedding_table_arrow_bytes, preflight_patch_embedding_table_parquet_bytes,
    preflight_region_embedding_table_arrow_bytes, preflight_region_embedding_table_parquet_bytes,
    preflight_slide_embedding_table_arrow_bytes, preflight_slide_embedding_table_parquet_bytes,
    publish_patch_embedding_table_arrow, publish_patch_embedding_table_parquet,
    publish_region_embedding_table_arrow, publish_region_embedding_table_parquet,
    publish_slide_embedding_table_arrow, publish_slide_embedding_table_parquet,
    validate_patch_embedding_table_arrow_bytes, validate_patch_embedding_table_arrow_from_store,
    validate_patch_embedding_table_parquet_bytes,
    validate_patch_embedding_table_parquet_from_store, validate_region_embedding_table_arrow_bytes,
    validate_region_embedding_table_arrow_from_store,
    validate_region_embedding_table_parquet_bytes,
    validate_region_embedding_table_parquet_from_store, validate_slide_embedding_table_arrow_bytes,
    validate_slide_embedding_table_arrow_from_store, validate_slide_embedding_table_parquet_bytes,
    validate_slide_embedding_table_parquet_from_store, write_patch_embedding_table_arrow,
    write_patch_embedding_table_parquet, write_region_embedding_table_arrow,
    write_region_embedding_table_parquet, write_slide_embedding_table_arrow,
    write_slide_embedding_table_parquet, ArtifactDraft, ArtifactId, ArtifactRecord, ArtifactRef,
    ArtifactStoreError, ContentDigest, EmbeddingColumnarBudgets, EmbeddingEntityKind,
    LocalArtifactStore, PublicationDisposition, StoreId, TableColumnType, TableFormat,
    TableScalarType, VerifiedReaderError,
};
use tempfile::TempDir;

use support::{artifact_id, matrix_fixture, patch_table_with_rows, LARGE};

fn budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(LARGE as u64, LARGE, LARGE, LARGE as u64)
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
        let written = maximum.min(buffer.len());
        self.bytes.extend_from_slice(&buffer[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct ExpectedRecordProfile<'a> {
    format: TableFormat,
    encoding: &'a str,
    kind: &'a str,
    schema_id: &'a str,
    id_column: &'a str,
    row_count: u64,
    dimension: u32,
    dependencies: [ArtifactId; 3],
}

fn assert_record(record: &ArtifactRecord, mut profile: ExpectedRecordProfile<'_>) {
    profile.dependencies.sort_unstable();
    assert_eq!(record.schema().id(), profile.schema_id);
    assert_eq!(record.schema().version(), 1);
    assert_eq!(record.content().kind(), profile.kind);
    assert_eq!(record.dependencies(), profile.dependencies);
    assert!(record.semantic_metadata().is_empty());
    let manifest = record.table().expect("matrix manifest");
    assert_eq!(manifest.format(), profile.format);
    assert_eq!(manifest.encoding_version(), profile.encoding);
    assert_eq!(manifest.row_count(), profile.row_count);
    assert_eq!(manifest.primary_key(), [profile.id_column]);
    assert_eq!(manifest.columns().len(), 3);
    assert_eq!(manifest.columns()[0].name(), profile.id_column);
    assert_eq!(
        manifest.columns()[0].column_type(),
        &TableColumnType::Scalar(TableScalarType::Utf8)
    );
    assert_eq!(manifest.columns()[1].name(), "embedding");
    assert_eq!(
        manifest.columns()[1].column_type(),
        &TableColumnType::FixedSizeList {
            element: TableScalarType::F32,
            length: profile.dimension,
        }
    );
    assert_eq!(manifest.columns()[2].name(), "embedding_status");
    assert_eq!(
        manifest.columns()[2].column_type(),
        &TableColumnType::Scalar(TableScalarType::Utf8)
    );
    assert!(manifest.columns().iter().all(|column| !column.nullable()));
}

macro_rules! assert_profile {
    ($table:expr, $kind:expr, $arrow_writer:path, $arrow_preflight:path,
        $parquet_writer:path, $parquet_preflight:path) => {{
        let table = $table;
        let mut arrow = Vec::new();
        let arrow_summary = $arrow_writer(&mut arrow, table, budgets()).expect("Arrow writer");
        let arrow_preflight = $arrow_preflight(&arrow, table, budgets()).expect("Arrow preflight");
        assert_eq!(arrow_summary.row_count(), table.row_count() as u64);
        assert_eq!(arrow_summary.dimension(), table.dimension());
        assert_eq!(arrow_preflight.entity_kind(), $kind);
        assert_eq!(arrow_preflight.row_count(), table.row_count() as u64);
        assert_eq!(arrow_preflight.dimension(), table.dimension());
        assert_eq!(arrow_preflight.qc_summary(), table.qc_summary());
        assert_eq!(
            arrow_preflight.content_digest(),
            arrow_summary.content_digest()
        );
        assert_eq!(arrow_preflight.encoded_byte_len(), arrow.len() as u64);

        let mut parquet = Vec::new();
        let parquet_summary =
            $parquet_writer(&mut parquet, table, budgets()).expect("Parquet writer");
        let parquet_preflight =
            $parquet_preflight(&parquet, table, budgets()).expect("Parquet preflight");
        assert_eq!(parquet_summary.row_count(), table.row_count() as u64);
        assert_eq!(parquet_summary.dimension(), table.dimension());
        assert_eq!(parquet_preflight.entity_kind(), $kind);
        assert_eq!(parquet_preflight.row_count(), table.row_count() as u64);
        assert_eq!(parquet_preflight.dimension(), table.dimension());
        assert_eq!(parquet_preflight.qc_summary(), table.qc_summary());
        assert_eq!(
            parquet_preflight.content_digest(),
            parquet_summary.content_digest()
        );
        assert_eq!(parquet_preflight.encoded_byte_len(), parquet.len() as u64);
    }};
}

#[test]
fn all_three_matrix_profiles_preserve_exact_typed_rows_and_qc() {
    let fixture = matrix_fixture();
    assert_eq!(
        fixture.expected_patches.ids().len(),
        fixture.patch_table.row_count()
    );
    assert_eq!(
        fixture.expected_regions.ids().len(),
        fixture.region_table.row_count()
    );
    assert_eq!(
        fixture.expected_slides.ids().len(),
        fixture.slide_table.row_count()
    );
    assert_profile!(
        &fixture.patch_table,
        EmbeddingEntityKind::Patch,
        write_patch_embedding_table_arrow,
        preflight_patch_embedding_table_arrow_bytes,
        write_patch_embedding_table_parquet,
        preflight_patch_embedding_table_parquet_bytes
    );
    assert_profile!(
        &fixture.region_table,
        EmbeddingEntityKind::Region,
        write_region_embedding_table_arrow,
        preflight_region_embedding_table_arrow_bytes,
        write_region_embedding_table_parquet,
        preflight_region_embedding_table_parquet_bytes
    );
    assert_profile!(
        &fixture.slide_table,
        EmbeddingEntityKind::Slide,
        write_slide_embedding_table_arrow,
        preflight_slide_embedding_table_arrow_bytes,
        write_slide_embedding_table_parquet,
        preflight_slide_embedding_table_parquet_bytes
    );
}

macro_rules! assert_publication {
    ($store:expr, $table:expr, $publisher:path, $writer:path, $borrowed:path, $managed:path,
        $format:expr, $encoding:expr, $kind:expr, $schema:expr, $id_column:expr) => {{
        let table = $table;
        let record = $publisher($store, table, budgets())
            .expect("matrix publication")
            .into_record();
        assert_record(
            &record,
            ExpectedRecordProfile {
                format: $format,
                encoding: $encoding,
                kind: $kind,
                schema_id: $schema,
                id_column: $id_column,
                row_count: table.row_count() as u64,
                dimension: table.dimension(),
                dependencies: [
                    table.expected_entities_artifact_id(),
                    table.support_artifact_id(),
                    table.provenance_artifact_id(),
                ],
            },
        );
        let repeated = $publisher($store, table, budgets()).expect("repeat publication");
        assert_eq!(
            repeated.disposition(),
            PublicationDisposition::AlreadyPresent
        );
        assert_eq!(repeated.record().id(), record.id());
        let mut bytes = Vec::new();
        $writer(&mut bytes, table, budgets()).expect("matrix bytes");
        let borrowed = $borrowed(&bytes, &record, table, budgets()).expect("borrowed matrix");
        let managed = $managed($store, &record, table, budgets()).expect("managed matrix");
        assert_eq!(borrowed, managed);
        assert_eq!(borrowed.qc_summary(), table.qc_summary());
    }};
}

#[test]
fn all_matrix_publications_have_exact_records_and_full_reader_parity() {
    let fixture = matrix_fixture();
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("multiscale-matrix-columnar").expect("store ID"),
    )
    .expect("store");
    assert_publication!(
        &store,
        &fixture.patch_table,
        publish_patch_embedding_table_arrow,
        write_patch_embedding_table_arrow,
        validate_patch_embedding_table_arrow_bytes,
        validate_patch_embedding_table_arrow_from_store,
        TableFormat::ArrowIpcFile,
        "marklab.arrow-ipc.patch-embedding-table.v1",
        "application/vnd.marklab.patch-embedding-table.v1+arrow",
        "marklab.patch_embedding_table",
        "patch_id"
    );
    assert_publication!(
        &store,
        &fixture.patch_table,
        publish_patch_embedding_table_parquet,
        write_patch_embedding_table_parquet,
        validate_patch_embedding_table_parquet_bytes,
        validate_patch_embedding_table_parquet_from_store,
        TableFormat::ParquetFile,
        "marklab.parquet.patch-embedding-table.v1",
        "application/vnd.marklab.patch-embedding-table.v1+parquet",
        "marklab.patch_embedding_table",
        "patch_id"
    );
    assert_publication!(
        &store,
        &fixture.region_table,
        publish_region_embedding_table_arrow,
        write_region_embedding_table_arrow,
        validate_region_embedding_table_arrow_bytes,
        validate_region_embedding_table_arrow_from_store,
        TableFormat::ArrowIpcFile,
        "marklab.arrow-ipc.region-embedding-table.v1",
        "application/vnd.marklab.region-embedding-table.v1+arrow",
        "marklab.region_embedding_table",
        "region_id"
    );
    assert_publication!(
        &store,
        &fixture.region_table,
        publish_region_embedding_table_parquet,
        write_region_embedding_table_parquet,
        validate_region_embedding_table_parquet_bytes,
        validate_region_embedding_table_parquet_from_store,
        TableFormat::ParquetFile,
        "marklab.parquet.region-embedding-table.v1",
        "application/vnd.marklab.region-embedding-table.v1+parquet",
        "marklab.region_embedding_table",
        "region_id"
    );
    assert_publication!(
        &store,
        &fixture.slide_table,
        publish_slide_embedding_table_arrow,
        write_slide_embedding_table_arrow,
        validate_slide_embedding_table_arrow_bytes,
        validate_slide_embedding_table_arrow_from_store,
        TableFormat::ArrowIpcFile,
        "marklab.arrow-ipc.slide-embedding-table.v1",
        "application/vnd.marklab.slide-embedding-table.v1+arrow",
        "marklab.slide_embedding_table",
        "slide_id"
    );
    assert_publication!(
        &store,
        &fixture.slide_table,
        publish_slide_embedding_table_parquet,
        write_slide_embedding_table_parquet,
        validate_slide_embedding_table_parquet_bytes,
        validate_slide_embedding_table_parquet_from_store,
        TableFormat::ParquetFile,
        "marklab.parquet.slide-embedding-table.v1",
        "application/vnd.marklab.slide-embedding-table.v1+parquet",
        "marklab.slide_embedding_table",
        "slide_id"
    );
}

#[test]
fn all_matrix_writers_are_deterministic_and_support_fragmented_sinks() {
    let fixture = matrix_fixture();
    macro_rules! assert_writer {
        ($writer:path, $table:expr) => {{
            let mut first = Vec::new();
            $writer(&mut first, $table, budgets()).expect("first write");
            let mut second = Vec::new();
            $writer(&mut second, $table, budgets()).expect("second write");
            let mut fragmented = FragmentingWriter::default();
            $writer(&mut fragmented, $table, budgets()).expect("fragmented write");
            assert_eq!(first, second);
            assert_eq!(first, fragmented.bytes);
        }};
    }
    assert_writer!(write_patch_embedding_table_arrow, &fixture.patch_table);
    assert_writer!(write_patch_embedding_table_parquet, &fixture.patch_table);
    assert_writer!(write_region_embedding_table_arrow, &fixture.region_table);
    assert_writer!(write_region_embedding_table_parquet, &fixture.region_table);
    assert_writer!(write_slide_embedding_table_arrow, &fixture.slide_table);
    assert_writer!(write_slide_embedding_table_parquet, &fixture.slide_table);
}

fn matrix_encoding_bytes() -> Vec<(&'static str, Vec<u8>)> {
    let fixture = matrix_fixture();
    let mut outputs = Vec::new();
    macro_rules! collect {
        ($name:literal, $writer:path, $table:expr) => {{
            let mut bytes = Vec::new();
            $writer(&mut bytes, $table, budgets()).expect("canonical matrix write");
            outputs.push(($name, bytes));
        }};
    }
    collect!(
        "patch-arrow",
        write_patch_embedding_table_arrow,
        &fixture.patch_table
    );
    collect!(
        "patch-parquet",
        write_patch_embedding_table_parquet,
        &fixture.patch_table
    );
    collect!(
        "region-arrow",
        write_region_embedding_table_arrow,
        &fixture.region_table
    );
    collect!(
        "region-parquet",
        write_region_embedding_table_parquet,
        &fixture.region_table
    );
    collect!(
        "slide-arrow",
        write_slide_embedding_table_arrow,
        &fixture.slide_table
    );
    collect!(
        "slide-parquet",
        write_slide_embedding_table_parquet,
        &fixture.slide_table
    );
    outputs
}

#[test]
fn all_matrix_encodings_have_stable_golden_identities() {
    let observed = matrix_encoding_bytes()
        .iter()
        .map(|(name, bytes)| {
            (
                *name,
                bytes.len() as u64,
                ContentDigest::from_bytes(bytes).to_string(),
            )
        })
        .collect::<Vec<_>>();
    let expected = [
        (
            "patch-arrow",
            3_394,
            "5db36dbb4b430dc13bf20b4674d7d54489a08183ec34436c101f472090c7926e".to_owned(),
        ),
        (
            "patch-parquet",
            1_224,
            "93dba0b4f55c93d3870aac853cff1c4aebfe7b9d443f93886c204e3ea051de1a".to_owned(),
        ),
        (
            "region-arrow",
            3_394,
            "70802935d5585917d7607609721bf99a2da39763de6c816a516bb91281404fa4".to_owned(),
        ),
        (
            "region-parquet",
            1_133,
            "68f71780e528de67214dc4d707f669a7c4bbc9ba9b65c2ea9e283735497725db".to_owned(),
        ),
        (
            "slide-arrow",
            3_394,
            "fadc6f04d785a619156b144e988fe83bed75ae204c3abc8cd66c4de510e5d0c4".to_owned(),
        ),
        (
            "slide-parquet",
            1_096,
            "644856f69f0fa323fb20c39ebc97f9afd63ad47489eb97209e02b9e43d8959a4".to_owned(),
        ),
    ];
    assert_eq!(observed, expected);
}

#[test]
fn all_matrix_writers_are_deterministic_across_fresh_processes() {
    let temporary = TempDir::new().expect("temporary directory");
    let executable = std::env::current_exe().expect("test executable");
    let first = temporary.path().join("first.bin");
    let second = temporary.path().join("second.bin");
    for path in [&first, &second] {
        let status = Command::new(&executable)
            .arg("--exact")
            .arg("multiscale_matrix_columnar_determinism_child")
            .env("MARKLAB_MULTISCALE_MATRIX_CHILD", path)
            .status()
            .expect("fresh child");
        assert!(status.success());
    }
    assert_eq!(
        fs::read(first).expect("first child output"),
        fs::read(second).expect("second child output")
    );
}

#[test]
fn multiscale_matrix_columnar_determinism_child() {
    let Some(path) = std::env::var_os("MARKLAB_MULTISCALE_MATRIX_CHILD") else {
        return;
    };
    let mut combined = Vec::new();
    for (_, bytes) in matrix_encoding_bytes() {
        combined.extend_from_slice(
            &u64::try_from(bytes.len())
                .expect("encoded length")
                .to_le_bytes(),
        );
        combined.extend_from_slice(&bytes);
    }
    fs::write(path, combined).expect("child output");
}

#[test]
fn matrix_managed_integrity_precedes_decode_and_callbacks_match_borrowed() {
    #[derive(Clone, Copy)]
    enum Format {
        Arrow,
        Parquet,
    }

    for format in [Format::Arrow, Format::Parquet] {
        let fixture = matrix_fixture();
        let table = &fixture.patch_table;
        let root = TempDir::new().expect("store root");
        let store = LocalArtifactStore::open(
            root.path(),
            StoreId::new(match format {
                Format::Arrow => "matrix-integrity-arrow",
                Format::Parquet => "matrix-integrity-parquet",
            })
            .expect("store ID"),
        )
        .expect("store");
        let canonical = match format {
            Format::Arrow => publish_patch_embedding_table_arrow(&store, table, budgets()),
            Format::Parquet => publish_patch_embedding_table_parquet(&store, table, budgets()),
        }
        .expect("publish canonical matrix")
        .into_record();
        let mut malformed = Vec::new();
        match format {
            Format::Arrow => write_patch_embedding_table_arrow(&mut malformed, table, budgets()),
            Format::Parquet => {
                write_patch_embedding_table_parquet(&mut malformed, table, budgets())
            }
        }
        .expect("write canonical matrix");
        let row_at = malformed
            .windows(b"patch-a".len())
            .position(|window| window == b"patch-a")
            .expect("physical row value");
        malformed[row_at] ^= 0x20;
        let draft = ArtifactDraft::new(
            ArtifactRef::from_bytes(canonical.content().kind(), &malformed)
                .expect("malformed content"),
            canonical.schema().clone(),
            canonical.table().cloned(),
            canonical.dependencies().to_vec(),
            Default::default(),
        )
        .expect("malformed draft");
        let malformed_record = store
            .publish_new_send(&draft, |output| output.write_all(&malformed))
            .expect("publish digest-matching malformed matrix")
            .into_record();

        let borrowed = match format {
            Format::Arrow => validate_patch_embedding_table_arrow_bytes(
                &malformed,
                &malformed_record,
                table,
                budgets(),
            )
            .map(|_| ()),
            Format::Parquet => validate_patch_embedding_table_parquet_bytes(
                &malformed,
                &malformed_record,
                table,
                budgets(),
            )
            .map(|_| ()),
        }
        .expect_err("borrowed malformed matrix");
        let managed = match format {
            Format::Arrow => validate_patch_embedding_table_arrow_from_store(
                &store,
                &malformed_record,
                table,
                budgets(),
            )
            .map(|_| ()),
            Format::Parquet => validate_patch_embedding_table_parquet_from_store(
                &store,
                &malformed_record,
                table,
                budgets(),
            )
            .map(|_| ()),
        };
        match managed {
            Err(VerifiedReaderError::Callback(error)) => assert_eq!(error, borrowed),
            observed => panic!("expected matching managed callback error, got {observed:?}"),
        }

        let path = root
            .path()
            .join(malformed_record.locations()[0].key().as_str());
        let mut corrupt = malformed;
        corrupt[0] ^= 1;
        fs::write(path, corrupt).expect("corrupt managed matrix");
        let integrity = match format {
            Format::Arrow => validate_patch_embedding_table_arrow_from_store(
                &store,
                &malformed_record,
                table,
                budgets(),
            )
            .map(|_| ()),
            Format::Parquet => validate_patch_embedding_table_parquet_from_store(
                &store,
                &malformed_record,
                table,
                budgets(),
            )
            .map(|_| ()),
        };
        assert!(matches!(
            integrity,
            Err(VerifiedReaderError::Store(
                ArtifactStoreError::ContentIntegrity { .. }
            ))
        ));
    }
}

#[test]
fn matrix_readers_reject_dependency_role_drift_before_decode() {
    let fixture = matrix_fixture();
    let table = &fixture.patch_table;
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("matrix-record-drift").expect("store ID"),
    )
    .expect("store");

    macro_rules! assert_drift {
        ($publisher:path, $writer:path, $validator:path) => {{
            let canonical = $publisher(&store, table, budgets())
                .expect("canonical publication")
                .into_record();
            let mut bytes = Vec::new();
            $writer(&mut bytes, table, budgets()).expect("canonical bytes");
            let mut dependencies = canonical.dependencies().to_vec();
            dependencies[0] = artifact_id(b"matrix-rogue-dependency");
            let draft = ArtifactDraft::new(
                ArtifactRef::from_bytes(canonical.content().kind(), &bytes).expect("drift content"),
                canonical.schema().clone(),
                canonical.table().cloned(),
                dependencies,
                Default::default(),
            )
            .expect("drifted draft");
            let drifted = store
                .publish_new_send(&draft, |output| output.write_all(&bytes))
                .expect("publish drifted declaration")
                .into_record();
            assert!(matches!(
                $validator(&bytes, &drifted, table, budgets()),
                Err(marklab::MultiscaleColumnarError::ArtifactBindingMismatch)
            ));
        }};
    }

    assert_drift!(
        publish_patch_embedding_table_arrow,
        write_patch_embedding_table_arrow,
        validate_patch_embedding_table_arrow_bytes
    );
    assert_drift!(
        publish_patch_embedding_table_parquet,
        write_patch_embedding_table_parquet,
        validate_patch_embedding_table_parquet_bytes
    );
}

#[test]
fn matrix_profiles_cover_empty_max_dimension_and_public_batch_boundaries() {
    for (row_count, dimension, groups) in [
        (0_usize, 65_536_u32, 0_u32),
        (8_192, 3, 1),
        (8_193, 3, 2),
        (16_386, 3, 3),
    ] {
        let table = patch_table_with_rows(row_count, dimension);
        let mut arrow = Vec::new();
        write_patch_embedding_table_arrow(&mut arrow, &table, budgets()).expect("Arrow boundary");
        let arrow_preflight =
            preflight_patch_embedding_table_arrow_bytes(&arrow, &table, budgets())
                .expect("Arrow boundary preflight");
        assert_eq!(arrow_preflight.row_count(), row_count as u64);
        assert_eq!(arrow_preflight.dimension(), dimension);
        assert_eq!(arrow_preflight.record_batch_count(), groups);

        let mut parquet = Vec::new();
        write_patch_embedding_table_parquet(&mut parquet, &table, budgets())
            .expect("Parquet boundary");
        let parquet_preflight =
            preflight_patch_embedding_table_parquet_bytes(&parquet, &table, budgets())
                .expect("Parquet boundary preflight");
        assert_eq!(parquet_preflight.row_count(), row_count as u64);
        assert_eq!(parquet_preflight.dimension(), dimension);
        assert_eq!(parquet_preflight.row_group_count(), groups);
    }
}

#[test]
fn maximum_dimension_one_row_round_trips_through_both_stock_decoders() {
    let table = patch_table_with_rows(1, 65_536);
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("matrix-maximum-dimension").expect("store ID"),
    )
    .expect("store");
    let arrow = publish_patch_embedding_table_arrow(&store, &table, budgets())
        .expect("publish maximum-dimension Arrow")
        .into_record();
    let parquet = publish_patch_embedding_table_parquet(&store, &table, budgets())
        .expect("publish maximum-dimension Parquet")
        .into_record();
    assert_eq!(
        validate_patch_embedding_table_arrow_from_store(&store, &arrow, &table, budgets())
            .expect("decode maximum-dimension Arrow")
            .dimension(),
        65_536
    );
    assert_eq!(
        validate_patch_embedding_table_parquet_from_store(&store, &parquet, &table, budgets())
            .expect("decode maximum-dimension Parquet")
            .dimension(),
        65_536
    );
}
