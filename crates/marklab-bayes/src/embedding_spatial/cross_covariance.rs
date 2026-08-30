use super::{
    find_bin, validate_bins, validate_points_and_features, CompensatedSum,
    EmbeddingCrossCovarianceMatrix, EmbeddingCrossCovarianceMatrixArtifact,
    EmbeddingCrossCovarianceResult, EmbeddingCrossCovarianceSpec, EmbeddingCrossCovarianceSummary,
    EmbeddingSpatialError,
};

pub fn embedding_cross_covariance_by_distance(
    mut spec: EmbeddingCrossCovarianceSpec,
) -> Result<EmbeddingCrossCovarianceResult, EmbeddingSpatialError> {
    let dimension = validate_points_and_features(&mut spec.points, &spec.feature_names)?;
    validate_bins(&mut spec.bins)?;
    let point_count = spec.points.len() as u64;
    let pair_visits = point_count
        .checked_mul(point_count - 1)
        .and_then(|value| value.checked_div(2))
        .ok_or_else(|| EmbeddingSpatialError::Invalid("pair count overflowed".into()))?;
    if spec.maximum_pair_visits == 0 || pair_visits > spec.maximum_pair_visits {
        return Err(EmbeddingSpatialError::Invalid(format!(
            "{pair_visits} unordered pair visits exceed maximum_pair_visits {}",
            spec.maximum_pair_visits
        )));
    }
    let elements_per_matrix = (dimension as u64)
        .checked_mul(dimension as u64)
        .ok_or_else(|| EmbeddingSpatialError::Invalid("matrix size overflowed".into()))?;
    let stored_elements = elements_per_matrix
        .checked_mul(spec.bins.len() as u64)
        .ok_or_else(|| EmbeddingSpatialError::Invalid("matrix artifact size overflowed".into()))?;
    let matrix_operations = elements_per_matrix
        .checked_mul(pair_visits)
        .ok_or_else(|| EmbeddingSpatialError::Invalid("matrix work overflowed".into()))?;
    if spec.maximum_matrix_elements == 0
        || stored_elements > 1_000_000
        || matrix_operations > spec.maximum_matrix_elements
        || spec.maximum_matrix_elements > 250_000_000
    {
        return Err(EmbeddingSpatialError::Invalid(format!(
            "{matrix_operations} pair-matrix element operations exceed the declared or fixed resource bound"
        )));
    }

    let mut mean_sums = vec![CompensatedSum::default(); dimension];
    for point in &spec.points {
        for (sum, value) in mean_sums.iter_mut().zip(&point.embedding) {
            sum.add(*value)?;
        }
    }
    let global_mean = mean_sums
        .into_iter()
        .map(|sum| sum.total().map(|total| total / point_count as f64))
        .collect::<Result<Vec<_>, _>>()?;
    let centered = spec
        .points
        .iter()
        .map(|point| {
            point
                .embedding
                .iter()
                .zip(&global_mean)
                .map(|(value, mean)| value - mean)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let mut pair_counts = vec![0_u64; spec.bins.len()];
    let mut accumulators = vec![CompensatedSum::default(); spec.bins.len() * dimension * dimension];
    for left in 0..spec.points.len() {
        for right in (left + 1)..spec.points.len() {
            let distance = (spec.points[left].x_um - spec.points[right].x_um)
                .hypot(spec.points[left].y_um - spec.points[right].y_um);
            if !distance.is_finite() {
                return Err(EmbeddingSpatialError::Numeric(format!(
                    "distance between {} and {} is non-finite",
                    spec.points[left].object_id, spec.points[right].object_id
                )));
            }
            let Some(bin) = find_bin(distance, &spec.bins) else {
                continue;
            };
            pair_counts[bin] += 1;
            let offset = bin * dimension * dimension;
            for row in 0..dimension {
                for column in 0..dimension {
                    accumulators[offset + row * dimension + column]
                        .add(centered[left][row] * centered[right][column])?;
                }
            }
        }
    }

    let mut summaries = Vec::with_capacity(spec.bins.len());
    let mut matrices = Vec::with_capacity(spec.bins.len());
    for (bin_index, bin) in spec.bins.iter().enumerate() {
        let matrix = if pair_counts[bin_index] == 0 {
            None
        } else {
            let mut values = vec![vec![0.0; dimension]; dimension];
            let offset = bin_index * dimension * dimension;
            for row in 0..dimension {
                for column in 0..dimension {
                    let forward = accumulators[offset + row * dimension + column].total()?
                        / pair_counts[bin_index] as f64;
                    let reverse = accumulators[offset + column * dimension + row].total()?
                        / pair_counts[bin_index] as f64;
                    let value = 0.5 * (forward + reverse);
                    if !value.is_finite() {
                        return Err(EmbeddingSpatialError::Numeric(format!(
                            "cross-covariance matrix for bin {} is non-finite",
                            bin.bin_id
                        )));
                    }
                    values[row][column] = value;
                }
            }
            Some(values)
        };
        let (trace, frobenius_norm) = if let Some(values) = &matrix {
            let mut trace = CompensatedSum::default();
            let mut squared = CompensatedSum::default();
            for (row, values_row) in values.iter().enumerate() {
                trace.add(values_row[row])?;
                for value in values_row {
                    squared.add(value * value)?;
                }
            }
            let trace = trace.total()?;
            let frobenius = squared.total()?.sqrt();
            if !frobenius.is_finite() {
                return Err(EmbeddingSpatialError::Numeric(format!(
                    "Frobenius norm for bin {} is non-finite",
                    bin.bin_id
                )));
            }
            (Some(trace), Some(frobenius))
        } else {
            (None, None)
        };
        summaries.push(EmbeddingCrossCovarianceSummary {
            bin_id: bin.bin_id.clone(),
            lower_um: bin.lower_um,
            upper_um: bin.upper_um,
            upper_inclusive: bin.upper_inclusive,
            pair_count: pair_counts[bin_index],
            trace,
            frobenius_norm,
        });
        matrices.push(EmbeddingCrossCovarianceMatrix {
            bin_id: bin.bin_id.clone(),
            pair_count: pair_counts[bin_index],
            matrix,
        });
    }

    Ok(EmbeddingCrossCovarianceResult {
        coordinate_unit: "micrometer",
        object_count: point_count as u32,
        embedding_dimension: dimension as u32,
        feature_names: spec.feature_names.clone(),
        pair_visits,
        global_mean,
        symmetrization: "undirected_average_c_plus_c_transpose_over_two",
        summaries,
        matrix_artifact: EmbeddingCrossCovarianceMatrixArtifact {
            format: "marklab.embedding_cross_covariance_matrix_artifact",
            version: 1,
            feature_names: spec.feature_names,
            matrices,
        },
    })
}
