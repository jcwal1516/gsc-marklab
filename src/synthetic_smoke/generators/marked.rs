#[cfg(test)]
use super::DeterministicRng;
#[cfg(test)]
use crate::common::seeds::splitmix64;
use crate::{
    data::{Pattern, PatternMeta},
    errors::{MarklabError, Result},
    permutation::labels::permute_fixed_count,
};

#[cfg(test)]
/// Cell layouts for the scheduled shuffle calibration, at 20 um spacing so that every layout
/// meets the shipped validation minimums of 200 cells and 100,000 square micrometres.
/// Clustered positions are redrawn for every replicate.
#[derive(Clone, Copy, Debug)]
pub(in crate::synthetic_smoke) enum CalibrationLayout {
    /// 16 x 16 square lattice, 256 cells.
    Square,
    /// 64 x 4 lattice, 256 cells: long and thin, with little interior.
    Strip,
    /// Points of a 21 x 21 lattice 4 to 10 spacings from its centre: a ring with a hole,
    /// 272 cells.
    Ring,
    /// Sixteen uniform clusters of sixteen cells inside a 16 x 16 square: uneven density.
    ClusteredPositions,
    /// 40 x 40 square lattice, 1,600 cells.
    LargeSquare,
}

#[cfg(test)]
const CALIBRATION_SPACING_UM: f64 = 20.0;

#[cfg(test)]
impl CalibrationLayout {
    pub(in crate::synthetic_smoke) const ALL: [Self; 5] = [
        Self::Square,
        Self::Strip,
        Self::Ring,
        Self::ClusteredPositions,
        Self::LargeSquare,
    ];

    pub(in crate::synthetic_smoke) const fn name(self) -> &'static str {
        match self {
            Self::Square => "square_16x16",
            Self::Strip => "strip_64x4",
            Self::Ring => "ring_21x21",
            Self::ClusteredPositions => "clustered_positions_256",
            Self::LargeSquare => "square_40x40",
        }
    }

    /// Cell coordinates, window area and bounding-box diagonal, in units of cell spacing.
    fn cells(self, rng: &mut DeterministicRng) -> (Vec<f64>, Vec<f64>, f64, f64) {
        match self {
            Self::Square => lattice(16, 16, |_, _| true),
            Self::Strip => lattice(64, 4, |_, _| true),
            Self::Ring => lattice(21, 21, |x, y| {
                (4.0..=10.0).contains(&(x - 10.0).hypot(y - 10.0))
            }),
            Self::LargeSquare => lattice(40, 40, |_, _| true),
            Self::ClusteredPositions => {
                let mut x = Vec::with_capacity(256);
                let mut y = Vec::with_capacity(256);
                for _ in 0..16 {
                    let parent = (1.0 + 14.0 * rng.unit(), 1.0 + 14.0 * rng.unit());
                    for _ in 0..16 {
                        x.push((parent.0 + rng.centered(1.2)).clamp(0.0, 16.0));
                        y.push((parent.1 + rng.centered(1.2)).clamp(0.0, 16.0));
                    }
                }
                (x, y, 256.0, 16_f64.hypot(16.0))
            }
        }
    }
}

fn lattice(
    width: usize,
    height: usize,
    keep: impl Fn(f64, f64) -> bool,
) -> (Vec<f64>, Vec<f64>, f64, f64) {
    let mut x = Vec::with_capacity(width * height);
    let mut y = Vec::with_capacity(width * height);
    for row in 0..height {
        for col in 0..width {
            let (cx, cy) = (col as f64, row as f64);
            if keep(cx, cy) {
                x.push(cx);
                y.push(cy);
            }
        }
    }
    let area_um2 = x.len() as f64;
    (x, y, area_um2, (width as f64).hypot(height as f64))
}

/// How a calibration pattern places its fixed number of marks.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::synthetic_smoke) enum CalibrationLabels {
    /// A uniformly random subset: the random-labelling null.
    Uniform,
    /// One Gaussian focus (sd two cell spacings) around a random cell. At 25% marked it is one
    /// clone holding a quarter of the cells, comparable to or larger than the primary
    /// endpoint's largest interpretable scale (0.33 of the window diagonal).
    SingleFocus,
    /// Foci of about nine cells (sd one cell spacing) around randomly chosen cells, inside
    /// the largest interpretable scale of every calibration layout.
    SmallFoci,
}

#[cfg(test)]
impl CalibrationLabels {
    pub(in crate::synthetic_smoke) const fn name(self) -> &'static str {
        match self {
            Self::Uniform => "uniform_fixed_count",
            Self::SingleFocus => "single_focus_gaussian_sd_2_cells",
            Self::SmallFoci => "small_foci_9_cells_gaussian_sd_1_cell",
        }
    }
}

/// A calibration pattern with a fixed marked count placed according to `labels`.
#[cfg(test)]
pub(in crate::synthetic_smoke) fn calibration_pattern(
    layout: CalibrationLayout,
    marked_percent: usize,
    labels: CalibrationLabels,
    scenario: u64,
    replicate: u64,
) -> Result<Pattern> {
    let mut rng = DeterministicRng::new(
        splitmix64(0x7368_7566_666c_6531 ^ scenario.wrapping_mul(0x9e37_79b9_7f4a_7c15))
            ^ replicate.wrapping_mul(0xbf58_476d_1ce4_e5b9),
    );
    let (x, y, area, diagonal) = layout.cells(&mut rng);
    let marked = x.len() * marked_percent / 100;
    let marks = match labels {
        CalibrationLabels::Uniform => permute_fixed_count(x.len(), marked, rng.next_u64())?,
        CalibrationLabels::SingleFocus => {
            let centre = ((rng.unit() * x.len() as f64) as usize).min(x.len() - 1);
            focused_marks(&x, &y, marked, &[centre], 2.0, &mut rng)
        }
        CalibrationLabels::SmallFoci => {
            let centres = permute_fixed_count(x.len(), marked.div_ceil(9), rng.next_u64())?
                .into_iter()
                .enumerate()
                .filter_map(|(index, mark)| (mark == 1).then_some(index))
                .collect::<Vec<_>>();
            focused_marks(&x, &y, marked, &centres, 1.0, &mut rng)
        }
    };
    let x = x
        .iter()
        .map(|value| value * CALIBRATION_SPACING_UM)
        .collect::<Vec<_>>();
    let y = y
        .iter()
        .map(|value| value * CALIBRATION_SPACING_UM)
        .collect::<Vec<_>>();
    let d_nn_mean_um = mean_nearest_neighbour_distance(&x, &y);
    positioned_pattern(
        layout.name(),
        x,
        y,
        marks,
        area * CALIBRATION_SPACING_UM * CALIBRATION_SPACING_UM,
        diagonal * CALIBRATION_SPACING_UM,
        d_nn_mean_um,
    )
}

/// Draws `count` marks with weight given by a Gaussian (standard deviation `sd`) of the
/// distance to the nearest centre.
#[cfg(test)]
fn focused_marks(
    x: &[f64],
    y: &[f64],
    count: usize,
    centres: &[usize],
    sd: f64,
    rng: &mut DeterministicRng,
) -> Vec<u8> {
    // Weighted sampling without replacement (Efraimidis-Spirakis): keep the `count` largest
    // keys u^(1/w), compared on the log scale.
    let mut keys = x
        .iter()
        .zip(y)
        .enumerate()
        .map(|(index, (&cx, &cy))| {
            let squared = centres
                .iter()
                .map(|&centre| (cx - x[centre]).powi(2) + (cy - y[centre]).powi(2))
                .fold(f64::INFINITY, f64::min);
            let weight = (-squared / (2.0 * sd * sd)).exp();
            (rng.unit().max(f64::MIN_POSITIVE).ln() / weight, index)
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| right.0.total_cmp(&left.0));
    let mut marks = vec![0; x.len()];
    for &(_, index) in keys.iter().take(count) {
        marks[index] = 1;
    }
    marks
}

#[cfg(test)]
fn mean_nearest_neighbour_distance(x: &[f64], y: &[f64]) -> f64 {
    let total = (0..x.len())
        .map(|i| {
            (0..x.len())
                .filter(|&j| j != i)
                .map(|j| (x[i] - x[j]).hypot(y[i] - y[j]))
                .fold(f64::INFINITY, f64::min)
        })
        .sum::<f64>();
    total / x.len() as f64
}

pub(in crate::synthetic_smoke) fn synthetic_pattern(
    generator: &str,
    replicate: u64,
) -> Result<Pattern> {
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
        "prepost_metadata_mismatch" => clustered_marks(width, height, &[(5.5, 5.5, 3.0)]),
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
            pattern.internal_control_valid_fraction = Some(0.20);
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
    let (x, y, area_um2, diagonal_um) = lattice(width, height, |_, _| true);
    positioned_pattern(name, x, y, marks, area_um2, diagonal_um, 1.0)
}

fn positioned_pattern(
    name: &str,
    x: Vec<f64>,
    y: Vec<f64>,
    marks: Vec<u8>,
    area_um2: f64,
    diagonal_um: f64,
    d_nn_mean_um: f64,
) -> Result<Pattern> {
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
    pattern.window.area_um2 = area_um2;
    pattern.window.analysis_effective_length_um = diagonal_um;
    pattern.window.d_nn_mean_um = d_nn_mean_um;
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
