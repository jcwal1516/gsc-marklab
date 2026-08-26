use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::{
    boundary_distance, validate_and_normalize, validate_spd, CompiledCuboid3D, CoordinateUnit,
    CuboidWindowInput, HomogeneousK3dSpec, K3dCorrection, NormalizedPoint3D, Point3DInput,
    Spatial3dError, MAXIMUM_PAIR_RADIUS_EVALUATIONS,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightedPoint3DInput {
    pub id: String,
    pub coordinates: [f64; 3],
    pub intensity_per_um3: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InhomogeneousK3dSpec {
    pub points: Vec<WeightedPoint3DInput>,
    pub window: CuboidWindowInput,
    pub coordinate_unit: CoordinateUnit,
    pub voxel_spacing: [f64; 3],
    pub anisotropy_matrix: Option<[[f64; 3]; 3]>,
    pub radii_um: Vec<f64>,
    pub correction: K3dCorrection,
    pub maximum_unordered_pairs: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectedCrossK3dSpec {
    pub type_a_points: Vec<WeightedPoint3DInput>,
    pub type_b_points: Vec<WeightedPoint3DInput>,
    pub window: CuboidWindowInput,
    pub coordinate_unit: CoordinateUnit,
    pub voxel_spacing: [f64; 3],
    pub anisotropy_matrix: Option<[[f64; 3]; 3]>,
    pub radii_um: Vec<f64>,
    pub correction: K3dCorrection,
    pub maximum_cross_pairs: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NormalizedWeightedPoint3D {
    pub id: String,
    pub coordinates_um: [f64; 3],
    pub intensity_per_um3: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct WeightedK3dCurvePoint {
    pub radius_um: f64,
    pub k_um3: f64,
    pub l3_um: f64,
    pub eligible_directed_pairs: u64,
    pub border_reference_points: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CrossK3dCurvePoint {
    pub radius_um: f64,
    pub cross_k_um3: f64,
    pub cross_g: Option<f64>,
    pub eligible_directed_pairs: u64,
    pub border_reference_points: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InhomogeneousK3dResult {
    pub format: &'static str,
    pub version: u32,
    pub metric: &'static str,
    pub correction: K3dCorrection,
    pub window: CompiledCuboid3D,
    pub normalized_points: Vec<NormalizedWeightedPoint3D>,
    pub curve: Vec<WeightedK3dCurvePoint>,
    pub unordered_pairs_visited: u64,
    pub pair_radius_evaluations: u64,
    pub claim_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct DirectedCrossK3dResult {
    pub format: &'static str,
    pub version: u32,
    pub direction: &'static str,
    pub metric: &'static str,
    pub correction: K3dCorrection,
    pub window: CompiledCuboid3D,
    pub normalized_type_a_points: Vec<NormalizedWeightedPoint3D>,
    pub normalized_type_b_points: Vec<NormalizedWeightedPoint3D>,
    pub curve: Vec<CrossK3dCurvePoint>,
    pub cross_pairs_visited: u64,
    pub pair_radius_evaluations: u64,
    pub claim_status: &'static str,
}

pub fn inhomogeneous_k3d(
    spec: InhomogeneousK3dSpec,
) -> Result<InhomogeneousK3dResult, Spatial3dError> {
    validate_intensities(&spec.points, "inhomogeneous")?;
    let base = HomogeneousK3dSpec {
        points: plain_points(&spec.points),
        window: spec.window,
        coordinate_unit: spec.coordinate_unit,
        voxel_spacing: spec.voxel_spacing,
        anisotropy_matrix: spec.anisotropy_matrix,
        radii_um: spec.radii_um,
        correction: spec.correction,
        maximum_unordered_pairs: spec.maximum_unordered_pairs,
    };
    let scale = base.coordinate_unit.micrometer_scale();
    let (normalized, window, pair_count, work) = validate_and_normalize(&base, scale)?;
    let metric = metric(&base)?;
    let weighted = bind(normalized, &spec.points);
    let mut curve = Vec::with_capacity(base.radii_um.len());
    for &radius in &base.radii_um {
        let (sum, eligible, references, normalizer) =
            inhomogeneous_sum(&weighted, &window, metric, radius, base.correction)?;
        let k = sum / normalizer;
        curve.push(WeightedK3dCurvePoint {
            radius_um: radius,
            k_um3: finite(k, "inhomogeneous K")?,
            l3_um: finite(
                (3.0 * k / (4.0 * std::f64::consts::PI)).cbrt(),
                "inhomogeneous L3",
            )?,
            eligible_directed_pairs: eligible,
            border_reference_points: references,
        });
    }
    Ok(InhomogeneousK3dResult {
        format: "marklab.inhomogeneous_k3d",
        version: 1,
        metric: metric_name(base.anisotropy_matrix),
        correction: base.correction,
        window,
        normalized_points: weighted,
        curve,
        unordered_pairs_visited: pair_count,
        pair_radius_evaluations: work,
        claim_status: "supplied_intensity_cuboid_3d_descriptive_only",
    })
}

pub fn directed_cross_k3d(
    spec: DirectedCrossK3dSpec,
) -> Result<DirectedCrossK3dResult, Spatial3dError> {
    if spec.type_a_points.is_empty() || spec.type_b_points.is_empty() {
        return Err(Spatial3dError::Invalid(
            "both cross-K point types must be nonempty".into(),
        ));
    }
    validate_intensities(&spec.type_a_points, "type A")?;
    validate_intensities(&spec.type_b_points, "type B")?;
    let mut combined = spec.type_a_points.clone();
    combined.extend(spec.type_b_points.clone());
    let total = u64::try_from(combined.len())
        .map_err(|_| Spatial3dError::Resource("combined point count overflowed".into()))?;
    let combined_pairs = total
        .checked_mul(total - 1)
        .and_then(|value| value.checked_div(2))
        .ok_or_else(|| Spatial3dError::Resource("combined pair count overflowed".into()))?;
    let base = HomogeneousK3dSpec {
        points: plain_points(&combined),
        window: spec.window,
        coordinate_unit: spec.coordinate_unit,
        voxel_spacing: spec.voxel_spacing,
        anisotropy_matrix: spec.anisotropy_matrix,
        radii_um: spec.radii_um,
        correction: spec.correction,
        maximum_unordered_pairs: combined_pairs,
    };
    let scale = base.coordinate_unit.micrometer_scale();
    let (normalized, window, _, _) = validate_and_normalize(&base, scale)?;
    let metric = metric(&base)?;
    let split = spec.type_a_points.len();
    let weighted = bind(normalized, &combined);
    let (type_a, type_b) = weighted.split_at(split);
    let cross_pairs = u64::try_from(type_a.len())
        .ok()
        .and_then(|a| {
            u64::try_from(type_b.len())
                .ok()
                .and_then(|b| a.checked_mul(b))
        })
        .ok_or_else(|| Spatial3dError::Resource("cross-pair count overflowed".into()))?;
    if cross_pairs > spec.maximum_cross_pairs {
        return Err(Spatial3dError::Resource(format!(
            "cross pairs {cross_pairs} exceed declared maximum {}",
            spec.maximum_cross_pairs
        )));
    }
    let work = cross_pairs
        .checked_mul(base.radii_um.len() as u64)
        .ok_or_else(|| Spatial3dError::Resource("cross pair-radius work overflowed".into()))?;
    if work > MAXIMUM_PAIR_RADIUS_EVALUATIONS {
        return Err(Spatial3dError::Resource(
            "cross pair-radius work exceeds 250 million".into(),
        ));
    }
    let mut curve = Vec::with_capacity(base.radii_um.len());
    let mut previous_k = 0.0;
    let mut previous_radius: f64 = 0.0;
    for &radius in &base.radii_um {
        let (sum, eligible, references, normalizer) =
            cross_sum(type_a, type_b, &window, metric, radius, base.correction)?;
        let k = finite(sum / normalizer, "directed cross-K")?;
        let shell_volume =
            4.0 * std::f64::consts::PI / 3.0 * (radius.powi(3) - previous_radius.powi(3));
        let cross_g = if shell_volume == 0.0 {
            None
        } else {
            Some(finite((k - previous_k) / shell_volume, "directed cross-g")?)
        };
        curve.push(CrossK3dCurvePoint {
            radius_um: radius,
            cross_k_um3: k,
            cross_g,
            eligible_directed_pairs: eligible,
            border_reference_points: references,
        });
        previous_k = k;
        previous_radius = radius;
    }
    Ok(DirectedCrossK3dResult {
        format: "marklab.directed_cross_k3d",
        version: 1,
        direction: "type_a_to_type_b",
        metric: metric_name(base.anisotropy_matrix),
        correction: base.correction,
        window,
        normalized_type_a_points: type_a.to_vec(),
        normalized_type_b_points: type_b.to_vec(),
        curve,
        cross_pairs_visited: cross_pairs,
        pair_radius_evaluations: work,
        claim_status: "supplied_intensity_directed_cuboid_3d_descriptive_only",
    })
}

fn inhomogeneous_sum(
    points: &[NormalizedWeightedPoint3D],
    window: &CompiledCuboid3D,
    metric: [[f64; 3]; 3],
    radius: f64,
    correction: K3dCorrection,
) -> Result<(f64, u64, Option<usize>, f64), Spatial3dError> {
    let references = border_references(points, window, radius, correction)?;
    let mut sum = 0.0;
    let mut eligible = 0;
    for left in 0..points.len() {
        for right in (left + 1)..points.len() {
            let displacement = subtract(points[right].coordinates_um, points[left].coordinates_um);
            if distance(displacement, metric)? > radius {
                continue;
            }
            let weight = edge_weight(displacement, window, correction)?
                / (points[left].intensity_per_um3 * points[right].intensity_per_um3);
            if references.as_ref().is_none_or(|set| set.contains(&left)) {
                sum += weight;
                eligible += 1;
            }
            if references.as_ref().is_none_or(|set| set.contains(&right)) {
                sum += weight;
                eligible += 1;
            }
        }
    }
    let normalizer = border_normalizer(window, radius, correction)?;
    Ok((
        sum,
        eligible,
        references.as_ref().map(HashSet::len),
        normalizer,
    ))
}

fn cross_sum(
    type_a: &[NormalizedWeightedPoint3D],
    type_b: &[NormalizedWeightedPoint3D],
    window: &CompiledCuboid3D,
    metric: [[f64; 3]; 3],
    radius: f64,
    correction: K3dCorrection,
) -> Result<(f64, u64, Option<usize>, f64), Spatial3dError> {
    let references = border_references(type_a, window, radius, correction)?;
    let mut sum = 0.0;
    let mut eligible = 0;
    for (left_index, left) in type_a.iter().enumerate() {
        if references
            .as_ref()
            .is_some_and(|set| !set.contains(&left_index))
        {
            continue;
        }
        for right in type_b {
            let displacement = subtract(right.coordinates_um, left.coordinates_um);
            if distance(displacement, metric)? <= radius {
                sum += edge_weight(displacement, window, correction)?
                    / (left.intensity_per_um3 * right.intensity_per_um3);
                eligible += 1;
            }
        }
    }
    let normalizer = border_normalizer(window, radius, correction)?;
    Ok((
        sum,
        eligible,
        references.as_ref().map(HashSet::len),
        normalizer,
    ))
}

fn border_references(
    points: &[NormalizedWeightedPoint3D],
    window: &CompiledCuboid3D,
    radius: f64,
    correction: K3dCorrection,
) -> Result<Option<HashSet<usize>>, Spatial3dError> {
    if !matches!(correction, K3dCorrection::Border) {
        return Ok(None);
    }
    let references = points
        .iter()
        .enumerate()
        .filter_map(|(index, point)| {
            (boundary_distance(point.coordinates_um, window) >= radius).then_some(index)
        })
        .collect::<HashSet<_>>();
    if references.is_empty() {
        return Err(Spatial3dError::Invalid(format!(
            "border correction has no reference point at radius {radius}"
        )));
    }
    Ok(Some(references))
}

fn border_normalizer(
    window: &CompiledCuboid3D,
    radius: f64,
    correction: K3dCorrection,
) -> Result<f64, Spatial3dError> {
    if !matches!(correction, K3dCorrection::Border) {
        return Ok(window.volume_um3);
    }
    let volume = window
        .side_lengths_um
        .iter()
        .map(|length| length - 2.0 * radius)
        .product::<f64>();
    if !volume.is_finite() || volume <= 0.0 {
        return Err(Spatial3dError::Invalid(format!(
            "border-eroded cuboid has no positive volume at radius {radius}"
        )));
    }
    Ok(volume)
}

fn edge_weight(
    displacement: [f64; 3],
    window: &CompiledCuboid3D,
    correction: K3dCorrection,
) -> Result<f64, Spatial3dError> {
    if !matches!(correction, K3dCorrection::Translation) {
        return Ok(1.0);
    }
    let overlap = (0..3)
        .map(|axis| window.side_lengths_um[axis] - displacement[axis].abs())
        .product::<f64>();
    if !overlap.is_finite() || overlap <= 0.0 {
        return Err(Spatial3dError::Numerical(
            "translation overlap is zero for an eligible pair".into(),
        ));
    }
    Ok(window.volume_um3 / overlap)
}

fn metric(spec: &HomogeneousK3dSpec) -> Result<[[f64; 3]; 3], Spatial3dError> {
    let matrix =
        spec.anisotropy_matrix
            .unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    validate_spd(matrix)?;
    Ok(matrix)
}

fn distance(displacement: [f64; 3], metric: [[f64; 3]; 3]) -> Result<f64, Spatial3dError> {
    let transformed = metric.map(|row| {
        row.iter()
            .zip(displacement)
            .map(|(coefficient, value)| coefficient * value)
            .sum::<f64>()
    });
    let squared = displacement
        .iter()
        .zip(transformed)
        .map(|(left, right)| left * right)
        .sum::<f64>();
    if !squared.is_finite() || squared < 0.0 {
        return Err(Spatial3dError::Numerical(
            "anisotropic squared distance is negative or non-finite".into(),
        ));
    }
    Ok(squared.sqrt())
}

fn validate_intensities(points: &[WeightedPoint3DInput], name: &str) -> Result<(), Spatial3dError> {
    if points
        .iter()
        .any(|point| !point.intensity_per_um3.is_finite() || point.intensity_per_um3 <= 0.0)
    {
        return Err(Spatial3dError::Invalid(format!(
            "{name} intensities must be positive and finite"
        )));
    }
    Ok(())
}

fn plain_points(points: &[WeightedPoint3DInput]) -> Vec<Point3DInput> {
    points
        .iter()
        .map(|point| Point3DInput {
            id: point.id.clone(),
            coordinates: point.coordinates,
        })
        .collect()
}

fn bind(
    points: Vec<NormalizedPoint3D>,
    source: &[WeightedPoint3DInput],
) -> Vec<NormalizedWeightedPoint3D> {
    points
        .into_iter()
        .zip(source)
        .map(|(point, source)| NormalizedWeightedPoint3D {
            id: point.id,
            coordinates_um: point.coordinates_um,
            intensity_per_um3: source.intensity_per_um3,
        })
        .collect()
}

fn subtract(right: [f64; 3], left: [f64; 3]) -> [f64; 3] {
    [right[0] - left[0], right[1] - left[1], right[2] - left[2]]
}

fn finite(value: f64, name: &str) -> Result<f64, Spatial3dError> {
    if !value.is_finite() {
        return Err(Spatial3dError::Numerical(format!("{name} is not finite")));
    }
    Ok(value)
}

fn metric_name(anisotropy: Option<[[f64; 3]; 3]>) -> &'static str {
    if anisotropy.is_some() {
        "anisotropic_mahalanobis_physical_um"
    } else {
        "euclidean_physical_um"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_g_is_explicitly_unavailable_for_a_zero_volume_first_shell() {
        let result = directed_cross_k3d(DirectedCrossK3dSpec {
            type_a_points: vec![point("a", [1.0, 1.0, 1.0])],
            type_b_points: vec![point("b", [2.0, 1.0, 1.0])],
            window: CuboidWindowInput {
                minimum: [0.0; 3],
                maximum: [10.0; 3],
            },
            coordinate_unit: CoordinateUnit::Micrometer,
            voxel_spacing: [1.0; 3],
            anisotropy_matrix: None,
            radii_um: vec![0.0, 1.0],
            correction: K3dCorrection::None,
            maximum_cross_pairs: 1,
        })
        .unwrap();
        assert!(result.curve[0].cross_g.is_none());
        assert!(result.curve[1].cross_g.unwrap() > 0.0);
    }

    fn point(id: &str, coordinates: [f64; 3]) -> WeightedPoint3DInput {
        WeightedPoint3DInput {
            id: id.into(),
            coordinates,
            intensity_per_um3: 0.001,
        }
    }
}
