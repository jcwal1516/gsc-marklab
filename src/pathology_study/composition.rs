use std::collections::{BTreeMap, BTreeSet};

use geo::{Coord, Rect};
use marklab_cohort::{
    hierarchical_bootstrap, max_t_multiple_endpoint_permutation, HierarchicalBootstrapSpec,
    HierarchicalScalarRecord, MaxTPermutationSpec, PatientEndpointVector,
};
use serde::{Deserialize, Serialize};

use super::common::*;
use crate::{geom::window::StudyTile, ObservationWindow2D, ObservationWindowError, Result};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionDesign {
    pub group_a: String,
    pub group_b: String,
    pub permutations: usize,
    pub bootstrap_replicates: usize,
    pub seed: u64,
    pub alpha: f64,
    pub balances: Vec<Balance>,
    pub maup: Maup,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Balance {
    pub name: String,
    pub kind: BalanceKind,
    pub numerator: Vec<String>,
    pub denominator: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BalanceKind {
    GeometricBalance,
    AmalgamatedLogRatio,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Maup {
    pub phenotype: String,
    pub origin_um: [f64; 2],
    pub primary: String,
    pub grids: Vec<Grid>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Grid {
    pub id: String,
    pub width_um: f64,
    pub offset_um: [f64; 2],
}

#[derive(Clone, Debug, Serialize)]
pub struct BalanceValue {
    pub name: String,
    pub value: Option<f64>,
    pub unavailable: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Tile {
    pub column: i64,
    pub row: i64,
    pub area_mm2: f64,
    pub phenotype_count: u64,
    pub total_cells: u64,
    pub geometry: serde_json::Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct GridResult {
    pub id: String,
    pub width_um: f64,
    pub offset_um: [f64; 2],
    pub area_mm2: f64,
    pub assigned_cells: u64,
    pub tiles: Vec<Tile>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CompositionSlide {
    pub slide_id: String,
    pub patient_id: String,
    pub group: String,
    pub coordinate_frame_id: String,
    pub area_mm2: f64,
    pub counts: Vec<u64>,
    pub total_cells: u64,
    pub densities_per_mm2: Vec<f64>,
    pub fractions: Vec<Option<f64>>,
    pub balances: Vec<BalanceValue>,
    pub grids: Vec<GridResult>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CompositionPatient {
    pub patient_id: String,
    pub group: String,
    pub area_mm2: f64,
    pub counts: Vec<u64>,
    pub densities_per_mm2: Vec<f64>,
    pub fractions: Vec<Option<f64>>,
    pub balances: Vec<BalanceValue>,
    pub density_variance: Vec<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EndpointContrast {
    pub endpoint: String,
    pub effect_group_a_minus_group_b: Option<f64>,
    pub pointwise_interval: Option<[f64; 2]>,
    pub adjusted_p_value: Option<f64>,
    pub unavailable: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CompositionInference {
    pub correction: &'static str,
    pub available_family: Vec<String>,
    pub unavailable: Option<String>,
    pub endpoints: Vec<EndpointContrast>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MaupSensitivity {
    pub primary: String,
    pub sign_reversals: Vec<String>,
    pub offset_effect_ranges: Vec<(f64, Option<[f64; 2]>)>,
}

/// Complete composition, exposure and partition-sensitivity result.
#[derive(Clone, Debug, Serialize)]
pub struct PathologyCompositionResult {
    pub format: &'static str,
    pub version: u32,
    pub study_id: String,
    pub phenotypes: Phenotypes,
    pub design: CompositionDesign,
    pub limits: Limits,
    pub slides: Vec<CompositionSlide>,
    pub patients: Vec<CompositionPatient>,
    pub inference: CompositionInference,
    pub sensitivity: MaupSensitivity,
    pub work_units: usize,
    pub estimated_storage_bytes: usize,
    pub claim_scope: &'static str,
}

/// Analyze hard-label composition and absolute density, with patient-level grid sensitivity.
pub fn analyze_pathology_composition(bytes: &[u8]) -> Result<PathologyCompositionResult> {
    let (recipe, windows, mut budget) =
        parse::<CompositionDesign>(bytes, "marklab.pathology_composition_recipe")?;
    validate(&recipe)?;
    let names = &recipe.study.phenotypes.names;
    let target = names
        .iter()
        .position(|n| n == &recipe.design.maup.phenotype)
        .expect("validated phenotype");
    let mut slides = Vec::new();
    for (slide, window) in recipe.study.slides.iter().zip(&windows) {
        let mut counts = vec![0u64; names.len()];
        for cell in &slide.cells {
            counts[names
                .iter()
                .position(|n| n == &cell.phenotype)
                .expect("admitted phenotype")] += 1;
        }
        let area_mm2 = finite(window.area_um2() / 1e6)?;
        let mut grids = Vec::new();
        for grid in &recipe.design.maup.grids {
            grids.push(tile(
                slide,
                window,
                grid,
                &recipe.design.maup.origin_um,
                &recipe.design.maup.phenotype,
                &mut budget,
            )?);
        }
        slides.push(CompositionSlide {
            slide_id: slide.slide_id.clone(),
            patient_id: slide.patient_id.clone(),
            group: slide.group.clone(),
            coordinate_frame_id: slide.coordinate_frame_id.clone(),
            area_mm2,
            densities_per_mm2: densities(&counts, area_mm2)?,
            fractions: fractions(&counts),
            balances: balances(&counts, names, &recipe.design.balances)?,
            counts,
            total_cells: slide.cells.len() as u64,
            grids,
        });
    }
    let mut grouped = BTreeMap::<&str, Vec<&CompositionSlide>>::new();
    for slide in &slides {
        grouped.entry(&slide.patient_id).or_default().push(slide);
    }
    let mut patients = Vec::new();
    for (id, patient_slides) in grouped {
        let area = finite(patient_slides.iter().map(|s| s.area_mm2).sum())?;
        let counts = (0..names.len())
            .map(|i| patient_slides.iter().map(|s| s.counts[i]).sum())
            .collect::<Vec<_>>();
        let density = densities(&counts, area)?;
        let variance = (0..recipe.design.maup.grids.len())
            .map(|i| {
                finite(
                    patient_slides
                        .iter()
                        .flat_map(|s| &s.grids[i].tiles)
                        .map(|t| {
                            t.area_mm2
                                * (t.phenotype_count as f64 / t.area_mm2 - density[target]).powi(2)
                        })
                        .sum::<f64>()
                        / area,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        patients.push(CompositionPatient {
            patient_id: id.into(),
            group: patient_slides[0].group.clone(),
            area_mm2: area,
            densities_per_mm2: density,
            fractions: fractions(&counts),
            balances: balances(&counts, names, &recipe.design.balances)?,
            counts,
            density_variance: variance,
        });
    }
    let inference = infer(&patients, names, &recipe.design, &mut budget)?;
    let effects = recipe
        .design
        .maup
        .grids
        .iter()
        .map(|g| {
            inference
                .endpoints
                .iter()
                .find(|e| e.endpoint == format!("maup:{}", g.id))
                .and_then(|e| e.effect_group_a_minus_group_b)
        })
        .collect::<Vec<_>>();
    let primary_index = recipe
        .design
        .maup
        .grids
        .iter()
        .position(|g| g.id == recipe.design.maup.primary)
        .expect("primary");
    let sign_reversals = recipe
        .design
        .maup
        .grids
        .iter()
        .zip(&effects)
        .filter_map(|(g, e)| match (effects[primary_index], e) {
            (Some(a), Some(b)) if a * b < 0.0 => Some(g.id.clone()),
            _ => None,
        })
        .collect();
    let mut widths = recipe
        .design
        .maup
        .grids
        .iter()
        .map(|g| g.width_um)
        .collect::<Vec<_>>();
    widths.sort_by(f64::total_cmp);
    widths.dedup();
    let offset_effect_ranges = widths
        .into_iter()
        .map(|width| {
            let selected = recipe
                .design
                .maup
                .grids
                .iter()
                .zip(&effects)
                .filter(|(g, _)| g.width_um == width)
                .filter_map(|(_, e)| *e)
                .collect::<Vec<_>>();
            (
                width,
                if selected.is_empty() {
                    None
                } else {
                    Some([
                        selected.iter().copied().fold(f64::INFINITY, f64::min),
                        selected.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                    ])
                },
            )
        })
        .collect();
    let sensitivity = MaupSensitivity {
        primary: recipe.design.maup.primary.clone(),
        sign_reversals,
        offset_effect_ranges,
    };
    Ok(PathologyCompositionResult {format:"marklab.pathology_composition_result",version:1,study_id:recipe.study.study_id,
        phenotypes:recipe.study.phenotypes,design:recipe.design,limits:recipe.limits,slides,patients,inference,sensitivity,
        work_units:budget.work,estimated_storage_bytes:budget.bytes,
        claim_scope:"experimental_patient_composition_density_and_grid_sensitivity; pointwise_intervals_not_simultaneous"})
}

fn validate(recipe: &Recipe<CompositionDesign>) -> Result<()> {
    let d = &recipe.design;
    inference_design(&d.group_a, &d.group_b, d.permutations, d.alpha)?;
    if !(20..=100_000).contains(&d.bootstrap_replicates)
        || d.balances.len() > 64
        || recipe
            .study
            .slides
            .iter()
            .any(|s| s.group != d.group_a && s.group != d.group_b)
    {
        return Err(invalid(
            "invalid bootstrap count, balances or undeclared patient group",
        ));
    }
    let mut ids = BTreeSet::new();
    for b in &d.balances {
        let both = b.numerator.iter().chain(&b.denominator).collect::<Vec<_>>();
        if !text(&b.name)
            || !ids.insert(&b.name)
            || b.numerator.is_empty()
            || b.denominator.is_empty()
            || both
                .iter()
                .any(|n| !recipe.study.phenotypes.names.contains(n))
            || both.iter().collect::<BTreeSet<_>>().len() != both.len()
        {
            return Err(invalid(
                "balance groups must be nonempty, disjoint declared phenotypes with unique names",
            ));
        }
    }
    let m = &d.maup;
    if !recipe.study.phenotypes.names.contains(&m.phenotype)
        || m.grids.is_empty()
        || m.grids.len() > 64
        || m.origin_um.iter().any(|v| !v.is_finite())
        || !m.grids.iter().any(|g| g.id == m.primary)
    {
        return Err(invalid(
            "invalid MAUP phenotype, origin, grids or primary specification",
        ));
    }
    let mut ids = BTreeSet::new();
    for g in &m.grids {
        if !text(&g.id)
            || !ids.insert(&g.id)
            || !g.width_um.is_finite()
            || g.width_um <= 0.0
            || g.offset_um
                .iter()
                .any(|v| !v.is_finite() || *v < 0.0 || *v >= g.width_um)
        {
            return Err(invalid("invalid grid identity, width or offset"));
        }
    }
    Ok(())
}

fn densities(counts: &[u64], area: f64) -> Result<Vec<f64>> {
    counts.iter().map(|n| finite(*n as f64 / area)).collect()
}
fn fractions(counts: &[u64]) -> Vec<Option<f64>> {
    let total = counts.iter().sum::<u64>();
    counts
        .iter()
        .map(|n| {
            if total == 0 {
                None
            } else {
                Some(*n as f64 / total as f64)
            }
        })
        .collect()
}
fn balances(
    counts: &[u64],
    names: &[String],
    definitions: &[Balance],
) -> Result<Vec<BalanceValue>> {
    definitions
        .iter()
        .map(|b| {
            let values = |group: &[String]| {
                group
                    .iter()
                    .map(|n| {
                        counts[names
                            .iter()
                            .position(|name| name == n)
                            .expect("validated balance")] as f64
                    })
                    .collect::<Vec<_>>()
            };
            let a = values(&b.numerator);
            let z = values(&b.denominator);
            let value = match b.kind {
                BalanceKind::GeometricBalance if a.iter().chain(&z).all(|n| *n > 0.0) => {
                    let coefficient =
                        (a.len() as f64 * z.len() as f64 / (a.len() + z.len()) as f64).sqrt();
                    Some(finite(
                        coefficient
                            * (a.iter().map(|v| v.ln()).sum::<f64>() / a.len() as f64
                                - z.iter().map(|v| v.ln()).sum::<f64>() / z.len() as f64),
                    )?)
                }
                BalanceKind::AmalgamatedLogRatio
                    if a.iter().sum::<f64>() > 0.0 && z.iter().sum::<f64>() > 0.0 =>
                {
                    Some(finite(
                        a.iter().sum::<f64>().ln() - z.iter().sum::<f64>().ln(),
                    )?)
                }
                _ => None,
            };
            Ok(BalanceValue {
                name: b.name.clone(),
                value,
                unavailable: value
                    .is_none()
                    .then(|| "zero component or amalgamated mass; no pseudocount".into()),
            })
        })
        .collect()
}

fn tile(
    slide: &Slide,
    window: &ObservationWindow2D,
    grid: &Grid,
    origin: &[f64; 2],
    phenotype: &str,
    budget: &mut Budget,
) -> Result<GridResult> {
    let anchor = [
        finite(origin[0] + grid.offset_um[0])?,
        finite(origin[1] + grid.offset_um[1])?,
    ];
    let index = |v: f64, axis: usize| -> Result<i64> {
        let i = ((v - anchor[axis]) / grid.width_um).floor();
        if !i.is_finite() || i.abs() > 1_000_000_000.0 {
            return Err(invalid("grid coordinate index outside supported range"));
        }
        Ok(i as i64)
    };
    let bounds = window.bounds_um();
    let (x0, x1, y0, y1) = (
        index(bounds[0], 0)?,
        index(bounds[2], 0)?,
        index(bounds[1], 1)?,
        index(bounds[3], 1)?,
    );
    let tile_count = (x1 - x0 + 1)
        .checked_mul(y1 - y0 + 1)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| invalid("tile count overflow"))?;
    budget.tiles(tile_count)?;
    budget.work(slide.cells.len())?;
    let mut tiles = Vec::new();
    for row in y0..=y1 {
        for column in x0..=x1 {
            let x = finite(anchor[0] + column as f64 * grid.width_um)?;
            let y = finite(anchor[1] + row as f64 * grid.width_um)?;
            let upper = [finite(x + grid.width_um)?, finite(y + grid.width_um)?];
            if upper[0] <= x || upper[1] <= y {
                return Err(invalid("grid width below coordinate resolution"));
            }
            let tile = Rect::new(
                Coord { x, y },
                Coord {
                    x: upper[0],
                    y: upper[1],
                },
            );
            // One boundary-envelope query per tile; only tiles that a boundary
            // segment can cross pay for the exact polygon clip.
            budget.work(1)?;
            let (area, geometry) = match window.classify_study_tile(&tile) {
                StudyTile::Outside => continue,
                StudyTile::Inside { area, geometry } => (area, geometry),
                StudyTile::Boundary => {
                    budget.geometry(window, 4)?;
                    window
                        .clip_study_polygon(
                            &tile.to_polygon(),
                            budget.limits.maximum_geometry_vertices,
                        )
                        .map_err(|e| invalid(e.to_string()))?
                }
            };
            if area == 0.0 {
                continue;
            }
            let vertices = geometry
                .0
                .iter()
                .flat_map(|p| std::iter::once(p.exterior()).chain(p.interiors()))
                .map(|r| r.0.len())
                .sum::<usize>();
            if vertices > budget.limits.maximum_geometry_vertices {
                return Err(invalid(
                    ObservationWindowError::VertexLimitExceeded {
                        observed: vertices,
                        maximum: budget.limits.maximum_geometry_vertices,
                    }
                    .to_string(),
                ));
            }
            budget.retain(vertices.saturating_mul(256))?;
            tiles.push(Tile {
                column,
                row,
                area_mm2: finite(area / 1e6)?,
                phenotype_count: 0,
                total_cells: 0,
                geometry: polygon_json(&geometry),
            });
        }
    }
    let lookup = tiles
        .iter()
        .enumerate()
        .map(|(i, t)| ((t.column, t.row), i))
        .collect::<BTreeMap<_, _>>();
    for cell in &slide.cells {
        let column = index(cell.x_um, 0)?;
        let row = index(cell.y_um, 1)?;
        let on_x = cell.x_um == anchor[0] + column as f64 * grid.width_um;
        let on_y = cell.y_um == anchor[1] + row as f64 * grid.width_um;
        // Standard half-open membership first. A closed observation boundary can lie
        // in a zero-exposure tile; assign it once to an incident positive-area tile.
        let candidates = [
            Some((column, row)),
            on_x.then_some((column - 1, row)),
            on_y.then_some((column, row - 1)),
            (on_x && on_y).then_some((column - 1, row - 1)),
        ];
        let target = candidates
            .into_iter()
            .flatten()
            .find_map(|key| lookup.get(&key))
            .ok_or_else(|| invalid("no positive-area incident tile for observed boundary cell"))?;
        tiles[*target].total_cells += 1;
        tiles[*target].phenotype_count += u64::from(cell.phenotype == phenotype);
    }
    let area_mm2 = tiles.iter().map(|t| t.area_mm2).sum::<f64>();
    let assigned_cells = tiles.iter().map(|t| t.total_cells).sum::<u64>();
    if assigned_cells != slide.cells.len() as u64
        || (area_mm2 - window.area_um2() / 1e6).abs() > window.area_um2() / 1e6 * 1e-8
    {
        return Err(invalid("grid count or exposure conservation failed"));
    }
    Ok(GridResult {
        id: grid.id.clone(),
        width_um: grid.width_um,
        offset_um: grid.offset_um,
        area_mm2,
        assigned_cells,
        tiles,
    })
}

fn infer(
    patients: &[CompositionPatient],
    names: &[String],
    design: &CompositionDesign,
    budget: &mut Budget,
) -> Result<CompositionInference> {
    let endpoints = names
        .iter()
        .map(|n| format!("density:{n}"))
        .chain(
            design
                .balances
                .iter()
                .map(|b| format!("balance:{}", b.name)),
        )
        .chain(design.maup.grids.iter().map(|g| format!("maup:{}", g.id)))
        .collect::<Vec<_>>();
    let matrix = patients
        .iter()
        .map(|p| {
            p.densities_per_mm2
                .iter()
                .copied()
                .map(Some)
                .chain(p.balances.iter().map(|b| b.value))
                .chain(p.density_variance.iter().copied().map(Some))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    budget.work(
        patients
            .len()
            .checked_mul(endpoints.len())
            .and_then(|n| n.checked_mul(design.permutations + design.bootstrap_replicates + 1))
            .ok_or_else(|| invalid("cohort work overflow"))?,
    )?;
    budget.retain(
        design
            .bootstrap_replicates
            .saturating_mul(64)
            .saturating_add(
                patients
                    .len()
                    .saturating_mul(endpoints.len())
                    .saturating_mul(64),
            ),
    )?;
    let mut results = Vec::new();
    let mut available = Vec::new();
    for (i, name) in endpoints.iter().enumerate() {
        let mut result = EndpointContrast {
            endpoint: name.clone(),
            effect_group_a_minus_group_b: None,
            pointwise_interval: None,
            adjusted_p_value: None,
            unavailable: None,
        };
        if matrix.iter().any(|row| row[i].is_none()) {
            result.unavailable =
                Some("endpoint unavailable for at least one patient; no patients dropped".into());
            results.push(result);
            continue;
        }
        let values = matrix
            .iter()
            .map(|row| row[i].expect("complete endpoint"))
            .collect::<Vec<_>>();
        let groups = [&design.group_a, &design.group_b].map(|g| {
            patients
                .iter()
                .zip(&values)
                .filter(|(p, _)| &p.group == g)
                .map(|(p, v)| HierarchicalScalarRecord {
                    patient_id: p.patient_id.clone(),
                    specimen_id: p.patient_id.clone(),
                    endpoint: *v,
                })
                .collect::<Vec<_>>()
        });
        if groups.iter().any(|g| g.len() < 2) {
            result.unavailable =
                Some("at least two independent patients per group required".into());
            results.push(result);
            continue;
        }
        result.effect_group_a_minus_group_b = Some(finite(
            groups[0].iter().map(|p| p.endpoint).sum::<f64>() / groups[0].len() as f64
                - groups[1].iter().map(|p| p.endpoint).sum::<f64>() / groups[1].len() as f64,
        )?);
        let a = hierarchical_bootstrap(
            &groups[0],
            &HierarchicalBootstrapSpec {
                replicates: design.bootstrap_replicates,
                seed: design.seed ^ 0x434f4d5041,
                alpha: design.alpha,
            },
        )
        .map_err(|e| invalid(e.to_string()))?;
        let b = hierarchical_bootstrap(
            &groups[1],
            &HierarchicalBootstrapSpec {
                replicates: design.bootstrap_replicates,
                seed: design.seed ^ 0x434f4d5042,
                alpha: design.alpha,
            },
        )
        .map_err(|e| invalid(e.to_string()))?;
        let mut differences = a
            .bootstrap_means
            .iter()
            .zip(&b.bootstrap_means)
            .map(|(a, b)| finite(a - b))
            .collect::<Result<Vec<_>>>()?;
        differences.sort_by(f64::total_cmp);
        let quantile =
            |q: f64| differences[((q * differences.len() as f64).ceil() as usize).max(1) - 1];
        result.pointwise_interval = Some([
            quantile(design.alpha / 2.0),
            quantile(1.0 - design.alpha / 2.0),
        ]);
        if values.iter().all(|v| *v == values[0]) {
            result.unavailable =
                Some("constant endpoint across all patients; studentized test unavailable".into());
        } else {
            available.push(i);
        }
        results.push(result);
    }
    let family = available
        .iter()
        .map(|i| endpoints[*i].clone())
        .collect::<Vec<_>>();
    let mut unavailable = None;
    if !family.is_empty() {
        let rows = patients
            .iter()
            .zip(&matrix)
            .map(|(p, v)| PatientEndpointVector {
                patient_id: p.patient_id.clone(),
                group: p.group.clone(),
                endpoints: family.clone(),
                values: available.iter().map(|i| v[*i].expect("complete")).collect(),
            })
            .collect::<Vec<_>>();
        match max_t_multiple_endpoint_permutation(
            &rows,
            &MaxTPermutationSpec {
                group_a: design.group_a.clone(),
                group_b: design.group_b.clone(),
                permutations: design.permutations,
                seed: design.seed,
                alpha: design.alpha,
            },
        ) {
            Ok(result) => {
                for (i, e) in available.iter().zip(result.endpoints) {
                    results[*i].adjusted_p_value = Some(e.adjusted_p_value);
                }
            }
            Err(error) => {
                let message = error.to_string();
                for i in available {
                    results[i].unavailable = Some(message.clone());
                }
                unavailable = Some(message);
            }
        }
    }
    Ok(CompositionInference {
        correction: "single_step_max_t_whole_patient; pointwise_bootstrap_intervals",
        available_family: family,
        unavailable,
        endpoints: results,
    })
}
