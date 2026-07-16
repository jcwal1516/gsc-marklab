pub fn territory_radius_from_scale(scale_um: f64) -> f64 {
    scale_um * 2.0_f64.sqrt()
}
