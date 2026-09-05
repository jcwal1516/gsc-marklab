#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn partial_ot_matches_the_forced_one_cell_plan() {
    let directory = tempfile::tempdir().expect("tempdir");
    let source = directory.path().join("source.csv");
    let target = directory.path().join("target.csv");
    let cost = directory.path().join("cost.csv");
    fs::write(&source, "source_id,mass\ns1,2\n").expect("source");
    fs::write(&target, "target_id,mass\nt1,3\n").expect("target");
    fs::write(&cost, "source_id,target_id,cost\ns1,t1,4\n").expect("cost");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .env("MARKLAB_PYTHON", "/nonexistent/marklab-python")
        .env("MARKLAB_RUNTIME_ROOT", "/nonexistent/marklab-runtime")
        .args([
            "bayes",
            "partial-ot",
            "--source",
            source.to_str().expect("source path"),
            "--target",
            target.to_str().expect("target path"),
            "--cost",
            cost.to_str().expect("cost path"),
            "--transported-mass",
            "1.5",
            "--epsilon",
            "0.5",
            "--timeout-seconds",
            "30",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.partial_ot");
    assert_eq!(result["version"], 2);
    assert_eq!(result["backend"]["name"], "marklab-rust");
    assert_eq!(
        result["optimizer"]["method"],
        "log_domain_partial_transport_dual"
    );
    assert!((result["transported_mass"].as_f64().unwrap() - 1.5).abs() < 1e-10);
    assert!((result["plan"][0]["mass"].as_f64().unwrap() - 1.5).abs() < 1e-10);
    assert!((result["unmatched_source_mass"][0].as_f64().unwrap() - 0.5).abs() < 1e-10);
    assert!((result["unmatched_target_mass"][0].as_f64().unwrap() - 1.5).abs() < 1e-10);
    assert!((result["transport_cost"].as_f64().unwrap() - 6.0).abs() < 1e-10);
    assert_eq!(result["constraint_status"], "feasible_within_tolerance");
}

#[test]
fn partial_native_wire_and_file_binding_preserve_exact_numbers_and_order() {
    let epsilon = 0.16392574019031287_f64;
    let cost = 0.16392574019031287_f64;
    let spec = marklab_bayes::PartialTransportSpec {
        source: vec![marklab_bayes::TransportMass {
            id: "s".into(),
            mass: 2.,
        }],
        target: vec![marklab_bayes::TransportMass {
            id: "t".into(),
            mass: 3.,
        }],
        costs_row_major: vec![cost],
        transported_mass: 1.5,
        epsilon,
        timeout_seconds: 30,
    };
    let wire = marklab::exact_float_json::encode(&spec).unwrap();
    let direct = serde_json::to_vec(&marklab_bayes::fit_partial_transport(spec).unwrap()).unwrap();
    let child = Command::cargo_bin("marklab")
        .unwrap()
        .args(["backend", "native-partial-transport"])
        .write_stdin(wire.to_vec())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(child, direct);
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.csv");
    let target = directory.path().join("target.csv");
    let costs = directory.path().join("cost.csv");
    let output = directory.path().join("result.json");
    let source_bytes = b"source_id,mass\ns,2\n";
    let target_bytes = b"target_id,mass\nt,3\n";
    let cost_bytes = format!("source_id,target_id,cost\ns,t,{cost}\n");
    fs::write(&source, source_bytes).unwrap();
    fs::write(&target, target_bytes).unwrap();
    fs::write(&costs, &cost_bytes).unwrap();
    let run = || {
        let mut command = Command::cargo_bin("marklab").unwrap();
        command
            .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
            .args(["bayes", "partial-ot", "--source"])
            .arg(&source)
            .arg("--target")
            .arg(&target)
            .arg("--cost")
            .arg(&costs)
            .args([
                "--transported-mass",
                "1.5",
                "--epsilon",
                &epsilon.to_string(),
                "--timeout-seconds",
                "30",
                "--out",
            ])
            .arg(&output);
        command
    };
    run().assert().success();
    let bytes = fs::read(&output).unwrap();
    let result: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
        serde_json::from_slice(&bytes).unwrap();
    let direct: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
        serde_json::from_slice(&direct).unwrap();
    for (key, value) in direct {
        assert_eq!(result[&key].get(), value.get(), "{key}");
    }
    for (key, expected) in [
        ("source_sha256", marklab_bayes::sha256_hex(source_bytes)),
        ("target_sha256", marklab_bayes::sha256_hex(target_bytes)),
        (
            "cost_sha256",
            marklab_bayes::sha256_hex(cost_bytes.as_bytes()),
        ),
    ] {
        assert_eq!(
            serde_json::from_str::<String>(result[key].get()).unwrap(),
            expected
        );
    }
    run()
        .assert()
        .failure()
        .stderr(predicates::str::contains("output already exists"));
    assert_eq!(fs::read(&output).unwrap(), bytes);
    for case in 0..3 {
        let mut bad: serde_json::Value = serde_json::from_slice(&wire).unwrap();
        match case {
            0 => bad["epsilon"] = serde_json::json!({"__marklab_f64_bits":f64::NAN.to_bits()}),
            1 => bad["extra"] = serde_json::json!(true),
            _ => bad["costs_row_major"] = serde_json::json!([]),
        };
        Command::cargo_bin("marklab")
            .unwrap()
            .args(["backend", "native-partial-transport"])
            .write_stdin(serde_json::to_vec(&bad).unwrap())
            .assert()
            .failure();
    }
}

#[test]
fn transport_csv_rejects_incomplete_duplicate_undeclared_and_oversized_inputs() {
    let source = b"source_id,mass\ns,2\n";
    let target = b"target_id,mass\nt,3\n";
    for cost in [
        "source_id,target_id,cost\n",
        "source_id,target_id,cost\ns,t,1\ns,t,1\n",
        "source_id,target_id,cost\ns,other,1\n",
        "source_id,target_id,cost\ns,t,NaN\n",
    ] {
        assert!(
            marklab::transport::fit_partial_csv(source, target, cost.as_bytes(), 1., 0.5, 30)
                .is_err()
        );
    }
    let oversized = vec![b'a'; 16 * 1024 * 1024 + 1];
    assert!(
        marklab::transport::fit_partial_csv(&oversized, target, b"", 1., 0.5, 30)
            .unwrap_err()
            .to_string()
            .contains("16 MiB")
    );
    let too_many = format!(
        "source_id,mass\n{}",
        (0..65).map(|i| format!("s{i},1\n")).collect::<String>()
    );
    assert!(
        marklab::transport::fit_partial_csv(too_many.as_bytes(), target, b"", 1., 0.5, 30)
            .unwrap_err()
            .to_string()
            .contains("support limit")
    );
}
