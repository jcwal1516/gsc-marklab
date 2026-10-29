//! The values or smoothed joint distribution of one or two census variables in one specimen
//! (R `plot_dist`, without the plot).

use serde::{Deserialize, Serialize};

use super::binning::{min_max, NearestGrid};
use super::census::{check_retained, draw_discs, draw_seeds, CensusCost, CensusPlan};
use super::distribution::chao_jost;
use super::ensembles::{selected_radii, selected_specimens};
use super::input::VariableSpec;
use super::{invalid, SpaceError, SpaceInput, SPACE_FORMAT_VERSION, SPACE_REFERENCE_COMMIT};
use crate::common::seeds::{derive_seed, SeedEndpoint};

pub const SPACE_DIST_FORMAT: &str = "marklab.space_dist_result";

/// The arguments of R `plot_dist` that shape its data; the plot arguments are not ported.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceDistParameters {
    /// One or two census variables (R `ensemble`).
    pub variables: Vec<String>,
    pub radius_um: f64,
    /// The specimen to describe; may be omitted when the request has one.
    pub specimen: Option<String>,
    /// Rounding bins per variable (R `bin_num`); without it each disc's values are reported.
    pub bins: Option<usize>,
    /// Describe one randomized census instead of the observed one (R `patch_list`).
    pub randomized: bool,
}

/// Which census the values come from.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpaceDistCensus {
    Observed,
    /// Every layer's members shuffled across discs, as one cisMI randomized census.
    Randomized,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceDistVariable {
    pub name: String,
    /// Smallest and largest value over this census's discs.
    pub minimum: f64,
    pub maximum: f64,
}

/// The values one variable takes in the distribution.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceDistAxis {
    pub variable: String,
    /// Whether the values were rounded onto `bins` equally spaced points between the
    /// minimum and maximum; otherwise the variable had at most `bins` distinct values,
    /// which are used as they are.
    pub rounded: bool,
    pub values: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceDistCell {
    /// One axis value per variable.
    pub values: Vec<f64>,
    /// Smoothed probability; absent where the variables are classes of one categorical
    /// channel whose percentages sum above 100 (R `NA`).
    pub frequency: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpaceDistValues {
    /// Each disc's values, in the order of `variables`.
    Discs { discs: Vec<Vec<f64>> },
    /// Every combination of axis values, the first variable varying fastest (R `build_dist`
    /// then `smooth_dist(full_dist = TRUE)`).
    Distribution {
        axes: Vec<SpaceDistAxis>,
        cells: Vec<SpaceDistCell>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceDistResult {
    pub format: String,
    pub version: u32,
    pub reference_commit: String,
    pub source_kind: String,
    pub seed: u64,
    pub parameters: SpaceDistParameters,
    pub specimen_id: String,
    pub census: SpaceDistCensus,
    pub disc_count: usize,
    pub variables: Vec<SpaceDistVariable>,
    pub values: SpaceDistValues,
}

/// Describe how one or two variables co-occur across one specimen's discs.
pub fn space_dist(
    input: &SpaceInput,
    parameters: &SpaceDistParameters,
) -> Result<SpaceDistResult, SpaceError> {
    let count = parameters.variables.len();
    if !(1..=2).contains(&count) {
        return Err(invalid("a distribution describes one or two variables"));
    }
    if count == 2 && parameters.variables[0] == parameters.variables[1] {
        return Err(invalid("the two variables must differ"));
    }
    if let Some(bins) = parameters.bins {
        if !(2..=255).contains(&bins) {
            return Err(invalid("bins must be between 2 and 255"));
        }
    }
    let radius_position = selected_radii(input, &[parameters.radius_um])?[0];
    let position = match &parameters.specimen {
        Some(id) => selected_specimens(input, std::slice::from_ref(id))?[0],
        None if input.specimens.len() == 1 => 0,
        None => return Err(invalid("name the specimen; the request has several")),
    };
    let specimen = &input.specimens[position];
    let specs = parameters
        .variables
        .iter()
        .map(|name| {
            let index = input
                .variable_index(name)
                .ok_or_else(|| invalid(format!("unknown variable {name}")))?;
            let spec = input.variables[index];
            if !specimen.has(spec) {
                return Err(invalid(format!(
                    "specimen {} has no cells of variable {name}",
                    specimen.id
                )));
            }
            Ok(spec)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let seeds = draw_seeds(input, position, radius_position)?;
    let discs_drawn = seeds.len();
    let cells = parameters.bins.map_or(0, |bins| bins.pow(count as u32));
    // The census values and the reported values or cells.
    let fixed = (2 * count * discs_drawn * 8).saturating_add(cells.saturating_mul(48));
    let cost = CensusCost::new(specimen, &specs, 1);
    check_retained(input, fixed.saturating_add(cost.bytes(discs_drawn, 0)))?;
    let discs = draw_discs(
        specimen,
        seeds,
        parameters.radius_um,
        cost.member_cap(
            discs_drawn,
            input.limits.maximum_retained_bytes - fixed,
            input.limits.maximum_disc_members,
        ),
    )?;
    let plan = CensusPlan::new(specimen, &discs, &specs);
    let mut scratch = plan.scratch();
    let values = if parameters.randomized {
        let mut values = vec![0.0; count * discs_drawn];
        let stream = derive_seed(input.census.seed, SeedEndpoint::SpaceDistNull, position);
        plan.randomized(
            specimen,
            derive_seed(stream, SeedEndpoint::SpaceDistNull, radius_position),
            &mut scratch,
            &mut values,
        );
        values
    } else {
        plan.observed(specimen, &mut scratch)
    };
    let column = |variable: usize| &values[variable * discs_drawn..(variable + 1) * discs_drawn];

    let variables = parameters
        .variables
        .iter()
        .enumerate()
        .map(|(variable, name)| {
            let (minimum, maximum) = min_max(column(variable));
            SpaceDistVariable {
                name: name.clone(),
                minimum,
                maximum,
            }
        })
        .collect::<Vec<_>>();
    let values = match parameters.bins {
        None => SpaceDistValues::Discs {
            discs: (0..discs_drawn)
                .map(|disc| (0..count).map(|variable| column(variable)[disc]).collect())
                .collect(),
        },
        Some(bins) => distribution(&parameters.variables, &specs, &column, discs_drawn, bins)?,
    };
    Ok(SpaceDistResult {
        format: SPACE_DIST_FORMAT.into(),
        version: SPACE_FORMAT_VERSION,
        reference_commit: SPACE_REFERENCE_COMMIT.into(),
        source_kind: input.source_kind.into(),
        seed: input.census.seed,
        parameters: parameters.clone(),
        specimen_id: specimen.id.clone(),
        census: if parameters.randomized {
            SpaceDistCensus::Randomized
        } else {
            SpaceDistCensus::Observed
        },
        disc_count: discs_drawn,
        variables,
        values,
    })
}

/// R rounds a variable onto `bins` points only when it has more distinct values than that,
/// on this census's own minimum and maximum, then counts and smooths every combination.
fn distribution<'a>(
    names: &[String],
    specs: &[VariableSpec],
    column: &impl Fn(usize) -> &'a [f64],
    discs: usize,
    bins: usize,
) -> Result<SpaceDistValues, SpaceError> {
    let mut axes = Vec::with_capacity(names.len());
    let mut codes = Vec::with_capacity(names.len());
    for (variable, name) in names.iter().enumerate() {
        let source = column(variable);
        // Adding zero turns -0.0 into 0.0, which R's unique() treats as one value.
        let mut distinct = source.iter().map(|value| value + 0.0).collect::<Vec<_>>();
        distinct.sort_unstable_by(f64::total_cmp);
        distinct.dedup();
        if distinct.len() > bins {
            let (low, high) = min_max(source);
            let grid = NearestGrid::new(low, high, bins).expect("more than one distinct value");
            codes.push(
                source
                    .iter()
                    .map(|value| grid.nearest(*value) as usize)
                    .collect(),
            );
            axes.push(SpaceDistAxis {
                variable: name.clone(),
                rounded: true,
                values: grid.points().to_vec(),
            });
        } else {
            codes.push(
                source
                    .iter()
                    .map(|value| {
                        distinct
                            .binary_search_by(|known| known.total_cmp(&(value + 0.0)))
                            .expect("every value is among the distinct values")
                    })
                    .collect::<Vec<_>>(),
            );
            axes.push(SpaceDistAxis {
                variable: name.clone(),
                rounded: false,
                values: distinct,
            });
        }
    }

    let lengths = axes
        .iter()
        .map(|axis| axis.values.len())
        .collect::<Vec<_>>();
    let cells = lengths.iter().product::<usize>();
    let mut disc_cells = vec![0_usize; discs];
    let mut stride = 1;
    for (variable_codes, length) in codes.iter().zip(&lengths) {
        for (cell, code) in disc_cells.iter_mut().zip(variable_codes) {
            *cell += code * stride;
        }
        stride *= length;
    }
    let mut counts = vec![0_u32; cells];
    for cell in disc_cells {
        counts[cell] += 1;
    }
    // Two classes of one categorical channel cannot share more than 100% of a disc.
    let compositional = matches!(
        specs,
        [VariableSpec::Object { map: first, .. }, VariableSpec::Object { map: second, .. }]
            if first == second
    );
    let axis_values = |cell: usize| {
        let mut remainder = cell;
        axes.iter()
            .map(|axis| {
                let value = axis.values[remainder % axis.values.len()];
                remainder /= axis.values.len();
                value
            })
            .collect::<Vec<_>>()
    };
    let mut frequencies = counts
        .iter()
        .enumerate()
        .map(|(cell, &count)| {
            if compositional && axis_values(cell).iter().sum::<f64>() > 100.0 {
                f64::NAN
            } else {
                f64::from(count)
            }
        })
        .collect::<Vec<_>>();
    if frequencies
        .iter()
        .all(|frequency| frequency.is_nan() || *frequency == 0.0)
    {
        return Err(invalid(
            "every observed combination rounds to an impossible composition; use more bins",
        ));
    }
    chao_jost(&mut frequencies, &counts, discs);
    Ok(SpaceDistValues::Distribution {
        cells: frequencies
            .iter()
            .enumerate()
            .map(|(cell, &frequency)| SpaceDistCell {
                values: axis_values(cell),
                frequency: (!frequency.is_nan()).then_some(frequency),
            })
            .collect(),
        axes,
    })
}
