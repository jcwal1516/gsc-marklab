use super::{provenance_digest::*, support::*};

fn assert_dependencies(
    provenance: &MultiscaleEmbeddingProvenance,
    expected: impl IntoIterator<Item = ArtifactId>,
) {
    assert_eq!(
        provenance.direct_dependencies().collect::<Vec<_>>(),
        sorted_artifacts(expected)
    );
}

pub(super) fn provenance_model() -> MultiscaleDirectPatchModelProvenance {
    MultiscaleDirectPatchModelProvenance::new(
        "patch_encoder",
        "1.2.0",
        "vit_h",
        artifact(b"checkpoint"),
        ContentDigest::from_bytes(b"checkpoint-content"),
        artifact(b"source-snapshot"),
        artifact(b"license"),
        "Apache-2.0",
        "doi:10.1000/marklab-patch-encoder",
        "encoder.layer_32",
        32,
        BUDGET,
    )
    .expect("direct model")
}

pub(super) fn provenance_execution() -> MultiscaleEmbeddingExecutionProvenance {
    MultiscaleEmbeddingExecutionProvenance::new(
        artifact(b"run-config"),
        artifact(b"environment"),
        artifact(b"converter"),
        "marklab_patch_converter",
        "2.1.0",
        BUDGET,
    )
    .expect("execution")
}

pub(super) fn direct_inputs() -> MultiscaleDirectPatchInputArtifacts {
    MultiscaleDirectPatchInputArtifacts::new(
        artifact(b"input-normalization"),
        artifact(b"source-entities"),
        artifact(b"source-vectors"),
        artifact(b"expected-patches"),
        artifact(b"identity-map"),
        artifact(b"source-row-link"),
        artifact(b"patch-support"),
    )
}

pub(super) fn provenance_patch_support(slide: &SlideId) -> MultiscaleEmbeddingSupport {
    MultiscaleEmbeddingSupport::patch(
        slide.clone(),
        binding(b"patch-context"),
        binding(b"patch-footprints"),
        binding(b"patch-overlap"),
        BUDGET,
    )
    .expect("patch support")
}

pub(super) fn direct_patch_provenance() -> MultiscaleEmbeddingProvenance {
    let slide = SlideId::new("provenance-slide").expect("slide");
    MultiscaleEmbeddingProvenance::direct_patch(
        slide.clone(),
        &provenance_patch_support(&slide),
        1_024,
        "mean_patch_tokens",
        provenance_model(),
        provenance_execution(),
        artifact(b"preprocessing"),
        direct_inputs(),
        BUDGET,
    )
    .expect("direct patch provenance")
}

pub(super) fn derived_region_provenance() -> MultiscaleEmbeddingProvenance {
    let slide = SlideId::new("provenance-slide").expect("slide");
    let derivation = MultiscaleEmbeddingDerivationContract::weighted_mean("fractions.v1", BUDGET)
        .expect("weighted derivation");
    let support = MultiscaleEmbeddingSupport::region_from_patches(
        slide.clone(),
        binding(b"patch-support-binding"),
        binding(b"patch-region-link-binding"),
        BUDGET,
    )
    .expect("region support");
    MultiscaleEmbeddingProvenance::derived_region(
        slide,
        &support,
        &derivation,
        1_024,
        provenance_execution(),
        artifact(b"source-patch-table"),
        artifact(b"patch-region-link"),
        artifact(b"expected-regions"),
        artifact(b"region-support"),
        artifact(b"weighted-derivation"),
        BUDGET,
    )
    .expect("derived region")
}

pub(super) fn slide_from_patches_provenance() -> MultiscaleEmbeddingProvenance {
    let slide = SlideId::new("provenance-slide").expect("slide");
    let derivation =
        MultiscaleEmbeddingDerivationContract::arithmetic_mean("stable_order.v1", BUDGET)
            .expect("arithmetic derivation");
    let support = MultiscaleEmbeddingSupport::slide_from_patches(
        slide.clone(),
        binding(b"patch-support-binding"),
        binding(b"source-patch-table-binding"),
        BUDGET,
    )
    .expect("slide patch support");
    MultiscaleEmbeddingProvenance::derived_slide_from_patches(
        slide,
        &support,
        &derivation,
        1_024,
        provenance_execution(),
        artifact(b"source-patch-table"),
        artifact(b"expected-slides"),
        artifact(b"slide-patch-support"),
        artifact(b"arithmetic-derivation"),
        BUDGET,
    )
    .expect("slide from patches")
}

pub(super) fn slide_from_regions_provenance() -> MultiscaleEmbeddingProvenance {
    let slide = SlideId::new("provenance-slide").expect("slide");
    let derivation =
        MultiscaleEmbeddingDerivationContract::arithmetic_mean("stable_order.v1", BUDGET)
            .expect("arithmetic derivation");
    let support = MultiscaleEmbeddingSupport::slide_from_regions(
        slide.clone(),
        binding(b"region-support-binding"),
        binding(b"source-region-table-binding"),
        BUDGET,
    )
    .expect("slide region support");
    MultiscaleEmbeddingProvenance::derived_slide_from_regions(
        slide,
        &support,
        &derivation,
        1_024,
        provenance_execution(),
        artifact(b"source-region-table"),
        artifact(b"expected-slides"),
        artifact(b"slide-region-support"),
        artifact(b"arithmetic-derivation"),
        BUDGET,
    )
    .expect("slide from regions")
}

struct DerivedWireCase {
    variant: &'static str,
    entity_kind: &'static str,
    source_key: &'static str,
    source_label: &'static [u8],
    link_label: Option<&'static [u8]>,
    expected_key: &'static str,
    expected_label: &'static [u8],
    support_key: &'static str,
    support_label: &'static [u8],
    derivation_label: &'static [u8],
    aggregation: &'static str,
}

fn expected_derived_json(case: &DerivedWireCase) -> String {
    let mut json = format!(
        concat!(
            "{{\"format\":\"marklab.multiscale_embedding_provenance\",\"version\":1,",
            "\"variant\":\"{variant}\",\"entity_kind\":\"{entity_kind}\",",
            "\"owning_slide_id\":\"provenance-slide\",\"output_dimension\":1024,",
            "\"dtype\":\"f32\",\"pooling_or_aggregation\":\"{aggregation}\",",
            "\"run_config_artifact_id\":\"{run_config}\",",
            "\"environment_artifact_id\":\"{environment}\",",
            "\"converter_artifact_id\":\"{converter}\",",
            "\"converter_name\":\"marklab_patch_converter\",",
            "\"converter_version\":\"2.1.0\",\"{source_key}\":\"{source_artifact_id}\""
        ),
        variant = case.variant,
        entity_kind = case.entity_kind,
        aggregation = case.aggregation,
        source_key = case.source_key,
        source_artifact_id = artifact(case.source_label),
        run_config = artifact(b"run-config"),
        environment = artifact(b"environment"),
        converter = artifact(b"converter"),
    );
    if let Some(link_label) = case.link_label {
        json.push_str(&format!(
            ",\"patch_region_link_artifact_id\":\"{}\",",
            artifact(link_label)
        ));
    } else {
        json.push(',');
    }
    json.push_str(&format!(
        concat!(
            "\"{expected_key}\":\"{expected_artifact_id}\",",
            "\"{support_key}\":\"{support_artifact_id}\",",
            "\"derivation_contract_artifact_id\":\"{derivation_artifact_id}\"}}\n"
        ),
        expected_key = case.expected_key,
        expected_artifact_id = artifact(case.expected_label),
        support_key = case.support_key,
        support_artifact_id = artifact(case.support_label),
        derivation_artifact_id = artifact(case.derivation_label),
    ));
    json
}

#[test]
fn all_four_provenance_variants_have_exact_canonical_wires_and_dependencies() {
    let direct = direct_patch_provenance();
    assert_eq!(
        direct.variant(),
        MultiscaleEmbeddingProvenanceVariant::DirectPatch
    );
    assert_eq!(direct.entity_kind(), EmbeddingEntityKind::Patch);
    assert_eq!(direct.output_dimension(), 1_024);
    assert_eq!(direct.pooling_or_aggregation(), "mean_patch_tokens");
    let direct_bytes = direct.to_canonical_json().expect("direct JSON");
    let expected_direct = format!(
        concat!(
            "{{\"format\":\"marklab.multiscale_embedding_provenance\",\"version\":1,",
            "\"variant\":\"direct_patch\",\"entity_kind\":\"patch\",",
            "\"owning_slide_id\":\"provenance-slide\",\"output_dimension\":1024,",
            "\"dtype\":\"f32\",\"pooling_or_aggregation\":\"mean_patch_tokens\",",
            "\"model_family\":\"patch_encoder\",\"model_version\":\"1.2.0\",",
            "\"encoder_architecture\":\"vit_h\",\"checkpoint_artifact_id\":\"{}\",",
            "\"checkpoint_content_sha256\":\"{}\",\"source_snapshot_artifact_id\":\"{}\",",
            "\"license_record_artifact_id\":\"{}\",\"license_spdx\":\"Apache-2.0\",",
            "\"citation\":\"doi:10.1000/marklab-patch-encoder\",",
            "\"extraction_tensor\":\"encoder.layer_32\",\"extraction_layer\":32,",
            "\"input_normalization_artifact_id\":\"{}\",",
            "\"preprocessing_artifact_id\":\"{}\",\"run_config_artifact_id\":\"{}\",",
            "\"environment_artifact_id\":\"{}\",\"converter_artifact_id\":\"{}\",",
            "\"converter_name\":\"marklab_patch_converter\",",
            "\"converter_version\":\"2.1.0\",\"source_entities_artifact_id\":\"{}\",",
            "\"source_vectors_artifact_id\":\"{}\",\"expected_patches_artifact_id\":\"{}\",",
            "\"identity_map_artifact_id\":\"{}\",\"source_row_link_artifact_id\":\"{}\",",
            "\"patch_support_artifact_id\":\"{}\"}}\n"
        ),
        artifact(b"checkpoint"),
        ContentDigest::from_bytes(b"checkpoint-content"),
        artifact(b"source-snapshot"),
        artifact(b"license"),
        artifact(b"input-normalization"),
        artifact(b"preprocessing"),
        artifact(b"run-config"),
        artifact(b"environment"),
        artifact(b"converter"),
        artifact(b"source-entities"),
        artifact(b"source-vectors"),
        artifact(b"expected-patches"),
        artifact(b"identity-map"),
        artifact(b"source-row-link"),
        artifact(b"patch-support"),
    );
    assert_eq!(
        String::from_utf8(direct_bytes.clone()).expect("UTF-8"),
        expected_direct
    );
    assert_dependencies(
        &direct,
        [
            artifact(b"checkpoint"),
            artifact(b"source-snapshot"),
            artifact(b"license"),
            artifact(b"input-normalization"),
            artifact(b"preprocessing"),
            artifact(b"run-config"),
            artifact(b"environment"),
            artifact(b"converter"),
            artifact(b"source-entities"),
            artifact(b"source-vectors"),
            artifact(b"expected-patches"),
            artifact(b"identity-map"),
            artifact(b"source-row-link"),
            artifact(b"patch-support"),
        ],
    );
    assert_eq!(direct.logical_digest(), direct_digest_oracle());
    assert_eq!(
        MultiscaleEmbeddingProvenance::from_canonical_json(
            &direct_bytes,
            direct_bytes.len(),
            BUDGET,
            BUDGET,
        )
        .expect("direct round trip"),
        direct
    );

    let region = derived_region_provenance();
    let region_bytes = region.to_canonical_json().expect("region JSON");
    assert_eq!(
        String::from_utf8(region_bytes.clone()).expect("UTF-8"),
        expected_derived_json(&DerivedWireCase {
            variant: "derived_region",
            entity_kind: "region",
            source_key: "source_patch_table_artifact_id",
            source_label: b"source-patch-table",
            link_label: Some(b"patch-region-link"),
            expected_key: "expected_regions_artifact_id",
            expected_label: b"expected-regions",
            support_key: "region_support_artifact_id",
            support_label: b"region-support",
            derivation_label: b"weighted-derivation",
            aggregation: "weighted_mean",
        })
    );
    assert_dependencies(
        &region,
        [
            artifact(b"run-config"),
            artifact(b"environment"),
            artifact(b"converter"),
            artifact(b"source-patch-table"),
            artifact(b"patch-region-link"),
            artifact(b"expected-regions"),
            artifact(b"region-support"),
            artifact(b"weighted-derivation"),
        ],
    );
    assert_eq!(region.logical_digest(), region_digest_oracle());

    let slide_from_patches = slide_from_patches_provenance();
    let slide_patch_bytes = slide_from_patches
        .to_canonical_json()
        .expect("slide patch JSON");
    assert_eq!(
        String::from_utf8(slide_patch_bytes.clone()).expect("UTF-8"),
        expected_derived_json(&DerivedWireCase {
            variant: "derived_slide_from_patches",
            entity_kind: "slide",
            source_key: "source_patch_table_artifact_id",
            source_label: b"source-patch-table",
            link_label: None,
            expected_key: "expected_slides_artifact_id",
            expected_label: b"expected-slides",
            support_key: "slide_support_artifact_id",
            support_label: b"slide-patch-support",
            derivation_label: b"arithmetic-derivation",
            aggregation: "arithmetic_mean",
        })
    );
    assert_dependencies(
        &slide_from_patches,
        [
            artifact(b"run-config"),
            artifact(b"environment"),
            artifact(b"converter"),
            artifact(b"source-patch-table"),
            artifact(b"expected-slides"),
            artifact(b"slide-patch-support"),
            artifact(b"arithmetic-derivation"),
        ],
    );
    assert_eq!(
        slide_from_patches.logical_digest(),
        slide_from_patches_digest_oracle()
    );

    let slide_from_regions = slide_from_regions_provenance();
    let slide_region_bytes = slide_from_regions
        .to_canonical_json()
        .expect("slide region JSON");
    assert_eq!(
        String::from_utf8(slide_region_bytes.clone()).expect("UTF-8"),
        expected_derived_json(&DerivedWireCase {
            variant: "derived_slide_from_regions",
            entity_kind: "slide",
            source_key: "source_region_table_artifact_id",
            source_label: b"source-region-table",
            link_label: None,
            expected_key: "expected_slides_artifact_id",
            expected_label: b"expected-slides",
            support_key: "slide_support_artifact_id",
            support_label: b"slide-region-support",
            derivation_label: b"arithmetic-derivation",
            aggregation: "arithmetic_mean",
        })
    );
    assert_dependencies(
        &slide_from_regions,
        [
            artifact(b"run-config"),
            artifact(b"environment"),
            artifact(b"converter"),
            artifact(b"source-region-table"),
            artifact(b"expected-slides"),
            artifact(b"slide-region-support"),
            artifact(b"arithmetic-derivation"),
        ],
    );
    assert_eq!(
        slide_from_regions.logical_digest(),
        slide_from_regions_digest_oracle()
    );

    assert_eq!(
        [
            direct.logical_digest().to_string(),
            region.logical_digest().to_string(),
            slide_from_patches.logical_digest().to_string(),
            slide_from_regions.logical_digest().to_string(),
        ],
        [
            "943c403154d9a13cfc8940dbc1e71cb02c5197a9b30639e32c5730ef3d326d42",
            "aa5bfb72b9233bf0155d47042f5ca001f0e3cd71c48b8d1bbf3a69ac148760bd",
            "59c3f11b08e375f0e6230b52f49aa61452c84e1b923b84acabf1915215c14084",
            "28608b2c6d1c176edb760ceda1fe55d179eabb39f3da8cf3d23642e1400006b2",
        ]
    );

    for provenance in [region, slide_from_patches, slide_from_regions] {
        let bytes = provenance.to_canonical_json().expect("provenance JSON");
        assert_eq!(
            MultiscaleEmbeddingProvenance::from_canonical_json(
                &bytes,
                bytes.len(),
                BUDGET,
                BUDGET,
            )
            .expect("round trip"),
            provenance
        );
    }
}
