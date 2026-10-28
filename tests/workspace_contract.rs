use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    process::Command,
};

fn parse_manifest(path: &str) -> toml::Value {
    let text = fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path}: {error}"));
    toml::from_str(&text).unwrap_or_else(|error| panic!("parse {path}: {error}"))
}

fn table<'a>(value: &'a toml::Value, key: &str, path: &str) -> &'a toml::value::Table {
    value
        .get(key)
        .and_then(toml::Value::as_table)
        .unwrap_or_else(|| panic!("{path} must declare [{key}]"))
}

fn cargo_metadata() -> serde_json::Value {
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
            "Cargo.toml",
        ])
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("parse cargo metadata JSON")
}

#[test]
fn workspace_preserves_root_compatibility_and_standalone_fuzz_boundary() {
    let root = parse_manifest("Cargo.toml");
    let workspace = table(&root, "workspace", "Cargo.toml");
    assert_eq!(
        workspace.get("resolver").and_then(toml::Value::as_str),
        Some("2")
    );

    let members = workspace
        .get("members")
        .and_then(toml::Value::as_array)
        .expect("workspace.members must list only immediate non-root packages")
        .iter()
        .map(|value| value.as_str().expect("workspace path must be a string"))
        .collect::<Vec<_>>();
    assert_eq!(
        members,
        [
            "crates/marklab-core",
            "crates/marklab-data",
            "crates/marklab-project",
            "crates/marklab-embeddings",
            "crates/marklab-cohort",
            "crates/marklab-bayes",
            "crates/marklab-transport",
            "crates/marklab-prediction",
            "crates/marklab-simulation",
            "crates/marklab-sbi",
            "crates/marklab-longitudinal",
            "crates/marklab-spatial3d",
            "crates/marklab-causal",
            "crates/marklab-numerics",
            "crates/marklab-policy",
            "crates/marklab-graph",
            "crates/marklab-topology",
            "crates/marklab-workflow",
            "crates/marklab-microbio",
        ],
        "the root remains implicit and only packages with immediate callers are listed"
    );
    assert!(
        workspace.get("default-members").is_none(),
        "Cargo defaults a non-virtual workspace to its root package"
    );
    let excluded = workspace
        .get("exclude")
        .and_then(toml::Value::as_array)
        .expect("workspace.exclude must be an array")
        .iter()
        .map(|value| value.as_str().expect("workspace path must be a string"))
        .collect::<Vec<_>>();
    assert_eq!(excluded, ["fuzz"]);

    let root_package = table(&root, "package", "Cargo.toml");
    assert_eq!(
        root_package.get("name").and_then(toml::Value::as_str),
        Some("marklab")
    );
    assert_eq!(
        table(&root, "lib", "Cargo.toml")
            .get("name")
            .and_then(toml::Value::as_str),
        Some("marklab")
    );
    let root_bins = root
        .get("bin")
        .and_then(toml::Value::as_array)
        .expect("Cargo.toml must retain [[bin]]");
    assert_eq!(root_bins.len(), 1);
    assert_eq!(
        root_bins[0].get("name").and_then(toml::Value::as_str),
        Some("marklab")
    );
    for manifest_path in [
        "Cargo.toml",
        "crates/marklab-core/Cargo.toml",
        "crates/marklab-data/Cargo.toml",
        "crates/marklab-project/Cargo.toml",
        "crates/marklab-embeddings/Cargo.toml",
        "crates/marklab-cohort/Cargo.toml",
        "crates/marklab-bayes/Cargo.toml",
        "crates/marklab-transport/Cargo.toml",
        "crates/marklab-prediction/Cargo.toml",
        "crates/marklab-simulation/Cargo.toml",
        "crates/marklab-sbi/Cargo.toml",
        "crates/marklab-longitudinal/Cargo.toml",
        "crates/marklab-spatial3d/Cargo.toml",
        "crates/marklab-causal/Cargo.toml",
        "crates/marklab-numerics/Cargo.toml",
        "crates/marklab-policy/Cargo.toml",
        "crates/marklab-graph/Cargo.toml",
        "crates/marklab-topology/Cargo.toml",
        "crates/marklab-workflow/Cargo.toml",
    ] {
        let manifest = parse_manifest(manifest_path);
        assert_eq!(
            table(&manifest, "lib", manifest_path)
                .get("bench")
                .and_then(toml::Value::as_bool),
            Some(false),
            "{manifest_path} must disable the default library benchmark harness so workspace Criterion flags are not forwarded to it"
        );
    }

    let workspace_package = workspace
        .get("package")
        .and_then(toml::Value::as_table)
        .expect("Cargo.toml must declare [workspace.package]");
    for (key, expected) in [
        ("edition", "2021"),
        ("rust-version", "1.99"),
        ("license", "MIT OR Apache-2.0"),
    ] {
        assert_eq!(
            workspace_package.get(key).and_then(toml::Value::as_str),
            Some(expected),
            "workspace.package.{key}"
        );
    }

    let metadata = cargo_metadata();
    let workspace_members = metadata["workspace_members"]
        .as_array()
        .expect("metadata workspace_members");
    let default_members = metadata["workspace_default_members"]
        .as_array()
        .expect("metadata workspace_default_members");
    assert_eq!(workspace_members.len(), 20);
    assert_eq!(default_members.len(), 1);

    let root_manifest = fs::canonicalize("Cargo.toml").expect("canonical root manifest");
    let packages = metadata["packages"].as_array().expect("metadata packages");
    assert_eq!(packages.len(), 20);
    let package = packages
        .iter()
        .find(|package| package["name"] == "marklab")
        .expect("root marklab package");
    assert_eq!(default_members[0], package["id"]);
    assert_eq!(package["name"].as_str(), Some("marklab"));
    let root_features = package["features"].as_object().expect("root features");
    assert!(root_features["csv"]
        .as_array()
        .expect("root csv feature")
        .iter()
        .any(|edge| edge == "marklab-embeddings/csv"));
    assert!(root_features["parquet"]
        .as_array()
        .expect("root parquet feature")
        .iter()
        .any(|edge| edge == "marklab-embeddings/parquet"));
    let embeddings = packages
        .iter()
        .find(|package| package["name"] == "marklab-embeddings")
        .expect("embedding package");
    let embedding_features = embeddings["features"]
        .as_object()
        .expect("embedding features");
    assert!(embedding_features["default"]
        .as_array()
        .is_some_and(Vec::is_empty));
    assert!(embedding_features.contains_key("csv"));
    assert!(embedding_features.contains_key("parquet"));
    assert_eq!(
        Path::new(package["manifest_path"].as_str().expect("manifest path")),
        root_manifest
    );
    let target_kinds = package["targets"]
        .as_array()
        .expect("package targets")
        .iter()
        .filter_map(|target| target["kind"].as_array())
        .flat_map(|kinds| kinds.iter().filter_map(serde_json::Value::as_str))
        .collect::<Vec<_>>();
    assert!(target_kinds.contains(&"rlib"));
    assert!(target_kinds.contains(&"bin"));
}

#[test]
fn workspace_dependencies_descend_layers_and_core_packages_have_no_cli_feature() {
    let metadata = cargo_metadata();
    let workspace_ids = metadata["workspace_members"]
        .as_array()
        .expect("metadata workspace_members")
        .iter()
        .map(|id| id.as_str().expect("workspace package ID"))
        .collect::<HashSet<_>>();
    let packages = metadata["packages"].as_array().expect("metadata packages");
    let workspace_packages = packages
        .iter()
        .filter(|package| {
            workspace_ids.contains(package["id"].as_str().expect("metadata package ID"))
        })
        .collect::<Vec<_>>();
    let workspace_names = workspace_packages
        .iter()
        .map(|package| package["name"].as_str().expect("workspace package name"))
        .collect::<HashSet<_>>();
    let layers = HashMap::from([
        ("marklab-core", 0_u8),
        ("marklab-numerics", 0_u8),
        ("marklab-cohort", 1_u8),
        ("marklab-causal", 1_u8),
        ("marklab-longitudinal", 1_u8),
        ("marklab-policy", 0_u8),
        ("marklab-simulation", 2_u8),
        ("marklab-spatial3d", 1_u8),
        ("marklab-topology", 1_u8),
        ("marklab-data", 1_u8),
        ("marklab-bayes", 2_u8),
        ("marklab-graph", 1_u8),
        ("marklab-transport", 1_u8),
        ("marklab-prediction", 1_u8),
        ("marklab-sbi", 3_u8),
        ("marklab-project", 2_u8),
        ("marklab-embeddings", 3_u8),
        ("marklab-workflow", 3_u8),
        ("marklab-microbio", 0_u8),
        ("marklab", 4_u8),
    ]);

    for package in workspace_packages {
        let name = package["name"].as_str().expect("workspace package name");
        let layer = layers
            .get(name)
            .unwrap_or_else(|| panic!("workspace package {name} needs an explicit policy layer"));

        for dependency in package["dependencies"]
            .as_array()
            .expect("metadata dependencies")
        {
            let dependency_name = dependency["name"].as_str().expect("dependency name");
            if dependency["path"].is_null() || !workspace_names.contains(dependency_name) {
                continue;
            }
            let dependency_layer = layers.get(dependency_name).unwrap_or_else(|| {
                panic!("workspace dependency {dependency_name} needs an explicit policy layer")
            });
            assert!(
                dependency_layer < layer,
                "{name} layer {layer} must not depend upward on {dependency_name} layer {dependency_layer}"
            );
        }

        let features = package["features"].as_object().expect("metadata features");
        if name != "marklab" {
            assert!(
                !features.contains_key("cli"),
                "core package {name} must not define a cli feature"
            );
        }
    }
}
