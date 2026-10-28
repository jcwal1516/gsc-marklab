//! Disc census of a cell table (R `census_table` and `summarize_patches`) and its randomized
//! counterpart (R `random_census`).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::input::{Specimen, VariableSpec, NO_CLASS};
use super::{
    invalid, resource, SpaceError, SpaceInput, SpaceVariable, SPACE_FORMAT_VERSION,
    SPACE_REFERENCE_COMMIT,
};
use crate::common::seeds::{derive_seed, SeedEndpoint};
use crate::permutation::labels::deterministic_shuffle;

pub const SPACE_CENSUS_FORMAT: &str = "marklab.space_census";

/// Discs drawn on one specimen at one radius; members include the centre cell.
pub(crate) struct Discs {
    pub seeds: Vec<usize>,
    pub offsets: Vec<usize>,
    pub members: Vec<u32>,
}

impl Discs {
    pub(crate) fn len(&self) -> usize {
        self.seeds.len()
    }
}

/// Bytes held per disc member and per disc while a census is computed, known before the discs
/// are drawn: the discs, the census plan and `scratches` scratch buffers.
pub(crate) struct CensusCost {
    per_member: usize,
    per_disc: usize,
}

impl CensusCost {
    pub(crate) fn new(variables: &[VariableSpec], scratches: usize) -> Self {
        let maps = variables
            .iter()
            .filter_map(|spec| match *spec {
                VariableSpec::Object { map, .. } | VariableSpec::Linked { map, .. } => Some(map),
                VariableSpec::Independent { .. } => None,
            })
            .collect::<BTreeSet<_>>()
            .len();
        let independents = variables
            .iter()
            .filter(|spec| matches!(spec, VariableSpec::Independent { .. }))
            .count();
        // Disc members and map-layer rows grow by doubling, so they count twice; the plan
        // copies every member once more, and each scratch holds every shuffled layer.
        let rows = 2 + 2 * maps + 1 + scratches.saturating_mul(maps + independents);
        Self {
            per_member: rows.saturating_mul(size_of::<u32>()),
            // Seed and disc offset, then one offset per map layer and one for all members.
            per_disc: (maps + 3) * size_of::<usize>(),
        }
    }

    pub(crate) fn bytes(&self, discs: usize, members: usize) -> usize {
        self.per_disc
            .saturating_mul(discs + 1)
            .saturating_add(self.per_member.saturating_mul(members))
    }

    /// The most member rows `discs` discs may hold within `available` bytes and the
    /// disc-member limit.
    pub(crate) fn member_cap(&self, discs: usize, available: usize, limit: usize) -> usize {
        (available.saturating_sub(self.bytes(discs, 0)) / self.per_member).min(limit)
    }
}

/// Fail when an estimate of the bytes held at once exceeds the retained-memory limit.
pub(crate) fn check_retained(input: &SpaceInput, bytes: usize) -> Result<(), SpaceError> {
    if bytes > input.limits.maximum_retained_bytes {
        return Err(resource(format!(
            "retained-memory estimate {bytes} exceeds the maximum of {}",
            input.limits.maximum_retained_bytes
        )));
    }
    Ok(())
}

/// The number of discs [`draw_seeds`] draws, known before drawing them.
pub(crate) fn disc_count(
    input: &SpaceInput,
    specimen_position: usize,
    radius_position: usize,
) -> usize {
    match &input.specimens[specimen_position].explicit_seeds {
        Some(explicit) => explicit[radius_position].len(),
        None => input
            .census
            .sample_size
            .as_ref()
            .expect("validated: sample_size accompanies sampled specimens")[radius_position],
    }
}

/// Disc centres at one radius: explicit cells, or a seeded sample without replacement.
pub(crate) fn draw_seeds(
    input: &SpaceInput,
    specimen_position: usize,
    radius_position: usize,
) -> Result<Vec<usize>, SpaceError> {
    let specimen = &input.specimens[specimen_position];
    if let Some(explicit) = &specimen.explicit_seeds {
        return Ok(explicit[radius_position].clone());
    }
    let size = input
        .census
        .sample_size
        .as_ref()
        .expect("validated: sample_size accompanies sampled specimens")[radius_position];
    let mut eligible = eligible_seeds(input, specimen);
    if size > eligible.len() {
        return Err(invalid(format!(
            "specimen {} has {} eligible disc centres, fewer than the {size} requested at {} um",
            specimen.id,
            eligible.len(),
            input.census.radii_um[radius_position]
        )));
    }
    let stream = derive_seed(
        input.census.seed,
        SeedEndpoint::SpaceCensus,
        specimen_position,
    );
    deterministic_shuffle(
        &mut eligible,
        derive_seed(stream, SeedEndpoint::SpaceCensus, radius_position),
    );
    // The candidates are proportional to the specimen already loaded; only the sample is kept.
    eligible.truncate(size);
    eligible.shrink_to_fit();
    Ok(eligible)
}

fn eligible_seeds(input: &SpaceInput, specimen: &Specimen) -> Vec<usize> {
    let rows = specimen.cell_ids.len();
    match &input.census.seed_classes {
        None if input.object_channels.is_empty() => (0..rows).collect(),
        None => (0..rows)
            .filter(|&row| specimen.classes.iter().any(|map| map[row] != NO_CLASS))
            .collect(),
        Some(seed_classes) => {
            let accepted = input
                .object_channels
                .iter()
                .map(|channel| {
                    let mut accept = vec![false; channel.classes.len()];
                    for class in seed_classes.get(&channel.id).into_iter().flatten() {
                        let position = channel
                            .classes
                            .iter()
                            .position(|known| known == class)
                            .expect("validated seed class");
                        accept[position] = true;
                    }
                    accept
                })
                .collect::<Vec<_>>();
            (0..rows)
                .filter(|&row| {
                    specimen
                        .classes
                        .iter()
                        .zip(&accepted)
                        .any(|(map, accept)| map[row] != NO_CLASS && accept[map[row] as usize])
                })
                .collect()
        }
    }
}

/// Every cell within `radius_um` (inclusive) of each centre, the centre included, as R's
/// `FNN::knnx` query of the table against itself.
pub(crate) fn draw_discs(
    specimen: &Specimen,
    seeds: Vec<usize>,
    radius_um: f64,
    maximum_members: usize,
) -> Result<Discs, SpaceError> {
    let mut offsets = Vec::with_capacity(seeds.len() + 1);
    let mut members = Vec::new();
    offsets.push(0);
    for &seed in &seeds {
        let start = members.len();
        // Members beyond the cap are never stored, so a crowded disc cannot outgrow it.
        let mut exceeded = false;
        let mut push = |row: u32| {
            if members.len() < maximum_members {
                members.push(row);
            } else {
                exceeded = true;
            }
        };
        push(seed as u32);
        specimen
            .index
            .visit_within_radius(seed, radius_um, |neighbor| push(neighbor.index as u32))
            .map_err(|error| invalid(format!("disc query failed: {error}")))?;
        if exceeded {
            return Err(resource(format!(
                "specimen {} has more than the {maximum_members} disc members at {radius_um} um that the disc-member and retained-memory limits allow",
                specimen.id
            )));
        }
        members[start..].sort_unstable();
        offsets.push(members.len());
    }
    Ok(Discs {
        seeds,
        offsets,
        members,
    })
}

/// How the variables of one census are computed from disc member rows.
///
/// Each object channel and each independent scalar is a layer whose member rows a
/// randomized census shuffles independently, as R shuffles each object map and each
/// independent scalar set; linked scalars move with their cell.
pub(crate) struct CensusPlan {
    variable_count: usize,
    disc_count: usize,
    maps: Vec<MapLayer>,
    independents: Vec<IndependentLayer>,
    all_offsets: Vec<usize>,
    all_rows: Vec<u32>,
}

struct MapLayer {
    map: usize,
    class_count: usize,
    offsets: Vec<usize>,
    rows: Vec<u32>,
    objects: Vec<(u32, usize)>,
    /// Per class, the linked scalars as (scalar, output variable).
    linked: Vec<Vec<(usize, usize)>>,
    linked_variables: Vec<(u32, usize)>,
}

struct IndependentLayer {
    scalar: usize,
    variable: usize,
}

/// Reusable buffers for computing one census.
pub(crate) struct CensusScratch {
    map_rows: Vec<Vec<u32>>,
    independent_rows: Vec<Vec<u32>>,
    counts: Vec<u32>,
    sums: Vec<f64>,
}

impl CensusPlan {
    pub(crate) fn new(specimen: &Specimen, discs: &Discs, variables: &[VariableSpec]) -> Self {
        let mut maps = Vec::<MapLayer>::new();
        let mut independents = Vec::new();
        for (variable, spec) in variables.iter().enumerate() {
            match *spec {
                VariableSpec::Object { map, class } => {
                    layer(&mut maps, specimen, discs, map)
                        .objects
                        .push((class, variable));
                }
                VariableSpec::Linked { map, class, scalar } => {
                    let layer = layer(&mut maps, specimen, discs, map);
                    layer.linked[class as usize].push((scalar, variable));
                    layer.linked_variables.push((class, variable));
                }
                VariableSpec::Independent { scalar } => {
                    independents.push(IndependentLayer { scalar, variable });
                }
            }
        }
        Self {
            variable_count: variables.len(),
            disc_count: discs.len(),
            maps,
            independents,
            all_offsets: discs.offsets.clone(),
            all_rows: discs.members.clone(),
        }
    }

    pub(crate) fn scratch(&self) -> CensusScratch {
        CensusScratch {
            map_rows: self.maps.iter().map(|layer| layer.rows.clone()).collect(),
            independent_rows: self
                .independents
                .iter()
                .map(|_| self.all_rows.clone())
                .collect(),
            counts: Vec::new(),
            sums: Vec::new(),
        }
    }

    /// The observed census, variable-major: `values[variable * discs + disc]`.
    pub(crate) fn observed(&self, specimen: &Specimen, scratch: &mut CensusScratch) -> Vec<f64> {
        for (rows, layer) in scratch.map_rows.iter_mut().zip(&self.maps) {
            rows.copy_from_slice(&layer.rows);
        }
        for rows in &mut scratch.independent_rows {
            rows.copy_from_slice(&self.all_rows);
        }
        let mut values = vec![0.0; self.variable_count * self.disc_count];
        self.summarize(specimen, scratch, &mut values);
        values
    }

    /// One randomized census: every layer's member rows shuffled across discs and re-split at
    /// the original disc sizes.
    pub(crate) fn randomized(
        &self,
        specimen: &Specimen,
        seed: u64,
        scratch: &mut CensusScratch,
        values: &mut [f64],
    ) {
        for (layer_index, (rows, layer)) in scratch.map_rows.iter_mut().zip(&self.maps).enumerate()
        {
            rows.copy_from_slice(&layer.rows);
            deterministic_shuffle(
                rows,
                derive_seed(seed, SeedEndpoint::SpaceCisNull, layer_index),
            );
        }
        for (position, rows) in scratch.independent_rows.iter_mut().enumerate() {
            rows.copy_from_slice(&self.all_rows);
            deterministic_shuffle(
                rows,
                derive_seed(seed, SeedEndpoint::SpaceCisNull, self.maps.len() + position),
            );
        }
        self.summarize(specimen, scratch, values);
    }

    fn summarize(&self, specimen: &Specimen, scratch: &mut CensusScratch, values: &mut [f64]) {
        let discs = self.disc_count;
        let CensusScratch {
            map_rows,
            independent_rows,
            counts,
            sums,
        } = scratch;
        for (layer, rows) in self.maps.iter().zip(map_rows.iter()) {
            let classes = &specimen.classes[layer.map];
            sums.resize(self.variable_count, 0.0);
            for disc in 0..discs {
                counts.clear();
                counts.resize(layer.class_count, 0);
                for &(_, variable) in &layer.linked_variables {
                    sums[variable] = 0.0;
                }
                let members = &rows[layer.offsets[disc]..layer.offsets[disc + 1]];
                for &row in members {
                    let class = classes[row as usize] as usize;
                    counts[class] += 1;
                    for &(scalar, variable) in &layer.linked[class] {
                        sums[variable] += specimen.scalars[scalar][row as usize];
                    }
                }
                let total = members.len();
                for &(class, variable) in &layer.objects {
                    values[variable * discs + disc] = if total == 0 {
                        0.0
                    } else {
                        100.0 * (counts[class as usize] as f64 / total as f64)
                    };
                }
                for &(class, variable) in &layer.linked_variables {
                    let count = counts[class as usize];
                    values[variable * discs + disc] = if count == 0 {
                        0.0
                    } else {
                        sums[variable] / count as f64
                    };
                }
            }
        }
        for (layer, rows) in self.independents.iter().zip(independent_rows.iter()) {
            let scalar = &specimen.scalars[layer.scalar];
            for disc in 0..discs {
                let members = &rows[self.all_offsets[disc]..self.all_offsets[disc + 1]];
                let sum = members.iter().map(|&row| scalar[row as usize]).sum::<f64>();
                values[layer.variable * discs + disc] = sum / members.len() as f64;
            }
        }
    }
}

fn layer<'a>(
    maps: &'a mut Vec<MapLayer>,
    specimen: &Specimen,
    discs: &Discs,
    map: usize,
) -> &'a mut MapLayer {
    if let Some(position) = maps.iter().position(|layer| layer.map == map) {
        return &mut maps[position];
    }
    let classes = &specimen.classes[map];
    let mut offsets = Vec::with_capacity(discs.len() + 1);
    let mut rows = Vec::new();
    offsets.push(0);
    for disc in 0..discs.len() {
        rows.extend(
            discs.members[discs.offsets[disc]..discs.offsets[disc + 1]]
                .iter()
                .filter(|&&row| classes[row as usize] != NO_CLASS),
        );
        offsets.push(rows.len());
    }
    let class_count = specimen.class_counts[map].len();
    maps.push(MapLayer {
        map,
        class_count,
        offsets,
        rows,
        objects: Vec::new(),
        linked: vec![Vec::new(); class_count],
        linked_variables: Vec::new(),
    });
    maps.last_mut().expect("just pushed")
}

/// Variables a specimen contains, as (position in the declared list, spec).
pub(crate) fn present_variables(
    input: &SpaceInput,
    specimen: &Specimen,
) -> Vec<(usize, VariableSpec)> {
    input
        .variables
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, spec)| specimen.has(*spec))
        .collect()
}

/// Census of every specimen at every radius (R `census_table`).
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceCensusResult {
    pub format: String,
    pub version: u32,
    pub reference_commit: String,
    pub source_kind: String,
    pub seed: u64,
    pub radii_um: Vec<f64>,
    pub variables: Vec<SpaceVariable>,
    pub specimens: Vec<SpaceCensusSpecimen>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceCensusSpecimen {
    pub specimen_id: String,
    pub groups: BTreeMap<String, String>,
    pub cell_count: usize,
    /// Names of the variables present in this specimen; disc values follow this order.
    pub variables: Vec<String>,
    pub radii: Vec<SpaceCensusRadius>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceCensusRadius {
    pub radius_um: f64,
    pub discs: Vec<SpaceCensusNeighborhood>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceCensusNeighborhood {
    pub seed_cell_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub member_count: usize,
    pub values: Vec<f64>,
}

/// Draw and summarize the discs of every specimen at every radius.
pub fn space_census(input: &SpaceInput) -> Result<SpaceCensusResult, SpaceError> {
    let mut specimens = Vec::with_capacity(input.specimens.len());
    // Bytes of the disc rows produced so far, which the result keeps.
    let mut retained = 0_usize;
    for (position, specimen) in input.specimens.iter().enumerate() {
        let present = present_variables(input, specimen);
        let specs = present.iter().map(|(_, spec)| *spec).collect::<Vec<_>>();
        let cost = CensusCost::new(&specs, 1);
        let mut radii = Vec::with_capacity(input.census.radii_um.len());
        for (radius_position, &radius_um) in input.census.radii_um.iter().enumerate() {
            let seeds = draw_seeds(input, position, radius_position)?;
            let count = seeds.len();
            let values_bytes = specs.len().saturating_mul(count).saturating_mul(8);
            let rows_bytes = seeds
                .iter()
                .map(|&seed| specimen.cell_ids[seed].len())
                .sum::<usize>()
                .saturating_add(count.saturating_mul(size_of::<SpaceCensusNeighborhood>()))
                .saturating_add(values_bytes);
            retained = retained.saturating_add(rows_bytes);
            // The rows so far and this radius's values; the discs and plan get what remains.
            let fixed = retained.saturating_add(values_bytes);
            check_retained(input, fixed.saturating_add(cost.bytes(count, 0)))?;
            let discs = draw_discs(
                specimen,
                seeds,
                radius_um,
                cost.member_cap(
                    count,
                    input.limits.maximum_retained_bytes - fixed,
                    input.limits.maximum_disc_members,
                ),
            )?;
            let plan = CensusPlan::new(specimen, &discs, &specs);
            let values = plan.observed(specimen, &mut plan.scratch());
            let rows = (0..count)
                .map(|disc| SpaceCensusNeighborhood {
                    seed_cell_id: specimen.cell_ids[discs.seeds[disc]].clone(),
                    x_um: specimen.x_um[discs.seeds[disc]],
                    y_um: specimen.y_um[discs.seeds[disc]],
                    member_count: discs.offsets[disc + 1] - discs.offsets[disc],
                    values: (0..specs.len())
                        .map(|variable| values[variable * count + disc])
                        .collect(),
                })
                .collect();
            radii.push(SpaceCensusRadius {
                radius_um,
                discs: rows,
            });
        }
        specimens.push(SpaceCensusSpecimen {
            specimen_id: specimen.id.clone(),
            groups: specimen.groups.clone(),
            cell_count: specimen.cell_ids.len(),
            variables: present
                .iter()
                .map(|(index, _)| input.variable_descriptions[*index].name.clone())
                .collect(),
            radii,
        });
    }
    Ok(SpaceCensusResult {
        format: SPACE_CENSUS_FORMAT.into(),
        version: SPACE_FORMAT_VERSION,
        reference_commit: SPACE_REFERENCE_COMMIT.into(),
        source_kind: input.source_kind.into(),
        seed: input.census.seed,
        radii_um: input.census.radii_um.clone(),
        variables: input.variable_descriptions.clone(),
        specimens,
    })
}
