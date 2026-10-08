use std::fmt::Write;

use crate::output::xml_text;

use super::result::{NeighborhoodMap, SlideResult};

const LOCAL_COLORS: [(&str, &str); 7] = [
    ("high_high_hotspot", "#b2182b"),
    ("low_low_coldspot", "#2166ac"),
    ("high_low_outlier", "#ef8a62"),
    ("low_high_outlier", "#67a9cf"),
    ("not_significant", "#b8bec6"),
    ("unavailable", "#444444"),
    ("neutral", "#eeeeee"),
];

pub(super) fn local_svg(slide: &SlideResult) -> String {
    let legends = LOCAL_COLORS
        .iter()
        .map(|(label, color)| (label.replace('_', " "), (*color).to_owned()))
        .collect::<Vec<_>>();
    let colors = slide
        .local_map
        .rows
        .iter()
        .map(|row| {
            LOCAL_COLORS
                .iter()
                .find(|(label, _)| *label == row.classification)
                .expect("closed local classification")
                .1
                .to_owned()
        })
        .collect::<Vec<_>>();
    render(
        slide,
        "Local Moran: corrected hotspot / outlier map",
        &legends,
        &colors,
    )
}

pub(super) fn neighborhood_svg(
    slide: &SlideResult,
    map: &NeighborhoodMap,
    phenotypes: &[String],
) -> String {
    let mut legends = phenotypes
        .iter()
        .enumerate()
        .map(|(index, name)| {
            (
                format!("{name} dominant"),
                format!("hsl({},55%,43%)", index * 360 / phenotypes.len()),
            )
        })
        .collect::<Vec<_>>();
    let colors = map
        .rows
        .iter()
        .map(|row| {
            row.dominant_phenotype
                .as_ref()
                .and_then(|name| phenotypes.iter().position(|item| item == name))
                .map(|index| legends[index].1.clone())
                .unwrap_or_else(|| {
                    match row.state {
                        "mixed" => "#7a5195",
                        "boundary_truncated" => "#cccccc",
                        _ => "#444444",
                    }
                    .into()
                })
        })
        .collect::<Vec<_>>();
    legends.extend([
        ("Mixed".into(), "#7a5195".into()),
        ("Window-edge truncated".into(), "#cccccc".into()),
        ("Insufficient neighbors".into(), "#444444".into()),
    ]);
    render(
        slide,
        &format!("Neighborhood composition: radius {} um", map.radius_um),
        &legends,
        &colors,
    )
}

fn render(
    slide: &SlideResult,
    title: &str,
    legends: &[(String, String)],
    colors: &[String],
) -> String {
    let [xmin, ymin, xmax, ymax] = slide.bounds_um;
    let scale = (540.0 / (xmax - xmin)).min(540.0 / (ymax - ymin));
    let height = (125 + legends.len() * 20).max(660);
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 {height}" role="img"><title>{}</title><rect width="1000" height="{height}" fill="white"/><g font-family="sans-serif" fill="#242424"><text x="30" y="28" font-size="18">{}</text><text x="30" y="50" font-size="12">{} / {} | coordinates in micrometres, source orientation</text></g><g transform="translate(30 80) scale({scale}) translate({} {})">"##,
        xml_text(title),
        xml_text(title),
        xml_text(&slide.patient_id),
        xml_text(&slide.slide_id),
        -xmin,
        -ymin
    );
    path(&mut svg, &slide.window, "#f5f5f3", "#555555", 1.0 / scale);
    for profile in &slide.profiles {
        path(&mut svg, &profile.geometry, "none", "#999999", 1.0 / scale);
    }
    let radius = (300.0 / (slide.local_map.rows.len() as f64).sqrt()).clamp(0.6, 3.0) / scale;
    for (row, color) in slide.local_map.rows.iter().zip(colors) {
        write!(
            svg,
            r#"<circle cx="{}" cy="{}" r="{radius}" fill="{color}"><title>{}</title></circle>"#,
            row.x_um,
            row.y_um,
            xml_text(&row.cell_id)
        )
        .expect("String write");
    }
    svg.push_str("</g><g font-family=\"sans-serif\" font-size=\"12\">");
    for (index, (label, color)) in legends.iter().enumerate() {
        let y = 95 + index * 20;
        write!(
            svg,
            r#"<circle cx="610" cy="{y}" r="4" fill="{color}"/><text x="625" y="{}">{}</text>"#,
            y + 4,
            xml_text(label)
        )
        .expect("String write");
    }
    write!(svg, r#"<text x="30" y="{}">Experimental within-slide map. See result.json for support, probabilities and claim limits.</text></g></svg>"#, height - 20).expect("String write");
    svg
}

fn path(svg: &mut String, value: &serde_json::Value, fill: &str, stroke: &str, width: f64) {
    let geometry = value.get("geometry").unwrap_or(value);
    let geometry = geometry
        .get("features")
        .and_then(|features| features.get(0))
        .and_then(|feature| feature.get("geometry"))
        .unwrap_or(geometry);
    let mut commands = String::new();
    for polygon in geometry["coordinates"]
        .as_array()
        .expect("admitted MultiPolygon")
    {
        for ring in polygon.as_array().expect("admitted polygon") {
            for (index, point) in ring.as_array().expect("admitted ring").iter().enumerate() {
                write!(
                    commands,
                    "{}{} {} ",
                    if index == 0 { 'M' } else { 'L' },
                    point[0].as_f64().expect("admitted x"),
                    point[1].as_f64().expect("admitted y")
                )
                .expect("String write");
            }
            commands.push('Z');
        }
    }
    write!(svg, r#"<path d="{commands}" fill="{fill}" fill-rule="evenodd" stroke="{stroke}" stroke-width="{width}"/>"#).expect("String write");
}
