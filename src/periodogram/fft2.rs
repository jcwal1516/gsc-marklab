use num_complex::Complex32;
use rustfft::FftPlanner;

pub fn fft2_power_spectrum(field: &[f32], width: usize, height: usize) -> Option<Vec<f64>> {
    if width == 0 || height == 0 || field.len() != width.checked_mul(height)? {
        return None;
    }

    let mut buffer = field
        .iter()
        .copied()
        .map(|value| Complex32::new(value, 0.0))
        .collect::<Vec<_>>();
    let mut planner = FftPlanner::<f32>::new();
    let row_fft = planner.plan_fft_forward(width);
    for row in buffer.chunks_exact_mut(width) {
        row_fft.process(row);
    }

    let col_fft = planner.plan_fft_forward(height);
    let mut column = vec![Complex32::new(0.0, 0.0); height];
    for x in 0..width {
        for y in 0..height {
            column[y] = buffer[y * width + x];
        }
        col_fft.process(&mut column);
        for y in 0..height {
            buffer[y * width + x] = column[y];
        }
    }

    Some(
        buffer
            .into_iter()
            .map(|value| {
                let re = f64::from(value.re);
                let im = f64::from(value.im);
                re * re + im * im
            })
            .collect(),
    )
}
