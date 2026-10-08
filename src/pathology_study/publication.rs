use super::composition::PathologyCompositionResult;
use super::scan::PathologyScanResult;
use super::spatial::PathologySpatialStudyResult;
use crate::{output::OutputTransaction, MarklabError, Result};
use serde::Serialize;
use std::{fs, path::Path};

pub(super) fn csv(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
pub(super) use crate::output::xml_text as xml;

fn probability(value: Option<f64>) -> String {
    value
        .map(|v| {
            if v > 0.0 && v < 0.0001 {
                format!("{v:.3e}")
            } else {
                format!("{v:.4}")
            }
        })
        .unwrap_or_else(|| "unavailable".into())
}

pub(super) fn publish<T: Serialize>(
    result: &T,
    output: &Path,
    files: Vec<(String, String)>,
) -> Result<()> {
    let transaction = OutputTransaction::new(output)?;
    let path = transaction.staging_path().join("result.json");
    let file = fs::File::create(&path).map_err(|e| MarklabError::io(&path, e))?;
    serde_json::to_writer_pretty(file, result)?;
    for (name, contents) in files {
        let path = transaction.staging_path().join(name);
        fs::write(&path, contents).map_err(|e| MarklabError::io(&path, e))?;
    }
    transaction.commit()
}

/// Publish composition tables, grid overlays and the interpretation report atomically.
pub fn publish_pathology_composition(
    result: &PathologyCompositionResult,
    output: &Path,
) -> Result<()> {
    let mut patients =
        String::from("patient_id,group,phenotype,count,area_mm2,density_per_mm2,fraction\n");
    for p in &result.patients {
        for (i, name) in result.phenotypes.names.iter().enumerate() {
            patients.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                csv(&p.patient_id),
                csv(&p.group),
                csv(name),
                p.counts[i],
                p.area_mm2,
                p.densities_per_mm2[i],
                p.fractions[i].map(|v| v.to_string()).unwrap_or_default()
            ));
        }
    }
    let mut contrasts=String::from("endpoint,effect_group_a_minus_group_b,pointwise_lower,pointwise_upper,max_t_p,unavailable\n");
    for e in &result.inference.endpoints {
        let field = |v: Option<f64>| v.map(|v| v.to_string()).unwrap_or_default();
        contrasts.push_str(&format!(
            "{},{},{},{},{},{}\n",
            csv(&e.endpoint),
            field(e.effect_group_a_minus_group_b),
            field(e.pointwise_interval.map(|v| v[0])),
            field(e.pointwise_interval.map(|v| v[1])),
            field(e.adjusted_p_value),
            csv(e.unavailable.as_deref().unwrap_or(""))
        ));
    }
    let mut files = vec![
        ("patients.csv".into(), patients),
        ("contrasts.csv".into(), contrasts),
    ];
    for (i, slide) in result.slides.iter().enumerate() {
        let features=slide.grids.iter().flat_map(|g|g.tiles.iter().map(move |t|serde_json::json!({"type":"Feature","geometry":t.geometry,"properties":{"grid":g.id,"patient_id":slide.patient_id,"slide_id":slide.slide_id,"area_mm2":t.area_mm2,"count":t.phenotype_count,"density_per_mm2":t.phenotype_count as f64/t.area_mm2}}))).collect::<Vec<_>>();
        files.push((format!("slide-{i:03}-grids.geojson"),serde_json::to_string(&serde_json::json!({"type":"FeatureCollection","coordinate_unit":"micrometer","coordinate_frame_id":slide.coordinate_frame_id,"features":features}))?));
    }
    let mut svg=String::from("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1000\" height=\"600\" viewBox=\"0 0 1000 600\"><rect width=\"1000\" height=\"600\" fill=\"white\"/><g font-family=\"sans-serif\" fill=\"#24384b\"><text x=\"30\" y=\"35\" font-size=\"22\">MAUP: phenotype density variance</text><text x=\"30\" y=\"60\">Group A minus group B; offsets compared within each width; units (cells/mm²)²</text>");
    let rows = result
        .inference
        .endpoints
        .iter()
        .filter(|e| e.endpoint.starts_with("maup:"))
        .collect::<Vec<_>>();
    let scale = rows
        .iter()
        .filter_map(|e| e.effect_group_a_minus_group_b)
        .map(f64::abs)
        .fold(0.0, f64::max)
        .max(f64::MIN_POSITIVE);
    let step = 480.0 / (rows.len().max(1) as f64);
    svg.push_str("<path d=\"M650 80V570\" stroke=\"#888\"/>");
    for (i, e) in rows.iter().enumerate() {
        let y = 90.0 + i as f64 * step;
        svg.push_str(&format!(
            "<text x=\"30\" y=\"{y}\">{}</text>",
            xml(&e.endpoint)
        ));
        if let Some(effect) = e.effect_group_a_minus_group_b {
            let end = 650.0 + effect / scale * 290.0;
            svg.push_str(&format!("<path d=\"M650 {}H{end}\" stroke=\"#008577\" stroke-width=\"8\"/><text x=\"250\" y=\"{y}\">{effect:.3e}</text>",y-5.0));
        }
    }
    svg.push_str("</g></svg>");
    files.push(("maup.svg".into(), svg));
    files.push(("report.md".into(),format!("# Composition, density and MAUP\n\nStudy: {}. {} independent patients, {} slides. Counts and observed areas are pooled within patients; patients receive equal weight.\n\nFractions and log-ratios are relative; density is cells/mm². Geometric balances and amalgamated ratios have distinct definitions in result.json. Zeros receive no pseudocount. Missing endpoints never cause patient deletion.\n\nMAUP is the area-weighted variance of tile phenotype density around the patient's pooled density. Observed empty tiles contribute zero density. Whole-domain counts/exposure are conserved. Primary grid: {}. Sign reversals: {:?}. Grid sensitivity is not a combined significance probability.\n\nPatient contrasts use a shared whole-patient Max-T family. Bootstrap intervals are pointwise, not simultaneous. Constant endpoints or numerical inference failures remain unavailable. Family status: {}. These synthetic/experimental results do not establish population validity for an unadmitted real study.\n\npatients.csv and contrasts.csv contain numerical summaries; per-slide GeoJSON contains tissue-clipped tiles in physical micrometres; maup.svg compares effects. Full provenance/design/availability are in result.json.\n",result.study_id,result.patients.len(),result.slides.len(),result.sensitivity.primary,result.sensitivity.sign_reversals,result.inference.unavailable.as_deref().unwrap_or("see per-endpoint availability"))));
    publish(result, output, files)
}

/// Publish common-axis patient curves and their distinct within-slide and cohort inference.
pub fn publish_pathology_spatial_study(
    result: &PathologySpatialStudyResult,
    output: &Path,
) -> Result<()> {
    let endpoint = match result.design.statistic {
        super::spatial::Statistic::LMinusR => "L(r) − r (µm)",
        super::spatial::Statistic::GMinusOne => "g(r) − 1",
        super::spatial::Statistic::CrossGMinusOne => "Directed cross-g(r) − 1",
    };
    let mut table = String::from("patient_id,group,radius_um,value,unavailable\n");
    for p in &result.patients {
        for (i, r) in result.design.radii_um.iter().enumerate() {
            table.push_str(&format!(
                "{},{},{},{},{}\n",
                csv(&p.patient_id),
                csv(&p.group),
                r,
                p.values
                    .as_ref()
                    .map(|v| v[i].to_string())
                    .unwrap_or_default(),
                csv(p.unavailable.as_deref().unwrap_or(""))
            ));
        }
    }
    let mut slide_table =
        String::from("slide_id,patient_id,radius_um,value,p_global,slide_family_p,unavailable\n");
    for s in &result.slides {
        for (i, r) in result.design.radii_um.iter().enumerate() {
            let field = |v: Option<f64>| v.map(|v| v.to_string()).unwrap_or_default();
            slide_table.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                csv(&s.slide_id),
                csv(&s.patient_id),
                r,
                field(s.values[i]),
                field(s.p_global),
                field(s.slide_family_p),
                csv(s.unavailable.as_deref().unwrap_or(""))
            ));
        }
    }
    let values = result
        .patients
        .iter()
        .filter_map(|p| p.values.as_ref())
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    let low = values.iter().copied().fold(0.0, f64::min);
    let high = values.iter().copied().fold(0.0, f64::max);
    let span = (high - low).max(1e-12);
    let axis = &result.design.radii_um;
    let x = |r: f64| 80.0 + (r - axis[0]) / (axis[axis.len() - 1] - axis[0]) * 780.0;
    let y = |v: f64| 500.0 - (v - low) / span * 380.0;
    let mut svg=format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1000 600\"><rect width=\"1000\" height=\"600\" fill=\"white\"/><g font-family=\"sans-serif\"><text x=\"60\" y=\"35\" font-size=\"22\">Area-weighted patient spatial curves</text><text x=\"60\" y=\"65\">{}; {:?} intensity / {:?} null</text><text x=\"60\" y=\"90\">{}: teal; {}: orange. Patient L2 p: {}</text><path d=\"M80 110V500H870\" fill=\"none\" stroke=\"#444\"/><path d=\"M80 {}H870\" stroke=\"#aaa\" stroke-dasharray=\"4 4\"/><text x=\"80\" y=\"530\">{} µm</text><text x=\"790\" y=\"530\">{} µm</text><text x=\"5\" y=\"125\">{high:.3}</text><text x=\"5\" y=\"500\">{low:.3}</text>",endpoint,result.design.intensity.model,result.design.intensity.null_mode,xml(&result.design.group_a),xml(&result.design.group_b),probability(result.inference.p_value),y(0.0),axis[0],axis[axis.len()-1]);
    for p in &result.patients {
        if let Some(values) = &p.values {
            let points = axis
                .iter()
                .zip(values)
                .map(|(r, v)| format!("{},{}", x(*r), y(*v)))
                .collect::<Vec<_>>()
                .join(" ");
            let color = if p.group == result.design.group_a {
                "#008577"
            } else {
                "#c16724"
            };
            svg.push_str(&format!("<polyline points=\"{points}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"1.5\" opacity=\"0.65\"><title>{}</title></polyline>",xml(&p.patient_id)));
        }
    }
    svg.push_str("<text x=\"60\" y=\"575\">Experimental fitted-null inference; unsupported patient curves are omitted from drawing, not from inference.</text></g></svg>");
    let report=format!("# Context-conditioned spatial study\n\nStudy: {}. {} slides, {} patients. Endpoint {}. Intensity {:?}, null {:?}. Standard-border correction.\n\nEach patient curve is the area-weighted mean of its slide curves. All curves must support the complete declared radius axis; no bins, slides or patients are silently dropped. Patient L2 comparison: p={}; status={}. Within-slide ERL probabilities and across-slide Bonferroni probabilities answer a different question from the patient comparison.\n\nGaussian refitting repeats the declared estimator, cross-fitting and enabled bandwidth selection for every simulated pattern. This is experimental parametric-bootstrap inference, not established nominal error control. Plug-in inference conditions on its fitted reference. Compartment inference conditions on the declared compartment counts. Intensity-support failures are reported, and failed slides conservatively consume their reserved work allowance.\n\nNo clinical claim or spatstat/GET parity is established. patient-curves.csv, slide-curves.csv and curves.svg show the available summaries; result.json retains the full design, provenance, intensity and support diagnostics.\n",result.study_id,result.slides.len(),result.patients.len(),endpoint,result.design.intensity.model,result.design.intensity.null_mode,probability(result.inference.p_value),result.inference.unavailable.as_deref().unwrap_or("available"));
    publish(
        result,
        output,
        vec![
            ("patient-curves.csv".into(), table),
            ("slide-curves.csv".into(), slide_table),
            ("curves.svg".into(), svg),
            ("report.md".into(), report),
        ],
    )
}

/// Publish scan maxima, stratum-specific enrichment and clipped physical-coordinate maps.
pub fn publish_pathology_scan(result: &PathologyScanResult, output: &Path) -> Result<()> {
    let mut table = String::from(
        "slide_id,patient_id,center_cell_id,radius_um,score,scan_p,slide_family_p,unavailable\n",
    );
    let mut strata=String::from("slide_id,stratum,inside_cases,inside_total,outside_cases,outside_total,relative_enrichment,unavailable\n");
    let mut files = Vec::new();
    for (index, s) in result.slides.iter().enumerate() {
        let field = |v: Option<f64>| v.map(|v| v.to_string()).unwrap_or_default();
        table.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            csv(&s.slide_id),
            csv(&s.patient_id),
            csv(s
                .winner
                .as_ref()
                .map(|w| w.center_cell_id.as_str())
                .unwrap_or("")),
            field(s.winner.as_ref().map(|w| w.radius_um)),
            field(s.winner.as_ref().map(|w| w.score)),
            field(s.p_global),
            field(s.slide_family_p),
            csv(s.unavailable.as_deref().unwrap_or(""))
        ));
        let mut features = Vec::new();
        if let Some(w) = &s.winner {
            features.push(serde_json::json!({"type":"Feature","geometry":w.geometry,"properties":{"slide_id":s.slide_id,"patient_id":s.patient_id,"score":w.score,"radius_um":w.radius_um,"p_global":s.p_global,"slide_family_p":s.slide_family_p,"display_maximum_chord_error_um":w.display_maximum_chord_error_um}}));
            for t in &w.strata {
                strata.push_str(&format!(
                    "{},{},{},{},{},{},{},{}\n",
                    csv(&s.slide_id),
                    csv(&t.stratum),
                    t.inside_cases,
                    t.inside_total,
                    t.outside_cases,
                    t.outside_total,
                    field(t.relative_enrichment),
                    csv(t.unavailable.as_deref().unwrap_or(""))
                ));
            }
        }
        files.push((format!("slide-{index:03}-scan.geojson"),serde_json::to_string(&serde_json::json!({"type":"FeatureCollection","coordinate_frame_id":s.coordinate_frame_id,"coordinate_unit":"micrometer","features":features}))?));
        let [xmin, ymin, xmax, ymax] = s.bounds_um;
        let scale = (740.0 / (xmax - xmin)).min(420.0 / (ymax - ymin));
        let mut svg=format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1000 600\"><rect width=\"1000\" height=\"600\" fill=\"white\"/><g font-family=\"sans-serif\"><text x=\"30\" y=\"35\" font-size=\"22\">Binary enrichment scan — {}</text><text x=\"30\" y=\"65\">Complete-search p={}; across-slide p={}; {} eligible disks</text><text x=\"30\" y=\"90\">Positive: red; other eligible: gray; winning disk ∩ observation window: blue</text></g><g transform=\"translate(30 120) scale({scale}) translate({} {})\">",xml(&s.slide_id),probability(s.p_global),probability(s.slide_family_p),s.candidate_count,-xmin,-ymin);
        let shape = |geometry: &serde_json::Value, fill: &str, stroke: &str| {
            let mut path = String::new();
            let geometry = geometry.get("geometry").unwrap_or(geometry);
            if let Some(polygons) = geometry["coordinates"].as_array() {
                for polygon in polygons {
                    if let Some(rings) = polygon.as_array() {
                        for ring in rings {
                            if let Some(points) = ring.as_array() {
                                for (i, p) in points.iter().enumerate() {
                                    path.push_str(&format!(
                                        "{}{} {} ",
                                        if i == 0 { 'M' } else { 'L' },
                                        p[0],
                                        p[1]
                                    ));
                                }
                                path.push('Z');
                            }
                        }
                    }
                }
            }
            format!("<path d=\"{path}\" fill=\"{fill}\" fill-rule=\"evenodd\" stroke=\"{stroke}\" stroke-width=\"{}\"/>",1.0/scale)
        };
        svg.push_str(&shape(&s.window, "#f6f5f1", "#444"));
        if let Some(w) = &s.winner {
            svg.push_str(&shape(&w.geometry, "#b6dcef", "#1677a5"));
        }
        let radius = 2.5 / scale;
        for cell in &s.cells {
            svg.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"{radius}\" fill=\"{}\"><title>{}</title></circle>",
                cell.x_um,
                cell.y_um,
                if cell.positive { "#b52e32" } else { "#777" },
                xml(&cell.id)
            ));
        }
        svg.push_str("</g><g font-family=\"sans-serif\"><text x=\"30\" y=\"575\">Exact radial counts; polygonal disk display. A candidate disk is not a lesion boundary.</text></g></svg>");
        files.push((format!("slide-{index:03}-scan.svg"), svg));
    }
    files.push(("scans.csv".into(), table));
    files.push(("strata.csv".into(), strata));
    files.push(("report.md".into(),format!("# Binary phenotype-enrichment scan\n\nStudy: {}. Positive phenotype: {}. {} slides. All eligible cell locations and all declared radii are searched. Disks require at least two cells inside and outside, with at most half the eligible population inside.\n\nThe score sums independent stratum-specific one-sided Bernoulli log-likelihood ratios. Every randomization preserves the positive total within each slide/stratum and repeats the complete search. Probabilities use inclusive ties and the plus-one rule, then Bonferroni across all declared slides. The first canonical center/radius wins exact score ties.\n\nOne winning candidate is reported per slide, whether or not significant. Per-stratum enrichment is unavailable when outside prevalence is zero or support is absent. Constant labels within every stratum and empty search families are unavailable. No secondary-cluster or population inference is performed.\n\nCounts use exact radial membership. Display disks use 128 polygon segments, retain tissue holes and report their maximum chord error. A circle spanning disconnected fragments does not imply tissue continuity or a lesion boundary. These are experimental within-slide enrichment results. See scans.csv, strata.csv, per-slide SVG/GeoJSON and result.json.\n",result.study_id,result.design.positive_phenotype,result.slides.len())));
    publish(result, output, files)
}
