use std::f64::consts::PI;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use rand_distr::{Distribution, Normal, Poisson};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::RectangularWindow;

const TRUNCATION_SIGMA_MULTIPLE: f64 = 6.0;

#[derive(Clone, Debug)]
pub struct ThomasProcessSpec {
    pub window: RectangularWindow,
    pub kappa_parent_per_um2: f64,
    pub mu_offspring: f64,
    pub sigma_um: f64,
    pub seed: u64,
    pub maximum_parents: u64,
    pub maximum_offspring: u64,
}

#[derive(Debug, Error)]
pub enum ThomasProcessError {
    #[error("invalid Thomas-process specification: {0}")]
    InvalidSpec(String),
    #[error("Thomas-process resource limit exceeded: {0}")]
    Resource(String),
    #[error("Thomas-process numerical failure: {0}")]
    Numerical(String),
}

#[derive(Debug, Serialize)]
pub struct NeymanScottParent {
    pub parent_id: String,
    pub parent_index: u64,
    pub x_um: f64,
    pub y_um: f64,
    pub generated_offspring: u64,
    pub retained_offspring: u64,
}

#[derive(Debug, Serialize)]
pub struct NeymanScottOffspring {
    pub offspring_id: String,
    pub parent_id: String,
    pub parent_index: u64,
    pub x_um: f64,
    pub y_um: f64,
}

#[derive(Debug, Serialize)]
pub struct ThomasTruncation {
    pub rule: &'static str,
    pub sigma_multiple: f64,
    pub radius_um: f64,
    pub expanded_area_um2: f64,
    pub radial_tail_probability_bound: f64,
    pub parent_window_sampling: &'static str,
    pub exact_infinite_plane: bool,
}

#[derive(Debug, Serialize)]
pub struct NeymanScottCounts {
    pub generated_parents: u64,
    pub generated_offspring: u64,
    pub retained_offspring: u64,
    pub discarded_offspring: u64,
    pub parent_proposals: u64,
}

#[derive(Debug, Serialize)]
pub struct ThomasProcessResult {
    pub format: &'static str,
    pub version: u32,
    pub coordinate_unit: &'static str,
    pub window: RectangularWindow,
    pub kappa_parent_per_um2: f64,
    pub mu_offspring: f64,
    pub sigma_um: f64,
    pub seed: u64,
    pub rng: &'static str,
    pub poisson_sampler: &'static str,
    pub normal_sampler: &'static str,
    pub maximum_parents: u64,
    pub maximum_offspring: u64,
    pub truncation: ThomasTruncation,
    pub counts: NeymanScottCounts,
    pub parents: Vec<NeymanScottParent>,
    pub offspring: Vec<NeymanScottOffspring>,
    pub claim_status: &'static str,
}

pub fn simulate_thomas_process(
    spec: ThomasProcessSpec,
) -> Result<ThomasProcessResult, ThomasProcessError> {
    validate(&spec)?;
    let width = spec.window.xmax_um - spec.window.xmin_um;
    let height = spec.window.ymax_um - spec.window.ymin_um;
    let radius = TRUNCATION_SIGMA_MULTIPLE * spec.sigma_um;
    let expanded_area = dilated_rectangle_area(width, height, radius);
    let parent_mean = spec.kappa_parent_per_um2 * expanded_area;
    let offspring_mean = parent_mean * spec.mu_offspring;
    if !expanded_area.is_finite()
        || !parent_mean.is_finite()
        || !offspring_mean.is_finite()
        || parent_mean <= 0.0
    {
        return Err(ThomasProcessError::Numerical(
            "expanded area or parent expectation is non-finite".into(),
        ));
    }
    if parent_mean > spec.maximum_parents as f64 {
        return Err(ThomasProcessError::Resource(
            "expected parent count exceeds the declared parent cap".into(),
        ));
    }
    if offspring_mean > spec.maximum_offspring as f64 {
        return Err(ThomasProcessError::Resource(
            "expected generated offspring count exceeds the declared offspring cap".into(),
        ));
    }
    let mut rng = ChaCha20Rng::from_seed(derive_seed(b"marklab-thomas-v1\0", spec.seed));
    let parent_distribution = Poisson::new(parent_mean).map_err(|error| {
        ThomasProcessError::Numerical(format!("parent Poisson distribution is invalid: {error}"))
    })?;
    let offspring_distribution = Poisson::new(spec.mu_offspring).map_err(|error| {
        ThomasProcessError::Numerical(format!(
            "offspring Poisson distribution is invalid: {error}"
        ))
    })?;
    let displacement = Normal::new(0.0, spec.sigma_um).map_err(|error| {
        ThomasProcessError::Numerical(format!("offspring Normal distribution is invalid: {error}"))
    })?;
    let parent_count =
        poisson_count(&parent_distribution, &mut rng).map_err(ThomasProcessError::Numerical)?;
    if parent_count > spec.maximum_parents {
        return Err(ThomasProcessError::Resource(
            "realized parent count exceeds the declared parent cap".into(),
        ));
    }
    let (coordinates, proposals) = sample_parent_coordinates(
        &spec.window,
        radius,
        parent_count,
        spec.maximum_parents,
        &mut rng,
    )
    .map_err(ThomasProcessError::Resource)?;
    let mut parents = Vec::with_capacity(coordinates.len());
    let mut offspring = Vec::new();
    let mut generated_offspring = 0_u64;
    for (parent_index, (parent_x, parent_y)) in coordinates.into_iter().enumerate() {
        let generated = poisson_count(&offspring_distribution, &mut rng)
            .map_err(ThomasProcessError::Numerical)?;
        generated_offspring = generated_offspring.checked_add(generated).ok_or_else(|| {
            ThomasProcessError::Resource("generated offspring count overflows".into())
        })?;
        if generated_offspring > spec.maximum_offspring {
            return Err(ThomasProcessError::Resource(
                "realized offspring count exceeds the declared offspring cap".into(),
            ));
        }
        let mut retained = 0_u64;
        for _ in 0..generated {
            let x = parent_x + displacement.sample(&mut rng);
            let y = parent_y + displacement.sample(&mut rng);
            if !x.is_finite() || !y.is_finite() {
                return Err(ThomasProcessError::Numerical(
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
    Ok(ThomasProcessResult {
        format: "marklab.thomas_process_simulation",
        version: 1,
        coordinate_unit: "micrometer",
        window: spec.window,
        kappa_parent_per_um2: spec.kappa_parent_per_um2,
        mu_offspring: spec.mu_offspring,
        sigma_um: spec.sigma_um,
        seed: spec.seed,
        rng: "rand_chacha_0.3.1_chacha20",
        poisson_sampler: "rand_distr_0.4.3_poisson",
        normal_sampler: "rand_distr_0.4.3_normal",
        maximum_parents: spec.maximum_parents,
        maximum_offspring: spec.maximum_offspring,
        truncation: ThomasTruncation {
            rule: "exact_rectangle_minkowski_dilation",
            sigma_multiple: TRUNCATION_SIGMA_MULTIPLE,
            radius_um: radius,
            expanded_area_um2: expanded_area,
            radial_tail_probability_bound: (-0.5
                * TRUNCATION_SIGMA_MULTIPLE
                * TRUNCATION_SIGMA_MULTIPLE)
                .exp(),
            parent_window_sampling: "bounding_rectangle_rejection_to_exact_dilation",
            exact_infinite_plane: false,
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

pub type ThomasParent = NeymanScottParent;
pub type ThomasOffspring = NeymanScottOffspring;
pub type ThomasCounts = NeymanScottCounts;

fn validate(spec: &ThomasProcessSpec) -> Result<(), ThomasProcessError> {
    if ![
        spec.window.xmin_um,
        spec.window.ymin_um,
        spec.window.xmax_um,
        spec.window.ymax_um,
        spec.kappa_parent_per_um2,
        spec.mu_offspring,
        spec.sigma_um,
    ]
    .into_iter()
    .all(f64::is_finite)
        || spec.window.xmin_um >= spec.window.xmax_um
        || spec.window.ymin_um >= spec.window.ymax_um
        || spec.kappa_parent_per_um2 <= 0.0
        || spec.mu_offspring <= 0.0
        || spec.sigma_um <= 0.0
        || !(1..=100_000).contains(&spec.maximum_parents)
        || !(1..=100_000).contains(&spec.maximum_offspring)
    {
        return Err(ThomasProcessError::InvalidSpec(
            "window, positive parameters, or 1-100000 resource caps are invalid".into(),
        ));
    }
    Ok(())
}

pub(crate) fn derive_seed(namespace: &[u8], seed: u64) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(namespace);
    hash.update(seed.to_le_bytes());
    hash.finalize().into()
}

pub(crate) fn poisson_count(
    distribution: &Poisson<f64>,
    rng: &mut ChaCha20Rng,
) -> Result<u64, String> {
    let value = distribution.sample(rng);
    if !value.is_finite() || value < 0.0 || value > u64::MAX as f64 {
        return Err("Poisson sampler returned an invalid count".into());
    }
    Ok(value as u64)
}

pub(crate) fn sample_parent_coordinates(
    window: &RectangularWindow,
    radius: f64,
    parent_count: u64,
    maximum_parents: u64,
    rng: &mut ChaCha20Rng,
) -> Result<(Vec<(f64, f64)>, u64), String> {
    let maximum_proposals = maximum_parents
        .checked_mul(16)
        .and_then(|value| value.checked_add(1_024))
        .ok_or_else(|| "parent proposal cap overflows".to_string())?;
    let mut coordinates = Vec::with_capacity(parent_count as usize);
    let mut proposals = 0_u64;
    while coordinates.len() < parent_count as usize {
        proposals += 1;
        if proposals > maximum_proposals {
            return Err("exact dilated-window parent rejection exceeded its proposal cap".into());
        }
        let x = rng.gen_range(window.xmin_um - radius..window.xmax_um + radius);
        let y = rng.gen_range(window.ymin_um - radius..window.ymax_um + radius);
        if in_dilated_rectangle(x, y, window, radius) {
            coordinates.push((x, y));
        }
    }
    Ok((coordinates, proposals))
}

pub(crate) fn dilated_rectangle_area(width: f64, height: f64, radius: f64) -> f64 {
    width * height + 2.0 * radius * (width + height) + PI * radius * radius
}

pub(crate) fn in_dilated_rectangle(
    x: f64,
    y: f64,
    window: &RectangularWindow,
    radius: f64,
) -> bool {
    let nearest_x = x.clamp(window.xmin_um, window.xmax_um);
    let nearest_y = y.clamp(window.ymin_um, window.ymax_um);
    (x - nearest_x).hypot(y - nearest_y) <= radius
}

pub(crate) fn in_window(x: f64, y: f64, window: &RectangularWindow) -> bool {
    (window.xmin_um..window.xmax_um).contains(&x) && (window.ymin_um..window.ymax_um).contains(&y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_nonpositive_cluster_scale() {
        let result = simulate_thomas_process(ThomasProcessSpec {
            window: RectangularWindow {
                xmin_um: 0.0,
                ymin_um: 0.0,
                xmax_um: 10.0,
                ymax_um: 10.0,
            },
            kappa_parent_per_um2: 0.1,
            mu_offspring: 2.0,
            sigma_um: 0.0,
            seed: 1,
            maximum_parents: 100,
            maximum_offspring: 100,
        });
        assert!(matches!(result, Err(ThomasProcessError::InvalidSpec(_))));
    }
}
