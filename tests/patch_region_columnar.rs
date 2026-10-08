#![cfg(feature = "parquet")]

#[allow(dead_code, unused_imports)]
#[path = "patch_region_link/support.rs"]
mod support;

use std::{fs, io, io::Write, process::Command};

use marklab::{
    preflight_patch_region_link_arrow_bytes, preflight_patch_region_link_parquet_bytes,
    publish_patch_region_link_arrow, publish_patch_region_link_parquet,
    validate_patch_region_link_arrow_bytes, validate_patch_region_link_arrow_from_store,
    validate_patch_region_link_parquet_bytes, validate_patch_region_link_parquet_from_store,
    write_patch_region_link_arrow, write_patch_region_link_parquet, ArtifactRecord, ContentDigest,
    EmbeddingColumnarBudgets, LocalArtifactStore, PatchRegionAssessment, PatchRegionDeclaration,
    PatchRegionLink, PublicationDisposition, StoreId, TableColumnType, TableFormat,
    TableScalarType,
};
use tempfile::TempDir;

fn budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        32 * 1024 * 1024,
        32 * 1024 * 1024,
        32 * 1024 * 1024,
        32 * 1024 * 1024,
    )
}

fn link() -> PatchRegionLink {
    let fixture = support::fixture();
    let assessment = PatchRegionAssessment::new(
        &fixture.expected_patches,
        &fixture.expected_regions,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        support::declarations(),
        support::BUDGET,
        support::BUDGET,
    )
    .expect("assessment");
    let assessment_bytes = assessment.to_canonical_json().expect("assessment bytes");
    PatchRegionLink::from_exhaustive_assessment(
        &assessment,
        support::artifact(b"patch-region-physical-assessment"),
        ContentDigest::from_bytes(&assessment_bytes),
        support::BUDGET,
        support::BUDGET,
    )
    .expect("patch-region link")
}

fn empty_sparse_link() -> PatchRegionLink {
    let fixture = support::fixture();
    let assessment = PatchRegionAssessment::new(
        &fixture.expected_patches,
        &fixture.expected_regions,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        Vec::new(),
        support::BUDGET,
        support::BUDGET,
    )
    .expect("empty sparse assessment");
    let assessment_bytes = assessment.to_canonical_json().expect("assessment bytes");
    PatchRegionLink::from_exhaustive_assessment(
        &assessment,
        support::artifact(b"patch-region-empty-physical-assessment"),
        ContentDigest::from_bytes(&assessment_bytes),
        support::BUDGET,
        support::BUDGET,
    )
    .expect("empty sparse patch-region link")
}

fn high_bit_fraction_link() -> PatchRegionLink {
    let fixture = support::fixture();
    let declaration = PatchRegionDeclaration::partial_overlap(
        support::patch("patch-a"),
        support::region("region-a"),
        u64::MAX - 1,
        u64::MAX,
    )
    .expect("high-bit reduced fraction");
    let assessment = PatchRegionAssessment::new(
        &fixture.expected_patches,
        &fixture.expected_regions,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        vec![declaration],
        support::BUDGET,
        support::BUDGET,
    )
    .expect("high-bit assessment");
    let assessment_bytes = assessment.to_canonical_json().expect("assessment bytes");
    PatchRegionLink::from_exhaustive_assessment(
        &assessment,
        support::artifact(b"patch-region-high-bit-assessment"),
        ContentDigest::from_bytes(&assessment_bytes),
        support::BUDGET,
        support::BUDGET,
    )
    .expect("high-bit patch-region link")
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

fn assert_exact_manifest(
    record: &ArtifactRecord,
    format: TableFormat,
    encoding: &str,
    kind: &str,
    link: &PatchRegionLink,
) {
    assert_eq!(record.schema().id(), "marklab.patch_region_link");
    assert_eq!(record.schema().version(), 1);
    assert_eq!(record.content().kind(), kind);
    assert!(record.semantic_metadata().is_empty());
    let table = record.table().expect("table manifest");
    assert_eq!(table.format(), format);
    assert_eq!(table.encoding_version(), encoding);
    assert_eq!(table.row_count(), 3);
    assert_eq!(table.columns().len(), 5);
    for (index, (name, scalar)) in [
        ("patch_id", TableScalarType::Utf8),
        ("region_id", TableScalarType::Utf8),
        ("relation", TableScalarType::Utf8),
        ("overlap_numerator", TableScalarType::U64),
        ("overlap_denominator", TableScalarType::U64),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(table.columns()[index].name(), name);
        assert_eq!(
            table.columns()[index].column_type(),
            &TableColumnType::Scalar(scalar)
        );
        assert!(!table.columns()[index].nullable());
    }
    assert_eq!(table.primary_key(), ["patch_id", "region_id"]);
    let mut expected = [
        link.expected_patches_artifact_id(),
        link.expected_regions_artifact_id(),
        link.patch_context_artifact_id(),
        link.patch_footprints_artifact_id(),
        link.converter_artifact_id(),
        link.assessment_artifact_id(),
    ];
    expected.sort_unstable();
    assert_eq!(record.dependencies(), expected);
}

#[test]
fn patch_region_arrow_writer_and_raw_preflight_preserve_exact_rows() {
    let link = link();
    let mut bytes = Vec::new();
    let summary = write_patch_region_link_arrow(&mut bytes, &link, budgets())
        .expect("write patch-region Arrow");
    assert_eq!(summary.row_count(), 3);
    assert_eq!(summary.encoded_byte_len(), bytes.len() as u64);
    assert_eq!(summary.content_digest(), ContentDigest::from_bytes(&bytes));

    let preflight = preflight_patch_region_link_arrow_bytes(&bytes, &link, budgets())
        .expect("preflight patch-region Arrow");
    assert_eq!(preflight.row_count(), 3);
    assert_eq!(preflight.record_batch_count(), 1);
    assert_eq!(preflight.content_digest(), summary.content_digest());
}

#[test]
fn patch_region_parquet_writer_and_raw_preflight_preserve_exact_rows() {
    let link = link();
    let mut bytes = Vec::new();
    let summary = write_patch_region_link_parquet(&mut bytes, &link, budgets())
        .expect("write patch-region Parquet");
    assert_eq!(summary.row_count(), 3);
    assert_eq!(summary.encoded_byte_len(), bytes.len() as u64);
    assert_eq!(summary.content_digest(), ContentDigest::from_bytes(&bytes));

    let preflight = preflight_patch_region_link_parquet_bytes(&bytes, &link, budgets())
        .expect("preflight patch-region Parquet");
    assert_eq!(preflight.row_count(), 3);
    assert_eq!(preflight.row_group_count(), 1);
    assert_eq!(preflight.content_digest(), summary.content_digest());
}

#[test]
fn patch_region_profiles_preserve_an_empty_sparse_link_with_positive_assessed_count() {
    let link = empty_sparse_link();
    assert_eq!(link.assessed_pair_count(), 4);
    assert_eq!(link.nonzero_relation_count(), 0);
    let mut arrow = Vec::new();
    let arrow_summary =
        write_patch_region_link_arrow(&mut arrow, &link, budgets()).expect("empty Arrow");
    let arrow_preflight = preflight_patch_region_link_arrow_bytes(&arrow, &link, budgets())
        .expect("empty Arrow preflight");
    assert_eq!(arrow_summary.row_count(), 0);
    assert_eq!(arrow_preflight.row_count(), 0);
    assert_eq!(arrow_preflight.record_batch_count(), 0);

    let mut parquet = Vec::new();
    let parquet_summary =
        write_patch_region_link_parquet(&mut parquet, &link, budgets()).expect("empty Parquet");
    let parquet_preflight = preflight_patch_region_link_parquet_bytes(&parquet, &link, budgets())
        .expect("empty Parquet preflight");
    assert_eq!(parquet_summary.row_count(), 0);
    assert_eq!(parquet_preflight.row_count(), 0);
    assert_eq!(parquet_preflight.row_group_count(), 0);
}

#[test]
fn patch_region_profiles_preserve_unsigned_high_bit_fraction_values() {
    let link = high_bit_fraction_link();
    let row = &link.nonzero_relations()[0];
    assert_eq!(row.numerator(), u64::MAX - 1);
    assert_eq!(row.denominator(), u64::MAX);
    let mut arrow = Vec::new();
    write_patch_region_link_arrow(&mut arrow, &link, budgets()).expect("high-bit Arrow");
    preflight_patch_region_link_arrow_bytes(&arrow, &link, budgets())
        .expect("high-bit Arrow preflight");
    let mut parquet = Vec::new();
    write_patch_region_link_parquet(&mut parquet, &link, budgets()).expect("high-bit Parquet");
    preflight_patch_region_link_parquet_bytes(&parquet, &link, budgets())
        .expect("high-bit Parquet preflight");
}

#[test]
fn patch_region_publications_have_exact_records_and_full_reader_parity() {
    let link = link();
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("patch-region-columnar").expect("store ID"),
    )
    .expect("store");
    let arrow = publish_patch_region_link_arrow(&store, &link, budgets())
        .expect("publish Arrow")
        .into_record();
    let parquet = publish_patch_region_link_parquet(&store, &link, budgets())
        .expect("publish Parquet")
        .into_record();
    assert_exact_manifest(
        &arrow,
        TableFormat::ArrowIpcFile,
        "marklab.arrow-ipc.patch-region-link.v1",
        "application/vnd.marklab.patch-region-link.v1+arrow",
        &link,
    );
    assert_exact_manifest(
        &parquet,
        TableFormat::ParquetFile,
        "marklab.parquet.patch-region-link.v1",
        "application/vnd.marklab.patch-region-link.v1+parquet",
        &link,
    );
    assert_ne!(arrow.id(), parquet.id());
    let repeated = publish_patch_region_link_arrow(&store, &link, budgets())
        .expect("repeat Arrow publication");
    assert_eq!(
        repeated.disposition(),
        PublicationDisposition::AlreadyPresent
    );
    assert_eq!(repeated.record().id(), arrow.id());

    let mut arrow_bytes = Vec::new();
    write_patch_region_link_arrow(&mut arrow_bytes, &link, budgets()).expect("Arrow bytes");
    let mut parquet_bytes = Vec::new();
    write_patch_region_link_parquet(&mut parquet_bytes, &link, budgets()).expect("Parquet bytes");
    assert_eq!(
        validate_patch_region_link_arrow_bytes(&arrow_bytes, &arrow, &link, budgets())
            .expect("borrowed Arrow"),
        validate_patch_region_link_arrow_from_store(&store, &arrow, &link, budgets())
            .expect("managed Arrow")
    );
    assert_eq!(
        validate_patch_region_link_parquet_bytes(&parquet_bytes, &parquet, &link, budgets())
            .expect("borrowed Parquet"),
        validate_patch_region_link_parquet_from_store(&store, &parquet, &link, budgets())
            .expect("managed Parquet")
    );
}

#[test]
fn patch_region_writers_are_deterministic_and_support_fragmented_sinks() {
    let link = link();
    macro_rules! assert_writer {
        ($writer:path) => {{
            let mut first = Vec::new();
            $writer(&mut first, &link, budgets()).expect("first write");
            let mut second = Vec::new();
            $writer(&mut second, &link, budgets()).expect("second write");
            let mut fragmented = FragmentingWriter::default();
            $writer(&mut fragmented, &link, budgets()).expect("fragmented write");
            assert_eq!(first, second);
            assert_eq!(first, fragmented.bytes);
        }};
    }
    assert_writer!(write_patch_region_link_arrow);
    assert_writer!(write_patch_region_link_parquet);
}

#[test]
fn patch_region_writers_are_deterministic_across_fresh_processes() {
    let temporary = TempDir::new().expect("temporary directory");
    let executable = std::env::current_exe().expect("test executable");
    let first = temporary.path().join("first.bin");
    let second = temporary.path().join("second.bin");
    for path in [&first, &second] {
        let status = Command::new(&executable)
            .arg("--exact")
            .arg("patch_region_determinism_child")
            .env("MARKLAB_PATCH_REGION_COLUMNAR_CHILD", path)
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
fn patch_region_determinism_child() {
    let Some(path) = std::env::var_os("MARKLAB_PATCH_REGION_COLUMNAR_CHILD") else {
        return;
    };
    let link = link();
    let mut combined = Vec::new();
    macro_rules! append_writer {
        ($writer:path) => {{
            let mut bytes = Vec::new();
            $writer(&mut bytes, &link, budgets()).expect("child canonical write");
            combined.extend_from_slice(
                &u64::try_from(bytes.len())
                    .expect("encoded length")
                    .to_le_bytes(),
            );
            combined.extend_from_slice(&bytes);
        }};
    }
    append_writer!(write_patch_region_link_arrow);
    append_writer!(write_patch_region_link_parquet);
    fs::write(path, combined).expect("child output");
}
