use std::str::FromStr;

use marklab::{
    ArtifactId, ContentDigest, MeasurementStatus, MultiscaleArtifactBinding,
    MultiscaleDirectPatchInputArtifacts, MultiscaleDirectPatchModelProvenance,
    MultiscaleEmbeddingDerivationContract, MultiscaleEmbeddingExecutionProvenance,
    MultiscaleEmbeddingProvenance, MultiscaleEmbeddingSupport, SlideId,
};

const BUDGET: usize = 1 << 20;

fn artifact(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

fn binding(label: &[u8]) -> MultiscaleArtifactBinding {
    MultiscaleArtifactBinding::new(artifact(label), ContentDigest::from_bytes(label))
}

fn execution() -> MultiscaleEmbeddingExecutionProvenance {
    MultiscaleEmbeddingExecutionProvenance::new(
        artifact(b"status-run-config"),
        artifact(b"status-environment"),
        artifact(b"status-converter"),
        "status_converter",
        "1.0.0",
        BUDGET,
    )
    .expect("execution provenance")
}

fn direct_patch(slide: &SlideId) -> MultiscaleEmbeddingProvenance {
    let support = MultiscaleEmbeddingSupport::patch(
        slide.clone(),
        binding(b"status-patch-context"),
        binding(b"status-patch-footprints"),
        binding(b"status-patch-overlap"),
        BUDGET,
    )
    .expect("patch support");
    let model = MultiscaleDirectPatchModelProvenance::new(
        "patch_encoder",
        "1.0.0",
        "vit",
        artifact(b"status-checkpoint"),
        ContentDigest::from_bytes(b"status-checkpoint-content"),
        artifact(b"status-source-snapshot"),
        artifact(b"status-license"),
        "Apache-2.0",
        "doi:10.1000/status-test",
        "encoder.output",
        1,
        BUDGET,
    )
    .expect("model provenance");
    let inputs = MultiscaleDirectPatchInputArtifacts::new(
        artifact(b"status-normalization"),
        artifact(b"status-source-entities"),
        artifact(b"status-source-vectors"),
        artifact(b"status-expected-patches"),
        artifact(b"status-identity-map"),
        artifact(b"status-source-row-link"),
        artifact(b"status-direct-support"),
    );
    MultiscaleEmbeddingProvenance::direct_patch(
        slide.clone(),
        &support,
        2,
        "mean_tokens",
        model,
        execution(),
        artifact(b"status-preprocessing"),
        inputs,
        BUDGET,
    )
    .expect("direct patch provenance")
}

fn derived_region(slide: &SlideId) -> MultiscaleEmbeddingProvenance {
    let support = MultiscaleEmbeddingSupport::region_from_patches(
        slide.clone(),
        binding(b"status-region-patch-support-binding"),
        binding(b"status-region-link-binding"),
        BUDGET,
    )
    .expect("region support");
    let derivation =
        MultiscaleEmbeddingDerivationContract::weighted_mean("status_weighted.v1", BUDGET)
            .expect("weighted derivation");
    MultiscaleEmbeddingProvenance::derived_region(
        slide.clone(),
        &support,
        &derivation,
        2,
        execution(),
        artifact(b"status-source-patch-table"),
        artifact(b"status-patch-region-link"),
        artifact(b"status-expected-regions"),
        artifact(b"status-region-support"),
        artifact(b"status-weighted-derivation"),
        BUDGET,
    )
    .expect("derived region provenance")
}

fn derived_slide_from_patches(slide: &SlideId) -> MultiscaleEmbeddingProvenance {
    let support = MultiscaleEmbeddingSupport::slide_from_patches(
        slide.clone(),
        binding(b"status-slide-patch-support-binding"),
        binding(b"status-slide-source-patch-table-binding"),
        BUDGET,
    )
    .expect("slide-from-patches support");
    let derivation =
        MultiscaleEmbeddingDerivationContract::arithmetic_mean("status_order.v1", BUDGET)
            .expect("arithmetic derivation");
    MultiscaleEmbeddingProvenance::derived_slide_from_patches(
        slide.clone(),
        &support,
        &derivation,
        2,
        execution(),
        artifact(b"status-slide-source-patch-table"),
        artifact(b"status-expected-slides-patches"),
        artifact(b"status-slide-patch-support"),
        artifact(b"status-slide-patch-derivation"),
        BUDGET,
    )
    .expect("derived slide-from-patches provenance")
}

fn derived_slide_from_regions(slide: &SlideId) -> MultiscaleEmbeddingProvenance {
    let support = MultiscaleEmbeddingSupport::slide_from_regions(
        slide.clone(),
        binding(b"status-slide-region-support-binding"),
        binding(b"status-slide-source-region-table-binding"),
        BUDGET,
    )
    .expect("slide-from-regions support");
    let derivation =
        MultiscaleEmbeddingDerivationContract::arithmetic_mean("status_order.v1", BUDGET)
            .expect("arithmetic derivation");
    MultiscaleEmbeddingProvenance::derived_slide_from_regions(
        slide.clone(),
        &support,
        &derivation,
        2,
        execution(),
        artifact(b"status-slide-source-region-table"),
        artifact(b"status-expected-slides-regions"),
        artifact(b"status-slide-region-support"),
        artifact(b"status-slide-region-derivation"),
        BUDGET,
    )
    .expect("derived slide-from-regions provenance")
}

#[test]
fn multiscale_provenance_has_one_exact_measurement_status() {
    let slide = SlideId::new("measurement-status-slide").expect("slide ID");

    assert_eq!(
        direct_patch(&slide).measurement_status(),
        MeasurementStatus::MorphologyPrediction
    );
    for provenance in [
        derived_region(&slide),
        derived_slide_from_patches(&slide),
        derived_slide_from_regions(&slide),
    ] {
        assert_eq!(
            provenance.measurement_status(),
            MeasurementStatus::DerivedSummary
        );
    }

    assert_ne!(
        MeasurementStatus::Measured,
        MeasurementStatus::ImportedPrediction
    );
}
