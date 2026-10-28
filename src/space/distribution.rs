//! R `smooth_dist`'s regularization of a joint distribution, shared by transMI and the
//! distribution view.

/// Turn `frequencies`, observed counts per grid cell with `NaN` for cells removed as
/// impossible compositions (R's `NA`), into probabilities. When every kept cell was observed
/// they are plain proportions; otherwise the proportions are scaled by the Chao–Jost estimate
/// of sample coverage and the missing mass is spread evenly over the unobserved cells.
/// `counts` are the observed combination counts (R `plyr::count`), from which singletons and
/// doubletons are taken, over `discs` discs.
pub(super) fn chao_jost(frequencies: &mut [f64], counts: &[u32], discs: usize) {
    // R counts singletons and doubletons over the observed combinations; `num_doub` sums the
    // doubletons' frequencies.
    let singletons = counts.iter().filter(|count| **count == 1).count() as f64;
    let doubleton_frequency = 2.0 * counts.iter().filter(|count| **count == 2).count() as f64;
    let observations = discs as f64 + 1.0;
    let singletons = if singletons == 0.0 { 0.5 } else { singletons };
    let mut missing = singletons / observations;
    let f1 = (observations - 1.0) * singletons;
    let f2 = doubleton_frequency * 2.0;
    if f1 != 0.0 || f2 != 0.0 {
        missing *= f1 / (f1 + f2);
    }

    let mut kept = 0.0;
    let mut zeros = 0_usize;
    for &frequency in frequencies.iter().filter(|frequency| !frequency.is_nan()) {
        kept += frequency;
        zeros += usize::from(frequency == 0.0);
    }
    if zeros == 0 {
        for slot in frequencies.iter_mut() {
            *slot /= kept;
        }
        return;
    }
    for slot in frequencies.iter_mut() {
        *slot = if *slot == 0.0 {
            missing / zeros as f64
        } else {
            (*slot / kept) * (1.0 - missing)
        };
    }
}
