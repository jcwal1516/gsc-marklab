use super::{
    build_weight_map, find_bin, stable_squared_distance, validate_bins,
    validate_points_and_features, CompensatedSum, EmbeddingSpatialError, VectorSemivariogramResult,
    VectorSemivariogramRow, VectorSemivariogramSpec,
};

pub fn vector_semivariogram(
    mut spec: VectorSemivariogramSpec,
) -> Result<VectorSemivariogramResult, EmbeddingSpatialError> {
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

    let weighted = spec.weights.is_some();
    let mut weights = build_weight_map(spec.weights, &spec.points)?;
    let mut pair_counts = vec![0_u64; spec.bins.len()];
    let mut weight_sums = vec![CompensatedSum::default(); spec.bins.len()];
    let mut weighted_semivariances = vec![CompensatedSum::default(); spec.bins.len()];

    for left_index in 0..spec.points.len() {
        let left = &spec.points[left_index];
        for right in &spec.points[(left_index + 1)..] {
            let distance = (left.x_um - right.x_um).hypot(left.y_um - right.y_um);
            if !distance.is_finite() {
                return Err(EmbeddingSpatialError::Numeric(format!(
                    "distance between {} and {} is non-finite",
                    left.object_id, right.object_id
                )));
            }
            let Some(bin_index) = find_bin(distance, &spec.bins) else {
                continue;
            };
            let pair_key = (left.object_id.clone(), right.object_id.clone());
            let weight = if weighted {
                weights.remove(&pair_key).ok_or_else(|| {
                    EmbeddingSpatialError::Invalid(format!(
                        "eligible pair {}--{} has no declared weight",
                        left.object_id, right.object_id
                    ))
                })?
            } else {
                1.0
            };
            let squared_distance = stable_squared_distance(&left.embedding, &right.embedding)?;
            let contribution = 0.5 * weight * squared_distance;
            if !contribution.is_finite() {
                return Err(EmbeddingSpatialError::Numeric(format!(
                    "semivariance contribution for {}--{} overflowed",
                    left.object_id, right.object_id
                )));
            }
            pair_counts[bin_index] += 1;
            weight_sums[bin_index].add(weight)?;
            weighted_semivariances[bin_index].add(contribution)?;
        }
    }
    if let Some(((left, right), _)) = weights.into_iter().next() {
        return Err(EmbeddingSpatialError::Invalid(format!(
            "declared weight for ineligible pair {left}--{right}"
        )));
    }

    let mut curve = Vec::with_capacity(spec.bins.len());
    for (index, bin) in spec.bins.into_iter().enumerate() {
        let weight_sum = weight_sums[index].total()?;
        let semivariance = if pair_counts[index] == 0 {
            None
        } else {
            let value = weighted_semivariances[index].total()? / weight_sum;
            if !value.is_finite() {
                return Err(EmbeddingSpatialError::Numeric(format!(
                    "semivariance for bin {} is non-finite",
                    bin.bin_id
                )));
            }
            Some(value)
        };
        curve.push(VectorSemivariogramRow {
            bin_id: bin.bin_id,
            lower_um: bin.lower_um,
            upper_um: bin.upper_um,
            upper_inclusive: bin.upper_inclusive,
            pair_count: pair_counts[index],
            weight_sum,
            semivariance,
            inference_eligible: pair_counts[index] >= 2,
        });
    }

    Ok(VectorSemivariogramResult {
        coordinate_unit: "micrometer",
        object_count: point_count as u32,
        embedding_dimension: dimension as u32,
        feature_names: spec.feature_names,
        pair_visits,
        weighting: if weighted {
            "declared_pair_weights"
        } else {
            "unit"
        },
        rotation_invariant: true,
        curve,
    })
}
