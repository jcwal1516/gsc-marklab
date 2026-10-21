use std::path::{Path, PathBuf};

use marklab::{
    autocorrelation::{
        local_multivariate_moran_permutation, LocalMultivariateMoranLimits,
        LocalMultivariateMoranPoint, LocalMultivariateMoranResult,
    },
    ObservationWindow2D, ObservationWindowLimits,
};

use super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError};
use crate::cli_error::CliError;

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct PreparedLocalMultivariateMoran {
    pub points: Vec<LocalMultivariateMoranPoint>,
    pub feature_names: Vec<String>,
    pub window: ObservationWindow2D,
    pub source_schema: &'static str,
}

#[derive(Debug, clap::Args)]
pub(crate) struct Arguments {
    /// CSV: point_id, permutation_stratum, x_um, y_um, marks; or cell_id, x_um, y_um, cellvit_pc_*.
    #[arg(long)]
    input: PathBuf,
    /// Observation-window geometry used for spatial integration or edge correction.
    #[arg(long)]
    window: PathBuf,
    /// Spatial radius in micrometers.
    #[arg(long)]
    radius_um: f64,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    permutations: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    seed: u64,
    /// Upper bound on points.
    #[arg(long)]
    maximum_points: usize,
    /// Upper bound on dimension.
    #[arg(long)]
    maximum_dimension: usize,
    /// Upper bound on directed edges.
    #[arg(long)]
    maximum_directed_edges: usize,
    /// Upper bound on permutation edge evaluations.
    #[arg(long)]
    maximum_permutation_edge_evaluations: usize,
    /// Maximum working memory in mebibytes.
    #[arg(long)]
    memory_budget_mib: usize,
    /// Destination for the analysis output.
    #[arg(long)]
    out: PathBuf,
}

pub(crate) fn run_direct(args: Arguments) -> Result<(), CliError> {
    let Arguments {
        input,
        window,
        radius_um,
        permutations,
        seed,
        maximum_points,
        maximum_dimension,
        maximum_directed_edges,
        maximum_permutation_edge_evaluations,
        memory_budget_mib,
        out,
    } = args;
    let memory_budget_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| CliError::Input("--memory-budget-mib is too large".into()))?;
    let prepared = prepare(&input, &window, memory_budget_bytes)?;
    let result = evaluate(
        &prepared,
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
    )?;
    let result = canonical_result_codec(result)?;
    publish_pretty_json(&out, &result, "local multivariate Moran").map_err(map_output_error)
}

pub(crate) fn canonical_result_codec(
    mut result: LocalMultivariateMoranResult,
) -> Result<LocalMultivariateMoranResult, CliError> {
    for _ in 0..4 {
        let bytes = serde_json::to_vec(&result)?;
        let next = serde_json::from_slice(&bytes).map_err(CliError::Json)?;
        if serde_json::to_vec(&next)? == bytes {
            return Ok(next);
        }
        result = next;
    }
    Err(CliError::Input(
        "local multivariate result did not reach a stable JSON representation".into(),
    ))
}

pub(crate) fn prepare(
    input: &Path,
    window: &Path,
    memory_budget_bytes: usize,
) -> Result<PreparedLocalMultivariateMoran, CliError> {
    if memory_budget_bytes == 0 {
        return Err(CliError::Input(
            "--memory-budget-mib must be positive".into(),
        ));
    }
    let input_bytes = read_bounded(input, memory_budget_bytes, "point")?;
    let window_bytes = read_bounded(window, memory_budget_bytes, "window")?;
    if input_bytes.len().saturating_add(window_bytes.len()) > memory_budget_bytes {
        return Err(CliError::Input(
            "local multivariate sources exceed the retained-memory budget".into(),
        ));
    }
    let (points, feature_names, source_schema) = parse_points(&input_bytes)?;
    let window_text = std::str::from_utf8(&window_bytes)
        .map_err(|_| CliError::Input("window GeoJSON must be UTF-8".into()))?;
    let window =
        ObservationWindow2D::from_geojson_str(window_text, ObservationWindowLimits::default())
            .map_err(|error| CliError::Input(error.to_string()))?;
    let value_cells = points
        .len()
        .checked_mul(feature_names.len())
        .and_then(|count| count.checked_mul(std::mem::size_of::<f64>()))
        .ok_or_else(|| CliError::Input("point matrix size overflowed".into()))?;
    let row_storage = points
        .iter()
        .try_fold(0_usize, |total, point| {
            total.checked_add(
                std::mem::size_of::<LocalMultivariateMoranPoint>()
                    .saturating_add(point.point_id.len())
                    .saturating_add(point.permutation_stratum.len()),
            )
        })
        .ok_or_else(|| CliError::Input("point storage size overflowed".into()))?;
    let boundary_storage = window
        .descriptor()
        .vertex_count
        .checked_mul(128)
        .ok_or_else(|| CliError::Input("window storage size overflowed".into()))?;
    let preparation_peak = input_bytes
        .len()
        .checked_add(window_bytes.len())
        .and_then(|value| value.checked_add(value_cells))
        .and_then(|value| value.checked_add(row_storage))
        .and_then(|value| value.checked_add(boundary_storage))
        .ok_or_else(|| CliError::Input("preparation memory size overflowed".into()))?;
    if preparation_peak > memory_budget_bytes {
        return Err(CliError::Input(format!(
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

pub(crate) fn evaluate(
    prepared: &PreparedLocalMultivariateMoran,
    radius_um: f64,
    permutations: usize,
    seed: u64,
    limits: LocalMultivariateMoranLimits,
) -> Result<LocalMultivariateMoranResult, CliError> {
    local_multivariate_moran_permutation(
        &prepared.points,
        &prepared.feature_names,
        &prepared.window,
        radius_um,
        permutations,
        seed,
        limits,
    )
    .map_err(|error| CliError::Input(error.to_string()))
}

fn parse_points(
    bytes: &[u8],
) -> Result<(Vec<LocalMultivariateMoranPoint>, Vec<String>, &'static str), CliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader
        .headers()
        .map_err(|error| CliError::Input(error.to_string()))?
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
        return Err(CliError::Input(
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
        let record = record.map_err(|error| CliError::Input(error.to_string()))?;
        let parse = |column: usize| {
            record[column].parse::<f64>().map_err(|_| {
                CliError::Input(format!(
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
                CliError::Input(format!(
                    "canonical CellViT row {} cell_id does not contain slide:cell identity",
                    row + 2
                ))
            })?;
            if slide_id.is_empty() || cell_suffix.is_empty() {
                return Err(CliError::Input(format!(
                    "canonical CellViT row {} cell_id has an empty identity component",
                    row + 2
                )));
            }
            if cellvit_slide
                .as_deref()
                .is_some_and(|expected| expected != slide_id)
            {
                return Err(CliError::Input(
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

fn read_bounded(path: &Path, memory_budget_bytes: usize, label: &str) -> Result<Vec<u8>, CliError> {
    let maximum = (MAXIMUM_INPUT_BYTES as usize).min(memory_budget_bytes);
    crate::input_file::read_regular_file(
        path,
        maximum as u64,
        &format!(
            "local multivariate {label} source must be a regular file within {maximum} bytes: {}",
            path.display()
        ),
    )
}

fn map_output_error(error: ExclusiveJsonOutputError) -> CliError {
    match error {
        ExclusiveJsonOutputError::OutputExists => CliError::Input("output already exists".into()),
        ExclusiveJsonOutputError::OutputMustNameFile => {
            CliError::Input("output must name a file".into())
        }
        ExclusiveJsonOutputError::Io { path, source } => CliError::Io { path, source },
        ExclusiveJsonOutputError::Json(error) => CliError::Json(error),
    }
}
