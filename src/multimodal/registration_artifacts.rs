use serde::{Deserialize, Serialize};

use crate::{
    errors::Result,
    geom::convex_hull::{ConvexHull2D, Point2},
    multimodal::cells::{CellSection, FusedCell},
    registration::{landmarks::LandmarkPair, transform::Transform2D},
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RegistrationResidual {
    pub landmark_index: usize,
    pub source_x_um: f64,
    pub source_y_um: f64,
    pub target_x_um: f64,
    pub target_y_um: f64,
    pub transformed_x_um: f64,
    pub transformed_y_um: f64,
    pub residual_dx_um: f64,
    pub residual_dy_um: f64,
    pub residual_um: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LandmarkHullAvailability {
    Assessable,
    InsufficientUniqueLandmarks,
    DegenerateCollinearLandmarks,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CellExtrapolationRecord {
    pub source_section: CellSection,
    pub source_cell_id: String,
    pub x_um_registered: f64,
    pub y_um_registered: f64,
    pub outside_landmark_hull: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RegistrationExtrapolation {
    pub availability: LandmarkHullAvailability,
    pub landmark_hull_unique_points: usize,
    pub n_cells: usize,
    pub n_outside_landmark_hull: Option<usize>,
    pub fraction_outside_landmark_hull: Option<f64>,
    pub cell_flags: Vec<CellExtrapolationRecord>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RegistrationArtifacts {
    pub residuals: Vec<RegistrationResidual>,
    pub extrapolation: RegistrationExtrapolation,
}

pub(super) fn analyze_registration_artifacts(
    landmarks: &[LandmarkPair],
    transform: &Transform2D,
    fused: &[FusedCell],
) -> Result<RegistrationArtifacts> {
    let residuals = registration_residuals(landmarks, transform);
    let hull = ConvexHull2D::from_points(
        &landmarks
            .iter()
            .map(|landmark| Point2::new(landmark.target_x_um, landmark.target_y_um))
            .collect::<Vec<_>>(),
    )?;
    let availability = match &hull {
        ConvexHull2D::Polygon { .. } => LandmarkHullAvailability::Assessable,
        ConvexHull2D::InsufficientUniquePoints { .. } => {
            LandmarkHullAvailability::InsufficientUniqueLandmarks
        }
        ConvexHull2D::Collinear { .. } => LandmarkHullAvailability::DegenerateCollinearLandmarks,
    };
    let cell_flags = fused
        .iter()
        .map(|cell| CellExtrapolationRecord {
            source_section: cell.source_section,
            source_cell_id: cell.source_cell_id.clone(),
            x_um_registered: cell.x_um_registered,
            y_um_registered: cell.y_um_registered,
            outside_landmark_hull: hull
                .contains(Point2::new(cell.x_um_registered, cell.y_um_registered))
                .map(|inside| !inside),
        })
        .collect::<Vec<_>>();
    let n_outside_landmark_hull =
        (availability == LandmarkHullAvailability::Assessable).then(|| {
            cell_flags
                .iter()
                .filter(|record| record.outside_landmark_hull == Some(true))
                .count()
        });
    let fraction_outside_landmark_hull = n_outside_landmark_hull
        .filter(|_| !cell_flags.is_empty())
        .map(|outside| outside as f64 / cell_flags.len() as f64);

    Ok(RegistrationArtifacts {
        residuals,
        extrapolation: RegistrationExtrapolation {
            availability,
            landmark_hull_unique_points: hull.unique_points(),
            n_cells: cell_flags.len(),
            n_outside_landmark_hull,
            fraction_outside_landmark_hull,
            cell_flags,
        },
    })
}

fn registration_residuals(
    landmarks: &[LandmarkPair],
    transform: &Transform2D,
) -> Vec<RegistrationResidual> {
    landmarks
        .iter()
        .enumerate()
        .map(|(landmark_index, landmark)| {
            let (transformed_x_um, transformed_y_um) =
                transform.apply(landmark.source_x_um, landmark.source_y_um);
            let residual_dx_um = transformed_x_um - landmark.target_x_um;
            let residual_dy_um = transformed_y_um - landmark.target_y_um;
            RegistrationResidual {
                landmark_index,
                source_x_um: landmark.source_x_um,
                source_y_um: landmark.source_y_um,
                target_x_um: landmark.target_x_um,
                target_y_um: landmark.target_y_um,
                transformed_x_um,
                transformed_y_um,
                residual_dx_um,
                residual_dy_um,
                residual_um: residual_dx_um.hypot(residual_dy_um),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::{
        multimodal::cells::{CellSection, FusedCell},
        registration::{landmarks::LandmarkPair, transform::fit_affine},
    };

    use super::{analyze_registration_artifacts, LandmarkHullAvailability};

    #[test]
    fn registration_extrapolation_boundary() {
        let landmarks = square_landmarks();
        let transform = fit_affine(&landmarks).expect("identity transform");
        let fused = vec![
            fused_cell("inside", 5.0, 5.0),
            fused_cell("boundary", 0.0, 5.0),
            fused_cell("outside", 11.0, 5.0),
        ];

        let artifacts =
            analyze_registration_artifacts(&landmarks, &transform, &fused).expect("artifacts");

        assert_eq!(
            artifacts.extrapolation.availability,
            LandmarkHullAvailability::Assessable
        );
        assert_eq!(artifacts.extrapolation.n_outside_landmark_hull, Some(1));
        assert_eq!(
            artifacts.extrapolation.fraction_outside_landmark_hull,
            Some(1.0 / 3.0)
        );
        assert_eq!(
            artifacts
                .extrapolation
                .cell_flags
                .iter()
                .map(|record| record.outside_landmark_hull)
                .collect::<Vec<_>>(),
            vec![Some(false), Some(false), Some(true)]
        );
    }

    #[test]
    fn registration_extrapolation_reports_degenerate_landmarks() {
        let landmarks = vec![
            LandmarkPair::new(0.0, 0.0, 0.0, 0.0),
            LandmarkPair::new(5.0, 0.0, 5.0, 0.0),
            LandmarkPair::new(10.0, 0.0, 10.0, 0.0),
        ];
        let transform = fit_affine(&square_landmarks()).expect("identity transform");

        let artifacts =
            analyze_registration_artifacts(&landmarks, &transform, &[fused_cell("cell", 5.0, 1.0)])
                .expect("artifacts");

        assert_eq!(
            artifacts.extrapolation.availability,
            LandmarkHullAvailability::DegenerateCollinearLandmarks
        );
        assert_eq!(artifacts.extrapolation.n_outside_landmark_hull, None);
        assert_eq!(artifacts.extrapolation.fraction_outside_landmark_hull, None);
        assert_eq!(
            artifacts.extrapolation.cell_flags[0].outside_landmark_hull,
            None
        );
    }

    #[test]
    fn registration_extrapolation_reports_insufficient_unique_landmarks() {
        let transform = fit_affine(&square_landmarks()).expect("identity transform");
        for landmarks in [
            Vec::new(),
            vec![LandmarkPair::new(0.0, 0.0, 0.0, 0.0)],
            vec![
                LandmarkPair::new(0.0, 0.0, 0.0, 0.0),
                LandmarkPair::new(1.0, 0.0, 1.0, 0.0),
                LandmarkPair::new(1.0, 0.0, 1.0, 0.0),
            ],
        ] {
            let artifacts = analyze_registration_artifacts(
                &landmarks,
                &transform,
                &[fused_cell("cell", 0.0, 0.0)],
            )
            .expect("artifacts");

            assert_eq!(
                artifacts.extrapolation.availability,
                LandmarkHullAvailability::InsufficientUniqueLandmarks
            );
            assert_eq!(artifacts.extrapolation.n_outside_landmark_hull, None);
            assert_eq!(
                artifacts.extrapolation.cell_flags[0].outside_landmark_hull,
                None
            );
        }
    }

    #[test]
    fn registration_extrapolation_is_order_independent_and_tolerates_boundary_noise() {
        let mut reversed = square_landmarks();
        reversed.reverse();
        let transform = fit_affine(&square_landmarks()).expect("identity transform");
        let fused = vec![fused_cell("near-boundary", -1.0e-13, 5.0)];

        let forward = analyze_registration_artifacts(&square_landmarks(), &transform, &fused)
            .expect("forward artifacts");
        let backward = analyze_registration_artifacts(&reversed, &transform, &fused)
            .expect("backward artifacts");

        assert_eq!(forward.extrapolation, backward.extrapolation);
        assert_eq!(
            forward.extrapolation.cell_flags[0].outside_landmark_hull,
            Some(false)
        );
    }

    #[test]
    fn registration_extrapolation_empty_cells_has_no_fraction() {
        let landmarks = square_landmarks();
        let transform = fit_affine(&landmarks).expect("identity transform");

        let artifacts =
            analyze_registration_artifacts(&landmarks, &transform, &[]).expect("empty artifacts");

        assert_eq!(
            artifacts.extrapolation.availability,
            LandmarkHullAvailability::Assessable
        );
        assert_eq!(artifacts.extrapolation.n_outside_landmark_hull, Some(0));
        assert_eq!(artifacts.extrapolation.fraction_outside_landmark_hull, None);
    }

    #[test]
    fn registration_residuals_use_the_application_transform() {
        let landmarks = square_landmarks();
        let transform = fit_affine(&landmarks).expect("identity transform");

        let artifacts =
            analyze_registration_artifacts(&landmarks, &transform, &[]).expect("artifacts");

        assert_eq!(artifacts.residuals.len(), landmarks.len());
        assert!(artifacts.residuals.iter().all(|residual| {
            residual.residual_dx_um.abs() < 1.0e-12
                && residual.residual_dy_um.abs() < 1.0e-12
                && residual.residual_um.abs() < 1.0e-12
        }));
    }

    fn square_landmarks() -> Vec<LandmarkPair> {
        vec![
            LandmarkPair::new(0.0, 0.0, 0.0, 0.0),
            LandmarkPair::new(10.0, 0.0, 10.0, 0.0),
            LandmarkPair::new(0.0, 10.0, 0.0, 10.0),
            LandmarkPair::new(10.0, 10.0, 10.0, 10.0),
        ]
    }

    fn fused_cell(cell_id: &str, x: f64, y: f64) -> FusedCell {
        FusedCell {
            source_section: CellSection::He,
            source_cell_id: cell_id.into(),
            x_um_registered: x,
            y_um_registered: y,
            mmr_mark: None,
            mmr_probability: None,
            cell_type: Some("tumor".into()),
            cell_type_probability: Some(0.9),
            same_section: false,
            registration_error_um: Some(0.1),
        }
    }
}
