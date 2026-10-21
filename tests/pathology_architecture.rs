use std::f64::consts::PI;

use serde_json::{json, Value};

fn polygon(left: f64, bottom: f64, right: f64, top: f64) -> Value {
    json!({"type":"MultiPolygon","coordinates":[[[[left,bottom],[right,bottom],
        [right,top],[left,top],[left,bottom]]]]})
}

fn recipe() -> Value {
    serde_json::from_str(include_str!("../examples/pathology-architecture/recipe.json")).unwrap()
}

fn analyze(input: &Value) -> Value {
    serde_json::to_value(
        marklab::pathology::analyze_pathology_architecture(&serde_json::to_vec(input).unwrap())
            .unwrap(),
    )
    .unwrap()
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap_or_else(|| panic!("expected number, found {value}"))
}

fn close(value: &Value, expected: f64, tolerance: f64) {
    let observed = number(value);
    assert!(
        (observed - expected).abs() <= tolerance,
        "observed {observed}, expected {expected} +/- {tolerance}"
    );
}

fn class_band<'a>(band: &'a Value, class: &str) -> &'a Value {
    band["classes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["class"] == class)
        .unwrap()
}

#[test]
fn closed_bulk_front_bands_and_components_match_exact_square_geometry() {
    let result = analyze(&recipe());
    let slide = &result["slides"][0];
    let front = &slide["front"];
    assert_eq!(front["status"], "available");
    assert_eq!(front["bulk_component_count"], 1);
    // Closing a convex square returns it; fragments 105 um away close into a sub-minimum bulk.
    close(&front["bulk_area_um2"], 160_000.0, 1.0);
    close(&front["length_um"], 1600.0, 0.5);
    let bands = slide["bands"].as_array().unwrap();
    assert_eq!(bands.len(), 4);
    close(&bands[0]["tissue_area_um2"], 40_000.0, 1.0);
    close(&class_band(&bands[0], "glandular")["epithelial_area_um2"], 30_000.0, 1.0);
    close(&bands[1]["tissue_area_um2"], 120_000.0, 1.0);
    close(&class_band(&bands[1], "glandular")["epithelial_area_um2"], 120_000.0, 1.0);
    // Outer offsets of the derived front: length * d + pi * d^2, less the polygonal-arc
    // deficit (about 13.5 and 54 um2 for full circles of radius 100 and 200 um).
    let length = number(&front["length_um"]);
    close(&bands[2]["tissue_area_um2"], length * 100.0 + PI * 1e4, 15.0);
    close(&bands[3]["tissue_area_um2"], length * 100.0 + PI * 3e4, 45.0);
    close(&front["maximum_buffer_chord_error_um"], 200.0 * (1.0 - 0.05f64.cos()), 1e-12);
    let fragments = class_band(&bands[3], "highly_fragmented");
    close(&fragments["epithelial_area_um2"], 200.0, 1e-6);
    assert_eq!(fragments["component_count"], 2);
    close(&fragments["median_component_area_um2"], 100.0, 1e-9);
    close(&fragments["normalized_component_area_entropy"], 1.0, 1e-12);
    assert_eq!(class_band(&bands[3], "unclassified")["component_count"], 0);

    let components = slide["components"].as_array().unwrap();
    assert_eq!(components.len(), 3);
    let gland = &components[0];
    assert_eq!(gland["class"], "glandular");
    close(&gland["area_um2"], 150_000.0, 1e-9);
    close(&gland["perimeter_um"], 2000.0, 1e-9);
    assert_eq!(gland["hole_count"], 1);
    assert_eq!(gland["lumen_count"], 1);
    close(&gland["isoperimetric_quotient"], 4.0 * PI * 150_000.0 / 4e6, 1e-12);
    // Convex exterior and convex hole each turn by exactly 2*pi after smoothing.
    close(&gland["absolute_turning_rad"], 4.0 * PI, 1e-9);
    let (x, y) = (number(&gland["x_um"]), number(&gland["y_um"]));
    let inner = (x - 300.0).min(700.0 - x).min(y - 300.0).min(700.0 - y);
    close(&gland["signed_front_distance_um"], -inner, 0.01);
    for fragment in &components[1..] {
        assert_eq!(fragment["class"], "highly_fragmented");
        assert_eq!(fragment["lumen_count"], 0);
        close(&fragment["absolute_turning_rad"], 2.0 * PI, 1e-9);
        close(
            &fragment["signed_front_distance_um"],
            number(&fragment["x_um"]) - 700.0,
            0.01,
        );
        assert_eq!(fragment["band"], 3);
    }
    let pooled = 4.0 * PI
        / (number(&components[1]["curvature_contour_length_um"])
            + number(&components[2]["curvature_contour_length_um"]));
    close(&fragments["mean_absolute_curvature_per_um"], pooled, 1e-12);
}

#[test]
fn front_excludes_tissue_edges_and_declared_exclusions() {
    let mut input = recipe();
    input["slides"][0]["front_exclusions"] = polygon(250.0, 250.0, 350.0, 750.0);
    let front = &analyze(&input)["slides"][0]["front"];
    // Left side (400) and the first 50 um of the top and bottom sides are excluded.
    close(&front["length_um"], 1100.0, 0.5);

    let mut input = recipe();
    input["slides"][0]["window"] = polygon(300.0, 0.0, 1000.0, 1000.0);
    input["slides"][0]["architecture_regions"][0]["geometry"] =
        polygon(300.0, 290.0, 710.0, 710.0);
    input["slides"][0]["declared_front"] = Value::Null;
    let slide = &analyze(&input)["slides"][0];
    // The gland's left side is the tissue edge; 25 um of each adjacent side is within tolerance.
    close(&slide["front"]["length_um"], 1150.0, 0.5);
    assert!(number(&slide["front"]["bulk_area_um2"]) > 159_000.0);
}

#[test]
fn declared_front_agreement_is_symmetric_and_sampled() {
    let front = &analyze(&recipe())["slides"][0]["front"];
    let agreement = &front["declared_agreement"];
    close(&agreement["declared_length_um"], 1640.0, 1e-9);
    let derived = &agreement["derived_to_declared"];
    close(&derived["mean_um"], 5.0, 0.02);
    close(&derived["maximum_um"], 5.0, 0.05);
    let declared = &agreement["declared_to_derived"];
    // Declared corners are 5*sqrt(2) from the derived corners.
    close(&declared["maximum_um"], 5.0 * 2f64.sqrt(), 0.05);
    assert!(number(&declared["mean_um"]) > 5.0 && number(&declared["mean_um"]) < 5.2);
    assert!(declared["samples"].as_u64().unwrap() >= 1640);
}

#[test]
fn unclassified_and_unavailable_states_stay_explicit() {
    let mut input = recipe();
    input["slides"][0]["architecture_regions"] = json!([]);
    input["slides"][0]["lumens"] = Value::Null;
    let slide = &analyze(&input)["slides"][0];
    assert!(slide["components"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["class"] == "unclassified" && row["lumen_count"].is_null()));
    let band = &slide["bands"][1];
    close(&class_band(band, "unclassified")["epithelial_area_um2"], 120_000.0, 1.0);
    assert!(class_band(band, "unclassified")["lumen_containing_component_fraction"].is_null());
    assert_eq!(class_band(band, "glandular")["epithelial_area_um2"], 0.0);
    assert!(class_band(band, "glandular")["median_component_area_um2"].is_null());

    let mut input = recipe();
    input["design"]["minimum_bulk_area_um2"] = json!(1e9);
    let slide = &analyze(&input)["slides"][0];
    assert_eq!(
        slide["front"]["status"],
        "unavailable_no_tumor_bulk_at_minimum_area"
    );
    assert_eq!(slide["bands"], json!([]));
    assert!(slide["front"]["declared_agreement"].is_null());
    assert!(slide["components"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["signed_front_distance_um"].is_null() && row["band"].is_null()));
}

#[test]
fn canonical_order_is_independent_of_input_order() {
    let original = analyze(&recipe());
    let mut reordered = recipe();
    reordered["slides"][0]["architecture_regions"]
        .as_array_mut()
        .unwrap()
        .reverse();
    reordered["slides"][0]["epithelium"]["coordinates"]
        .as_array_mut()
        .unwrap()
        .reverse();
    assert_eq!(original, analyze(&reordered));
}

#[test]
fn invalid_layers_and_designs_are_rejected() {
    let cases: Vec<(&str, Box<dyn Fn(&mut Value)>)> = vec![
        (
            "overlapping regions",
            Box::new(|r| {
                r["slides"][0]["architecture_regions"][1]["geometry"] =
                    polygon(700.0, 460.0, 850.0, 530.0)
            }),
        ),
        (
            "epithelium outside tissue",
            Box::new(|r| r["slides"][0]["window"] = polygon(0.0, 0.0, 820.0, 1000.0)),
        ),
        (
            "lumen overlaps epithelium",
            Box::new(|r| r["slides"][0]["lumens"] = polygon(440.0, 440.0, 560.0, 560.0)),
        ),
        (
            "undeclared class",
            Box::new(|r| r["slides"][0]["architecture_regions"][0]["class"] = json!("cribriform")),
        ),
        (
            "duplicate region identity",
            Box::new(|r| r["slides"][0]["architecture_regions"][1]["id"] = json!("gland")),
        ),
        (
            "bands narrower than twice the boundary uncertainty",
            Box::new(|r| r["slides"][0]["boundary_uncertainty_um"] = json!(60.0)),
        ),
        (
            "declared front without provenance",
            Box::new(|r| r.as_object_mut().unwrap().remove("declared_front").map(drop).unwrap()),
        ),
        (
            "declared front outside tissue",
            Box::new(|r| {
                r["slides"][0]["declared_front"]["coordinates"][0][0] = json!([-5.0, 295.0])
            }),
        ),
        (
            "reserved class name",
            Box::new(|r| r["architecture"]["classes"][1] = json!("unclassified")),
        ),
        (
            "nonincreasing bands",
            Box::new(|r| r["design"]["band_edges_um"] = json!([0.0, 0.0])),
        ),
        (
            "geometry work limit",
            Box::new(|r| r["limits"] = json!({"maximum_geometry_work": 10})),
        ),
    ];
    for (name, mutate) in cases {
        let mut input = recipe();
        mutate(&mut input);
        assert!(
            marklab::pathology::analyze_pathology_architecture(
                &serde_json::to_vec(&input).unwrap()
            )
            .is_err(),
            "{name} was accepted"
        );
    }
}

#[cfg(feature = "cli")]
#[test]
fn cli_publishes_result_tables_and_overlays_atomically() {
    use assert_cmd::Command;
    use std::fs;
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("recipe.json");
    fs::write(&source, serde_json::to_vec(&recipe()).unwrap()).unwrap();
    let out = directory.path().join("output");
    let run = || {
        Command::cargo_bin("marklab")
            .unwrap()
            .args(["pathology", "architecture", "--recipe"])
            .arg(&source)
            .arg("--out")
            .arg(&out)
            .assert()
    };
    run().success();
    let result: Value =
        serde_json::from_slice(&fs::read(out.join("result.json")).unwrap()).unwrap();
    assert_eq!(result, analyze(&recipe()));
    let components = fs::read_to_string(out.join("components.csv")).unwrap();
    assert_eq!(components.lines().count(), 4);
    assert!(components.lines().nth(1).unwrap().contains("\"glandular\""));
    let bands = fs::read_to_string(out.join("bands.csv")).unwrap();
    // Four bands times four declared classes plus unclassified, and a header.
    assert_eq!(bands.lines().count(), 21);
    let overlay: Value = serde_json::from_slice(
        &fs::read(out.join("slide-000-architecture.geojson")).unwrap(),
    )
    .unwrap();
    assert_eq!(overlay["features"][1]["properties"]["layer"], "invasive_front");
    assert_eq!(overlay["features"].as_array().unwrap().len(), 5);
    let before = fs::read(out.join("result.json")).unwrap();
    run().failure();
    assert_eq!(fs::read(out.join("result.json")).unwrap(), before);
}
