use std::{fs, path::Path};

#[test]
fn curated_python_startup_honors_the_seed_and_keeps_import_isolation() {
    let directory = tempfile::tempdir().unwrap();
    let worker = directory.path().join("probe.py");
    fs::write(&worker, r#"import json, os, site, sys
print(json.dumps({"hash":hash("marklab-reproducibility"),"safe_path":sys.flags.safe_path,"no_user_site":sys.flags.no_user_site,"write_bytecode":not sys.dont_write_bytecode,"seed":os.environ.get("PYTHONHASHSEED"),"pythonpath":os.environ.get("PYTHONPATH"),"path":sys.path,"user_site":site.getusersitepackages()}))
"#).unwrap();
    let interpreter =
        marklab::python_backend_interpreter(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut outputs = Vec::new();
    for _ in 0..2 {
        let output = marklab::python_backend_command(&interpreter, &worker)
            .current_dir(directory.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["safe_path"], true);
        assert_eq!(value["no_user_site"], 1);
        assert_eq!(value["write_bytecode"], false);
        assert_eq!(value["seed"], "0");
        assert_eq!(value["pythonpath"], serde_json::Value::Null);
        for entry in value["path"].as_array().unwrap() {
            assert_ne!(entry, "");
            assert_ne!(entry.as_str().unwrap(), directory.path().to_str().unwrap());
            assert_ne!(entry, &value["user_site"]);
        }
        outputs.push(value["hash"].clone());
    }
    assert_eq!(outputs[0], outputs[1]);
}
