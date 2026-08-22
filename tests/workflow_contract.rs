use std::fs;

#[test]
fn criterion_benchmarks_cover_required_spec_workloads() {
    let manifest = fs::read_to_string("Cargo.toml").expect("Cargo manifest");
    let bench_sources = fs::read_dir("benches")
        .expect("bench directory")
        .map(|entry| {
            let path = entry.expect("bench entry").path();
            fs::read_to_string(path).expect("bench source")
        })
        .collect::<Vec<_>>()
        .join("\n");

    for target in [
        "structure_factor",
        "permutation_engine",
        "periodogram",
        "multiscale_residual",
        "random_labeling_envelope",
        "pattern_load",
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
}

#[test]
fn ci_workflow_runs_locked_rust_wsi_and_benchmark_gates() {
    let workflow = fs::read_to_string(".github/workflows/ci.yml").expect("ci workflow");
    let scheduled_benchmarks = fs::read_to_string(".github/workflows/benchmarks.yml")
        .expect("scheduled benchmark workflow");

    for required in [
        "cargo fmt --check",
        "cargo clippy --locked --all-targets --all-features -- -D warnings",
        "cargo nextest run --locked --all-features",
        "cargo test --locked --doc --all-features",
        "cargo check --locked --no-default-features",
        "cargo test --locked --features wsi,cli --test wsi_integration",
        "MARKLAB_BENCH_PROFILE=smoke cargo bench --locked --all-features -- --quick",
        "cargo test --locked --no-default-features --features dhat-heap --lib dhat_ -- --test-threads=1",
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
            .contains("MARKLAB_BENCH_PROFILE=full cargo bench --locked --all-features"),
        "scheduled workflow should execute the full declared benchmark profile"
    );
    assert!(
        scheduled_benchmarks.contains("schedule:"),
        "full benchmark workflow should be scheduled as well as manually runnable"
    );
    assert!(scheduled_benchmarks.contains("benchmark-resources.txt"));

    assert!(!workflow.contains("maturin"));
    assert!(!workflow.contains("python/tests"));
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
        "args: --release --locked --features wsi --bin marklab",
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
        "public_aperio_jp2k_region_matches_openslide_oracle",
        "--ignored --exact",
    ] {
        assert!(
            workflow.contains(required),
            "public WSI workflow should include {required}"
        );
    }
}
