pub fn mean_nearest_neighbor_distance(x: &[f64], y: &[f64]) -> Option<f64> {
    if x.len() != y.len() || x.len() < 2 {
        return None;
    }

    let mut total = 0.0;
    for i in 0..x.len() {
        let mut best = f64::INFINITY;
        for j in 0..x.len() {
            if i == j {
                continue;
            }
            let dx = x[i] - x[j];
            let dy = y[i] - y[j];
            best = best.min((dx * dx + dy * dy).sqrt());
        }
        total += best;
    }

    Some(total / x.len() as f64)
}
