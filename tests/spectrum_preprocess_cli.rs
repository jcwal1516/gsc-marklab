#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};

/// Six linear-mode time points; with ML3 = 0, m/z = (DELAY + 2 i - ML2)^2 / 1e6, giving
/// 441.0, 441.084004, 441.168016, 441.252036, 441.336064 and 441.4201.
fn write_acquisition(dir: &Path, extra_fields: &str, counts: &[i32]) {
    fs::create_dir_all(dir).unwrap();
    let acqu = format!(
        "##TITLE= XMASS Parameter file\r\n##$AQOP_m= 0 \r\n##$BYTORDA= 0 \r\n##$DELAY= 21010 \r\n\
         ##$DW= 2 \r\n##$ML1= 1000000 \r\n##$ML2= 10 \r\n##$ML3= 0 \r\n##$NoSHOTS= 240 \r\n\
         ##$SPType= 0 \r\n##$TD= {} \r\n{extra_fields}##END= \r\n",
        counts.len()
    );
    fs::write(dir.join("acqu"), acqu).unwrap();
    let fid: Vec<u8> = counts
        .iter()
        .flat_map(|count| count.to_le_bytes())
        .collect();
    fs::write(dir.join("fid"), fid).unwrap();
}

fn run(acquisition: &Path, config: &Path, out: &Path) -> assert_cmd::assert::Assert {
    run_input("--bruker", acquisition, config, out)
}

fn run_input(flag: &str, input: &Path, config: &Path, out: &Path) -> assert_cmd::assert::Assert {
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "spectrum",
            "preprocess",
            flag,
            input.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
}

#[test]
fn preprocesses_a_bruker_acquisition_into_bins() {
    let dir = tempfile::tempdir().unwrap();
    let acquisition = dir.path().join("0_A1/1/1SLin");
    write_acquisition(&acquisition, "", &[4, 9, 16, -1, 25, 36]);
    let config = dir.path().join("config.json");
    let mut settings = json!({
        "steps": [
            {"step": "sqrt"},
            {"step": "total_ion_current"},
            {"step": "trim", "min_mz": 441.0, "max_mz": 441.3}
        ],
        "binning": {"min_mz": 441.0, "max_mz": 441.3, "bins": 3}
    });
    fs::write(&config, settings.to_string()).unwrap();
    let out = dir.path().join("result.json");
    run(&acquisition, &config, &out).success();

    let result: Value = serde_json::from_slice(&fs::read(&out).unwrap()).unwrap();
    assert_eq!(
        result["source"],
        json!({
            "format": "bruker_flex",
            "time_points": 6,
            "byte_order": "little",
            "laser_shots": 240,
            "tof_mode": "linear",
            "negative_intensities_clipped": 1
        })
    );
    let values = |key: &str| -> Vec<f64> {
        result["spectrum"][key]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_f64().unwrap())
            .collect()
    };
    let (mz, intensity) = (values("mz"), values("intensity"));
    assert_eq!(mz.len(), 4, "trim keeps the first four points");
    // The square roots 2, 3, 4, 0 share one total-ion-current divisor.
    assert!((intensity[1] / intensity[0] - 1.5).abs() < 1e-12);
    assert!((intensity[2] / intensity[0] - 2.0).abs() < 1e-12);
    assert_eq!(intensity[3], 0.0);
    let binned: Vec<f64> = result["binned"]["intensity"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_f64().unwrap())
        .collect();
    assert_eq!(binned, [intensity[0] + intensity[1], intensity[2], 0.0]);
    assert_eq!(result["binned"]["points_outside"], 0);
    // Summed bins are written without `aggregate`, as before it existed.
    let keys: Vec<&String> = result["binned"].as_object().unwrap().keys().collect();
    assert_eq!(
        keys,
        ["bins", "intensity", "max_mz", "min_mz", "points_outside"]
    );

    // Area bins: the kept spectrum starts at the first edge and ends inside the last bin, so the
    // bins together hold its whole trapezoidal area.
    settings["binning"]["aggregate"] = json!("area");
    fs::write(&config, settings.to_string()).unwrap();
    let out = dir.path().join("area.json");
    run(&acquisition, &config, &out).success();
    let area: Value = serde_json::from_slice(&fs::read(&out).unwrap()).unwrap();
    assert_eq!(area["spectrum"], result["spectrum"]);
    assert_eq!(area["binned"]["aggregate"], "area");
    assert_eq!(area["binned"]["points_outside"], 0);
    let total: f64 = area["binned"]["intensity"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_f64().unwrap())
        .sum();
    let trapezoids: f64 = mz
        .windows(2)
        .zip(intensity.windows(2))
        .map(|(m, y)| (y[0] + y[1]) / 2.0 * (m[1] - m[0]))
        .sum();
    assert!((total - trapezoids).abs() <= 1e-12 * trapezoids);

    settings["binning"]["aggregate"] = json!("mean");
    fs::write(&config, settings.to_string()).unwrap();
    let out = dir.path().join("mean.json");
    run(&acquisition, &config, &out)
        .failure()
        .stderr(predicate::str::contains("unknown variant `mean`"));
    assert!(!out.exists());
}

#[test]
fn rejects_unsupported_calibrations_without_overwriting_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.json");
    fs::write(&config, r#"{"steps": [{"step": "sqrt"}]}"#).unwrap();

    let hpc = dir.path().join("hpc");
    write_acquisition(
        &hpc,
        "##$HPClUse= yes \r\n##$HPClBLo= 700 \r\n##$HPClBHi= 3700 \r\n##$HPClOrd= 10 \r\n",
        &[1, 2, 3],
    );
    let out = dir.path().join("hpc.json");
    run(&hpc, &config, &out)
        .failure()
        .stderr(predicate::str::contains("high-precision calibration"));
    assert!(!out.exists());

    let plain = dir.path().join("plain");
    write_acquisition(&plain, "", &[1, 2, 3]);
    let existing = dir.path().join("existing.json");
    fs::write(&existing, "keep").unwrap();
    run(&plain, &config, &existing).failure();
    assert_eq!(fs::read_to_string(&existing).unwrap(), "keep");
}

#[test]
fn preprocesses_a_text_spectrum_and_requires_exactly_one_input() {
    let dir = tempfile::tempdir().unwrap();
    let spectrum = dir.path().join("spectrum.txt");
    fs::write(
        &spectrum,
        "# exported\n\"mass\" \"intensity\"\n2000.5 4\n2001.0 9\n",
    )
    .unwrap();
    let config = dir.path().join("config.json");
    fs::write(&config, r#"{"steps": [{"step": "sqrt"}]}"#).unwrap();
    let out = dir.path().join("text.json");
    run_input("--text", &spectrum, &config, &out).success();
    let result: Value = serde_json::from_slice(&fs::read(&out).unwrap()).unwrap();
    assert_eq!(result["source"], json!({"format": "text", "points": 2}));
    assert_eq!(
        result["spectrum"],
        json!({"mz": [2000.5, 2001.0], "intensity": [2.0, 3.0]})
    );

    let both = dir.path().join("both.json");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "spectrum",
            "preprocess",
            "--text",
            spectrum.to_str().unwrap(),
        ])
        .args(["--bruker", dir.path().to_str().unwrap()])
        .args([
            "--config",
            config.to_str().unwrap(),
            "--out",
            both.to_str().unwrap(),
        ])
        .assert()
        .failure();
    assert!(!both.exists());
}
