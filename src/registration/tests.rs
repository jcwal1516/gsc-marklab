use crate::registration::{
    landmarks::LandmarkPair,
    qc::registration_qc,
    transform::{fit_affine, fit_rigid, Transform2D, TransformKind},
};

const RIGID_TOLERANCE: f64 = 1.0e-9;

fn rigid_landmarks(
    source: &[(f64, f64)],
    theta_radians: f64,
    translation: (f64, f64),
) -> Vec<LandmarkPair> {
    let cosine = theta_radians.cos();
    let sine = theta_radians.sin();
    source
        .iter()
        .map(|(x, y)| {
            LandmarkPair::new(
                *x,
                *y,
                cosine * x - sine * y + translation.0,
                sine * x + cosine * y + translation.1,
            )
        })
        .collect()
}

fn assert_maps_landmarks(transform: &Transform2D, landmarks: &[LandmarkPair], tolerance: f64) {
    for landmark in landmarks {
        let (x, y) = transform.apply(landmark.source_x_um, landmark.source_y_um);
        assert!((x - landmark.target_x_um).abs() <= tolerance, "x={x}");
        assert!((y - landmark.target_y_um).abs() <= tolerance, "y={y}");
    }
}

#[test]
fn rigid_identity() {
    let landmarks = rigid_landmarks(&[(0.0, 0.0), (2.0, 0.0), (0.0, 3.0)], 0.0, (0.0, 0.0));

    let transform = fit_rigid(&landmarks).expect("rigid identity");

    assert_eq!(transform.transform_type, TransformKind::Rigid);
    assert_maps_landmarks(&transform, &landmarks, RIGID_TOLERANCE);
}

#[test]
fn rigid_translation() {
    let landmarks = rigid_landmarks(&[(0.0, 0.0), (2.0, 0.0), (0.0, 3.0)], 0.0, (7.0, -4.0));

    let transform = fit_rigid(&landmarks).expect("rigid translation");

    assert_maps_landmarks(&transform, &landmarks, RIGID_TOLERANCE);
}

#[test]
fn rigid_rotation_90_degrees() {
    let landmarks = rigid_landmarks(
        &[(0.0, 0.0), (2.0, 0.0), (0.0, 3.0), (2.0, 3.0)],
        std::f64::consts::FRAC_PI_2,
        (0.0, 0.0),
    );

    let transform = fit_rigid(&landmarks).expect("90 degree rotation");

    assert_maps_landmarks(&transform, &landmarks, RIGID_TOLERANCE);
}

#[test]
fn rigid_rotation_and_translation() {
    let landmarks = rigid_landmarks(
        &[(0.0, 0.0), (2.0, 0.0), (0.0, 3.0), (2.0, 3.0)],
        0.37,
        (10.0, -4.0),
    );

    let transform = fit_rigid(&landmarks).expect("rotation and translation");

    assert_maps_landmarks(&transform, &landmarks, RIGID_TOLERANCE);
}

#[test]
fn rigid_preserves_distance() {
    let landmarks = rigid_landmarks(&[(-2.0, 1.0), (3.0, -4.0), (5.0, 6.0)], -0.8, (2.0, 9.0));
    let transform = fit_rigid(&landmarks).expect("rigid fit");
    let first = transform.apply(-2.0, 1.0);
    let second = transform.apply(3.0, -4.0);

    assert!(first.0.is_finite() && first.1.is_finite());
    assert!(
        ((first.0 - second.0).hypot(first.1 - second.1) - 5.0_f64.hypot(-5.0)).abs()
            < RIGID_TOLERANCE
    );
}

#[test]
fn rigid_does_not_absorb_scale() {
    let landmarks = vec![
        LandmarkPair::new(0.0, 0.0, 0.0, 0.0),
        LandmarkPair::new(2.0, 0.0, 4.0, 0.0),
        LandmarkPair::new(0.0, 2.0, 0.0, 4.0),
    ];

    let transform = fit_rigid(&landmarks).expect("best rigid fit");
    let mapped_origin = transform.apply(0.0, 0.0);
    let mapped_x = transform.apply(2.0, 0.0);

    assert!(
        ((mapped_x.0 - mapped_origin.0).hypot(mapped_x.1 - mapped_origin.1) - 2.0).abs()
            < RIGID_TOLERANCE
    );
    assert!(
        (transform.m00 * transform.m11 - transform.m01 * transform.m10 - 1.0).abs()
            < RIGID_TOLERANCE
    );
}

#[test]
fn rigid_rejects_degenerate_landmarks() {
    let landmarks = vec![
        LandmarkPair::new(1.0, 1.0, 0.0, 0.0),
        LandmarkPair::new(1.0, 1.0, 2.0, 3.0),
    ];

    assert!(fit_rigid(&landmarks).is_err());
}

#[test]
fn rigid_handles_small_noise() {
    let mut landmarks = rigid_landmarks(
        &[(-2.0, -1.0), (2.0, -1.0), (-2.0, 1.0), (2.0, 1.0)],
        0.3,
        (5.0, -2.0),
    );
    for (index, landmark) in landmarks.iter_mut().enumerate() {
        let noise = (index as f64 - 1.5) * 1.0e-4;
        landmark.target_x_um += noise;
        landmark.target_y_um -= noise * 0.5;
    }

    let transform = fit_rigid(&landmarks).expect("noisy rigid fit");
    let expected = rigid_landmarks(&[(1.25, -0.75)], 0.3, (5.0, -2.0))[0].clone();
    let (x, y) = transform.apply(expected.source_x_um, expected.source_y_um);

    assert!((x - expected.target_x_um).abs() < 5.0e-4);
    assert!((y - expected.target_y_um).abs() < 5.0e-4);
}

#[test]
fn rigid_result_is_finite() {
    let landmarks = rigid_landmarks(
        &[(-1.0e150, 0.0), (1.0e150, 0.0), (0.0, 1.0e150)],
        0.1,
        (2.0e149, -3.0e149),
    );

    let transform = fit_rigid(&landmarks).expect("finite large-coordinate fit");

    assert!([
        transform.m00,
        transform.m01,
        transform.m02,
        transform.m10,
        transform.m11,
        transform.m12
    ]
    .into_iter()
    .all(f64::is_finite));
}

#[test]
fn rigid_does_not_fit_reflection() {
    let landmarks = vec![
        LandmarkPair::new(0.0, 0.0, 0.0, 0.0),
        LandmarkPair::new(2.0, 0.0, -2.0, 0.0),
        LandmarkPair::new(0.0, 3.0, 0.0, 3.0),
    ];

    let transform = fit_rigid(&landmarks).expect("orientation-preserving best fit");
    let determinant = transform.m00 * transform.m11 - transform.m01 * transform.m10;
    let residual_sum = landmarks
        .iter()
        .map(|landmark| {
            let (x, y) = transform.apply(landmark.source_x_um, landmark.source_y_um);
            (x - landmark.target_x_um).hypot(y - landmark.target_y_um)
        })
        .sum::<f64>();

    assert!((determinant - 1.0).abs() < RIGID_TOLERANCE);
    assert!(residual_sum > 1.0);
}

#[test]
fn affine_transform_recovers_shear() {
    for offset in [0.0, 100_000.0] {
        let landmarks = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
            .into_iter()
            .map(|(x, y)| {
                LandmarkPair::new(
                    x + offset,
                    y + offset,
                    2.0 * x + 0.5 * y + 1.0,
                    3.0 * y + 2.0,
                )
            })
            .collect::<Vec<_>>();
        let transform = fit_affine(&landmarks).expect("affine in translated coordinates");
        assert_maps_landmarks(&transform, &landmarks, 1.0e-9);
        let (x, y) = transform.apply(offset + 0.5, offset + 0.5);
        assert!((x - 2.25).abs() < 1.0e-9);
        assert!((y - 3.5).abs() < 1.0e-9);
    }
}

#[test]
fn affine_least_squares_ignores_noise_orthogonal_to_the_design() {
    // A parallelogram has correlated source axes with unequal spreads. The
    // +,-,-,+ contrast is orthogonal to {1, x, y} on these corners, so the
    // least-squares map is exactly the generating map and each residual is
    // the perturbation norm hypot(0.3, 0.4) = 0.5. An interpolating or
    // uncorrelated-axis fit cannot reproduce both.
    for offset in [0.0, 100_000.0] {
        let landmarks = [
            (0.0, 0.0, 1.0),
            (4.0, 0.0, -1.0),
            (1.0, 1.0, -1.0),
            (5.0, 1.0, 1.0),
        ]
        .into_iter()
        .map(|(x, y, sign)| {
            LandmarkPair::new(
                x + offset,
                y + offset,
                2.0 * x + 0.5 * y + 1.0 + 0.3 * sign,
                -0.25 * x + 3.0 * y + 2.0 - 0.4 * sign,
            )
        })
        .collect::<Vec<_>>();
        let transform = fit_affine(&landmarks).expect("least-squares affine");
        for (actual, expected) in [
            (transform.m00, 2.0),
            (transform.m01, 0.5),
            (transform.m10, -0.25),
            (transform.m11, 3.0),
        ] {
            assert!((actual - expected).abs() < 1.0e-9, "{transform:?}");
        }
        let (x, y) = transform.apply(offset + 2.0, offset - 3.0);
        assert!((x - 3.5).abs() < 1.0e-9, "x={x}");
        assert!((y - -7.5).abs() < 1.0e-9, "y={y}");
        let qc = registration_qc(&landmarks, &transform, 1.0).expect("qc");
        assert!((qc.rmse_um - 0.5).abs() < 1.0e-9, "{qc:?}");
        assert!((qc.max_residual_um - 0.5).abs() < 1.0e-9, "{qc:?}");
    }
}

#[test]
fn registration_qc_reports_usable_distance_scale() {
    let transform = Transform2D::identity();
    let landmarks = vec![
        LandmarkPair::new(0.0, 0.0, 0.0, 0.0),
        LandmarkPair::new(10.0, 0.0, 11.0, 0.0),
        LandmarkPair::new(0.0, 10.0, 0.0, 12.0),
    ];

    let qc = registration_qc(&landmarks, &transform, 2.0).expect("qc");
    assert_eq!(qc.transform_type, TransformKind::Identity);
    assert_eq!(qc.landmark_count, 3);
    assert!((qc.rmse_um - (5.0_f64 / 3.0).sqrt()).abs() < 1.0e-9);
    assert_eq!(qc.median_residual_um, 1.0);
    assert_eq!(qc.p95_residual_um, 2.0);
    assert_eq!(qc.usable_min_distance_um, 4.0);
}

#[test]
fn affine_rejects_fewer_than_three_landmarks() {
    let landmarks = vec![
        LandmarkPair::new(0.0, 0.0, 0.0, 0.0),
        LandmarkPair::new(1.0, 0.0, 1.0, 0.0),
    ];

    assert!(fit_affine(&landmarks).is_err());
}

#[test]
fn affine_rejects_collinear_landmarks() {
    let landmarks = vec![
        LandmarkPair::new(0.0, 0.0, 0.0, 0.0),
        LandmarkPair::new(1.0, 1.0, 2.0, 2.0),
        LandmarkPair::new(2.0, 2.0, 4.0, 4.0),
    ];

    assert!(fit_affine(&landmarks).is_err());
}

#[test]
fn affine_rejects_singular_target_geometry() {
    let landmarks = vec![
        LandmarkPair::new(0.0, 0.0, 0.0, 0.0),
        LandmarkPair::new(1.0, 0.0, 1.0, 0.0),
        LandmarkPair::new(0.0, 1.0, 0.0, 0.0),
        LandmarkPair::new(1.0, 1.0, 1.0, 0.0),
    ];

    assert!(fit_affine(&landmarks).is_err());

    // Both Jacobian rows are nonzero but parallel: [0.3, -1.7] and
    // [-0.6, 3.4] = -2 * [0.3, -1.7]. Every target lies on one line, so the
    // exact least-squares map has zero residual and zero determinant even
    // though rounding prevents an exactly zero computed determinant. Slide-
    // scale offsets in both frames raise that rounding level.
    for offset in [0.0, 25_000.0, 100_000.0] {
        let collapsed = [(0.1, 0.7), (1.3, 0.2), (0.4, 1.9), (2.2, 1.1)]
            .into_iter()
            .map(|(x, y)| {
                LandmarkPair::new(
                    x + offset,
                    y + offset,
                    0.3 * x - 1.7 * y + 5.1 + offset,
                    -0.6 * x + 3.4 * y + 0.9 + offset,
                )
            })
            .collect::<Vec<_>>();
        assert!(
            fit_affine(&collapsed).is_err(),
            "offset {offset}: {:?}",
            fit_affine(&collapsed)
        );
    }
}

#[test]
fn affine_accepts_reflections_and_tiny_or_unequal_axis_scales() {
    // Raw determinants are -1, 1e-12 and 1e-3; none of these maps collapses.
    for jacobian in [
        [[-1.0, 0.0], [0.0, 1.0]],
        [[1.0e-6, 0.0], [0.0, 1.0e-6]],
        [[1.0e-6, 2.0e-6], [0.0, 1.0e3]],
    ] {
        let landmarks = [(0.0, 0.0), (3.0, 0.0), (0.0, 2.0), (3.0, 2.0)]
            .into_iter()
            .map(|(x, y)| {
                LandmarkPair::new(
                    x,
                    y,
                    jacobian[0][0] * x + jacobian[0][1] * y + 7.0,
                    jacobian[1][0] * x + jacobian[1][1] * y - 4.0,
                )
            })
            .collect::<Vec<_>>();
        let transform = fit_affine(&landmarks).expect("nonsingular affine map");
        for (actual, expected) in [
            (transform.m00, jacobian[0][0]),
            (transform.m01, jacobian[0][1]),
            (transform.m10, jacobian[1][0]),
            (transform.m11, jacobian[1][1]),
        ] {
            assert!(
                (actual - expected).abs() <= 1.0e-9 * expected.abs().max(1.0e-6),
                "{jacobian:?}: {transform:?}"
            );
        }
    }
}

#[test]
fn transform_fits_reject_non_finite_landmark_coordinates() {
    let landmarks = vec![
        LandmarkPair::new(0.0, 0.0, 0.0, 0.0),
        LandmarkPair::new(f64::NAN, 1.0, 1.0, 1.0),
        LandmarkPair::new(1.0, 0.0, 1.0, 0.0),
    ];

    assert!(fit_rigid(&landmarks).is_err());
    assert!(fit_affine(&landmarks).is_err());
}

#[test]
fn rigid_rotation() {
    let landmarks = vec![
        LandmarkPair::new(0.0, 0.0, 10.0, -4.0),
        LandmarkPair::new(2.0, 0.0, 10.0, -2.0),
        LandmarkPair::new(0.0, 3.0, 7.0, -4.0),
        LandmarkPair::new(2.0, 3.0, 7.0, -2.0),
    ];

    let transform = fit_rigid(&landmarks).expect("configured rigid fit");
    let (x, y) = transform.apply(1.0, 2.0);

    assert!((x - 8.0).abs() < 1.0e-9, "x={x}, transform={transform:?}");
    assert!((y - -3.0).abs() < 1.0e-9, "y={y}, transform={transform:?}");
    assert!((transform.m00.hypot(transform.m10) - 1.0).abs() < 1.0e-9);
    assert!((transform.m01.hypot(transform.m11) - 1.0).abs() < 1.0e-9);
}

#[test]
fn registration_qc_rejects_empty_landmarks() {
    let transform = Transform2D::identity();

    assert!(registration_qc(&[], &transform, 2.0).is_err());
}

#[test]
fn registration_qc_rejects_non_finite_or_non_positive_multiplier() {
    let transform = Transform2D::identity();
    let landmarks = vec![LandmarkPair::new(0.0, 0.0, 0.0, 0.0)];

    assert!(registration_qc(&landmarks, &transform, f64::NAN).is_err());
    assert!(registration_qc(&landmarks, &transform, 0.0).is_err());
    assert!(registration_qc(&landmarks, &transform, -1.0).is_err());
}
