use super::{provenance_records::*, support::*};

const VALID_MODEL_TEXT: [&str; 6] = [
    "patch_encoder",
    "1.2.0",
    "vit_h",
    "Apache-2.0",
    "doi:10.1000/marklab-patch-encoder",
    "encoder.layer_32",
];

fn model_with_text(
    text: [&str; 6],
    extraction_layer: u32,
) -> Result<MultiscaleDirectPatchModelProvenance, MultiscaleEmbeddingError> {
    MultiscaleDirectPatchModelProvenance::new(
        text[0],
        text[1],
        text[2],
        artifact(b"constraint-checkpoint"),
        ContentDigest::from_bytes(b"constraint-checkpoint-content"),
        artifact(b"constraint-snapshot"),
        artifact(b"constraint-license"),
        text[3],
        text[4],
        text[5],
        extraction_layer,
        BUDGET,
    )
}

fn assert_decode_rejected(bytes: &str) {
    assert!(MultiscaleEmbeddingProvenance::from_canonical_json(
        bytes.as_bytes(),
        BUDGET,
        BUDGET,
        BUDGET,
    )
    .is_err());
}

fn assert_all_role_aliases_rejected(
    provenance: &MultiscaleEmbeddingProvenance,
    role_labels: &[&[u8]],
) {
    let canonical = String::from_utf8(
        provenance
            .to_canonical_json()
            .expect("canonical provenance"),
    )
    .expect("UTF-8");
    let ids: Vec<_> = role_labels
        .iter()
        .map(|label| artifact(label).to_string())
        .collect();
    for left in 0..ids.len() {
        for right in (left + 1)..ids.len() {
            let aliased = canonical.replacen(&ids[right], &ids[left], 1);
            assert_ne!(aliased, canonical, "missing role {}", ids[right]);
            assert_decode_rejected(&aliased);
        }
    }
}

#[test]
fn provenance_text_limits_are_exact_and_byte_based() {
    let token_128 = "a".repeat(128);
    for index in [0, 1, 2, 3, 5] {
        let mut text = VALID_MODEL_TEXT;
        text[index] = &token_128;
        model_with_text(text, 1).expect("128-byte model token");
    }
    let token_129 = "a".repeat(129);
    for index in [0, 1, 2, 3, 5] {
        let mut text = VALID_MODEL_TEXT;
        text[index] = &token_129;
        assert!(model_with_text(text, 1).is_err());
    }
    let mut text = VALID_MODEL_TEXT;
    text[0] = "é";
    assert!(model_with_text(text, 1).is_err());
    assert!(model_with_text(VALID_MODEL_TEXT, 0).is_err());

    let citation_4096 = "c".repeat(4_096);
    let mut text = VALID_MODEL_TEXT;
    text[4] = &citation_4096;
    model_with_text(text, 1).expect("4,096-byte citation");
    let citation_4097 = "c".repeat(4_097);
    let mut text = VALID_MODEL_TEXT;
    text[4] = &citation_4097;
    assert!(model_with_text(text, 1).is_err());

    let unicode_4096 = "é".repeat(2_048);
    let mut text = VALID_MODEL_TEXT;
    text[4] = &unicode_4096;
    model_with_text(text, 1).expect("4,096-byte UTF-8 citation");
    let unicode_4097 = format!("{unicode_4096}a");
    let mut text = VALID_MODEL_TEXT;
    text[4] = &unicode_4097;
    assert!(model_with_text(text, 1).is_err());

    for invalid in ["", " leading", "trailing ", "embedded\ncontrol"] {
        let mut text = VALID_MODEL_TEXT;
        text[4] = invalid;
        assert!(model_with_text(text, 1).is_err());
    }

    MultiscaleEmbeddingExecutionProvenance::new(
        artifact(b"limit-run"),
        artifact(b"limit-environment"),
        artifact(b"limit-converter"),
        &token_128,
        &token_128,
        BUDGET,
    )
    .expect("128-byte execution tokens");
    assert!(MultiscaleEmbeddingExecutionProvenance::new(
        artifact(b"limit-run"),
        artifact(b"limit-environment"),
        artifact(b"limit-converter"),
        &token_129,
        "v1",
        BUDGET,
    )
    .is_err());

    let slide = SlideId::new("limit-slide").expect("slide");
    let support = provenance_patch_support(&slide);
    for (dimension, pooling) in [(1, token_128.as_str()), (65_536, "mean")] {
        MultiscaleEmbeddingProvenance::direct_patch(
            slide.clone(),
            &support,
            dimension,
            pooling,
            provenance_model(),
            provenance_execution(),
            artifact(b"limit-preprocessing"),
            direct_inputs(),
            BUDGET,
        )
        .expect("exact dimension/pooling boundary");
    }
    assert!(MultiscaleEmbeddingProvenance::direct_patch(
        slide,
        &support,
        1,
        token_129,
        provenance_model(),
        provenance_execution(),
        artifact(b"limit-preprocessing"),
        direct_inputs(),
        BUDGET,
    )
    .is_err());
}

#[test]
fn provenance_accepts_escaped_typed_ids_but_rejects_noncanonical_or_drifted_wires() {
    let slide = SlideId::new("quoted-\"slide\\path").expect("escaped slide ID");
    let provenance = MultiscaleEmbeddingProvenance::direct_patch(
        slide.clone(),
        &provenance_patch_support(&slide),
        1_024,
        "mean_patch_tokens",
        provenance_model(),
        provenance_execution(),
        artifact(b"escaped-slide-preprocessing"),
        direct_inputs(),
        BUDGET,
    )
    .expect("escaped slide provenance");
    let bytes = provenance.to_canonical_json().expect("canonical JSON");
    assert_eq!(
        MultiscaleEmbeddingProvenance::from_canonical_json(&bytes, bytes.len(), BUDGET, BUDGET,)
            .expect("escaped slide round trip"),
        provenance
    );

    let direct = String::from_utf8(
        direct_patch_provenance()
            .to_canonical_json()
            .expect("direct JSON"),
    )
    .expect("UTF-8");
    assert_decode_rejected(&direct.replace("patch_encoder", "patch\\u005fencoder"));
    assert_decode_rejected(&direct.replace(
        &artifact(b"checkpoint").to_string(),
        &artifact(b"checkpoint").to_string().to_uppercase(),
    ));

    let region = String::from_utf8(
        derived_region_provenance()
            .to_canonical_json()
            .expect("region JSON"),
    )
    .expect("UTF-8");
    assert_decode_rejected(&region.replace("weighted_mean", "arithmetic_mean"));
    assert_decode_rejected(
        &region.replace("\"entity_kind\":\"region\"", "\"entity_kind\":\"slide\""),
    );

    for provenance in [
        slide_from_patches_provenance(),
        slide_from_regions_provenance(),
    ] {
        let slide_json =
            String::from_utf8(provenance.to_canonical_json().expect("derived slide JSON"))
                .expect("UTF-8");
        assert_decode_rejected(&slide_json.replace("arithmetic_mean", "weighted_mean"));
        assert_decode_rejected(
            &slide_json.replace("\"entity_kind\":\"slide\"", "\"entity_kind\":\"region\""),
        );
    }
}

#[test]
fn every_provenance_artifact_role_must_be_distinct() {
    assert_all_role_aliases_rejected(
        &direct_patch_provenance(),
        &[
            b"checkpoint",
            b"source-snapshot",
            b"license",
            b"input-normalization",
            b"preprocessing",
            b"run-config",
            b"environment",
            b"converter",
            b"source-entities",
            b"source-vectors",
            b"expected-patches",
            b"identity-map",
            b"source-row-link",
            b"patch-support",
        ],
    );
    assert_all_role_aliases_rejected(
        &derived_region_provenance(),
        &[
            b"run-config",
            b"environment",
            b"converter",
            b"source-patch-table",
            b"patch-region-link",
            b"expected-regions",
            b"region-support",
            b"weighted-derivation",
        ],
    );
    assert_all_role_aliases_rejected(
        &slide_from_patches_provenance(),
        &[
            b"run-config",
            b"environment",
            b"converter",
            b"source-patch-table",
            b"expected-slides",
            b"slide-patch-support",
            b"arithmetic-derivation",
        ],
    );
    assert_all_role_aliases_rejected(
        &slide_from_regions_provenance(),
        &[
            b"run-config",
            b"environment",
            b"converter",
            b"source-region-table",
            b"expected-slides",
            b"slide-region-support",
            b"arithmetic-derivation",
        ],
    );
}
