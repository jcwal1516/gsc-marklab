use super::support::*;

#[test]
fn patch_context_and_footprints_freeze_sampled_support() {
    let (hierarchy, slide) = hierarchy();
    let expected = expected_patches(&hierarchy, &slide);
    let (registry, image, physical, transform) = coordinate_registry();
    let context_required = match patch_context_with_budget(
        &hierarchy,
        &slide,
        &registry,
        image.clone(),
        physical.clone(),
        transform.clone(),
        0,
    ) {
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, .. }) => required,
        other => panic!("expected context retained-budget error, observed {other:?}"),
    };
    let context_oracle = size_of::<PatchEmbeddingContext>()
        + slide.as_str().len()
        + image.as_str().len()
        + physical.as_str().len()
        + transform.as_str().len();
    assert_eq!(context_required, context_oracle);
    assert!(patch_context_with_budget(
        &hierarchy,
        &slide,
        &registry,
        image.clone(),
        physical.clone(),
        transform.clone(),
        context_required - 1,
    )
    .is_err());
    let context = patch_context_with_budget(
        &hierarchy,
        &slide,
        &registry,
        image,
        physical,
        transform,
        context_required,
    )
    .expect("exact context retained budget");

    assert_eq!(context.source_image_px(), [1_024, 768]);
    assert_eq!(context.patch_px(), [224, 224]);
    assert_eq!(context.stride_px(), [192, 192]);
    assert_eq!(context.overlap_px(), [32, 32]);
    assert_eq!(
        context.effective_receptive_field(),
        EffectiveReceptiveField::FullInput
    );

    let encoded = context
        .to_canonical_json()
        .expect("canonical patch context");
    assert_eq!(
        encoded,
        concat!(
            "{\"format\":\"marklab.patch_embedding_context\",\"version\":1,",
            "\"owning_slide_id\":\"slide\",\"image_frame_id\":\"image-pixels\",",
            "\"image_axes\":[\"x\",\"y\"],\"image_space\":\"image\",",
            "\"image_unit\":\"pixel\",\"pixel_convention\":\"pixel_center_at_integer\",",
            "\"physical_frame_id\":\"slide-micrometers\",\"physical_axes\":[\"x\",\"y\"],",
            "\"physical_space\":\"physical\",\"physical_unit\":\"micrometer\",",
            "\"transform_id\":\"pixel-to-micrometer\",",
            "\"transform_source_frame_id\":\"image-pixels\",",
            "\"transform_target_frame_id\":\"slide-micrometers\",",
            "\"transform_uncertainty\":null,\"transform_matrix_bits\":[",
            "\"3fe0000000000000\",\"0000000000000000\",\"3ff4000000000000\",",
            "\"0000000000000000\",\"3fd0000000000000\",\"c000000000000000\"],",
            "\"mpp_x\":{\"numerator\":1,\"denominator\":2},",
            "\"mpp_y\":{\"numerator\":1,\"denominator\":4},",
            "\"source_image_px\":[1024,768],\"patch_px\":[224,224],",
            "\"stride_px\":[192,192],\"overlap_px\":[32,32],",
            "\"effective_receptive_field\":{\"kind\":\"full_input\"},",
            "\"boundary_policy\":{\"kind\":\"fully_contained_only\"}}\n"
        )
        .as_bytes()
    );
    assert_eq!(
        context.logical_digest().to_string(),
        "584dbd213f30bd78dbdb1c2fbbe0944ac54d6295fd250dbbbc818a7472d03c32"
    );
    assert!(encoded.ends_with(b"\n"));
    let decoded_required = match PatchEmbeddingContext::from_canonical_json(
        &encoded,
        &hierarchy,
        &registry,
        encoded.len(),
        0,
        RETAINED_BUDGET,
    ) {
        Err(MultiscaleEmbeddingError::DecodedByteBudgetExceeded { required, .. }) => required,
        other => panic!("expected context decoded-budget error, observed {other:?}"),
    };
    assert!(PatchEmbeddingContext::from_canonical_json(
        &encoded,
        &hierarchy,
        &registry,
        encoded.len(),
        decoded_required - 1,
        RETAINED_BUDGET,
    )
    .is_err());
    assert_eq!(
        PatchEmbeddingContext::from_canonical_json(
            &encoded,
            &hierarchy,
            &registry,
            encoded.len(),
            decoded_required,
            context_required,
        )
        .expect("exact context decoded and retained budgets"),
        context
    );
    assert!(PatchEmbeddingContext::from_canonical_json(
        &encoded,
        &hierarchy,
        &registry,
        encoded.len(),
        decoded_required,
        context_required - 1,
    )
    .is_err());

    let expected_artifact = artifact_id(b"expected-patches");
    let context_artifact = artifact_id(b"patch-context");
    let footprint_rows = || {
        vec![
            PatchFootprint::new(patch("patch-a"), [0, 0]),
            PatchFootprint::new(patch("patch-b"), [192, 0]),
        ]
    };
    let footprint_required = match PatchFootprintSet::new(
        &hierarchy,
        &expected,
        expected_artifact,
        &context,
        context_artifact,
        footprint_rows(),
        0,
    ) {
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, .. }) => required,
        other => panic!("expected footprint budget error, observed {other:?}"),
    };
    let footprint_oracle = size_of::<PatchFootprintSet>()
        + 2 * size_of::<PatchFootprint>()
        + "patch-a".len()
        + "patch-b".len();
    assert_eq!(footprint_required, footprint_oracle);
    assert!(PatchFootprintSet::new(
        &hierarchy,
        &expected,
        expected_artifact,
        &context,
        context_artifact,
        footprint_rows(),
        footprint_required - 1,
    )
    .is_err());
    let footprints = PatchFootprintSet::new(
        &hierarchy,
        &expected,
        expected_artifact,
        &context,
        context_artifact,
        footprint_rows(),
        footprint_required,
    )
    .expect("footprint set");
    assert_eq!(footprints.row_count(), 2);
    assert_eq!(
        footprints.logical_digest().to_string(),
        "4d2490e1b437de6ca4e5993dc28438060334333157cbdb4b9fed4e1b56c5643f"
    );
    assert_eq!(footprints.footprints()[1].origin_px(), [192, 0]);
    assert_ne!(footprints.logical_digest(), context.logical_digest());
    PatchFootprintSet::new(
        &hierarchy,
        &expected,
        expected_artifact,
        &context,
        context_artifact,
        vec![
            PatchFootprint::new(patch("patch-a"), [0, 0]),
            PatchFootprint::new(patch("patch-b"), [800, 544]),
        ],
        RETAINED_BUDGET,
    )
    .expect("left/top and exact right/bottom contacts");

    assert!(PatchFootprintSet::new(
        &hierarchy,
        &expected,
        expected_artifact,
        &context,
        context_artifact,
        vec![
            PatchFootprint::new(patch("patch-a"), [-1, 0]),
            PatchFootprint::new(patch("patch-b"), [192, 0]),
        ],
        RETAINED_BUDGET,
    )
    .is_err());
}

#[test]
fn centered_receptive_fields_and_padding_boundaries_are_exact() {
    let (hierarchy, slide) = hierarchy();
    let expected = expected_patches(&hierarchy, &slide);
    let (registry, image, physical, transform) = coordinate_registry();
    let centered = PatchEmbeddingContext::new(
        &hierarchy,
        &registry,
        slide.clone(),
        image.clone(),
        physical.clone(),
        transform.clone(),
        PositiveRational::new(1, 2).expect("x scale"),
        PositiveRational::new(1, 4).expect("y scale"),
        [1_024, 768],
        [224, 224],
        [192, 192],
        [32, 32],
        EffectiveReceptiveField::CenteredRectangle {
            width_px: PositiveRational::new(112, 1).expect("receptive width"),
            height_px: PositiveRational::new(56, 1).expect("receptive height"),
        },
        PatchBoundaryPolicy::ConstantRgb([1, 2, 3]),
        RETAINED_BUDGET,
    )
    .expect("centered constant-RGB context");
    let centered_wire = centered.to_canonical_json().expect("centered context wire");
    assert!(centered_wire.ends_with(
        concat!(
            "\"effective_receptive_field\":{\"kind\":\"centered_rectangle\",",
            "\"width_px\":{\"numerator\":112,\"denominator\":1},",
            "\"height_px\":{\"numerator\":56,\"denominator\":1}},",
            "\"boundary_policy\":{\"kind\":\"constant_rgb\",\"rgb\":[1,2,3]}}\n"
        )
        .as_bytes()
    ));
    assert_eq!(
        centered.logical_digest().to_string(),
        "e3e244a16c8a7d4d394a78bf750d541e7a67fa0b375ff37d2948e47a08a7b1d0"
    );

    let large_source = PatchEmbeddingContext::new(
        &hierarchy,
        &registry,
        slide.clone(),
        image.clone(),
        physical.clone(),
        transform.clone(),
        PositiveRational::new(1, 2).expect("x scale"),
        PositiveRational::new(1, 4).expect("y scale"),
        [u64::MAX, u64::MAX],
        [224, 224],
        [192, 192],
        [32, 32],
        EffectiveReceptiveField::FullInput,
        PatchBoundaryPolicy::FullyContainedOnly,
        RETAINED_BUDGET,
    )
    .expect("u64 source extent context");
    assert_eq!(large_source.source_image_px(), [u64::MAX, u64::MAX]);
    PatchFootprintSet::new(
        &hierarchy,
        &expected,
        artifact_id(b"large-source-expected"),
        &large_source,
        artifact_id(b"large-source-context"),
        vec![
            PatchFootprint::new(patch("patch-a"), [i64::MAX - 224, 0]),
            PatchFootprint::new(patch("patch-b"), [0, i64::MAX - 224]),
        ],
        RETAINED_BUDGET,
    )
    .expect("footprints within a source extent above i64::MAX");

    assert!(PatchEmbeddingContext::new(
        &hierarchy,
        &registry,
        slide.clone(),
        image.clone(),
        physical.clone(),
        transform.clone(),
        PositiveRational::new(1, 2).expect("x scale"),
        PositiveRational::new(1, 4).expect("y scale"),
        [1_024, 768],
        [224, 224],
        [191, 192],
        [32, 32],
        EffectiveReceptiveField::FullInput,
        PatchBoundaryPolicy::Reflect,
        RETAINED_BUDGET,
    )
    .is_err());
    assert!(PatchEmbeddingContext::new(
        &hierarchy,
        &registry,
        slide.clone(),
        image.clone(),
        physical.clone(),
        transform,
        PositiveRational::new(1, 2).expect("x scale"),
        PositiveRational::new(1, 4).expect("y scale"),
        [1_024, 768],
        [224, 224],
        [192, 192],
        [32, 32],
        EffectiveReceptiveField::CenteredRectangle {
            width_px: PositiveRational::new(225, 1).expect("oversize width"),
            height_px: PositiveRational::new(56, 1).expect("height"),
        },
        PatchBoundaryPolicy::Reflect,
        RETAINED_BUDGET,
    )
    .is_err());

    let (negative_zero_registry, negative_image, negative_physical, negative_transform) =
        coordinate_registry_with_matrix([0.5, -0.0, 1.25, 0.0, 0.25, -2.0]);
    assert!(PatchEmbeddingContext::new(
        &hierarchy,
        &negative_zero_registry,
        slide.clone(),
        negative_image,
        negative_physical,
        negative_transform,
        PositiveRational::new(1, 2).expect("x scale"),
        PositiveRational::new(1, 4).expect("y scale"),
        [1_024, 768],
        [224, 224],
        [192, 192],
        [32, 32],
        EffectiveReceptiveField::FullInput,
        PatchBoundaryPolicy::Reflect,
        RETAINED_BUDGET,
    )
    .is_err());

    let reflect = PatchEmbeddingContext::new(
        &hierarchy,
        &registry,
        slide,
        image,
        physical,
        TransformId::new("pixel-to-micrometer").expect("transform ID"),
        PositiveRational::new(1, 2).expect("x scale"),
        PositiveRational::new(1, 4).expect("y scale"),
        [1_024, 768],
        [224, 224],
        [192, 192],
        [32, 32],
        EffectiveReceptiveField::FullInput,
        PatchBoundaryPolicy::Reflect,
        RETAINED_BUDGET,
    )
    .expect("reflect context");
    let expected_artifact = artifact_id(b"reflect-expected");
    let context_artifact = artifact_id(b"reflect-context");
    let reflected = PatchFootprintSet::new(
        &hierarchy,
        &expected,
        expected_artifact,
        &reflect,
        context_artifact,
        vec![
            PatchFootprint::new(patch("patch-a"), [-223, 0]),
            PatchFootprint::new(patch("patch-b"), [1_023, 0]),
        ],
        RETAINED_BUDGET,
    )
    .expect("positive-area reflected footprints");
    assert_eq!(reflected.row_count(), 2);
    assert!(PatchFootprintSet::new(
        &hierarchy,
        &expected,
        expected_artifact,
        &reflect,
        context_artifact,
        vec![
            PatchFootprint::new(patch("patch-a"), [-224, 0]),
            PatchFootprint::new(patch("patch-b"), [1_023, 0]),
        ],
        RETAINED_BUDGET,
    )
    .is_err());
    assert!(PatchFootprintSet::new(
        &hierarchy,
        &expected,
        expected_artifact,
        &reflect,
        context_artifact,
        vec![
            PatchFootprint::new(patch("patch-a"), [i64::MAX, 0]),
            PatchFootprint::new(patch("patch-b"), [1_023, 0]),
        ],
        RETAINED_BUDGET,
    )
    .is_err());
}
