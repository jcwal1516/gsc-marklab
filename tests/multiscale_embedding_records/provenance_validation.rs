use super::{provenance_records::*, support::*};

#[test]
fn provenance_rejects_variant_algorithm_support_token_dimension_and_alias_drift() {
    let slide = SlideId::new("provenance-slide").expect("slide");
    let patch_support = provenance_patch_support(&slide);
    let weighted = MultiscaleEmbeddingDerivationContract::weighted_mean("fractions.v1", BUDGET)
        .expect("weighted");
    let arithmetic = MultiscaleEmbeddingDerivationContract::arithmetic_mean("stable.v1", BUDGET)
        .expect("arithmetic");
    for dimension in [0, 65_537] {
        assert!(MultiscaleEmbeddingProvenance::direct_patch(
            slide.clone(),
            &patch_support,
            dimension,
            "mean_patch_tokens",
            provenance_model(),
            provenance_execution(),
            artifact(b"preprocessing"),
            direct_inputs(),
            BUDGET,
        )
        .is_err());
    }
    assert!(MultiscaleEmbeddingProvenance::direct_patch(
        slide.clone(),
        &patch_support,
        1_024,
        "bad pooling",
        provenance_model(),
        provenance_execution(),
        artifact(b"preprocessing"),
        direct_inputs(),
        BUDGET,
    )
    .is_err());
    let wrong_support = MultiscaleEmbeddingSupport::slide_from_patches(
        slide.clone(),
        binding(b"patch-support-binding"),
        binding(b"source-patch-table-binding"),
        BUDGET,
    )
    .expect("wrong support");
    assert!(MultiscaleEmbeddingProvenance::direct_patch(
        slide.clone(),
        &wrong_support,
        1_024,
        "mean_patch_tokens",
        provenance_model(),
        provenance_execution(),
        artifact(b"preprocessing"),
        direct_inputs(),
        BUDGET,
    )
    .is_err());
    let region_support = MultiscaleEmbeddingSupport::region_from_patches(
        slide.clone(),
        binding(b"patch-support-binding"),
        binding(b"patch-region-link-binding"),
        BUDGET,
    )
    .expect("region support");
    assert!(MultiscaleEmbeddingProvenance::derived_region(
        slide.clone(),
        &region_support,
        &arithmetic,
        1_024,
        provenance_execution(),
        artifact(b"source-patch-table"),
        artifact(b"patch-region-link"),
        artifact(b"expected-regions"),
        artifact(b"region-support"),
        artifact(b"derivation"),
        BUDGET,
    )
    .is_err());
    let slide_support = MultiscaleEmbeddingSupport::slide_from_patches(
        slide.clone(),
        binding(b"patch-support-binding"),
        binding(b"source-patch-table-binding"),
        BUDGET,
    )
    .expect("slide support");
    assert!(MultiscaleEmbeddingProvenance::derived_slide_from_patches(
        slide,
        &slide_support,
        &weighted,
        1_024,
        provenance_execution(),
        artifact(b"source-patch-table"),
        artifact(b"expected-slides"),
        artifact(b"slide-support"),
        artifact(b"derivation"),
        BUDGET,
    )
    .is_err());

    let direct = direct_patch_provenance();
    let bytes = direct.to_canonical_json().expect("direct JSON");
    let aliased = String::from_utf8(bytes.clone()).expect("UTF-8").replace(
        &artifact(b"source-snapshot").to_string(),
        &artifact(b"checkpoint").to_string(),
    );
    assert!(MultiscaleEmbeddingProvenance::from_canonical_json(
        aliased.as_bytes(),
        BUDGET,
        BUDGET,
        BUDGET,
    )
    .is_err());
    let drifted = String::from_utf8(bytes)
        .expect("UTF-8")
        .replace("\"entity_kind\":\"patch\"", "\"entity_kind\":\"region\"");
    assert!(MultiscaleEmbeddingProvenance::from_canonical_json(
        drifted.as_bytes(),
        BUDGET,
        BUDGET,
        BUDGET,
    )
    .is_err());
}

#[test]
fn provenance_decoder_enforces_strict_fixed_point_and_exact_resource_edges() {
    let quoted_model = MultiscaleDirectPatchModelProvenance::new(
        "patch_encoder",
        "1.2.0",
        "vit_h",
        artifact(b"quoted-checkpoint"),
        ContentDigest::from_bytes(b"quoted-checkpoint-content"),
        artifact(b"quoted-source-snapshot"),
        artifact(b"quoted-license"),
        "Apache-2.0",
        "A canonical citation with \"quotes\" and a \\ path",
        "encoder.layer_32",
        32,
        BUDGET,
    )
    .expect("quoted model");
    let quoted_slide = SlideId::new("quoted-provenance-slide").expect("slide");
    let quoted = MultiscaleEmbeddingProvenance::direct_patch(
        quoted_slide.clone(),
        &provenance_patch_support(&quoted_slide),
        1_024,
        "mean_patch_tokens",
        quoted_model,
        provenance_execution(),
        artifact(b"quoted-preprocessing"),
        MultiscaleDirectPatchInputArtifacts::new(
            artifact(b"quoted-normalization"),
            artifact(b"quoted-source-entities"),
            artifact(b"quoted-source-vectors"),
            artifact(b"quoted-expected-patches"),
            artifact(b"quoted-identity-map"),
            artifact(b"quoted-source-row-link"),
            artifact(b"quoted-patch-support"),
        ),
        BUDGET,
    )
    .expect("quoted provenance");
    let quoted_bytes = quoted.to_canonical_json().expect("quoted JSON");
    assert_eq!(
        MultiscaleEmbeddingProvenance::from_canonical_json(
            &quoted_bytes,
            quoted_bytes.len(),
            BUDGET,
            BUDGET,
        )
        .expect("quoted citation round trip"),
        quoted
    );

    let provenance = direct_patch_provenance();
    let bytes = provenance.to_canonical_json().expect("provenance JSON");
    assert!(matches!(
        MultiscaleEmbeddingProvenance::from_canonical_json(
            &bytes,
            bytes.len() - 1,
            BUDGET,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded { observed, maximum })
            if observed == bytes.len() && maximum == bytes.len() - 1
    ));
    let decoded_required =
        match MultiscaleEmbeddingProvenance::from_canonical_json(&bytes, bytes.len(), 0, BUDGET) {
            Err(MultiscaleEmbeddingError::DecodedByteBudgetExceeded { required, .. }) => required,
            other => panic!("unexpected decoded probe: {other:?}"),
        };
    let retained_required = match MultiscaleEmbeddingProvenance::from_canonical_json(
        &bytes,
        bytes.len(),
        decoded_required,
        0,
    ) {
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, .. }) => required,
        other => panic!("unexpected retained probe: {other:?}"),
    };
    assert_eq!(
        MultiscaleEmbeddingProvenance::from_canonical_json(
            &bytes,
            bytes.len(),
            decoded_required,
            retained_required,
        )
        .expect("exact budgets"),
        provenance
    );
    assert!(MultiscaleEmbeddingProvenance::from_canonical_json(
        &bytes,
        bytes.len(),
        decoded_required - 1,
        retained_required,
    )
    .is_err());
    assert!(MultiscaleEmbeddingProvenance::from_canonical_json(
        &bytes,
        bytes.len(),
        decoded_required,
        retained_required - 1,
    )
    .is_err());

    let text = String::from_utf8(bytes).expect("UTF-8");
    for invalid in [
        text.trim_end_matches('\n').to_owned(),
        format!("{text}\n"),
        text.replace(
            "\"variant\":\"direct_patch\"",
            "\"variant\":\"direct_patch\",\"unexpected\":0",
        ),
        text.replace(
            "\"model_family\":\"patch_encoder\"",
            "\"model_family\":\"patch_encoder\",\"model_family\":\"patch_encoder\"",
        ),
    ] {
        assert!(MultiscaleEmbeddingProvenance::from_canonical_json(
            invalid.as_bytes(),
            BUDGET,
            BUDGET,
            BUDGET,
        )
        .is_err());
    }
    let hostile = vec![b' '; 256 * 1024 + 1];
    assert!(matches!(
        MultiscaleEmbeddingProvenance::from_canonical_json(
            &hostile,
            usize::MAX,
            BUDGET,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded { maximum, .. })
            if maximum == 256 * 1024
    ));
}
