use crate::{
    data::{Pattern, PatternMeta},
    errors::{MarklabError, Result},
    permutation::labels::permute_fixed_count,
};

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
