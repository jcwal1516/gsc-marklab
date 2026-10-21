#![cfg(feature = "cli")]
use assert_cmd::Command;
use std::{fmt::Write as _, fs};
#[test]
fn posterior_predictive_count_and_translation_k_diagnostics_are_simultaneous() {
    let d = tempfile::tempdir().unwrap();
    let obs = d.path().join("obs.csv");
    let rep = d.path().join("rep.csv");
    let rad = d.path().join("radii.csv");
    let out = d.path().join("out.json");
    fs::write(
        &obs,
        "pattern_id,point_id,x_um,y_um\np1,a,1,1\np1,b,1.5,1\np2,a,2,2\np2,b,4,2\np2,c,8,8\n",
    )
    .unwrap();
    let mut csv = String::from("replicate,pattern_id,point_id,x_um,y_um\n");
    for s in 0..20 {
        for p in 0..2 {
            writeln!(csv, "{s},p{},a,1,1", p + 1).unwrap();
            writeln!(csv, "{s},p{},b,{},1", p + 1, 2 + (s % 4)).unwrap();
            writeln!(csv, "{s},p{},c,{},{}", p + 1, 7 + (s % 2), 7).unwrap();
        }
    }
    fs::write(&rep, csv).unwrap();
    fs::write(&rad, "radius_um\n1\n3\n6\n9\n").unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "point-process-posterior-predictive-diagnostics",
            "--observed",
            obs.to_str().unwrap(),
            "--replicated",
            rep.to_str().unwrap(),
            "--radii",
            rad.to_str().unwrap(),
            "--xmin-um",
            "0",
            "--ymin-um",
            "0",
            "--xmax-um",
            "10",
            "--ymax-um",
            "10",
            "--alpha",
            "0.1",
            "--maximum-pair-visits",
            "100000",
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();
    let r: serde_json::Value = serde_json::from_slice(&fs::read(out).unwrap()).unwrap();
    assert_eq!(
        r["format"],
        "marklab.point_process_posterior_predictive_diagnostics"
    );
    assert_eq!(r["pattern_count"], 2);
    assert_eq!(r["replicate_count"], 20);
    assert_eq!(r["observed_total_count"], 5);
    assert_eq!(r["replicated_total_count_mean"], 6.0);
    assert_eq!(r["replicated_total_count_sd"], 0.0);
    assert_eq!(r["pair_visits"], 124);
    let curve = r["curve"].as_array().unwrap();
    assert_eq!(curve.len(), 4);
    // Each translated pair contributes 2 A^2 / (n(n-1) overlap). The four
    // replicate layouts repeat five times; these values are direct pair sums.
    let null_curves = [
        [1000.0 / 27.0, 1000.0 / 27.0, 1000.0 / 27.0, 11125.0 / 27.0],
        [0.0, 125.0 / 3.0, 125.0 / 3.0, 625.0 / 3.0],
        [0.0, 1000.0 / 21.0, 1000.0 / 21.0, 375.0],
        [0.0, 0.0, 500.0 / 9.0, 11000.0 / 63.0],
    ];
    let means: [f64; 4] =
        std::array::from_fn(|j| null_curves.iter().map(|c| c[j]).sum::<f64>() / 4.0);
    let sds: [f64; 4] = std::array::from_fn(|j| {
        (5.0 * null_curves
            .iter()
            .map(|c| (c[j] - means[j]).powi(2))
            .sum::<f64>()
            / 19.0)
            .sqrt()
    });
    // The 18th of 20 ordered null maxima is the largest of the four distinct layouts.
    let critical = null_curves
        .iter()
        .flat_map(|c| (0..4).map(move |j| ((c[j] - means[j]) / sds[j]).abs()))
        .fold(0.0_f64, f64::max);
    let observed = [
        1000.0 / 19.0,
        8375.0 / 114.0,
        8375.0 / 114.0,
        42250.0 / 171.0,
    ];
    for j in 0..4 {
        let row = &curve[j];
        assert_eq!(row["radius_um"], [1.0, 3.0, 6.0, 9.0][j]);
        for (field, expected) in [
            ("observed_mean_k_um2", observed[j]),
            ("replicated_mean_k_um2", means[j]),
            ("replicated_sd_k_um2", sds[j]),
            ("simultaneous_critical_value", critical),
            ("lower", means[j] - critical * sds[j]),
            ("upper", means[j] + critical * sds[j]),
        ] {
            assert!(
                (row[field].as_f64().unwrap() - expected).abs() < 1e-10,
                "radius {j}, {field}"
            );
        }
        assert_eq!(row["observed_exceeds_envelope"], j < 3);
    }
    assert_eq!(r["exceeded_radii_um"], serde_json::json!([1.0, 3.0, 6.0]));
    assert_eq!(
        r["interpretation"],
        "posterior_predictive_consistency_is_not_model_truth"
    );
}
