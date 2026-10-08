use super::common::*;
use crate::{
    common::seeds::{derive_seed, SeedEndpoint},
    geom::spatial_index::SpatialIndex2D,
    permutation::stratified::StratifiedPermutationPlan,
    ObservationWindow2D, Result,
};
use geo::{Coord, LineString, Polygon};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScanDesign {
    pub positive_phenotype: String,
    pub eligible_phenotypes: Vec<String>,
    pub radii_um: Vec<f64>,
    pub permutations: usize,
    pub seed: u64,
    pub alpha: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScanCell {
    pub id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub positive: bool,
    pub stratum: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct StratumEnrichment {
    pub stratum: String,
    pub inside_cases: u64,
    pub inside_total: u64,
    pub outside_cases: u64,
    pub outside_total: u64,
    pub inside_rate: Option<f64>,
    pub outside_rate: Option<f64>,
    pub relative_enrichment: Option<f64>,
    pub unavailable: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ScanWinner {
    pub center_cell_id: String,
    pub center_um: [f64; 2],
    pub radius_um: f64,
    pub score: f64,
    pub inside_cases: u64,
    pub inside_total: u64,
    pub outside_cases: u64,
    pub outside_total: u64,
    pub strata: Vec<StratumEnrichment>,
    pub geometry: Value,
    pub display_maximum_chord_error_um: f64,
}
#[derive(Clone, Debug, Serialize)]
pub struct ScanSlide {
    pub slide_id: String,
    pub patient_id: String,
    pub coordinate_frame_id: String,
    pub window: Value,
    pub bounds_um: [f64; 4],
    pub cells: Vec<ScanCell>,
    pub candidate_count: usize,
    pub winner: Option<ScanWinner>,
    pub p_global: Option<f64>,
    pub slide_family_p: Option<f64>,
    pub exceedances: usize,
    pub permutations_completed: usize,
    pub unavailable: Option<String>,
}
/// One-marker stratified Bernoulli scan with complete-search and across-slide correction.
#[derive(Clone, Debug, Serialize)]
pub struct PathologyScanResult {
    pub format: &'static str,
    pub version: u32,
    pub study_id: String,
    pub phenotypes: Phenotypes,
    pub design: ScanDesign,
    pub limits: Limits,
    pub slides: Vec<ScanSlide>,
    pub work_units: usize,
    pub estimated_storage_bytes: usize,
    pub claim_scope: &'static str,
}
struct Candidate {
    center: usize,
    radius: f64,
    members: Vec<usize>,
    inside_by_stratum: Vec<u64>,
}

/// Search fixed cell-centered disks using one-sided enrichment scores and stratified null maxima.
pub fn analyze_pathology_scan(bytes: &[u8]) -> Result<PathologyScanResult> {
    let (recipe, windows, mut budget) =
        parse::<ScanDesign>(bytes, "marklab.pathology_scan_recipe")?;
    let d = &recipe.design;
    if !recipe
        .study
        .phenotypes
        .names
        .contains(&d.positive_phenotype)
        || !d.eligible_phenotypes.contains(&d.positive_phenotype)
        || d.eligible_phenotypes
            .iter()
            .any(|n| !recipe.study.phenotypes.names.contains(n))
        || d.eligible_phenotypes.iter().collect::<BTreeSet<_>>().len()
            != d.eligible_phenotypes.len()
        || d.radii_um.is_empty()
        || d.radii_um.len() > 64
        || d.radii_um
            .iter()
            .any(|r| !r.is_finite() || *r <= 0.0 || !(r * r).is_finite())
        || d.radii_um.windows(2).any(|r| r[0] >= r[1])
        || !(1..=100_000).contains(&d.permutations)
        || !(0.0 < d.alpha && d.alpha < 0.5)
        || (d.permutations + 1) as f64 * d.alpha < 1.0
    {
        return Err(invalid(
            "invalid scan phenotype, eligible population, radii or inference design",
        ));
    }
    let mut slides = Vec::new();
    for (i, (slide, window)) in recipe.study.slides.iter().zip(&windows).enumerate() {
        let mut result =
            analyze_slide(slide, window, d, d.seed.wrapping_add(i as u64), &mut budget)?;
        result.slide_family_p = result
            .p_global
            .map(|p| (p * recipe.study.slides.len() as f64).min(1.0));
        slides.push(result);
    }
    Ok(PathologyScanResult{format:"marklab.pathology_scan_result",version:1,study_id:recipe.study.study_id,
        phenotypes:recipe.study.phenotypes,design:recipe.design,limits:recipe.limits,slides,work_units:budget.work,estimated_storage_bytes:budget.bytes,
        claim_scope:"experimental_within_slide_binary_enrichment; complete_disk_search_then_bonferroni_slides; no_population_or_lesion_boundary_claim"})
}

fn analyze_slide(
    slide: &Slide,
    window: &ObservationWindow2D,
    d: &ScanDesign,
    seed: u64,
    budget: &mut Budget,
) -> Result<ScanSlide> {
    let cells = slide
        .cells
        .iter()
        .filter(|c| d.eligible_phenotypes.contains(&c.phenotype))
        .map(|c| ScanCell {
            id: c.id.clone(),
            x_um: c.x_um,
            y_um: c.y_um,
            positive: c.phenotype == d.positive_phenotype,
            stratum: c.stratum.clone(),
        })
        .collect::<Vec<_>>();
    let mut result = ScanSlide {
        slide_id: slide.slide_id.clone(),
        patient_id: slide.patient_id.clone(),
        coordinate_frame_id: slide.coordinate_frame_id.clone(),
        window: slide.window.clone(),
        bounds_um: window.bounds_um(),
        cells,
        candidate_count: 0,
        winner: None,
        p_global: None,
        slide_family_p: None,
        exceedances: 0,
        permutations_completed: 0,
        unavailable: None,
    };
    let cells = &result.cells;
    if cells.len() < 4 {
        result.unavailable = Some("fewer than four eligible cells".into());
        return Ok(result);
    }
    let names = cells
        .iter()
        .map(|c| c.stratum.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if names.len() > 1024 {
        return Err(invalid("scan stratum count exceeds 1024"));
    }
    let by_name = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n, i))
        .collect::<BTreeMap<_, _>>();
    let strata = cells
        .iter()
        .map(|c| by_name[&c.stratum])
        .collect::<Vec<_>>();
    let labels = cells
        .iter()
        .map(|c| u8::from(c.positive))
        .collect::<Vec<_>>();
    let mut totals = vec![0u64; names.len()];
    let mut cases = totals.clone();
    for (&stratum, &label) in strata.iter().zip(&labels) {
        totals[stratum] += 1;
        cases[stratum] += u64::from(label);
    }
    if !totals.iter().zip(&cases).any(|(n, c)| *c > 0 && c < n) {
        result.unavailable = Some("no exchangeable binary variation within declared strata".into());
        return Ok(result);
    }
    budget.retain(cells.len().saturating_mul(512))?;
    let index = SpatialIndex2D::new(
        &cells.iter().map(|c| c.x_um).collect::<Vec<_>>(),
        &cells.iter().map(|c| c.y_um).collect::<Vec<_>>(),
    )?;
    // A conservative query ceiling covers all visited index candidates, including rejected disks.
    budget.work(
        cells
            .len()
            .checked_mul(cells.len())
            .and_then(|n| n.checked_mul(d.radii_um.len()))
            .ok_or_else(|| invalid("scan query work overflow"))?,
    )?;
    let mut candidates = Vec::new();
    let mut evaluation_work = 0usize;
    for (center, cell) in cells.iter().enumerate() {
        for radius in &d.radii_um {
            let neighbors = index.points_within_radius(cell.x_um, cell.y_um, *radius)?;
            if neighbors.len() < 2
                || neighbors.len() > cells.len() / 2
                || cells.len() - neighbors.len() < 2
            {
                continue;
            }
            let mut members = neighbors.iter().map(|n| n.index).collect::<Vec<_>>();
            members.sort_unstable();
            budget.tiles(1)?;
            budget.retain((members.len() + names.len()).saturating_mul(8))?;
            evaluation_work = evaluation_work
                .checked_add(members.len() + names.len())
                .ok_or_else(|| invalid("scan work overflow"))?;
            let mut inside_by_stratum = vec![0u64; names.len()];
            for member in &members {
                inside_by_stratum[strata[*member]] += 1;
            }
            candidates.push(Candidate {
                center,
                radius: *radius,
                members,
                inside_by_stratum,
            });
        }
    }
    result.candidate_count = candidates.len();
    if candidates.is_empty() {
        result.unavailable =
            Some("no disk satisfies two inside/outside and at most half the population".into());
        return Ok(result);
    }
    budget.work(
        evaluation_work
            .checked_mul(d.permutations + 1)
            .and_then(|w| w.checked_add(cells.len().saturating_mul(d.permutations)))
            .ok_or_else(|| invalid("scan permutation work overflow"))?,
    )?;
    let mut scratch = vec![0u64; names.len()];
    let (winner, observed) = maximum(&candidates, &labels, &strata, &totals, &cases, &mut scratch)?;
    let plan = StratifiedPermutationPlan::new(
        &labels,
        &strata.iter().map(|i| *i as u64).collect::<Vec<_>>(),
    )?;
    let mut randomized = Vec::with_capacity(cells.len());
    let mut stratum_labels = Vec::with_capacity(plan.maximum_stratum_size());
    for replicate in 0..d.permutations {
        plan.permute_into(
            derive_seed(seed, SeedEndpoint::PathologyBernoulliScan, replicate),
            &mut randomized,
            &mut stratum_labels,
        )?;
        let (_, score) = maximum(
            &candidates,
            &randomized,
            &strata,
            &totals,
            &cases,
            &mut scratch,
        )?;
        result.exceedances += usize::from(score >= observed);
    }
    result.permutations_completed = d.permutations;
    result.p_global = Some((result.exceedances + 1) as f64 / (d.permutations + 1) as f64);
    let candidate = &candidates[winner];
    let center = &cells[candidate.center];
    scratch.fill(0);
    for member in &candidate.members {
        scratch[strata[*member]] += u64::from(labels[*member]);
    }
    let strata_result=names.iter().enumerate().map(|(i,name)| {
        let inside=candidate.inside_by_stratum[i];let outside=totals[i]-inside;
        let inside_cases=scratch[i];let outside_cases=cases[i]-inside_cases;
        let rate=|n:u64,d:u64|if d==0 {None}else{Some(n as f64/d as f64)};
        let inside_rate=rate(inside_cases,inside);let outside_rate=rate(outside_cases,outside);
        let relative_enrichment=inside_rate.zip(outside_rate).and_then(|(a,b)|(b>0.0).then(||a/b));
        StratumEnrichment{stratum:name.clone(),inside_cases,inside_total:inside,outside_cases,outside_total:outside,inside_rate,outside_rate,relative_enrichment,
            unavailable:relative_enrichment.is_none().then(||"relative enrichment requires inside/outside support and positive outside rate".into())}
    }).collect();
    const SEGMENTS: usize = 128;
    budget.geometry(window, SEGMENTS)?;
    let mut ring = (0..SEGMENTS)
        .map(|i| {
            let angle = std::f64::consts::TAU * i as f64 / SEGMENTS as f64;
            Coord {
                x: center.x_um + candidate.radius * angle.cos(),
                y: center.y_um + candidate.radius * angle.sin(),
            }
        })
        .collect::<Vec<_>>();
    ring.push(ring[0]);
    let (_, geometry) = window
        .clip_study_polygon(
            &Polygon::new(LineString(ring), vec![]),
            budget.limits.maximum_geometry_vertices,
        )
        .map_err(|e| invalid(e.to_string()))?;
    budget.retain(
        geometry
            .0
            .iter()
            .flat_map(|p| std::iter::once(p.exterior()).chain(p.interiors()))
            .map(|r| r.0.len())
            .sum::<usize>()
            .saturating_mul(256),
    )?;
    let inside_cases = scratch.iter().sum::<u64>();
    let total_cases = cases.iter().sum::<u64>();
    result.winner = Some(ScanWinner {
        center_cell_id: center.id.clone(),
        center_um: [center.x_um, center.y_um],
        radius_um: candidate.radius,
        score: observed,
        inside_cases,
        inside_total: candidate.members.len() as u64,
        outside_cases: total_cases - inside_cases,
        outside_total: (cells.len() - candidate.members.len()) as u64,
        strata: strata_result,
        geometry: polygon_json(&geometry),
        display_maximum_chord_error_um: candidate.radius
            * (1.0 - (std::f64::consts::PI / SEGMENTS as f64).cos()),
    });
    Ok(result)
}

fn maximum(
    candidates: &[Candidate],
    labels: &[u8],
    strata: &[usize],
    totals: &[u64],
    cases: &[u64],
    scratch: &mut [u64],
) -> Result<(usize, f64)> {
    let mut best = 0.0;
    let mut winner = 0;
    for (i, candidate) in candidates.iter().enumerate() {
        scratch.fill(0);
        for member in &candidate.members {
            scratch[strata[*member]] += u64::from(labels[*member]);
        }
        let score = finite(
            totals
                .iter()
                .enumerate()
                .map(|(s, n)| llr(scratch[s], candidate.inside_by_stratum[s], cases[s], *n))
                .sum(),
        )?;
        if score > best {
            best = score;
            winner = i;
        }
    }
    Ok((winner, best))
}

fn llr(inside_cases: u64, inside: u64, cases: u64, total: u64) -> f64 {
    if inside == 0
        || inside == total
        || cases == 0
        || cases == total
        || inside_cases * total <= cases * inside
    {
        return 0.0;
    }
    let outside = total - inside;
    let outside_cases = cases - inside_cases;
    let prevalence = cases as f64 / total as f64;
    let term = |observed: u64, expected: f64| {
        if observed == 0 {
            0.0
        } else {
            observed as f64 * ((observed as f64 - expected) / expected).ln_1p()
        }
    };
    (term(inside_cases, inside as f64 * prevalence)
        + term(inside - inside_cases, inside as f64 * (1.0 - prevalence))
        + term(outside_cases, outside as f64 * prevalence)
        + term(outside - outside_cases, outside as f64 * (1.0 - prevalence)))
    .max(0.0)
}
