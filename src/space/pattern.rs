//! The spatial pattern of one ensemble (R `learn_pattern`): a one-dimensional self-organizing
//! map drawn through the discs' values orders every disc along a latent path, and rolling
//! summaries along that path show how the variables covary. A reference census, randomized or
//! from other specimens, adds the enrichment of each part of the path.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::census::{check_retained, draw_discs, draw_seeds, CensusCost, CensusPlan, Discs};
use super::ensembles::{selected_radii, selected_specimens};
use super::som::{nearest, path_order, train, Schedule, SomGrid};
use super::{
    for_each_chunk, invalid, resource, SpaceError, SpaceInput, SpaceVariableKind,
    SPACE_FORMAT_VERSION, SPACE_REFERENCE_COMMIT,
};
use crate::common::seeds::{derive_seed, splitmix64, SeedEndpoint};
use crate::permutation::labels::deterministic_shuffle;

pub const SPACE_PATTERN_FORMAT: &str = "marklab.space_pattern_result";

/// R subsamples the pooled focal discs, and the pooled reference discs, to this many when
/// `sub_sample` is set.
const SUBSAMPLE: usize = 10_000;
/// R's largest map.
const MAXIMUM_NODES: usize = 1_000;
const MAXIMUM_SMOOTH_WINDOW: usize = 1_000_000;

/// How curves are rescaled for display (R `obj_norm` and `scl_norm`).
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpaceNormalization {
    /// Each variable by its own largest upper bound (R `"ind"`).
    Individual,
    /// Every variable of the kind by their largest upper bound (R `"all"`).
    Collective,
}

/// The arguments of R `learn_pattern`, with its defaults; the plot arguments are not ported.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpacePatternParameters {
    /// The ensemble's variables.
    pub variables: Vec<String>,
    pub radius_um: f64,
    /// Specimens whose censuses are used; empty for all.
    pub specimens: Vec<String>,
    /// The grouping factor holding `focal` and `reference`; inferred as R does when absent.
    pub factor: Option<String>,
    /// Levels whose specimens' discs draw the path (R `focal`).
    pub focal: Vec<String>,
    /// Levels whose specimens' discs are the reference (R `reference`).
    pub reference: Vec<String>,
    /// Compare one specimen with one randomized census of itself (R `patch_list`).
    pub randomized_reference: bool,
    /// Coverage of the rolling band around each curve (R `conf_int`).
    pub conf_int: f64,
    pub obj_norm: Option<SpaceNormalization>,
    pub scl_norm: Option<SpaceNormalization>,
    /// Passes over the data while training the map (R `som_reps`).
    pub som_reps: usize,
    /// Whether the map is a ring rather than a chain.
    pub toroidal: bool,
    /// Discs smoothed together along the path (R `smooth_window`).
    pub smooth_window: usize,
    /// Subsample the focal discs, and the reference discs, to 10,000 each.
    pub sub_sample: bool,
}

impl Default for SpacePatternParameters {
    fn default() -> Self {
        Self {
            variables: Vec::new(),
            radius_um: 0.0,
            specimens: Vec::new(),
            factor: None,
            focal: Vec::new(),
            reference: Vec::new(),
            randomized_reference: false,
            conf_int: 0.95,
            obj_norm: None,
            scl_norm: Some(SpaceNormalization::Collective),
            som_reps: 50,
            toroidal: false,
            smooth_window: 100,
            sub_sample: false,
        }
    }
}

/// What the focal discs were compared with.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpacePatternReference {
    None,
    Randomized,
    Group,
}

/// One variable's curves along the path, one value per focal disc.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpacePatternCurve {
    pub variable: String,
    pub kind: SpaceVariableKind,
    /// Rolling mean (R `Y`).
    pub mean: Vec<f64>,
    /// Smoothed rolling lower and upper quantiles of the `conf_int` band (R `Ymin`, `Ymax`).
    pub lower: Vec<f64>,
    pub upper: Vec<f64>,
    /// The curves times `100 / normalization_scale` (R `Y_norm` and so on); the raw curves
    /// when the kind is not normalized or the scale is not positive.
    pub mean_normalized: Vec<f64>,
    pub lower_normalized: Vec<f64>,
    pub upper_normalized: Vec<f64>,
    /// The largest upper bound used as 100%, when one applied.
    pub normalization_scale: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpacePatternResult {
    pub format: String,
    pub version: u32,
    pub reference_commit: String,
    pub source_kind: String,
    pub seed: u64,
    pub parameters: SpacePatternParameters,
    pub factor: Option<String>,
    pub focal_specimens: Vec<String>,
    pub reference: SpacePatternReference,
    pub reference_specimens: Vec<String>,
    /// Discs the path is drawn through, after subsampling.
    pub focal_disc_count: usize,
    pub reference_disc_count: usize,
    /// The map's starting radius, kohonen's default.
    pub initial_radius: f64,
    /// The trained map's nodes in path order.
    pub nodes: Vec<Vec<f64>>,
    /// Each focal disc's place along the path, 0 to 100, in path order (R `X`).
    pub positions: Vec<f64>,
    /// Rolling log ratio of weighted focal to reference frequency at each place; 1 everywhere
    /// without a reference, as in R (R `Enr`).
    pub enrichment: Vec<f64>,
    pub curves: Vec<SpacePatternCurve>,
}

/// One specimen's census for an ensemble at one radius.
pub(super) struct EnsembleCensus {
    pub discs: Discs,
    /// Disc-major: `values[disc * variables + variable]`; a variable the specimen lacks is 0,
    /// as R's `standardize_censuses` makes it.
    pub values: Vec<f64>,
}

/// Census one specimen for the declared variables `variables` (positions in the request's
/// variable list), observed or randomized with `randomized`, within the bytes left after `held`.
pub(super) fn ensemble_census(
    input: &SpaceInput,
    position: usize,
    radius_position: usize,
    variables: &[usize],
    randomized: Option<u64>,
    held: usize,
) -> Result<EnsembleCensus, SpaceError> {
    let specimen = &input.specimens[position];
    let present = variables
        .iter()
        .enumerate()
        .filter(|(_, &index)| specimen.has(input.variables[index]))
        .map(|(slot, &index)| (slot, input.variables[index]))
        .collect::<Vec<_>>();
    let specs = present.iter().map(|(_, spec)| *spec).collect::<Vec<_>>();
    let seeds = draw_seeds(input, position, radius_position)?;
    let count = seeds.len();
    let cost = CensusCost::new(&specs, 1);
    let fixed = held.saturating_add(
        count
            .saturating_mul(variables.len() + specs.len())
            .saturating_mul(8),
    );
    check_retained(input, fixed.saturating_add(cost.bytes(count, 0)))?;
    let discs = draw_discs(
        specimen,
        seeds,
        input.census.radii_um[radius_position],
        cost.member_cap(
            count,
            input.limits.maximum_retained_bytes - fixed,
            input.limits.maximum_disc_members,
        ),
    )?;
    let plan = CensusPlan::new(specimen, &discs, &specs);
    let mut scratch = plan.scratch();
    let census = match randomized {
        Some(seed) => {
            let mut values = vec![0.0; specs.len() * count];
            plan.randomized(specimen, seed, &mut scratch, &mut values);
            values
        }
        None => plan.observed(specimen, &mut scratch),
    };
    let mut values = vec![0.0; count * variables.len()];
    for (column, &(slot, _)) in present.iter().enumerate() {
        for disc in 0..count {
            values[disc * variables.len() + slot] = census[column * count + disc];
        }
    }
    Ok(EnsembleCensus { discs, values })
}

/// Positions in the request's variable list of the named ensemble.
pub(super) fn ensemble_variables(
    input: &SpaceInput,
    names: &[String],
) -> Result<Vec<usize>, SpaceError> {
    if names.is_empty() {
        return Err(invalid("name at least one variable"));
    }
    if names.iter().collect::<BTreeSet<_>>().len() != names.len() {
        return Err(invalid("the ensemble repeats a variable"));
    }
    names
        .iter()
        .map(|name| {
            input
                .variable_index(name)
                .ok_or_else(|| invalid(format!("unknown variable {name}")))
        })
        .collect()
}

/// Uniform draws from SplitMix64 in counter mode.
struct Draws {
    seed: u64,
    count: u64,
}

impl Draws {
    fn new(seed: u64) -> Self {
        Self { seed, count: 0 }
    }

    /// A uniform index below `bound`.
    fn index(&mut self, bound: usize) -> usize {
        let value = splitmix64(self.seed.wrapping_add(self.count.wrapping_mul(GAMMA)));
        self.count += 1;
        (value % bound as u64) as usize
    }

    /// `count` values drawn from `values` with replacement (R `sample(x, size, replace = TRUE)`).
    fn sample(&mut self, values: &[f64], count: usize) -> Vec<f64> {
        (0..count)
            .map(|_| values[self.index(values.len())])
            .collect()
    }
}

const GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;

/// Random streams of one pattern run, each derived from the census seed.
#[derive(Clone, Copy)]
enum Stream {
    FocalSample,
    ReferenceSample,
    Initial,
    Presentation,
    Padding,
    Randomized,
}

/// Which specimens draw the path and which are compared with it.
struct Design {
    factor: Option<String>,
    focal: Vec<usize>,
    reference: Vec<usize>,
    kind: SpacePatternReference,
}

/// Learn the spatial pattern of one ensemble at one radius.
pub fn space_pattern(
    input: &SpaceInput,
    parameters: &SpacePatternParameters,
) -> Result<SpacePatternResult, SpaceError> {
    let variables = ensemble_variables(input, &parameters.variables)?;
    let dims = variables.len();
    if !(0.0..=1.0).contains(&parameters.conf_int) {
        return Err(invalid("conf_int must be between 0 and 1"));
    }
    if parameters.som_reps == 0 {
        return Err(invalid("som_reps must be at least 1"));
    }
    if parameters.smooth_window > MAXIMUM_SMOOTH_WINDOW {
        return Err(invalid(format!(
            "smooth_window must be at most {MAXIMUM_SMOOTH_WINDOW}"
        )));
    }
    let radius_position = selected_radii(input, &[parameters.radius_um])?[0];
    let design = design(input, parameters)?;
    let base = derive_seed(
        input.census.seed,
        SeedEndpoint::SpacePattern,
        radius_position,
    );
    let stream = |stream: Stream| derive_seed(base, SeedEndpoint::SpacePattern, stream as usize);

    // The focal and reference discs, each subsampled as R does.
    let mut held = 0_usize;
    let mut gather = |positions: &[usize],
                      randomized: Option<u64>,
                      sample: Stream|
     -> Result<Vec<f64>, SpaceError> {
        let mut rows = Vec::new();
        for &position in positions {
            let census = ensemble_census(
                input,
                position,
                radius_position,
                &variables,
                randomized.map(|seed| derive_seed(seed, SeedEndpoint::SpacePattern, position)),
                held,
            )?;
            held = held.saturating_add(census.values.len() * 8);
            rows.extend(census.values);
        }
        let count = rows.len() / dims;
        if !parameters.sub_sample || count <= SUBSAMPLE {
            return Ok(rows);
        }
        let mut order = (0..count).collect::<Vec<_>>();
        deterministic_shuffle(&mut order, stream(sample));
        Ok(order[..SUBSAMPLE]
            .iter()
            .flat_map(|&row| rows[row * dims..(row + 1) * dims].iter().copied())
            .collect())
    };
    let focal = gather(&design.focal, None, Stream::FocalSample)?;
    let reference = match design.kind {
        SpacePatternReference::None => Vec::new(),
        SpacePatternReference::Randomized => gather(
            &design.focal,
            Some(stream(Stream::Randomized)),
            Stream::ReferenceSample,
        )?,
        SpacePatternReference::Group => gather(&design.reference, None, Stream::ReferenceSample)?,
    };
    let focal_count = focal.len() / dims;
    let reference_count = reference.len() / dims;
    if focal_count == 0 {
        return Err(invalid("the focal specimens have no discs"));
    }

    let nodes = focal_count.min(MAXIMUM_NODES);
    let steps = parameters.som_reps.saturating_mul(focal_count);
    let work = (steps as u64)
        .saturating_add((focal_count + reference_count) as u64)
        .saturating_mul(nodes as u64)
        .saturating_mul(dims as u64 + 1);
    if work > input.limits.maximum_entropy_visits {
        return Err(resource(format!(
            "{work} map visits exceed the maximum of {}",
            input.limits.maximum_entropy_visits
        )));
    }
    // The censuses, the map, and per focal disc its node, order and eight curve values per
    // variable (padded windows included).
    let result_bytes = (focal_count + 2 * parameters.smooth_window)
        .saturating_mul(8 * dims + 3)
        .saturating_mul(8);
    check_retained(
        input,
        held.saturating_add(nodes * dims * 16)
            .saturating_add(result_bytes),
    )?;

    // Train the map from rows drawn without replacement, as R's default `init`.
    let grid = SomGrid {
        nodes,
        toroidal: parameters.toroidal,
    };
    let mut initial = (0..focal_count).collect::<Vec<_>>();
    deterministic_shuffle(&mut initial, stream(Stream::Initial));
    let mut codes = initial[..nodes]
        .iter()
        .flat_map(|&row| focal[row * dims..(row + 1) * dims].iter().copied())
        .collect::<Vec<_>>();
    let initial_radius = grid.default_radius();
    let mut presentations = Draws::new(stream(Stream::Presentation));
    train(
        grid,
        &mut codes,
        &focal,
        dims,
        &Schedule {
            alpha: [0.05, 0.01],
            radius: [initial_radius, 0.0],
            steps,
        },
        || presentations.index(focal_count),
    );
    let order = path_order(&codes, dims, parameters.toroidal);
    let codes = order
        .iter()
        .flat_map(|&node| codes[node * dims..(node + 1) * dims].iter().copied())
        .collect::<Vec<_>>();

    // Order the focal discs by their nearest node along the path (R `order(SOM_nn)`).
    let focal_nodes = nearest_nodes(&codes, dims, &focal);
    let mut sorted = (0..focal_count).collect::<Vec<_>>();
    sorted.sort_by_key(|&row| focal_nodes[row]);
    let column = |variable: usize| {
        sorted
            .iter()
            .map(|&row| focal[row * dims + variable])
            .collect::<Vec<_>>()
    };
    let sorted_nodes = sorted
        .iter()
        .map(|&row| focal_nodes[row])
        .collect::<Vec<_>>();
    let first_block = sorted_nodes
        .iter()
        .take_while(|node| **node == sorted_nodes[0])
        .count();
    let last_block = sorted_nodes
        .iter()
        .rev()
        .take_while(|node| **node == sorted_nodes[focal_count - 1])
        .count();

    let half = (parameters.smooth_window as f64 / 2.0).round_ties_even() as usize;
    let width = 2 * half + 1;
    let mut padding = Draws::new(stream(Stream::Padding));
    // R pads the band and enrichment from the first `half` and last `half + 1` values.
    let smooth = |values: &[f64], padding: &mut Draws| {
        let count = values.len();
        let mut padded = padding.sample(&values[..half.min(count)], half);
        padded.extend_from_slice(values);
        padded.extend(padding.sample(&values[count - (half + 1).min(count)..], half));
        rolling_mean(&padded, width)
    };
    let lower_probability = (1.0 - parameters.conf_int) / 2.0;
    let upper_probability = 1.0 - (1.0 - parameters.conf_int) / 2.0;
    let mut curves = Vec::with_capacity(dims);
    for (variable, &index) in variables.iter().enumerate() {
        let values = column(variable);
        let mut padded = padding.sample(&values[..first_block], half);
        padded.extend_from_slice(&values);
        padded.extend(padding.sample(&values[focal_count - last_block..], half));
        let mean = rolling_mean(&padded, width);
        let lower = smooth(
            &rolling_quantile(&padded, width, lower_probability),
            &mut padding,
        );
        let upper = smooth(
            &rolling_quantile(&padded, width, upper_probability),
            &mut padding,
        );
        let description = &input.variable_descriptions[index];
        curves.push(SpacePatternCurve {
            variable: description.name.clone(),
            kind: description.kind,
            lower: lower.iter().zip(&mean).map(|(l, m)| l.min(*m)).collect(),
            upper: upper.iter().zip(&mean).map(|(u, m)| u.max(*m)).collect(),
            mean,
            mean_normalized: Vec::new(),
            lower_normalized: Vec::new(),
            upper_normalized: Vec::new(),
            normalization_scale: None,
        });
    }
    normalize(&mut curves, parameters.obj_norm, |kind| {
        kind == SpaceVariableKind::Object
    });
    normalize(&mut curves, parameters.scl_norm, |kind| {
        kind != SpaceVariableKind::Object
    });

    let enrichment = if reference_count == 0 {
        vec![1.0; focal_count]
    } else {
        let reference_nodes = nearest_nodes(&codes, dims, &reference);
        let focal_weight = node_frequencies(&codes, dims, &focal, &focal_nodes)?;
        let reference_weight = node_frequencies(&codes, dims, &reference, &reference_nodes)?;
        let mut ratio = focal_weight
            .iter()
            .zip(&reference_weight)
            .map(|(&f, &r)| if f == 0.0 { 0.0 } else { (f / r).ln() })
            .collect::<Vec<_>>();
        // Nodes reached only by focal discs take the largest finite enrichment (0 if none is
        // finite, where R's max() of nothing gives -Inf).
        let largest = ratio
            .iter()
            .copied()
            .filter(|value| value.is_finite())
            .reduce(f64::max)
            .unwrap_or(0.0);
        for value in &mut ratio {
            if *value == f64::INFINITY {
                *value = largest;
            }
        }
        let per_disc = sorted_nodes
            .iter()
            .map(|&node| ratio[node])
            .collect::<Vec<_>>();
        smooth(&per_disc, &mut padding)
    };

    let positions = (0..focal_count)
        .map(|place| {
            if place + 1 == focal_count && focal_count > 1 {
                100.0
            } else {
                place as f64 * (100.0 / (focal_count - 1).max(1) as f64)
            }
        })
        .collect();
    let names = |positions: &[usize]| {
        positions
            .iter()
            .map(|&position| input.specimens[position].id.clone())
            .collect::<Vec<_>>()
    };
    Ok(SpacePatternResult {
        format: SPACE_PATTERN_FORMAT.into(),
        version: SPACE_FORMAT_VERSION,
        reference_commit: SPACE_REFERENCE_COMMIT.into(),
        source_kind: input.source_kind.into(),
        seed: input.census.seed,
        parameters: parameters.clone(),
        factor: design.factor,
        focal_specimens: names(&design.focal),
        reference: design.kind,
        reference_specimens: names(&design.reference),
        focal_disc_count: focal_count,
        reference_disc_count: reference_count,
        initial_radius,
        nodes: codes.chunks_exact(dims).map(<[f64]>::to_vec).collect(),
        positions,
        enrichment,
        curves,
    })
}

/// R: one census with a patch list, focal and reference groups of several censuses, or every
/// census pooled.
fn design(input: &SpaceInput, parameters: &SpacePatternParameters) -> Result<Design, SpaceError> {
    let selected = selected_specimens(input, &parameters.specimens)?;
    let grouped = !parameters.focal.is_empty() || !parameters.reference.is_empty();
    if parameters.randomized_reference {
        if grouped || parameters.factor.is_some() || selected.len() != 1 {
            return Err(invalid(
                "a randomized reference compares one specimen with itself; give one specimen and no groups",
            ));
        }
        return Ok(Design {
            factor: None,
            focal: selected,
            reference: Vec::new(),
            kind: SpacePatternReference::Randomized,
        });
    }
    if !grouped {
        if parameters.factor.is_some() {
            return Err(invalid("a factor needs focal and reference levels"));
        }
        return Ok(Design {
            factor: None,
            focal: selected,
            reference: Vec::new(),
            kind: SpacePatternReference::None,
        });
    }
    if parameters.focal.is_empty() || parameters.reference.is_empty() {
        return Err(invalid("give both focal and reference levels"));
    }
    if parameters
        .focal
        .iter()
        .any(|level| parameters.reference.contains(level))
    {
        return Err(invalid("a level cannot be both focal and reference"));
    }
    let level = |position: usize, factor: &str| input.specimens[position].groups.get(factor);
    let factor =
        match &parameters.factor {
            Some(factor) => factor.clone(),
            None => {
                // R: the factor whose levels include every focal and reference level.
                let candidates = selected
                    .iter()
                    .flat_map(|&position| input.specimens[position].groups.keys())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .filter(|factor| {
                        parameters
                            .focal
                            .iter()
                            .chain(&parameters.reference)
                            .all(|wanted| {
                                selected
                                    .iter()
                                    .any(|&position| level(position, factor) == Some(wanted))
                            })
                    })
                    .collect::<Vec<_>>();
                match candidates.as_slice() {
                    [only] => (*only).clone(),
                    [] => {
                        return Err(invalid(
                            "the focal and reference levels do not match any grouping factor",
                        ))
                    }
                    _ => return Err(invalid(
                        "the focal and reference levels match several grouping factors; name one",
                    )),
                }
            }
        };
    let members = |levels: &[String]| {
        selected
            .iter()
            .copied()
            .filter(|&position| level(position, &factor).is_some_and(|l| levels.contains(l)))
            .collect::<Vec<_>>()
    };
    let (focal, reference) = (members(&parameters.focal), members(&parameters.reference));
    if focal.is_empty() || reference.is_empty() {
        return Err(invalid(format!(
            "grouping factor {factor} leaves the focal or the reference group without specimens"
        )));
    }
    Ok(Design {
        factor: Some(factor),
        focal,
        reference,
        kind: SpacePatternReference::Group,
    })
}

/// Each row's nearest node (R `FNN::knnx.index(..., k = 1)`; the first node on ties).
pub(super) fn nearest_nodes(codes: &[f64], dims: usize, rows: &[f64]) -> Vec<usize> {
    let mut nodes = vec![0_usize; rows.len() / dims];
    let chunk = 1024;
    for_each_chunk(
        &mut nodes,
        chunk,
        || (),
        |(), index, slots| {
            for (offset, slot) in slots.iter_mut().enumerate() {
                let row = index * chunk + offset;
                *slot = nearest(codes, dims, &rows[row * dims..(row + 1) * dims]);
            }
        },
    );
    nodes
}

/// Per node, the summed weights `1 - D/sqrt(k)` of its rows over the number of rows, with `D`
/// the distance to the node on SPACE's 0 to 100 scale.
fn node_frequencies(
    codes: &[f64],
    dims: usize,
    rows: &[f64],
    nodes: &[usize],
) -> Result<Vec<f64>, SpaceError> {
    let mut frequencies = vec![0.0; codes.len() / dims];
    let root = (dims as f64).sqrt();
    for (row, &node) in rows.chunks_exact(dims).zip(nodes) {
        let code = &codes[node * dims..(node + 1) * dims];
        let distance = row
            .iter()
            .zip(code)
            .map(|(value, centre)| ((value - centre) / 100.0).powi(2))
            .sum::<f64>()
            .sqrt();
        let weight = (root - distance) / root;
        if weight < 0.0 {
            return Err(invalid(
                "enrichment weights need every variable on SPACE's 0 to 100 scale; a disc lies farther than that from its node",
            ));
        }
        frequencies[node] += weight;
    }
    let count = nodes.len() as f64;
    Ok(frequencies
        .into_iter()
        .map(|frequency| frequency / count)
        .collect())
}

/// Rescale the curves of one kind so their largest upper bound becomes 100.
fn normalize(
    curves: &mut [SpacePatternCurve],
    normalization: Option<SpaceNormalization>,
    applies: impl Fn(SpaceVariableKind) -> bool,
) {
    let largest = |curve: &SpacePatternCurve| {
        curve
            .upper
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let collective = curves
        .iter()
        .filter(|curve| applies(curve.kind))
        .map(largest)
        .fold(f64::NEG_INFINITY, f64::max);
    for curve in curves.iter_mut().filter(|curve| applies(curve.kind)) {
        let scale = match normalization {
            None => None,
            Some(SpaceNormalization::Collective) => Some(collective),
            Some(SpaceNormalization::Individual) => Some(largest(curve)),
        }
        .filter(|scale| *scale > 0.0);
        let rescale = |values: &[f64]| match scale {
            Some(scale) => values.iter().map(|value| 100.0 * (value / scale)).collect(),
            None => values.to_vec(),
        };
        curve.mean_normalized = rescale(&curve.mean);
        curve.lower_normalized = rescale(&curve.lower);
        curve.upper_normalized = rescale(&curve.upper);
        curve.normalization_scale = scale;
    }
}

/// R `zoo::rollapply(x, width, mean)`, which zoo computes as `rollmean`: the first window's sum,
/// then a running sum of each entering value minus the leaving one, divided by the width.
fn rolling_mean(values: &[f64], width: usize) -> Vec<f64> {
    let count = width as f64;
    let mut sum = values[..width].iter().sum::<f64>();
    let mut means = Vec::with_capacity(values.len() - width + 1);
    means.push(sum / count);
    for (entering, leaving) in values[width..].iter().zip(values) {
        sum += entering - leaving;
        means.push(sum / count);
    }
    means
}

/// R `zoo::rollapply(x, width, quantile, probability)` with R's default type-7 quantile.
fn rolling_quantile(values: &[f64], width: usize, probability: f64) -> Vec<f64> {
    let mut window = values[..width].to_vec();
    window.sort_unstable_by(f64::total_cmp);
    let index = (width - 1) as f64 * probability;
    let (lower, upper) = (index.floor() as usize, index.ceil() as usize);
    let fraction = index - index.floor();
    let quantile = |sorted: &[f64]| {
        let low = sorted[lower];
        if fraction > 0.0 && sorted[upper] != low {
            (1.0 - fraction) * low + fraction * sorted[upper]
        } else {
            low
        }
    };
    let mut quantiles = Vec::with_capacity(values.len() + 1 - width);
    quantiles.push(quantile(&window));
    for (outgoing, incoming) in values.iter().zip(&values[width..]) {
        let gone = window
            .binary_search_by(|value| value.total_cmp(outgoing))
            .expect("the outgoing value is in the window");
        window.remove(gone);
        let place = window
            .binary_search_by(|value| value.total_cmp(incoming))
            .unwrap_or_else(|place| place);
        window.insert(place, *incoming);
        quantiles.push(quantile(&window));
    }
    quantiles
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(observed: &[f64], expected: &[f64]) {
        assert_eq!(observed.len(), expected.len());
        for (observed, expected) in observed.iter().zip(expected) {
            assert!(
                (observed - expected).abs() <= 1e-12 * expected.abs().max(1.0),
                "{observed:?} versus {expected:?}"
            );
        }
    }

    #[test]
    fn rolling_summaries_match_zoo_rollapply() {
        // R zoo 1.9-1: rollapply(x, 5, mean) and rollapply(x, 5, quantile, p).
        let x = [
            3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0, 5.0, 3.0, 5.0, 8.0, 9.0, 7.0, 9.0, 3.0, 2.0,
            3.0, 8.0, 4.0,
        ];
        close(
            &rolling_mean(&x, 5),
            &[
                2.8, 4.0, 4.2, 4.6, 5.4, 5.0, 4.2, 5.4, 6.0, 6.4, 7.6, 7.2, 6.0, 4.8, 5.0, 4.0,
            ],
        );
        // zoo's running sum rounds differently from averaging each window: 7 of these 10 means
        // differ from R mean() of the window, and every one equals zoo's bit for bit.
        let thirds = [
            32.96366666666667,
            13.258333333333333,
            3.856666666666667,
            2.3249999999999997,
            8.125,
            26.400333333333332,
            11.335333333333333,
            32.402,
            5.528666666666666,
            15.303333333333333,
            5.7250000000000005,
            7.716,
        ];
        assert_eq!(
            rolling_mean(&thirds, 3),
            [
                16.69288888888889,
                6.48,
                4.768888888888889,
                12.283444444444447,
                15.28688888888889,
                23.379222222222225,
                16.422,
                17.74466666666667,
                8.852333333333336,
                9.581444444444449,
            ]
        );
        close(
            &rolling_quantile(&x, 5, 0.025),
            &[
                1.0,
                1.0,
                1.1,
                1.1,
                2.3000000000000003,
                2.1,
                2.1,
                3.2,
                3.2,
                3.2,
                5.200000000000001,
                3.4000000000000004,
                2.1,
                2.1,
                2.1,
                2.1,
            ],
        );
        close(
            &rolling_quantile(&x, 5, 0.975),
            &[
                4.9,
                8.600000000000001,
                8.600000000000001,
                8.700000000000001,
                8.700000000000001,
                8.700000000000001,
                5.9,
                7.800000000000001,
                8.9,
                8.9,
                9.0,
                9.0,
                9.0,
                8.8,
                8.9,
                7.600000000000001,
            ],
        );
        close(
            &rolling_mean(&[0.1, 0.2, 0.3, 1.0 / 3.0, 2.0 / 3.0, 0.7], 3),
            &[
                0.20000000000000004,
                0.2777777777777778,
                0.43333333333333335,
                0.5666666666666667,
            ],
        );
    }
}
