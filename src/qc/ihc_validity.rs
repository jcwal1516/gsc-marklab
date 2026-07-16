pub fn validity_fraction(valid_cells: usize, total_cells: usize) -> f64 {
    if total_cells == 0 {
        0.0
    } else {
        valid_cells as f64 / total_cells as f64
    }
}
