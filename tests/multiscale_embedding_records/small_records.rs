use super::support::*;

fn frame(writer: &mut impl Write, value: &[u8]) {
    writer
        .write_all(&(value.len() as u128).to_be_bytes())
        .expect("reference length");
    writer.write_all(value).expect("reference value");
}

fn reference_derivation_digest(
    derivation: &MultiscaleEmbeddingDerivationContract,
) -> ContentDigest {
    let mut writer = ContentDigest::builder();
    frame(
        &mut writer,
        b"marklab-multiscale-embedding-derivation-logical-v1",
    );
    frame(&mut writer, b"marklab.multiscale_embedding_derivation");
    frame(&mut writer, &1_u32.to_be_bytes());
    frame(&mut writer, b"deterministic");
    frame(&mut writer, derivation.algorithm().as_bytes());
    frame(&mut writer, derivation.algorithm_version().as_bytes());
    frame(&mut writer, b"exclude_non_present_require_one");
    frame(&mut writer, b"f64");
    frame(&mut writer, b"f32");
    writer.finish().0
}

fn reference_support_digest(
    variant: &str,
    entity_kind: &str,
    slide: &SlideId,
    bindings: &[MultiscaleArtifactBinding],
) -> ContentDigest {
    let mut writer = ContentDigest::builder();
    frame(
        &mut writer,
        b"marklab-multiscale-embedding-support-logical-v1",
    );
    frame(&mut writer, b"marklab.multiscale_embedding_support");
    frame(&mut writer, &1_u32.to_be_bytes());
    frame(&mut writer, variant.as_bytes());
    frame(&mut writer, entity_kind.as_bytes());
    frame(&mut writer, slide.as_str().as_bytes());
    for value in bindings {
        frame(&mut writer, value.artifact_id().digest().as_bytes());
        frame(&mut writer, value.logical_digest().as_bytes());
    }
    writer.finish().0
}

#[test]
fn derivation_and_four_support_variants_are_exact_closed_logical_values() {
    let weighted = MultiscaleEmbeddingDerivationContract::weighted_mean("fractions.v1", BUDGET)
        .expect("weighted derivation");
    let arithmetic =
        MultiscaleEmbeddingDerivationContract::arithmetic_mean("stable_order.v1", BUDGET)
            .expect("arithmetic derivation");
    assert_eq!(weighted.algorithm(), "weighted_mean");
    assert_eq!(arithmetic.algorithm(), "arithmetic_mean");
    assert_eq!(
        weighted.logical_digest().to_string(),
        "2b8565ee1ae00615e7f877ad145a1143fcefe51eff341818b96f912a0e4da022"
    );
    assert_eq!(
        arithmetic.logical_digest().to_string(),
        "40bd3bfef7d12d241baf7a5c31ba9aefe97fbc9673c49e282d209db9a7ace59c"
    );
    assert!(!format!("{weighted:?}").contains("fractions.v1"));
    assert_eq!(
        weighted.logical_digest(),
        reference_derivation_digest(&weighted)
    );
    assert_eq!(
        arithmetic.logical_digest(),
        reference_derivation_digest(&arithmetic)
    );
    let arithmetic_bytes = arithmetic.to_canonical_json().expect("arithmetic JSON");
    assert_eq!(
        String::from_utf8(arithmetic_bytes.clone()).expect("UTF-8"),
        concat!(
            "{\"format\":\"marklab.multiscale_embedding_derivation\",\"version\":1,",
            "\"kind\":\"deterministic\",\"algorithm\":\"arithmetic_mean\",",
            "\"algorithm_version\":\"stable_order.v1\",",
            "\"missing_policy\":\"exclude_non_present_require_one\",",
            "\"accumulator_dtype\":\"f64\",\"output_dtype\":\"f32\"}\n"
        )
    );
    assert_eq!(
        MultiscaleEmbeddingDerivationContract::from_canonical_json(
            &arithmetic_bytes,
            arithmetic_bytes.len(),
            BUDGET,
            BUDGET,
        )
        .expect("arithmetic round trip"),
        arithmetic
    );
    assert_eq!(
        String::from_utf8(weighted.to_canonical_json().expect("weighted JSON")).expect("UTF-8"),
        concat!(
            "{\"format\":\"marklab.multiscale_embedding_derivation\",\"version\":1,",
            "\"kind\":\"deterministic\",\"algorithm\":\"weighted_mean\",",
            "\"algorithm_version\":\"fractions.v1\",",
            "\"missing_policy\":\"exclude_non_present_require_one\",",
            "\"accumulator_dtype\":\"f64\",\"output_dtype\":\"f32\"}\n"
        )
    );

    let slide = SlideId::new("records-support-slide").expect("slide");
    let context = binding(b"patch-context");
    let footprints = binding(b"patch-footprints");
    let overlap = binding(b"patch-overlap");
    let patch_support =
        MultiscaleEmbeddingSupport::patch(slide.clone(), context, footprints, overlap, BUDGET)
            .expect("patch support");
    assert_eq!(
        patch_support.variant(),
        MultiscaleEmbeddingSupportVariant::Patch
    );
    assert_eq!(patch_support.entity_kind(), EmbeddingEntityKind::Patch);
    assert!(!format!("{context:?}").contains(&context.artifact_id().to_string()));
    assert!(!format!("{patch_support:?}").contains("records-support-slide"));
    assert_eq!(
        patch_support.logical_digest(),
        reference_support_digest("patch", "patch", &slide, &[context, footprints, overlap])
    );
    assert_eq!(
        patch_support.direct_dependencies().collect::<Vec<_>>(),
        sorted_artifacts([
            context.artifact_id(),
            footprints.artifact_id(),
            overlap.artifact_id(),
        ])
    );
    let expected_patch_json = format!(
        concat!(
            "{{\"format\":\"marklab.multiscale_embedding_support\",\"version\":1,",
            "\"variant\":\"patch\",\"entity_kind\":\"patch\",",
            "\"owning_slide_id\":\"records-support-slide\",",
            "\"patch_context\":{{\"artifact_id\":\"{}\",\"logical_digest\":\"{}\"}},",
            "\"patch_footprints\":{{\"artifact_id\":\"{}\",\"logical_digest\":\"{}\"}},",
            "\"patch_overlap_graph\":{{\"artifact_id\":\"{}\",\"logical_digest\":\"{}\"}}}}\n"
        ),
        context.artifact_id(),
        context.logical_digest(),
        footprints.artifact_id(),
        footprints.logical_digest(),
        overlap.artifact_id(),
        overlap.logical_digest(),
    );
    let patch_bytes = patch_support
        .to_canonical_json()
        .expect("patch support JSON");
    assert_eq!(
        patch_support.logical_digest().to_string(),
        "54e656dc1122f1d25779964b8d81c934ec0b0ea918c47cb5a70d014f9035ff1a"
    );
    assert_eq!(
        String::from_utf8(patch_bytes.clone()).expect("UTF-8"),
        expected_patch_json
    );
    assert_eq!(
        MultiscaleEmbeddingSupport::from_canonical_json(
            &patch_bytes,
            patch_bytes.len(),
            BUDGET,
            BUDGET,
        )
        .expect("patch support round trip"),
        patch_support
    );

    let patch_support_binding = binding(b"patch-support");
    let patch_region_link = binding(b"patch-region-link");
    let region = MultiscaleEmbeddingSupport::region_from_patches(
        slide.clone(),
        patch_support_binding,
        patch_region_link,
        BUDGET,
    )
    .expect("region support");
    assert_eq!(
        region.logical_digest(),
        reference_support_digest(
            "region_from_patches",
            "region",
            &slide,
            &[patch_support_binding, patch_region_link],
        )
    );
    let expected_region_json = format!(
        concat!(
            "{{\"format\":\"marklab.multiscale_embedding_support\",\"version\":1,",
            "\"variant\":\"region_from_patches\",\"entity_kind\":\"region\",",
            "\"owning_slide_id\":\"records-support-slide\",",
            "\"patch_support\":{{\"artifact_id\":\"{}\",\"logical_digest\":\"{}\"}},",
            "\"patch_region_link\":{{\"artifact_id\":\"{}\",\"logical_digest\":\"{}\"}}}}\n"
        ),
        patch_support_binding.artifact_id(),
        patch_support_binding.logical_digest(),
        patch_region_link.artifact_id(),
        patch_region_link.logical_digest(),
    );
    assert_eq!(
        String::from_utf8(region.to_canonical_json().expect("region JSON")).expect("UTF-8"),
        expected_region_json
    );
    assert_eq!(
        region.logical_digest().to_string(),
        "e4ab39eddb379cbdbbffb21adf7ccf6e9f90da9e2fa071e651948b4a63ca74f1"
    );
    assert_eq!(
        region.direct_dependencies().collect::<Vec<_>>(),
        sorted_artifacts([
            patch_support_binding.artifact_id(),
            patch_region_link.artifact_id(),
        ])
    );

    let source_patch_table = binding(b"source-patch-table");
    let slide_from_patches = MultiscaleEmbeddingSupport::slide_from_patches(
        slide.clone(),
        patch_support_binding,
        source_patch_table,
        BUDGET,
    )
    .expect("slide patch support");
    assert_eq!(
        slide_from_patches.logical_digest(),
        reference_support_digest(
            "slide_from_patches",
            "slide",
            &slide,
            &[patch_support_binding, source_patch_table],
        )
    );
    let expected_slide_patch_json = format!(
        concat!(
            "{{\"format\":\"marklab.multiscale_embedding_support\",\"version\":1,",
            "\"variant\":\"slide_from_patches\",\"entity_kind\":\"slide\",",
            "\"owning_slide_id\":\"records-support-slide\",",
            "\"patch_support\":{{\"artifact_id\":\"{}\",\"logical_digest\":\"{}\"}},",
            "\"source_patch_table\":{{\"artifact_id\":\"{}\",\"logical_digest\":\"{}\"}}}}\n"
        ),
        patch_support_binding.artifact_id(),
        patch_support_binding.logical_digest(),
        source_patch_table.artifact_id(),
        source_patch_table.logical_digest(),
    );
    assert_eq!(
        String::from_utf8(
            slide_from_patches
                .to_canonical_json()
                .expect("slide-patch JSON")
        )
        .expect("UTF-8"),
        expected_slide_patch_json
    );
    assert_eq!(
        slide_from_patches.logical_digest().to_string(),
        "7c63abd92dabf0efa9f950bd9660fe796567e631b8eb67c2f4e752c53ce20051"
    );

    let region_support_binding = binding(b"region-support");
    let source_region_table = binding(b"source-region-table");
    let slide_from_regions = MultiscaleEmbeddingSupport::slide_from_regions(
        slide.clone(),
        region_support_binding,
        source_region_table,
        BUDGET,
    )
    .expect("slide region support");
    assert_eq!(
        slide_from_regions.logical_digest(),
        reference_support_digest(
            "slide_from_regions",
            "slide",
            &slide,
            &[region_support_binding, source_region_table],
        )
    );
    let expected_slide_region_json = format!(
        concat!(
            "{{\"format\":\"marklab.multiscale_embedding_support\",\"version\":1,",
            "\"variant\":\"slide_from_regions\",\"entity_kind\":\"slide\",",
            "\"owning_slide_id\":\"records-support-slide\",",
            "\"region_support\":{{\"artifact_id\":\"{}\",\"logical_digest\":\"{}\"}},",
            "\"source_region_table\":{{\"artifact_id\":\"{}\",\"logical_digest\":\"{}\"}}}}\n"
        ),
        region_support_binding.artifact_id(),
        region_support_binding.logical_digest(),
        source_region_table.artifact_id(),
        source_region_table.logical_digest(),
    );
    assert_eq!(
        String::from_utf8(
            slide_from_regions
                .to_canonical_json()
                .expect("slide-region JSON")
        )
        .expect("UTF-8"),
        expected_slide_region_json
    );
    assert_eq!(
        slide_from_regions.logical_digest().to_string(),
        "de73b950c2e74d2591cd131345d9e6fdcb984f79e426464e60e3519ca7347b82"
    );

    for (support, variant, entity, expected_dependencies) in [
        (
            region,
            MultiscaleEmbeddingSupportVariant::RegionFromPatches,
            EmbeddingEntityKind::Region,
            sorted_artifacts([
                patch_support_binding.artifact_id(),
                patch_region_link.artifact_id(),
            ]),
        ),
        (
            slide_from_patches,
            MultiscaleEmbeddingSupportVariant::SlideFromPatches,
            EmbeddingEntityKind::Slide,
            sorted_artifacts([
                patch_support_binding.artifact_id(),
                source_patch_table.artifact_id(),
            ]),
        ),
        (
            slide_from_regions,
            MultiscaleEmbeddingSupportVariant::SlideFromRegions,
            EmbeddingEntityKind::Slide,
            sorted_artifacts([
                region_support_binding.artifact_id(),
                source_region_table.artifact_id(),
            ]),
        ),
    ] {
        assert_eq!(support.variant(), variant);
        assert_eq!(support.entity_kind(), entity);
        assert_eq!(
            support.direct_dependencies().collect::<Vec<_>>(),
            expected_dependencies
        );
        let bytes = support.to_canonical_json().expect("support JSON");
        assert_eq!(
            MultiscaleEmbeddingSupport::from_canonical_json(&bytes, bytes.len(), BUDGET, BUDGET,)
                .expect("support round trip"),
            support
        );
    }
}
