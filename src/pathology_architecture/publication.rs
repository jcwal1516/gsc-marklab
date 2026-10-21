use std::{
    fmt::Write as _,
    fs,
    io::{BufWriter, Write},
    path::Path,
};

use crate::{output::OutputTransaction, MarklabError, Result};

use super::PathologyArchitectureResult;

fn csv(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn optional<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

/// Atomically publish one architecture result, CSV tables and GeoJSON front overlays.
///
/// Geometry is in the explicitly declared micrometre frame, not geographic longitude/latitude.
/// Missing CSV values are empty fields. The output transaction rejects nonempty destinations.
pub fn publish_pathology_architecture(
    result: &PathologyArchitectureResult,
    output: &Path,
) -> Result<()> {
    let transaction = OutputTransaction::new(output)?;
    let write = |name: &str, bytes: &[u8]| -> Result<()> {
        let path = transaction.staging_path().join(name);
        fs::write(&path, bytes).map_err(|error| MarklabError::io(path, error))
    };
    let result_path = transaction.staging_path().join("result.json");
    let file =
        fs::File::create(&result_path).map_err(|error| MarklabError::io(&result_path, error))?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, result)?;
    writer
        .flush()
        .map_err(|error| MarklabError::io(&result_path, error))?;
    let mut components = String::from(
        "slide_id,patient_id,component_id,class,x_um,y_um,area_um2,perimeter_um,hole_count,lumen_count,\
         isoperimetric_quotient,mean_absolute_curvature_per_um,signed_front_distance_um,band_lower_um,band_upper_um\n",
    );
    let mut bands = String::from(
        "slide_id,patient_id,band_lower_um,band_upper_um,class,tissue_area_um2,band_epithelial_area_um2,\
         epithelial_area_um2,epithelial_area_share,component_count,component_area_um2,median_component_area_um2,\
         normalized_component_area_entropy,mean_absolute_curvature_per_um,area_weighted_isoperimetric_quotient,\
         lumen_containing_component_fraction\n",
    );
    let edges = &result.design.band_edges_um;
    for (index, slide) in result.slides.iter().enumerate() {
        for row in &slide.components {
            let _ = writeln!(
                components,
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                csv(&slide.slide_id),
                csv(&slide.patient_id),
                csv(&row.id),
                csv(&row.class),
                row.x_um,
                row.y_um,
                row.area_um2,
                row.perimeter_um,
                row.hole_count,
                optional(row.lumen_count),
                row.isoperimetric_quotient,
                optional(row.mean_absolute_curvature_per_um),
                optional(row.signed_front_distance_um),
                optional(row.band.map(|band| edges[band])),
                optional(row.band.map(|band| edges[band + 1])),
            );
        }
        for band in &slide.bands {
            for class in &band.classes {
                let _ = writeln!(
                    bands,
                    "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                    csv(&slide.slide_id),
                    csv(&slide.patient_id),
                    band.lower_um,
                    band.upper_um,
                    csv(&class.class),
                    band.tissue_area_um2,
                    band.epithelial_area_um2,
                    class.epithelial_area_um2,
                    optional(class.epithelial_area_share),
                    class.component_count,
                    class.component_area_um2,
                    optional(class.median_component_area_um2),
                    optional(class.normalized_component_area_entropy),
                    optional(class.mean_absolute_curvature_per_um),
                    optional(class.area_weighted_isoperimetric_quotient),
                    optional(class.lumen_containing_component_fraction),
                );
            }
        }
        let properties = |layer: &str| {
            serde_json::json!({
                "layer": layer, "slide_id": slide.slide_id, "patient_id": slide.patient_id,
                "coordinate_frame_id": slide.coordinate_frame_id, "coordinate_unit": "micrometer",
            })
        };
        let mut features = vec![
            serde_json::json!({"type": "Feature", "id": "tumor_bulk", "geometry": slide.front.bulk,
                "properties": properties("tumor_bulk")}),
            serde_json::json!({"type": "Feature", "id": "invasive_front", "geometry": slide.front.lines,
                "properties": properties("invasive_front")}),
        ];
        for row in &slide.components {
            let mut properties = properties("epithelial_component");
            properties["component"] = serde_json::to_value(row)?;
            features.push(serde_json::json!({"type": "Feature", "id": row.id,
                "geometry": {"type": "Point", "coordinates": [row.x_um, row.y_um]},
                "properties": properties}));
        }
        write(
            &format!("slide-{index:03}-architecture.geojson"),
            &serde_json::to_vec(&serde_json::json!({
                "type": "FeatureCollection", "coordinate_unit": "micrometer",
                "coordinate_frame_id": slide.coordinate_frame_id, "features": features,
            }))?,
        )?;
    }
    write("components.csv", components.as_bytes())?;
    write("bands.csv", bands.as_bytes())?;
    write(
        "report.md",
        format!(
            "# Epithelial architecture relative to the invasive front\n\n{} slides. Research-tier descriptive geometry.\n\n\
             Epithelium, architecture regions, lumens, mucin and exclusions are declared upstream; Marklab does not \
             segment tissue or classify architecture. The tumor bulk is the morphological closing of epithelium, lumens \
             and mucin at {} um with holes filled, clipped to tissue and kept when at least {} um2. The invasive front is \
             the bulk boundary more than {} um from the tissue edge and outside declared exclusions.\n\n\
             Signed distances are negative inside the bulk. Band areas are tissue-clipped round offsets of the front at \
             0.05 radians; the result reports the maximum chord error. Whole components are assigned to a band by the \
             signed distance of their interior point. Curvature is the absolute turning of each contour after uniform \
             resampling and Gaussian smoothing, divided by smoothed contour length.\n\n\
             `result.json` holds the design, provenance and all results. `components.csv` and `bands.csv` hold the same \
             rows as tables; empty fields are unavailable values. `slide-NNN-architecture.geojson` holds the bulk, front \
             and component points in the declared micrometre frame, not geographic coordinates.\n",
            result.slides.len(),
            result.design.closing_radius_um,
            result.design.minimum_bulk_area_um2,
            result.design.tissue_edge_tolerance_um,
        )
        .as_bytes(),
    )?;
    transaction.commit()
}
