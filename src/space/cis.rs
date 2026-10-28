//! cisMI: non-random co-patterning of every ensemble within one specimen (R `measure_cisMI`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::binning::{bin_count, min_max, minimum_discs_for_two_bins, NearestGrid};
use super::census::{
    check_retained, disc_count, draw_discs, draw_seeds, present_variables, CensusCost, CensusPlan,
};
use super::ensembles::{selected_radii, selected_specimens, EnsembleOptions, Ensembles};
use super::information::{co_information, joint_entropy, lexicographic_rank};
use super::input::{Specimen, VariableSpec};
use super::{
    for_each_chunk, invalid, resource, worker_threads, SpaceError, SpaceInput, SpaceVariable,
    SPACE_FORMAT_VERSION, SPACE_REFERENCE_COMMIT,
};
use crate::common::seeds::{derive_seed, SeedEndpoint};
use crate::inference::multiple_testing::benjamini_hochberg;

pub const SPACE_CIS_FORMAT: &str = "marklab.space_cis_result";

/// The arguments of R `measure_cisMI`, with SPACE's defaults. `depth` has no default in R;
/// 3 is the value the SPACE tutorial uses.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceCisParameters {
    /// Largest ensemble size.
    pub depth: usize,
    /// Randomized censuses per radius; at least two.
    pub bootstraps: usize,
    /// Variables every largest ensemble must contain.
    pub all: Vec<String>,
    /// Variables of which every largest ensemble must contain at least one.
    pub alo: Vec<String>,
    /// Variables excluded from every ensemble.
    pub not: Vec<String>,
    pub max_bins: usize,
    /// Census radii to analyse; empty for all.
    pub radii_um: Vec<f64>,
    /// Specimens to analyse; empty for all.
    pub specimens: Vec<String>,
}

impl SpaceCisParameters {
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

impl Default for SpaceCisParameters {
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
        }
    }
}

/// Whether the randomized censuses varied for an ensemble.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpaceNullStatus {
    Available,
    /// Every randomized census gave the same co-information. R reports Z = 0 and P = 1 when
    /// the observed value matches it and Z = ±Inf and P = 0 otherwise; Marklab keeps those
    /// P values and reports no Z for the second case.
    DegenerateNull,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceCisEnsemble {
    pub variables: Vec<String>,
    /// Plug-in co-information of the observed census, in bits (entropy at depth one).
    pub observed_mi: f64,
    pub null_mean_mi: f64,
    /// Sample standard deviation of the randomized co-information.
    pub null_sd_mi: f64,
    /// `observed_mi - null_mean_mi` (R `CisMI`).
    pub cis_mi: f64,
    pub z_score: Option<f64>,
    pub status: SpaceNullStatus,
    /// SPACE's normal approximation `2 * Phi(-|Z|)` (R `Pvalue`).
    pub p_value_normal: f64,
    /// Benjamini–Hochberg adjustment of `p_value_normal` over every ensemble at the radius
    /// (R `Padjust`).
    pub p_adjust_bh: f64,
    /// `(1 + #{b: |null_b - mean| >= |cis_mi|}) / (1 + bootstraps)` from the same draws.
    pub p_value_monte_carlo: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceCisRadius {
    pub radius_um: f64,
    pub disc_count: usize,
    /// Per analysed variable, the `[minimum, maximum]` over the observed and every randomized
    /// census: the span of its `bins` equally spaced grid points.
    pub variable_ranges: Vec<[f64; 2]>,
    pub ensembles: Vec<SpaceCisEnsemble>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceCisSpecimen {
    pub specimen_id: String,
    pub groups: BTreeMap<String, String>,
    pub cell_count: usize,
    /// Variables analysed: those present in the specimen, after `not` and the implicit
    /// exclusion when `all` and `alo` fill the depth.
    pub variables: Vec<String>,
    /// Bins per variable, from the smallest disc count over the analysed radii.
    pub bins: usize,
    pub minimum_disc_count: usize,
    pub radii: Vec<SpaceCisRadius>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceCisResult {
    pub format: String,
    pub version: u32,
    pub reference_commit: String,
    pub source_kind: String,
    pub seed: u64,
    pub parameters: SpaceCisParameters,
    pub variables: Vec<SpaceVariable>,
    pub specimens: Vec<SpaceCisSpecimen>,
}

/// Rank every ensemble of up to `depth` variables in each specimen by cisMI.
pub fn space_cis(
    input: &SpaceInput,
    parameters: &SpaceCisParameters,
) -> Result<SpaceCisResult, SpaceError> {
    parameters.options().validate(input)?;
    let radius_positions = selected_radii(input, &parameters.radii_um)?;
    let mut specimens = Vec::new();
    // Estimated bytes of the ensembles reported so far, which the result keeps.
    let mut retained = 0_usize;
    for position in selected_specimens(input, &parameters.specimens)? {
        specimens.push(analyze_specimen(
            input,
            parameters,
            position,
            &input.specimens[position],
            &radius_positions,
            &mut retained,
        )?);
    }
    Ok(SpaceCisResult {
        format: SPACE_CIS_FORMAT.into(),
        version: SPACE_FORMAT_VERSION,
        reference_commit: SPACE_REFERENCE_COMMIT.into(),
        source_kind: input.source_kind.into(),
        seed: input.census.seed,
        parameters: parameters.clone(),
        variables: input.variable_descriptions.clone(),
        specimens,
    })
}

fn analyze_specimen(
    input: &SpaceInput,
    parameters: &SpaceCisParameters,
    position: usize,
    specimen: &Specimen,
    radius_positions: &[usize],
    retained: &mut usize,
) -> Result<SpaceCisSpecimen, SpaceError> {
    let names = |index: usize| input.variable_descriptions[index].name.as_str();
    let options = parameters.options();
    let active = options.active(input, present_variables(input, specimen));
    let variable_count = active.len();
    if parameters.depth > variable_count {
        return Err(invalid(format!(
            "depth {} exceeds the {variable_count} variables analysed in specimen {}",
            parameters.depth, specimen.id
        )));
    }
    let specs = active.iter().map(|(_, spec)| *spec).collect::<Vec<_>>();
    let active_names = active
        .iter()
        .map(|(index, _)| names(*index).to_owned())
        .collect::<Vec<_>>();

    let minimum_disc_count = radius_positions
        .iter()
        .map(|&radius_position| disc_count(input, position, radius_position))
        .min()
        .expect("at least one radius");
    let bins = bin_count(minimum_disc_count, parameters.depth, parameters.max_bins);
    if bins < 2 {
        return Err(invalid(format!(
            "specimen {} has {minimum_disc_count} discs at its smallest radius sample; depth {} needs at least {} for two bins per variable",
            specimen.id,
            parameters.depth,
            minimum_discs_for_two_bins(parameters.depth)
        )));
    }
    let ensembles = Ensembles::new(variable_count, &options, &active_names, input, *retained)?;

    let result_bytes = ensembles.result_bytes(&active_names, size_of::<SpaceCisEnsemble>());
    let mut radii = Vec::with_capacity(radius_positions.len());
    for &radius_position in radius_positions {
        // Discs are drawn one radius at a time so only one radius's members are held.
        let seeds = draw_seeds(input, position, radius_position)?;
        let disc_count = seeds.len();
        let stream = derive_seed(input.census.seed, SeedEndpoint::SpaceCisNull, position);
        let radius_seed = derive_seed(stream, SeedEndpoint::SpaceCisNull, radius_position);
        let (variable_ranges, ensembles) = analyze_radius(
            input,
            parameters,
            specimen,
            &specs,
            seeds,
            input.census.radii_um[radius_position],
            bins,
            radius_seed,
            &ensembles,
            &active_names,
            retained.saturating_add(result_bytes),
        )?;
        *retained = retained.saturating_add(result_bytes);
        radii.push(SpaceCisRadius {
            radius_um: input.census.radii_um[radius_position],
            disc_count,
            variable_ranges,
            ensembles,
        });
    }
    Ok(SpaceCisSpecimen {
        specimen_id: specimen.id.clone(),
        groups: specimen.groups.clone(),
        cell_count: specimen.cell_ids.len(),
        variables: active_names,
        bins,
        minimum_disc_count,
        radii,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "one radius needs the specimen, its census plan inputs and the shared ensemble table"
)]
fn analyze_radius(
    input: &SpaceInput,
    parameters: &SpaceCisParameters,
    specimen: &Specimen,
    specs: &[VariableSpec],
    seeds: Vec<usize>,
    radius_um: f64,
    bins: usize,
    radius_seed: u64,
    ensembles: &Ensembles,
    names: &[String],
    retained_results: usize,
) -> Result<(Vec<[f64; 2]>, Vec<SpaceCisEnsemble>), SpaceError> {
    let variables = specs.len();
    let disc_count = seeds.len();
    let bootstraps = parameters.bootstraps;
    let censuses = bootstraps + 1;
    let total_ensembles = ensembles.total();
    let visits = (total_ensembles as u64)
        .saturating_mul(censuses as u64)
        .saturating_mul(disc_count as u64);
    if visits > input.limits.maximum_entropy_visits {
        return Err(resource(format!(
            "{visits} joint-entropy cell visits exceed the maximum of {}",
            input.limits.maximum_entropy_visits
        )));
    }
    let threads = worker_threads();
    // Results so far and this radius's, then the peak working set: ensemble tables, binned
    // censuses, entropy and co-information tables, values per worker and ranges. The discs and census plan with
    // a scratch per worker get what remains.
    let fixed = [
        ensembles.table_bytes(),
        censuses
            .saturating_mul(variables)
            .saturating_mul(disc_count),
        total_ensembles
            .saturating_mul(2)
            .saturating_mul(censuses)
            .saturating_mul(8),
        variables
            .saturating_mul(disc_count)
            .saturating_mul(8)
            .saturating_mul(threads + 1),
        bootstraps.saturating_mul(variables).saturating_mul(16),
    ]
    .iter()
    .fold(retained_results, |sum, bytes| sum.saturating_add(*bytes));
    let cost = CensusCost::new(specs, threads + 1);
    check_retained(input, fixed.saturating_add(cost.bytes(disc_count, 0)))?;
    let discs = draw_discs(
        specimen,
        seeds,
        radius_um,
        cost.member_cap(
            disc_count,
            input.limits.maximum_retained_bytes - fixed,
            input.limits.maximum_disc_members,
        ),
    )?;
    let plan = CensusPlan::new(specimen, &discs, specs);

    let bootstrap_seed =
        |bootstrap: usize| derive_seed(radius_seed, SeedEndpoint::SpaceCisNull, bootstrap);
    let observed = plan.observed(specimen, &mut plan.scratch());

    // Pass 1: each variable's range over the observed and every randomized census (R pools
    // them before rounding). Randomized censuses are regenerated in pass 2 from the same
    // seeds instead of being stored.
    let mut ranges = vec![(0.0_f64, 0.0_f64); bootstraps * variables];
    for_each_chunk(
        &mut ranges,
        variables,
        || (plan.scratch(), vec![0.0; variables * disc_count]),
        |(scratch, values), bootstrap, range| {
            plan.randomized(specimen, bootstrap_seed(bootstrap), scratch, values);
            for (variable, slot) in range.iter_mut().enumerate() {
                *slot = min_max(&values[variable * disc_count..(variable + 1) * disc_count]);
            }
        },
    );
    let variable_ranges = (0..variables)
        .map(|variable| {
            let (mut low, mut high) =
                min_max(&observed[variable * disc_count..(variable + 1) * disc_count]);
            for range in ranges.chunks_exact(variables) {
                low = low.min(range[variable].0);
                high = high.max(range[variable].1);
            }
            [low, high]
        })
        .collect::<Vec<_>>();
    drop(ranges);
    let grids = variable_ranges
        .iter()
        .map(|&[low, high]| NearestGrid::new(low, high, bins))
        .collect::<Vec<_>>();

    // Pass 2: binned censuses, census-major then variable-major.
    let stride = variables * disc_count;
    let mut binned = vec![0_u8; censuses * stride];
    let round = |values: &[f64], output: &mut [u8]| {
        for (variable, grid) in grids.iter().enumerate() {
            let source = &values[variable * disc_count..(variable + 1) * disc_count];
            let target = &mut output[variable * disc_count..(variable + 1) * disc_count];
            match grid {
                Some(grid) => {
                    for (bin, value) in target.iter_mut().zip(source) {
                        *bin = grid.nearest(*value);
                    }
                }
                None => target.fill(0),
            }
        }
    };
    round(&observed, &mut binned[..stride]);
    drop(observed);
    for_each_chunk(
        &mut binned[stride..],
        stride,
        || (plan.scratch(), vec![0.0; stride]),
        |(scratch, values), bootstrap, output| {
            plan.randomized(specimen, bootstrap_seed(bootstrap), scratch, values);
            round(values, output);
        },
    );

    // Joint entropy of every ensemble in every census.
    let mut entropies = (1..=ensembles.depth)
        .map(|size| vec![0.0; ensembles.count(size) * censuses])
        .collect::<Vec<_>>();
    for (size_index, table) in entropies.iter_mut().enumerate() {
        let size = size_index + 1;
        for_each_chunk(
            table,
            censuses,
            || (Vec::new(), Vec::with_capacity(size)),
            |(histogram, columns): &mut (Vec<u32>, Vec<&[u8]>), index, row| {
                let ensemble = ensembles.ensemble(size, index);
                for (census, entropy) in row.iter_mut().enumerate() {
                    columns.clear();
                    columns.extend(ensemble.iter().map(|&variable| {
                        let start = census * stride + variable as usize * disc_count;
                        &binned[start..start + disc_count]
                    }));
                    *entropy = joint_entropy(columns, bins, histogram);
                }
            },
        );
    }
    drop(binned);

    // Co-information by inclusion–exclusion, then the null summaries.
    let mut rows = Vec::with_capacity(total_ensembles);
    for size in 1..=ensembles.depth {
        for index in 0..ensembles.count(size) {
            rows.push((size, index));
        }
    }
    let mut information = vec![0.0; total_ensembles * censuses];
    for_each_chunk(
        &mut information,
        censuses,
        || (),
        |(), row, output| {
            let (size, index) = rows[row];
            let ensemble = ensembles.ensemble(size, index);
            co_information(
                ensemble,
                censuses,
                |subset| {
                    let position = if subset.len() == ensembles.depth {
                        index
                    } else {
                        lexicographic_rank(subset, ensembles.variable_count)
                    };
                    let table = &entropies[subset.len() - 1];
                    &table[position * censuses..(position + 1) * censuses]
                },
                output,
            );
        },
    );
    drop(entropies);

    let mut results = rows
        .iter()
        .zip(information.chunks_exact(censuses))
        .map(|(&(size, index), values)| {
            summarize(
                ensembles
                    .ensemble(size, index)
                    .iter()
                    .map(|&variable| names[variable as usize].clone())
                    .collect(),
                values,
            )
        })
        .collect::<Vec<_>>();
    let adjusted = benjamini_hochberg(
        &results
            .iter()
            .map(|result| result.p_value_normal)
            .collect::<Vec<_>>(),
    )
    .map_err(|error| invalid(format!("Benjamini-Hochberg adjustment failed: {error}")))?;
    for (result, adjusted) in results.iter_mut().zip(adjusted) {
        result.p_adjust_bh = adjusted;
    }
    Ok((variable_ranges, results))
}

fn summarize(variables: Vec<String>, values: &[f64]) -> SpaceCisEnsemble {
    let observed = values[0];
    let null = &values[1..];
    let first = null[0];
    let (mean, sd) = if null.iter().all(|value| *value == first) {
        (first, 0.0)
    } else {
        let mean = null.iter().sum::<f64>() / null.len() as f64;
        let squares = null.iter().map(|value| (value - mean).powi(2)).sum::<f64>();
        (mean, (squares / (null.len() - 1) as f64).sqrt())
    };
    let cis_mi = observed - mean;
    let (z_score, status, p_value_normal) = if sd == 0.0 {
        let matches = cis_mi == 0.0;
        (
            matches.then_some(0.0),
            SpaceNullStatus::DegenerateNull,
            if matches { 1.0 } else { 0.0 },
        )
    } else {
        let z = cis_mi / sd;
        (
            Some(z),
            SpaceNullStatus::Available,
            super::normal::two_sided_p(z),
        )
    };
    let extreme = null
        .iter()
        .filter(|value| (*value - mean).abs() >= cis_mi.abs())
        .count();
    SpaceCisEnsemble {
        variables,
        observed_mi: observed,
        null_mean_mi: mean,
        null_sd_mi: sd,
        cis_mi,
        z_score,
        status,
        p_value_normal,
        p_adjust_bh: p_value_normal,
        p_value_monte_carlo: (1 + extreme) as f64 / (1 + null.len()) as f64,
    }
}
