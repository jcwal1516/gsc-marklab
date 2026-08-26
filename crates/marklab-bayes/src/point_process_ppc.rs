use crate::RectangularWindow;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PpcPoint {
    pub pattern_id: String,
    pub point_id: String,
    pub x_um: f64,
    pub y_um: f64,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PpcReplicatedPoint {
    pub replicate: u32,
    pub pattern_id: String,
    pub point_id: String,
    pub x_um: f64,
    pub y_um: f64,
}
#[derive(Clone, Debug)]
pub struct PointProcessPpcSpec {
    pub observed: Vec<PpcPoint>,
    pub replicated: Vec<PpcReplicatedPoint>,
    pub radii_um: Vec<f64>,
    pub window: RectangularWindow,
    pub alpha: f64,
    pub maximum_pair_visits: u64,
}
#[derive(Debug, thiserror::Error)]
pub enum PointProcessPpcError {
    #[error("invalid point-process PPC input: {0}")]
    Invalid(String),
    #[error("point-process PPC resource limit exceeded: {0}")]
    Resource(String),
    #[error("point-process PPC numerical failure: {0}")]
    Numerical(String),
}
#[derive(Debug, Serialize)]
pub struct PointProcessPpcCurveRow {
    pub radius_um: f64,
    pub observed_mean_k_um2: f64,
    pub replicated_mean_k_um2: f64,
    pub replicated_sd_k_um2: f64,
    pub simultaneous_critical_value: f64,
    pub lower: f64,
    pub upper: f64,
    pub observed_exceeds_envelope: bool,
}
#[derive(Debug, Serialize)]
pub struct PointProcessPpcResult {
    pub pattern_count: u32,
    pub replicate_count: u32,
    pub observed_total_count: u64,
    pub replicated_total_count_mean: f64,
    pub replicated_total_count_sd: f64,
    pub pair_visits: u64,
    pub alpha: f64,
    pub curve: Vec<PointProcessPpcCurveRow>,
    pub exceeded_radii_um: Vec<f64>,
}
pub fn posterior_predictive_point_process_diagnostics(
    s: PointProcessPpcSpec,
) -> Result<PointProcessPpcResult, PointProcessPpcError> {
    if ![
        s.window.xmin_um,
        s.window.ymin_um,
        s.window.xmax_um,
        s.window.ymax_um,
        s.alpha,
    ]
    .into_iter()
    .all(f64::is_finite)
        || s.window.xmin_um >= s.window.xmax_um
        || s.window.ymin_um >= s.window.ymax_um
        || !(0.0..0.5).contains(&s.alpha)
        || !(2..=256).contains(&s.radii_um.len())
        || !(1..=100_000_000).contains(&s.maximum_pair_visits)
    {
        return Err(PointProcessPpcError::Invalid(
            "window, alpha, radii, or resources are invalid".into(),
        ));
    }
    for (i, r) in s.radii_um.iter().enumerate() {
        if !r.is_finite()
            || *r <= 0.0
            || (i > 0 && *r <= s.radii_um[i - 1])
            || *r > (s.window.xmax_um - s.window.xmin_um).hypot(s.window.ymax_um - s.window.ymin_um)
        {
            return Err(PointProcessPpcError::Invalid(
                "radii must be increasing, positive, and within window diagonal".into(),
            ));
        }
    }
    let observed = group_observed(&s.observed, &s.window)?;
    if !(1..=100).contains(&observed.len()) {
        return Err(PointProcessPpcError::Invalid(
            "requires 1-100 observed patterns".into(),
        ));
    }
    let replicated = group_replicated(&s.replicated, &s.window)?;
    let reps = replicated.len();
    if !(20..=1000).contains(&reps) || replicated.keys().copied().ne(0..reps as u32) {
        return Err(PointProcessPpcError::Invalid(
            "replicate indices must be complete 0..S with S 20-1000".into(),
        ));
    }
    for patterns in replicated.values() {
        if patterns.keys().ne(observed.keys()) {
            return Err(PointProcessPpcError::Invalid(
                "every replicate must contain every exact pattern ID".into(),
            ));
        }
    }
    let mut visits = 0_u64;
    let observed_curve = aggregate(
        &observed,
        &s.radii_um,
        &s.window,
        &mut visits,
        s.maximum_pair_visits,
    )?;
    let observed_total = observed.values().map(|p| p.len() as u64).sum();
    let mut curves = Vec::with_capacity(reps);
    let mut totals = Vec::with_capacity(reps);
    for patterns in replicated.values() {
        curves.push(aggregate(
            patterns,
            &s.radii_um,
            &s.window,
            &mut visits,
            s.maximum_pair_visits,
        )?);
        totals.push(patterns.values().map(|p| p.len() as f64).sum::<f64>());
    }
    let means = (0..s.radii_um.len())
        .map(|j| curves.iter().map(|c| c[j]).sum::<f64>() / reps as f64)
        .collect::<Vec<_>>();
    let sds = (0..s.radii_um.len())
        .map(|j| sd(&curves.iter().map(|c| c[j]).collect::<Vec<_>>()))
        .collect::<Vec<_>>();
    let mut maxdev = curves
        .iter()
        .map(|c| {
            (0..c.len())
                .map(|j| {
                    if sds[j] > 0.0 {
                        ((c[j] - means[j]) / sds[j]).abs()
                    } else {
                        0.0
                    }
                })
                .fold(0.0, f64::max)
        })
        .collect::<Vec<_>>();
    maxdev.sort_by(f64::total_cmp);
    let rank = ((1.0 - s.alpha) * reps as f64).ceil() as usize;
    let critical = maxdev[rank.saturating_sub(1).min(reps - 1)];
    let mut exceeded = Vec::new();
    let curve = s
        .radii_um
        .iter()
        .enumerate()
        .map(|(j, r)| {
            let lower = means[j] - critical * sds[j];
            let upper = means[j] + critical * sds[j];
            let ex = observed_curve[j] < lower || observed_curve[j] > upper;
            if ex {
                exceeded.push(*r);
            }
            PointProcessPpcCurveRow {
                radius_um: *r,
                observed_mean_k_um2: observed_curve[j],
                replicated_mean_k_um2: means[j],
                replicated_sd_k_um2: sds[j],
                simultaneous_critical_value: critical,
                lower,
                upper,
                observed_exceeds_envelope: ex,
            }
        })
        .collect();
    Ok(PointProcessPpcResult {
        pattern_count: observed.len() as u32,
        replicate_count: reps as u32,
        observed_total_count: observed_total,
        replicated_total_count_mean: totals.iter().sum::<f64>() / reps as f64,
        replicated_total_count_sd: sd(&totals),
        pair_visits: visits,
        alpha: s.alpha,
        curve,
        exceeded_radii_um: exceeded,
    })
}
type Pattern = BTreeMap<String, Vec<(f64, f64)>>;
fn group_observed(
    rows: &[PpcPoint],
    w: &RectangularWindow,
) -> Result<Pattern, PointProcessPpcError> {
    let mut out = Pattern::new();
    let mut ids = HashSet::new();
    for p in rows {
        validate_point(&p.pattern_id, &p.point_id, p.x_um, p.y_um, w)?;
        if !ids.insert((p.pattern_id.as_str(), p.point_id.as_str())) {
            return Err(PointProcessPpcError::Invalid(
                "duplicate observed point identity".into(),
            ));
        }
        out.entry(p.pattern_id.clone())
            .or_default()
            .push((p.x_um, p.y_um));
    }
    if out.values().any(|p| p.len() < 2) {
        return Err(PointProcessPpcError::Invalid(
            "each observed pattern needs at least two points".into(),
        ));
    }
    Ok(out)
}
fn group_replicated(
    rows: &[PpcReplicatedPoint],
    w: &RectangularWindow,
) -> Result<BTreeMap<u32, Pattern>, PointProcessPpcError> {
    let mut out = BTreeMap::new();
    let mut ids = HashSet::new();
    for p in rows {
        validate_point(&p.pattern_id, &p.point_id, p.x_um, p.y_um, w)?;
        if !ids.insert((p.replicate, p.pattern_id.as_str(), p.point_id.as_str())) {
            return Err(PointProcessPpcError::Invalid(
                "duplicate replicated point identity".into(),
            ));
        }
        out.entry(p.replicate)
            .or_insert_with(Pattern::new)
            .entry(p.pattern_id.clone())
            .or_default()
            .push((p.x_um, p.y_um));
    }
    if out.values().flat_map(|p| p.values()).any(|p| p.len() < 2) {
        return Err(PointProcessPpcError::Invalid(
            "each replicated pattern needs at least two points".into(),
        ));
    }
    Ok(out)
}
fn validate_point(
    pattern: &str,
    id: &str,
    x: f64,
    y: f64,
    w: &RectangularWindow,
) -> Result<(), PointProcessPpcError> {
    if pattern.is_empty()
        || id.is_empty()
        || ![x, y].into_iter().all(f64::is_finite)
        || !(w.xmin_um..w.xmax_um).contains(&x)
        || !(w.ymin_um..w.ymax_um).contains(&y)
    {
        return Err(PointProcessPpcError::Invalid(
            "point identity or window membership is invalid".into(),
        ));
    }
    Ok(())
}
fn aggregate(
    patterns: &Pattern,
    radii: &[f64],
    w: &RectangularWindow,
    visits: &mut u64,
    cap: u64,
) -> Result<Vec<f64>, PointProcessPpcError> {
    let mut sum = vec![0.0; radii.len()];
    for points in patterns.values() {
        let k = k_curve(points, radii, w, visits, cap)?;
        for (j, v) in k.into_iter().enumerate() {
            sum[j] += v;
        }
    }
    for v in &mut sum {
        *v /= patterns.len() as f64;
    }
    Ok(sum)
}
fn k_curve(
    points: &[(f64, f64)],
    radii: &[f64],
    w: &RectangularWindow,
    visits: &mut u64,
    cap: u64,
) -> Result<Vec<f64>, PointProcessPpcError> {
    let n = points.len();
    let area = (w.xmax_um - w.xmin_um) * (w.ymax_um - w.ymin_um);
    let mut sums = vec![0.0; radii.len()];
    for i in 0..n {
        for j in i + 1..n {
            *visits = visits
                .checked_add(1)
                .ok_or_else(|| PointProcessPpcError::Resource("pair visits overflow".into()))?;
            if *visits > cap {
                return Err(PointProcessPpcError::Resource(
                    "pair visits exceed cap".into(),
                ));
            }
            let dx = (points[i].0 - points[j].0).abs();
            let dy = (points[i].1 - points[j].1).abs();
            let overlap = (w.xmax_um - w.xmin_um - dx) * (w.ymax_um - w.ymin_um - dy);
            let d = dx.hypot(dy);
            if overlap <= 0.0 || !overlap.is_finite() {
                return Err(PointProcessPpcError::Numerical(
                    "translation overlap is invalid".into(),
                ));
            }
            for (k, r) in radii.iter().enumerate() {
                if d <= *r {
                    sums[k] += 2.0 * area * area / (n as f64 * (n - 1) as f64 * overlap);
                }
            }
        }
    }
    Ok(sums)
}
fn sd(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return 0.0;
    }
    let m = v.iter().sum::<f64>() / v.len() as f64;
    (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (v.len() - 1) as f64).sqrt()
}
