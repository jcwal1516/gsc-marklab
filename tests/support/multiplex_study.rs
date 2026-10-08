use serde_json::{json, Value};

pub fn recipe() -> Value {
    let mut slides = Vec::new();
    for patient in 0..6 {
        for slide in 0..2 {
            let shift = 0.23 * patient as f64 + 0.13 * slide as f64;
            slides.push(json!({
                "slide_id": format!("slide-{patient}-{slide}"),
                "patient_id": format!("patient-{patient}"),
                "group": if patient < 3 { "reference" } else { "comparison" },
                "coordinate_frame_id": format!("slide-{patient}-{slide}-um"),
                "window": {"type":"MultiPolygon", "coordinates":[[[[-1.0,-1.0],[6.0,-1.0],[6.0,1.0],[-1.0,1.0],[-1.0,-1.0]]]]},
                "cell_ids": (0..6).map(|row| format!("slide-{patient}-{slide}:cell-{row}")).collect::<Vec<_>>(),
                "coordinates_um": [[0.0,0.0],[1.0,0.0],[2.0,0.0],[3.0,0.0],[4.0,0.0],[5.0,0.0]],
                "observations": {"CD3": [1.0, 2.0+shift, 4.0, 1.0+shift, 8.0, 4.0-shift], "CD8": [3.0, 1.0, 4.0+shift, 1.0, 5.0+shift, 9.0], "positive": [1,0,null,1,0,1], "cell_type": [0,0,1,null,1,0]}
            }));
        }
    }
    json!({
        "format": "marklab.multiplex_study_recipe", "version": 1, "study_id": "test-panel",
        "channels": [
            {"id":"CD3","label":"CD3 intensity","kind":"continuous","unit":"fluorescence_au","measurement_status":"measured","provenance":"assay-1/background-subtraction-1"},
            {"id":"CD8","label":"CD8 intensity","kind":"continuous","unit":"fluorescence_au","measurement_status":"measured","provenance":"assay-1/background-subtraction-1"},
            {"id":"positive","label":"declared phenotype","kind":"binary","unit":"unitless","measurement_status":"measured","provenance":"assay-1/threshold-1"},
            {"id":"cell_type","label":"cell class","kind":"categorical","unit":"categorical","measurement_status":"imported_prediction","provenance":"classifier-1","levels":["immune","tumor"]}
        ],
        "design": {"selected_channels":["CD3","CD8"],"radius_um":1.1,"weight_policy":"binary_symmetric","missingness":"per_channel_complete_case","patient_reduction":"equal_slide_mean","exchangeability":"independent_patients","group_a":"comparison","group_b":"reference","permutations":99,"seed":41,"alpha":0.05},
        "slides":slides
    })
}
