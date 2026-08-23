#![cfg(feature = "parquet")]

#[path = "support/multiscale_columnar.rs"]
mod support;

use std::{fs, io::Write, mem::size_of, process::Command};

use marklab::{
    preflight_patch_footprint_set_parquet_bytes, preflight_patch_overlap_graph_parquet_bytes,
    publish_patch_footprint_set_parquet, publish_patch_overlap_graph_parquet,
    validate_patch_footprint_set_parquet_bytes, validate_patch_footprint_set_parquet_from_store,
    validate_patch_overlap_graph_parquet_bytes, validate_patch_overlap_graph_parquet_from_store,
    write_patch_footprint_set_parquet, write_patch_overlap_graph_parquet, ContentDigest,
    EmbeddingColumnarBudgets, LocalArtifactStore, MultiscaleColumnarError, PatchOverlapGraph,
    PublicationDisposition, SpatialParquetFailure, StoreId, TableFormat,
};
use parquet::{
    file::metadata::ParquetMetaDataReader,
    format::{
        ColumnChunk, ColumnMetaData, ColumnOrder, Encoding, FileMetaData, KeyValue,
        PageEncodingStats, RowGroup, SchemaElement,
    },
};
use tempfile::TempDir;

use support::{fixture, reflecting_fixture, FragmentingWriter};

fn budgets(file_bytes: u64) -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        file_bytes,
        128 * 1024 * 1024,
        64 * 1024 * 1024,
        64 * 1024 * 1024,
    )
}

fn parquet_footer_start(bytes: &[u8]) -> usize {
    let length_offset = bytes.len() - 8;
    let length = u32::from_le_bytes(
        bytes[length_offset..length_offset + 4]
            .try_into()
            .expect("footer length"),
    ) as usize;
    length_offset - length
}

fn parquet_reader_requirements(bytes: &[u8]) -> (usize, usize) {
    const MAXIMUM_FOOTER_BYTES: usize = 1024 * 1024;
    const MAXIMUM_PAGE_HEADER_BYTES: usize = 64 * 1024;
    const READER_WORKSPACE: usize = 64 * 1024;

    let mut file = tempfile::tempfile().expect("temporary Parquet file");
    file.write_all(bytes)
        .expect("write trusted Parquet fixture");
    let metadata = ParquetMetaDataReader::new()
        .parse_and_finish(&file)
        .expect("parse trusted Parquet metadata");
    let groups = metadata.num_row_groups();
    let footer_length = bytes.len() - 8 - parquet_footer_start(bytes);
    let maximum_inline_element = [
        size_of::<SchemaElement>(),
        size_of::<RowGroup>(),
        size_of::<ColumnChunk>(),
        size_of::<ColumnMetaData>(),
        size_of::<KeyValue>(),
        size_of::<PageEncodingStats>(),
        size_of::<ColumnOrder>(),
        size_of::<Encoding>(),
        size_of::<String>(),
    ]
    .into_iter()
    .max()
    .expect("nonempty size list");
    let total_elements = groups * 18 + 16;
    let estimate_raw = |footer: usize| {
        let protocol_state = 16 * 64;
        let metadata_tree =
            total_elements * maximum_inline_element * 2 + footer + size_of::<FileMetaData>();
        let page_workspace =
            MAXIMUM_PAGE_HEADER_BYTES * 2 + 64 * maximum_inline_element + protocol_state;
        metadata_tree + protocol_state + footer + page_workspace
    };
    let raw_retained = estimate_raw(footer_length);
    let stock_retained = estimate_raw(MAXIMUM_FOOTER_BYTES) * 2;
    let maximum_chunk = metadata
        .row_groups()
        .iter()
        .flat_map(|group| group.columns())
        .map(|column| usize::try_from(column.compressed_size()).expect("chunk bytes"))
        .max()
        .unwrap_or(0);
    let retained_preflight = raw_retained
        .max(stock_retained)
        .max(raw_retained + maximum_chunk);
    let maximum_group_peak = metadata
        .row_groups()
        .iter()
        .map(|group| {
            let encoded = group
                .columns()
                .iter()
                .map(|column| usize::try_from(column.compressed_size()).expect("chunk bytes"))
                .sum::<usize>();
            let rows = usize::try_from(group.num_rows()).expect("group rows");
            let arrow_output = encoded + rows * 24 + (rows + 1) * 8;
            encoded * 2 + arrow_output * 2 + rows * 4
        })
        .max()
        .unwrap_or(0);
    let retained_full =
        retained_preflight.max(metadata.memory_size() + maximum_group_peak + READER_WORKSPACE);
    (maximum_group_peak, retained_full)
}

fn footprint_decoded_bytes(rows: usize) -> u64 {
    let validity = rows.div_ceil(8);
    u64::try_from(validity * 3 + (rows + 1) * 4 + rows * 14 + rows * 16)
        .expect("footprint decoded bytes")
}

fn overlap_decoded_bytes(rows: usize) -> u64 {
    let validity = rows.div_ceil(8);
    u64::try_from(validity * 2 + (rows + 1) * 8 + rows * 28).expect("overlap decoded bytes")
}

#[test]
fn canonical_spatial_parquet_writers_are_deterministic_and_preflight_exact_rows() {
    let fixture = fixture(3);
    let limits = budgets(8 * 1024 * 1024);
    let mut footprint_first = Vec::new();
    let footprint_summary = write_patch_footprint_set_parquet(
        &mut footprint_first,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("write footprint Parquet");
    let mut footprint_second = Vec::new();
    write_patch_footprint_set_parquet(
        &mut footprint_second,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("repeat footprint Parquet");
    assert_eq!(footprint_second, footprint_first);
    let mut fragmented = FragmentingWriter::default();
    write_patch_footprint_set_parquet(
        &mut fragmented,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("fragmented footprint Parquet");
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
        "9ff3c9c135ea3160b4e96384b81dbd18ec7a53ca8213d79a1ef9b37ead43d862"
    );
    let footprint_preflight = preflight_patch_footprint_set_parquet_bytes(
        &footprint_first,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("preflight footprint Parquet");
    assert_eq!(footprint_preflight.row_count(), 3);
    assert_eq!(footprint_preflight.row_group_count(), 1);

    let mut overlap_first = Vec::new();
    let overlap_summary = write_patch_overlap_graph_parquet(
        &mut overlap_first,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &fixture.overlap,
        limits,
    )
    .expect("write overlap Parquet");
    let mut overlap_second = Vec::new();
    write_patch_overlap_graph_parquet(
        &mut overlap_second,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &fixture.overlap,
        limits,
    )
    .expect("repeat overlap Parquet");
    assert_eq!(overlap_second, overlap_first);
    assert_eq!(overlap_summary.row_count(), 2);
    assert_eq!(
        overlap_summary.content_digest(),
        ContentDigest::from_bytes(&overlap_first)
    );
    assert_eq!(
        overlap_summary.content_digest().to_string(),
        "1ce5574426a54c708c395c0e26aea1235772fa208ad0a63efd0167e2cc721f25"
    );
    let overlap_preflight = preflight_patch_overlap_graph_parquet_bytes(
        &overlap_first,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &fixture.overlap,
        limits,
    )
    .expect("preflight overlap Parquet");
    assert_eq!(overlap_preflight.row_count(), 2);
    assert_eq!(overlap_preflight.row_group_count(), 1);
}

#[test]
fn spatial_parquet_uses_exact_empty_and_public_row_group_boundaries() {
    for (rows, groups) in [(0, 0), (1, 1), (8_192, 1), (8_193, 2)] {
        let fixture = fixture(rows);
        let limits = budgets(64 * 1024 * 1024);
        let mut encoded = Vec::new();
        write_patch_footprint_set_parquet(
            &mut encoded,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            limits,
        )
        .expect("write boundary Parquet");
        let preflight = preflight_patch_footprint_set_parquet_bytes(
            &encoded,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            limits,
        )
        .expect("preflight boundary Parquet");
        assert_eq!(preflight.row_count(), rows as u64);
        assert_eq!(preflight.row_group_count(), groups);
    }
}

#[test]
fn spatial_parquet_preserves_negative_reflect_origins_and_empty_overlap_graphs() {
    let reflecting = reflecting_fixture();
    let limits = budgets(8 * 1024 * 1024);
    let mut footprints = Vec::new();
    write_patch_footprint_set_parquet(
        &mut footprints,
        &reflecting.expected,
        &reflecting.context,
        &reflecting.footprints,
        limits,
    )
    .expect("write reflecting footprints");
    preflight_patch_footprint_set_parquet_bytes(
        &footprints,
        &reflecting.expected,
        &reflecting.context,
        &reflecting.footprints,
        limits,
    )
    .expect("preflight negative origin");

    let isolated = fixture(1);
    let mut overlap = Vec::new();
    write_patch_overlap_graph_parquet(
        &mut overlap,
        &isolated.expected,
        &isolated.context,
        &isolated.footprints,
        &isolated.overlap,
        limits,
    )
    .expect("write empty overlap graph");
    let preflight = preflight_patch_overlap_graph_parquet_bytes(
        &overlap,
        &isolated.expected,
        &isolated.context,
        &isolated.footprints,
        &isolated.overlap,
        limits,
    )
    .expect("preflight empty overlap graph");
    assert_eq!(preflight.row_count(), 0);
    assert_eq!(preflight.row_group_count(), 0);
}

#[test]
fn spatial_parquet_publication_has_exact_profiles_and_dependencies() {
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("spatial-parquet").expect("store ID"),
    )
    .expect("store");
    let fixture = fixture(3);
    let limits = budgets(8 * 1024 * 1024);

    let footprints = publish_patch_footprint_set_parquet(
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
        "application/vnd.marklab.patch-footprint-table.v1+parquet"
    );
    assert_eq!(
        footprints.record().table().expect("manifest").format(),
        TableFormat::ParquetFile
    );
    let mut expected_dependencies = vec![fixture.expected_artifact_id, fixture.context_artifact_id];
    expected_dependencies.sort_unstable();
    assert_eq!(footprints.record().dependencies(), expected_dependencies);
    let mut footprint_bytes = Vec::new();
    write_patch_footprint_set_parquet(
        &mut footprint_bytes,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("write footprint bytes");
    validate_patch_footprint_set_parquet_bytes(
        &footprint_bytes,
        footprints.record(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("validate borrowed footprints");
    validate_patch_footprint_set_parquet_from_store(
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

    let overlap = publish_patch_overlap_graph_parquet(
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
        "application/vnd.marklab.patch-overlap-edge-table.v1+parquet"
    );
    let mut overlap_dependencies = vec![
        fixture.expected_artifact_id,
        fixture.context_artifact_id,
        footprints.record().id(),
    ];
    overlap_dependencies.sort_unstable();
    assert_eq!(overlap.record().dependencies(), overlap_dependencies);
    let mut overlap_bytes = Vec::new();
    write_patch_overlap_graph_parquet(
        &mut overlap_bytes,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &physical_overlap,
        limits,
    )
    .expect("write overlap bytes");
    validate_patch_overlap_graph_parquet_bytes(
        &overlap_bytes,
        overlap.record(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &physical_overlap,
        limits,
    )
    .expect("validate borrowed overlap");
    validate_patch_overlap_graph_parquet_from_store(
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
fn spatial_parquet_fully_decodes_three_bounded_groups_for_both_profiles() {
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("three-group-parquet").expect("store ID"),
    )
    .expect("store");
    let fixture = fixture(16_386);
    let limits = budgets(64 * 1024 * 1024);
    let footprint = publish_patch_footprint_set_parquet(
        &store,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("publish three-group footprints");
    let footprint_summary = validate_patch_footprint_set_parquet_from_store(
        &store,
        footprint.record(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("decode three-group footprints");
    assert_eq!(footprint_summary.row_group_count(), 3);

    let overlap = PatchOverlapGraph::derive(
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        footprint.record().id(),
        64 * 1024 * 1024,
        64 * 1024 * 1024,
    )
    .expect("derive three-group overlap");
    let publication = publish_patch_overlap_graph_parquet(
        &store,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &overlap,
        limits,
    )
    .expect("publish three-group overlap");
    let overlap_summary = validate_patch_overlap_graph_parquet_from_store(
        &store,
        publication.record(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &overlap,
        limits,
    )
    .expect("decode three-group overlap");
    assert_eq!(overlap_summary.row_group_count(), 3);
}

#[test]
fn spatial_parquet_file_budget_is_enforced_before_encoding_or_preflight() {
    let fixture = fixture(3);
    let generous = budgets(8 * 1024 * 1024);
    let mut bytes = Vec::new();
    write_patch_footprint_set_parquet(
        &mut bytes,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        generous,
    )
    .expect("write Parquet");
    let short = budgets(bytes.len() as u64 - 1);
    assert!(write_patch_footprint_set_parquet(
        &mut Vec::new(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        short,
    )
    .is_err());
    assert!(preflight_patch_footprint_set_parquet_bytes(
        &bytes,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        short,
    )
    .is_err());
}

#[test]
fn spatial_parquet_raw_preflight_rejects_magic_metadata_and_row_drift() {
    let fixture = fixture(3);
    let limits = budgets(8 * 1024 * 1024);
    let mut canonical = Vec::new();
    write_patch_footprint_set_parquet(
        &mut canonical,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("write Parquet");

    let mut bad_magic = canonical.clone();
    bad_magic[0] = 0;
    assert!(matches!(
        preflight_patch_footprint_set_parquet_bytes(
            &bad_magic,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            limits,
        ),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidMagic,
        })
    ));

    let mut wrong_compact_type = canonical.clone();
    let footer = parquet_footer_start(&wrong_compact_type);
    assert_eq!(wrong_compact_type[footer] & 0x0f, 5, "version is i32");
    wrong_compact_type[footer] = (wrong_compact_type[footer] & 0xf0) | 8;
    assert!(matches!(
        preflight_patch_footprint_set_parquet_bytes(
            &wrong_compact_type,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            limits,
        ),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidFooter,
        })
    ));

    let created_by = b"marklab-embeddings/0.1.0 parquet-56.2.1 profile-v1";
    let created_at = canonical
        .windows(created_by.len())
        .position(|window| window == created_by)
        .expect("created-by metadata");
    let mut wrong_metadata = canonical.clone();
    wrong_metadata[created_at] = b'M';
    assert!(matches!(
        preflight_patch_footprint_set_parquet_bytes(
            &wrong_metadata,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            limits,
        ),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidMetadata,
        })
    ));

    let origin = 192_i64.to_le_bytes();
    let origin_offsets = canonical
        .windows(origin.len())
        .enumerate()
        .filter_map(|(index, window)| (window == origin).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(origin_offsets.len(), 1, "one physical 192 origin");
    let mut wrong_origin = canonical.clone();
    wrong_origin[origin_offsets[0]] ^= 1;
    assert!(matches!(
        preflight_patch_footprint_set_parquet_bytes(
            &wrong_origin,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            limits,
        ),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidCanonicalRows,
        })
    ));

    let patch = b"patch-00000000";
    let patch_at = canonical
        .windows(patch.len())
        .position(|window| window == patch)
        .expect("physical patch row");
    let mut wrong_row = canonical.clone();
    wrong_row[patch_at] = b'P';
    assert!(matches!(
        preflight_patch_footprint_set_parquet_bytes(
            &wrong_row,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            limits,
        ),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidCanonicalRows,
        })
    ));

    let root_name = b"marklab_patch_footprint_table";
    let root_at = canonical
        .windows(root_name.len())
        .position(|window| window == root_name)
        .expect("Parquet root name");
    let mut wrong_schema = canonical.clone();
    wrong_schema[root_at] = b'M';
    assert!(matches!(
        preflight_patch_footprint_set_parquet_bytes(
            &wrong_schema,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            limits,
        ),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidSchema,
        })
    ));

    let mut overlap_bytes = Vec::new();
    write_patch_overlap_graph_parquet(
        &mut overlap_bytes,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &fixture.overlap,
        limits,
    )
    .expect("write overlap Parquet");
    let endpoint = b"patch-00000001";
    let endpoint_at = overlap_bytes
        .windows(endpoint.len())
        .position(|window| window == endpoint)
        .expect("overlap endpoint");
    overlap_bytes[endpoint_at] = b'P';
    assert!(matches!(
        preflight_patch_overlap_graph_parquet_bytes(
            &overlap_bytes,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            &fixture.overlap,
            limits,
        ),
        Err(MultiscaleColumnarError::Parquet {
            reason: SpatialParquetFailure::InvalidCanonicalRows,
        })
    ));
}

#[test]
fn parquet_row_group_budget_precedes_malformed_page_allocation() {
    let fixture = fixture(3);
    let limits = budgets(8 * 1024 * 1024);
    let mut canonical = Vec::new();
    write_patch_footprint_set_parquet(
        &mut canonical,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        limits,
    )
    .expect("write Parquet");
    let group_bytes = parquet_footer_start(&canonical) - 4;
    let exact = EmbeddingColumnarBudgets::new(
        limits.maximum_file_bytes(),
        limits.maximum_retained_bytes(),
        group_bytes,
        109,
    );
    preflight_patch_footprint_set_parquet_bytes(
        &canonical,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        exact,
    )
    .expect("exact row-group budget");

    let mut malformed_page = canonical;
    malformed_page[4] ^= 0x0f;
    let one_short = EmbeddingColumnarBudgets::new(
        limits.maximum_file_bytes(),
        limits.maximum_retained_bytes(),
        group_bytes - 1,
        109,
    );
    assert!(matches!(
        preflight_patch_footprint_set_parquet_bytes(
            &malformed_page,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            one_short,
        ),
        Err(MultiscaleColumnarError::RowGroupByteBudgetExceeded {
            required,
            maximum,
        }) if required == group_bytes && maximum == group_bytes - 1
    ));
    assert!(matches!(
        preflight_patch_footprint_set_parquet_bytes(
            &malformed_page,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            exact,
        ),
        Err(MultiscaleColumnarError::Parquet { .. })
    ));
}

#[test]
fn spatial_parquet_decoded_row_group_and_retained_budgets_have_exact_edges() {
    let fixture = fixture(3);
    let generous = budgets(8 * 1024 * 1024);
    let no_decoded = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        generous.maximum_retained_bytes(),
        generous.maximum_row_group_bytes(),
        0,
    );
    let decoded_required = match write_patch_footprint_set_parquet(
        &mut Vec::new(),
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        no_decoded,
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
    write_patch_footprint_set_parquet(
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
        write_patch_footprint_set_parquet(
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
    let group_required = match write_patch_footprint_set_parquet(
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
        observed => panic!("expected group budget failure, got {observed:?}"),
    };
    let no_retained = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        0,
        group_required,
        generous.maximum_decoded_bytes(),
    );
    let retained_required = match write_patch_footprint_set_parquet(
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
    write_patch_footprint_set_parquet(
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
        write_patch_footprint_set_parquet(
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
        write_patch_footprint_set_parquet(
            &mut Vec::new(),
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            retained_short,
        ),
        Err(MultiscaleColumnarError::RetainedByteBudgetExceeded { .. })
    ));

    let mut overlap_bytes = Vec::new();
    write_patch_overlap_graph_parquet(
        &mut overlap_bytes,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &fixture.overlap,
        generous,
    )
    .expect("write overlap budget fixture");
    let overlap_exact = EmbeddingColumnarBudgets::new(
        generous.maximum_file_bytes(),
        generous.maximum_retained_bytes(),
        generous.maximum_row_group_bytes(),
        82,
    );
    preflight_patch_overlap_graph_parquet_bytes(
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
        81,
    );
    assert!(matches!(
        preflight_patch_overlap_graph_parquet_bytes(
            &overlap_bytes,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            &fixture.overlap,
            overlap_short,
        ),
        Err(MultiscaleColumnarError::DecodedByteBudgetExceeded { .. })
    ));
}

#[test]
fn spatial_parquet_full_readers_have_exact_group_and_retained_edges() {
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("spatial-parquet-reader-budgets").expect("store ID"),
    )
    .expect("store");
    let fixture = fixture(3);
    let generous = budgets(8 * 1024 * 1024);

    let footprint_publication = publish_patch_footprint_set_parquet(
        &store,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        generous,
    )
    .expect("publish footprint budget fixture");
    let mut footprint_bytes = Vec::new();
    write_patch_footprint_set_parquet(
        &mut footprint_bytes,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        generous,
    )
    .expect("write footprint budget fixture");
    let (footprint_group, footprint_retained) = parquet_reader_requirements(&footprint_bytes);
    let footprint_decoded = footprint_decoded_bytes(fixture.footprints.row_count());
    let footprint_exact = EmbeddingColumnarBudgets::new(
        footprint_bytes.len() as u64,
        footprint_retained,
        footprint_group,
        footprint_decoded,
    );
    validate_patch_footprint_set_parquet_bytes(
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
        let error = validate_patch_footprint_set_parquet_bytes(
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
    let overlap_publication = publish_patch_overlap_graph_parquet(
        &store,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &overlap,
        generous,
    )
    .expect("publish overlap budget fixture");
    let mut overlap_bytes = Vec::new();
    write_patch_overlap_graph_parquet(
        &mut overlap_bytes,
        &fixture.expected,
        &fixture.context,
        &fixture.footprints,
        &overlap,
        generous,
    )
    .expect("write overlap budget fixture");
    let (overlap_group, overlap_retained) = parquet_reader_requirements(&overlap_bytes);
    let overlap_decoded = overlap_decoded_bytes(overlap.edge_count());
    let overlap_exact = EmbeddingColumnarBudgets::new(
        overlap_bytes.len() as u64,
        overlap_retained,
        overlap_group,
        overlap_decoded,
    );
    validate_patch_overlap_graph_parquet_bytes(
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
        let error = validate_patch_overlap_graph_parquet_bytes(
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
fn spatial_parquet_is_deterministic_across_fresh_processes() {
    let temporary = TempDir::new().expect("temporary directory");
    let executable = std::env::current_exe().expect("test executable");
    let first = temporary.path().join("first.parquet");
    let second = temporary.path().join("second.parquet");
    for path in [&first, &second] {
        let status = Command::new(&executable)
            .arg("--exact")
            .arg("spatial_parquet_child")
            .env("MARKLAB_SPATIAL_PARQUET_CHILD", path)
            .status()
            .expect("fresh child");
        assert!(status.success());
    }
    assert_eq!(
        fs::read(first).expect("first"),
        fs::read(second).expect("second")
    );

    let overlap_first = temporary.path().join("first-overlap.parquet");
    let overlap_second = temporary.path().join("second-overlap.parquet");
    for path in [&overlap_first, &overlap_second] {
        let status = Command::new(&executable)
            .arg("--exact")
            .arg("spatial_parquet_child")
            .env("MARKLAB_SPATIAL_PARQUET_OVERLAP_CHILD", path)
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
fn spatial_parquet_child() {
    let footprint_path = std::env::var_os("MARKLAB_SPATIAL_PARQUET_CHILD");
    let overlap_path = std::env::var_os("MARKLAB_SPATIAL_PARQUET_OVERLAP_CHILD");
    let Some(path) = footprint_path.or_else(|| overlap_path.clone()) else {
        return;
    };
    let fixture = fixture(3);
    let mut bytes = Vec::new();
    if overlap_path.is_some() {
        write_patch_overlap_graph_parquet(
            &mut bytes,
            &fixture.expected,
            &fixture.context,
            &fixture.footprints,
            &fixture.overlap,
            budgets(8 * 1024 * 1024),
        )
        .expect("child overlap write");
    } else {
        write_patch_footprint_set_parquet(
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
