use std::{ffi::OsStr, fs, path::Path};

#[test]
fn criterion_benchmarks_cover_required_spec_workloads() {
    let manifest = fs::read_to_string("Cargo.toml").expect("Cargo manifest");
    let bench_sources = rust_sources_below(Path::new("benches"));

    for target in [
        "structure_factor",
        "permutation_engine",
        "periodogram",
        "multiscale_residual",
        "random_labeling_envelope",
        "pattern_load",
        "cohort_hierarchy",
        "cell_embeddings",
        "patch_embeddings",
    ] {
        assert!(
            manifest.contains(&format!("name = \"{target}\"")),
            "Cargo.toml should declare bench target {target}"
        );
    }

    for workload in [
        "bench_marked_analysis_structure_factor_n10k_k1k",
        "bench_marked_analysis_permutations_n10k_k1k_b999",
        "bench_marked_analysis_periodogram_grid1024",
        "bench_marked_analysis_multiscale_residual_grid1024",
        "bench_marked_analysis_erl_b999",
        "bench_pattern_csv_load_1m_cells",
        "bench_cohort_hierarchy_1m_cells_10k_specimens",
        "bench_cell_embeddings",
        "bench_patch_embeddings",
    ] {
        assert!(
            bench_sources.contains(workload),
            "benches should include {workload}"
        );
    }

    let pattern_load = fs::read_to_string("benches/pattern_load.rs").expect("pattern benchmark");
    assert!(pattern_load.contains("BufWriter"));
    assert!(pattern_load.contains("pattern_csv_decode_filter"));
    assert!(pattern_load.contains("pattern_nearest_neighbor"));
    assert!(!pattern_load.contains("String::with_capacity"));

    let patch_embeddings =
        fs::read_to_string("benches/support/patch_embeddings.rs").expect("patch benchmark");
    for required in [
        "10k_x_1024_100k_links",
        "100k_x_1024_1m_links",
        "marklab-patch-embedding-benchmark-numerics-v1",
        "marklab-patch-embedding-benchmark-links-v1",
        "derive_contained_shared",
        "publish_patch_embedding_table_arrow",
        "publish_patch_embedding_table_parquet",
        "publish_cell_patch_assignment_table_arrow",
        "publish_cell_patch_edge_table_parquet",
    ] {
        assert!(
            patch_embeddings.contains(required),
            "patch benchmark should include {required}"
        );
    }
}

fn rust_sources_below(root: &Path) -> String {
    let mut pending = vec![root.to_owned()];
    let mut sources = Vec::new();
    while let Some(directory) = pending.pop() {
        let mut entries = fs::read_dir(&directory)
            .expect("Rust source directory")
            .map(|entry| entry.expect("Rust source entry"))
            .collect::<Vec<_>>();
        entries.sort_unstable_by_key(|entry| entry.path());
        for entry in entries {
            let path = entry.path();
            let file_type = entry.file_type().expect("Rust source file type");
            if file_type.is_dir() {
                pending.push(path);
            } else if file_type.is_file() && path.extension() == Some(OsStr::new("rs")) {
                let source = fs::read_to_string(&path).expect("Rust source");
                sources.push((path, source));
            }
        }
    }
    sources.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    sources
        .into_iter()
        .map(|(_, source)| source)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn ci_workflow_runs_locked_rust_wsi_and_benchmark_gates() {
    let workflow = fs::read_to_string(".github/workflows/ci.yml").expect("ci workflow");
    let scheduled_benchmarks = fs::read_to_string(".github/workflows/benchmarks.yml")
        .expect("scheduled benchmark workflow");

    for required in [
        "cargo fmt --all --check",
        "cargo clippy --locked --workspace --all-targets --all-features -- -D warnings",
        "cargo nextest run --locked --workspace --all-features",
        "cargo test --locked --workspace --doc --all-features",
        "cargo check --locked --workspace --no-default-features",
        "cargo test --locked --package marklab --features wsi,cli --test wsi_integration",
        "MARKLAB_BENCH_PROFILE=smoke cargo bench --locked --workspace --all-features -- --quick",
        "cargo test --locked --package marklab --no-default-features --features dhat-heap --lib dhat_ -- --test-threads=1",
        "cargo +nightly fuzz check",
        "cargo audit",
        "cargo deny check advisories licenses bans sources",
        "cargo machete",
        "cargo package --locked",
        "actions/upload-artifact",
        "benchmark-resources.txt",
        "smoke.json",
    ] {
        assert!(
            workflow.contains(required),
            "ci workflow should include {required}"
        );
    }

    assert!(
        scheduled_benchmarks
            .contains("MARKLAB_BENCH_PROFILE=full cargo bench --locked --workspace --all-features"),
        "scheduled workflow should execute the full declared benchmark profile"
    );
    assert!(
        scheduled_benchmarks.contains("schedule:"),
        "full benchmark workflow should be scheduled as well as manually runnable"
    );
    assert!(scheduled_benchmarks.contains("benchmark-resources.txt"));

    assert!(!workflow.contains("maturin"));
    assert!(!workflow.contains("python/tests"));
    assert!(!workflow.contains("--replicates 1000"));
}

#[test]
fn workspace_policy_is_explicit() {
    let ci = fs::read_to_string(".github/workflows/ci.yml").expect("ci workflow");
    let benchmarks =
        fs::read_to_string(".github/workflows/benchmarks.yml").expect("benchmark workflow");
    let calibration =
        fs::read_to_string(".github/workflows/calibration.yml").expect("calibration workflow");
    let release = fs::read_to_string(".github/workflows/release.yml").expect("release workflow");
    let public_wsi =
        fs::read_to_string(".github/workflows/wsi-public.yml").expect("public WSI workflow");

    for required in [
        "cargo fmt --all --check",
        "cargo clippy --locked --workspace --all-targets --all-features -- -D warnings",
        "cargo nextest run --locked --workspace --all-features",
        "cargo test --locked --workspace --doc --all-features",
        "cargo check --locked --workspace --no-default-features",
        "cargo package --locked --workspace",
        "cargo clippy --locked --workspace --all-targets --no-default-features --features cli -- -D warnings",
        "cargo check --locked --workspace --all-targets ${{ matrix.args }}",
        "name: default",
        "args: --no-default-features",
        "args: --all-features",
        "args: --no-default-features --features csv",
        "args: --no-default-features --features parquet",
        "args: --no-default-features --features cli",
        "args: --no-default-features --features wsi",
        "args: --no-default-features --features wsi,cli",
        "cargo test --locked --package marklab --features wsi,cli --test wsi_integration",
        "cargo test --locked --package marklab --no-default-features --features dhat-heap",
        "cargo run --release --locked --package marklab --features wsi --bin marklab",
    ] {
        assert!(ci.contains(required), "CI workspace policy should include {required}");
    }

    assert!(benchmarks.contains("cargo bench --locked --workspace --all-features"));
    assert!(calibration.contains("cargo test --release --locked --workspace --all-features"));
    assert!(release.contains("--package marklab"));
    assert!(public_wsi.contains("cargo test --locked --package marklab --features wsi,cli"));
    assert!(!ci.contains("cargo xtask"));
}

#[test]
fn formal_calibration_is_scheduled_outside_pull_request_ci() {
    let workflow =
        fs::read_to_string(".github/workflows/calibration.yml").expect("calibration workflow");

    for required in [
        "schedule:",
        "workflow_dispatch:",
        "cargo test --release --locked --workspace --all-features negative_control_calibrates",
        "--ignored --nocapture --test-threads=1",
    ] {
        assert!(
            workflow.contains(required),
            "calibration workflow should include {required}"
        );
    }
}

#[test]
fn fuzz_manifest_covers_current_public_input_boundaries() {
    let manifest = fs::read_to_string("fuzz/Cargo.toml").expect("fuzz manifest");
    let sources = fs::read_dir("fuzz/fuzz_targets")
        .expect("fuzz target directory")
        .map(|entry| {
            let path = entry.expect("fuzz target entry").path();
            fs::read_to_string(path).expect("fuzz target source")
        })
        .collect::<Vec<_>>()
        .join("\n");

    for target in [
        "config",
        "geojson_mask",
        "csv_row_parser",
        "result_document",
        "wsi_region_request",
        "artifact_catalog",
        "embedding_inputs",
    ] {
        assert!(
            manifest.contains(&format!("name = \"{target}\"")),
            "fuzz manifest should declare {target}"
        );
    }
    for boundary in [
        "AnalysisConfig::from_toml_overrides",
        "TumorMask::from_geojson_str",
        "PatternLoader::new",
        "ResultDocument::from_json",
        "validate_for",
        "ArtifactCatalog::from_json",
        "PatchEmbeddingContext::from_canonical_json",
        "PatchFootprintSet::new",
        "PatchEmbeddingTable::from_rows",
        "CellPatchLink::derive_contained_shared",
        "PatchRegionLink::from_exhaustive_assessment",
        "MultiscaleEmbeddingProvenance::from_canonical_json",
        "preflight_patch_embedding_table_arrow_bytes",
        "preflight_patch_embedding_table_parquet_bytes",
    ] {
        assert!(
            sources.contains(boundary),
            "fuzz targets should cover {boundary}"
        );
    }
    assert!(!sources.contains("Pattern::from_paths"));
}

#[test]
fn release_workflow_builds_locked_wsi_archives_with_licenses_and_checksums() {
    let workflow = fs::read_to_string(".github/workflows/release.yml").expect("release workflow");

    for required in [
        "ubuntu-latest",
        "macos-latest",
        "windows-latest",
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
        "houseabsolute/actions-rust-cross@v1",
        "args: --release --locked --package marklab --features wsi --bin marklab",
        "README.md LICENSE-MIT LICENSE-APACHE",
        "sha256sum",
        "Get-FileHash",
        "actions/upload-artifact",
    ] {
        assert!(
            workflow.contains(required),
            "release workflow should include {required}"
        );
    }

    assert!(!workflow.contains("maturin"));
    assert!(!workflow.contains("wheel"));
}

#[test]
fn public_wsi_workflow_verifies_fixture_and_independent_oracle() {
    let workflow =
        fs::read_to_string(".github/workflows/wsi-public.yml").expect("public WSI workflow");

    for required in [
        "schedule:",
        "workflow_dispatch:",
        "6205ccf75a8fa6c32df7c5c04b7377398971a490fb6b320d50d91f7ba6a0e6fd",
        "openslide-write-png",
        "MARKLAB_PUBLIC_APERIO_SVS",
        "MARKLAB_PUBLIC_APERIO_ORACLE_PNG",
        "cargo test --locked --package marklab --features wsi,cli",
        "public_aperio_jp2k_region_matches_openslide_oracle",
        "--ignored --exact",
    ] {
        assert!(
            workflow.contains(required),
            "public WSI workflow should include {required}"
        );
    }
}
