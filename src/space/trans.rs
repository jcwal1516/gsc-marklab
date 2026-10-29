//! transMI: how differently each ensemble is patterned across groups of specimens
//! (R `measure_transMI`, `build_dist` and `smooth_dist`).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::binning::{bin_count, min_max, minimum_discs_for_two_bins, NearestGrid};
use super::census::{
    check_retained, disc_count, draw_discs, draw_seeds, present_variables, CensusCost, CensusPlan,
};
use super::cis::SpaceNullStatus;
use super::distribution::chao_jost;
use super::ensembles::{selected_radii, selected_specimens, EnsembleOptions, Ensembles};
use super::input::VariableSpec;
use super::{
    for_each_chunk, invalid, resource, worker_threads, SpaceError, SpaceInput, SpaceVariable,
    SPACE_FORMAT_VERSION, SPACE_REFERENCE_COMMIT,
};
use crate::common::seeds::{derive_seed, SeedEndpoint};
use crate::inference::multiple_testing::benjamini_hochberg;
use crate::permutation::labels::deterministic_shuffle;

pub const SPACE_TRANS_FORMAT: &str = "marklab.space_trans_result";

/// The arguments of R `measure_transMI`, with SPACE's defaults. `depth` has no default in R;
/// 3 is the value the SPACE tutorial uses.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceTransParameters {
    /// Largest ensemble size.
    pub depth: usize,
    /// Shuffles of the pairwise divergences per ensemble; at least two.
    pub bootstraps: usize,
    /// Variables every largest ensemble must contain.
    pub all: Vec<String>,
    /// Variables of which every largest ensemble must contain at least one.
    pub alo: Vec<String>,
    /// Variables excluded from every ensemble.
    pub not: Vec<String>,
    /// Upper bound on bins per variable. R accepts it but does not apply it.
    pub max_bins: usize,
    /// Census radii to analyse; empty for all.
    pub radii_um: Vec<f64>,
    /// Specimens to compare; empty for all.
    pub specimens: Vec<String>,
    /// Grouping factors to test; empty for every factor the specimens carry.
    pub factors: Vec<String>,
}

impl SpaceTransParameters {
    fn options(&self) -> EnsembleOptions<'_> {
        EnsembleOptions {
            depth: self.depth,
            bootstraps: self.bootstraps,
            max_bins: self.max_bins,
            all: &self.all,
            alo: &self.alo,
            not: &self.not,
        }
    }
}

impl Default for SpaceTransParameters {
    fn default() -> Self {
        Self {
            depth: 3,
            bootstraps: 100,
            all: Vec::new(),
            alo: Vec::new(),
            not: Vec::new(),
            max_bins: 100,
            radii_um: Vec::new(),
            specimens: Vec::new(),
            factors: Vec::new(),
        }
    }
}

/// One grouping factor and its levels, in the order statistics refer to them.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceTransFactor {
    pub factor: String,
    pub levels: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceTransSpecimen {
    pub specimen_id: String,
    pub groups: BTreeMap<String, String>,
    pub cell_count: usize,
}

/// One ensemble's transMI for one grouping factor.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceTransStatistic {
    pub factor: String,
    /// Directed weighted modularity of the factor's groups on the specimen similarity network
    /// (R `TransMI_<factor>`).
    pub trans_mi: f64,
    pub null_mean: f64,
    /// Sample standard deviation of the modularity over shuffled divergences.
    pub null_sd: f64,
    /// `(trans_mi - null_mean) / null_sd`; 0 when `null_sd` is 0, as in R (R `Zscore`).
    pub z_score: f64,
    pub status: SpaceNullStatus,
    /// SPACE's one-sided normal approximation `1 - Phi(Z)` (R `Pvalue`).
    pub p_value_normal: f64,
    /// Benjamini–Hochberg adjustment of `p_value_normal` over every ensemble at the radius,
    /// separately for each factor (R `Padjust`).
    pub p_adjust_bh: f64,
    /// `(1 + #{b: null_b >= trans_mi}) / (1 + bootstraps)` from the same shuffles.
    pub p_value_monte_carlo: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceTransEnsemble {
    pub variables: Vec<String>,
    /// One statistic per factor, in the order of the result's `factors`.
    pub factors: Vec<SpaceTransStatistic>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceTransRadius {
    pub radius_um: f64,
    /// Discs per specimen, in the order of the result's `specimens`.
    pub disc_counts: Vec<usize>,
    pub ensembles: Vec<SpaceTransEnsemble>,
}

/// transMI of every ensemble at every analysed radius.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceTransResult {
    pub format: String,
    pub version: u32,
    pub reference_commit: String,
    pub source_kind: String,
    pub seed: u64,
    pub parameters: SpaceTransParameters,
    pub variables: Vec<SpaceVariable>,
    /// Variables analysed: those present in any compared specimen, after `not` and the
    /// implicit exclusion when `all` and `alo` fill the depth.
    pub analysed_variables: Vec<String>,
    /// Compared specimens; divergences pair them in this order.
    pub specimens: Vec<SpaceTransSpecimen>,
    pub factors: Vec<SpaceTransFactor>,
    /// Bins per variable, from the smallest disc count over the compared specimens and
    /// analysed radii.
    pub bins: usize,
    pub minimum_disc_count: usize,
    pub radii: Vec<SpaceTransRadius>,
}

/// Rank every ensemble of up to `depth` variables by how well its patterning separates the
/// groups of each factor.
pub fn space_trans(
    input: &SpaceInput,
    parameters: &SpaceTransParameters,
) -> Result<SpaceTransResult, SpaceError> {
    let options = parameters.options();
    options.validate(input)?;
    let radius_positions = selected_radii(input, &parameters.radii_um)?;
    let compared = selected_specimens(input, &parameters.specimens)?;
    if compared.len() < 2 {
        return Err(invalid("transMI needs at least two specimens"));
    }
    let factors = grouping_factors(input, &compared, &parameters.factors)?;

    let present = compared
        .iter()
        .flat_map(|&position| present_variables(input, &input.specimens[position]))
        .map(|(index, _)| index)
        .collect::<BTreeSet<_>>();
    let candidates = input
        .variables
        .iter()
        .copied()
        .enumerate()
        .filter(|(index, _)| present.contains(index))
        .collect();
    let active = options.active(input, candidates);
    if parameters.depth > active.len() {
        return Err(invalid(format!(
            "depth {} exceeds the {} variables analysed",
            parameters.depth,
            active.len()
        )));
    }
    let specs = active.iter().map(|(_, spec)| *spec).collect::<Vec<_>>();
    let names = active
        .iter()
        .map(|(index, _)| input.variable_descriptions[*index].name.clone())
        .collect::<Vec<_>>();

    let minimum_disc_count = compared
        .iter()
        .flat_map(|&specimen| {
            radius_positions
                .iter()
                .map(move |&radius| disc_count(input, specimen, radius))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .min()
        .expect("at least one specimen and radius");
    let bins = bin_count(minimum_disc_count, parameters.depth, parameters.max_bins);
    if bins < 2 {
        return Err(invalid(format!(
            "the smallest census has {minimum_disc_count} discs; depth {} needs at least {} for two bins per variable",
            parameters.depth,
            minimum_discs_for_two_bins(parameters.depth)
        )));
    }
    let ensembles = Ensembles::new(specs.len(), &options, &names, input, 0)?;
    let study = Study {
        input,
        bootstraps: parameters.bootstraps,
        compared: &compared,
        factors: &factors,
        specs: &specs,
        names: &names,
        ensembles: &ensembles,
        bins,
    };

    let longest_factor = factors.iter().map(|(factor, _)| factor.factor.len()).max();
    let record = size_of::<SpaceTransEnsemble>()
        + factors.len() * (size_of::<SpaceTransStatistic>() + longest_factor.unwrap_or(0));
    let result_bytes = ensembles.result_bytes(&names, record);
    let mut retained = 0_usize;
    let mut radii = Vec::with_capacity(radius_positions.len());
    for &radius_position in &radius_positions {
        let radius =
            study.analyze_radius(radius_position, retained.saturating_add(result_bytes))?;
        retained = retained.saturating_add(result_bytes);
        radii.push(radius);
    }
    Ok(SpaceTransResult {
        format: SPACE_TRANS_FORMAT.into(),
        version: SPACE_FORMAT_VERSION,
        reference_commit: SPACE_REFERENCE_COMMIT.into(),
        source_kind: input.source_kind.into(),
        seed: input.census.seed,
        parameters: parameters.clone(),
        variables: input.variable_descriptions.clone(),
        analysed_variables: names,
        specimens: compared
            .iter()
            .map(|&position| {
                let specimen = &input.specimens[position];
                SpaceTransSpecimen {
                    specimen_id: specimen.id.clone(),
                    groups: specimen.groups.clone(),
                    cell_count: specimen.cell_ids.len(),
                }
            })
            .collect(),
        factors: factors.into_iter().map(|(factor, _)| factor).collect(),
        bins,
        minimum_disc_count,
        radii,
    })
}

/// Each tested factor with every compared specimen's level index (R
/// `as.numeric(as.factor(groups[, g]))`).
fn grouping_factors(
    input: &SpaceInput,
    compared: &[usize],
    requested: &[String],
) -> Result<Vec<(SpaceTransFactor, Vec<u32>)>, SpaceError> {
    let names = if requested.is_empty() {
        compared
            .iter()
            .flat_map(|&position| input.specimens[position].groups.keys().cloned())
            .collect::<BTreeSet<_>>()
    } else {
        requested.iter().cloned().collect()
    };
    if names.is_empty() {
        return Err(invalid(
            "transMI needs a grouping factor; give every specimen groups",
        ));
    }
    names
        .into_iter()
        .map(|factor| {
            let levels = compared
                .iter()
                .map(|&position| {
                    let specimen = &input.specimens[position];
                    specimen.groups.get(&factor).cloned().ok_or_else(|| {
                        invalid(format!(
                            "specimen {} has no level for grouping factor {factor}",
                            specimen.id
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let distinct = levels
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let membership = levels
                .iter()
                .map(|level| distinct.binary_search(level).expect("collected level") as u32)
                .collect();
            Ok((
                SpaceTransFactor {
                    factor,
                    levels: distinct,
                },
                membership,
            ))
        })
        .collect()
}

/// One specimen's census at one radius, rounded onto its own grid (R `round_column` with
/// each census's own minimum and maximum).
struct RoundedCensus {
    disc_count: usize,
    /// Variable-major grid indices: `bins[variable * disc_count + disc]`.
    bins: Vec<u8>,
    /// Per variable, the grid point values; a constant variable repeats its value, as R's
    /// `seq(value, value, length.out = b)` does.
    points: Vec<Vec<f64>>,
    constant: Vec<bool>,
}

/// What every radius of one transMI run shares.
struct Study<'a> {
    input: &'a SpaceInput,
    bootstraps: usize,
    compared: &'a [usize],
    factors: &'a [(SpaceTransFactor, Vec<u32>)],
    specs: &'a [VariableSpec],
    names: &'a [String],
    ensembles: &'a Ensembles,
    bins: usize,
}

impl Study<'_> {
    fn analyze_radius(
        &self,
        radius_position: usize,
        retained_results: usize,
    ) -> Result<SpaceTransRadius, SpaceError> {
        let input = self.input;
        let specimens = self.compared.len();
        let pairs = specimens * (specimens - 1) / 2;
        let variables = self.specs.len();
        let factor_count = self.factors.len();
        let depth = self.ensembles.depth;
        let total = self.ensembles.total();
        let disc_counts = self
            .compared
            .iter()
            .map(|&specimen| disc_count(input, specimen, radius_position))
            .collect::<Result<Vec<_>, _>>()?;
        let discs = disc_counts
            .iter()
            .fold(0_u64, |sum, count| sum.saturating_add(*count as u64));

        // Visits: each ensemble's discs and grid cells in every specimen, its divergence
        // pairs, and the modularity of every factor on the observed and shuffled networks.
        let work = (1..=depth).fold(0_u64, |sum, size| {
            let cells = (self.bins as u64).saturating_pow(size as u32);
            let per_ensemble = discs
                .saturating_add(cells.saturating_mul((specimens + 2 * pairs) as u64))
                .saturating_add(
                    ((self.bootstraps + 1) * factor_count) as u64 * (pairs + specimens) as u64,
                );
            sum.saturating_add(per_ensemble.saturating_mul(self.ensembles.count(size) as u64))
        });
        if work > input.limits.maximum_entropy_visits {
            return Err(resource(format!(
                "{work} transMI visits exceed the maximum of {}",
                input.limits.maximum_entropy_visits
            )));
        }
        let threads = worker_threads();
        let cells = self.bins.saturating_pow(depth as u32);
        let worker = cells
            .saturating_mul(specimens.saturating_add(1))
            .saturating_mul(8)
            .saturating_add(pairs.saturating_mul(16))
            .saturating_add(
                self.bootstraps
                    .saturating_mul(factor_count)
                    .saturating_mul(8),
            );
        // Results so far and this radius's, then the rounded censuses, the per-ensemble
        // statistics and each worker's distributions and networks.
        let fixed = [
            self.ensembles.table_bytes(),
            usize::try_from(discs)
                .unwrap_or(usize::MAX)
                .saturating_mul(variables),
            specimens
                .saturating_mul(variables)
                .saturating_mul(self.bins)
                .saturating_mul(8),
            total.saturating_mul(factor_count).saturating_mul(32),
            worker.saturating_mul(threads),
        ]
        .iter()
        .fold(retained_results, |sum, bytes| sum.saturating_add(*bytes));
        check_retained(input, fixed)?;

        let mut censuses = Vec::with_capacity(specimens);
        let mut held = fixed;
        for &position in self.compared {
            let census = self.rounded_census(position, radius_position, held)?;
            held = held.saturating_add(census.points.len() * self.bins * 8);
            censuses.push(census);
        }

        // Per ensemble and factor: modularity, null mean, null sd and Monte Carlo count.
        let mut rows = Vec::with_capacity(total);
        for size in 1..=depth {
            rows.extend((0..self.ensembles.count(size)).map(|index| (size, index)));
        }
        let pairs_list = (0..specimens)
            .flat_map(|first| ((first + 1)..specimens).map(move |second| (first, second)))
            .collect::<Vec<_>>();
        let radius_seed = derive_seed(
            input.census.seed,
            SeedEndpoint::SpaceTransNull,
            radius_position,
        );
        let mut statistics = vec![0.0; total * factor_count * 4];
        for_each_chunk(
            &mut statistics,
            factor_count * 4,
            || Scratch::new(specimens, cells, pairs, self.bootstraps * factor_count),
            |scratch, row, output| {
                let (size, index) = rows[row];
                let ensemble = self.ensembles.ensemble(size, index);
                self.ensemble_statistics(
                    ensemble,
                    &censuses,
                    &pairs_list,
                    derive_seed(radius_seed, SeedEndpoint::SpaceTransNull, row),
                    scratch,
                    output,
                );
            },
        );
        drop(censuses);

        let mut results = rows
            .iter()
            .zip(statistics.chunks_exact(factor_count * 4))
            .map(|(&(size, index), values)| SpaceTransEnsemble {
                variables: self
                    .ensembles
                    .ensemble(size, index)
                    .iter()
                    .map(|&variable| self.names[variable as usize].clone())
                    .collect(),
                factors: self
                    .factors
                    .iter()
                    .zip(values.as_chunks::<4>().0)
                    .map(|((factor, _), values)| summarize(&factor.factor, values, self.bootstraps))
                    .collect(),
            })
            .collect::<Vec<_>>();
        for factor in 0..factor_count {
            let adjusted = benjamini_hochberg(
                &results
                    .iter()
                    .map(|result| result.factors[factor].p_value_normal)
                    .collect::<Vec<_>>(),
            )
            .map_err(|error| invalid(format!("Benjamini-Hochberg adjustment failed: {error}")))?;
            for (result, adjusted) in results.iter_mut().zip(adjusted) {
                result.factors[factor].p_adjust_bh = adjusted;
            }
        }
        Ok(SpaceTransRadius {
            radius_um: input.census.radii_um[radius_position],
            disc_counts,
            ensembles: results,
        })
    }

    /// Draw, summarize and round one specimen's census within the bytes left after `held`.
    fn rounded_census(
        &self,
        position: usize,
        radius_position: usize,
        held: usize,
    ) -> Result<RoundedCensus, SpaceError> {
        let input = self.input;
        let specimen = &input.specimens[position];
        let seeds = draw_seeds(input, position, radius_position)?;
        let count = seeds.len();
        let variables = self.specs.len();
        let cost = CensusCost::new(specimen, self.specs, 1);
        let fixed = held.saturating_add(variables.saturating_mul(count).saturating_mul(8));
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
        let plan = CensusPlan::new(specimen, &discs, self.specs);
        let values = plan.observed(specimen, &mut plan.scratch());
        let mut bins = vec![0_u8; variables * count];
        let mut points = Vec::with_capacity(variables);
        let mut constant = Vec::with_capacity(variables);
        for (variable, target) in bins.chunks_exact_mut(count).enumerate() {
            let column = &values[variable * count..(variable + 1) * count];
            let (low, high) = min_max(column);
            match NearestGrid::new(low, high, self.bins) {
                Some(grid) => {
                    for (bin, value) in target.iter_mut().zip(column) {
                        *bin = grid.nearest(*value);
                    }
                    points.push(grid.points().to_vec());
                    constant.push(false);
                }
                None => {
                    points.push(vec![low; self.bins]);
                    constant.push(true);
                }
            }
        }
        Ok(RoundedCensus {
            disc_count: count,
            bins,
            points,
            constant,
        })
    }

    /// Writes `[modularity, null mean, null sd, Monte Carlo count]` per factor to `output`.
    fn ensemble_statistics(
        &self,
        ensemble: &[u16],
        censuses: &[RoundedCensus],
        pairs: &[(usize, usize)],
        seed: u64,
        scratch: &mut Scratch,
        output: &mut [f64],
    ) {
        let cells = self.bins.pow(ensemble.len() as u32);
        // Positions within the ensemble of objects that share a categorical channel.
        let mut channels = BTreeMap::<usize, Vec<usize>>::new();
        for (position, &variable) in ensemble.iter().enumerate() {
            if let VariableSpec::Object { map, .. } = self.specs[variable as usize] {
                channels.entry(map).or_default().push(position);
            }
        }
        let compositional = channels
            .into_values()
            .filter(|positions| positions.len() > 1)
            .collect::<Vec<_>>();
        for (census, distribution) in censuses
            .iter()
            .zip(scratch.distributions.chunks_exact_mut(scratch.cells))
        {
            smoothed_distribution(
                census,
                ensemble,
                self.bins,
                &compositional,
                &mut scratch.histogram,
                &mut distribution[..cells],
            );
        }
        let distribution = |specimen: usize| {
            &scratch.distributions[specimen * scratch.cells..specimen * scratch.cells + cells]
        };
        for (divergence, &(first, second)) in scratch.weights.iter_mut().zip(pairs) {
            *divergence = symmetric_kl(distribution(first), distribution(second));
        }

        // R: similarity 1 - divergence / max(divergence), self-similarity 1. With every
        // divergence zero R divides by zero; identical distributions are fully similar here.
        let maximum = scratch.weights.iter().copied().fold(0.0, f64::max);
        for weight in &mut scratch.weights {
            *weight = if maximum > 0.0 {
                1.0 - *weight / maximum
            } else {
                1.0
            };
        }
        let total = censuses.len() as f64 + 2.0 * scratch.weights.iter().sum::<f64>();
        let bootstraps = self.bootstraps;
        for (factor, (_, membership)) in self.factors.iter().enumerate() {
            output[factor * 4] = modularity(
                &scratch.weights,
                pairs,
                membership,
                total,
                &mut scratch.inside,
                &mut scratch.strength,
            );
        }
        for bootstrap in 0..bootstraps {
            scratch.shuffled.copy_from_slice(&scratch.weights);
            deterministic_shuffle(
                &mut scratch.shuffled,
                derive_seed(seed, SeedEndpoint::SpaceTransNull, bootstrap),
            );
            for (factor, (_, membership)) in self.factors.iter().enumerate() {
                scratch.null[factor * bootstraps + bootstrap] = modularity(
                    &scratch.shuffled,
                    pairs,
                    membership,
                    total,
                    &mut scratch.inside,
                    &mut scratch.strength,
                );
            }
        }
        for (factor, slot) in output.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let null = &scratch.null[factor * bootstraps..(factor + 1) * bootstraps];
            let observed = slot[0];
            let first = null[0];
            let (mean, sd) = if null.iter().all(|value| *value == first) {
                (first, 0.0)
            } else {
                let mean = null.iter().sum::<f64>() / bootstraps as f64;
                let squares = null.iter().map(|value| (value - mean).powi(2)).sum::<f64>();
                (mean, (squares / (bootstraps - 1) as f64).sqrt())
            };
            slot[1] = mean;
            slot[2] = sd;
            slot[3] = null.iter().filter(|value| **value >= observed).count() as f64;
        }
    }
}

/// Per-worker buffers for one ensemble at a time.
struct Scratch {
    cells: usize,
    histogram: Vec<u32>,
    /// One smoothed distribution per specimen, `cells` apart.
    distributions: Vec<f64>,
    /// Pairwise divergences, then similarities.
    weights: Vec<f64>,
    shuffled: Vec<f64>,
    /// Factor-major modularities of the shuffled networks.
    null: Vec<f64>,
    inside: Vec<f64>,
    strength: Vec<f64>,
}

impl Scratch {
    fn new(specimens: usize, cells: usize, pairs: usize, null: usize) -> Self {
        Self {
            cells,
            histogram: Vec::with_capacity(cells),
            distributions: vec![0.0; specimens * cells],
            weights: vec![0.0; pairs],
            shuffled: vec![0.0; pairs],
            null: vec![0.0; null],
            inside: Vec::new(),
            strength: Vec::new(),
        }
    }
}

/// R `build_dist` then `smooth_dist`: the ensemble's joint frequencies over every grid cell
/// (first variable varying fastest, as `expand.grid`), with impossible compositions removed
/// and unseen cells given Chao–Jost missing mass. `NaN` marks removed cells, R's `NA`.
fn smoothed_distribution(
    census: &RoundedCensus,
    ensemble: &[u16],
    bins: usize,
    compositional: &[Vec<usize>],
    histogram: &mut Vec<u32>,
    output: &mut [f64],
) {
    let discs = census.disc_count;
    histogram.clear();
    histogram.resize(output.len(), 0);
    for disc in 0..discs {
        let mut code = 0;
        let mut stride = 1;
        for &variable in ensemble {
            code += census.bins[variable as usize * discs + disc] as usize * stride;
            stride *= bins;
        }
        histogram[code] += 1;
    }

    // A constant variable spans `bins` identical grid values, so its single observed value
    // fills every cell along that axis (R's join matches each copy).
    let mut indices = vec![0; ensemble.len()];
    for (cell, slot) in output.iter_mut().enumerate() {
        let mut remainder = cell;
        let mut source = 0;
        let mut stride = 1;
        for (position, &variable) in ensemble.iter().enumerate() {
            let index = remainder % bins;
            remainder /= bins;
            indices[position] = index;
            if !census.constant[variable as usize] {
                source += index * stride;
            }
            stride *= bins;
        }
        let impossible = compositional.iter().any(|positions| {
            positions
                .iter()
                .map(|&position| census.points[ensemble[position] as usize][indices[position]])
                .sum::<f64>()
                > 100.0
        });
        *slot = if impossible {
            f64::NAN
        } else {
            histogram[source] as f64
        };
    }
    chao_jost(output, histogram, discs);
}

/// Mean of the two Kullback–Leibler divergences in bits over cells both distributions keep
/// (R `rowSums(d1 * log2(d1 / d2), na.rm = TRUE)`, both directions, averaged).
fn symmetric_kl(first: &[f64], second: &[f64]) -> f64 {
    let (mut forward, mut backward) = (0.0, 0.0);
    for (&p, &q) in first.iter().zip(second) {
        if p.is_nan() || q.is_nan() {
            continue;
        }
        forward += p * (p / q).log2();
        backward += q * (q / p).log2();
    }
    (forward + backward) / 2.0
}

/// igraph's directed weighted modularity (resolution 1) of `membership` on the complete
/// similarity graph: every pair in both directions with its weight, and a self-loop of
/// weight 1 at each specimen. `total` is the sum of all those weights.
fn modularity(
    weights: &[f64],
    pairs: &[(usize, usize)],
    membership: &[u32],
    total: f64,
    inside: &mut Vec<f64>,
    strength: &mut Vec<f64>,
) -> f64 {
    let levels = membership
        .iter()
        .max()
        .map_or(0, |level| *level as usize + 1);
    inside.clear();
    inside.resize(levels, 0.0);
    strength.clear();
    strength.resize(levels, 0.0);
    for &level in membership {
        inside[level as usize] += 1.0;
        strength[level as usize] += 1.0;
    }
    for (&weight, &(first, second)) in weights.iter().zip(pairs) {
        let (a, b) = (membership[first] as usize, membership[second] as usize);
        strength[a] += weight;
        strength[b] += weight;
        if a == b {
            inside[a] += 2.0 * weight;
        }
    }
    inside
        .iter()
        .zip(strength.iter())
        .map(|(inside, strength)| inside / total - (strength / total).powi(2))
        .sum()
}

fn summarize(factor: &str, values: &[f64; 4], bootstraps: usize) -> SpaceTransStatistic {
    let [trans_mi, null_mean, null_sd, extreme] = *values;
    let (z_score, status) = if null_sd == 0.0 {
        (0.0, SpaceNullStatus::DegenerateNull)
    } else {
        ((trans_mi - null_mean) / null_sd, SpaceNullStatus::Available)
    };
    let p_value_normal = super::normal::upper_tail(z_score);
    SpaceTransStatistic {
        factor: factor.to_owned(),
        trans_mi,
        null_mean,
        null_sd,
        z_score,
        status,
        p_value_normal,
        p_adjust_bh: p_value_normal,
        p_value_monte_carlo: (1.0 + extreme) / (1 + bootstraps) as f64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn census(bins: Vec<u8>, points: Vec<Vec<f64>>, constant: Vec<bool>) -> RoundedCensus {
        RoundedCensus {
            disc_count: bins.len() / points.len(),
            bins,
            points,
            constant,
        }
    }

    #[test]
    fn smoothing_follows_r_smooth_dist() {
        // R: build_dist then smooth_dist(full_dist = FALSE) on a 2-bin census of 7 discs with
        // frequencies 4, 2, 1 over grid cells (0,0), (1,0), (0,1) and none at (1,1).
        let rounded = census(
            vec![0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1],
            vec![vec![0.0, 10.0], vec![0.0, 10.0]],
            vec![false, false],
        );
        let mut histogram = Vec::new();
        let mut output = vec![0.0; 4];
        smoothed_distribution(&rounded, &[0, 1], 2, &[], &mut histogram, &mut output);
        // num_obs = 8, singletons 1, num_doub = 2; missing = 1/8 * 7/(7 + 4).
        let missing = 1.0 / 8.0 * (7.0 / 11.0);
        let expected = [
            (4.0 / 7.0) * (1.0 - missing),
            (2.0 / 7.0) * (1.0 - missing),
            (1.0 / 7.0) * (1.0 - missing),
            missing,
        ];
        assert_eq!(output, expected);
        assert!((output.iter().sum::<f64>() - 1.0).abs() < 1e-15);

        // Two object percentages of one channel: cells summing above 100 are removed, and a
        // constant variable's counts fill its whole axis.
        let rounded = census(
            vec![0, 1, 1, 0, 0, 0],
            vec![vec![0.0, 100.0], vec![40.0, 40.0]],
            vec![false, true],
        );
        smoothed_distribution(
            &rounded,
            &[0, 1],
            2,
            &[vec![0, 1]],
            &mut histogram,
            &mut output,
        );
        // Cells (0,0), (1,0), (0,1), (1,1) with sums 40, 140, 40, 140; counts 1, 2, 1, 2.
        assert!(output[1].is_nan() && output[3].is_nan());
        assert_eq!(output[0], 0.5);
        assert_eq!(output[2], 0.5);
    }

    #[test]
    fn modularity_matches_igraph_directed_weighted() {
        // R igraph 2.3.0: W <- 1 - D / max(D) with unit diagonal for
        // D[1,2] = 1, D[1,3] = 4, D[2,3] = 2;
        // modularity(graph_from_adjacency_matrix(W, weighted = TRUE), c(1, 1, 2),
        //            weights = E(g)$weight) = 0.21487603305785127.
        let weights = [1.0 - 1.0 / 4.0, 0.0, 1.0 - 2.0 / 4.0];
        let pairs = [(0, 1), (0, 2), (1, 2)];
        let total = 3.0 + 2.0 * weights.iter().sum::<f64>();
        let (mut inside, mut strength) = (Vec::new(), Vec::new());
        let observed = modularity(
            &weights,
            &pairs,
            &[0, 0, 1],
            total,
            &mut inside,
            &mut strength,
        );
        assert!((observed - 0.21487603305785127).abs() < 1e-15, "{observed}");
    }
}
