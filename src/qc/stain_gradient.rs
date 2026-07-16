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
    let first_mean = mean(first_half);
    let second_mean = mean(second_half);
    let range = finite.iter().copied().fold(f32::NEG_INFINITY, f32::max)
        - finite.iter().copied().fold(f32::INFINITY, f32::min);

    range > 0.25 && (second_mean - first_mean).abs() > 0.20
}

fn mean(values: &[f32]) -> f32 {
    values.iter().sum::<f32>() / values.len() as f32
}
