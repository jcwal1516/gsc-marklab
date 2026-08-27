use std::{fs, process::Command};

#[test]
fn coordinate_stability_adapter_preserves_field_windows_and_subsamples_cells() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let input = dir.path().join("input");
    let patient = input.join("patients/p1");
    fs::create_dir_all(&patient).expect("patient directory");
    fs::write(
        input.join("patients.csv"),
        "patient_id,field_count,cell_count\np1,2,10\n",
    )
    .expect("patient manifest");
    let mut cells = String::from(
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,slide_id,region_id\n",
    );
    for index in 0..5 {
        cells.push_str(&format!(
            "{},1,0,p1,baseline,coordinate_only,true,true,s1,s1__r0000_c0000\n",
            index + 1
        ));
        cells.push_str(&format!(
            "{},21,0,p1,baseline,coordinate_only,true,true,s1,s1__r0002_c0000\n",
            index + 1
        ));
    }
    fs::write(patient.join("cells.csv"), cells).expect("patient cells");
    fs::write(
        patient.join("window.geojson"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "type":"Feature","properties":{"patient_id":"p1"},
            "geometry":{"type":"MultiPolygon","coordinates":[
                [[[0,0],[10,0],[10,10],[0,10],[0,0]]],
                [[[0,20],[10,20],[10,30],[0,30],[0,20]]]
            ]}
        }))
        .unwrap(),
    )
    .expect("patient window");

    let out = dir.path().join("out");
    let status = Command::new("python3")
        .arg("workers/python/marklab_tcga_crc_coordinate_stability_adapter.py")
        .args([
            "--coordinate-input",
            input.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .expect("run stability adapter");
    assert!(status.success());

    let fields = fs::read_to_string(out.join("fields.csv")).expect("field manifest");
    assert_eq!(fields.lines().count(), 3);
    for field in ["s1__r0000_c0000", "s1__r0002_c0000"] {
        let root = out.join("fields/p1").join(field);
        assert_eq!(
            fs::read_to_string(root.join("cells.csv"))
                .unwrap()
                .lines()
                .count(),
            6
        );
        assert_eq!(
            fs::read_to_string(root.join("cells_subsample_80.csv"))
                .unwrap()
                .lines()
                .count(),
            5
        );
        let window: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("window.geojson")).unwrap()).unwrap();
        assert_eq!(window["geometry"]["type"], "MultiPolygon");
    }
    let admission: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("admission.json")).unwrap()).unwrap();
    assert_eq!(admission["patient_count"], 1);
    assert_eq!(admission["field_count"], 2);
    assert_eq!(admission["population_unit"], "patient");
    assert_eq!(admission["cell_subsample_fraction"], 0.8);
}
