use super::support::*;

#[test]
fn small_records_reject_aliases_variant_drift_and_noncanonical_json() {
    assert!(MultiscaleEmbeddingDerivationContract::weighted_mean("bad version", BUDGET).is_err());
    assert!(MultiscaleEmbeddingDerivationContract::weighted_mean("a".repeat(128), BUDGET).is_ok());
    assert!(MultiscaleEmbeddingDerivationContract::weighted_mean("a".repeat(129), BUDGET).is_err());
    let derivation = MultiscaleEmbeddingDerivationContract::weighted_mean("fractions.v1", BUDGET)
        .expect("derivation");
    let bytes = derivation.to_canonical_json().expect("derivation JSON");
    let reordered = String::from_utf8(bytes.clone()).expect("UTF-8").replace(
        "\"kind\":\"deterministic\",\"algorithm\":\"weighted_mean\"",
        "\"algorithm\":\"weighted_mean\",\"kind\":\"deterministic\"",
    );
    assert!(MultiscaleEmbeddingDerivationContract::from_canonical_json(
        reordered.as_bytes(),
        BUDGET,
        BUDGET,
        BUDGET,
    )
    .is_err());

    let duplicate = binding(b"duplicate-support-role");
    assert!(matches!(
        MultiscaleEmbeddingSupport::region_from_patches(
            SlideId::new("support-slide").expect("slide"),
            duplicate,
            duplicate,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::DuplicateMultiscaleSupportArtifactDependency)
    ));
    assert!(matches!(
        MultiscaleEmbeddingSupport::region_from_patches(
            SlideId::new("support-slide").expect("slide"),
            MultiscaleArtifactBinding::new(
                duplicate.artifact_id(),
                ContentDigest::from_bytes(b"logical-a"),
            ),
            MultiscaleArtifactBinding::new(
                duplicate.artifact_id(),
                ContentDigest::from_bytes(b"logical-b"),
            ),
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::DuplicateMultiscaleSupportArtifactDependency)
    ));
    let support = MultiscaleEmbeddingSupport::patch(
        SlideId::new("support-slide").expect("slide"),
        binding(b"context"),
        binding(b"footprints"),
        binding(b"overlap"),
        BUDGET,
    )
    .expect("support");
    let drifted = String::from_utf8(support.to_canonical_json().expect("support JSON"))
        .expect("UTF-8")
        .replace("\"entity_kind\":\"patch\"", "\"entity_kind\":\"region\"");
    assert!(MultiscaleEmbeddingSupport::from_canonical_json(
        drifted.as_bytes(),
        BUDGET,
        BUDGET,
        BUDGET,
    )
    .is_err());
    let support_bytes = support.to_canonical_json().expect("support JSON");
    let support_text = String::from_utf8(support_bytes.clone()).expect("UTF-8");
    let uppercase = support_text.replace(
        &artifact(b"context").to_string(),
        &artifact(b"context").to_string().to_ascii_uppercase(),
    );
    let support_cases = [
        support_text.trim_end_matches('\n').to_owned(),
        format!("{support_text}\n"),
        support_text.replace(
            "\"variant\":\"patch\"",
            "\"variant\":\"patch\",\"variant\":\"patch\"",
        ),
        support_text.replace(
            "\"patch_overlap_graph\":",
            "\"unexpected\":0,\"patch_overlap_graph\":",
        ),
        uppercase,
    ];
    for invalid in support_cases {
        assert!(MultiscaleEmbeddingSupport::from_canonical_json(
            invalid.as_bytes(),
            BUDGET,
            BUDGET,
            BUDGET,
        )
        .is_err());
    }

    let alias = artifact(b"producer-alias");
    assert!(matches!(
        CellPatchLinkProducer::contained_shared(
            "contained.v1",
            alias,
            alias,
            artifact(b"environment"),
            artifact(b"converter"),
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::DuplicateCellPatchProducerArtifactDependency)
    ));
    assert!(CellPatchLinkProducer::declared_weighted_interpolation(
        "not a token",
        "weights.v1",
        artifact(b"coordinates"),
        artifact(b"run"),
        artifact(b"environment"),
        artifact(b"converter"),
        BUDGET,
    )
    .is_err());
    assert!(CellPatchLinkProducer::declared_weighted_interpolation(
        "all_half_open_anchor_containment",
        "weights.v1",
        artifact(b"coordinates"),
        artifact(b"run"),
        artifact(b"environment"),
        artifact(b"converter"),
        BUDGET,
    )
    .is_err());
    assert!(CellPatchLinkProducer::declared_weighted_interpolation(
        "a".repeat(128),
        "v1",
        artifact(b"coordinates"),
        artifact(b"run"),
        artifact(b"environment"),
        artifact(b"converter"),
        BUDGET,
    )
    .is_ok());
    assert!(CellPatchLinkProducer::declared_weighted_interpolation(
        "a".repeat(129),
        "v1",
        artifact(b"coordinates"),
        artifact(b"run"),
        artifact(b"environment"),
        artifact(b"converter"),
        BUDGET,
    )
    .is_err());
    let base_dependencies = [
        artifact(b"coordinates"),
        artifact(b"run"),
        artifact(b"environment"),
        artifact(b"converter"),
    ];
    for left in 0..base_dependencies.len() {
        for right in (left + 1)..base_dependencies.len() {
            let mut dependencies = base_dependencies;
            dependencies[right] = dependencies[left];
            assert!(matches!(
                CellPatchLinkProducer::contained_shared(
                    "contained.v1",
                    dependencies[0],
                    dependencies[1],
                    dependencies[2],
                    dependencies[3],
                    BUDGET,
                ),
                Err(MultiscaleEmbeddingError::DuplicateCellPatchProducerArtifactDependency)
            ));
        }
    }
    let producer = CellPatchLinkProducer::declared_weighted_interpolation(
        "registered_bilinear_weights",
        "weights.v1",
        base_dependencies[0],
        base_dependencies[1],
        base_dependencies[2],
        base_dependencies[3],
        BUDGET,
    )
    .expect("producer");
    let producer_text =
        String::from_utf8(producer.to_canonical_json().expect("producer JSON")).expect("UTF-8");
    let producer_cases = [
        producer_text.trim_end_matches('\n').to_owned(),
        format!("{producer_text}\n"),
        producer_text.replace(
            "\"algorithm\":\"registered_bilinear_weights\"",
            "\"algorithm\":\"registered\\u005fbilinear_weights\"",
        ),
        producer_text.replace(
            "\"algorithm\":\"registered_bilinear_weights\"",
            "\"algorithm\":\"all_half_open_anchor_containment\"",
        ),
        producer_text.replace(
            "\"algorithm_version\":\"weights.v1\"",
            "\"algorithm\":\"registered_bilinear_weights\",\"algorithm_version\":\"weights.v1\"",
        ),
        producer_text.replace(
            &format!(",\"converter_artifact_id\":\"{}\"", base_dependencies[3]),
            "",
        ),
        producer_text.replace(
            "\"converter_artifact_id\":",
            "\"unexpected\":0,\"converter_artifact_id\":",
        ),
    ];
    for invalid in producer_cases {
        assert!(CellPatchLinkProducer::from_canonical_json(
            invalid.as_bytes(),
            BUDGET,
            BUDGET,
            BUDGET,
        )
        .is_err());
    }

    let fixture = region_record_fixture();
    let assessment = PatchRegionAssessment::new(
        &fixture.expected_patches,
        &fixture.expected_regions,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        region_declarations(),
        BUDGET,
        BUDGET,
    )
    .expect("assessment");
    let assessment_text =
        String::from_utf8(assessment.to_canonical_json().expect("assessment JSON")).expect("UTF-8");
    let uppercase_converter = assessment_text.replace(
        &fixture.bindings.converter_artifact_id().to_string(),
        &fixture
            .bindings
            .converter_artifact_id()
            .to_string()
            .to_ascii_uppercase(),
    );
    let assessment_cases = [
        assessment_text.replace("\"assessed_pair_count\":4", "\"assessed_pair_count\":3"),
        assessment_text.trim_end_matches('\n').to_owned(),
        format!("{assessment_text}\n"),
        assessment_text.replace(
            "\"assessment_policy\":\"expected_cartesian_exhaustive\"",
            concat!(
                "\"assessment_policy\":\"expected_cartesian_exhaustive\",",
                "\"assessment_policy\":\"expected_cartesian_exhaustive\""
            ),
        ),
        assessment_text.replace(
            "\"nonzero_relations_digest\":",
            "\"unexpected\":0,\"nonzero_relations_digest\":",
        ),
        uppercase_converter,
    ];
    for invalid in assessment_cases {
        assert!(assessment
            .validate_canonical_json(invalid.as_bytes(), BUDGET, BUDGET)
            .is_err());
    }
}
