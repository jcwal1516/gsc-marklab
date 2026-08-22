use std::{fs, path::Path, process::Command};

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

#[test]
fn workspace_preserves_root_compatibility_and_standalone_fuzz_boundary() {
    let root = parse_manifest("Cargo.toml");
    let workspace = table(&root, "workspace", "Cargo.toml");
    assert_eq!(
        workspace.get("resolver").and_then(toml::Value::as_str),
        Some("2")
    );

    assert!(
        workspace.get("members").is_none(),
        "the root package must remain an implicit member so the nested fuzz exclusion works"
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
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse cargo metadata JSON");
    let workspace_members = metadata["workspace_members"]
        .as_array()
        .expect("metadata workspace_members");
    let default_members = metadata["workspace_default_members"]
        .as_array()
        .expect("metadata workspace_default_members");
    assert_eq!(workspace_members.len(), 1);
    assert_eq!(default_members, workspace_members);

    let root_manifest = fs::canonicalize("Cargo.toml").expect("canonical root manifest");
    let packages = metadata["packages"].as_array().expect("metadata packages");
    assert_eq!(packages.len(), 1);
    let package = &packages[0];
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
