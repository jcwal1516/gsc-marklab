use super::support::*;

#[test]
fn producer_and_assessment_records_have_exact_canonical_content_identity() {
    let producer = CellPatchLinkProducer::contained_shared(
        "containment.v1",
        artifact(b"source-coordinates"),
        artifact(b"run-config"),
        artifact(b"environment"),
        artifact(b"converter"),
        BUDGET,
    )
    .expect("contained producer");
    assert!(!format!("{producer:?}").contains("containment.v1"));
    assert!(!format!("{producer:?}").contains(&artifact(b"converter").to_string()));
    let producer_bytes = producer.to_canonical_json().expect("producer JSON");
    assert_eq!(
        ContentDigest::from_bytes(&producer_bytes).to_string(),
        "4a643365fa4a49afc410202640c106b0e4cdbdf1ab83273a311128047dad42e3"
    );
    let expected_producer = format!(
        concat!(
            "{{\"format\":\"marklab.cell_patch_link_producer\",\"version\":1,",
            "\"assignment_mode\":\"contained_shared\",",
            "\"algorithm\":\"all_half_open_anchor_containment\",",
            "\"algorithm_version\":\"containment.v1\",",
            "\"source_coordinates_artifact_id\":\"{}\",",
            "\"run_config_artifact_id\":\"{}\",",
            "\"environment_artifact_id\":\"{}\",",
            "\"converter_artifact_id\":\"{}\"}}\n"
        ),
        artifact(b"source-coordinates"),
        artifact(b"run-config"),
        artifact(b"environment"),
        artifact(b"converter"),
    );
    assert_eq!(
        String::from_utf8(producer_bytes.clone()).expect("UTF-8"),
        expected_producer
    );
    assert_eq!(
        CellPatchLinkProducer::from_canonical_json(
            &producer_bytes,
            producer_bytes.len(),
            BUDGET,
            BUDGET,
        )
        .expect("producer round trip"),
        producer
    );
    assert_eq!(
        producer.assignment_mode(),
        CellPatchAssignmentMode::ContainedShared
    );
    assert_eq!(producer.algorithm_version(), "containment.v1");
    assert_eq!(
        producer.direct_dependencies().collect::<Vec<_>>(),
        sorted_artifacts([
            artifact(b"source-coordinates"),
            artifact(b"run-config"),
            artifact(b"environment"),
            artifact(b"converter"),
        ])
    );

    let interpolation = CellPatchLinkProducer::declared_weighted_interpolation(
        "registered_bilinear_weights",
        "weights.v2",
        artifact(b"source-coordinates"),
        artifact(b"run-config"),
        artifact(b"environment"),
        artifact(b"converter"),
        BUDGET,
    )
    .expect("interpolation producer");
    assert_eq!(interpolation.algorithm(), "registered_bilinear_weights");
    assert_eq!(
        interpolation.assignment_mode(),
        CellPatchAssignmentMode::DeclaredWeightedInterpolation
    );
    assert_eq!(interpolation.algorithm_version(), "weights.v2");
    let interpolation_bytes = interpolation
        .to_canonical_json()
        .expect("interpolation JSON");
    assert_eq!(
        ContentDigest::from_bytes(&interpolation_bytes).to_string(),
        "49bbb0e70f00ecfd03d9c8dc96c73b1c2d0e15b1343e8b0fce3a7b777d4336bc"
    );
    let expected_interpolation = format!(
        concat!(
            "{{\"format\":\"marklab.cell_patch_link_producer\",\"version\":1,",
            "\"assignment_mode\":\"declared_weighted_interpolation\",",
            "\"algorithm\":\"registered_bilinear_weights\",",
            "\"algorithm_version\":\"weights.v2\",",
            "\"source_coordinates_artifact_id\":\"{}\",",
            "\"run_config_artifact_id\":\"{}\",",
            "\"environment_artifact_id\":\"{}\",",
            "\"converter_artifact_id\":\"{}\"}}\n"
        ),
        artifact(b"source-coordinates"),
        artifact(b"run-config"),
        artifact(b"environment"),
        artifact(b"converter"),
    );
    assert_eq!(
        String::from_utf8(interpolation_bytes.clone()).expect("UTF-8"),
        expected_interpolation
    );
    assert_eq!(
        CellPatchLinkProducer::from_canonical_json(
            &interpolation_bytes,
            interpolation_bytes.len(),
            BUDGET,
            BUDGET,
        )
        .expect("interpolation round trip"),
        interpolation
    );
    assert_eq!(
        interpolation.direct_dependencies().collect::<Vec<_>>(),
        sorted_artifacts([
            artifact(b"source-coordinates"),
            artifact(b"run-config"),
            artifact(b"environment"),
            artifact(b"converter"),
        ])
    );

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
    let assessment_bytes = assessment.to_canonical_json().expect("assessment JSON");
    assert_eq!(
        ContentDigest::from_bytes(&assessment_bytes).to_string(),
        "dfcaabc52dfd86a403c32d6aea6e953f538f09c564ba87941b783aca444670d3"
    );
    let expected_assessment = format!(
        concat!(
            "{{\"format\":\"marklab.patch_region_assessment\",\"version\":1,",
            "\"assessment_policy\":\"expected_cartesian_exhaustive\",",
            "\"owning_slide_id\":\"{}\",",
            "\"expected_patches_artifact_id\":\"{}\",",
            "\"expected_patches_logical_digest\":\"{}\",",
            "\"expected_regions_artifact_id\":\"{}\",",
            "\"expected_regions_logical_digest\":\"{}\",",
            "\"patch_context_artifact_id\":\"{}\",",
            "\"patch_context_logical_digest\":\"{}\",",
            "\"footprint_artifact_id\":\"{}\",",
            "\"footprint_logical_digest\":\"{}\",",
            "\"converter_artifact_id\":\"{}\",",
            "\"assessed_pair_count\":4,\"nonzero_relation_count\":3,",
            "\"nonzero_relations_digest\":\"{}\"}}\n"
        ),
        assessment.owning_slide_id(),
        fixture.footprints.expected_patches_artifact_id(),
        fixture.expected_patches.logical_digest(),
        fixture.bindings.expected_regions_artifact_id(),
        fixture.expected_regions.logical_digest(),
        fixture.footprints.patch_context_artifact_id(),
        fixture.context.logical_digest(),
        fixture.bindings.patch_footprints_artifact_id(),
        fixture.footprints.logical_digest(),
        fixture.bindings.converter_artifact_id(),
        assessment.nonzero_relations_digest(),
    );
    assert_eq!(
        String::from_utf8(assessment_bytes.clone()).expect("UTF-8"),
        expected_assessment
    );
    assessment
        .validate_canonical_json(&assessment_bytes, assessment_bytes.len(), BUDGET)
        .expect("assessment fixed point");
    assert_eq!(
        assessment.direct_dependencies().collect::<Vec<_>>(),
        sorted_artifacts([
            fixture.footprints.expected_patches_artifact_id(),
            fixture.bindings.expected_regions_artifact_id(),
            fixture.footprints.patch_context_artifact_id(),
            fixture.bindings.patch_footprints_artifact_id(),
            fixture.bindings.converter_artifact_id(),
        ])
    );
}
