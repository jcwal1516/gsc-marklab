pub(crate) fn standardized_mcse_difference(difference: f64, mcse: f64) -> f64 {
    if mcse > 0.0 {
        difference / mcse
    } else if difference == 0.0 {
        0.0
    } else {
        f64::INFINITY
    }
}
