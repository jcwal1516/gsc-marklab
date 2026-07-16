use crate::registration::{
    landmarks::LandmarkPair,
    qc::registration_qc,
    transform::{fit_affine, fit_similarity, Transform2D},
};

#[test]
fn similarity_transform_recovers_translation_and_scale() {
    let landmarks = vec![
        LandmarkPair::new(0.0, 0.0, 10.0, 20.0),
        LandmarkPair::new(10.0, 0.0, 30.0, 20.0),
        LandmarkPair::new(0.0, 10.0, 10.0, 40.0),
        LandmarkPair::new(10.0, 10.0, 30.0, 40.0),
    ];

    let transform = fit_similarity(&landmarks).expect("similarity");
    assert_eq!(transform.transform_type, "scale_translation");
    let (x, y) = transform.apply(5.0, 5.0);
    assert!((x - 20.0).abs() < 1.0e-9);
    assert!((y - 30.0).abs() < 1.0e-9);
}

#[test]
fn affine_transform_recovers_shear() {
    let landmarks = vec![
        LandmarkPair::new(0.0, 0.0, 1.0, 2.0),
        LandmarkPair::new(1.0, 0.0, 3.0, 2.0),
        LandmarkPair::new(0.0, 1.0, 1.5, 5.0),
        LandmarkPair::new(1.0, 1.0, 3.5, 5.0),
    ];

    let transform = fit_affine(&landmarks).expect("affine");
    let (x, y) = transform.apply(0.5, 0.5);
    assert!((x - 2.25).abs() < 1.0e-9);
    assert!((y - 3.5).abs() < 1.0e-9);
}

#[test]
fn registration_qc_reports_usable_distance_scale() {
    let transform = Transform2D::identity("identity");
    let landmarks = vec![
        LandmarkPair::new(0.0, 0.0, 0.0, 0.0),
        LandmarkPair::new(10.0, 0.0, 11.0, 0.0),
        LandmarkPair::new(0.0, 10.0, 0.0, 12.0),
    ];

    let qc = registration_qc(&landmarks, &transform, 2.0).expect("qc");
    assert_eq!(qc.transform_type, "identity");
    assert_eq!(qc.landmark_count, 3);
    assert!((qc.rmse_um - (5.0_f64 / 3.0).sqrt()).abs() < 1.0e-9);
    assert_eq!(qc.median_residual_um, 1.0);
    assert_eq!(qc.p95_residual_um, 2.0);
    assert_eq!(qc.usable_min_distance_um, 4.0);
    assert_eq!(qc.status, "ok");
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
fn transform_fits_reject_non_finite_landmark_coordinates() {
    let landmarks = vec![
        LandmarkPair::new(0.0, 0.0, 0.0, 0.0),
        LandmarkPair::new(f64::NAN, 1.0, 1.0, 1.0),
        LandmarkPair::new(1.0, 0.0, 1.0, 0.0),
    ];

    assert!(fit_similarity(&landmarks).is_err());
    assert!(fit_affine(&landmarks).is_err());
}

#[test]
fn similarity_rejects_degenerate_source_geometry() {
    let landmarks = vec![
        LandmarkPair::new(1.0, 1.0, 0.0, 0.0),
        LandmarkPair::new(1.0, 1.0, 1.0, 0.0),
    ];

    assert!(fit_similarity(&landmarks).is_err());
}

#[test]
fn registration_qc_rejects_empty_landmarks() {
    let transform = Transform2D::identity("identity");

    assert!(registration_qc(&[], &transform, 2.0).is_err());
}

#[test]
fn registration_qc_rejects_non_finite_or_non_positive_multiplier() {
    let transform = Transform2D::identity("identity");
    let landmarks = vec![LandmarkPair::new(0.0, 0.0, 0.0, 0.0)];

    assert!(registration_qc(&landmarks, &transform, f64::NAN).is_err());
    assert!(registration_qc(&landmarks, &transform, 0.0).is_err());
    assert!(registration_qc(&landmarks, &transform, -1.0).is_err());
}
