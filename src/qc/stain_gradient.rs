use crate::common::stats::mean_all_finite;

pub fn gradient_suspect(values: &[f32]) -> bool {
    if values.len() < 8 {
        return false;
    }
    let finite = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if finite.len() < 8 {
        return false;
    }

    let first_half = &finite[..finite.len() / 2];
    let second_half = &finite[finite.len() / 2..];
    let first_mean = mean_all_finite(first_half.iter().copied().map(f64::from))
        .expect("the finite first half is non-empty");
    let second_mean = mean_all_finite(second_half.iter().copied().map(f64::from))
        .expect("the finite second half is non-empty");
    let range = finite.iter().copied().fold(f32::NEG_INFINITY, f32::max)
        - finite.iter().copied().fold(f32::INFINITY, f32::min);

    range > 0.25 && (second_mean - first_mean).abs() > 0.20_f64
}
