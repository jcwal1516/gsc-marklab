use std::str::FromStr;

use marklab_data::{
    CellId, CoordinateFrame, CoordinateFrameId, CoordinateRegistry, CoordinateSpace,
    CoordinateUnit, FrameTransform, ImageCoordinateConvention, SpatialAxis, TransformId,
    TransformMatrix,
};
use marklab_embeddings::{
    CellEmbeddingRow, CellEmbeddingTable, CellIdentityMap, CellIdentityMapEntry, EmbeddingError,
    EmbeddingSpatialContext, EmbeddingStatus, ExpectedCellSet, PatchBoundaryPolicy,
    PositiveRational,
};
use marklab_project::{ArtifactId, ContentDigest};
use proptest::prelude::*;

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

fn registry() -> CoordinateRegistry {
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
        transform_id,
        image_id,
        physical_id,
        TransformMatrix::affine_2d([0.5, 0.0, 1.25, 0.0, 0.25, -2.0]).expect("matrix"),
        None,
    );
    CoordinateRegistry::new(
        vec![image, physical],
        Vec::new(),
        vec![transform],
        Vec::new(),
    )
    .expect("registry")
}

fn context(registry: &CoordinateRegistry) -> EmbeddingSpatialContext {
    EmbeddingSpatialContext::new(
        registry,
        CoordinateFrameId::new("image-pixels").expect("image frame ID"),
        CoordinateFrameId::new("slide-micrometers").expect("physical frame ID"),
        TransformId::new("pixel-to-micrometer").expect("transform ID"),
        PositiveRational::new(1, 2).expect("x scale"),
        PositiveRational::new(1, 4).expect("y scale"),
        [1_024, 1_024],
        [64, 64],
        [16, 16],
        PatchBoundaryPolicy::FullyContainedOnly,
    )
    .expect("context")
}

#[test]
fn expected_set_and_identity_map_have_fixed_logical_vectors() {
    let expected = expected();
    assert_eq!(
        expected.logical_digest().to_string(),
        "2d5a163a0c9640dc8283456add120c67f1d3f49bb91637e9628cae781a9358c6"
    );
    let identity_map = CellIdentityMap::new(
        artifact_id(b"source-cells"),
        artifact_id(b"expected-cells"),
        &expected,
        vec![
            CellIdentityMapEntry::new("source-a", cell("cell-a")).expect("entry"),
            CellIdentityMapEntry::new("source-b", cell("cell-b")).expect("entry"),
            CellIdentityMapEntry::new("source-c", cell("cell-c")).expect("entry"),
        ],
    )
    .expect("identity map");
    assert_eq!(
        identity_map.logical_digest().to_string(),
        "a1dffc6c4e1252b48e637a9b69b5367c20a60627ae50bde5ce49ba9ede56364a"
    );
}

#[test]
fn binary_decoders_reject_noncanonical_order_truncation_and_resource_excess() {
    let expected = expected();
    let encoded = expected.to_bytes().expect("expected bytes");
    assert!(matches!(
        ExpectedCellSet::from_bytes(&encoded, encoded.len() - 1),
        Err(EmbeddingError::EncodedByteBudgetExceeded { .. })
    ));
    assert!(matches!(
        ExpectedCellSet::from_bytes(&encoded[..encoded.len() - 1], encoded.len()),
        Err(EmbeddingError::InvalidBinaryEncoding)
    ));
    assert!(matches!(
        ExpectedCellSet::new("all-qc-eligible.v1", vec![cell("cell-b"), cell("cell-a")]),
        Err(EmbeddingError::NonCanonicalCellOrder)
    ));
    assert!(matches!(
        CellIdentityMapEntry::new("source\nsecret", cell("cell-a")),
        Err(EmbeddingError::InvalidSourceCellId)
    ));
}

#[test]
fn context_json_is_a_strict_fixed_point_and_registry_drift_is_rejected() {
    let registry = registry();
    let context = context(&registry);
    let encoded = context.to_canonical_json().expect("context JSON");
    let expected_json = concat!(
        "{\"format\":\"marklab.cell_embedding_spatial_context\",\"version\":1,",
        "\"image_frame_id\":\"image-pixels\",\"image_axes\":[\"x\",\"y\"],",
        "\"image_space\":\"image\",\"image_unit\":\"pixel\",",
        "\"pixel_convention\":\"pixel_center_at_integer\",",
        "\"physical_frame_id\":\"slide-micrometers\",",
        "\"physical_axes\":[\"x\",\"y\"],\"physical_space\":\"physical\",",
        "\"physical_unit\":\"micrometer\",",
        "\"pixel_to_physical_transform_id\":\"pixel-to-micrometer\",",
        "\"transform_uncertainty\":{\"kind\":\"none\"},",
        "\"pixel_to_physical_matrix_f64_bits\":[\"3fe0000000000000\",",
        "\"0000000000000000\",\"3ff4000000000000\",\"0000000000000000\",",
        "\"3fd0000000000000\",\"c000000000000000\"],",
        "\"mpp_x\":{\"numerator\":1,\"denominator\":2},",
        "\"mpp_y\":{\"numerator\":1,\"denominator\":4},",
        "\"patch_width_px\":1024,\"patch_height_px\":1024,",
        "\"overlap_x_px\":64,\"overlap_y_px\":64,",
        "\"stride_x_px\":960,\"stride_y_px\":960,",
        "\"token_patch_width_px\":16,\"token_patch_height_px\":16,",
        "\"patch_boundary_policy\":{\"kind\":\"fully_contained_only\"},",
        "\"effective_context\":\"nucleus_bbox_intersecting_encoder_tokens_within_patch\"}\n"
    );
    assert_eq!(encoded, expected_json.as_bytes());
    assert_eq!(
        EmbeddingSpatialContext::from_canonical_json(&encoded, &registry)
            .expect("canonical round trip"),
        context
    );

    let mut noncanonical = encoded.clone();
    noncanonical.insert(0, b' ');
    assert!(matches!(
        EmbeddingSpatialContext::from_canonical_json(&noncanonical, &registry),
        Err(EmbeddingError::InvalidCanonicalJson)
    ));
    assert!(matches!(
        PositiveRational::new(2, 4),
        Err(EmbeddingError::InvalidPositiveRational)
    ));
}

#[test]
fn table_digest_status_counts_and_signed_zero_are_fixed() {
    let expected = expected();
    let table = CellEmbeddingTable::from_rows(
        3,
        &expected,
        artifact_id(b"expected-cells"),
        artifact_id(b"provenance"),
        ContentDigest::from_bytes(b"row-link"),
        vec![
            CellEmbeddingRow::present(cell("cell-a"), vec![1.0, -0.0, 3.0]),
            CellEmbeddingRow::non_present(cell("cell-b"), EmbeddingStatus::MissingVector)
                .expect("missing"),
            CellEmbeddingRow::non_present(cell("cell-c"), EmbeddingStatus::QcRejected)
                .expect("rejected"),
        ],
        1_024,
    )
    .expect("table");
    let qc = table.qc_summary();
    assert_eq!(
        qc.logical_digest().to_string(),
        "e40f40008b6ff36cbbb201ff8fca97f95699a70bd4402fed00dd05a698a05887"
    );
    assert_eq!(qc.present_count(), 1);
    assert_eq!(qc.missing_vector_count(), 1);
    assert_eq!(qc.extraction_failed_count(), 0);
    assert_eq!(qc.qc_rejected_count(), 1);
    assert_eq!(
        table.row(0).expect("row").vector().expect("vector")[1].to_bits(),
        0
    );
    assert!(table.row(1).expect("row").vector().is_none());
}

proptest! {
    #[test]
    fn finite_present_vectors_round_trip_with_only_signed_zero_canonicalized(
        values in prop::collection::vec(any::<f32>().prop_filter("finite", |value| value.is_finite()), 1..32)
    ) {
        let expected = ExpectedCellSet::new("property.v1", vec![cell("cell")])
            .expect("expected");
        let dimension = u32::try_from(values.len()).expect("small dimension");
        let canonical = values
            .iter()
            .map(|value| if *value == 0.0 { 0.0 } else { *value })
            .collect::<Vec<_>>();
        let table = CellEmbeddingTable::from_rows(
            dimension,
            &expected,
            artifact_id(b"expected"),
            artifact_id(b"provenance"),
            ContentDigest::from_bytes(b"row-link"),
            vec![CellEmbeddingRow::present(cell("cell"), values)],
            4_096,
        )
        .expect("table");
        prop_assert_eq!(table.row(0).expect("row").vector(), Some(canonical.as_slice()));
    }
}
