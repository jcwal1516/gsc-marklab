use crate::{
    data::{Pattern, PatternMeta},
    errors::{MarklabError, Result},
    permutation::labels::permute_fixed_count,
};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct MultimodalOutcome {
    pub(super) detected: bool,
    pub(super) false_positive: bool,
    pub(super) below_registration_resolution: bool,
    pub(super) within_margin: bool,
}

pub(super) fn multimodal_replicate_outcome(
    generator: &str,
    seed: u64,
    generator_index: u64,
    replicate: usize,
) -> Result<MultimodalOutcome> {
    let mut rng = DeterministicRng::new(
        seed ^ (generator_index.wrapping_mul(0x9e37_79b9_7f4a_7c15))
            ^ ((replicate as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9)),
    );
    let outcome = match generator {
        "two_unrelated_mmr_territories" => {
            let centers = [
                SyntheticPoint { x: 20.0, y: 20.0 },
                SyntheticPoint { x: 82.0, y: 78.0 },
            ];
            let baseline_gap_um = distance(centers[0], centers[1]);
            let observed_relation_um = baseline_gap_um - 40.0 + rng.centered(12.0);
            let detected = observed_relation_um < 35.0;
            MultimodalOutcome {
                detected,
                false_positive: detected,
                below_registration_resolution: false,
                within_margin: false,
            }
        }
        "two_related_mmr_territories" => {
            let centers = [
                SyntheticPoint { x: 40.0, y: 48.0 },
                SyntheticPoint { x: 58.0, y: 51.0 },
            ];
            let observed_relation_um = distance(centers[0], centers[1]) + rng.centered(8.0);
            let bridge_support = 0.70 + rng.centered(0.16);
            let detected = observed_relation_um < 28.0 && bridge_support > 0.58;
            MultimodalOutcome {
                detected,
                false_positive: false,
                below_registration_resolution: false,
                within_margin: false,
            }
        }
        "immune_associated_mmr_territory" => {
            let registration_error_um = 4.0 + rng.unit() * 2.0;
            let enrichment = 2.45 + rng.centered(0.65);
            let detected = enrichment > 2.0 && registration_error_um < 10.0;
            MultimodalOutcome {
                detected,
                false_positive: false,
                below_registration_resolution: false,
                within_margin: false,
            }
        }
        "registration_jitter" => {
            let registration_error_um = 12.0 + rng.unit() * 6.0;
            let observed_association_scale_um = 20.0 + rng.centered(8.0);
            let below_registration_resolution =
                observed_association_scale_um < 2.0 * registration_error_um;
            let apparent_association = observed_association_scale_um < 23.0;
            MultimodalOutcome {
                detected: apparent_association,
                false_positive: apparent_association && !below_registration_resolution,
                below_registration_resolution,
                within_margin: false,
            }
        }
        "prepost_within_margin_spatial_pattern" => {
            let curve_delta = (0.085 + rng.centered(0.08)).abs();
            let margin = 0.15;
            let within_margin = curve_delta <= margin;
            let changed = curve_delta > 0.25;
            MultimodalOutcome {
                detected: changed,
                false_positive: changed,
                below_registration_resolution: false,
                within_margin,
            }
        }
        "prepost_changed_spatial_pattern" => {
            let curve_delta = 0.33 + rng.centered(0.10);
            let margin = 0.15;
            let changed = curve_delta > 0.25;
            MultimodalOutcome {
                detected: changed,
                false_positive: false,
                below_registration_resolution: false,
                within_margin: curve_delta <= margin,
            }
        }
        _ => {
            return Err(MarklabError::Validation(format!(
                "unknown multimodal synthetic generator {generator}"
            )));
        }
    };
    Ok(outcome)
}

#[derive(Clone, Copy, Debug)]
struct SyntheticPoint {
    x: f64,
    y: f64,
}

fn distance(a: SyntheticPoint, b: SyntheticPoint) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

#[derive(Clone, Copy, Debug)]
struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0xa076_1d64_78bd_642f,
        }
    }

    fn unit(&mut self) -> f64 {
        let value = self.next_u64() >> 11;
        value as f64 / ((1_u64 << 53) as f64)
    }

    fn centered(&mut self, width: f64) -> f64 {
        (2.0 * self.unit() - 1.0) * width
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state ^ (self.state >> 33)
    }
}

pub(super) fn synthetic_pattern(generator: &str, replicate: u64) -> Result<Pattern> {
    let width = 12;
    let height = 12;
    let seed = 100_003_u64 ^ replicate.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let random_mark_count = width * height / 4;
    let mut marks = match generator {
        "random_labeling" | "cell_density_gradient_random_labels" => {
            permute_fixed_count(width * height, random_mark_count, seed)?
        }
        "single_gaussian_cluster" => clustered_marks(width, height, &[(5.5, 5.5, 3.0)]),
        "single_matern_cluster" => {
            clustered_marks(width, height, &[(3.0, 3.0, 2.1), (8.5, 8.0, 2.0)])
        }
        "many_small_foci" => clustered_marks(
            width,
            height,
            &[
                (2.0, 2.0, 1.2),
                (9.0, 2.0, 1.2),
                (2.0, 9.0, 1.2),
                (9.0, 9.0, 1.2),
            ],
        ),
        "anisotropic_stripe" => stripe_marks(width, height),
        "low_k_suppressed_dispersed" => dispersed_marks(width, height),
        "stain_gradient_artifact" | "internal_control_dropout_artifact" => {
            clustered_marks(width, height, &[(3.5, 3.5, 2.0)])
        }
        "fragmented_tumor_islands" => permute_fixed_count(width * height, 16, seed)?,
        "rare_phenotype" => rare_marks(width, height),
        "serial_section_misregistration" => shifted_section_marks(width, height),
        _ => {
            return Err(MarklabError::Validation(format!(
                "unknown synthetic generator {generator}"
            )));
        }
    };

    if marks.iter().filter(|mark| **mark == 1).count() == 0 {
        marks[0] = 1;
    }

    let mut pattern = grid_pattern(generator, width, height, marks)?;
    match generator {
        "stain_gradient_artifact" => {
            pattern.local_dab_od = Some(
                (0..pattern.len())
                    .map(|index| index as f32 / pattern.len().max(1) as f32)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            );
        }
        "internal_control_dropout_artifact" => {
            pattern.window.valid_mask_fraction = 0.20;
        }
        "fragmented_tumor_islands" => {
            pattern.component_id = Some(
                (0..pattern.len())
                    .map(|index| index as u32)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            );
        }
        "cell_density_gradient_random_labels" => {
            pattern.mark_prob = Some(
                pattern
                    .x_um
                    .iter()
                    .map(|x| (0.20 + 0.05 * *x as f32).min(0.90))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            );
        }
        _ => {}
    }
    Ok(pattern)
}

fn grid_pattern(name: &str, width: usize, height: usize, marks: Vec<u8>) -> Result<Pattern> {
    let mut x = Vec::with_capacity(width * height);
    let mut y = Vec::with_capacity(width * height);
    for row in 0..height {
        for col in 0..width {
            x.push(col as f64);
            y.push(row as f64);
        }
    }

    let mut pattern = Pattern::from_arrays(
        x,
        y,
        marks,
        PatternMeta {
            case_id: format!("synthetic_{name}"),
            timepoint: "post".into(),
            protein: "MSH6".into(),
            slide_id: None,
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )?;
    pattern.window.area_um2 = (width * height) as f64;
    pattern.window.l_eff_um = width.max(height) as f64;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.valid_mask_fraction = 1.0;
    Ok(pattern)
}

fn clustered_marks(width: usize, height: usize, centers: &[(f64, f64, f64)]) -> Vec<u8> {
    let mut marks = Vec::with_capacity(width * height);
    for row in 0..height {
        for col in 0..width {
            let marked = centers.iter().any(|(cx, cy, radius)| {
                let dx = col as f64 - *cx;
                let dy = row as f64 - *cy;
                dx * dx + dy * dy <= radius * radius
            });
            marks.push(u8::from(marked));
        }
    }
    marks
}

fn stripe_marks(width: usize, height: usize) -> Vec<u8> {
    let mut marks = Vec::with_capacity(width * height);
    let start = width / 4;
    let end = (start + (width / 6).max(2)).min(width);
    for _row in 0..height {
        for col in 0..width {
            marks.push(u8::from((start..end).contains(&col)));
        }
    }
    marks
}

fn dispersed_marks(width: usize, height: usize) -> Vec<u8> {
    let mut marks = Vec::with_capacity(width * height);
    for row in 0..height {
        for col in 0..width {
            marks.push(u8::from((row + col) % 4 == 0));
        }
    }
    marks
}

fn rare_marks(width: usize, height: usize) -> Vec<u8> {
    let mut marks = vec![0; width * height];
    if !marks.is_empty() {
        marks[width.min(width * height - 1)] = 1;
    }
    marks
}

fn shifted_section_marks(width: usize, height: usize) -> Vec<u8> {
    let mut marks = Vec::with_capacity(width * height);
    let row_start = height / 4;
    let row_end = (row_start + height / 3).min(height);
    let col_start = width / 3;
    let col_end = (col_start + width / 3).min(width);
    for row in 0..height {
        for col in 0..width {
            marks.push(u8::from(
                (row_start..row_end).contains(&row) && (col_start..col_end).contains(&col),
            ));
        }
    }
    marks
}
