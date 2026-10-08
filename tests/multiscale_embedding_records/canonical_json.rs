use super::support::*;

#[test]
fn source_identity_json_is_an_exact_strict_fixed_point() {
    let fixture = fixture();
    let source_bytes = fixture
        .source_entities
        .to_canonical_json()
        .expect("source JSON");
    assert_eq!(
        String::from_utf8(source_bytes.clone()).expect("UTF-8"),
        concat!(
            "{\"format\":\"marklab.patch_source_entity_set\",\"version\":1,",
            "\"source_profile\":\"synthetic_patch_profile.v1\",\"entries\":[",
            "{\"source_patch_id\":\"source-a\",\"source_entity_row\":2},",
            "{\"source_patch_id\":\"source-b\",\"source_entity_row\":0},",
            "{\"source_patch_id\":\"source-c\",\"source_entity_row\":3},",
            "{\"source_patch_id\":\"source-d\",\"source_entity_row\":1}]}\n"
        )
    );
    assert_eq!(
        PatchSourceEntitySet::from_canonical_json(&source_bytes, BUDGET, BUDGET, BUDGET,)
            .expect("source round trip"),
        fixture.source_entities
    );

    let map_bytes = fixture.identity_map.to_canonical_json().expect("map JSON");
    let expected_map = format!(
        concat!(
            "{{\"format\":\"marklab.patch_identity_map\",\"version\":1,",
            "\"source_entities_artifact_id\":\"{}\",",
            "\"source_entities_logical_digest\":\"{}\",",
            "\"expected_patches_artifact_id\":\"{}\",",
            "\"expected_patches_logical_digest\":\"{}\",\"entries\":[",
            "{{\"source_patch_id\":\"source-a\",\"patch_id\":\"patch-c\"}},",
            "{{\"source_patch_id\":\"source-b\",\"patch_id\":\"patch-a\"}},",
            "{{\"source_patch_id\":\"source-c\",\"patch_id\":\"patch-d\"}},",
            "{{\"source_patch_id\":\"source-d\",\"patch_id\":\"patch-b\"}}]}}\n"
        ),
        artifact(b"source-entities"),
        fixture.source_entities.logical_digest(),
        artifact(b"expected-patches"),
        fixture.expected.logical_digest(),
    );
    assert_eq!(map_bytes, expected_map.as_bytes());
    assert_eq!(
        PatchIdentityMap::from_canonical_json(
            &map_bytes,
            &fixture.source_entities,
            &fixture.expected,
            BUDGET,
            BUDGET,
            BUDGET,
        )
        .expect("map round trip"),
        fixture.identity_map
    );

    let link = row_link(&fixture);
    let link_bytes = link.to_canonical_json().expect("row-link JSON");
    let expected_link = format!(
        concat!(
            "{{\"format\":\"marklab.patch_embedding_source_row_link\",\"version\":1,",
            "\"source_entities_artifact_id\":\"{}\",",
            "\"source_entities_logical_digest\":\"{}\",",
            "\"source_vectors_artifact_id\":\"{}\",",
            "\"expected_patches_artifact_id\":\"{}\",",
            "\"expected_patches_logical_digest\":\"{}\",",
            "\"identity_map_artifact_id\":\"{}\",",
            "\"identity_map_logical_digest\":\"{}\",",
            "\"converter_artifact_id\":\"{}\",\"entries\":[",
            "{{\"patch_id\":\"patch-a\",\"embedding_status\":\"present\",",
            "\"source_entity_row\":0,\"source_vector_row\":1}},",
            "{{\"patch_id\":\"patch-b\",\"embedding_status\":\"missing_vector\",",
            "\"source_entity_row\":1,\"source_vector_row\":null}},",
            "{{\"patch_id\":\"patch-c\",\"embedding_status\":\"qc_rejected\",",
            "\"source_entity_row\":2,\"source_vector_row\":0}},",
            "{{\"patch_id\":\"patch-d\",\"embedding_status\":\"extraction_failed\",",
            "\"source_entity_row\":3,\"source_vector_row\":null}}]}}\n"
        ),
        artifact(b"source-entities"),
        fixture.source_entities.logical_digest(),
        artifact(b"source-vectors"),
        artifact(b"expected-patches"),
        fixture.expected.logical_digest(),
        artifact(b"identity-map"),
        fixture.identity_map.logical_digest(),
        artifact(b"converter"),
    );
    assert_eq!(link_bytes, expected_link.as_bytes());
    assert_eq!(
        PatchEmbeddingSourceRowLink::from_canonical_json(
            &link_bytes,
            &fixture.source_entities,
            &fixture.expected,
            &fixture.identity_map,
            BUDGET,
            BUDGET,
            BUDGET,
        )
        .expect("row-link round trip"),
        link
    );

    let invalid_source = [
        source_bytes[..source_bytes.len() - 1].to_vec(),
        [b" ".as_slice(), source_bytes.as_slice()].concat(),
        source_bytes.replacen(b"\"format\"", b"\"unknown\"", 1),
        source_bytes.replacen(b"\"version\":1", b"\"version\":1,\"version\":1", 1),
        source_bytes.replacen(
            b"\"source_profile\":\"synthetic_patch_profile.v1\"",
            b"\"source_profile\":\"synthetic_patch_profile.v1\",\"extra\":0",
            1,
        ),
    ];
    for invalid in invalid_source {
        assert!(
            PatchSourceEntitySet::from_canonical_json(&invalid, BUDGET, BUDGET, BUDGET,).is_err()
        );
    }
    let wrong_source_digest = ContentDigest::from_bytes(b"wrong-source-digest").to_string();
    let source_artifact = artifact(b"source-entities").to_string();
    let invalid_map = [
        map_bytes.replacen(
            fixture
                .source_entities
                .logical_digest()
                .to_string()
                .as_bytes(),
            wrong_source_digest.as_bytes(),
            1,
        ),
        map_bytes.replacen(b"\"source_entities_artifact_id\"", b"\"wrong_key\"", 1),
        map_bytes.replacen(b"\"patch_id\":\"patch-a\"", b"\"patch_id\":\"patch-c\"", 1),
        map_bytes.replacen(
            source_artifact.as_bytes(),
            source_artifact.to_uppercase().as_bytes(),
            1,
        ),
    ];
    for invalid in invalid_map {
        assert!(PatchIdentityMap::from_canonical_json(
            &invalid,
            &fixture.source_entities,
            &fixture.expected,
            BUDGET,
            BUDGET,
            BUDGET,
        )
        .is_err());
    }
    let invalid_link = [
        link_bytes.replacen(b"\"source_vector_row\":1", b"\"source_vector_row\":null", 1),
        link_bytes.replacen(
            b"\"embedding_status\":\"present\"",
            b"\"embedding_status\":\"unknown\"",
            1,
        ),
        link_bytes.replacen(
            b"\"converter_artifact_id\"",
            b"\"extra\":0,\"converter_artifact_id\"",
            1,
        ),
    ];
    for invalid in invalid_link {
        assert!(PatchEmbeddingSourceRowLink::from_canonical_json(
            &invalid,
            &fixture.source_entities,
            &fixture.expected,
            &fixture.identity_map,
            BUDGET,
            BUDGET,
            BUDGET,
        )
        .is_err());
    }
    assert!(matches!(
        PatchSourceEntitySet::from_canonical_json(
            &source_bytes,
            source_bytes.len() - 1,
            BUDGET,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded { .. })
    ));
}

#[test]
fn input_normalization_has_a_bounded_decimal_grammar_and_fixed_identity() {
    let normalization = normalization();
    assert_eq!(
        normalization.logical_digest(),
        reference_normalization_digest(&normalization)
    );
    assert_eq!(
        normalization.logical_digest().to_string(),
        "bd7e6b0644392a32a26f8e38e6d32e5a6fa35e25f27378657a3a898a06773e0b"
    );
    let bytes = normalization
        .to_canonical_json()
        .expect("normalization JSON");
    assert_eq!(
        String::from_utf8(bytes.clone()).expect("UTF-8"),
        concat!(
            "{\"format\":\"marklab.patch_embedding_input_normalization\",\"version\":1,",
            "\"modality\":\"he_brightfield\",\"color_space\":\"srgb\",",
            "\"channel_order\":[\"r\",\"g\",\"b\"],",
            "\"value_range\":{\"minimum\":\"0\",\"maximum\":\"1\"},",
            "\"channel_mean\":[\"0.485\",\"0.456\",\"0.406\"],",
            "\"channel_std\":[\"0.229\",\"0.224\",\"0.225\"],",
            "\"stain_normalization\":{\"kind\":\"none\"}}\n"
        )
    );
    assert_eq!(
        PatchEmbeddingInputNormalization::from_canonical_json(&bytes, BUDGET, BUDGET, BUDGET,)
            .expect("normalization round trip"),
        normalization
    );
    let invalid_json = [
        bytes[..bytes.len() - 1].to_vec(),
        bytes.replacen(
            b"\"modality\":\"he_brightfield\"",
            b"\"modality\":\"fluorescence\"",
            1,
        ),
        bytes.replacen(
            b"\"channel_order\":[\"r\",\"g\",\"b\"]",
            b"\"channel_order\":[\"b\",\"g\",\"r\"]",
            1,
        ),
        bytes.replacen(b"\"kind\":\"none\"", b"\"kind\":\"learned\"", 1),
    ];
    for invalid in invalid_json {
        assert!(PatchEmbeddingInputNormalization::from_canonical_json(
            &invalid, BUDGET, BUDGET, BUDGET,
        )
        .is_err());
    }

    for invalid in [
        "", "+1", "-0", "00", "01", "1.", ".1", "0.0", "1.20", "1e2", "NaN", "inf",
    ] {
        assert!(PatchNormalizationDecimal::new(invalid).is_err());
    }
    assert!(PatchNormalizationDecimal::new("1".repeat(65)).is_err());
    let decimal = |value| PatchNormalizationDecimal::new(value).expect("decimal");
    assert!(PatchEmbeddingInputNormalization::he_srgb(
        [decimal("0"), decimal("0"), decimal("0")],
        [decimal("0.1"), decimal("0"), decimal("0.1")],
        BUDGET,
    )
    .is_err());
}

trait ReplaceBytes {
    fn replacen(&self, from: &[u8], to: &[u8], count: usize) -> Vec<u8>;
}

impl ReplaceBytes for Vec<u8> {
    fn replacen(&self, from: &[u8], to: &[u8], count: usize) -> Vec<u8> {
        let mut result = Vec::new();
        let mut remaining = self.as_slice();
        let mut replaced = 0;
        while replaced < count {
            let Some(offset) = remaining
                .windows(from.len())
                .position(|window| window == from)
            else {
                break;
            };
            result.extend_from_slice(&remaining[..offset]);
            result.extend_from_slice(to);
            remaining = &remaining[offset + from.len()..];
            replaced += 1;
        }
        result.extend_from_slice(remaining);
        result
    }
}
