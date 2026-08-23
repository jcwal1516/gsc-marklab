use super::{provenance_records::*, support::*};

fn text_with_capacity(capacity: usize, value: &str) -> String {
    let mut text = String::with_capacity(capacity);
    text.push_str(value);
    text
}

fn model_text() -> [String; 6] {
    [
        text_with_capacity(40, "patch_encoder"),
        text_with_capacity(48, "1.2.0"),
        text_with_capacity(56, "vit_h"),
        text_with_capacity(64, "Apache-2.0"),
        text_with_capacity(160, "doi:10.1000/marklab-patch-encoder"),
        text_with_capacity(72, "encoder.layer_32"),
    ]
}

fn budget_model(
    maximum_retained_bytes: usize,
) -> Result<MultiscaleDirectPatchModelProvenance, MultiscaleEmbeddingError> {
    let [family, version, architecture, license, citation, tensor] = model_text();
    MultiscaleDirectPatchModelProvenance::new(
        family,
        version,
        architecture,
        artifact(b"budget-checkpoint"),
        ContentDigest::from_bytes(b"budget-checkpoint-content"),
        artifact(b"budget-snapshot"),
        artifact(b"budget-license"),
        license,
        citation,
        tensor,
        32,
        maximum_retained_bytes,
    )
}

fn execution_text() -> [String; 2] {
    [
        text_with_capacity(96, "marklab_patch_converter"),
        text_with_capacity(80, "2.1.0"),
    ]
}

fn budget_execution(
    maximum_retained_bytes: usize,
) -> Result<MultiscaleEmbeddingExecutionProvenance, MultiscaleEmbeddingError> {
    let [name, version] = execution_text();
    MultiscaleEmbeddingExecutionProvenance::new(
        artifact(b"budget-run"),
        artifact(b"budget-environment"),
        artifact(b"budget-converter"),
        name,
        version,
        maximum_retained_bytes,
    )
}

fn assert_retained_edge<T>(
    required: usize,
    mut construct: impl FnMut(usize) -> Result<T, MultiscaleEmbeddingError>,
) {
    assert!(construct(required).is_ok());
    assert!(matches!(
        construct(required - 1),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded {
            required: observed,
            maximum,
        }) if observed == required && maximum == required - 1
    ));
}

fn provenance_with_citation(slide: SlideId, citation: &str) -> MultiscaleEmbeddingProvenance {
    let model = MultiscaleDirectPatchModelProvenance::new(
        "patch_encoder",
        "1.2.0",
        "vit_h",
        artifact(b"scratch-checkpoint"),
        ContentDigest::from_bytes(b"scratch-checkpoint-content"),
        artifact(b"scratch-snapshot"),
        artifact(b"scratch-license"),
        "Apache-2.0",
        citation,
        "encoder.layer_32",
        32,
        BUDGET,
    )
    .expect("scratch model");
    MultiscaleEmbeddingProvenance::direct_patch(
        slide.clone(),
        &provenance_patch_support(&slide),
        1_024,
        "mean_patch_tokens",
        model,
        provenance_execution(),
        artifact(b"scratch-preprocessing"),
        MultiscaleDirectPatchInputArtifacts::new(
            artifact(b"scratch-normalization"),
            artifact(b"scratch-source-entities"),
            artifact(b"scratch-source-vectors"),
            artifact(b"scratch-expected-patches"),
            artifact(b"scratch-identity-map"),
            artifact(b"scratch-source-row-link"),
            artifact(b"scratch-patch-support"),
        ),
        BUDGET,
    )
    .expect("scratch provenance")
}

fn decoded_requirement(bytes: &[u8]) -> usize {
    match MultiscaleEmbeddingProvenance::from_canonical_json(bytes, bytes.len(), 0, BUDGET) {
        Err(MultiscaleEmbeddingError::DecodedByteBudgetExceeded {
            required,
            maximum: 0,
        }) => required,
        other => panic!("unexpected decoded probe: {other:?}"),
    }
}

#[test]
fn provenance_component_constructors_charge_exact_input_capacities() {
    let model_strings = model_text();
    let model_required = size_of::<MultiscaleDirectPatchModelProvenance>()
        + model_strings.iter().map(String::capacity).sum::<usize>();
    assert_retained_edge(model_required, budget_model);

    let execution_strings = execution_text();
    let execution_required = size_of::<MultiscaleEmbeddingExecutionProvenance>()
        + execution_strings
            .iter()
            .map(String::capacity)
            .sum::<usize>();
    assert_retained_edge(execution_required, budget_execution);
}

#[test]
fn escaped_provenance_decode_charges_persistent_json_scratch_exactly() {
    let plain_citation = "c".repeat(4_096);
    let plain = provenance_with_citation(
        SlideId::new("scratch-decoder-slide").expect("plain slide"),
        &plain_citation,
    );
    let plain_bytes = plain.to_canonical_json().expect("plain JSON");
    let plain_required = decoded_requirement(&plain_bytes);

    let escaped_citation = format!("{}\"c", "c".repeat(4_094));
    assert_eq!(escaped_citation.len(), 4_096);
    let escaped = provenance_with_citation(
        SlideId::new("scratch-\"decoder\\slide").expect("escaped slide"),
        &escaped_citation,
    );
    let escaped_bytes = escaped.to_canonical_json().expect("escaped JSON");
    let escaped_required = decoded_requirement(&escaped_bytes);
    let citation_raw_bytes = escaped_citation.len() + 1;
    let citation_scratch_capacity_bound = 2 * citation_raw_bytes;
    assert_eq!(
        escaped_required - plain_required,
        escaped_bytes.len() - plain_bytes.len() + citation_scratch_capacity_bound
    );
    assert_eq!(
        MultiscaleEmbeddingProvenance::from_canonical_json(
            &escaped_bytes,
            escaped_bytes.len(),
            escaped_required,
            BUDGET,
        )
        .expect("exact scratch-aware decoded budget"),
        escaped
    );
    assert!(matches!(
        MultiscaleEmbeddingProvenance::from_canonical_json(
            &escaped_bytes,
            escaped_bytes.len(),
            escaped_required - 1,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::DecodedByteBudgetExceeded { required, maximum })
            if required == escaped_required && maximum == escaped_required - 1
    ));
}

#[test]
fn every_provenance_constructor_enforces_its_exact_retained_boundary() {
    let slide = SlideId::new("budget-provenance-slide").expect("slide");
    let patch_support = provenance_patch_support(&slide);
    let pooling_capacity = text_with_capacity(96, "mean_patch_tokens").capacity();
    let direct_text_bytes = pooling_capacity
        + [
            "patch_encoder",
            "1.2.0",
            "vit_h",
            "Apache-2.0",
            "doi:10.1000/marklab-patch-encoder",
            "encoder.layer_32",
            "marklab_patch_converter",
            "2.1.0",
        ]
        .into_iter()
        .map(str::len)
        .sum::<usize>();
    let direct_required =
        size_of::<MultiscaleEmbeddingProvenance>() + slide.as_str().len() + direct_text_bytes;
    assert_retained_edge(direct_required, |budget| {
        MultiscaleEmbeddingProvenance::direct_patch(
            slide.clone(),
            &patch_support,
            1_024,
            text_with_capacity(96, "mean_patch_tokens"),
            provenance_model(),
            provenance_execution(),
            artifact(b"budget-preprocessing"),
            direct_inputs(),
            budget,
        )
    });

    let derived_text_bytes = "marklab_patch_converter".len() + "2.1.0".len();
    let derived_required =
        size_of::<MultiscaleEmbeddingProvenance>() + slide.as_str().len() + derived_text_bytes;
    let weighted = MultiscaleEmbeddingDerivationContract::weighted_mean("fractions.v1", BUDGET)
        .expect("weighted derivation");
    let region_support = MultiscaleEmbeddingSupport::region_from_patches(
        slide.clone(),
        binding(b"budget-patch-support-binding"),
        binding(b"budget-patch-region-link-binding"),
        BUDGET,
    )
    .expect("region support");
    assert_retained_edge(derived_required, |budget| {
        MultiscaleEmbeddingProvenance::derived_region(
            slide.clone(),
            &region_support,
            &weighted,
            1_024,
            provenance_execution(),
            artifact(b"budget-source-patch-table"),
            artifact(b"budget-patch-region-link"),
            artifact(b"budget-expected-regions"),
            artifact(b"budget-region-support"),
            artifact(b"budget-weighted-derivation"),
            budget,
        )
    });

    let arithmetic =
        MultiscaleEmbeddingDerivationContract::arithmetic_mean("stable_order.v1", BUDGET)
            .expect("arithmetic derivation");
    let slide_patch_support = MultiscaleEmbeddingSupport::slide_from_patches(
        slide.clone(),
        binding(b"budget-patch-support-binding"),
        binding(b"budget-patch-table-binding"),
        BUDGET,
    )
    .expect("slide patch support");
    assert_retained_edge(derived_required, |budget| {
        MultiscaleEmbeddingProvenance::derived_slide_from_patches(
            slide.clone(),
            &slide_patch_support,
            &arithmetic,
            1_024,
            provenance_execution(),
            artifact(b"budget-source-patch-table"),
            artifact(b"budget-expected-slides"),
            artifact(b"budget-slide-patch-support"),
            artifact(b"budget-arithmetic-derivation"),
            budget,
        )
    });

    let slide_region_support = MultiscaleEmbeddingSupport::slide_from_regions(
        slide.clone(),
        binding(b"budget-region-support-binding"),
        binding(b"budget-region-table-binding"),
        BUDGET,
    )
    .expect("slide region support");
    assert_retained_edge(derived_required, |budget| {
        MultiscaleEmbeddingProvenance::derived_slide_from_regions(
            slide.clone(),
            &slide_region_support,
            &arithmetic,
            1_024,
            provenance_execution(),
            artifact(b"budget-source-region-table"),
            artifact(b"budget-expected-slides"),
            artifact(b"budget-slide-region-support"),
            artifact(b"budget-arithmetic-derivation"),
            budget,
        )
    });
}

#[test]
fn provenance_debug_output_does_not_disclose_declared_values() {
    let model = provenance_model();
    let execution = provenance_execution();
    let inputs = direct_inputs();
    let provenance = direct_patch_provenance();
    for (debug, private_value) in [
        (format!("{model:?}"), "patch_encoder"),
        (format!("{execution:?}"), "marklab_patch_converter"),
        (
            format!("{inputs:?}"),
            &artifact(b"source-vectors").to_string(),
        ),
        (format!("{provenance:?}"), "provenance-slide"),
    ] {
        assert!(!debug.contains(private_value));
    }
}
