use std::{collections::HashSet, fs};

#[test]
fn criterion_benchmarks_are_declared_without_the_default_test_harness() {
    let manifest: toml::Value =
        toml::from_str(&fs::read_to_string("Cargo.toml").expect("Cargo manifest"))
            .expect("parse Cargo manifest");
    let benches = manifest["bench"].as_array().expect("benchmark targets");
    let names = benches
        .iter()
        .map(|bench| {
            assert_eq!(bench["harness"].as_bool(), Some(false));
            bench["name"].as_str().expect("benchmark name")
        })
        .collect::<HashSet<_>>();
    for target in [
        "structure_factor",
        "permutation_engine",
        "periodogram",
        "multiscale_residual",
        "random_labeling_envelope",
        "pattern_load",
        "spatial_stages",
        "cohort_hierarchy",
        "cell_embeddings",
        "patch_embeddings",
    ] {
        assert!(names.contains(target), "missing benchmark target {target}");
    }
}

#[test]
fn fuzz_manifest_declares_the_public_input_targets_outside_test_and_doc_harnesses() {
    let manifest: toml::Value =
        toml::from_str(&fs::read_to_string("fuzz/Cargo.toml").expect("fuzz manifest"))
            .expect("parse fuzz manifest");
    assert_eq!(
        manifest["package"]["metadata"]["cargo-fuzz"].as_bool(),
        Some(true)
    );
    let targets = manifest["bin"].as_array().expect("fuzz targets");
    let names = targets
        .iter()
        .map(|target| {
            for harness in ["test", "doc", "bench"] {
                assert_eq!(target[harness].as_bool(), Some(false));
            }
            target["name"].as_str().expect("fuzz target name")
        })
        .collect::<HashSet<_>>();
    for target in [
        "config",
        "geojson_mask",
        "csv_row_parser",
        "result_document",
        "wsi_region_request",
        "artifact_catalog",
        "embedding_inputs",
    ] {
        assert!(names.contains(target), "missing fuzz target {target}");
    }
}
