use std::{
    fs,
    path::{Path, PathBuf},
};

use marklab::{
    local_multivariate_moran_permutation, LocalMultivariateMoranLimits,
    LocalMultivariateMoranPoint, LocalMultivariateMoranResult, ObservationWindow2D,
    ObservationWindowLimits,
};

use super::{
    exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError},
    numerics::NumericsCliError,
};

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct PreparedLocalMultivariateMoran {
    pub points: Vec<LocalMultivariateMoranPoint>,
    pub feature_names: Vec<String>,
    pub window: ObservationWindow2D,
    pub source_schema: &'static str,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_direct(
    input: PathBuf,
    window: PathBuf,
    radius_um: f64,
    permutations: usize,
    seed: u64,
    maximum_points: usize,
    maximum_dimension: usize,
    maximum_directed_edges: usize,
    maximum_permutation_edge_evaluations: usize,
    memory_budget_mib: usize,
    out: PathBuf,
) -> Result<(), NumericsCliError> {
    let memory_budget_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| NumericsCliError::Input("--memory-budget-mib is too large".into()))?;
    let prepared = prepare(&input, &window, memory_budget_bytes)?;
    let result = evaluate(
        &prepared,
        radius_um,
        permutations,
        seed,
        maximum_points,
        maximum_dimension,
        maximum_directed_edges,
        maximum_permutation_edge_evaluations,
        memory_budget_bytes,
    )?;
    let result = canonical_result_codec(result)?;
    publish_pretty_json(&out, &result, "local multivariate Moran").map_err(map_output_error)
}

pub(crate) fn canonical_result_codec(
    mut result: LocalMultivariateMoranResult,
) -> Result<LocalMultivariateMoranResult, NumericsCliError> {
    for _ in 0..4 {
        let bytes = serde_json::to_vec(&result)?;
        let next = serde_json::from_slice(&bytes).map_err(NumericsCliError::Json)?;
        if serde_json::to_vec(&next)? == bytes {
            return Ok(next);
        }
        result = next;
    }
    Err(NumericsCliError::Input(
        "local multivariate result did not reach a stable JSON representation".into(),
    ))
}

pub(crate) fn prepare(
    input: &Path,
    window: &Path,
    memory_budget_bytes: usize,
) -> Result<PreparedLocalMultivariateMoran, NumericsCliError> {
    if memory_budget_bytes == 0 {
        return Err(NumericsCliError::Input(
            "--memory-budget-mib must be positive".into(),
        ));
    }
    let input_bytes = read_bounded(input, memory_budget_bytes, "point")?;
    let window_bytes = read_bounded(window, memory_budget_bytes, "window")?;
    if input_bytes.len().saturating_add(window_bytes.len()) > memory_budget_bytes {
        return Err(NumericsCliError::Input(
            "local multivariate sources exceed the retained-memory budget".into(),
        ));
    }
    let (points, feature_names, source_schema) = parse_points(&input_bytes)?;
    let window_text = std::str::from_utf8(&window_bytes)
        .map_err(|_| NumericsCliError::Input("window GeoJSON must be UTF-8".into()))?;
    let window =
        ObservationWindow2D::from_geojson_str(window_text, ObservationWindowLimits::default())
            .map_err(|error| NumericsCliError::Input(error.to_string()))?;
    let value_cells = points
        .len()
        .checked_mul(feature_names.len())
        .and_then(|count| count.checked_mul(std::mem::size_of::<f64>()))
        .ok_or_else(|| NumericsCliError::Input("point matrix size overflowed".into()))?;
    let row_storage = points
        .iter()
        .try_fold(0_usize, |total, point| {
            total.checked_add(
                std::mem::size_of::<LocalMultivariateMoranPoint>()
                    .saturating_add(point.point_id.len())
                    .saturating_add(point.permutation_stratum.len()),
            )
        })
        .ok_or_else(|| NumericsCliError::Input("point storage size overflowed".into()))?;
    let boundary_storage = window
        .descriptor()
        .vertex_count
        .checked_mul(128)
        .ok_or_else(|| NumericsCliError::Input("window storage size overflowed".into()))?;
    let preparation_peak = input_bytes
        .len()
        .checked_add(window_bytes.len())
        .and_then(|value| value.checked_add(value_cells))
        .and_then(|value| value.checked_add(row_storage))
        .and_then(|value| value.checked_add(boundary_storage))
        .ok_or_else(|| NumericsCliError::Input("preparation memory size overflowed".into()))?;
    if preparation_peak > memory_budget_bytes {
        return Err(NumericsCliError::Input(format!(
            "local multivariate preparation-memory estimate {preparation_peak} exceeds budget {memory_budget_bytes}"
        )));
    }
    Ok(PreparedLocalMultivariateMoran {
        points,
        feature_names,
        window,
        source_schema,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate(
    prepared: &PreparedLocalMultivariateMoran,
    radius_um: f64,
    permutations: usize,
    seed: u64,
    maximum_points: usize,
    maximum_dimension: usize,
    maximum_directed_edges: usize,
    maximum_permutation_edge_evaluations: usize,
    memory_budget_bytes: usize,
) -> Result<LocalMultivariateMoranResult, NumericsCliError> {
    local_multivariate_moran_permutation(
        &prepared.points,
        &prepared.feature_names,
        &prepared.window,
        radius_um,
        permutations,
        seed,
        LocalMultivariateMoranLimits {
            maximum_points,
            maximum_dimension,
            maximum_directed_edges,
            maximum_permutation_edge_evaluations,
            memory_budget_bytes,
        },
    )
    .map_err(|error| NumericsCliError::Input(error.to_string()))
}

fn parse_points(
    bytes: &[u8],
) -> Result<(Vec<LocalMultivariateMoranPoint>, Vec<String>, &'static str), NumericsCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader
        .headers()
        .map_err(|error| NumericsCliError::Input(error.to_string()))?
        .clone();
    let generic = headers.len() >= 6
        && headers.get(0) == Some("point_id")
        && headers.get(1) == Some("permutation_stratum")
        && headers.get(2) == Some("x_um")
        && headers.get(3) == Some("y_um");
    let cellvit = headers.len() >= 5
        && headers.get(0) == Some("cell_id")
        && headers.get(1) == Some("x_um")
        && headers.get(2) == Some("y_um")
        && headers
            .iter()
            .skip(3)
            .all(|name| name.starts_with("cellvit_pc_"));
    if !generic && !cellvit {
        return Err(NumericsCliError::Input(
            "point CSV requires either point_id,permutation_stratum,x_um,y_um plus features or canonical cell_id,x_um,y_um,cellvit_pc_* columns".into(),
        ));
    }
    let (x_column, y_column, feature_start, source_schema) = if generic {
        (2, 3, 4, "generic_complete_multivariate_mark_csv_v1")
    } else {
        (1, 2, 3, "canonical_single_slide_cellvit_projection_csv_v1")
    };
    let feature_names = headers
        .iter()
        .skip(feature_start)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut points = Vec::new();
    let mut cellvit_slide = None::<String>;
    for (row, record) in reader.records().enumerate() {
        let record = record.map_err(|error| NumericsCliError::Input(error.to_string()))?;
        let parse = |column: usize| {
            record[column].parse::<f64>().map_err(|_| {
                NumericsCliError::Input(format!(
                    "point CSV row {} column {} is not finite numeric text",
                    row + 2,
                    &headers[column]
                ))
            })
        };
        let values = (feature_start..record.len())
            .map(parse)
            .collect::<Result<Vec<_>, _>>()?;
        let permutation_stratum = if generic {
            record[1].to_owned()
        } else {
            let (slide_id, cell_suffix) = record[0].split_once(':').ok_or_else(|| {
                NumericsCliError::Input(format!(
                    "canonical CellViT row {} cell_id does not contain slide:cell identity",
                    row + 2
                ))
            })?;
            if slide_id.is_empty() || cell_suffix.is_empty() {
                return Err(NumericsCliError::Input(format!(
                    "canonical CellViT row {} cell_id has an empty identity component",
                    row + 2
                )));
            }
            if cellvit_slide
                .as_deref()
                .is_some_and(|expected| expected != slide_id)
            {
                return Err(NumericsCliError::Input(
                    "canonical CellViT local inference requires exactly one slide/window".into(),
                ));
            }
            cellvit_slide.get_or_insert_with(|| slide_id.to_owned());
            slide_id.to_owned()
        };
        points.push(LocalMultivariateMoranPoint {
            point_id: record[0].to_owned(),
            permutation_stratum,
            x_um: parse(x_column)?,
            y_um: parse(y_column)?,
            values,
        });
    }
    Ok((points, feature_names, source_schema))
}

fn read_bounded(
    path: &Path,
    memory_budget_bytes: usize,
    label: &str,
) -> Result<Vec<u8>, NumericsCliError> {
    let metadata = fs::metadata(path).map_err(|source| NumericsCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    let maximum = (MAXIMUM_INPUT_BYTES as usize).min(memory_budget_bytes);
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(NumericsCliError::Input(format!(
            "local multivariate {label} source must be a regular file within {maximum} bytes: {}",
            path.display()
        )));
    }
    fs::read(path).map_err(|source| NumericsCliError::Io {
        path: path.to_owned(),
        source,
    })
}

fn map_output_error(error: ExclusiveJsonOutputError) -> NumericsCliError {
    match error {
        ExclusiveJsonOutputError::OutputExists => {
            NumericsCliError::Input("output already exists".into())
        }
        ExclusiveJsonOutputError::OutputMustNameFile => {
            NumericsCliError::Input("output must name a file".into())
        }
        ExclusiveJsonOutputError::Io { path, source } => NumericsCliError::Io { path, source },
        ExclusiveJsonOutputError::Json(error) => NumericsCliError::Json(error),
    }
}
