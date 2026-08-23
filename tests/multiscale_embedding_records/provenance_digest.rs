use super::support::*;

const PROVENANCE_DOMAIN: &[u8] = b"marklab-multiscale-embedding-provenance-logical-v1";

struct DigestOracle(Vec<u8>);

impl DigestOracle {
    fn new() -> Self {
        let mut oracle = Self(Vec::new());
        oracle.bytes(PROVENANCE_DOMAIN);
        oracle
    }

    fn bytes(&mut self, value: &[u8]) {
        self.0
            .extend_from_slice(&(value.len() as u128).to_be_bytes());
        self.0.extend_from_slice(value);
    }

    fn text(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_be_bytes());
    }

    fn artifact(&mut self, value: ArtifactId) {
        self.bytes(value.digest().as_bytes());
    }

    fn digest(&mut self, value: ContentDigest) {
        self.bytes(value.as_bytes());
    }

    fn finish(self) -> ContentDigest {
        ContentDigest::from_bytes(&self.0)
    }
}

fn common_digest_oracle(variant: &str, entity_kind: &str, aggregation: &str) -> DigestOracle {
    let mut oracle = DigestOracle::new();
    oracle.text("marklab.multiscale_embedding_provenance");
    oracle.u32(1);
    oracle.text(variant);
    oracle.text(entity_kind);
    oracle.text("provenance-slide");
    oracle.u32(1_024);
    oracle.text("f32");
    oracle.text(aggregation);
    oracle
}

pub(super) fn direct_digest_oracle() -> ContentDigest {
    let mut oracle = common_digest_oracle("direct_patch", "patch", "mean_patch_tokens");
    oracle.text("patch_encoder");
    oracle.text("1.2.0");
    oracle.text("vit_h");
    oracle.artifact(artifact(b"checkpoint"));
    oracle.digest(ContentDigest::from_bytes(b"checkpoint-content"));
    oracle.artifact(artifact(b"source-snapshot"));
    oracle.artifact(artifact(b"license"));
    oracle.text("Apache-2.0");
    oracle.text("doi:10.1000/marklab-patch-encoder");
    oracle.text("encoder.layer_32");
    oracle.u32(32);
    oracle.artifact(artifact(b"input-normalization"));
    oracle.artifact(artifact(b"preprocessing"));
    oracle.artifact(artifact(b"run-config"));
    oracle.artifact(artifact(b"environment"));
    oracle.artifact(artifact(b"converter"));
    oracle.text("marklab_patch_converter");
    oracle.text("2.1.0");
    oracle.artifact(artifact(b"source-entities"));
    oracle.artifact(artifact(b"source-vectors"));
    oracle.artifact(artifact(b"expected-patches"));
    oracle.artifact(artifact(b"identity-map"));
    oracle.artifact(artifact(b"source-row-link"));
    oracle.artifact(artifact(b"patch-support"));
    oracle.finish()
}

struct DerivedDigestCase {
    variant: &'static str,
    entity_kind: &'static str,
    aggregation: &'static str,
    source_label: &'static [u8],
    link_label: Option<&'static [u8]>,
    expected_label: &'static [u8],
    support_label: &'static [u8],
    derivation_label: &'static [u8],
}

fn derived_digest_oracle(case: &DerivedDigestCase) -> ContentDigest {
    let mut oracle = common_digest_oracle(case.variant, case.entity_kind, case.aggregation);
    oracle.artifact(artifact(b"run-config"));
    oracle.artifact(artifact(b"environment"));
    oracle.artifact(artifact(b"converter"));
    oracle.text("marklab_patch_converter");
    oracle.text("2.1.0");
    oracle.artifact(artifact(case.source_label));
    if let Some(label) = case.link_label {
        oracle.artifact(artifact(label));
    }
    oracle.artifact(artifact(case.expected_label));
    oracle.artifact(artifact(case.support_label));
    oracle.artifact(artifact(case.derivation_label));
    oracle.finish()
}

pub(super) fn region_digest_oracle() -> ContentDigest {
    derived_digest_oracle(&DerivedDigestCase {
        variant: "derived_region",
        entity_kind: "region",
        aggregation: "weighted_mean",
        source_label: b"source-patch-table",
        link_label: Some(b"patch-region-link"),
        expected_label: b"expected-regions",
        support_label: b"region-support",
        derivation_label: b"weighted-derivation",
    })
}

pub(super) fn slide_from_patches_digest_oracle() -> ContentDigest {
    derived_digest_oracle(&DerivedDigestCase {
        variant: "derived_slide_from_patches",
        entity_kind: "slide",
        aggregation: "arithmetic_mean",
        source_label: b"source-patch-table",
        link_label: None,
        expected_label: b"expected-slides",
        support_label: b"slide-patch-support",
        derivation_label: b"arithmetic-derivation",
    })
}

pub(super) fn slide_from_regions_digest_oracle() -> ContentDigest {
    derived_digest_oracle(&DerivedDigestCase {
        variant: "derived_slide_from_regions",
        entity_kind: "slide",
        aggregation: "arithmetic_mean",
        source_label: b"source-region-table",
        link_label: None,
        expected_label: b"expected-slides",
        support_label: b"slide-region-support",
        derivation_label: b"arithmetic-derivation",
    })
}
