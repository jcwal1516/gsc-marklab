use super::*;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;
use serde_json::Number;

pub(super) fn validate_nonstationary_adaptive_result(
    spec: &NonstationaryAdaptiveWindowSpdeSpec,
    result: &NonstationaryAdaptiveWindowSpdeResult,
) -> Result<(), &'static str> {
    let invalid = "nonstationary adaptive SPDE result structure or mathematics differ";
    if result.version != 1
        || result.statistical_unit != StatisticalUnit::WithinSpecimenFieldDiagnostic
        || result.claim_status != ClaimStatus::FittedNonstationaryAnisotropicAdaptiveSpdeDiagnostic
    {
        return Err(invalid);
    }
    let mesh = &result.mesh;
    let vertex_count = mesh.vertex_count;
    if mesh.operator != "mass_lumped_nonstationary_alpha_two_spde"
        || mesh.basis != "piecewise_linear_triangular"
        || mesh.boundary_condition != "natural_neumann_with_positive_local_kappa"
        || mesh.alpha != 2
        || vertex_count < 3
        || mesh.triangle_count == 0
        || mesh.vertices.len() != vertex_count
        || mesh.triangles.len() != mesh.triangle_count
        || ![
            mesh.mesh_area,
            mesh.relative_area_error,
            mesh.minimum_precision_eigenvalue,
            mesh.maximum_precision_eigenvalue,
            mesh.maximum_precision_symmetry_error,
            mesh.maximum_projection_row_sum_error,
        ]
        .iter()
        .all(|value| value.is_finite())
        || mesh.minimum_precision_eigenvalue <= 0.0
        || mesh.maximum_precision_eigenvalue < mesh.minimum_precision_eigenvalue
        || mesh.maximum_precision_symmetry_error > 1e-10
        || mesh.maximum_projection_row_sum_error > 1e-10
    {
        return Err(invalid);
    }
    for matrix in [
        &mesh.mass_matrix,
        &mesh.anisotropic_stiffness_matrix,
        &mesh.precision_matrix,
    ] {
        if matrix.shape != [vertex_count, vertex_count]
            || matrix.rows.len() != matrix.columns.len()
            || matrix.rows.len() != matrix.values.len()
            || matrix
                .rows
                .iter()
                .chain(&matrix.columns)
                .any(|index| *index >= vertex_count)
            || matrix.values.iter().any(|number| !number.is_finite())
        {
            return Err(invalid);
        }
    }
    if spec.parameter_regions.len() != result.parameter_regions.len() {
        return Err(invalid);
    }
    validate_local_parameter_summary(
        [
            &spec.background.kappa,
            &spec.background.tau,
            &spec.background.anisotropy_ratio,
            &spec.background.major_axis_degrees,
        ],
        &result.background,
        "background",
    )?;
    for (expected, actual) in spec.parameter_regions.iter().zip(&result.parameter_regions) {
        validate_local_parameter_summary(
            [
                &expected.kappa,
                &expected.tau,
                &expected.anisotropy_ratio,
                &expected.major_axis_degrees,
            ],
            actual,
            &expected.region_id,
        )?;
        if !same_number(
            &actual.interior_refinement_levels,
            &expected.interior_refinement_levels,
        ) {
            return Err(invalid);
        }
    }
    let limits = &result.limits;
    let limits_match = [
        (&limits.maximum_vertices, &spec.maximum_vertices),
        (&limits.maximum_triangles, &spec.maximum_triangles),
        (
            &limits.maximum_candidate_points,
            &spec.maximum_candidate_points,
        ),
        (
            &limits.maximum_boundary_segment_triangle_checks,
            &spec.maximum_boundary_segment_triangle_checks,
        ),
        (
            &limits.maximum_projection_triangle_visits,
            &spec.maximum_projection_triangle_visits,
        ),
        (&limits.memory_budget_mib, &spec.memory_budget_mib),
        (&limits.maximum_result_bytes, &spec.maximum_result_bytes),
        (&limits.maximum_iterations, &spec.maximum_iterations),
    ]
    .into_iter()
    .all(|(echoed, declared)| same_number(echoed, declared));
    if !result.spatial_factor.gradient_maximum.is_finite()
        || result.spatial_factor.gradient_maximum > 2e-4
        || !limits_match
    {
        return Err(invalid);
    }
    Ok(())
}

/// The worker echoes declared values after parsing them as Python `int` or `float`, so `2`
/// and `2.0` denote the same declaration. Two integers still compare exactly, because `f64`
/// cannot distinguish integers above 2^53.
fn same_number(echoed: &Number, declared: &Number) -> bool {
    echoed == declared
        || ((echoed.is_f64() || declared.is_f64())
            && matches!(
                (echoed.as_f64(), declared.as_f64()),
                (Some(echoed), Some(declared)) if echoed == declared
            ))
}

fn validate_local_parameter_summary(
    [kappa, tau, anisotropy_ratio, major_axis_degrees]: [&Number; 4],
    actual: &LocalParameterSummary,
    identity: &str,
) -> Result<(), &'static str> {
    let invalid = "nonstationary adaptive SPDE local parameter summary differs";
    if actual.region_id != identity
        || !same_number(&actual.kappa, kappa)
        || !same_number(&actual.tau, tau)
        || !same_number(&actual.anisotropy_ratio, anisotropy_ratio)
        || !same_number(&actual.major_axis_degrees, major_axis_degrees)
        || actual.triangle_count == 0
        || !actual.median_triangle_area.is_finite()
        || actual.median_triangle_area <= 0.0
    {
        return Err(invalid);
    }
    let ratio = anisotropy_ratio.as_f64().ok_or(invalid)?;
    let radians = major_axis_degrees.as_f64().ok_or(invalid)?.to_radians();
    let cosine = radians.cos();
    let sine = radians.sin();
    let tensor = [
        [
            ratio * cosine * cosine + sine * sine / ratio,
            (ratio - 1.0 / ratio) * sine * cosine,
        ],
        [
            (ratio - 1.0 / ratio) * sine * cosine,
            ratio * sine * sine + cosine * cosine / ratio,
        ],
    ];
    if actual
        .anisotropy_tensor
        .iter()
        .flatten()
        .zip(tensor.iter().flatten())
        .any(|(left, right)| (left - right).abs() > 1e-12 * right.abs().max(1.0))
    {
        return Err(invalid);
    }
    Ok(())
}

pub fn validate_adaptive_scientific_result(
    result: &AdaptiveWindowSpdeResult,
) -> Result<(), std::io::Error> {
    if result.format != "marklab.adaptive_window_spde"
        || result.version != 1
        || result.claim_status != ClaimStatus::FittedArbitraryWindowSpdeDiagnostic
        || !result.window.exact_area.is_finite()
        || result.window.exact_area <= 0.0
        || !result.mesh.mesh_area.is_finite()
        || result.mesh.mesh_area <= 0.0
        || !result.mesh.minimum_precision_eigenvalue.is_finite()
        || result.mesh.minimum_precision_eigenvalue <= 0.0
        || !result.mesh.maximum_projection_row_sum_error.is_finite()
        || !(0.0..=1e-8).contains(&result.mesh.maximum_projection_row_sum_error)
        || result.mesh_sensitivity.is_empty()
        || result.mesh.vertex_count != result.mesh.vertices.len()
        || result.mesh.triangle_count != result.mesh.triangles.len()
        || result.spatial_factor.mesh_field_weights.len() != result.mesh.vertex_count
        || result.spatial_factor.feature_means.len() != result.spatial_factor.feature_count
        || result.spatial_factor.loadings.len() != result.spatial_factor.feature_count
        || result.spatial_factor.region_ids.len()
            != result.spatial_factor.projected_region_factor.len()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "decoded adaptive SPDE result is scientifically incoherent",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(text: &str) -> Number {
        serde_json::from_str(text).expect("JSON number")
    }

    #[test]
    fn echoed_local_parameters_match_declarations_by_value() {
        let summary = LocalParameterSummary {
            region_id: "background".into(),
            kappa: number("2.0"),
            tau: number("1.5"),
            anisotropy_ratio: number("1.0"),
            major_axis_degrees: number("0.0"),
            anisotropy_tensor: [[1.0, 0.0], [0.0, 1.0]],
            interior_refinement_levels: number("0"),
            triangle_count: 4,
            median_triangle_area: 0.25,
        };
        let [kappa, tau, ratio, axis] = [number("2"), number("1.5"), number("1"), number("0")];
        assert_eq!(
            validate_local_parameter_summary([&kappa, &tau, &ratio, &axis], &summary, "background"),
            Ok(())
        );
        let other_kappa = number("2.5");
        assert!(validate_local_parameter_summary(
            [&other_kappa, &tau, &ratio, &axis],
            &summary,
            "background"
        )
        .is_err());
        // A worker that truncates a fractional limit is still detected.
        assert!(same_number(&number("1000"), &number("1000.0")));
        assert!(!same_number(&number("1000"), &number("1000.7")));
        // Integers above 2^53 compare exactly rather than through f64.
        assert!(!same_number(
            &number("9007199254740993"),
            &number("9007199254740992")
        ));
    }
}
