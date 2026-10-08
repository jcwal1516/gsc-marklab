use std::{
    fs,
    io::{BufWriter, Write},
    path::Path,
};

use crate::{output::OutputTransaction, MarklabError, Result};

use super::{render, PathologyMapsResult};

/// Atomically publish one complete map result, SVG figures and spatial GeoJSON overlays.
///
/// Geometry is in the explicitly declared micrometre frame, not geographic longitude/latitude.
/// Existing output transaction semantics reject nonempty destinations.
pub fn publish_pathology_maps(result: &PathologyMapsResult, output: &Path) -> Result<()> {
    let transaction = OutputTransaction::new(output)?;
    let write = |name: &str, bytes: &[u8]| -> Result<()> {
        let path = transaction.staging_path().join(name);
        fs::write(&path, bytes).map_err(|error| MarklabError::io(path, error))
    };
    let result_path = transaction.staging_path().join("result.json");
    let result_file =
        fs::File::create(&result_path).map_err(|error| MarklabError::io(&result_path, error))?;
    let mut result_writer = BufWriter::new(result_file);
    serde_json::to_writer_pretty(&mut result_writer, result)?;
    result_writer
        .flush()
        .map_err(|error| MarklabError::io(&result_path, error))?;
    for (index, slide) in result.slides.iter().enumerate() {
        let path = transaction
            .staging_path()
            .join(format!("slide-{index:03}-maps.geojson"));
        let file = fs::File::create(&path).map_err(|error| MarklabError::io(&path, error))?;
        let mut writer = BufWriter::new(file);
        writer.write_all(br#"{"type":"FeatureCollection","coordinate_unit":"micrometer","coordinate_frame_id":"#)
            .map_err(|error| MarklabError::io(&path, error))?;
        serde_json::to_writer(&mut writer, &slide.coordinate_frame_id)?;
        writer
            .write_all(b",\"features\":[")
            .map_err(|error| MarklabError::io(&path, error))?;
        for (row_index, row) in slide.local_map.rows.iter().enumerate() {
            let neighborhoods = slide
                .neighborhood_maps
                .iter()
                .map(|map| {
                    serde_json::json!({
                        "radius_um":map.radius_um, "result":map.rows[row_index],
                    })
                })
                .collect::<Vec<_>>();
            if row_index > 0 {
                writer
                    .write_all(b",")
                    .map_err(|error| MarklabError::io(&path, error))?;
            }
            serde_json::to_writer(
                &mut writer,
                &serde_json::json!({
                    "type":"Feature", "id":row.cell_id,
                    "geometry":{"type":"Point","coordinates":[row.x_um,row.y_um]},
                    "properties":{"patient_id":slide.patient_id,"slide_id":slide.slide_id,
                        "coordinate_frame_id":slide.coordinate_frame_id,"coordinate_unit":"micrometer",
                        "local":row,"neighborhoods":neighborhoods},
                }),
            )?;
        }
        writer
            .write_all(b"]}\n")
            .and_then(|()| writer.flush())
            .map_err(|error| MarklabError::io(&path, error))?;
        write(
            &format!("slide-{index:03}-local.svg"),
            render::local_svg(slide).as_bytes(),
        )?;
        for (map_index, map) in slide.neighborhood_maps.iter().enumerate() {
            write(
                &format!("slide-{index:03}-neighborhood-{map_index:02}.svg"),
                render::neighborhood_svg(slide, map, &result.phenotypes.names).as_bytes(),
            )?;
        }
    }
    write("report.md", format!(
        "# H&E/IHC spatial characterization\n\n{} slides. Experimental within-slide maps; cells are not independent patients.\n\n\
         Slides without supplied structure annotations have an empty profiles array; structure-relative analysis is unavailable. \
         When annotations are supplied, profiles use signed distances (negative inside) and tissue-clipped round-buffer areas. \
         Curved buffers are approximated at 0.05 radians; each profile reports its maximum chord error. \
         Counts and marker means use exact point-to-boundary distances. Missing markers remain missing.\n\n\
         Neighborhood maps report supplied phenotype composition and effective diversity at each declared radius. \
         Dominant/mixed labels require the declared support and a complete radius disk inside the supplied observation window. \
         A sampled-patch window describes observation coverage, not a segmented tissue boundary. \
         They do not establish discovered or reproducible biological niches.\n\n\
         Local Moran uses whole-value random labeling within declared strata, with single-step maximum-absolute \
         adjustment across eligible locations followed by Bonferroni across all input slides. \
         Missing, isolated and constant observations have no inferential label. \
         A significant local pattern is not a population difference or clinical conclusion.\n\n\
         `result.json` contains assay provenance, design, denominators and all results. \
         `slide-NNN-maps.geojson` contains per-cell overlays in the declared micrometre frame; these are not geographic coordinates. \
         The SVG files show corrected local classifications and phenotype-composition maps with their legends.\n",
        result.slides.len(),
    ).as_bytes())?;
    transaction.commit()
}
