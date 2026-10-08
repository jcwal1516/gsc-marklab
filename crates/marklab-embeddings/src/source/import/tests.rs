use std::{io::Cursor, str::FromStr};

use marklab_data::{
    CellId, CohortHierarchy, HierarchyId, HierarchyNode, PatientId, ReplicationRole, SlideId,
};
use marklab_project::{ArtifactId, ContentDigest};

use crate::{
    CellEmbeddingRowLink, CellEmbeddingRowLinkEntry, ExpectedCellSet,
    VerifiedCellEmbeddingArtifactGraph,
};

use super::super::{
    CellVitCsvSummary, CellVitNpySummary, ImportFailure, SourceBundleBudgets, SourceBundleError,
};
use super::CellVitHeImportCandidate;

fn artifact_id(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

fn fixture() -> (
    ExpectedCellSet,
    CellVitHeImportCandidate,
    VerifiedCellEmbeddingArtifactGraph,
) {
    let cell = CellId::new("cell-a").expect("cell");
    let expected = ExpectedCellSet::new("all.v1", vec![cell.clone()]).expect("expected");
    let patient = HierarchyId::from(PatientId::new("patient").expect("patient"));
    let slide = HierarchyId::from(SlideId::new("slide").expect("slide"));
    let hierarchy = CohortHierarchy::new(
        vec![
            HierarchyNode::new(patient.clone(), None, ReplicationRole::BiologicalUnit),
            HierarchyNode::new(
                slide.clone(),
                None,
                ReplicationRole::TechnicalReplicate {
                    biological_source: patient,
                },
            ),
            HierarchyNode::new(
                HierarchyId::from(cell.clone()),
                Some(slide),
                ReplicationRole::Structural,
            ),
        ],
        Vec::new(),
    )
    .expect("hierarchy");
    let source_cells_artifact_id = artifact_id(b"source-cells");
    let source_vectors_artifact_id = artifact_id(b"source-vectors");
    let expected_cells_artifact_id = artifact_id(b"expected-cells");
    let identity_map_artifact_id = artifact_id(b"identity-map");
    let converter_artifact_id = artifact_id(b"converter");
    let provenance_artifact_id = artifact_id(b"provenance");
    let row_link_artifact_id = artifact_id(b"row-link");
    let row_link = CellEmbeddingRowLink::new(
        source_cells_artifact_id,
        source_vectors_artifact_id,
        expected_cells_artifact_id,
        identity_map_artifact_id,
        converter_artifact_id,
        &expected,
        &hierarchy,
        vec![CellEmbeddingRowLinkEntry::present(cell, 0, 0)],
        1024 * 1024,
    )
    .expect("row link");
    let npy_bytes = npy_bytes();
    let csv_bytes = csv_bytes();
    let budgets = SourceBundleBudgets::new(
        npy_bytes.len() as u64,
        csv_bytes.len() as u64,
        64 * 1024,
        1024 * 1024,
        1_280 * 4,
    );
    let mut npy_reader = Cursor::new(npy_bytes);
    let npy_summary = CellVitNpySummary::from_reader(&mut npy_reader, budgets).expect("NPY");
    let csv_summary = CellVitCsvSummary::from_bytes(&csv_bytes, budgets).expect("CSV");
    let graph = VerifiedCellEmbeddingArtifactGraph {
        provenance_artifact_id,
        dependency_count: 13,
        source_cells_artifact_id,
        source_vectors_artifact_id,
        expected_cells_artifact_id,
        identity_map_artifact_id,
        converter_artifact_id,
        row_link_artifact_id,
        expected_cells_logical_digest: expected.logical_digest(),
        row_link_logical_digest: row_link.logical_digest(),
        output_dimension: 1_280,
    };
    let candidate = CellVitHeImportCandidate {
        values: vec![0.0; 1_280],
        dimension: 1_280,
        row_link,
        expected_cells_logical_digest: expected.logical_digest(),
        maximum_retained_bytes: 1024 * 1024,
        npy_summary,
        csv_summary,
    };
    (expected, candidate, graph)
}

fn npy_bytes() -> Vec<u8> {
    let dictionary = "{'descr': '<f4', 'fortran_order': False, 'shape': (1, 1280), }";
    let padding = (16 - ((10 + dictionary.len() + 1) % 16)) % 16;
    let mut header = dictionary.as_bytes().to_vec();
    header.resize(header.len() + padding, b' ');
    header.push(b'\n');
    let mut bytes = b"\x93NUMPY\x01\x00".to_vec();
    bytes.extend_from_slice(&(header.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&header);
    bytes.resize(bytes.len() + 1_280 * 4, 0);
    bytes
}

fn csv_bytes() -> Vec<u8> {
    concat!(
        "cell_id,case_id,specimen_id,timepoint,fragment_id,roi_id,native_row,",
        "embedding_row,x_px,y_px,x_um,y_um,cell_type_id,cell_type_label,",
        "type_probability,nucleus_area_um2,nucleus_perimeter_um,eccentricity,",
        "solidity,circularity,qc_pass,block_500_id,split\r\n",
        "source,case,specimen,time,fragment,roi,0,0,1,2,3,4,1,label,0.5,",
        "10,5,0.2,0.8,0.7,True,block,train\r\n"
    )
    .as_bytes()
    .to_vec()
}

#[test]
fn candidate_finalization_requires_the_exact_verified_graph_binding() {
    let (expected, candidate, graph) = fixture();
    let imported = candidate
        .finalize(&expected, &graph)
        .expect("verified finalization");
    assert_eq!(imported.table().row_count(), 1);
    assert_eq!(imported.table().dimension(), 1_280);

    let (expected, candidate, graph) = fixture();
    let mismatched = VerifiedCellEmbeddingArtifactGraph {
        row_link_logical_digest: ContentDigest::from_bytes(b"other-row-link"),
        ..graph
    };
    assert!(matches!(
        candidate.finalize(&expected, &mismatched),
        Err(SourceBundleError::Import {
            reason: ImportFailure::VerifiedGraphMismatch,
        })
    ));

    let (expected, candidate, graph) = fixture();
    let mismatched = VerifiedCellEmbeddingArtifactGraph {
        output_dimension: 1,
        ..graph
    };
    assert!(matches!(
        candidate.finalize(&expected, &mismatched),
        Err(SourceBundleError::Import {
            reason: ImportFailure::VerifiedGraphMismatch,
        })
    ));
}
