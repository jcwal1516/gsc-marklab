//! Normal tail probabilities to near machine precision, for SPACE's normal-approximation P values.

/// Complementary error function by W. J. Cody's rational Chebyshev approximations
/// (Math. Comp. 23, 1969, 631–637; SPECFUN `CALERF`). `exp(-x^2)` is split as in that routine
/// so the large-argument tail keeps its relative accuracy.
pub(crate) fn erfc(x: f64) -> f64 {
    const A: [f64; 5] = [
        3.1611237438705655,
        113.86415415105016,
        377.485237685302,
        3209.3775891384694,
        0.18577770618460315,
    ];
    const B: [f64; 4] = [
        23.601290952344122,
        244.02463793444417,
        1282.6165260773723,
        2844.236833439171,
    ];
    const C: [f64; 9] = [
        0.5641884969886701,
        8.883149794388377,
        66.11919063714163,
        298.6351381974001,
        881.952221241769,
        1712.0476126340707,
        2051.0783778260716,
        1230.3393547979972,
        2.1531153547440383e-8,
    ];
    const D: [f64; 8] = [
        15.744926110709835,
        117.6939508913125,
        537.1811018620099,
        1621.3895745666903,
        3290.7992357334597,
        4362.619090143247,
        3439.3676741437216,
        1230.3393548037493,
    ];
    const P: [f64; 6] = [
        0.30532663496123236,
        0.36034489994980445,
        0.12578172611122926,
        0.01608378514874228,
        0.0006587491615298378,
        0.016315387137302097,
    ];
    const Q: [f64; 5] = [
        2.568520192289822,
        1.872_952_849_923_467_3,
        0.5279051029514285,
        0.06051834131244132,
        0.0023352049762686918,
    ];
    const ONE_OVER_SQRT_PI: f64 = 0.5641895835477563;
    let y = x.abs();
    if y <= 0.46875 {
        let square = if y > f64::EPSILON / 2.0 { y * y } else { 0.0 };
        let mut numerator = A[4] * square;
        let mut denominator = square;
        for index in 0..3 {
            numerator = (numerator + A[index]) * square;
            denominator = (denominator + B[index]) * square;
        }
        return 1.0 - x * (numerator + A[3]) / (denominator + B[3]);
    }
    let mut result = if y <= 4.0 {
        let mut numerator = C[8] * y;
        let mut denominator = y;
        for index in 0..7 {
            numerator = (numerator + C[index]) * y;
            denominator = (denominator + D[index]) * y;
        }
        (numerator + C[7]) / (denominator + D[7])
    } else {
        let inverse_square = 1.0 / (y * y);
        let mut numerator = P[5] * inverse_square;
        let mut denominator = inverse_square;
        for index in 0..4 {
            numerator = (numerator + P[index]) * inverse_square;
            denominator = (denominator + Q[index]) * inverse_square;
        }
        let correction = inverse_square * (numerator + P[4]) / (denominator + Q[4]);
        (ONE_OVER_SQRT_PI - correction) / y
    };
    let rounded = (y * 16.0).trunc() / 16.0;
    let remainder = (y - rounded) * (y + rounded);
    result *= (-rounded * rounded).exp() * (-remainder).exp();
    if x < 0.0 {
        2.0 - result
    } else {
        result
    }
}

/// `2 * Phi(-|z|)`, R's `2 * pnorm(-abs(z))`.
pub(crate) fn two_sided_p(z: f64) -> f64 {
    erfc(z.abs() / std::f64::consts::SQRT_2)
}

/// `1 - Phi(z)`, R's `pnorm(z, lower.tail = FALSE)`.
pub(crate) fn upper_tail(z: f64) -> f64 {
    0.5 * erfc(z / std::f64::consts::SQRT_2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_sided_p_matches_r_pnorm() {
        // R 4.5.2: sprintf("%.17g", 2 * pnorm(-z)).
        let cases = [
            (0.70850847160235142, 0.47862955319152395),
            (0.5, 0.61707507745197387),
            (1.0, 0.31731050786291409),
            (2.0, 0.045500263896358424),
            (3.0, 0.0026997960632601892),
            (5.0, 5.7330314375838782e-07),
            (8.0, 1.244192114854357e-15),
            (10.0, 1.5239706048321054e-23),
            (15.0, 7.3419323986255028e-51),
            (20.0, 5.5072482372124675e-89),
            (25.0, 6.1133934127651232e-138),
            (30.0, 9.8134278542963744e-198),
            (37.0, 1.1451142445049154e-299),
        ];
        // Rounding z / sqrt(2) costs about 2 x^2 epsilon relative error in the far tail; on a
        // dense 0..38 grid the largest relative error against R was 1.9e-13 (z = 37).
        for (z, expected) in cases {
            let observed = two_sided_p(z);
            assert!(
                (observed - expected).abs() <= 1e-12 * expected,
                "z = {z}: {observed:e} versus {expected:e}"
            );
            assert_eq!(two_sided_p(-z), observed);
        }
        assert_eq!(two_sided_p(0.0), 1.0);
    }

    #[test]
    fn upper_tail_matches_r_pnorm_upper() {
        // R 4.5.2: sprintf("%.17g", pnorm(z, lower.tail = FALSE)).
        let cases = [
            (-3.0, 0.9986501019683699),
            (-0.5, 0.69146246127401301),
            (0.0, 0.5),
            (0.5, 0.30853753872598694),
            (1.0, 0.15865525393145705),
            (2.5, 0.0062096653257761349),
            (5.0, 2.8665157187919391e-07),
            (10.0, 7.6198530241605269e-24),
            (30.0, 4.9067139271481872e-198),
        ];
        for (z, expected) in cases {
            let observed = upper_tail(z);
            assert!(
                (observed - expected).abs() <= 1e-12 * expected,
                "z = {z}: {observed:e} versus {expected:e}"
            );
        }
    }
}
