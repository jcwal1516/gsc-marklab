use std::str::FromStr;

use marklab::{
    ArtifactId, CellEmbeddingRow, CellEmbeddingTable, CellId, CellIdentityMap,
    CellIdentityMapEntry, CohortHierarchy, ContentDigest, CoordinateFrame, CoordinateFrameId,
    CoordinateRegistry, CoordinateSpace, CoordinateUnit, EmbeddingSpatialContext, EmbeddingStatus,
    ExpectedCellSet, FrameTransform, HierarchyId, HierarchyNode, ImageCoordinateConvention,
    PatchBoundaryPolicy, PatientId, PositiveRational, ReplicationRole, SlideId, SpatialAxis,
    TransformId, TransformMatrix,
};

fn artifact_id(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

fn cell(value: &str) -> CellId {
    CellId::new(value).expect("cell ID")
}

fn expected_cells() -> ExpectedCellSet {
    ExpectedCellSet::new(
        "all-qc-eligible.v1",
        vec![cell("cell-a"), cell("cell-b"), cell("cell-c")],
    )
    .expect("expected cell set")
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
    CohortHierarchy::new(nodes, Vec::new()).expect("valid cell hierarchy")
}

fn coordinate_registry() -> (
    CoordinateRegistry,
    CoordinateFrameId,
    CoordinateFrameId,
    TransformId,
) {
    let image_id = CoordinateFrameId::new("image-pixels").expect("image frame ID");
    let physical_id = CoordinateFrameId::new("slide-micrometers").expect("physical frame ID");
    let transform_id = TransformId::new("pixel-to-micrometer").expect("transform ID");
    let image = CoordinateFrame::new(
        image_id.clone(),
        vec![SpatialAxis::X, SpatialAxis::Y],
        CoordinateUnit::Pixel,
        CoordinateSpace::Image(ImageCoordinateConvention::PixelCenterAtInteger),
    )
    .expect("image frame");
    let physical = CoordinateFrame::new(
        physical_id.clone(),
        vec![SpatialAxis::X, SpatialAxis::Y],
        CoordinateUnit::Micrometer,
        CoordinateSpace::Physical,
    )
    .expect("physical frame");
    let transform = FrameTransform::new(
        transform_id.clone(),
        image_id.clone(),
        physical_id.clone(),
        TransformMatrix::affine_2d([0.5, 0.0, 1.25, 0.0, 0.25, -2.0]).expect("affine matrix"),
        None,
    );
    let registry = CoordinateRegistry::new(
        vec![image, physical],
        Vec::new(),
        vec![transform],
        Vec::new(),
    )
    .expect("coordinate registry");
    (registry, image_id, physical_id, transform_id)
}

#[test]
fn expected_cells_have_strict_streamable_canonical_bytes() {
    let expected = expected_cells();
    let hierarchy = hierarchy(&expected);
    assert!(expected
        .cells()
        .iter()
        .cloned()
        .map(HierarchyId::from)
        .all(|cell_id| hierarchy.contains(&cell_id)));
    assert_eq!(
        expected
            .cells()
            .iter()
            .map(CellId::as_str)
            .collect::<Vec<_>>(),
        ["cell-a", "cell-b", "cell-c"]
    );
    assert_eq!(expected.selection_rule(), "all-qc-eligible.v1");

    let encoded = expected.to_bytes().expect("canonical expected-cell bytes");
    assert!(encoded.starts_with(b"ML-ECS\0\x01"));
    assert_eq!(
        ExpectedCellSet::from_bytes(&encoded, encoded.len()).expect("expected-cell round trip"),
        expected
    );
    assert!(ExpectedCellSet::from_bytes(&encoded, encoded.len() - 1).is_err());
    assert!(
        ExpectedCellSet::new("all-qc-eligible.v1", vec![cell("cell-b"), cell("cell-a")]).is_err()
    );
    assert!(
        ExpectedCellSet::new("all-qc-eligible.v1", vec![cell("cell-a"), cell("cell-a")]).is_err()
    );
}

#[test]
fn identity_map_requires_an_exact_one_to_one_expected_cell_range() {
    let expected = expected_cells();
    let source_cells = artifact_id(b"source-cells");
    let expected_artifact = artifact_id(b"expected-cells");
    let entries = vec![
        CellIdentityMapEntry::new("source-a", cell("cell-a")).expect("entry"),
        CellIdentityMapEntry::new("source-b", cell("cell-b")).expect("entry"),
        CellIdentityMapEntry::new("source-c", cell("cell-c")).expect("entry"),
    ];
    let identity_map =
        CellIdentityMap::new(source_cells, expected_artifact, &expected, entries.clone())
            .expect("identity map");
    let encoded = identity_map
        .to_bytes()
        .expect("canonical identity-map bytes");
    assert!(encoded.starts_with(b"ML-CIM\0\x01"));
    assert_eq!(
        CellIdentityMap::from_bytes(
            &encoded,
            encoded.len(),
            source_cells,
            expected_artifact,
            &expected,
        )
        .expect("identity-map round trip"),
        identity_map
    );

    let mut duplicate_source = entries.clone();
    duplicate_source[1] =
        CellIdentityMapEntry::new("source-a", cell("cell-b")).expect("duplicate source entry");
    assert!(
        CellIdentityMap::new(source_cells, expected_artifact, &expected, duplicate_source,)
            .is_err()
    );
    assert!(CellIdentityMap::new(
        source_cells,
        expected_artifact,
        &expected,
        entries.into_iter().take(2).collect(),
    )
    .is_err());
}

#[test]
fn spatial_context_freezes_both_frames_transform_scale_stride_and_boundary() {
    let (registry, image, physical, transform) = coordinate_registry();
    let context = EmbeddingSpatialContext::new(
        &registry,
        image,
        physical,
        transform,
        PositiveRational::new(1, 2).expect("x scale"),
        PositiveRational::new(1, 4).expect("y scale"),
        [1_024, 1_024],
        [64, 64],
        [16, 16],
        PatchBoundaryPolicy::FullyContainedOnly,
    )
    .expect("spatial context");
    assert_eq!(context.stride_px(), [960, 960]);
    let encoded = context.to_canonical_json().expect("canonical context JSON");
    assert!(encoded.ends_with(b"\n"));
    assert_eq!(
        EmbeddingSpatialContext::from_canonical_json(&encoded, &registry)
            .expect("context round trip"),
        context
    );

    let (drifted_registry, drifted_image, drifted_physical, drifted_transform) = {
        let image_id = CoordinateFrameId::new("image-pixels").expect("image frame ID");
        let physical_id = CoordinateFrameId::new("slide-micrometers").expect("physical frame ID");
        let transform_id = TransformId::new("pixel-to-micrometer").expect("transform ID");
        let image_frame = CoordinateFrame::new(
            image_id.clone(),
            vec![SpatialAxis::X, SpatialAxis::Y],
            CoordinateUnit::Pixel,
            CoordinateSpace::Image(ImageCoordinateConvention::PixelCenterAtInteger),
        )
        .expect("image frame");
        let physical_frame = CoordinateFrame::new(
            physical_id.clone(),
            vec![SpatialAxis::X, SpatialAxis::Y],
            CoordinateUnit::Micrometer,
            CoordinateSpace::Physical,
        )
        .expect("physical frame");
        let changed = FrameTransform::new(
            transform_id.clone(),
            image_id.clone(),
            physical_id.clone(),
            TransformMatrix::affine_2d([0.5, 0.0, 1.5, 0.0, 0.25, -2.0]).expect("changed matrix"),
            None,
        );
        (
            CoordinateRegistry::new(
                vec![image_frame, physical_frame],
                Vec::new(),
                vec![changed],
                Vec::new(),
            )
            .expect("drifted registry"),
            image_id,
            physical_id,
            transform_id,
        )
    };
    assert_eq!(drifted_image.as_str(), context.image_frame_id().as_str());
    assert_eq!(
        drifted_physical.as_str(),
        context.physical_frame_id().as_str()
    );
    assert_eq!(drifted_transform.as_str(), context.transform_id().as_str());
    assert!(EmbeddingSpatialContext::from_canonical_json(&encoded, &drifted_registry).is_err());
}

#[test]
fn table_uses_explicit_statuses_and_never_exposes_private_fillers() {
    let expected = expected_cells();
    let table = CellEmbeddingTable::from_rows(
        3,
        &expected,
        artifact_id(b"expected-cells"),
        artifact_id(b"provenance"),
        ContentDigest::from_bytes(b"row-link"),
        vec![
            CellEmbeddingRow::present(cell("cell-a"), vec![1.0, -0.0, 3.0]),
            CellEmbeddingRow::non_present(cell("cell-b"), EmbeddingStatus::MissingVector)
                .expect("missing row"),
            CellEmbeddingRow::non_present(cell("cell-c"), EmbeddingStatus::QcRejected)
                .expect("rejected row"),
        ],
        1_024,
    )
    .expect("embedding table");

    assert_eq!(table.row_count(), 3);
    assert_eq!(table.dimension(), 3);
    assert_eq!(
        table.row(0).expect("present row").vector(),
        Some(&[1.0, 0.0, 3.0][..])
    );
    assert_eq!(
        table.row(1).expect("missing row").status(),
        EmbeddingStatus::MissingVector
    );
    assert!(table.row(1).expect("missing row").vector().is_none());
    assert!(table.row(2).expect("rejected row").vector().is_none());
    assert_eq!(table.qc_summary().row_count(), 3);
    assert_eq!(table.qc_summary().present_count(), 1);
    assert_eq!(table.qc_summary().missing_vector_count(), 1);
    assert_eq!(table.qc_summary().qc_rejected_count(), 1);
    assert_eq!(table.qc_summary().all_zero_present_count(), 0);

    let block = table.block(1, 2).expect("status-aware block");
    assert_eq!(block.row_count(), 2);
    assert_eq!(block.dimension(), 3);
    assert_eq!(
        block.row(0).expect("block missing row").status(),
        EmbeddingStatus::MissingVector
    );
    assert!(block.row(0).expect("block missing row").vector().is_none());
    assert!(block.row(1).expect("block rejected row").vector().is_none());
    assert_eq!(block.rows().len(), 2);
    assert!(block.rows().all(|row| row.vector().is_none()));
    for debug in [format!("{table:?}"), format!("{block:?}")] {
        assert!(!debug.contains("values"));
        assert!(!debug.contains("1.0"));
        assert!(!debug.contains("3.0"));
    }

    let empty = table.block(table.row_count(), 0).expect("empty tail block");
    assert_eq!(empty.row_count(), 0);
    assert!(empty.rows().next().is_none());
    assert!(table.block(table.row_count() + 1, 0).is_err());
    assert!(table.block(2, 2).is_err());
    assert!(table.block(usize::MAX, 2).is_err());

    for maximum_block_rows in [1, 2, 3, 8_192] {
        assert_eq!(
            table
                .scan_qc(maximum_block_rows)
                .expect("chunk-independent QC scan"),
            table.qc_summary()
        );
    }
    assert!(table.scan_qc(0).is_err());
}

#[test]
fn table_rejects_row_set_status_dimension_finiteness_and_budget_errors() {
    let expected = expected_cells();
    let build = |rows, budget| {
        CellEmbeddingTable::from_rows(
            2,
            &expected,
            artifact_id(b"expected-cells"),
            artifact_id(b"provenance"),
            ContentDigest::from_bytes(b"row-link"),
            rows,
            budget,
        )
    };
    assert!(build(
        vec![
            CellEmbeddingRow::present(cell("cell-a"), vec![1.0, 2.0]),
            CellEmbeddingRow::present(cell("cell-b"), vec![3.0, 4.0]),
        ],
        1_024,
    )
    .is_err());
    assert!(build(
        vec![
            CellEmbeddingRow::present(cell("cell-b"), vec![1.0, 2.0]),
            CellEmbeddingRow::present(cell("cell-a"), vec![3.0, 4.0]),
            CellEmbeddingRow::present(cell("cell-c"), vec![5.0, 6.0]),
        ],
        1_024,
    )
    .is_err());
    assert!(build(
        vec![
            CellEmbeddingRow::present(cell("cell-a"), vec![1.0, f32::NAN]),
            CellEmbeddingRow::present(cell("cell-b"), vec![3.0, 4.0]),
            CellEmbeddingRow::present(cell("cell-c"), vec![5.0, 6.0]),
        ],
        1_024,
    )
    .is_err());
    assert!(build(
        vec![
            CellEmbeddingRow::present(cell("cell-a"), vec![1.0]),
            CellEmbeddingRow::present(cell("cell-b"), vec![3.0, 4.0]),
            CellEmbeddingRow::present(cell("cell-c"), vec![5.0, 6.0]),
        ],
        1_024,
    )
    .is_err());
    assert!(build(
        vec![
            CellEmbeddingRow::present(cell("cell-a"), vec![1.0, 2.0]),
            CellEmbeddingRow::present(cell("cell-b"), vec![3.0, 4.0]),
            CellEmbeddingRow::present(cell("cell-c"), vec![5.0, 6.0]),
        ],
        1,
    )
    .is_err());
}

#[test]
fn contiguous_present_values_construct_without_per_row_vectors() {
    let expected = expected_cells();
    let expected_id = artifact_id(b"expected-cells");
    let provenance_id = artifact_id(b"provenance");
    let row_link_digest = ContentDigest::from_bytes(b"row-link");
    let contiguous = CellEmbeddingTable::from_present_values(
        2,
        &expected,
        expected_id,
        provenance_id,
        row_link_digest,
        vec![1.0, -0.0, 3.0, 4.0, 0.0, 0.0],
        1_024,
    )
    .expect("contiguous table");
    let rowwise = CellEmbeddingTable::from_rows(
        2,
        &expected,
        expected_id,
        provenance_id,
        row_link_digest,
        vec![
            CellEmbeddingRow::present(cell("cell-a"), vec![1.0, 0.0]),
            CellEmbeddingRow::present(cell("cell-b"), vec![3.0, 4.0]),
            CellEmbeddingRow::present(cell("cell-c"), vec![0.0, 0.0]),
        ],
        1_024,
    )
    .expect("rowwise table");
    assert_eq!(contiguous, rowwise);
    assert_eq!(contiguous.qc_summary().all_zero_present_count(), 1);
    assert_eq!(
        contiguous.row(0).expect("first row").vector(),
        Some(&[1.0, 0.0][..])
    );
}
