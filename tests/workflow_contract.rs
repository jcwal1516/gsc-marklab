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
        "wavelet",
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
        "bench_marked_analysis_wavelet_grid1024",
        "bench_marked_analysis_erl_b999",
        "bench_pattern_csv_load_1m_cells",
    ] {
        assert!(
            bench_sources.contains(workload),
            "benches should include {workload}"
        );
    }
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
        "validation.json",
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
