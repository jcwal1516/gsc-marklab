use super::super::*;

pub(in crate::cli) fn run(request: MultimodalAnalyzeRequest) -> Result<()> {
    let MultimodalAnalyzeRequest {
        he_cells,
        ihc_cells,
        landmarks,
        config,
        out,
        case_id,
        timepoint,
        protein,
        he_format,
        cellvit_min_probability,
    } = request;
    let config = AnalysisConfig::from_toml_path(&config)?;
    let engine = MultimodalEngine::new(config.clone())?;
    #[cfg(not(feature = "parquet"))]
    if config.output.write_parquet_curves {
        bail!("Multimodal parquet output requires the parquet feature");
    }

    let he = match he_format {
        HeInputFormat::HeCsv => load_he_cell_table_csv(&he_cells)?,
        HeInputFormat::CellvitCsv => {
            load_cellvit_he_cell_table_csv(&he_cells, cellvit_min_probability)?
        }
    };
    let ihc = load_ihc_cell_table_csv(&ihc_cells)?;
    let landmarks = read_landmark_pairs(&landmarks)?;
    let result = engine.analyze(&MultimodalInput {
        he_cells: he,
        ihc_cells: ihc,
        landmarks: landmarks.clone(),
        case_id,
        timepoint,
        protein,
    })?;
    let transform = match config.registration.transform {
        RegistrationTransform::Affine => fit_affine(&landmarks)?,
        RegistrationTransform::Rigid => fit_similarity(&landmarks)?,
    };
    let fused = result.fused_cells.clone();
    let graph = build_spatial_graph(
        &fused,
        GraphConfig {
            radius_um: Some(config.neighborhood.radius_um),
            k_nearest: nonzero_option(config.neighborhood.k_nearest),
        },
    )?;
    let label_pairs = config
        .neighborhood
        .label_pairs
        .iter()
        .map(|pair| LabelPair::new(pair[0].clone(), pair[1].clone()))
        .collect::<Vec<_>>();
    let null_model_sensitivity = null_model_sensitivity_results(
        &fused,
        &graph,
        &label_pairs,
        &config.neighborhood.null_models,
        config.permutation.b,
        config.permutation.seed,
    )?;
    OutputWriter::write(
        &ResultDocument::multimodal(result.clone()),
        &out,
        &config.output,
    )?;
    write_registration_qc_sidecars(&out, &landmarks, &transform, &fused)?;
    write_pretty_json(
        &out.join("null_model_sensitivity.json"),
        &null_model_sensitivity,
    )?;
    write_multimodal_csv_sidecars(&out, &result, &null_model_sensitivity)?;
    Ok(())
}

pub(super) fn write_pretty_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

fn write_registration_qc_sidecars(
    out: &Path,
    landmarks: &[LandmarkPair],
    transform: &crate::registration::transform::Transform2D,
    fused: &[FusedCell],
) -> Result<()> {
    let residuals = registration_residual_records(landmarks, transform);
    write_pretty_json(&out.join("registration_residuals.json"), &residuals)?;
    write_csv_records(&out.join("registration_residuals.csv"), &residuals)?;
    write_pretty_json(
        &out.join("registration_transform.json"),
        &serde_json::json!({
            "transform_type": transform.transform_type,
            "matrix": [
                [transform.m00, transform.m01, transform.m02],
                [transform.m10, transform.m11, transform.m12]
            ]
        }),
    )?;

    let extrapolation = cell_extrapolation_records(landmarks, fused);
    let outside = extrapolation
        .iter()
        .filter(|record| record.outside_landmark_hull)
        .count();
    write_csv_records(&out.join("registration_extrapolation.csv"), &extrapolation)?;
    write_pretty_json(
        &out.join("registration_extrapolation.json"),
        &serde_json::json!({
            "n_cells": extrapolation.len(),
            "n_outside_landmark_hull": outside,
            "fraction_outside_landmark_hull": if extrapolation.is_empty() {
                0.0
            } else {
                outside as f64 / extrapolation.len() as f64
            },
            "cell_flags": extrapolation,
        }),
    )?;
    Ok(())
}

fn registration_residual_records(
    landmarks: &[LandmarkPair],
    transform: &crate::registration::transform::Transform2D,
) -> Vec<RegistrationResidualRecord> {
    landmarks
        .iter()
        .enumerate()
        .map(|(index, landmark)| {
            let (transformed_x_um, transformed_y_um) =
                transform.apply(landmark.source_x_um, landmark.source_y_um);
            let residual_dx_um = transformed_x_um - landmark.target_x_um;
            let residual_dy_um = transformed_y_um - landmark.target_y_um;
            RegistrationResidualRecord {
                landmark_index: index,
                source_x_um: landmark.source_x_um,
                source_y_um: landmark.source_y_um,
                target_x_um: landmark.target_x_um,
                target_y_um: landmark.target_y_um,
                transformed_x_um,
                transformed_y_um,
                residual_dx_um,
                residual_dy_um,
                residual_um: residual_dx_um.hypot(residual_dy_um),
            }
        })
        .collect()
}

fn cell_extrapolation_records(
    landmarks: &[LandmarkPair],
    fused: &[FusedCell],
) -> Vec<CellExtrapolationRecord> {
    let hull = convex_hull(
        &landmarks
            .iter()
            .map(|landmark| Point2 {
                x: landmark.target_x_um,
                y: landmark.target_y_um,
            })
            .collect::<Vec<_>>(),
    );
    fused
        .iter()
        .map(|cell| {
            let point = Point2 {
                x: cell.x_um_registered,
                y: cell.y_um_registered,
            };
            CellExtrapolationRecord {
                source_section: match cell.source_section {
                    CellSection::He => "he".into(),
                    CellSection::Ihc => "ihc".into(),
                },
                source_cell_id: cell.source_cell_id.clone(),
                x_um_registered: cell.x_um_registered,
                y_um_registered: cell.y_um_registered,
                outside_landmark_hull: !point_in_hull(point, &hull),
            }
        })
        .collect()
}

fn convex_hull(points: &[Point2]) -> Vec<Point2> {
    let mut points = points.to_vec();
    points.sort_by(|left, right| {
        left.x
            .total_cmp(&right.x)
            .then_with(|| left.y.total_cmp(&right.y))
    });
    points.dedup_by(|left, right| left.x == right.x && left.y == right.y);
    if points.len() <= 1 {
        return points;
    }

    let mut lower = Vec::new();
    for point in &points {
        while lower.len() >= 2
            && cross(lower[lower.len() - 2], lower[lower.len() - 1], *point) <= 0.0
        {
            lower.pop();
        }
        lower.push(*point);
    }
    let mut upper = Vec::new();
    for point in points.iter().rev() {
        while upper.len() >= 2
            && cross(upper[upper.len() - 2], upper[upper.len() - 1], *point) <= 0.0
        {
            upper.pop();
        }
        upper.push(*point);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

fn cross(origin: Point2, left: Point2, right: Point2) -> f64 {
    (left.x - origin.x) * (right.y - origin.y) - (left.y - origin.y) * (right.x - origin.x)
}

fn point_in_hull(point: Point2, hull: &[Point2]) -> bool {
    if hull.len() < 3 {
        return true;
    }
    let mut sign = 0_i8;
    for index in 0..hull.len() {
        let left = hull[index];
        let right = hull[(index + 1) % hull.len()];
        let value = cross(left, right, point);
        if value.abs() <= 1.0e-9 {
            continue;
        }
        let current_sign = if value > 0.0 { 1 } else { -1 };
        if sign == 0 {
            sign = current_sign;
        } else if sign != current_sign {
            return false;
        }
    }
    true
}

fn write_multimodal_csv_sidecars(
    out: &Path,
    result: &MultimodalResult,
    null_model_sensitivity: &[NullModelSensitivityResult],
) -> Result<()> {
    write_csv_records(&out.join("fused_cells.csv"), &result.fused_cells)?;
    if let Some(territories) = result
        .neighborhood_territories
        .value()
        .filter(|value| !value.is_empty())
    {
        write_csv_records(&out.join("neighborhood_territories.csv"), territories)?;
    }
    if let Some(comparisons) = result
        .territory_comparisons
        .value()
        .filter(|value| !value.is_empty())
    {
        write_csv_records(&out.join("territory_comparisons.csv"), comparisons)?;
    }
    if let Some(enrichment) = result
        .neighborhood_enrichment
        .value()
        .filter(|value| !value.is_empty())
    {
        write_csv_records(&out.join("neighborhood_enrichment.csv"), enrichment)?;
    }
    if result
        .cross_interaction_curves
        .value()
        .is_some_and(|value| !value.is_empty())
    {
        write_cross_interaction_curves_csv(&out.join("cross_interaction_curves.csv"), result)?;
    }
    if result
        .territory_profiles
        .value()
        .is_some_and(|value| !value.is_empty())
    {
        write_territory_profiles_csv(&out.join("territory_profiles.csv"), result)?;
    }
    write_null_model_sensitivity_csv(
        &out.join("null_model_sensitivity.csv"),
        null_model_sensitivity,
    )?;
    Ok(())
}

fn write_csv_records<T: Serialize>(path: &Path, records: &[T]) -> Result<()> {
    let mut writer = csv::Writer::from_path(path)?;
    for record in records {
        writer.serialize(record)?;
    }
    writer.flush()?;
    Ok(())
}

fn write_cross_interaction_curves_csv(path: &Path, result: &MultimodalResult) -> Result<()> {
    let mut writer = csv::Writer::from_path(path)?;
    writer.write_record([
        "label_a",
        "label_b",
        "r_min_um",
        "r_max_um",
        "value",
        "lower_global_envelope",
        "upper_global_envelope",
        "count",
        "p_global",
    ])?;
    if let Some(curves) = result.cross_interaction_curves.value() {
        for curve in curves {
            for point in &curve.points {
                writer.serialize((
                    &curve.label_a,
                    &curve.label_b,
                    point.r_min_um,
                    point.r_max_um,
                    point.value,
                    point.lower_global_envelope,
                    point.upper_global_envelope,
                    point.count,
                    curve.p_global,
                ))?;
            }
        }
    }
    writer.flush()?;
    Ok(())
}

fn write_territory_profiles_csv(path: &Path, result: &MultimodalResult) -> Result<()> {
    let mut writer = csv::Writer::from_path(path)?;
    writer.write_record([
        "territory_id",
        "label",
        "fraction",
        "count",
        "below_registration_resolution",
    ])?;
    if let Some(profiles) = result.territory_profiles.value() {
        for profile in profiles {
            for fraction in &profile.cell_type_fractions {
                writer.serialize((
                    profile.territory_id,
                    &fraction.label,
                    fraction.fraction,
                    fraction.count,
                    profile.below_registration_resolution,
                ))?;
            }
        }
    }
    writer.flush()?;
    Ok(())
}

fn write_null_model_sensitivity_csv(
    path: &Path,
    sensitivity: &[NullModelSensitivityResult],
) -> Result<()> {
    let mut writer = csv::Writer::from_path(path)?;
    writer.write_record([
        "null_model",
        "label_a",
        "label_b",
        "observed_edges",
        "expected_edges",
        "enrichment_ratio",
        "z_score",
        "p_value",
        "q_value",
    ])?;
    for model in sensitivity {
        for row in &model.results {
            writer.serialize((
                &model.null_model,
                &row.label_a,
                &row.label_b,
                row.observed_edges,
                row.expected_edges,
                row.enrichment_ratio,
                row.z_score,
                row.p_value,
                row.q_value,
            ))?;
        }
    }
    writer.flush()?;
    Ok(())
}

fn null_model_sensitivity_results(
    fused: &[FusedCell],
    graph: &SpatialGraph,
    label_pairs: &[LabelPair],
    null_models: &[NeighborhoodNullModel],
    permutations: usize,
    seed: u64,
) -> Result<Vec<NullModelSensitivityResult>> {
    null_models
        .iter()
        .map(|model| {
            let (name, results) = match model {
                NeighborhoodNullModel::SourceSection => (
                    "source_section",
                    edge_enrichment(fused, graph, label_pairs, permutations, seed)?,
                ),
                NeighborhoodNullModel::SourceSectionDensity => (
                    "source_section_density",
                    edge_enrichment_with_strata(
                        fused,
                        graph,
                        label_pairs,
                        permutations,
                        seed,
                        &source_section_density_strata(fused, graph),
                    )?,
                ),
                NeighborhoodNullModel::SourceSectionCellClass => (
                    "source_section_cell_class",
                    edge_enrichment_with_strata(
                        fused,
                        graph,
                        label_pairs,
                        permutations,
                        seed,
                        &source_section_cell_class_strata(fused),
                    )?,
                ),
                NeighborhoodNullModel::SourceSectionRegistrationQc => (
                    "source_section_registration_qc",
                    edge_enrichment_with_strata(
                        fused,
                        graph,
                        label_pairs,
                        permutations,
                        seed,
                        &source_section_registration_qc_strata(fused, graph),
                    )?,
                ),
            };
            Ok(NullModelSensitivityResult {
                null_model: name.into(),
                results,
            })
        })
        .collect()
}

fn source_section_density_strata(fused: &[FusedCell], graph: &SpatialGraph) -> Vec<String> {
    let degrees = graph_degrees(fused.len(), graph);
    let mut sorted = degrees.clone();
    sorted.sort_unstable();
    let median_degree = sorted.get(sorted.len() / 2).copied().unwrap_or(0);
    fused
        .iter()
        .enumerate()
        .map(|(index, cell)| {
            format!(
                "{}:{}",
                section_name(cell.source_section),
                if degrees[index] <= median_degree {
                    "low_density"
                } else {
                    "high_density"
                }
            )
        })
        .collect()
}

fn source_section_cell_class_strata(fused: &[FusedCell]) -> Vec<String> {
    fused
        .iter()
        .map(|cell| match cell.source_section {
            CellSection::He => format!(
                "he:{}",
                primary_label(cell).unwrap_or_else(|| "unknown".into())
            ),
            CellSection::Ihc => "ihc:mmr_status".into(),
        })
        .collect()
}

fn source_section_registration_qc_strata(fused: &[FusedCell], graph: &SpatialGraph) -> Vec<String> {
    let mut below_resolution_incident = vec![false; fused.len()];
    for edge in &graph.edges {
        if edge.below_registration_resolution {
            below_resolution_incident[edge.source] = true;
            below_resolution_incident[edge.target] = true;
        }
    }
    fused
        .iter()
        .enumerate()
        .map(|(index, cell)| {
            format!(
                "{}:{}",
                section_name(cell.source_section),
                if below_resolution_incident[index] {
                    "below_resolution_edge"
                } else {
                    "above_resolution_edges"
                }
            )
        })
        .collect()
}

fn graph_degrees(n_cells: usize, graph: &SpatialGraph) -> Vec<usize> {
    let mut degrees = vec![0usize; n_cells];
    for edge in &graph.edges {
        degrees[edge.source] += 1;
        degrees[edge.target] += 1;
    }
    degrees
}

fn section_name(section: CellSection) -> &'static str {
    match section {
        CellSection::He => "he",
        CellSection::Ihc => "ihc",
    }
}

fn read_landmark_pairs(path: &Path) -> Result<Vec<LandmarkPair>> {
    let mut reader = ::csv::Reader::from_path(path)?;
    let headers = reader.headers()?.clone();
    let expected_headers = ["source_x_um", "source_y_um", "target_x_um", "target_y_um"];
    if headers.iter().collect::<Vec<_>>() != expected_headers {
        bail!(
            "{}: expected landmark CSV headers source_x_um,source_y_um,target_x_um,target_y_um",
            path.display()
        );
    }

    let mut landmarks = Vec::new();
    for (index, row) in reader.deserialize::<LandmarkRow>().enumerate() {
        let row_number = index + 2;
        let row = row.map_err(|err| {
            MmrspaceError::Validation(format!(
                "{} row {}: invalid landmark row: {}",
                path.display(),
                row_number,
                err
            ))
        })?;
        if !row.source_x_um.is_finite()
            || !row.source_y_um.is_finite()
            || !row.target_x_um.is_finite()
            || !row.target_y_um.is_finite()
        {
            bail!(
                "{} row {}: landmark coordinates must be finite",
                path.display(),
                row_number
            );
        }
        landmarks.push(LandmarkPair::new(
            row.source_x_um,
            row.source_y_um,
            row.target_x_um,
            row.target_y_um,
        ));
    }
    Ok(landmarks)
}

fn nonzero_option(value: usize) -> Option<usize> {
    if value == 0 {
        None
    } else {
        Some(value)
    }
}
