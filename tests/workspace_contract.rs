use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
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

fn rust_sources(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut sources = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        {
            let path = entry.expect("source directory entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
                sources.push(path);
            }
        }
    }
    sources.sort();
    sources
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
        ["crates/marklab-project", "crates/marklab-workflow"],
        "the root remains implicit and only packages with immediate B-04 callers are listed"
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

    let workspace_package = workspace
        .get("package")
        .and_then(toml::Value::as_table)
        .expect("Cargo.toml must declare [workspace.package]");
    for (key, expected) in [
        ("edition", "2021"),
        ("rust-version", "1.96"),
        ("license", "MIT OR Apache-2.0"),
    ] {
        assert_eq!(
            workspace_package.get(key).and_then(toml::Value::as_str),
            Some(expected),
            "workspace.package.{key}"
        );
    }

    let root_lib = fs::read_to_string("src/lib.rs").expect("read src/lib.rs");
    assert!(
        root_lib.starts_with("#![forbid(unsafe_code)]"),
        "the compatibility crate must retain the safety boundary"
    );

    let metadata = cargo_metadata();
    let workspace_members = metadata["workspace_members"]
        .as_array()
        .expect("metadata workspace_members");
    let default_members = metadata["workspace_default_members"]
        .as_array()
        .expect("metadata workspace_default_members");
    assert_eq!(workspace_members.len(), 3);
    assert_eq!(default_members.len(), 1);

    let root_manifest = fs::canonicalize("Cargo.toml").expect("canonical root manifest");
    let packages = metadata["packages"].as_array().expect("metadata packages");
    assert_eq!(packages.len(), 3);
    let package = packages
        .iter()
        .find(|package| package["name"] == "marklab")
        .expect("root marklab package");
    assert_eq!(default_members[0], package["id"]);
    assert_eq!(package["name"].as_str(), Some("marklab"));
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
fn workspace_dependencies_descend_layers_and_core_libraries_are_not_cli_gated() {
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
        ("marklab-project", 0_u8),
        ("marklab-workflow", 1_u8),
        ("marklab", 2_u8),
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

        for target in package["targets"].as_array().expect("metadata targets") {
            let is_library = target["crate_types"]
                .as_array()
                .expect("target crate_types")
                .iter()
                .any(|crate_type| crate_type == "rlib" || crate_type == "lib");
            if !is_library {
                continue;
            }
            let source_path = target["src_path"].as_str().expect("library source path");
            let source_root = Path::new(source_path)
                .parent()
                .expect("library source directory");
            let cli_gated_files = rust_sources(source_root)
                .into_iter()
                .filter(|path| {
                    fs::read_to_string(path)
                        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
                        .contains("feature = \"cli\"")
                })
                .map(|path| {
                    path.strip_prefix(source_root)
                        .expect("source path below root")
                        .to_string_lossy()
                        .replace('\\', "/")
                })
                .collect::<HashSet<_>>();
            if name == "marklab" {
                assert_eq!(
                    cli_gated_files,
                    HashSet::from([
                        "io/mod.rs".to_string(),
                        "lib.rs".to_string(),
                        "multimodal/mod.rs".to_string(),
                        "neighborhood/mod.rs".to_string(),
                        "output/document.rs".to_string(),
                        "output/marked_artifacts.rs".to_string(),
                        "output/mod.rs".to_string(),
                        "output/writer.rs".to_string(),
                        "perf/mod.rs".to_string(),
                    ]),
                    "the compatibility library may CLI-gate only its current adapter files"
                );
            } else {
                assert!(
                    cli_gated_files.is_empty(),
                    "core library {name} must not be CLI-gated: {cli_gated_files:?}"
                );
            }
        }
    }
}
