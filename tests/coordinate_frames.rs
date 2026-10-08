use marklab_data::{
    CoordinateError, CoordinateFrame, CoordinateFrameId, CoordinateIdentityError,
    CoordinateIdentityKind, CoordinateRegistry, CoordinateSpace, CoordinateUnit, FrameTransform,
    ImageCoordinateConvention, SectionId, SerialSection, SerialSectionPlacement,
    SerialSectionSeries, SerialSectionStatus, SpatialAxis, SpatialDimension, TransformId,
    TransformMatrix, UncertaintyId, UncertaintyReference,
};

macro_rules! assert_coordinate_id_contract {
    ($id_type:ty, $kind:expr) => {{
        assert!(matches!(
            <$id_type>::new(""),
            Err(CoordinateIdentityError::Blank { kind }) if kind == $kind
        ));
        assert!(matches!(
            <$id_type>::new(" value"),
            Err(CoordinateIdentityError::SurroundingWhitespace { kind }) if kind == $kind
        ));
        assert!(matches!(
            <$id_type>::new("value\n"),
            Err(CoordinateIdentityError::ControlCharacter { kind }) if kind == $kind
        ));
        assert!(matches!(
            <$id_type>::new("x".repeat(256)),
            Err(CoordinateIdentityError::TooLong { kind, byte_len: 256 }) if kind == $kind
        ));
    }};
}

fn frame(value: &str) -> CoordinateFrameId {
    CoordinateFrameId::new(value).expect("coordinate-frame ID")
}

fn transform(value: &str) -> TransformId {
    TransformId::new(value).expect("transform ID")
}

fn uncertainty(value: &str) -> UncertaintyId {
    UncertaintyId::new(value).expect("uncertainty ID")
}

fn section_id(value: &str) -> SectionId {
    SectionId::new(value).expect("section ID")
}

fn missing_section(value: &str, ordinal: u32, z_center: f64) -> SerialSection {
    SerialSection::new(
        section_id(value),
        ordinal,
        z_center,
        4.0,
        SerialSectionStatus::Missing,
        None,
    )
    .expect("missing serial section")
}

fn observed_series(
    volume: CoordinateFrameId,
    section_value: &str,
    source: CoordinateFrameId,
    uncertainty: Option<UncertaintyId>,
) -> SerialSectionSeries {
    let placement =
        SerialSectionPlacement::new(source, [1.0, 0.0, 0.0, 0.0, 1.0, 0.0], uncertainty)
            .expect("serial-section placement");
    let observed = SerialSection::new(
        section_id(section_value),
        1,
        0.0,
        4.0,
        SerialSectionStatus::Observed,
        Some(placement),
    )
    .expect("observed serial section");
    SerialSectionSeries::new(volume, vec![observed]).expect("serial-section series")
}

fn physical_frame(value: &str, axes: Vec<SpatialAxis>) -> CoordinateFrame {
    CoordinateFrame::new(
        frame(value),
        axes,
        CoordinateUnit::Micrometer,
        CoordinateSpace::Physical,
    )
    .expect("physical frame")
}

#[test]
fn coordinate_ids_are_typed_opaque_and_bounded() {
    assert_coordinate_id_contract!(CoordinateFrameId, CoordinateIdentityKind::CoordinateFrame);
    assert_coordinate_id_contract!(TransformId, CoordinateIdentityKind::Transform);
    assert_coordinate_id_contract!(UncertaintyId, CoordinateIdentityKind::Uncertainty);

    let opaque = "slide-01/level-0::pixel-frame";
    assert_eq!(
        CoordinateFrameId::new(opaque)
            .expect("opaque frame ID")
            .as_str(),
        opaque
    );
    assert_eq!(
        TransformId::new("x".repeat(255))
            .expect("255-byte transform ID")
            .as_str()
            .len(),
        255
    );
    assert_eq!(
        CoordinateFrameId::new("shared").expect("frame ID").as_str(),
        TransformId::new("shared").expect("transform ID").as_str(),
        "the same opaque text may belong to distinct ID types"
    );
}

#[test]
fn frames_retain_axis_order_dimension_space_and_unit() {
    let pixel = CoordinateFrame::new(
        frame("pixel-yx"),
        vec![SpatialAxis::Y, SpatialAxis::X],
        CoordinateUnit::Pixel,
        CoordinateSpace::Image(ImageCoordinateConvention::PixelCenterAtInteger),
    )
    .expect("valid image frame");
    assert_eq!(pixel.axes(), &[SpatialAxis::Y, SpatialAxis::X]);
    assert_eq!(pixel.dimension(), SpatialDimension::Two);
    assert_eq!(pixel.unit(), CoordinateUnit::Pixel);
    assert_eq!(
        pixel.space(),
        CoordinateSpace::Image(ImageCoordinateConvention::PixelCenterAtInteger)
    );

    let volume = CoordinateFrame::new(
        frame("physical-zyx"),
        vec![SpatialAxis::Z, SpatialAxis::Y, SpatialAxis::X],
        CoordinateUnit::Micrometer,
        CoordinateSpace::Physical,
    )
    .expect("valid physical volume frame");
    assert_eq!(volume.dimension(), SpatialDimension::Three);
    assert_eq!(
        volume.axes(),
        &[SpatialAxis::Z, SpatialAxis::Y, SpatialAxis::X]
    );

    for axes in [
        vec![SpatialAxis::X],
        vec![SpatialAxis::X, SpatialAxis::X],
        vec![SpatialAxis::X, SpatialAxis::Z],
        vec![SpatialAxis::X, SpatialAxis::Y, SpatialAxis::Y],
    ] {
        let invalid_id = frame(&format!("invalid-axes-{axes:?}"));
        assert!(matches!(
            CoordinateFrame::new(
                invalid_id.clone(),
                axes,
                CoordinateUnit::Micrometer,
                CoordinateSpace::Physical,
            ),
            Err(CoordinateError::InvalidAxisSet { frame }) if frame == invalid_id
        ));
    }

    let image_in_micrometers = frame("image-in-micrometers");
    assert!(matches!(
        CoordinateFrame::new(
            image_in_micrometers.clone(),
            vec![SpatialAxis::X, SpatialAxis::Y],
            CoordinateUnit::Micrometer,
            CoordinateSpace::Image(ImageCoordinateConvention::PixelCornerAtInteger),
        ),
        Err(CoordinateError::UnitSpaceMismatch { frame, .. })
            if frame == image_in_micrometers
    ));

    let physical_in_pixels = frame("physical-in-pixels");
    assert!(matches!(
        CoordinateFrame::new(
            physical_in_pixels.clone(),
            vec![SpatialAxis::X, SpatialAxis::Y],
            CoordinateUnit::Pixel,
            CoordinateSpace::Physical,
        ),
        Err(CoordinateError::UnitSpaceMismatch { frame, .. })
            if frame == physical_in_pixels
    ));
}

#[test]
fn transform_and_uncertainty_definitions_reject_nonfinite_values() {
    assert!(matches!(
        TransformMatrix::affine_2d([1.0, 0.0, 0.0, 0.0, f64::NAN, 0.0]),
        Err(CoordinateError::NonFiniteTransformCoefficient { index: 4 })
    ));
    assert!(matches!(
        TransformMatrix::affine_3d([
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            f64::INFINITY,
            0.0,
        ]),
        Err(CoordinateError::NonFiniteTransformCoefficient { index: 10 })
    ));

    let uncertainty_id = uncertainty("registration-bound");
    assert!(matches!(
        UncertaintyReference::new(
            uncertainty_id.clone(),
            frame("physical"),
            Some(f64::NAN),
        ),
        Err(CoordinateError::InvalidUncertaintyRadius { uncertainty })
            if uncertainty == uncertainty_id
    ));
    assert!(matches!(
        UncertaintyReference::new(
            uncertainty_id.clone(),
            frame("physical"),
            Some(-0.1),
        ),
        Err(CoordinateError::InvalidUncertaintyRadius { uncertainty })
            if uncertainty == uncertainty_id
    ));

    let absent = UncertaintyReference::new(uncertainty("artifact-only"), frame("physical"), None)
        .expect("uncertainty without scalar bound");
    let zero = UncertaintyReference::new(uncertainty("known-zero"), frame("physical"), Some(0.0))
        .expect("known zero bound");
    assert_eq!(absent.conservative_radius(), None);
    assert_eq!(zero.conservative_radius(), Some(0.0));
}

#[test]
fn registry_rejects_duplicate_missing_incompatible_and_cyclic_transforms() {
    let a = physical_frame("a", vec![SpatialAxis::X, SpatialAxis::Y]);
    let b = physical_frame("b", vec![SpatialAxis::X, SpatialAxis::Y]);
    let c = physical_frame("c", vec![SpatialAxis::X, SpatialAxis::Y]);
    let volume = physical_frame(
        "volume",
        vec![SpatialAxis::X, SpatialAxis::Y, SpatialAxis::Z],
    );
    let identity = TransformMatrix::identity_2d();

    assert!(matches!(
        CoordinateRegistry::new(
            vec![a.clone(), a.clone()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        Err(CoordinateError::DuplicateFrame { frame }) if frame == *a.id()
    ));

    let missing_frame = frame("missing");
    let missing_source = transform("missing-source");
    assert!(matches!(
        CoordinateRegistry::new(
            vec![a.clone()],
            Vec::new(),
            vec![FrameTransform::new(
                missing_source.clone(),
                missing_frame.clone(),
                a.id().clone(),
                identity.clone(),
                None,
            )],
            Vec::new(),
        ),
        Err(CoordinateError::MissingTransformSourceFrame {
            transform,
            frame,
        }) if transform == missing_source && frame == missing_frame
    ));

    let self_transform = transform("self");
    assert!(matches!(
        CoordinateRegistry::new(
            vec![a.clone()],
            Vec::new(),
            vec![FrameTransform::new(
                self_transform.clone(),
                a.id().clone(),
                a.id().clone(),
                identity.clone(),
                None,
            )],
            Vec::new(),
        ),
        Err(CoordinateError::SelfTransform { transform, frame })
            if transform == self_transform && frame == *a.id()
    ));

    let wrong_dimension = transform("wrong-dimension");
    assert!(matches!(
        CoordinateRegistry::new(
            vec![a.clone(), volume.clone()],
            Vec::new(),
            vec![FrameTransform::new(
                wrong_dimension.clone(),
                a.id().clone(),
                volume.id().clone(),
                identity.clone(),
                None,
            )],
            Vec::new(),
        ),
        Err(CoordinateError::TransformDimensionMismatch { transform })
            if transform == wrong_dimension
    ));

    let uncertainty_id = uncertainty("wrong-frame-bound");
    let transform_with_bound = transform("transform-with-bound");
    let wrong_frame_bound =
        UncertaintyReference::new(uncertainty_id.clone(), c.id().clone(), Some(1.0))
            .expect("bound");
    assert!(matches!(
        CoordinateRegistry::new(
            vec![a.clone(), b.clone(), c.clone()],
            vec![wrong_frame_bound],
            vec![FrameTransform::new(
                transform_with_bound.clone(),
                a.id().clone(),
                b.id().clone(),
                identity.clone(),
                Some(uncertainty_id.clone()),
            )],
            Vec::new(),
        ),
        Err(CoordinateError::TransformUncertaintyFrameMismatch {
            transform,
            uncertainty,
            expected,
            observed,
        }) if transform == transform_with_bound
            && uncertainty == uncertainty_id
            && expected == *b.id()
            && observed == *c.id()
    ));

    let ab = transform("a-to-b");
    let bc = transform("b-to-c");
    let cb = transform("c-to-b-back-edge");
    assert!(matches!(
        CoordinateRegistry::new(
            vec![a.clone(), b.clone(), c.clone()],
            Vec::new(),
            vec![
                FrameTransform::new(
                    ab,
                    a.id().clone(),
                    b.id().clone(),
                    identity.clone(),
                    None,
                ),
                FrameTransform::new(
                    bc,
                    b.id().clone(),
                    c.id().clone(),
                    identity.clone(),
                    None,
                ),
                FrameTransform::new(
                    cb.clone(),
                    c.id().clone(),
                    b.id().clone(),
                    identity,
                    None,
                ),
            ],
            Vec::new(),
        ),
        Err(CoordinateError::TransformCycle {
            transform,
            source_frame,
            target_frame,
        }) if transform == cb && source_frame == *c.id() && target_frame == *b.id()
    ));
}

#[test]
fn registry_validates_uncertainty_and_transform_declarations_in_input_order() {
    let a = physical_frame("declaration-a", vec![SpatialAxis::X, SpatialAxis::Y]);
    let b = physical_frame("declaration-b", vec![SpatialAxis::X, SpatialAxis::Y]);

    let duplicate_uncertainty = uncertainty("duplicate-uncertainty");
    let first_reference =
        UncertaintyReference::new(duplicate_uncertainty.clone(), b.id().clone(), Some(1.0))
            .expect("first uncertainty");
    let second_reference =
        UncertaintyReference::new(duplicate_uncertainty.clone(), b.id().clone(), Some(2.0))
            .expect("second uncertainty");
    assert!(matches!(
        CoordinateRegistry::new(
            vec![a.clone(), b.clone()],
            vec![first_reference, second_reference],
            Vec::new(),
            Vec::new(),
        ),
        Err(CoordinateError::DuplicateUncertainty { uncertainty })
            if uncertainty == duplicate_uncertainty
    ));

    let duplicate_before_reference = uncertainty("duplicate-before-reference");
    let first_invalid_reference = UncertaintyReference::new(
        duplicate_before_reference.clone(),
        frame("missing-before-duplicate"),
        None,
    )
    .expect("locally valid first uncertainty");
    let second_duplicate_reference =
        UncertaintyReference::new(duplicate_before_reference.clone(), a.id().clone(), None)
            .expect("locally valid duplicate uncertainty");
    assert!(matches!(
        CoordinateRegistry::new(
            vec![a.clone(), b.clone()],
            vec![first_invalid_reference, second_duplicate_reference],
            Vec::new(),
            Vec::new(),
        ),
        Err(CoordinateError::DuplicateUncertainty { uncertainty })
            if uncertainty == duplicate_before_reference
    ));

    let missing_uncertainty_frame = frame("missing-uncertainty-frame");
    let unbound = uncertainty("unbound-uncertainty");
    assert!(matches!(
        CoordinateRegistry::new(
            vec![a.clone(), b.clone()],
            vec![UncertaintyReference::new(
                unbound.clone(),
                missing_uncertainty_frame.clone(),
                None,
            )
            .expect("locally valid uncertainty")],
            Vec::new(),
            Vec::new(),
        ),
        Err(CoordinateError::MissingUncertaintyFrame { uncertainty, frame })
            if uncertainty == unbound && frame == missing_uncertainty_frame
    ));

    let duplicate_transform_before_reference = transform("duplicate-before-missing-target");
    assert!(matches!(
        CoordinateRegistry::new(
            vec![a.clone(), b.clone()],
            Vec::new(),
            vec![
                FrameTransform::new(
                    duplicate_transform_before_reference.clone(),
                    a.id().clone(),
                    frame("missing-target-before-duplicate"),
                    TransformMatrix::identity_2d(),
                    None,
                ),
                FrameTransform::new(
                    duplicate_transform_before_reference.clone(),
                    a.id().clone(),
                    b.id().clone(),
                    TransformMatrix::identity_2d(),
                    None,
                ),
            ],
            Vec::new(),
        ),
        Err(CoordinateError::DuplicateTransform { transform })
            if transform == duplicate_transform_before_reference
    ));

    let cross_phase_missing_frame = uncertainty("cross-phase-missing-frame");
    let cross_phase_duplicate_transform = transform("cross-phase-duplicate-transform");
    assert!(matches!(
        CoordinateRegistry::new(
            vec![a.clone(), b.clone()],
            vec![UncertaintyReference::new(
                cross_phase_missing_frame,
                frame("cross-phase-absent-frame"),
                None,
            )
            .expect("locally valid uncertainty")],
            vec![
                FrameTransform::new(
                    cross_phase_duplicate_transform.clone(),
                    a.id().clone(),
                    b.id().clone(),
                    TransformMatrix::identity_2d(),
                    None,
                ),
                FrameTransform::new(
                    cross_phase_duplicate_transform.clone(),
                    a.id().clone(),
                    b.id().clone(),
                    TransformMatrix::identity_2d(),
                    None,
                ),
            ],
            Vec::new(),
        ),
        Err(CoordinateError::DuplicateTransform { transform })
            if transform == cross_phase_duplicate_transform
    ));

    let duplicate_transform = transform("duplicate-transform");
    assert!(matches!(
        CoordinateRegistry::new(
            vec![a.clone(), b.clone()],
            Vec::new(),
            vec![
                FrameTransform::new(
                    duplicate_transform.clone(),
                    a.id().clone(),
                    b.id().clone(),
                    TransformMatrix::identity_2d(),
                    None,
                ),
                FrameTransform::new(
                    duplicate_transform.clone(),
                    a.id().clone(),
                    b.id().clone(),
                    TransformMatrix::identity_2d(),
                    None,
                ),
            ],
            Vec::new(),
        ),
        Err(CoordinateError::DuplicateTransform { transform })
            if transform == duplicate_transform
    ));

    let valid_bound = uncertainty("valid-target-bound");
    let valid_bound_reference =
        UncertaintyReference::new(valid_bound.clone(), b.id().clone(), Some(0.5))
            .expect("valid target-framed uncertainty");
    let valid_bounded_transform = transform("valid-bounded-transform");
    let registry = CoordinateRegistry::new(
        vec![a.clone(), b.clone()],
        vec![valid_bound_reference],
        vec![FrameTransform::new(
            valid_bounded_transform.clone(),
            a.id().clone(),
            b.id().clone(),
            TransformMatrix::identity_2d(),
            Some(valid_bound.clone()),
        )],
        Vec::new(),
    )
    .expect("target-framed transform uncertainty");
    assert_eq!(
        registry
            .transform(&valid_bounded_transform)
            .expect("bounded transform")
            .uncertainty(),
        Some(&valid_bound)
    );

    let missing_target = frame("missing-transform-target");
    let missing_target_transform = transform("missing-target-transform");
    assert!(matches!(
        CoordinateRegistry::new(
            vec![a.clone()],
            Vec::new(),
            vec![FrameTransform::new(
                missing_target_transform.clone(),
                a.id().clone(),
                missing_target.clone(),
                TransformMatrix::identity_2d(),
                None,
            )],
            Vec::new(),
        ),
        Err(CoordinateError::MissingTransformTargetFrame { transform, frame })
            if transform == missing_target_transform && frame == missing_target
    ));

    let absent_bound = uncertainty("missing-transform-bound");
    let unbounded_transform = transform("transform-with-missing-bound");
    assert!(matches!(
        CoordinateRegistry::new(
            vec![a.clone(), b.clone()],
            Vec::new(),
            vec![FrameTransform::new(
                unbounded_transform.clone(),
                a.id().clone(),
                b.id().clone(),
                TransformMatrix::identity_2d(),
                Some(absent_bound.clone()),
            )],
            Vec::new(),
        ),
        Err(CoordinateError::MissingTransformUncertainty {
            transform,
            uncertainty,
        }) if transform == unbounded_transform && uncertainty == absent_bound
    ));

    let wrong_matrix_dimension = transform("wrong-matrix-dimension");
    assert!(matches!(
        CoordinateRegistry::new(
            vec![a.clone(), b],
            Vec::new(),
            vec![FrameTransform::new(
                wrong_matrix_dimension.clone(),
                a.id().clone(),
                frame("declaration-b"),
                TransformMatrix::identity_3d(),
                None,
            )],
            Vec::new(),
        ),
        Err(CoordinateError::TransformDimensionMismatch { transform })
            if transform == wrong_matrix_dimension
    ));

    let absent_frame = frame("absent-coordinate-frame");
    let registry = CoordinateRegistry::new(vec![a], Vec::new(), Vec::new(), Vec::new())
        .expect("minimal registry");
    assert!(matches!(
        registry.coordinate_2d(&absent_frame, [0.0, 0.0]),
        Err(CoordinateError::MissingFrame { frame }) if frame == absent_frame
    ));
}

#[test]
fn explicit_transform_chains_honor_source_and_target_axis_order() {
    let pixel = CoordinateFrame::new(
        frame("pixel-yx"),
        vec![SpatialAxis::Y, SpatialAxis::X],
        CoordinateUnit::Pixel,
        CoordinateSpace::Image(ImageCoordinateConvention::PixelCenterAtInteger),
    )
    .expect("pixel frame");
    let physical = physical_frame("physical-xy", vec![SpatialAxis::X, SpatialAxis::Y]);
    let analysis = physical_frame("analysis-xy", vec![SpatialAxis::X, SpatialAxis::Y]);
    let volume = physical_frame(
        "source-zyx",
        vec![SpatialAxis::Z, SpatialAxis::Y, SpatialAxis::X],
    );
    let target_volume = physical_frame(
        "target-xyz",
        vec![SpatialAxis::X, SpatialAxis::Y, SpatialAxis::Z],
    );
    let calibration_id = transform("pixel-to-physical");
    let analysis_id = transform("physical-to-analysis");
    let volume_id = transform("volume-axis-map");
    let calibration = FrameTransform::new(
        calibration_id.clone(),
        pixel.id().clone(),
        physical.id().clone(),
        TransformMatrix::affine_2d([0.0, 2.0, 10.0, 3.0, 0.0, -5.0]).expect("2-D affine"),
        None,
    );
    let to_analysis = FrameTransform::new(
        analysis_id.clone(),
        physical.id().clone(),
        analysis.id().clone(),
        TransformMatrix::identity_2d(),
        None,
    );
    let volume_map = FrameTransform::new(
        volume_id.clone(),
        volume.id().clone(),
        target_volume.id().clone(),
        TransformMatrix::affine_3d([0.0, 0.0, 2.0, 1.0, 0.0, 3.0, 0.0, 2.0, 4.0, 0.0, 0.0, 3.0])
            .expect("3-D affine"),
        None,
    );
    let registry = CoordinateRegistry::new(
        vec![
            pixel.clone(),
            physical.clone(),
            analysis.clone(),
            volume.clone(),
            target_volume.clone(),
        ],
        Vec::new(),
        vec![calibration, to_analysis, volume_map],
        Vec::new(),
    )
    .expect("coordinate registry");

    let pixel_point = registry
        .coordinate_2d(pixel.id(), [4.0, 7.0])
        .expect("pixel point in [y, x] order");
    let unchanged = registry
        .apply_chain_2d(&pixel_point, &[])
        .expect("empty chain");
    assert_eq!(unchanged, pixel_point);
    assert!(matches!(
        registry.require_physical(pixel.id()),
        Err(CoordinateError::SpaceMismatch { frame }) if frame == *pixel.id()
    ));

    let mapped = registry
        .apply_chain_2d(&pixel_point, std::slice::from_ref(&calibration_id))
        .expect("explicit calibration");
    assert_eq!(mapped.frame(), physical.id());
    assert_eq!(mapped.values(), &[24.0, 7.0]);

    let mapped_twice = registry
        .apply_chain_2d(&pixel_point, &[calibration_id.clone(), analysis_id.clone()])
        .expect("ordered chain");
    assert_eq!(mapped_twice.frame(), analysis.id());
    assert_eq!(mapped_twice.values(), &[24.0, 7.0]);

    assert!(matches!(
        registry.apply_chain_2d(&pixel_point, std::slice::from_ref(&analysis_id)),
        Err(CoordinateError::ChainSourceMismatch { transform, .. })
            if transform == analysis_id
    ));
    let missing_transform = transform("absent-transform");
    assert!(matches!(
        registry.apply_chain_2d(&pixel_point, std::slice::from_ref(&missing_transform)),
        Err(CoordinateError::MissingTransform { transform })
            if transform == missing_transform
    ));

    assert!(matches!(
        registry.require_dimension(volume.id(), SpatialDimension::Two),
        Err(CoordinateError::DimensionMismatch {
            frame,
            expected: SpatialDimension::Two,
            observed: SpatialDimension::Three,
        }) if frame == *volume.id()
    ));
    assert!(matches!(
        registry.coordinate_2d(pixel.id(), [f64::NAN, 0.0]),
        Err(CoordinateError::NonFiniteCoordinate { frame, index: 0 })
            if frame == *pixel.id()
    ));

    let volume_point = registry
        .coordinate_3d(volume.id(), [1.0, 2.0, 3.0])
        .expect("volume point in [z, y, x] order");
    let mapped_volume = registry
        .apply_chain_3d(&volume_point, std::slice::from_ref(&volume_id))
        .expect("3-D axis map");
    assert_eq!(mapped_volume.frame(), target_volume.id());
    assert_eq!(mapped_volume.values(), &[7.0, 8.0, 7.0]);
}

#[test]
fn deep_transform_cycle_detection_is_iterative() {
    const DEPTH: usize = 10_000;

    let frames = (0..DEPTH)
        .map(|index| {
            physical_frame(
                &format!("deep-frame-{index}"),
                vec![SpatialAxis::X, SpatialAxis::Y],
            )
        })
        .collect::<Vec<_>>();
    let mut transforms = Vec::with_capacity(DEPTH);
    for index in 0..DEPTH - 1 {
        transforms.push(FrameTransform::new(
            transform(&format!("deep-transform-{index}")),
            frames[index].id().clone(),
            frames[index + 1].id().clone(),
            TransformMatrix::identity_2d(),
            None,
        ));
    }
    let back_edge = transform("deep-back-edge");
    transforms.push(FrameTransform::new(
        back_edge.clone(),
        frames[DEPTH - 1].id().clone(),
        frames[DEPTH - 100].id().clone(),
        TransformMatrix::identity_2d(),
        None,
    ));

    assert!(matches!(
        CoordinateRegistry::new(frames.clone(), Vec::new(), transforms, Vec::new()),
        Err(CoordinateError::TransformCycle {
            transform,
            source_frame,
            target_frame,
        }) if transform == back_edge
            && source_frame == *frames[DEPTH - 1].id()
            && target_frame == *frames[DEPTH - 100].id()
    ));
}

#[test]
fn serial_section_definitions_enforce_local_state_and_ordering() {
    let source_frame = frame("section-source");
    assert!(matches!(
        SerialSectionPlacement::new(
            source_frame.clone(),
            [1.0, 0.0, 0.0, f64::INFINITY, 1.0, 0.0],
            None,
        ),
        Err(CoordinateError::NonFiniteSerialSectionPlacementCoefficient { index: 3 })
    ));

    let placement = SerialSectionPlacement::new(source_frame, [1.0, 0.0, 0.0, 0.0, 1.0, 0.0], None)
        .expect("finite placement");
    let invalid_z = section_id("invalid-z");
    assert!(matches!(
        SerialSection::new(
            invalid_z.clone(),
            1,
            f64::NAN,
            4.0,
            SerialSectionStatus::Observed,
            Some(placement.clone()),
        ),
        Err(CoordinateError::InvalidSerialSectionZ { section }) if section == invalid_z
    ));
    let invalid_thickness = section_id("invalid-thickness");
    assert!(matches!(
        SerialSection::new(
            invalid_thickness.clone(),
            1,
            0.0,
            0.0,
            SerialSectionStatus::Observed,
            Some(placement.clone()),
        ),
        Err(CoordinateError::InvalidSerialSectionThickness { section })
            if section == invalid_thickness
    ));

    for (section_id, status, invalid_placement) in [
        (
            section_id("observed-without-placement"),
            SerialSectionStatus::Observed,
            None,
        ),
        (
            section_id("missing-with-placement"),
            SerialSectionStatus::Missing,
            Some(placement.clone()),
        ),
        (
            section_id("distorted-without-uncertainty"),
            SerialSectionStatus::Distorted,
            Some(placement.clone()),
        ),
    ] {
        assert!(matches!(
            SerialSection::new(
                section_id.clone(),
                1,
                0.0,
                4.0,
                status,
                invalid_placement,
            ),
            Err(CoordinateError::SerialSectionStateMismatch { section, status: observed })
                if section == section_id && observed == status
        ));
    }

    let volume = frame("series-volume");
    assert!(matches!(
        SerialSectionSeries::new(volume.clone(), Vec::new()),
        Err(CoordinateError::EmptySerialSectionSeries { volume: observed })
            if observed == volume
    ));

    let repeated = section_id("repeated-section");
    assert!(matches!(
        SerialSectionSeries::new(
            volume.clone(),
            vec![
                missing_section(repeated.as_str(), 1, 0.0),
                missing_section(repeated.as_str(), 2, 1.0),
            ],
        ),
        Err(CoordinateError::DuplicateSerialSection { section }) if section == repeated
    ));

    let skipped = section_id("ordinal-three");
    assert!(matches!(
        SerialSectionSeries::new(
            volume.clone(),
            vec![
                missing_section("ordinal-one", 1, 0.0),
                missing_section(skipped.as_str(), 3, 1.0),
            ],
        ),
        Err(CoordinateError::NonConsecutiveSerialSectionOrdinal {
            section,
            expected: 2,
            observed: 3,
        }) if section == skipped
    ));

    let overflow_at = section_id("ordinal-max");
    assert!(matches!(
        SerialSectionSeries::new(
            volume.clone(),
            vec![
                missing_section(overflow_at.as_str(), u32::MAX, 0.0),
                missing_section("ordinal-after-max", 0, 1.0),
            ],
        ),
        Err(CoordinateError::SerialSectionOrdinalOverflow { section })
            if section == overflow_at
    ));

    let non_increasing = section_id("same-z");
    assert!(matches!(
        SerialSectionSeries::new(
            volume,
            vec![
                missing_section("lower-z", 8, 2.0),
                missing_section(non_increasing.as_str(), 9, 2.0),
            ],
        ),
        Err(CoordinateError::NonIncreasingSerialSectionZ { section, .. })
            if section == non_increasing
    ));
}

#[test]
fn registry_rejects_invalid_serial_section_cross_references() {
    let source = physical_frame("section-xy", vec![SpatialAxis::X, SpatialAxis::Y]);
    let source_3d = physical_frame(
        "section-xyz",
        vec![SpatialAxis::X, SpatialAxis::Y, SpatialAxis::Z],
    );
    let source_mm = CoordinateFrame::new(
        frame("section-mm"),
        vec![SpatialAxis::X, SpatialAxis::Y],
        CoordinateUnit::Millimeter,
        CoordinateSpace::Physical,
    )
    .expect("millimeter section frame");
    let pixel_source = CoordinateFrame::new(
        frame("section-pixel"),
        vec![SpatialAxis::X, SpatialAxis::Y],
        CoordinateUnit::Pixel,
        CoordinateSpace::Image(ImageCoordinateConvention::PixelCenterAtInteger),
    )
    .expect("pixel section frame");
    let volume = physical_frame(
        "volume-xyz",
        vec![SpatialAxis::X, SpatialAxis::Y, SpatialAxis::Z],
    );
    let other_volume = physical_frame(
        "other-volume-xyz",
        vec![SpatialAxis::X, SpatialAxis::Y, SpatialAxis::Z],
    );
    let image_volume = CoordinateFrame::new(
        frame("image-volume"),
        vec![SpatialAxis::X, SpatialAxis::Y, SpatialAxis::Z],
        CoordinateUnit::Pixel,
        CoordinateSpace::Image(ImageCoordinateConvention::PixelCornerAtInteger),
    )
    .expect("3-D image frame");

    let missing_volume = frame("absent-volume");
    assert!(matches!(
        CoordinateRegistry::new(
            vec![source.clone()],
            Vec::new(),
            Vec::new(),
            vec![observed_series(
                missing_volume.clone(),
                "missing-volume-section",
                source.id().clone(),
                None,
            )],
        ),
        Err(CoordinateError::MissingSerialSectionVolumeFrame { volume })
            if volume == missing_volume
    ));

    assert!(matches!(
        CoordinateRegistry::new(
            vec![source.clone()],
            Vec::new(),
            Vec::new(),
            vec![observed_series(
                source.id().clone(),
                "two-dimensional-volume",
                source.id().clone(),
                None,
            )],
        ),
        Err(CoordinateError::SerialSectionVolumeDimensionMismatch {
            volume: observed,
            observed: SpatialDimension::Two,
        }) if observed == *source.id()
    ));

    assert!(matches!(
        CoordinateRegistry::new(
            vec![pixel_source.clone(), image_volume.clone()],
            Vec::new(),
            Vec::new(),
            vec![observed_series(
                image_volume.id().clone(),
                "nonphysical-volume",
                pixel_source.id().clone(),
                None,
            )],
        ),
        Err(CoordinateError::SerialSectionVolumeSpaceMismatch { volume: observed })
            if observed == *image_volume.id()
    ));

    let missing_source = frame("absent-section-source");
    assert!(matches!(
        CoordinateRegistry::new(
            vec![volume.clone()],
            Vec::new(),
            Vec::new(),
            vec![observed_series(
                volume.id().clone(),
                "missing-source",
                missing_source.clone(),
                None,
            )],
        ),
        Err(CoordinateError::MissingSerialSectionSourceFrame {
            section,
            frame: observed,
        }) if section == section_id("missing-source") && observed == missing_source
    ));

    assert!(matches!(
        CoordinateRegistry::new(
            vec![source_3d.clone(), volume.clone()],
            Vec::new(),
            Vec::new(),
            vec![observed_series(
                volume.id().clone(),
                "three-dimensional-source",
                source_3d.id().clone(),
                None,
            )],
        ),
        Err(CoordinateError::SerialSectionSourceDimensionMismatch {
            section,
            observed: SpatialDimension::Three,
            ..
        }) if section == section_id("three-dimensional-source")
    ));

    assert!(matches!(
        CoordinateRegistry::new(
            vec![pixel_source.clone(), volume.clone()],
            Vec::new(),
            Vec::new(),
            vec![observed_series(
                volume.id().clone(),
                "pixel-source",
                pixel_source.id().clone(),
                None,
            )],
        ),
        Err(CoordinateError::SerialSectionSourceSpaceMismatch { section, .. })
            if section == section_id("pixel-source")
    ));

    assert!(matches!(
        CoordinateRegistry::new(
            vec![source_mm.clone(), volume.clone()],
            Vec::new(),
            Vec::new(),
            vec![observed_series(
                volume.id().clone(),
                "unit-mismatch",
                source_mm.id().clone(),
                None,
            )],
        ),
        Err(CoordinateError::SerialSectionUnitMismatch { section, .. })
            if section == section_id("unit-mismatch")
    ));

    let absent_uncertainty = uncertainty("absent-placement-uncertainty");
    assert!(matches!(
        CoordinateRegistry::new(
            vec![source.clone(), volume.clone()],
            Vec::new(),
            Vec::new(),
            vec![observed_series(
                volume.id().clone(),
                "missing-placement-uncertainty",
                source.id().clone(),
                Some(absent_uncertainty.clone()),
            )],
        ),
        Err(CoordinateError::MissingSerialSectionUncertainty {
            section,
            uncertainty,
        }) if section == section_id("missing-placement-uncertainty")
            && uncertainty == absent_uncertainty
    ));

    let wrong_uncertainty = uncertainty("wrong-placement-uncertainty");
    let wrong_reference = UncertaintyReference::new(
        wrong_uncertainty.clone(),
        other_volume.id().clone(),
        Some(2.0),
    )
    .expect("wrong-frame uncertainty");
    assert!(matches!(
        CoordinateRegistry::new(
            vec![source.clone(), volume.clone(), other_volume.clone()],
            vec![wrong_reference],
            Vec::new(),
            vec![observed_series(
                volume.id().clone(),
                "wrong-placement-uncertainty",
                source.id().clone(),
                Some(wrong_uncertainty.clone()),
            )],
        ),
        Err(CoordinateError::SerialSectionUncertaintyFrameMismatch {
            section,
            uncertainty,
            expected,
            observed,
        }) if section == section_id("wrong-placement-uncertainty")
            && uncertainty == wrong_uncertainty
            && expected == *volume.id()
            && observed == *other_volume.id()
    ));

    let first_series = observed_series(
        volume.id().clone(),
        "first-volume-owner",
        source.id().clone(),
        None,
    );
    let second_series = observed_series(
        volume.id().clone(),
        "second-volume-owner",
        source.id().clone(),
        None,
    );
    assert!(matches!(
        CoordinateRegistry::new(
            vec![source.clone(), volume.clone()],
            Vec::new(),
            Vec::new(),
            vec![first_series, second_series],
        ),
        Err(CoordinateError::DuplicateSerialSectionSeries { volume: observed })
            if observed == *volume.id()
    ));

    let repeated_section = "cross-series-duplicate";
    let first_series = observed_series(
        volume.id().clone(),
        repeated_section,
        source.id().clone(),
        None,
    );
    let second_series = observed_series(
        other_volume.id().clone(),
        repeated_section,
        source.id().clone(),
        None,
    );
    assert!(matches!(
        CoordinateRegistry::new(
            vec![source, volume, other_volume],
            Vec::new(),
            Vec::new(),
            vec![first_series, second_series],
        ),
        Err(CoordinateError::DuplicateSerialSection { section: observed })
            if observed == section_id(repeated_section)
    ));
}

#[test]
fn serial_section_embedding_honors_source_and_volume_axis_order() {
    let source = physical_frame("section-yx", vec![SpatialAxis::Y, SpatialAxis::X]);
    let other_source = physical_frame("other-section-xy", vec![SpatialAxis::X, SpatialAxis::Y]);
    let volume = physical_frame(
        "serial-volume-zxy",
        vec![SpatialAxis::Z, SpatialAxis::X, SpatialAxis::Y],
    );
    let placement_uncertainty = uncertainty("distorted-placement-bound");
    let uncertainty_reference =
        UncertaintyReference::new(placement_uncertainty.clone(), volume.id().clone(), None)
            .expect("target-volume uncertainty without scalar radius");

    let observed_placement =
        SerialSectionPlacement::new(source.id().clone(), [1.0, 0.0, 0.0, 0.0, 1.0, 0.0], None)
            .expect("observed placement");
    let observed = SerialSection::new(
        section_id("observed-section"),
        7,
        5.0,
        4.0,
        SerialSectionStatus::Observed,
        Some(observed_placement),
    )
    .expect("observed section");
    let missing = missing_section("explicit-missing-section", 8, 10.0);
    let distorted_placement = SerialSectionPlacement::new(
        source.id().clone(),
        [0.0, 2.0, 10.0, 3.0, 0.0, -5.0],
        Some(placement_uncertainty.clone()),
    )
    .expect("distorted placement");
    let distorted = SerialSection::new(
        section_id("distorted-section"),
        9,
        12.5,
        4.5,
        SerialSectionStatus::Distorted,
        Some(distorted_placement),
    )
    .expect("distorted section");
    let series = SerialSectionSeries::new(volume.id().clone(), vec![observed, missing, distorted])
        .expect("valid serial-section series");
    let registry = CoordinateRegistry::new(
        vec![source.clone(), other_source.clone(), volume.clone()],
        vec![uncertainty_reference],
        Vec::new(),
        vec![series],
    )
    .expect("serial-section registry");

    let retained_series = registry
        .serial_section_series_for_volume(volume.id())
        .expect("series by volume");
    assert_eq!(registry.serial_section_series().len(), 1);
    assert_eq!(retained_series.sections().len(), 3);
    assert_eq!(
        registry
            .serial_section(&section_id("explicit-missing-section"))
            .expect("missing entry is retained")
            .status(),
        SerialSectionStatus::Missing
    );

    let coordinate = registry
        .coordinate_2d(source.id(), [4.0, 7.0])
        .expect("source coordinate in [y, x] order");
    let embedded = registry
        .embed_serial_section(&section_id("distorted-section"), &coordinate)
        .expect("explicit section placement");
    assert_eq!(embedded.frame(), volume.id());
    assert_eq!(embedded.values(), &[12.5, 24.0, 7.0]);

    assert!(matches!(
        registry.embed_serial_section(&section_id("explicit-missing-section"), &coordinate),
        Err(CoordinateError::SerialSectionNotPlaced { section })
            if section == section_id("explicit-missing-section")
    ));
    let absent_section = section_id("absent-section");
    assert!(matches!(
        registry.embed_serial_section(&absent_section, &coordinate),
        Err(CoordinateError::MissingSerialSection { section })
            if section == absent_section
    ));

    let wrong_coordinate = registry
        .coordinate_2d(other_source.id(), [4.0, 7.0])
        .expect("other source coordinate");
    assert!(matches!(
        registry.embed_serial_section(&section_id("distorted-section"), &wrong_coordinate),
        Err(CoordinateError::SerialSectionCoordinateFrameMismatch {
            section,
            expected,
            observed,
        }) if section == section_id("distorted-section")
            && expected == *source.id()
            && observed == *other_source.id()
    ));
}
