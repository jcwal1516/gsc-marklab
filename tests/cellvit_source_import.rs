#![cfg(feature = "csv")]

use std::{
    collections::BTreeMap,
    io::{Cursor, Read, Seek, SeekFrom},
};

use marklab::{
    import_cellvit_he_bundle_bytes, import_cellvit_he_bundle_from_store,
    import_cellvit_he_bundle_readers, ArtifactId, ArtifactKey, ArtifactLocator, ArtifactRecord,
    ArtifactRef, ArtifactSchema, CellId, CellIdentityMap, CellIdentityMapEntry, CellVitCsvSummary,
    CellVitHeArtifactBindings, CellVitHeImportRequest, CohortHierarchy, EmbeddingStatus,
    ExpectedCellSet, HierarchyId, HierarchyNode, ImportFailure, LocalArtifactStore, PatientId,
    ReplicationRole, SlideId, SourceBundleBudgets, SourceBundleError, StoreId,
};

const HEADER: &str = "cell_id,case_id,specimen_id,timepoint,fragment_id,roi_id,native_row,embedding_row,x_px,y_px,x_um,y_um,cell_type_id,cell_type_label,type_probability,nucleus_area_um2,nucleus_perimeter_um,eccentricity,solidity,circularity,qc_pass,block_500_id,split\r\n";

fn record(schema: &str, kind: &str, bytes: &[u8], dependencies: Vec<ArtifactId>) -> ArtifactRecord {
    ArtifactRecord::new(
        ArtifactRef::from_bytes(kind, bytes).expect("content"),
        ArtifactSchema::new(schema, 1).expect("schema"),
        None,
        dependencies,
        BTreeMap::new(),
        vec![ArtifactLocator::new(
            StoreId::new("source").expect("store"),
            ArtifactKey::new(format!("fixtures/{schema}")).expect("key"),
            None,
        )
        .expect("locator")],
    )
    .expect("record")
}

fn cell(value: &str) -> CellId {
    CellId::new(value).expect("cell ID")
}

fn csv() -> Vec<u8> {
    let mut bytes = HEADER.as_bytes().to_vec();
    for (row, source) in [(0, "local-alpha"), (1, "local-beta")] {
        bytes.extend_from_slice(
            format!(
                "{source},case,synthetic,time,fragment,roi,{row},{row},1,2,3,4,1,label,0.5,10,5,0.2,0.8,0.7,True,block,train\r\n"
            )
            .as_bytes(),
        );
    }
    bytes
}

fn npy(rows: &[Vec<f32>]) -> Vec<u8> {
    let dictionary = format!(
        "{{'descr': '<f4', 'fortran_order': False, 'shape': ({}, 1280), }}",
        rows.len()
    );
    let prefix_length = 10;
    let padding = (16 - ((prefix_length + dictionary.len() + 1) % 16)) % 16;
    let mut header = dictionary.into_bytes();
    header.resize(header.len() + padding, b' ');
    header.push(b'\n');
    let mut bytes = b"\x93NUMPY\x01\x00".to_vec();
    bytes.extend_from_slice(
        &u16::try_from(header.len())
            .expect("small header")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&header);
    for row in rows {
        for value in row {
            bytes.extend_from_slice(&value.to_bits().to_le_bytes());
        }
    }
    bytes
}

fn expected() -> ExpectedCellSet {
    ExpectedCellSet::new("all.v1", vec![cell("cell-a"), cell("cell-b")]).expect("expected")
}

fn hierarchy(expected: &ExpectedCellSet) -> CohortHierarchy {
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
    nodes.extend(expected.cells().iter().cloned().map(|cell_id| {
        HierarchyNode::new(
            HierarchyId::from(cell_id),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    CohortHierarchy::new(nodes, Vec::new()).expect("hierarchy")
}

fn bound_domain(
    npy: &[u8],
    csv: &[u8],
    expected: &ExpectedCellSet,
) -> (CellVitHeArtifactBindings, CellIdentityMap) {
    let source_cells = record(
        "marklab.cell_embedding_source_cells",
        "text/csv;profile=marklab-cellvit-he-bundle-v1",
        csv,
        Vec::new(),
    );
    let source_vectors = record(
        "marklab.cell_embedding_source_npy",
        "application/x-npy;profile=marklab-cellvit-he-f4-v1",
        npy,
        Vec::new(),
    );
    bound_domain_with_source_records(expected, source_cells, source_vectors)
}

fn bound_domain_with_source_records(
    expected: &ExpectedCellSet,
    source_cells: ArtifactRecord,
    source_vectors: ArtifactRecord,
) -> (CellVitHeArtifactBindings, CellIdentityMap) {
    let expected_record = record(
        "marklab.cell_embedding_expected_cells",
        "application/vnd.marklab.embedding-expected-cells.v1",
        &expected.to_bytes().expect("expected bytes"),
        Vec::new(),
    );
    let identity_map = CellIdentityMap::new(
        source_cells.id(),
        expected_record.id(),
        expected,
        vec![
            CellIdentityMapEntry::new("local-alpha", cell("cell-b")).expect("identity"),
            CellIdentityMapEntry::new("local-beta", cell("cell-a")).expect("identity"),
        ],
    )
    .expect("identity map");
    let identity_record = record(
        "marklab.cell_embedding_identity_map",
        "application/vnd.marklab.embedding-identity-map.v1",
        &identity_map.to_bytes().expect("identity bytes"),
        vec![source_cells.id(), expected_record.id()],
    );
    let converter = record(
        "marklab.converter_manifest",
        "application/vnd.marklab.converter-manifest.v1+json",
        b"synthetic-converter",
        Vec::new(),
    );
    let bindings = CellVitHeArtifactBindings::from_records(
        source_cells,
        source_vectors,
        expected_record,
        identity_record,
        converter,
    )
    .expect("record bindings");
    (bindings, identity_map)
}

fn budgets(npy: usize, csv: usize, retained: usize) -> SourceBundleBudgets {
    SourceBundleBudgets::new(
        u64::try_from(npy).expect("NPY bytes"),
        u64::try_from(csv).expect("CSV bytes"),
        64 * 1024,
        retained,
        2 * 1_280 * 4,
    )
}

struct ChunkedCursor {
    inner: Cursor<Vec<u8>>,
    maximum_chunk: usize,
}

impl ChunkedCursor {
    fn new(bytes: Vec<u8>, maximum_chunk: usize) -> Self {
        Self {
            inner: Cursor::new(bytes),
            maximum_chunk,
        }
    }
}

impl Read for ChunkedCursor {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let length = buffer.len().min(self.maximum_chunk);
        self.inner.read(&mut buffer[..length])
    }
}

impl Seek for ChunkedCursor {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(position)
    }
}

#[test]
fn source_import_maps_explicit_ids_into_one_canonical_present_matrix() {
    let csv = csv();
    let mut source_zero = (0..1_280)
        .map(|column| column as f32 + 0.25)
        .collect::<Vec<_>>();
    source_zero[1] = -0.0;
    let mut source_one = (0..1_280)
        .map(|column| -(column as f32) - 0.5)
        .collect::<Vec<_>>();
    source_one[2] = -0.0;
    let expected_values = source_one
        .iter()
        .chain(&source_zero)
        .map(|value| if *value == 0.0 { 0.0 } else { *value })
        .collect::<Vec<_>>();
    let npy = npy(&[source_zero, source_one]);
    let expected = expected();
    let (bindings, identity_map) = bound_domain(&npy, &csv, &expected);
    let hierarchy = hierarchy(&expected);
    let request = CellVitHeImportRequest::new(
        &expected,
        &identity_map,
        &hierarchy,
        &bindings,
        budgets(npy.len(), csv.len(), 4 * 1024 * 1024),
    );
    let imported =
        import_cellvit_he_bundle_bytes(&npy, &csv, request).expect("borrowed source import");
    assert_eq!(imported.row_count(), 2);
    assert_eq!(imported.dimension(), 1_280);
    assert_eq!(imported.canonical_values(), expected_values);
    let links = imported.row_link().entries();
    assert!(links
        .iter()
        .all(|entry| entry.status() == EmbeddingStatus::Present));
    assert_eq!(
        links
            .iter()
            .map(|entry| (entry.source_cell_row(), entry.source_embedding_row()))
            .collect::<Vec<_>>(),
        [(1, Some(1)), (0, Some(0))]
    );

    let mut npy_reader = ChunkedCursor::new(npy, 1);
    let mut csv_reader = ChunkedCursor::new(csv, 3);
    let streamed = import_cellvit_he_bundle_readers(&mut npy_reader, &mut csv_reader, request)
        .expect("chunked source import");
    assert_eq!(streamed.canonical_values(), imported.canonical_values());
    assert_eq!(streamed.npy_summary(), imported.npy_summary());
    assert_eq!(streamed.csv_summary(), imported.csv_summary());
    assert_eq!(
        streamed.row_link().logical_digest(),
        imported.row_link().logical_digest()
    );
    assert_eq!(streamed.row_link().entries(), imported.row_link().entries());
}

#[test]
fn source_import_rejects_count_identity_binding_and_peak_budget_mismatches() {
    let csv = csv();
    let short_npy = npy(&[vec![0.0; 1_280]]);
    let expected = expected();
    let (short_bindings, short_map) = bound_domain(&short_npy, &csv, &expected);
    let hierarchy = hierarchy(&expected);
    let request = CellVitHeImportRequest::new(
        &expected,
        &short_map,
        &hierarchy,
        &short_bindings,
        budgets(short_npy.len(), csv.len(), 4 * 1024 * 1024),
    );
    assert!(matches!(
        import_cellvit_he_bundle_bytes(&short_npy, &csv, request),
        Err(SourceBundleError::RowCountMismatch { npy: 1, csv: 2 })
    ));

    let full_npy = npy(&[vec![0.0; 1_280], vec![0.0; 1_280]]);
    let (bindings, map) = bound_domain(&full_npy, &csv, &expected);
    let wrong_map = CellIdentityMap::new(
        bindings.source_cells_artifact_id(),
        bindings.expected_cells_artifact_id(),
        &expected,
        vec![
            CellIdentityMapEntry::new("other-alpha", cell("cell-b")).expect("identity"),
            CellIdentityMapEntry::new("other-beta", cell("cell-a")).expect("identity"),
        ],
    )
    .expect("wrong map");
    let request = CellVitHeImportRequest::new(
        &expected,
        &wrong_map,
        &hierarchy,
        &bindings,
        budgets(full_npy.len(), csv.len(), 4 * 1024 * 1024),
    );
    assert!(matches!(
        import_cellvit_he_bundle_bytes(&full_npy, &csv, request),
        Err(SourceBundleError::Import {
            reason: ImportFailure::ArtifactBindingMismatch
        })
    ));

    let request = CellVitHeImportRequest::new(
        &expected,
        &map,
        &hierarchy,
        &bindings,
        budgets(full_npy.len(), csv.len(), 1),
    );
    assert!(matches!(
        import_cellvit_he_bundle_bytes(&full_npy, &csv, request),
        Err(SourceBundleError::RetainedByteBudgetExceeded { .. })
    ));
}

#[test]
fn source_import_rejects_same_length_bytes_that_do_not_match_bound_artifact_records() {
    let csv = csv();
    let npy = npy(&[vec![0.0; 1_280], vec![1.0; 1_280]]);
    let expected = expected();
    let source_cells = record(
        "marklab.cell_embedding_source_cells",
        "text/csv;profile=marklab-cellvit-he-bundle-v1",
        &csv,
        Vec::new(),
    );
    let source_vectors = record(
        "marklab.cell_embedding_source_npy",
        "application/x-npy;profile=marklab-cellvit-he-f4-v1",
        &npy,
        Vec::new(),
    );
    let expected_record = record(
        "marklab.cell_embedding_expected_cells",
        "application/vnd.marklab.embedding-expected-cells.v1",
        &expected.to_bytes().expect("expected bytes"),
        Vec::new(),
    );
    let identity_map = CellIdentityMap::new(
        source_cells.id(),
        expected_record.id(),
        &expected,
        vec![
            CellIdentityMapEntry::new("local-alpha", cell("cell-b")).expect("identity"),
            CellIdentityMapEntry::new("local-beta", cell("cell-a")).expect("identity"),
        ],
    )
    .expect("identity map");
    let identity_record = record(
        "marklab.cell_embedding_identity_map",
        "application/vnd.marklab.embedding-identity-map.v1",
        &identity_map.to_bytes().expect("identity bytes"),
        vec![source_cells.id(), expected_record.id()],
    );
    let converter = record(
        "marklab.converter_manifest",
        "application/vnd.marklab.converter-manifest.v1+json",
        b"synthetic-converter",
        Vec::new(),
    );
    let bindings = CellVitHeArtifactBindings::from_records(
        source_cells,
        source_vectors,
        expected_record,
        identity_record,
        converter,
    )
    .expect("record bindings");
    let hierarchy = hierarchy(&expected);
    let request = CellVitHeImportRequest::new(
        &expected,
        &identity_map,
        &hierarchy,
        &bindings,
        budgets(npy.len(), csv.len(), 4 * 1024 * 1024),
    );
    let mut forged = npy.clone();
    let last = forged.last_mut().expect("payload byte");
    *last ^= 1;
    assert!(matches!(
        import_cellvit_he_bundle_bytes(&forged, &csv, request),
        Err(SourceBundleError::Import {
            reason: ImportFailure::ArtifactBindingMismatch,
        })
    ));
}

#[test]
fn source_import_precharges_csv_metadata_with_npy_header_before_shape_read() {
    let csv = csv();
    let npy = npy(&[vec![0.0; 1_280], vec![1.0; 1_280]]);
    let expected = expected();
    let hierarchy = hierarchy(&expected);
    let (bindings, identity_map) = bound_domain(&npy, &csv, &expected);
    let mut csv_retained = 64 * 1024;
    loop {
        let limits = budgets(npy.len(), csv.len(), csv_retained);
        match CellVitCsvSummary::from_bytes(&csv, limits) {
            Ok(_) => break,
            Err(SourceBundleError::RetainedByteBudgetExceeded { required, .. })
                if required > csv_retained =>
            {
                csv_retained = required;
            }
            Err(error) => panic!("unexpected CSV retained-budget probe failure: {error:?}"),
        }
    }
    let request = CellVitHeImportRequest::new(
        &expected,
        &identity_map,
        &hierarchy,
        &bindings,
        budgets(npy.len(), csv.len(), csv_retained),
    );
    assert_eq!(
        import_cellvit_he_bundle_bytes(&npy, &csv, request).expect_err("composed peak"),
        SourceBundleError::RetainedByteBudgetExceeded {
            required: csv_retained + 64 * 1024 + 12,
            maximum: csv_retained,
        }
    );
}

#[test]
fn managed_store_import_verifies_both_sources_and_rejects_unmanaged_bindings() {
    let csv = csv();
    let npy = npy(&[vec![0.0; 1_280], vec![1.0; 1_280]]);
    let expected = expected();
    let hierarchy = hierarchy(&expected);
    let root = tempfile::tempdir().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("managed-import").expect("store ID"),
    )
    .expect("store");
    let source_cells = record(
        "marklab.cell_embedding_source_cells",
        "text/csv;profile=marklab-cellvit-he-bundle-v1",
        &csv,
        Vec::new(),
    );
    let source_cells = store
        .publish(&source_cells, |writer| writer.write_all(&csv))
        .expect("publish CSV")
        .into_record();
    let source_vectors = record(
        "marklab.cell_embedding_source_npy",
        "application/x-npy;profile=marklab-cellvit-he-f4-v1",
        &npy,
        Vec::new(),
    );
    let source_vectors = store
        .publish(&source_vectors, |writer| writer.write_all(&npy))
        .expect("publish NPY")
        .into_record();
    let (bindings, identity_map) =
        bound_domain_with_source_records(&expected, source_cells, source_vectors);
    let request = CellVitHeImportRequest::new(
        &expected,
        &identity_map,
        &hierarchy,
        &bindings,
        budgets(npy.len(), csv.len(), 4 * 1024 * 1024),
    );
    let borrowed =
        import_cellvit_he_bundle_bytes(&npy, &csv, request).expect("borrowed source import");
    let imported =
        import_cellvit_he_bundle_from_store(&store, request).expect("managed source import");
    assert_eq!(imported.canonical_values(), borrowed.canonical_values());
    assert_eq!(imported.npy_summary(), borrowed.npy_summary());
    assert_eq!(imported.csv_summary(), borrowed.csv_summary());
    assert_eq!(
        imported.row_link().logical_digest(),
        borrowed.row_link().logical_digest()
    );
    assert_eq!(imported.row_link().entries(), borrowed.row_link().entries());

    let (unmanaged_bindings, unmanaged_map) = bound_domain(&npy, &csv, &expected);
    let request = CellVitHeImportRequest::new(
        &expected,
        &unmanaged_map,
        &hierarchy,
        &unmanaged_bindings,
        budgets(npy.len(), csv.len(), 4 * 1024 * 1024),
    );
    assert!(matches!(
        import_cellvit_he_bundle_from_store(&store, request),
        Err(SourceBundleError::Import {
            reason: ImportFailure::ArtifactUnavailable,
        })
    ));
}
