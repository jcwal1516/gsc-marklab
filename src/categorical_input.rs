use std::collections::{HashMap, HashSet};

use thiserror::Error;

use crate::{geom::spatial_index::SpatialIndex2D, ObservationWindow2D, ObservationWindowLimits};

#[derive(Clone, Copy)]
pub(crate) struct CategoricalChannelView<'a> {
    pub(crate) id: &'a str,
    pub(crate) label: &'a str,
    pub(crate) kind: &'a str,
    pub(crate) unit: &'a str,
    pub(crate) measurement_status: &'a str,
    pub(crate) provenance: &'a str,
    pub(crate) classes: &'a [String],
}

#[derive(Clone, Copy)]
pub(crate) struct CategoricalCellView<'a> {
    pub(crate) id: &'a str,
    pub(crate) x_um: f64,
    pub(crate) y_um: f64,
    pub(crate) class_id: &'a str,
}

#[derive(Clone, Copy)]
pub(crate) struct CategoricalInputLimits {
    pub(crate) maximum_points: usize,
    pub(crate) maximum_classes: usize,
    pub(crate) maximum_retained_bytes: usize,
}

pub(crate) struct PreparedCategoricalInput {
    pub(crate) coordinate_frame_id: String,
    pub(crate) channel_id: String,
    pub(crate) classes: Box<[String]>,
    pub(crate) class_indices: Box<[usize]>,
    pub(crate) class_counts: Box<[usize]>,
    pub(crate) index: SpatialIndex2D,
    /// Exact source-file bytes retained by the caller while the analysis runs.
    pub(crate) input_file_bytes: usize,
    /// Conservative retained size of this prepared value, excluding source-file bytes.
    pub(crate) estimated_storage_bytes: usize,
}

#[derive(Debug, Error, PartialEq)]
pub(crate) enum CategoricalInputError {
    #[error("invalid categorical input: {0}")]
    Invalid(String),
    #[error("categorical input resource limit exceeded: {0}")]
    Resource(String),
    #[error("categorical input geometry failed: {0}")]
    Geometry(String),
}

#[expect(
    clippy::too_many_arguments,
    reason = "the internal boundary keeps request-owned bytes and shared categorical fields explicit"
)]
pub(crate) fn prepare_categorical_input<'a>(
    coordinate_frame_id: &str,
    coordinate_unit: &str,
    window_geojson: &str,
    channel: CategoricalChannelView<'_>,
    cells: impl ExactSizeIterator<Item = CategoricalCellView<'a>>,
    limits: CategoricalInputLimits,
    input_file_bytes: usize,
    decoded_request_bytes: usize,
) -> Result<PreparedCategoricalInput, CategoricalInputError> {
    validate_limits(limits)?;
    validate_text(coordinate_frame_id, "coordinate_frame_id")?;
    if coordinate_unit != "micrometer" {
        return Err(invalid("coordinate_unit must be micrometer"));
    }
    validate_channel(channel)?;

    let point_count = cells.len();
    let class_count = channel.classes.len();
    if point_count == 0 {
        return Err(invalid("at least one cell is required"));
    }
    if point_count > limits.maximum_points {
        return Err(resource(format!(
            "point count {point_count} exceeds maximum {}",
            limits.maximum_points
        )));
    }
    if class_count == 0 {
        return Err(invalid("at least one class is required"));
    }
    if class_count > limits.maximum_classes {
        return Err(resource(format!(
            "class count {class_count} exceeds maximum {}",
            limits.maximum_classes
        )));
    }

    let mut class_text_bytes = 0_usize;
    for class_id in channel.classes {
        validate_text(class_id, "class ID")?;
        class_text_bytes = checked_add(class_text_bytes, class_id.len())?;
    }

    let prepared_storage = estimate_prepared_storage(
        point_count,
        class_count,
        class_text_bytes,
        coordinate_frame_id.len(),
        channel.id.len(),
    )?;
    let temporary_lookup_bytes = class_count
        .checked_mul(64)
        .and_then(|value| value.checked_add(point_count.checked_mul(48)?))
        .ok_or_else(|| resource("categorical lookup size overflowed"))?;
    let window_decode_bytes = window_geojson
        .len()
        .checked_mul(64)
        .and_then(|value| value.checked_add(4 * 1024))
        .ok_or_else(|| resource("window decode size overflowed"))?;
    let pre_parse_peak = sum_bytes(&[
        input_file_bytes,
        decoded_request_bytes,
        prepared_storage,
        temporary_lookup_bytes,
        window_decode_bytes,
    ])?;
    enforce_retained(pre_parse_peak, limits.maximum_retained_bytes)?;

    let mut class_lookup = HashMap::new();
    class_lookup
        .try_reserve(class_count)
        .map_err(|_| resource("class lookup allocation failed"))?;
    for (index, class_id) in channel.classes.iter().enumerate() {
        if class_lookup.insert(class_id.as_str(), index).is_some() {
            return Err(invalid(format!("class ID {class_id:?} is duplicated")));
        }
    }

    let window =
        ObservationWindow2D::from_geojson_str(window_geojson, ObservationWindowLimits::default())
            .map_err(|error| CategoricalInputError::Geometry(error.to_string()))?;
    let window_storage = window
        .descriptor()
        .vertex_count
        .checked_mul(128)
        .and_then(|value| value.checked_add(window.boundary_storage_bytes()))
        .ok_or_else(|| resource("window storage size overflowed"))?;
    let parsed_peak = sum_bytes(&[
        input_file_bytes,
        decoded_request_bytes,
        prepared_storage,
        temporary_lookup_bytes,
        window_storage,
    ])?;
    enforce_retained(parsed_peak, limits.maximum_retained_bytes)?;

    let mut cell_ids = HashSet::new();
    cell_ids
        .try_reserve(point_count)
        .map_err(|_| resource("cell ID lookup allocation failed"))?;
    let mut x_um = Vec::new();
    let mut y_um = Vec::new();
    let mut class_indices = Vec::new();
    let mut class_counts = Vec::new();
    class_counts
        .try_reserve_exact(class_count)
        .map_err(|_| resource("class-count allocation failed"))?;
    class_counts.resize(class_count, 0_usize);
    x_um.try_reserve_exact(point_count)
        .map_err(|_| resource("x-coordinate allocation failed"))?;
    y_um.try_reserve_exact(point_count)
        .map_err(|_| resource("y-coordinate allocation failed"))?;
    class_indices
        .try_reserve_exact(point_count)
        .map_err(|_| resource("class-index allocation failed"))?;
    for (row, cell) in cells.enumerate() {
        validate_text(cell.id, "cell ID")?;
        if !cell_ids.insert(cell.id) {
            return Err(invalid(format!("cell ID {:?} is duplicated", cell.id)));
        }
        if !cell.x_um.is_finite() || !cell.y_um.is_finite() {
            return Err(invalid(format!(
                "cell row {row} must have finite coordinates"
            )));
        }
        if !window.contains(cell.x_um, cell.y_um) {
            return Err(invalid(format!(
                "cell row {row} lies outside the observation window"
            )));
        }
        let class_index = class_lookup.get(cell.class_id).copied().ok_or_else(|| {
            invalid(format!(
                "cell row {row} uses undeclared class ID {:?}",
                cell.class_id
            ))
        })?;
        class_counts[class_index] = class_counts[class_index]
            .checked_add(1)
            .ok_or_else(|| resource("class count overflowed"))?;
        x_um.push(cell.x_um);
        y_um.push(cell.y_um);
        class_indices.push(class_index);
    }
    if let Some(class_id) = channel
        .classes
        .iter()
        .enumerate()
        .find_map(|(index, class_id)| (class_counts[index] == 0).then_some(class_id))
    {
        return Err(invalid(format!(
            "declared class ID {class_id:?} has no observed cell"
        )));
    }

    // The exact polygon is no longer needed after containment validation.
    drop(window);
    drop(cell_ids);
    drop(class_lookup);
    let index = SpatialIndex2D::new(&x_um, &y_um)
        .map_err(|error| CategoricalInputError::Geometry(error.to_string()))?;
    debug_assert_eq!(index.len(), point_count);
    Ok(PreparedCategoricalInput {
        coordinate_frame_id: coordinate_frame_id.to_owned(),
        channel_id: channel.id.to_owned(),
        classes: channel.classes.to_vec().into_boxed_slice(),
        class_indices: class_indices.into_boxed_slice(),
        class_counts: class_counts.into_boxed_slice(),
        index,
        input_file_bytes,
        estimated_storage_bytes: prepared_storage,
    })
}

fn validate_limits(limits: CategoricalInputLimits) -> Result<(), CategoricalInputError> {
    if [
        limits.maximum_points,
        limits.maximum_classes,
        limits.maximum_retained_bytes,
    ]
    .contains(&0)
    {
        return Err(invalid("categorical input limits must be positive"));
    }
    Ok(())
}

fn validate_channel(channel: CategoricalChannelView<'_>) -> Result<(), CategoricalInputError> {
    validate_text(channel.id, "channel ID")?;
    validate_text(channel.label, "channel label")?;
    validate_text(channel.provenance, "channel provenance")?;
    if channel.kind != "categorical"
        || channel.unit != "categorical"
        || channel.measurement_status != "classified"
    {
        return Err(invalid(
            "channel kind, unit, and measurement_status must be categorical, categorical, and classified",
        ));
    }
    Ok(())
}

fn validate_text(value: &str, field: &str) -> Result<(), CategoricalInputError> {
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(invalid(format!(
            "{field} must be nonempty and contain no control characters"
        )));
    }
    Ok(())
}

fn estimate_prepared_storage(
    points: usize,
    classes: usize,
    class_text_bytes: usize,
    frame_text_bytes: usize,
    channel_text_bytes: usize,
) -> Result<usize, CategoricalInputError> {
    let coordinate_and_index_bytes = points
        .checked_mul(2 * std::mem::size_of::<f64>() + std::mem::size_of::<usize>())
        .and_then(|value| {
            value.checked_add(SpatialIndex2D::estimated_storage_bytes_for_len(points))
        })
        .ok_or_else(|| resource("prepared coordinate size overflowed"))?;
    let class_bytes = classes
        .checked_mul(std::mem::size_of::<String>() + std::mem::size_of::<usize>())
        .and_then(|value| value.checked_add(class_text_bytes))
        .ok_or_else(|| resource("prepared class size overflowed"))?;
    sum_bytes(&[
        std::mem::size_of::<PreparedCategoricalInput>(),
        coordinate_and_index_bytes,
        class_bytes,
        frame_text_bytes,
        channel_text_bytes,
    ])
}

fn enforce_retained(observed: usize, maximum: usize) -> Result<(), CategoricalInputError> {
    if observed > maximum {
        return Err(resource(format!(
            "retained-memory estimate {observed} exceeds maximum {maximum}"
        )));
    }
    Ok(())
}

fn sum_bytes(values: &[usize]) -> Result<usize, CategoricalInputError> {
    values.iter().try_fold(0_usize, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(|| resource("retained-memory size overflowed"))
    })
}

fn checked_add(left: usize, right: usize) -> Result<usize, CategoricalInputError> {
    left.checked_add(right)
        .ok_or_else(|| resource("categorical text size overflowed"))
}

fn invalid(message: impl Into<String>) -> CategoricalInputError {
    CategoricalInputError::Invalid(message.into())
}

fn resource(message: impl Into<String>) -> CategoricalInputError {
    CategoricalInputError::Resource(message.into())
}
