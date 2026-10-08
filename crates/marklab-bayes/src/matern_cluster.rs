use std::f64::consts::TAU;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use rand_distr::Poisson;
use serde::Serialize;
use thiserror::Error;

use crate::{
    thomas::{
        derive_seed, dilated_rectangle_area, in_window, poisson_count, sample_parent_coordinates,
    },
    NeymanScottCounts, NeymanScottOffspring, NeymanScottParent, RectangularWindow,
};

#[derive(Clone, Debug)]
pub struct MaternClusterProcessSpec {
    pub window: RectangularWindow,
    pub kappa_parent_per_um2: f64,
    pub mu_offspring: f64,
    pub radius_um: f64,
    pub seed: u64,
    pub maximum_parents: u64,
    pub maximum_offspring: u64,
}

#[derive(Debug, Error)]
pub enum MaternClusterProcessError {
    #[error("invalid Matérn-cluster specification: {0}")]
    InvalidSpec(String),
    #[error("Matérn-cluster resource limit exceeded: {0}")]
    Resource(String),
    #[error("Matérn-cluster numerical failure: {0}")]
    Numerical(String),
}

#[derive(Debug, Serialize)]
pub struct MaternClusterBoundary {
    pub rule: &'static str,
    pub radius_um: f64,
    pub expanded_area_um2: f64,
    pub parent_window_sampling: &'static str,
    pub displacement_rule: &'static str,
    pub exact_for_bounded_offspring: bool,
}

#[derive(Debug, Serialize)]
pub struct MaternClusterProcessResult {
    pub format: &'static str,
    pub version: u32,
    pub coordinate_unit: &'static str,
    pub window: RectangularWindow,
    pub kappa_parent_per_um2: f64,
    pub mu_offspring: f64,
    pub radius_um: f64,
    pub seed: u64,
    pub rng: &'static str,
    pub poisson_sampler: &'static str,
    pub uniform_sampler: &'static str,
    pub maximum_parents: u64,
    pub maximum_offspring: u64,
    pub boundary: MaternClusterBoundary,
    pub counts: NeymanScottCounts,
    pub parents: Vec<NeymanScottParent>,
    pub offspring: Vec<NeymanScottOffspring>,
    pub claim_status: &'static str,
}

pub fn simulate_matern_cluster_process(
    spec: MaternClusterProcessSpec,
) -> Result<MaternClusterProcessResult, MaternClusterProcessError> {
    validate(&spec)?;
    let width = spec.window.xmax_um - spec.window.xmin_um;
    let height = spec.window.ymax_um - spec.window.ymin_um;
    let expanded_area = dilated_rectangle_area(width, height, spec.radius_um);
    let parent_mean = spec.kappa_parent_per_um2 * expanded_area;
    let offspring_mean = parent_mean * spec.mu_offspring;
    if !expanded_area.is_finite()
        || !parent_mean.is_finite()
        || !offspring_mean.is_finite()
        || parent_mean <= 0.0
    {
        return Err(MaternClusterProcessError::Numerical(
            "expanded area or process expectation is non-finite".into(),
        ));
    }
    if parent_mean > spec.maximum_parents as f64 {
        return Err(MaternClusterProcessError::Resource(
            "expected parent count exceeds the declared parent cap".into(),
        ));
    }
    if offspring_mean > spec.maximum_offspring as f64 {
        return Err(MaternClusterProcessError::Resource(
            "expected generated offspring count exceeds the declared offspring cap".into(),
        ));
    }
    let mut rng = ChaCha20Rng::from_seed(derive_seed(b"marklab-matern-cluster-v1\0", spec.seed));
    let parent_distribution = Poisson::new(parent_mean).map_err(|error| {
        MaternClusterProcessError::Numerical(format!(
            "parent Poisson distribution is invalid: {error}"
        ))
    })?;
    let offspring_distribution = Poisson::new(spec.mu_offspring).map_err(|error| {
        MaternClusterProcessError::Numerical(format!(
            "offspring Poisson distribution is invalid: {error}"
        ))
    })?;
    let parent_count = poisson_count(&parent_distribution, &mut rng)
        .map_err(MaternClusterProcessError::Numerical)?;
    if parent_count > spec.maximum_parents {
        return Err(MaternClusterProcessError::Resource(
            "realized parent count exceeds the declared parent cap".into(),
        ));
    }
    let (coordinates, proposals) = sample_parent_coordinates(
        &spec.window,
        spec.radius_um,
        parent_count,
        spec.maximum_parents,
        &mut rng,
    )
    .map_err(MaternClusterProcessError::Resource)?;
    let mut parents = Vec::with_capacity(coordinates.len());
    let mut offspring = Vec::new();
    let mut generated_offspring = 0_u64;
    for (parent_index, (parent_x, parent_y)) in coordinates.into_iter().enumerate() {
        let generated = poisson_count(&offspring_distribution, &mut rng)
            .map_err(MaternClusterProcessError::Numerical)?;
        generated_offspring = generated_offspring.checked_add(generated).ok_or_else(|| {
            MaternClusterProcessError::Resource("generated offspring count overflows".into())
        })?;
        if generated_offspring > spec.maximum_offspring {
            return Err(MaternClusterProcessError::Resource(
                "realized offspring count exceeds the declared offspring cap".into(),
            ));
        }
        let mut retained = 0_u64;
        for _ in 0..generated {
            let radial = spec.radius_um * rng.gen::<f64>().sqrt();
            let angle = TAU * rng.gen::<f64>();
            let (sin, cos) = angle.sin_cos();
            let x = parent_x + radial * cos;
            let y = parent_y + radial * sin;
            if !x.is_finite() || !y.is_finite() {
                return Err(MaternClusterProcessError::Numerical(
                    "offspring coordinate is non-finite".into(),
                ));
            }
            if in_window(x, y, &spec.window) {
                let offspring_index = offspring.len();
                offspring.push(NeymanScottOffspring {
                    offspring_id: format!("offspring:{offspring_index}"),
                    parent_id: format!("parent:{parent_index}"),
                    parent_index: parent_index as u64,
                    x_um: x,
                    y_um: y,
                });
                retained += 1;
            }
        }
        parents.push(NeymanScottParent {
            parent_id: format!("parent:{parent_index}"),
            parent_index: parent_index as u64,
            x_um: parent_x,
            y_um: parent_y,
            generated_offspring: generated,
            retained_offspring: retained,
        });
    }
    let retained_offspring = offspring.len() as u64;
    Ok(MaternClusterProcessResult {
        format: "marklab.matern_cluster_process_simulation",
        version: 1,
        coordinate_unit: "micrometer",
        window: spec.window,
        kappa_parent_per_um2: spec.kappa_parent_per_um2,
        mu_offspring: spec.mu_offspring,
        radius_um: spec.radius_um,
        seed: spec.seed,
        rng: "rand_chacha_0.3.1_chacha20",
        poisson_sampler: "rand_distr_0.4.3_poisson",
        uniform_sampler: "rand_0.8.6_closed_open_unit_transform",
        maximum_parents: spec.maximum_parents,
        maximum_offspring: spec.maximum_offspring,
        boundary: MaternClusterBoundary {
            rule: "exact_rectangle_minkowski_dilation",
            radius_um: spec.radius_um,
            expanded_area_um2: expanded_area,
            parent_window_sampling: "bounding_rectangle_rejection_to_exact_dilation",
            displacement_rule: "uniform_disc_radius_R_sqrt_U_angle_2pi_V",
            exact_for_bounded_offspring: true,
        },
        counts: NeymanScottCounts {
            generated_parents: parents.len() as u64,
            generated_offspring,
            retained_offspring,
            discarded_offspring: generated_offspring - retained_offspring,
            parent_proposals: proposals,
        },
        parents,
        offspring,
        claim_status: "experimental_simulation",
    })
}

fn validate(spec: &MaternClusterProcessSpec) -> Result<(), MaternClusterProcessError> {
    if ![
        spec.window.xmin_um,
        spec.window.ymin_um,
        spec.window.xmax_um,
        spec.window.ymax_um,
        spec.kappa_parent_per_um2,
        spec.mu_offspring,
        spec.radius_um,
    ]
    .into_iter()
    .all(f64::is_finite)
        || spec.window.xmin_um >= spec.window.xmax_um
        || spec.window.ymin_um >= spec.window.ymax_um
        || spec.kappa_parent_per_um2 <= 0.0
        || spec.mu_offspring <= 0.0
        || spec.radius_um <= 0.0
        || !(1..=100_000).contains(&spec.maximum_parents)
        || !(1..=100_000).contains(&spec.maximum_offspring)
    {
        return Err(MaternClusterProcessError::InvalidSpec(
            "window, positive parameters, or 1-100000 resource caps are invalid".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_nonpositive_offspring_radius() {
        let result = simulate_matern_cluster_process(MaternClusterProcessSpec {
            window: RectangularWindow {
                xmin_um: 0.0,
                ymin_um: 0.0,
                xmax_um: 10.0,
                ymax_um: 10.0,
            },
            kappa_parent_per_um2: 0.1,
            mu_offspring: 2.0,
            radius_um: 0.0,
            seed: 1,
            maximum_parents: 100,
            maximum_offspring: 100,
        });
        assert!(matches!(
            result,
            Err(MaternClusterProcessError::InvalidSpec(_))
        ));
    }
}
