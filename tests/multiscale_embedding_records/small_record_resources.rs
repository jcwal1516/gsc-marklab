use super::support::*;

#[test]
fn small_record_constructors_and_decoders_enforce_exact_budget_edges() {
    let mut version = String::with_capacity(64);
    version.push_str("fractions.v1");
    let required = match MultiscaleEmbeddingDerivationContract::weighted_mean(version, 0) {
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, .. }) => required,
        other => panic!("unexpected retained probe: {other:?}"),
    };
    let mut exact_version = String::with_capacity(64);
    exact_version.push_str("fractions.v1");
    assert!(MultiscaleEmbeddingDerivationContract::weighted_mean(exact_version, required).is_ok());
    let mut version = String::with_capacity(64);
    version.push_str("fractions.v1");
    assert!(matches!(
        MultiscaleEmbeddingDerivationContract::weighted_mean(version, required - 1),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required: observed, maximum })
            if observed == required && maximum == required - 1
    ));

    let slide = SlideId::new("budget-support-slide").expect("slide");
    let context = binding(b"budget-context");
    let footprints = binding(b"budget-footprints");
    let overlap = binding(b"budget-overlap");
    let support_required =
        match MultiscaleEmbeddingSupport::patch(slide.clone(), context, footprints, overlap, 0) {
            Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, .. }) => required,
            other => panic!("unexpected support retained probe: {other:?}"),
        };
    assert!(MultiscaleEmbeddingSupport::patch(
        slide.clone(),
        context,
        footprints,
        overlap,
        support_required,
    )
    .is_ok());
    assert!(matches!(
        MultiscaleEmbeddingSupport::patch(
            slide,
            context,
            footprints,
            overlap,
            support_required - 1,
        ),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, maximum })
            if required == support_required && maximum == support_required - 1
    ));
    let support = MultiscaleEmbeddingSupport::region_from_patches(
        SlideId::new("budget-support-slide").expect("slide"),
        binding(b"budget-patch-support"),
        binding(b"budget-region-link"),
        BUDGET,
    )
    .expect("support");
    let support_bytes = support.to_canonical_json().expect("support JSON");
    assert!(matches!(
        MultiscaleEmbeddingSupport::from_canonical_json(
            &support_bytes,
            support_bytes.len() - 1,
            BUDGET,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded { observed, maximum })
            if observed == support_bytes.len() && maximum == support_bytes.len() - 1
    ));
    let support_decoded = match MultiscaleEmbeddingSupport::from_canonical_json(
        &support_bytes,
        support_bytes.len(),
        0,
        BUDGET,
    ) {
        Err(MultiscaleEmbeddingError::DecodedByteBudgetExceeded { required, .. }) => required,
        other => panic!("unexpected support decoded probe: {other:?}"),
    };
    let support_retained = match MultiscaleEmbeddingSupport::from_canonical_json(
        &support_bytes,
        support_bytes.len(),
        support_decoded,
        0,
    ) {
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, .. }) => required,
        other => panic!("unexpected support retained decode probe: {other:?}"),
    };
    assert!(MultiscaleEmbeddingSupport::from_canonical_json(
        &support_bytes,
        support_bytes.len(),
        support_decoded,
        support_retained,
    )
    .is_ok());
    assert!(matches!(
        MultiscaleEmbeddingSupport::from_canonical_json(
            &support_bytes,
            support_bytes.len(),
            support_decoded - 1,
            support_retained,
        ),
        Err(MultiscaleEmbeddingError::DecodedByteBudgetExceeded { required, maximum })
            if required == support_decoded && maximum == support_decoded - 1
    ));
    assert!(matches!(
        MultiscaleEmbeddingSupport::from_canonical_json(
            &support_bytes,
            support_bytes.len(),
            support_decoded,
            support_retained - 1,
        ),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, maximum })
            if required == support_retained && maximum == support_retained - 1
    ));

    let mut algorithm = String::with_capacity(80);
    algorithm.push_str("registered_bilinear_weights");
    let mut producer_version = String::with_capacity(96);
    producer_version.push_str("weights.v1");
    let producer_required = match CellPatchLinkProducer::declared_weighted_interpolation(
        algorithm,
        producer_version,
        artifact(b"budget-coordinates"),
        artifact(b"budget-run"),
        artifact(b"budget-environment"),
        artifact(b"budget-converter"),
        0,
    ) {
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, .. }) => required,
        other => panic!("unexpected producer retained probe: {other:?}"),
    };
    let make_producer = |budget| {
        let mut algorithm = String::with_capacity(80);
        algorithm.push_str("registered_bilinear_weights");
        let mut version = String::with_capacity(96);
        version.push_str("weights.v1");
        CellPatchLinkProducer::declared_weighted_interpolation(
            algorithm,
            version,
            artifact(b"budget-coordinates"),
            artifact(b"budget-run"),
            artifact(b"budget-environment"),
            artifact(b"budget-converter"),
            budget,
        )
    };
    let producer = make_producer(producer_required).expect("exact producer retained budget");
    assert!(matches!(
        make_producer(producer_required - 1),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, maximum })
            if required == producer_required && maximum == producer_required - 1
    ));
    let producer_bytes = producer.to_canonical_json().expect("producer JSON");
    assert!(matches!(
        CellPatchLinkProducer::from_canonical_json(
            &producer_bytes,
            producer_bytes.len() - 1,
            BUDGET,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded { observed, maximum })
            if observed == producer_bytes.len() && maximum == producer_bytes.len() - 1
    ));
    let producer_decoded = match CellPatchLinkProducer::from_canonical_json(
        &producer_bytes,
        producer_bytes.len(),
        0,
        BUDGET,
    ) {
        Err(MultiscaleEmbeddingError::DecodedByteBudgetExceeded { required, .. }) => required,
        other => panic!("unexpected producer decoded probe: {other:?}"),
    };
    let producer_retained = match CellPatchLinkProducer::from_canonical_json(
        &producer_bytes,
        producer_bytes.len(),
        producer_decoded,
        0,
    ) {
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, .. }) => required,
        other => panic!("unexpected producer retained decode probe: {other:?}"),
    };
    assert!(CellPatchLinkProducer::from_canonical_json(
        &producer_bytes,
        producer_bytes.len(),
        producer_decoded,
        producer_retained,
    )
    .is_ok());
    assert!(matches!(
        CellPatchLinkProducer::from_canonical_json(
            &producer_bytes,
            producer_bytes.len(),
            producer_decoded - 1,
            producer_retained,
        ),
        Err(MultiscaleEmbeddingError::DecodedByteBudgetExceeded { required, maximum })
            if required == producer_decoded && maximum == producer_decoded - 1
    ));
    assert!(matches!(
        CellPatchLinkProducer::from_canonical_json(
            &producer_bytes,
            producer_bytes.len(),
            producer_decoded,
            producer_retained - 1,
        ),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, maximum })
            if required == producer_retained && maximum == producer_retained - 1
    ));

    let derivation =
        MultiscaleEmbeddingDerivationContract::arithmetic_mean("stable_order.v1", BUDGET)
            .expect("derivation");
    let bytes = derivation.to_canonical_json().expect("derivation JSON");
    assert!(matches!(
        MultiscaleEmbeddingDerivationContract::from_canonical_json(
            &bytes,
            bytes.len() - 1,
            BUDGET,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded { observed, maximum })
            if observed == bytes.len() && maximum == bytes.len() - 1
    ));
    let decoded_required = match MultiscaleEmbeddingDerivationContract::from_canonical_json(
        &bytes,
        bytes.len(),
        0,
        BUDGET,
    ) {
        Err(MultiscaleEmbeddingError::DecodedByteBudgetExceeded { required, .. }) => required,
        other => panic!("unexpected decoded probe: {other:?}"),
    };
    let derivation_retained = match MultiscaleEmbeddingDerivationContract::from_canonical_json(
        &bytes,
        bytes.len(),
        decoded_required,
        0,
    ) {
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, .. }) => required,
        other => panic!("unexpected derivation retained decode probe: {other:?}"),
    };
    assert!(MultiscaleEmbeddingDerivationContract::from_canonical_json(
        &bytes,
        bytes.len(),
        decoded_required,
        derivation_retained,
    )
    .is_ok());
    assert!(matches!(
        MultiscaleEmbeddingDerivationContract::from_canonical_json(
            &bytes,
            bytes.len(),
            decoded_required - 1,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::DecodedByteBudgetExceeded { required, maximum })
            if required == decoded_required && maximum == decoded_required - 1
    ));
    assert!(matches!(
        MultiscaleEmbeddingDerivationContract::from_canonical_json(
            &bytes,
            bytes.len(),
            decoded_required,
            derivation_retained - 1,
        ),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, maximum })
            if required == derivation_retained && maximum == derivation_retained - 1
    ));

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
    let bytes = assessment.to_canonical_json().expect("assessment JSON");
    assert!(matches!(
        assessment.validate_canonical_json(&bytes, bytes.len() - 1, BUDGET),
        Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded { observed, maximum })
            if observed == bytes.len() && maximum == bytes.len() - 1
    ));
    let decoded_required = match assessment.validate_canonical_json(&bytes, bytes.len(), 0) {
        Err(MultiscaleEmbeddingError::DecodedByteBudgetExceeded { required, .. }) => required,
        other => panic!("unexpected assessment decoded probe: {other:?}"),
    };
    assessment
        .validate_canonical_json(&bytes, bytes.len(), decoded_required)
        .expect("exact assessment decoded budget");
    assert!(matches!(
        assessment.validate_canonical_json(&bytes, bytes.len(), decoded_required - 1),
        Err(MultiscaleEmbeddingError::DecodedByteBudgetExceeded { required, maximum })
            if required == decoded_required && maximum == decoded_required - 1
    ));

    const SMALL_RECORD_HARD_MAXIMUM: usize = 256 * 1024;
    let hostile = vec![b' '; SMALL_RECORD_HARD_MAXIMUM + 1];
    assert!(matches!(
        MultiscaleEmbeddingDerivationContract::from_canonical_json(
            &hostile,
            usize::MAX,
            BUDGET,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded { observed, maximum })
            if observed == SMALL_RECORD_HARD_MAXIMUM + 1
                && maximum == SMALL_RECORD_HARD_MAXIMUM
    ));
    assert!(matches!(
        MultiscaleEmbeddingSupport::from_canonical_json(
            &hostile,
            usize::MAX,
            BUDGET,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded { observed, maximum })
            if observed == SMALL_RECORD_HARD_MAXIMUM + 1
                && maximum == SMALL_RECORD_HARD_MAXIMUM
    ));
    assert!(matches!(
        CellPatchLinkProducer::from_canonical_json(
            &hostile,
            usize::MAX,
            BUDGET,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded { observed, maximum })
            if observed == SMALL_RECORD_HARD_MAXIMUM + 1
                && maximum == SMALL_RECORD_HARD_MAXIMUM
    ));
    assert!(matches!(
        assessment.validate_canonical_json(&hostile, usize::MAX, BUDGET),
        Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded { observed, maximum })
            if observed == SMALL_RECORD_HARD_MAXIMUM + 1
                && maximum == SMALL_RECORD_HARD_MAXIMUM
    ));
}
