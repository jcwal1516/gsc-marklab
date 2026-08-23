use super::*;

pub(crate) fn hierarchy(entity_count: usize) -> (CohortHierarchy, SlideId) {
    let patient = HierarchyId::from(PatientId::new("graph-patient").expect("patient ID"));
    let slide_id = SlideId::new("graph-slide").expect("slide ID");
    let slide = HierarchyId::from(slide_id.clone());
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
    nodes.extend((0..entity_count).map(|index| {
        HierarchyNode::new(
            HierarchyId::from(patch_at(index)),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    (
        CohortHierarchy::new(nodes, Vec::new()).expect("hierarchy"),
        slide_id,
    )
}

pub(crate) fn context(
    hierarchy: &CohortHierarchy,
    slide: SlideId,
    entity_count: usize,
) -> PatchEmbeddingContext {
    let image_id = CoordinateFrameId::new("graph-image-pixels").expect("image frame ID");
    let physical_id = CoordinateFrameId::new("graph-slide-micrometers").expect("physical frame ID");
    let transform_id = TransformId::new("graph-pixel-to-micrometer").expect("transform ID");
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
        TransformMatrix::affine_2d([0.5, 0.0, 0.0, 0.0, 0.5, 0.0]).expect("matrix"),
        None,
    );
    let registry = CoordinateRegistry::new(
        vec![image, physical],
        Vec::new(),
        vec![transform],
        Vec::new(),
    )
    .expect("coordinate registry");
    let final_origin = u64::try_from(entity_count - 1).expect("entity count") * 192;
    PatchEmbeddingContext::new(
        hierarchy,
        &registry,
        slide,
        image_id,
        physical_id,
        transform_id,
        PositiveRational::new(1, 2).expect("x scale"),
        PositiveRational::new(1, 2).expect("y scale"),
        [final_origin + 224, 224],
        [224, 224],
        [192, 192],
        [32, 32],
        EffectiveReceptiveField::FullInput,
        PatchBoundaryPolicy::FullyContainedOnly,
        BUDGET,
    )
    .expect("patch context")
}

pub(crate) fn footprint_manifest(
    row_count: u64,
    parquet: bool,
    mutation: Option<PhysicalManifestMutation>,
) -> TableManifest {
    let mut columns = vec![
        TableColumn::new(
            "patch_id",
            TableColumnType::Scalar(TableScalarType::Utf8),
            false,
        )
        .expect("patch ID column"),
        TableColumn::new(
            "origin_x_px",
            TableColumnType::Scalar(TableScalarType::I64),
            mutation == Some(PhysicalManifestMutation::Nullability),
        )
        .expect("origin x column"),
        TableColumn::new(
            "origin_y_px",
            TableColumnType::Scalar(TableScalarType::I64),
            false,
        )
        .expect("origin y column"),
    ];
    if mutation == Some(PhysicalManifestMutation::Columns) {
        columns.push(
            TableColumn::new(
                "unexpected_column",
                TableColumnType::Scalar(TableScalarType::I64),
                false,
            )
            .expect("unexpected column"),
        );
    }
    TableManifest::new(
        if mutation == Some(PhysicalManifestMutation::Format) {
            if parquet {
                TableFormat::ArrowIpcFile
            } else {
                TableFormat::ParquetFile
            }
        } else if parquet {
            TableFormat::ParquetFile
        } else {
            TableFormat::ArrowIpcFile
        },
        if mutation == Some(PhysicalManifestMutation::Encoding) {
            "marklab.wrong.patch-footprint-table.v1"
        } else if parquet {
            "marklab.parquet.patch-footprint-table.v1"
        } else {
            "marklab.arrow-ipc.patch-footprint-table.v1"
        },
        if mutation == Some(PhysicalManifestMutation::RowCount) {
            row_count + 1
        } else {
            row_count
        },
        columns,
        if mutation == Some(PhysicalManifestMutation::PrimaryKey) {
            vec!["origin_y_px".to_owned()]
        } else {
            vec!["patch_id".to_owned()]
        },
    )
    .expect("footprint manifest")
}

pub(crate) fn overlap_manifest(
    row_count: u64,
    parquet: bool,
    mutation: Option<PhysicalManifestMutation>,
) -> TableManifest {
    let nullable_right = mutation == Some(PhysicalManifestMutation::Nullability);
    let mut columns = vec![
        TableColumn::new(
            "left_patch_id",
            TableColumnType::Scalar(TableScalarType::Utf8),
            false,
        )
        .expect("left patch column"),
        TableColumn::new(
            "right_patch_id",
            TableColumnType::Scalar(TableScalarType::Utf8),
            nullable_right,
        )
        .expect("right patch column"),
    ];
    if mutation == Some(PhysicalManifestMutation::Columns) {
        columns.push(
            TableColumn::new(
                "unexpected_column",
                TableColumnType::Scalar(TableScalarType::I64),
                false,
            )
            .expect("unexpected column"),
        );
    }
    TableManifest::new(
        if mutation == Some(PhysicalManifestMutation::Format) {
            if parquet {
                TableFormat::ArrowIpcFile
            } else {
                TableFormat::ParquetFile
            }
        } else if parquet {
            TableFormat::ParquetFile
        } else {
            TableFormat::ArrowIpcFile
        },
        if mutation == Some(PhysicalManifestMutation::Encoding) {
            "marklab.wrong.patch-overlap-edge-table.v1"
        } else if parquet {
            "marklab.parquet.patch-overlap-edge-table.v1"
        } else {
            "marklab.arrow-ipc.patch-overlap-edge-table.v1"
        },
        if mutation == Some(PhysicalManifestMutation::RowCount) {
            row_count + 1
        } else {
            row_count
        },
        columns,
        if nullable_right || mutation == Some(PhysicalManifestMutation::PrimaryKey) {
            vec!["left_patch_id".to_owned()]
        } else {
            vec!["left_patch_id".to_owned(), "right_patch_id".to_owned()]
        },
    )
    .expect("overlap manifest")
}
