use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::categorical_input::{
    prepare_categorical_input, CategoricalCellView, CategoricalChannelView, CategoricalInputError,
    CategoricalInputLimits, PreparedCategoricalInput,
};

pub const CATEGORICAL_COOCCURRENCE_FORMAT: &str = "marklab.categorical_cooccurrence_result";
pub const CATEGORICAL_COOCCURRENCE_VERSION: u32 = 1;
const MAXIMUM_INPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    format: String,
    version: u32,
    coordinate_frame_id: String,
    coordinate_unit: String,
    window: serde_json::Value,
    channel: CategoricalCooccurrenceChannel,
    cells: Vec<CategoricalCooccurrenceCell>,
    radii_um: Vec<f64>,
    limits: RequestLimits,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestLimits {
    maximum_input_bytes: usize,
    maximum_points: usize,
    maximum_classes: usize,
    maximum_radii: usize,
    maximum_output_matrix_bytes: usize,
    maximum_directed_pair_visits: usize,
    maximum_retained_bytes: usize,
}

/// One categorical cell row with coordinates in the declared physical frame.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CategoricalCooccurrenceCell {
    pub id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub class_id: String,
}

/// One ordered categorical channel declaration.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CategoricalCooccurrenceChannel {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub unit: String,
    pub measurement_status: String,
    pub provenance: String,
    pub classes: Vec<String>,
}

/// Explicit input, output, work, and retained-memory ceilings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CategoricalCooccurrenceLimits {
    pub maximum_points: usize,
    pub maximum_classes: usize,
    pub maximum_radii: usize,
    pub maximum_output_matrix_bytes: usize,
    pub maximum_directed_pair_visits: usize,
    pub maximum_retained_bytes: usize,
}

/// Availability of one directed class-pair co-occurrence ratio.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CategoricalCooccurrenceStatus {
    Available,
    UnavailableZeroEdgeIncidence,
}

/// One source-major cell of a categorical co-occurrence matrix.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CategoricalCooccurrenceEntry {
    pub source_class_id: String,
    pub target_class_id: String,
    pub directed_pair_count: usize,
    pub source_edge_incidence: usize,
    pub target_edge_incidence: usize,
    pub status: CategoricalCooccurrenceStatus,
    pub score: Option<f64>,
}

/// One cumulative directed class-pair matrix at a physical radius.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CategoricalCooccurrenceMatrix {
    pub radius_um: f64,
    pub total_directed_pairs: usize,
    pub entries: Vec<CategoricalCooccurrenceEntry>,
}

/// Complete bounded categorical co-occurrence result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CategoricalCooccurrenceResult {
    pub format: String,
    pub version: u32,
    pub coordinate_frame_id: String,
    pub channel_id: String,
    pub classes: Vec<String>,
    pub point_count: usize,
    pub class_count: usize,
    pub maximum_radius_directed_pair_visits: usize,
    pub matrices: Vec<CategoricalCooccurrenceMatrix>,
}

/// Rejected categorical input, resource request, geometry, or numerical result.
#[derive(Debug, Error, PartialEq)]
pub enum CategoricalCooccurrenceError {
    #[error("invalid categorical co-occurrence input: {0}")]
    Invalid(String),
    #[error("categorical co-occurrence resource limit exceeded: {0}")]
    Resource(String),
    #[error("categorical co-occurrence geometry failed: {0}")]
    Geometry(String),
    #[error("categorical co-occurrence numerical failure: {0}")]
    Numerical(String),
}

/// Compute cumulative directed categorical co-occurrence at fixed physical radii.
pub fn categorical_cooccurrence(
    coordinate_frame_id: &str,
    coordinate_unit: &str,
    window_geojson: &str,
    channel: &CategoricalCooccurrenceChannel,
    cells: &[CategoricalCooccurrenceCell],
    radii_um: &[f64],
    limits: CategoricalCooccurrenceLimits,
) -> Result<CategoricalCooccurrenceResult, CategoricalCooccurrenceError> {
    let decoded_request_bytes = estimate_decoded_input_bytes(
        coordinate_frame_id,
        coordinate_unit,
        window_geojson,
        channel,
        cells,
        radii_um,
    )?;
    categorical_cooccurrence_with_retained_input(
        coordinate_frame_id,
        coordinate_unit,
        window_geojson,
        channel,
        cells,
        radii_um,
        limits,
        0,
        decoded_request_bytes,
    )
}

/// Decode and analyze one strict version-1 categorical co-occurrence request.
pub fn categorical_cooccurrence_from_json(
    input: &[u8],
) -> Result<CategoricalCooccurrenceResult, CategoricalCooccurrenceError> {
    if input.len() > MAXIMUM_INPUT_BYTES {
        return Err(resource("categorical co-occurrence input exceeds 16 MiB"));
    }
    let request: Request = serde_json::from_slice(input)
        .map_err(|error| invalid(format!("invalid categorical co-occurrence JSON: {error}")))?;
    let Request {
        format,
        version,
        coordinate_frame_id,
        coordinate_unit,
        window,
        channel,
        cells,
        radii_um,
        limits,
    } = request;
    if format != "marklab.categorical_cooccurrence" || version != 1 {
        return Err(invalid(
            "format and version must be marklab.categorical_cooccurrence version 1",
        ));
    }
    if limits.maximum_input_bytes == 0
        || limits.maximum_input_bytes > MAXIMUM_INPUT_BYTES
        || input.len() > limits.maximum_input_bytes
    {
        return Err(resource(format!(
            "input has {} bytes; declared maximum is {} and hard maximum is {MAXIMUM_INPUT_BYTES}",
            input.len(),
            limits.maximum_input_bytes
        )));
    }
    let decoded_request_bytes = input
        .len()
        .checked_mul(64)
        .ok_or_else(|| resource("decoded request size overflowed"))?;
    let window_geojson = serde_json::to_string(&window)
        .map_err(|error| invalid(format!("window JSON could not be encoded: {error}")))?;
    drop(window);
    categorical_cooccurrence_with_retained_input(
        &coordinate_frame_id,
        &coordinate_unit,
        &window_geojson,
        &channel,
        &cells,
        &radii_um,
        CategoricalCooccurrenceLimits {
            maximum_points: limits.maximum_points,
            maximum_classes: limits.maximum_classes,
            maximum_radii: limits.maximum_radii,
            maximum_output_matrix_bytes: limits.maximum_output_matrix_bytes,
            maximum_directed_pair_visits: limits.maximum_directed_pair_visits,
            maximum_retained_bytes: limits.maximum_retained_bytes,
        },
        input.len(),
        decoded_request_bytes,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "the strict JSON adapter keeps both raw and decoded retained bytes explicit"
)]
fn categorical_cooccurrence_with_retained_input(
    coordinate_frame_id: &str,
    coordinate_unit: &str,
    window_geojson: &str,
    channel: &CategoricalCooccurrenceChannel,
    cells: &[CategoricalCooccurrenceCell],
    radii_um: &[f64],
    limits: CategoricalCooccurrenceLimits,
    input_file_bytes: usize,
    decoded_request_bytes: usize,
) -> Result<CategoricalCooccurrenceResult, CategoricalCooccurrenceError> {
    validate_limits(limits)?;
    validate_radii(radii_um, limits.maximum_radii)?;
    let prepared = prepare_categorical_input(
        coordinate_frame_id,
        coordinate_unit,
        window_geojson,
        CategoricalChannelView {
            id: &channel.id,
            label: &channel.label,
            kind: &channel.kind,
            unit: &channel.unit,
            measurement_status: &channel.measurement_status,
            provenance: &channel.provenance,
            classes: &channel.classes,
        },
        cells.iter().map(|cell| CategoricalCellView {
            id: &cell.id,
            x_um: cell.x_um,
            y_um: cell.y_um,
            class_id: &cell.class_id,
        }),
        CategoricalInputLimits {
            maximum_points: limits.maximum_points,
            maximum_classes: limits.maximum_classes,
            maximum_retained_bytes: limits.maximum_retained_bytes,
        },
        input_file_bytes,
        decoded_request_bytes,
    )
    .map_err(map_input_error)?;
    analyze_prepared(&prepared, radii_um, limits, decoded_request_bytes)
}

fn analyze_prepared(
    prepared: &PreparedCategoricalInput,
    radii_um: &[f64],
    limits: CategoricalCooccurrenceLimits,
    decoded_request_bytes: usize,
) -> Result<CategoricalCooccurrenceResult, CategoricalCooccurrenceError> {
    let class_count = prepared.classes.len();
    let matrix_len = class_count
        .checked_mul(class_count)
        .ok_or_else(|| resource("class matrix size overflowed"))?;
    let count_len = matrix_len
        .checked_mul(radii_um.len())
        .ok_or_else(|| resource("radius matrix size overflowed"))?;
    let count_bytes = count_len
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(|| resource("pair-count storage size overflowed"))?;
    let output_matrix_bytes = estimate_output_matrix_bytes(&prepared.classes, radii_um.len())?;
    if output_matrix_bytes > limits.maximum_output_matrix_bytes {
        return Err(resource(format!(
            "output matrix estimate {output_matrix_bytes} exceeds maximum {}",
            limits.maximum_output_matrix_bytes
        )));
    }
    let result_overhead_bytes = estimate_result_overhead_bytes(prepared)?;
    let incidence_scratch_bytes = class_count
        .checked_mul(2 * std::mem::size_of::<usize>())
        .ok_or_else(|| resource("edge-incidence scratch size overflowed"))?;
    let retained_peak = sum_bytes(&[
        prepared.input_file_bytes,
        decoded_request_bytes,
        prepared.estimated_storage_bytes,
        count_bytes,
        output_matrix_bytes,
        result_overhead_bytes,
        incidence_scratch_bytes,
    ])?;
    if retained_peak > limits.maximum_retained_bytes {
        return Err(resource(format!(
            "retained-memory estimate {retained_peak} exceeds maximum {}",
            limits.maximum_retained_bytes
        )));
    }

    let mut counts = Vec::new();
    counts
        .try_reserve_exact(count_len)
        .map_err(|_| resource("pair-count allocation failed"))?;
    counts.resize(count_len, 0_usize);
    let maximum_radius = *radii_um.last().expect("validated nonempty radii");
    let mut directed_pair_visits = 0_usize;
    for source in 0..prepared.class_indices.len() {
        let source_class = prepared.class_indices[source];
        let mut visit_error = None;
        prepared
            .index
            .visit_within_radius(source, maximum_radius, |neighbor| {
                if visit_error.is_some() {
                    return;
                }
                let Some(next_visits) = directed_pair_visits.checked_add(1) else {
                    visit_error = Some(resource("directed-pair visit count overflowed"));
                    return;
                };
                directed_pair_visits = next_visits;
                if directed_pair_visits > limits.maximum_directed_pair_visits {
                    visit_error = Some(resource(format!(
                        "directed-pair visits {directed_pair_visits} exceed maximum {}",
                        limits.maximum_directed_pair_visits
                    )));
                    return;
                }
                let radius_index =
                    radii_um.partition_point(|radius| *radius < neighbor.distance_um);
                if radius_index == radii_um.len() {
                    visit_error = Some(CategoricalCooccurrenceError::Numerical(
                        "spatial index returned a neighbor beyond the maximum radius".into(),
                    ));
                    return;
                }
                let target_class = prepared.class_indices[neighbor.index];
                let count_index =
                    radius_index * matrix_len + source_class * class_count + target_class;
                let Some(next_count) = counts[count_index].checked_add(1) else {
                    visit_error = Some(resource("directed-pair count overflowed"));
                    return;
                };
                counts[count_index] = next_count;
            })
            .map_err(|error| CategoricalCooccurrenceError::Geometry(error.to_string()))?;
        if let Some(error) = visit_error {
            return Err(error);
        }
    }

    for radius_index in 1..radii_um.len() {
        let (previous, current_and_later) = counts.split_at_mut(radius_index * matrix_len);
        let previous = &previous[(radius_index - 1) * matrix_len..radius_index * matrix_len];
        let current = &mut current_and_later[..matrix_len];
        for (value, prior) in current.iter_mut().zip(previous) {
            *value = value
                .checked_add(*prior)
                .ok_or_else(|| resource("cumulative directed-pair count overflowed"))?;
        }
    }

    let mut matrices = Vec::new();
    matrices
        .try_reserve_exact(radii_um.len())
        .map_err(|_| resource("result matrix allocation failed"))?;
    for (radius_index, radius_um) in radii_um.iter().copied().enumerate() {
        let matrix = &counts[radius_index * matrix_len..(radius_index + 1) * matrix_len];
        let mut source_incidences = Vec::new();
        source_incidences
            .try_reserve_exact(class_count)
            .map_err(|_| resource("edge-incidence allocation failed"))?;
        for source in 0..class_count {
            source_incidences.push(checked_sum(
                &matrix[source * class_count..(source + 1) * class_count],
                "edge-incidence count overflowed",
            )?);
        }
        let mut target_incidences = Vec::new();
        target_incidences
            .try_reserve_exact(class_count)
            .map_err(|_| resource("target edge-incidence allocation failed"))?;
        target_incidences.resize(class_count, 0_usize);
        for source in 0..class_count {
            for target in 0..class_count {
                target_incidences[target] = target_incidences[target]
                    .checked_add(matrix[source * class_count + target])
                    .ok_or_else(|| resource("target edge-incidence count overflowed"))?;
            }
        }
        let total_directed_pairs =
            checked_sum(&source_incidences, "directed-pair total overflowed")?;
        if checked_sum(&target_incidences, "target directed-pair total overflowed")?
            != total_directed_pairs
        {
            return Err(CategoricalCooccurrenceError::Numerical(
                "source and target directed-pair totals differ".into(),
            ));
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(matrix_len)
            .map_err(|_| resource("co-occurrence entry allocation failed"))?;
        for source in 0..class_count {
            for target in 0..class_count {
                let directed_pair_count = matrix[source * class_count + target];
                let source_edge_incidence = source_incidences[source];
                let target_edge_incidence = target_incidences[target];
                let (status, score) = if source_edge_incidence == 0 || target_edge_incidence == 0 {
                    (
                        CategoricalCooccurrenceStatus::UnavailableZeroEdgeIncidence,
                        None,
                    )
                } else {
                    let score = directed_pair_count as f64 * total_directed_pairs as f64
                        / (source_edge_incidence as f64 * target_edge_incidence as f64);
                    if !score.is_finite() {
                        return Err(CategoricalCooccurrenceError::Numerical(
                            "co-occurrence score is not finite".into(),
                        ));
                    }
                    (CategoricalCooccurrenceStatus::Available, Some(score))
                };
                entries.push(CategoricalCooccurrenceEntry {
                    source_class_id: prepared.classes[source].clone(),
                    target_class_id: prepared.classes[target].clone(),
                    directed_pair_count,
                    source_edge_incidence,
                    target_edge_incidence,
                    status,
                    score,
                });
            }
        }
        matrices.push(CategoricalCooccurrenceMatrix {
            radius_um,
            total_directed_pairs,
            entries,
        });
    }
    let maximum_radius_directed_pair_visits = matrices
        .last()
        .map_or(0, |matrix| matrix.total_directed_pairs);
    if maximum_radius_directed_pair_visits != directed_pair_visits {
        return Err(CategoricalCooccurrenceError::Numerical(
            "maximum-radius pair totals are inconsistent".into(),
        ));
    }
    Ok(CategoricalCooccurrenceResult {
        format: CATEGORICAL_COOCCURRENCE_FORMAT.into(),
        version: CATEGORICAL_COOCCURRENCE_VERSION,
        coordinate_frame_id: prepared.coordinate_frame_id.clone(),
        channel_id: prepared.channel_id.clone(),
        classes: prepared.classes.to_vec(),
        point_count: prepared.class_indices.len(),
        class_count,
        maximum_radius_directed_pair_visits,
        matrices,
    })
}

fn validate_limits(
    limits: CategoricalCooccurrenceLimits,
) -> Result<(), CategoricalCooccurrenceError> {
    if [
        limits.maximum_points,
        limits.maximum_classes,
        limits.maximum_radii,
        limits.maximum_output_matrix_bytes,
        limits.maximum_directed_pair_visits,
        limits.maximum_retained_bytes,
    ]
    .contains(&0)
    {
        return Err(invalid("all co-occurrence limits must be positive"));
    }
    Ok(())
}

fn validate_radii(
    radii_um: &[f64],
    maximum_radii: usize,
) -> Result<(), CategoricalCooccurrenceError> {
    if radii_um.is_empty() {
        return Err(invalid("at least one radius is required"));
    }
    if radii_um.len() > maximum_radii {
        return Err(resource(format!(
            "radius count {} exceeds maximum {maximum_radii}",
            radii_um.len()
        )));
    }
    for (index, radius) in radii_um.iter().copied().enumerate() {
        if !radius.is_finite() || radius <= 0.0 || !(radius * radius).is_finite() {
            return Err(invalid(format!(
                "radius row {index} must be finite, positive, and safely squareable"
            )));
        }
        if index > 0 && radius <= radii_um[index - 1] {
            return Err(invalid("radii must be strictly increasing"));
        }
    }
    Ok(())
}

fn estimate_decoded_input_bytes(
    coordinate_frame_id: &str,
    coordinate_unit: &str,
    window_geojson: &str,
    channel: &CategoricalCooccurrenceChannel,
    cells: &[CategoricalCooccurrenceCell],
    radii_um: &[f64],
) -> Result<usize, CategoricalCooccurrenceError> {
    let cell_text_bytes = cells.iter().try_fold(0_usize, |total, cell| {
        total
            .checked_add(cell.id.len())
            .and_then(|value| value.checked_add(cell.class_id.len()))
            .ok_or_else(|| resource("cell text size overflowed"))
    })?;
    let class_text_bytes = channel.classes.iter().try_fold(0_usize, |total, class| {
        total
            .checked_add(class.len())
            .ok_or_else(|| resource("class text size overflowed"))
    })?;
    let fixed_text_bytes = sum_bytes(&[
        coordinate_frame_id.len(),
        coordinate_unit.len(),
        window_geojson.len(),
        channel.id.len(),
        channel.label.len(),
        channel.kind.len(),
        channel.unit.len(),
        channel.measurement_status.len(),
        channel.provenance.len(),
    ])?;
    let cell_storage = cells
        .len()
        .checked_mul(std::mem::size_of::<CategoricalCooccurrenceCell>())
        .ok_or_else(|| resource("cell storage size overflowed"))?;
    let class_storage = channel
        .classes
        .len()
        .checked_mul(std::mem::size_of::<String>())
        .ok_or_else(|| resource("class storage size overflowed"))?;
    let radius_storage = radii_um
        .len()
        .checked_mul(std::mem::size_of::<f64>())
        .ok_or_else(|| resource("radius storage size overflowed"))?;
    sum_bytes(&[
        std::mem::size_of::<CategoricalCooccurrenceChannel>(),
        std::mem::size_of::<CategoricalCooccurrenceLimits>(),
        cell_storage,
        class_storage,
        radius_storage,
        cell_text_bytes,
        class_text_bytes,
        fixed_text_bytes,
    ])
}

fn estimate_output_matrix_bytes(
    classes: &[String],
    radii: usize,
) -> Result<usize, CategoricalCooccurrenceError> {
    let class_count = classes.len();
    let matrix_len = class_count
        .checked_mul(class_count)
        .ok_or_else(|| resource("output matrix size overflowed"))?;
    let entry_bytes = matrix_len
        .checked_mul(std::mem::size_of::<CategoricalCooccurrenceEntry>())
        .ok_or_else(|| resource("output entry size overflowed"))?;
    let class_text_bytes = classes.iter().try_fold(0_usize, |total, class| {
        total
            .checked_add(class.len())
            .ok_or_else(|| resource("output class text size overflowed"))
    })?;
    let repeated_text_bytes = class_text_bytes
        .checked_mul(2)
        .and_then(|value| value.checked_mul(class_count))
        .ok_or_else(|| resource("output entry text size overflowed"))?;
    let one_matrix = sum_bytes(&[
        std::mem::size_of::<CategoricalCooccurrenceMatrix>(),
        entry_bytes,
        repeated_text_bytes,
    ])?;
    one_matrix
        .checked_mul(radii)
        .ok_or_else(|| resource("output matrix storage size overflowed"))
}

fn estimate_result_overhead_bytes(
    prepared: &PreparedCategoricalInput,
) -> Result<usize, CategoricalCooccurrenceError> {
    let class_text_bytes = prepared.classes.iter().try_fold(0_usize, |total, class| {
        total
            .checked_add(class.len())
            .ok_or_else(|| resource("result class text size overflowed"))
    })?;
    let class_storage = prepared
        .classes
        .len()
        .checked_mul(std::mem::size_of::<String>())
        .ok_or_else(|| resource("result class storage size overflowed"))?;
    sum_bytes(&[
        std::mem::size_of::<CategoricalCooccurrenceResult>(),
        class_storage,
        class_text_bytes,
        prepared.coordinate_frame_id.len(),
        prepared.channel_id.len(),
        CATEGORICAL_COOCCURRENCE_FORMAT.len(),
    ])
}

fn checked_sum(
    values: &[usize],
    overflow_message: &str,
) -> Result<usize, CategoricalCooccurrenceError> {
    values.iter().try_fold(0_usize, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(|| resource(overflow_message))
    })
}

fn sum_bytes(values: &[usize]) -> Result<usize, CategoricalCooccurrenceError> {
    checked_sum(values, "retained-memory size overflowed")
}

fn map_input_error(error: CategoricalInputError) -> CategoricalCooccurrenceError {
    match error {
        CategoricalInputError::Invalid(message) => invalid(message),
        CategoricalInputError::Resource(message) => resource(message),
        CategoricalInputError::Geometry(message) => CategoricalCooccurrenceError::Geometry(message),
    }
}

fn invalid(message: impl Into<String>) -> CategoricalCooccurrenceError {
    CategoricalCooccurrenceError::Invalid(message.into())
}

fn resource(message: impl Into<String>) -> CategoricalCooccurrenceError {
    CategoricalCooccurrenceError::Resource(message.into())
}
