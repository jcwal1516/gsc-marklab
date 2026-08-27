use std::{fs, process::Command};

#[test]
fn tcga_coordinate_adapter_builds_exact_patient_windows_and_graphs() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let cells = dir.path().join("field_cells");
    fs::create_dir(&cells).expect("field cell directory");
    fs::write(
        dir.path().join("admitted.csv"),
        "patient_id,molecular_group\np1,MSI\n",
    )
    .expect("admitted patients");
    fs::write(
        dir.path().join("field_manifest.csv"),
        "patient_id,roi_id,cells\np1,slide1__r0001_c0002,field_cells/a.csv\np1,slide1__r0003_c0004,field_cells/b.csv\n",
    )
    .expect("field manifest");
    fs::write(
        dir.path().join("field_qc.csv"),
        "patient_id,roi_id,field_id,patch_row,patch_column,background_fraction,nominal_area_mm2,tumor_cell_count\np1,slide1,r0001_c0002,1,2,0,0.01,2\np1,slide1,r0003_c0004,3,4,0,0.01,2\n",
    )
    .expect("field QC");
    fs::write(
        dir.path().join("slide_qc.csv"),
        "patient_id,roi_id,target_mpp\np1,slide1,1\n",
    )
    .expect("slide QC");
    fs::write(
        cells.join("a.csv"),
        "cell_id,x_um,y_um,cellvit_pc_000\na,16.5,8.5,0.1\nb,24.5,16.5,0.2\n",
    )
    .expect("first field");
    fs::write(
        cells.join("b.csv"),
        "cell_id,x_um,y_um,cellvit_pc_000\nc,32.5,24.5,0.3\nd,39.5,31.5,0.4\n",
    )
    .expect("second field");

    let out = dir.path().join("out");
    let status = Command::new("python3")
        .arg("workers/python/marklab_tcga_crc_coordinate_adapter.py")
        .args([
            "--field-manifest",
            dir.path().join("field_manifest.csv").to_str().unwrap(),
            "--field-qc",
            dir.path().join("field_qc.csv").to_str().unwrap(),
            "--slide-qc",
            dir.path().join("slide_qc.csv").to_str().unwrap(),
            "--field-cells-root",
            cells.to_str().unwrap(),
            "--admitted-patients",
            dir.path().join("admitted.csv").to_str().unwrap(),
            "--patch-pixels",
            "10",
            "--patch-overlap-pixels",
            "2",
            "--out",
            out.to_str().unwrap(),
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .expect("run adapter");
    assert!(status.success());

    let admission: serde_json::Value = serde_json::from_slice(
        &fs::read(out.join("admission.json")).expect("coordinate admission"),
    )
    .expect("admission JSON");
    assert_eq!(admission["patient_count"], 1);
    assert_eq!(admission["field_count"], 2);
    assert_eq!(admission["cell_count"], 4);
    assert_eq!(admission["population_unit"], "patient");
    assert_eq!(admission["molecular_labels_used_for_features"], false);
    assert_eq!(
        admission["field_side_um_range"],
        serde_json::json!([10.0, 10.0])
    );

    let mask: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("patients/p1/window.geojson")).expect("window"))
            .expect("window JSON");
    assert_eq!(mask["geometry"]["type"], "MultiPolygon");
    assert_eq!(
        mask["geometry"]["coordinates"][0][0][0],
        serde_json::json!([15.0, 7.0])
    );
    assert_eq!(
        mask["geometry"]["coordinates"][1][0][2],
        serde_json::json!([41.0, 33.0])
    );

    let patient_cells = fs::read_to_string(out.join("patients/p1/cells.csv")).expect("cells");
    assert!(patient_cells.starts_with(
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,slide_id,region_id\n"
    ));
    assert_eq!(patient_cells.lines().count(), 5);

    for grid in [2, 3] {
        let graph: serde_json::Value = serde_json::from_slice(
            &fs::read(out.join(format!("patients/p1/graph_{grid}x{grid}.json")))
                .expect("graph input"),
        )
        .expect("graph JSON");
        assert_eq!(graph["nodes"].as_array().unwrap().len(), 2 * grid * grid);
        assert_eq!(graph["radius_um"], 10.0 / grid as f64 * 1.01);
        let signal_sum: f64 = graph["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| node["signal"].as_f64().unwrap())
            .sum();
        assert!(signal_sum.abs() < 1e-12);
    }
}
