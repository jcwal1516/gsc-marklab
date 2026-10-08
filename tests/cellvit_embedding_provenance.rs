use std::str::FromStr;

use marklab::{
    ArtifactId, CanonicalDecimal, CellEmbeddingExecutionProvenance, CellEmbeddingInputArtifacts,
    CellEmbeddingModelProvenance, CellEmbeddingProvenance, CellEmbeddingRowLink,
    CellEmbeddingRowLinkEntry, CellEmbeddingTensorContract, CellId, CohortHierarchy, ContentDigest,
    EmbeddingError, EmbeddingStatus, ExpectedCellSet, HierarchyId, HierarchyNode, PatientId,
    ReplicationRole, SlideId,
};

fn artifact_id(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

fn cell(value: &str) -> CellId {
    CellId::new(value).expect("cell ID")
}

fn expected() -> ExpectedCellSet {
    ExpectedCellSet::new(
        "all-qc-eligible.v1",
        vec![cell("cell-a"), cell("cell-b"), cell("cell-c")],
    )
    .expect("expected cells")
}

fn hierarchy(expected: &ExpectedCellSet) -> CohortHierarchy {
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
    CohortHierarchy::new(nodes, Vec::new()).expect("hierarchy")
}

fn link() -> CellEmbeddingRowLink {
    let expected = expected();
    CellEmbeddingRowLink::new(
        artifact_id(b"source-cells"),
        artifact_id(b"source-vectors"),
        artifact_id(b"expected-cells"),
        artifact_id(b"identity-map"),
        artifact_id(b"converter"),
        &expected,
        &hierarchy(&expected),
        vec![
            CellEmbeddingRowLinkEntry::present(cell("cell-a"), 2, 1),
            CellEmbeddingRowLinkEntry::missing_vector(cell("cell-b"), 0),
            CellEmbeddingRowLinkEntry::qc_rejected(cell("cell-c"), 1, 0),
        ],
        4_096,
    )
    .expect("row link")
}

fn decimals(values: [&str; 3]) -> [CanonicalDecimal; 3] {
    values.map(|value| CanonicalDecimal::new(value).expect("canonical decimal"))
}

fn provenance(row_link_artifact_id: ArtifactId) -> CellEmbeddingProvenance {
    let model = CellEmbeddingModelProvenance::new(
        "cellvit_sam_h",
        "1.0",
        "sam_h",
        artifact_id(b"checkpoint"),
        ContentDigest::from_bytes(b"checkpoint-bytes"),
        artifact_id(b"source-snapshot"),
        artifact_id(b"license"),
        "Apache-2.0",
        "doi:10.0000-example",
        "z4",
        32,
    )
    .expect("model provenance");
    let tensor = CellEmbeddingTensorContract::rgb_he_raw(
        1_280,
        decimals(["0.485", "0.456", "0.406"]),
        decimals(["0.229", "0.224", "0.225"]),
    )
    .expect("tensor contract");
    let execution = CellEmbeddingExecutionProvenance::new(
        artifact_id(b"preprocessing"),
        artifact_id(b"run-config"),
        artifact_id(b"environment"),
        artifact_id(b"converter"),
        "marklab_cellvit_converter",
        "1.0",
        "cellvit_he_bundle",
        "1.0",
    )
    .expect("execution provenance");
    let inputs = CellEmbeddingInputArtifacts::new(
        artifact_id(b"source-cells"),
        artifact_id(b"source-vectors"),
        artifact_id(b"expected-cells"),
        artifact_id(b"identity-map"),
        artifact_id(b"spatial-context"),
        row_link_artifact_id,
    );
    CellEmbeddingProvenance::new(model, tensor, execution, inputs).expect("embedding provenance")
}

#[test]
fn row_link_binds_exact_rows_statuses_hierarchy_and_dependencies() {
    let link = link();
    assert_eq!(link.row_count(), 3);
    assert_eq!(link.entries()[0].source_cell_row(), 2);
    assert_eq!(link.entries()[0].source_embedding_row(), Some(1));
    assert_eq!(link.entries()[1].source_embedding_row(), None);
    assert_eq!(link.entries()[2].source_embedding_row(), Some(0));
    assert_eq!(
        link.expected_cells_artifact_id(),
        artifact_id(b"expected-cells")
    );
    assert_eq!(
        link.logical_digest().to_string(),
        "728f7beebee46b068c9cee2db907949c31f3dacc55962d3085587493762cb349"
    );
    assert_eq!(link.direct_dependencies().len(), 5);
    assert!(link
        .direct_dependencies()
        .windows(2)
        .all(|pair| pair[0] < pair[1]));
}

#[test]
fn row_link_rejects_gaps_duplicates_wrong_status_rows_and_non_cells() {
    let expected = expected();
    let expected_hierarchy = hierarchy(&expected);
    let build = |entries| {
        CellEmbeddingRowLink::new(
            artifact_id(b"source-cells"),
            artifact_id(b"source-vectors"),
            artifact_id(b"expected-cells"),
            artifact_id(b"identity-map"),
            artifact_id(b"converter"),
            &expected,
            &expected_hierarchy,
            entries,
            4_096,
        )
    };
    assert!(build(vec![
        CellEmbeddingRowLinkEntry::present(cell("cell-a"), 0, 0),
        CellEmbeddingRowLinkEntry::missing_vector(cell("cell-b"), 0),
        CellEmbeddingRowLinkEntry::qc_rejected(cell("cell-c"), 2, 1),
    ])
    .is_err());
    assert!(build(vec![
        CellEmbeddingRowLinkEntry::present(cell("cell-a"), 0, 1),
        CellEmbeddingRowLinkEntry::missing_vector(cell("cell-b"), 1),
        CellEmbeddingRowLinkEntry::qc_rejected(cell("cell-c"), 2, 2),
    ])
    .is_err());
    assert!(CellEmbeddingRowLinkEntry::new(
        cell("cell-a"),
        EmbeddingStatus::MissingVector,
        0,
        Some(0),
    )
    .is_err());

    let singleton = ExpectedCellSet::new("all.v1", vec![cell("cell-a")]).expect("expected");
    let patient = HierarchyId::from(PatientId::new("patient").expect("patient ID"));
    let wrong_kind = CohortHierarchy::new(
        vec![
            HierarchyNode::new(patient.clone(), None, ReplicationRole::BiologicalUnit),
            HierarchyNode::new(
                HierarchyId::from(SlideId::new("cell-a").expect("slide ID")),
                None,
                ReplicationRole::TechnicalReplicate {
                    biological_source: patient,
                },
            ),
        ],
        Vec::new(),
    )
    .expect("hierarchy with same text but wrong kind");
    assert!(CellEmbeddingRowLink::new(
        artifact_id(b"source-cells"),
        artifact_id(b"source-vectors"),
        artifact_id(b"expected-cells"),
        artifact_id(b"identity-map"),
        artifact_id(b"converter"),
        &singleton,
        &wrong_kind,
        vec![CellEmbeddingRowLinkEntry::present(cell("cell-a"), 0, 0)],
        4_096,
    )
    .is_err());

    let extraction = CellEmbeddingRowLink::new(
        artifact_id(b"source-cells"),
        artifact_id(b"source-vectors"),
        artifact_id(b"expected-cells"),
        artifact_id(b"identity-map"),
        artifact_id(b"converter"),
        &singleton,
        &hierarchy(&singleton),
        vec![CellEmbeddingRowLinkEntry::extraction_failed(
            cell("cell-a"),
            0,
        )],
        4_096,
    )
    .expect("extraction-failed row link");
    assert_eq!(
        extraction.entries()[0].status(),
        EmbeddingStatus::ExtractionFailed
    );
    assert_eq!(extraction.entries()[0].source_embedding_row(), None);

    let empty = ExpectedCellSet::new("none.v1", Vec::new()).expect("empty expected set");
    let empty_hierarchy = CohortHierarchy::new(
        vec![HierarchyNode::new(
            HierarchyId::from(PatientId::new("patient").expect("patient ID")),
            None,
            ReplicationRole::BiologicalUnit,
        )],
        Vec::new(),
    )
    .expect("empty cell hierarchy");
    assert_eq!(
        CellEmbeddingRowLink::new(
            artifact_id(b"source-cells"),
            artifact_id(b"source-vectors"),
            artifact_id(b"expected-cells"),
            artifact_id(b"identity-map"),
            artifact_id(b"converter"),
            &empty,
            &empty_hierarchy,
            Vec::new(),
            0,
        )
        .expect("empty row link")
        .row_count(),
        0
    );

    assert!(CellEmbeddingRowLink::new(
        artifact_id(b"same"),
        artifact_id(b"same"),
        artifact_id(b"expected-cells"),
        artifact_id(b"identity-map"),
        artifact_id(b"converter"),
        &singleton,
        &hierarchy(&singleton),
        vec![CellEmbeddingRowLinkEntry::present(cell("cell-a"), 0, 0)],
        4_096,
    )
    .is_err());

    assert!(CellEmbeddingRowLink::new(
        artifact_id(b"source-cells"),
        artifact_id(b"source-vectors"),
        artifact_id(b"expected-cells"),
        artifact_id(b"identity-map"),
        artifact_id(b"converter"),
        &singleton,
        &hierarchy(&singleton),
        vec![CellEmbeddingRowLinkEntry::present(cell("cell-a"), 0, 0)],
        0,
    )
    .is_err());
}

#[test]
fn provenance_json_is_canonical_complete_and_dependency_bound() {
    let provenance = provenance(artifact_id(b"row-link"));
    let encoded = provenance
        .to_canonical_json()
        .expect("canonical provenance");
    assert!(encoded.ends_with(b"\n"));
    assert_eq!(
        CellEmbeddingProvenance::from_canonical_json(&encoded).expect("round trip"),
        provenance
    );
    assert_eq!(provenance.output_dimension(), 1_280);
    assert_eq!(
        ContentDigest::from_bytes(&encoded).to_string(),
        "b91ad252a378ec07bd617cbdf2943f4f51fde03d21d0b138bafcae0db77f30d8"
    );
    let dependencies = provenance.direct_dependencies();
    assert_eq!(dependencies.len(), 13);
    assert!(dependencies.windows(2).all(|pair| pair[0] < pair[1]));

    let mut noncanonical = encoded.clone();
    noncanonical.insert(0, b' ');
    assert!(CellEmbeddingProvenance::from_canonical_json(&noncanonical).is_err());
    assert!(CanonicalDecimal::new("0.500").is_err());
    assert!(CanonicalDecimal::new("1e-3").is_err());
}

#[test]
fn provenance_rejects_unknown_fields_role_collisions_and_non_tokens() {
    let provenance = provenance(artifact_id(b"row-link"));
    let encoded = provenance
        .to_canonical_json()
        .expect("canonical provenance");
    let mut unknown = String::from_utf8(encoded.clone()).expect("UTF-8 provenance");
    unknown.replace_range(unknown.len() - 2..unknown.len() - 1, ",\"unknown\":1}");
    assert!(CellEmbeddingProvenance::from_canonical_json(unknown.as_bytes()).is_err());

    let source_cells = artifact_id(b"source-cells").to_string();
    let row_link = artifact_id(b"row-link").to_string();
    let duplicate_role = String::from_utf8(encoded)
        .expect("UTF-8 provenance")
        .replace(&row_link, &source_cells);
    assert!(CellEmbeddingProvenance::from_canonical_json(duplicate_role.as_bytes()).is_err());

    assert!(CellEmbeddingModelProvenance::new(
        "cellvit_sam_h",
        "1.0",
        "sam_h",
        artifact_id(b"checkpoint"),
        ContentDigest::from_bytes(b"checkpoint-bytes"),
        artifact_id(b"source-snapshot"),
        artifact_id(b"license"),
        "Apache-2.0",
        "https://example.invalid/license",
        "z4",
        32,
    )
    .is_err());
    assert!(CellEmbeddingProvenance::from_canonical_json(&vec![b' '; 64 * 1024 + 1]).is_err());
    assert!(matches!(
        CellEmbeddingModelProvenance::new(
            "cellvit_sam_h",
            "1.0",
            "sam_h",
            artifact_id(b"checkpoint"),
            ContentDigest::from_bytes(b"checkpoint-bytes"),
            artifact_id(b"source-snapshot"),
            artifact_id(b"license"),
            "Apache-2.0",
            "",
            "z4",
            32,
        ),
        Err(EmbeddingError::ProvenanceIncomplete)
    ));
}

#[test]
fn provenance_json_distinguishes_missing_state_from_invalid_values() {
    let encoded = String::from_utf8(
        provenance(artifact_id(b"row-link"))
            .to_canonical_json()
            .expect("canonical provenance"),
    )
    .expect("UTF-8 provenance");
    let missing = encoded.replace("\"model_family\":\"cellvit_sam_h\",", "");
    assert!(matches!(
        CellEmbeddingProvenance::from_canonical_json(missing.as_bytes()),
        Err(EmbeddingError::ProvenanceIncomplete)
    ));

    let duplicate = encoded.replacen("\"version\":1,", "\"version\":1,\"version\":1,", 1);
    assert!(matches!(
        CellEmbeddingProvenance::from_canonical_json(duplicate.as_bytes()),
        Err(EmbeddingError::InvalidProvenanceJson)
    ));
    for invalid in [
        encoded.replacen("\"version\":1", "\"version\":2", 1),
        encoded.replacen(
            "nucleus_bbox_intersecting_token_mean",
            "unsupported_pooling",
            1,
        ),
        encoded.replacen(
            &artifact_id(b"checkpoint").to_string(),
            &artifact_id(b"checkpoint").to_string().to_uppercase(),
            1,
        ),
    ] {
        assert!(matches!(
            CellEmbeddingProvenance::from_canonical_json(invalid.as_bytes()),
            Err(EmbeddingError::InvalidProvenanceJson)
        ));
    }
}

#[test]
fn canonical_decimals_accept_only_fixed_point_canonical_text() {
    for value in ["0", "1", "-1", "0.1", "-0.1", "123456789.987654321"] {
        assert_eq!(
            CanonicalDecimal::new(value)
                .expect("valid canonical decimal")
                .as_str(),
            value
        );
    }
    let long = "1".repeat(65);
    assert_eq!(
        CanonicalDecimal::new(long.clone())
            .expect("valid long canonical decimal")
            .as_str(),
        long
    );
    for value in [
        "", "+1", "-0", "00", "01", "1.", ".1", "0.0", "1.20", "1e2", "NaN", "inf",
    ] {
        assert!(CanonicalDecimal::new(value).is_err(), "accepted {value:?}");
    }
    assert!(CellEmbeddingTensorContract::rgb_he_raw(
        1_280,
        decimals(["0.485", "0.456", "0.406"]),
        decimals(["0.229", "0", "0.225"]),
    )
    .is_err());
    assert!(CellEmbeddingTensorContract::rgb_he_raw(
        1_024,
        decimals(["0.485", "0.456", "0.406"]),
        decimals(["0.229", "0.224", "0.225"]),
    )
    .is_err());
}
