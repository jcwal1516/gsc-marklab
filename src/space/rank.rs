//! Ensemble and variable rankings read from a cisMI or transMI result (R `plot_MI_rank` and
//! `plot_MI_radius`, without the plots).

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::cis::{SpaceCisResult, SPACE_CIS_FORMAT};
use super::input::read_bounded;
use super::trans::{SpaceTransResult, SPACE_TRANS_FORMAT};
use super::{invalid, SpaceError, SPACE_FORMAT_VERSION, SPACE_REFERENCE_COMMIT};

pub const SPACE_RANK_FORMAT: &str = "marklab.space_rank_result";

const MAXIMUM_RESULT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// A cisMI or transMI result to rank.
#[derive(Clone, Debug, PartialEq)]
pub enum SpaceMiResult {
    Cis(SpaceCisResult),
    Trans(SpaceTransResult),
}

/// Read a cisMI or transMI result file, recognized by its `format`.
pub fn load_space_mi_result(path: &Path) -> Result<SpaceMiResult, SpaceError> {
    space_mi_result_from_json(&read_bounded(
        path,
        MAXIMUM_RESULT_BYTES,
        "cisMI or transMI result",
    )?)
}

/// Decode a cisMI or transMI result, recognized by its `format`.
pub fn space_mi_result_from_json(bytes: &[u8]) -> Result<SpaceMiResult, SpaceError> {
    #[derive(Deserialize)]
    struct Header {
        format: String,
        version: u32,
    }
    let decode = |error: serde_json::Error| invalid(format!("invalid result JSON: {error}"));
    let header: Header = serde_json::from_slice(bytes).map_err(decode)?;
    if header.version != SPACE_FORMAT_VERSION {
        return Err(invalid(format!(
            "result version {} is not {SPACE_FORMAT_VERSION}",
            header.version
        )));
    }
    match header.format.as_str() {
        SPACE_CIS_FORMAT => serde_json::from_slice(bytes)
            .map(SpaceMiResult::Cis)
            .map_err(decode),
        SPACE_TRANS_FORMAT => serde_json::from_slice(bytes)
            .map(SpaceMiResult::Trans)
            .map_err(decode),
        other => Err(invalid(format!("{other} is not a cisMI or transMI result"))),
    }
}

/// The arguments of R `plot_MI_rank` and `plot_MI_radius` that select and order data.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceRankParameters {
    /// The radius whose ensembles are ranked; give it or `ensemble`.
    pub radius_um: Option<f64>,
    /// For a cisMI result, the specimen; may be omitted when the result has one.
    pub specimen: Option<String>,
    /// For a transMI result, the grouping factor; may be omitted when the result has one
    /// (R `group`).
    pub factor: Option<String>,
    /// Ensemble sizes to rank; empty for all (R `depth`).
    pub depths: Vec<usize>,
    /// Largest P value kept (R `p_thr`).
    pub p_threshold: f64,
    /// Smallest |cisMI| or transMI kept (R `mi_thr`); 0.1 for cisMI and 0 for transMI when
    /// absent, as in R.
    pub mi_threshold: Option<f64>,
    /// Filter on the Benjamini–Hochberg P value, otherwise on the normal-approximation P
    /// (R `p_adj`).
    pub p_adjust: bool,
    /// Variables every kept ensemble must contain.
    pub all: Vec<String>,
    /// Variables of which every kept ensemble must contain at least one.
    pub alo: Vec<String>,
    /// Variables no kept ensemble may contain.
    pub not: Vec<String>,
    /// Report this ensemble at every radius instead (R `plot_MI_radius`).
    pub ensemble: Vec<String>,
}

impl Default for SpaceRankParameters {
    fn default() -> Self {
        Self {
            radius_um: None,
            specimen: None,
            factor: None,
            depths: Vec::new(),
            p_threshold: 0.05,
            mi_threshold: None,
            p_adjust: true,
            all: Vec::new(),
            alo: Vec::new(),
            not: Vec::new(),
            ensemble: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceRankedEnsemble {
    pub variables: Vec<String>,
    /// cisMI or transMI.
    pub mi: f64,
    /// Absent where a cisMI null was degenerate and the observed value differed from it;
    /// R's Z is then infinite and such ensembles rank first.
    pub z_score: Option<f64>,
    pub p_value_normal: f64,
    pub p_adjust_bh: f64,
    pub p_value_monte_carlo: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceRankedVariable {
    pub name: String,
    /// Sum of |Z| over the kept ensembles that contain the variable, divided by the number
    /// of kept ensembles (R's aggregate Z score).
    pub score: f64,
    /// Kept ensembles containing the variable without a Z score. R's score is then
    /// infinite, so these variables rank first.
    pub unbounded_ensembles: usize,
    /// Kept ensembles containing the variable.
    pub ensembles: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceRadiusStatistic {
    pub radius_um: f64,
    pub mi: f64,
    pub z_score: Option<f64>,
    pub p_value_normal: f64,
    pub p_adjust_bh: f64,
    pub p_value_monte_carlo: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpaceRankView {
    /// Kept ensembles by decreasing |Z|, then variables by decreasing score
    /// (R `plot_MI_rank`).
    Ensembles {
        radius_um: f64,
        mi_threshold: f64,
        ensembles: Vec<SpaceRankedEnsemble>,
        variables: Vec<SpaceRankedVariable>,
    },
    /// One ensemble at every radius of the result (R `plot_MI_radius`).
    Radii {
        variables: Vec<String>,
        radii: Vec<SpaceRadiusStatistic>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceRankResult {
    pub format: String,
    pub version: u32,
    pub reference_commit: String,
    /// Format of the ranked result.
    pub source_format: String,
    pub parameters: SpaceRankParameters,
    pub specimen_id: Option<String>,
    pub factor: Option<String>,
    pub view: SpaceRankView,
}

/// One ensemble's statistics, from either kind of result.
struct Row<'a> {
    variables: &'a [String],
    statistic: SpaceRadiusStatistic,
}

/// What ranking needs from a cisMI specimen or a transMI factor.
struct Selection<'a> {
    cis: bool,
    depth: usize,
    variables: &'a [String],
    radii: Vec<(f64, Vec<Row<'a>>)>,
    specimen_id: Option<String>,
    factor: Option<String>,
}

/// Rank the ensembles of one radius, or follow one ensemble across radii.
pub fn space_rank(
    result: &SpaceMiResult,
    parameters: &SpaceRankParameters,
) -> Result<SpaceRankResult, SpaceError> {
    let selection = select(result, parameters)?;
    for (_, rows) in &selection.radii {
        for row in rows {
            check_row(row)?;
        }
    }
    let known = |names: &[String]| {
        names
            .iter()
            .find(|name| !selection.variables.contains(name))
            .map_or(Ok(()), |name| {
                Err(invalid(format!("unknown variable {name}")))
            })
    };
    for names in [
        &parameters.all,
        &parameters.alo,
        &parameters.not,
        &parameters.ensemble,
    ] {
        known(names)?;
    }
    let view = if parameters.ensemble.is_empty() {
        let radius_um = parameters
            .radius_um
            .ok_or_else(|| invalid("give the radius to rank, or an ensemble"))?;
        rank_radius(&selection, parameters, radius_um)?
    } else {
        if parameters.radius_um.is_some() {
            return Err(invalid(
                "an ensemble is followed across every radius; omit radius_um",
            ));
        }
        follow_ensemble(&selection, &parameters.ensemble)?
    };
    Ok(SpaceRankResult {
        format: SPACE_RANK_FORMAT.into(),
        version: SPACE_FORMAT_VERSION,
        reference_commit: SPACE_REFERENCE_COMMIT.into(),
        source_format: if selection.cis {
            SPACE_CIS_FORMAT
        } else {
            SPACE_TRANS_FORMAT
        }
        .into(),
        parameters: parameters.clone(),
        specimen_id: selection.specimen_id,
        factor: selection.factor,
        view,
    })
}

fn select<'a>(
    result: &'a SpaceMiResult,
    parameters: &SpaceRankParameters,
) -> Result<Selection<'a>, SpaceError> {
    match result {
        SpaceMiResult::Cis(result) => {
            if parameters.factor.is_some() {
                return Err(invalid("a cisMI result has no grouping factors"));
            }
            let specimen = match &parameters.specimen {
                Some(id) => result
                    .specimens
                    .iter()
                    .find(|specimen| &specimen.specimen_id == id)
                    .ok_or_else(|| invalid(format!("unknown specimen {id}")))?,
                None => match result.specimens.as_slice() {
                    [only] => only,
                    _ => return Err(invalid("name the specimen; the result has several")),
                },
            };
            Ok(Selection {
                cis: true,
                depth: result.parameters.depth,
                variables: &specimen.variables,
                radii: specimen
                    .radii
                    .iter()
                    .map(|radius| {
                        let rows = radius
                            .ensembles
                            .iter()
                            .map(|ensemble| Row {
                                variables: &ensemble.variables,
                                statistic: SpaceRadiusStatistic {
                                    radius_um: radius.radius_um,
                                    mi: ensemble.cis_mi,
                                    z_score: ensemble.z_score,
                                    p_value_normal: ensemble.p_value_normal,
                                    p_adjust_bh: ensemble.p_adjust_bh,
                                    p_value_monte_carlo: ensemble.p_value_monte_carlo,
                                },
                            })
                            .collect();
                        (radius.radius_um, rows)
                    })
                    .collect(),
                specimen_id: Some(specimen.specimen_id.clone()),
                factor: None,
            })
        }
        SpaceMiResult::Trans(result) => {
            if parameters.specimen.is_some() {
                return Err(invalid(
                    "a transMI result compares specimens; omit specimen",
                ));
            }
            let position = match &parameters.factor {
                Some(name) => result
                    .factors
                    .iter()
                    .position(|factor| &factor.factor == name)
                    .ok_or_else(|| invalid(format!("unknown grouping factor {name}")))?,
                None if result.factors.len() == 1 => 0,
                None => {
                    return Err(invalid(
                        "name the grouping factor; the result tests several",
                    ))
                }
            };
            let radii = result
                .radii
                .iter()
                .map(|radius| {
                    let rows = radius
                        .ensembles
                        .iter()
                        .map(|ensemble| {
                            let statistic = ensemble.factors.get(position).ok_or_else(|| {
                                invalid("an ensemble lacks a statistic for every factor")
                            })?;
                            Ok(Row {
                                variables: &ensemble.variables,
                                statistic: SpaceRadiusStatistic {
                                    radius_um: radius.radius_um,
                                    mi: statistic.trans_mi,
                                    z_score: Some(statistic.z_score),
                                    p_value_normal: statistic.p_value_normal,
                                    p_adjust_bh: statistic.p_adjust_bh,
                                    p_value_monte_carlo: statistic.p_value_monte_carlo,
                                },
                            })
                        })
                        .collect::<Result<Vec<_>, SpaceError>>()?;
                    Ok((radius.radius_um, rows))
                })
                .collect::<Result<Vec<_>, SpaceError>>()?;
            Ok(Selection {
                cis: false,
                depth: result.parameters.depth,
                variables: &result.analysed_variables,
                radii,
                specimen_id: None,
                factor: Some(result.factors[position].factor.clone()),
            })
        }
    }
}

/// Results are read back from files, so their values are checked before they are ordered.
fn check_row(row: &Row) -> Result<(), SpaceError> {
    let statistic = &row.statistic;
    let probability = |value: f64| (0.0..=1.0).contains(&value);
    if row.variables.is_empty()
        || !statistic.mi.is_finite()
        || statistic.z_score.is_some_and(|z| !z.is_finite())
        || !probability(statistic.p_value_normal)
        || !probability(statistic.p_adjust_bh)
        || !probability(statistic.p_value_monte_carlo)
    {
        return Err(invalid(format!(
            "the result's ensemble {} has an empty variable list, a non-finite statistic or a P value outside [0, 1]",
            row.variables.join(", ")
        )));
    }
    Ok(())
}

fn rank_radius(
    selection: &Selection,
    parameters: &SpaceRankParameters,
    radius_um: f64,
) -> Result<SpaceRankView, SpaceError> {
    let rows = selection
        .radii
        .iter()
        .find(|(known, _)| *known == radius_um)
        .map(|(_, rows)| rows)
        .ok_or_else(|| invalid(format!("the result has no radius {radius_um} um")))?;
    if let Some(depth) = parameters
        .depths
        .iter()
        .find(|depth| !(1..=selection.depth).contains(*depth))
    {
        return Err(invalid(format!(
            "depth {depth} is outside the result's 1 to {}",
            selection.depth
        )));
    }
    let smallest_depth = parameters.depths.iter().copied().min().unwrap_or(1);
    if parameters.all.len() + usize::from(!parameters.alo.is_empty()) > smallest_depth {
        return Err(invalid(
            "the number of required variables exceeds the smallest ensemble depth",
        ));
    }
    if !(0.0..=1.0).contains(&parameters.p_threshold) {
        return Err(invalid("p_threshold must be between 0 and 1"));
    }
    let mi_threshold = parameters
        .mi_threshold
        .unwrap_or(if selection.cis { 0.1 } else { 0.0 });
    if !mi_threshold.is_finite() {
        return Err(invalid("mi_threshold must be finite"));
    }

    let contains = |row: &Row, name: &String| row.variables.contains(name);
    let mut kept = rows
        .iter()
        .filter(|row| {
            (parameters.depths.is_empty() || parameters.depths.contains(&row.variables.len()))
                && parameters.all.iter().all(|name| contains(row, name))
                && (parameters.alo.is_empty()
                    || parameters.alo.iter().any(|name| contains(row, name)))
                && !parameters.not.iter().any(|name| contains(row, name))
        })
        .filter(|row| {
            let statistic = &row.statistic;
            let p = if parameters.p_adjust {
                statistic.p_adjust_bh
            } else {
                statistic.p_value_normal
            };
            let large = if selection.cis {
                statistic.mi.abs() >= mi_threshold
            } else {
                statistic.mi >= mi_threshold
            };
            p <= parameters.p_threshold && large
        })
        .collect::<Vec<_>>();
    // R sorts by decreasing |Z|, keeping ties in order; an infinite Z comes first.
    let magnitude = |row: &Row| row.statistic.z_score.map_or(f64::INFINITY, f64::abs);
    kept.sort_by(|a, b| magnitude(b).total_cmp(&magnitude(a)));

    // R lists variables as unlist() reads the sorted table: column by column.
    let mut names = Vec::<&String>::new();
    let mut seen = BTreeSet::new();
    for position in 0..selection.depth {
        for row in &kept {
            if let Some(name) = row.variables.get(position) {
                if seen.insert(name) {
                    names.push(name);
                }
            }
        }
    }
    let mut variables = names
        .into_iter()
        .map(|name| {
            let containing = kept.iter().filter(|row| contains(row, name));
            let (mut sum, mut unbounded, mut ensembles) = (0.0, 0, 0);
            for row in containing {
                ensembles += 1;
                match row.statistic.z_score {
                    Some(z) => sum += z.abs(),
                    None => unbounded += 1,
                }
            }
            SpaceRankedVariable {
                name: name.clone(),
                score: sum / kept.len() as f64,
                unbounded_ensembles: unbounded,
                ensembles,
            }
        })
        .collect::<Vec<_>>();
    variables.sort_by(
        |a, b| match (a.unbounded_ensembles > 0, b.unbounded_ensembles > 0) {
            (true, true) => std::cmp::Ordering::Equal,
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            (false, false) => b.score.total_cmp(&a.score),
        },
    );
    Ok(SpaceRankView::Ensembles {
        radius_um,
        mi_threshold,
        ensembles: kept
            .iter()
            .map(|row| SpaceRankedEnsemble {
                variables: row.variables.to_vec(),
                mi: row.statistic.mi,
                z_score: row.statistic.z_score,
                p_value_normal: row.statistic.p_value_normal,
                p_adjust_bh: row.statistic.p_adjust_bh,
                p_value_monte_carlo: row.statistic.p_value_monte_carlo,
            })
            .collect(),
        variables,
    })
}

fn follow_ensemble(
    selection: &Selection,
    ensemble: &[String],
) -> Result<SpaceRankView, SpaceError> {
    let wanted = ensemble.iter().collect::<BTreeSet<_>>();
    if wanted.len() != ensemble.len() {
        return Err(invalid("the ensemble repeats a variable"));
    }
    let radii = selection
        .radii
        .iter()
        .map(|(radius_um, rows)| {
            rows.iter()
                .find(|row| row.variables.iter().collect::<BTreeSet<_>>() == wanted)
                .map(|row| row.statistic.clone())
                .ok_or_else(|| {
                    invalid(format!(
                        "the result has no ensemble {} at {radius_um} um",
                        ensemble.join(", ")
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SpaceRankView::Radii {
        variables: ensemble.to_vec(),
        radii,
    })
}
