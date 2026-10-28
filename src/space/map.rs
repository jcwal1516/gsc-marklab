//! Cell regions from a learned pattern (R `map_pattern`, for cell tables). Every disc takes the
//! region of the nearest point on the pattern's mean curves and votes for it in each of its
//! cells, weighted by how close the disc lies to the curves; each cell joins the region with
//! the most votes. R votes for the pixels of image discs; here the votes go to disc members.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::census::check_retained;
use super::ensembles::{selected_radii, selected_specimens};
use super::input::read_bounded;
use super::pattern::{
    ensemble_census, ensemble_variables, SpacePatternResult, SPACE_PATTERN_FORMAT,
};
use super::{
    for_each_chunk, invalid, resource, SpaceError, SpaceInput, SPACE_FORMAT_VERSION,
    SPACE_REFERENCE_COMMIT,
};

pub const SPACE_MAP_FORMAT: &str = "marklab.space_map_result";

const MAXIMUM_RESULT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// R's vote for the background, which a cell keeps when no region outvotes it.
const BACKGROUND_VOTE: f64 = 0.000_001;

/// Read a map result file.
pub fn load_space_map_result(path: &Path) -> Result<SpaceMapResult, SpaceError> {
    let bytes = read_bounded(path, MAXIMUM_RESULT_BYTES, "map result")?;
    let result = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("invalid map JSON: {error}")))?;
    check_map(&result)?;
    Ok(result)
}

/// Check a map result read back from a file: regions numbered 1, 2, ... and one region or
/// none per cell.
pub(super) fn check_map(result: &SpaceMapResult) -> Result<(), SpaceError> {
    if result.format != SPACE_MAP_FORMAT || result.version != SPACE_FORMAT_VERSION {
        return Err(invalid(format!(
            "the map must be {SPACE_MAP_FORMAT} version {SPACE_FORMAT_VERSION}"
        )));
    }
    let regions = result.regions.len() as u32;
    let numbered = result
        .regions
        .iter()
        .zip(1..)
        .all(|(region, number)| region.region == number);
    let assigned = result.specimens.iter().all(|specimen| {
        specimen.regions.len() == specimen.cell_ids.len()
            && specimen
                .regions
                .iter()
                .flatten()
                .all(|region| (1..=regions).contains(region))
    });
    if !(numbered && assigned) {
        return Err(invalid(
            "the map needs regions numbered from 1 and one region or none per cell",
        ));
    }
    Ok(())
}

/// Read a pattern result file.
pub fn load_space_pattern_result(path: &Path) -> Result<SpacePatternResult, SpaceError> {
    let bytes = read_bounded(path, MAXIMUM_RESULT_BYTES, "pattern result")?;
    serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("invalid pattern JSON: {error}")))
}

/// The arguments of R `map_pattern` that apply to cell tables.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceMapParameters {
    /// Non-overlapping `[lower, upper]` ranges of path position between 0 and 100, in
    /// increasing order (R `region_bounds`); they are regions 1, 2 and so on.
    pub regions: Vec<[f64; 2]>,
    /// Specimens to map; empty for all.
    pub specimens: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceMapRegion {
    pub region: u32,
    /// The path ranges the region covers.
    pub ranges: Vec<[f64; 2]>,
    /// Whether this is R's catch-all for the gaps between declared regions.
    pub catch_all: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceMapSpecimen {
    pub specimen_id: String,
    pub disc_count: usize,
    pub cell_ids: Vec<String>,
    /// Each cell's region; absent where no disc outvoted R's background vote.
    pub regions: Vec<Option<u32>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceMapResult {
    pub format: String,
    pub version: u32,
    pub reference_commit: String,
    pub source_kind: String,
    pub seed: u64,
    pub parameters: SpaceMapParameters,
    pub variables: Vec<String>,
    pub radius_um: f64,
    pub regions: Vec<SpaceMapRegion>,
    pub specimens: Vec<SpaceMapSpecimen>,
}

/// Assign every cell of the mapped specimens to a region of the pattern's path.
pub fn space_map(
    input: &SpaceInput,
    pattern: &SpacePatternResult,
    parameters: &SpaceMapParameters,
) -> Result<SpaceMapResult, SpaceError> {
    let curve = check_pattern(pattern)?;
    let names = pattern
        .curves
        .iter()
        .map(|curve| curve.variable.clone())
        .collect::<Vec<_>>();
    let variables = ensemble_variables(input, &names)?;
    let dims = variables.len();
    let (lowers, regions) = partition(&parameters.regions)?;
    let radius_position = selected_radii(input, &[pattern.parameters.radius_um])?[0];
    let mapped = selected_specimens(input, &parameters.specimens)?;
    let points = curve.len() / dims;

    // Nearest curve point and its distance for every disc of every mapped specimen.
    let mut censuses = Vec::with_capacity(mapped.len());
    let mut held = curve.len() * 8;
    let mut visits = 0_u64;
    for &position in &mapped {
        let census = ensemble_census(input, position, radius_position, &variables, None, held)?;
        held = held
            .saturating_add(census.values.len() * 8)
            .saturating_add(census.discs.members.len() * 4);
        visits = visits.saturating_add((census.discs.len() * points * dims) as u64);
        if visits > input.limits.maximum_entropy_visits {
            return Err(resource(format!(
                "{visits} curve visits exceed the maximum of {}",
                input.limits.maximum_entropy_visits
            )));
        }
        let mut nearest = vec![(0_usize, 0.0_f64); census.discs.len()];
        let chunk = 256;
        for_each_chunk(
            &mut nearest,
            chunk,
            || (),
            |(), index, slots| {
                for (offset, slot) in slots.iter_mut().enumerate() {
                    let disc = index * chunk + offset;
                    *slot = nearest_point(&curve, &census.values[disc * dims..(disc + 1) * dims]);
                }
            },
        );
        censuses.push((position, census, nearest));
    }
    let farthest = censuses
        .iter()
        .flat_map(|(_, _, nearest)| nearest.iter().map(|(_, distance)| *distance))
        .fold(0.0, f64::max);

    let slots = regions.len();
    let mut specimens = Vec::with_capacity(mapped.len());
    for (position, census, nearest) in censuses {
        let specimen = &input.specimens[position];
        let cells = specimen.cell_ids.len();
        check_retained(
            input,
            held.saturating_add(cells.saturating_mul(slots * 8 + 8)),
        )?;
        let mut votes = vec![0.0; cells * slots];
        for (disc, &(point, distance)) in nearest.iter().enumerate() {
            // R: 1 - (d / max(d) + 1e-4). With every disc on the curves R divides zero by zero;
            // here they all vote fully.
            let weight = if farthest > 0.0 {
                1.0 - (distance / farthest + 0.0001)
            } else {
                1.0 - 0.0001
            };
            let place = pattern.positions[point];
            let region = lowers.iter().filter(|(lower, _)| place >= *lower).count() - 1;
            let slot = lowers[region].1 as usize - 1;
            let members =
                &census.discs.members[census.discs.offsets[disc]..census.discs.offsets[disc + 1]];
            for &cell in members {
                votes[cell as usize * slots + slot] += weight;
            }
        }
        let assigned = votes
            .chunks_exact(slots)
            .map(|cell| {
                let (best, vote) =
                    cell.iter()
                        .enumerate()
                        .fold((0, f64::NEG_INFINITY), |best, (slot, &vote)| {
                            if vote > best.1 {
                                (slot, vote)
                            } else {
                                best
                            }
                        });
                (vote >= BACKGROUND_VOTE).then_some(best as u32 + 1)
            })
            .collect();
        specimens.push(SpaceMapSpecimen {
            specimen_id: specimen.id.clone(),
            disc_count: census.discs.len(),
            cell_ids: specimen.cell_ids.clone(),
            regions: assigned,
        });
    }
    Ok(SpaceMapResult {
        format: SPACE_MAP_FORMAT.into(),
        version: SPACE_FORMAT_VERSION,
        reference_commit: SPACE_REFERENCE_COMMIT.into(),
        source_kind: input.source_kind.into(),
        seed: input.census.seed,
        parameters: parameters.clone(),
        variables: names,
        radius_um: pattern.parameters.radius_um,
        regions,
        specimens,
    })
}

/// The pattern's mean curves as points along the path, point-major, after checking a result
/// read back from a file.
fn check_pattern(pattern: &SpacePatternResult) -> Result<Vec<f64>, SpaceError> {
    if pattern.format != SPACE_PATTERN_FORMAT || pattern.version != SPACE_FORMAT_VERSION {
        return Err(invalid(format!(
            "the pattern must be {SPACE_PATTERN_FORMAT} version {SPACE_FORMAT_VERSION}"
        )));
    }
    let points = pattern.positions.len();
    let valid = points > 0
        && !pattern.curves.is_empty()
        && pattern
            .positions
            .iter()
            .all(|place| (0.0..=100.0).contains(place))
        && pattern
            .curves
            .iter()
            .all(|curve| curve.mean.len() == points && curve.mean.iter().all(|v| v.is_finite()));
    if !valid {
        return Err(invalid(
            "the pattern needs curves of finite means, one per path position between 0 and 100",
        ));
    }
    let mut curve = Vec::with_capacity(points * pattern.curves.len());
    for point in 0..points {
        curve.extend(pattern.curves.iter().map(|curve| curve.mean[point]));
    }
    Ok(curve)
}

/// The nearest point of `curve` to `values` and their Euclidean distance (R `FNN::knnx`; the
/// first point on ties).
fn nearest_point(curve: &[f64], values: &[f64]) -> (usize, f64) {
    let mut best = (0, f64::INFINITY);
    for (point, coordinates) in curve.chunks_exact(values.len()).enumerate() {
        let distance = coordinates
            .iter()
            .zip(values)
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f64>();
        if distance < best.1 {
            best = (point, distance);
        }
    }
    (best.0, best.1.sqrt())
}

/// Each part of the path's lower bound and region, in path order, and the regions.
type Partition = (Vec<(f64, u32)>, Vec<SpaceMapRegion>);

/// R's partition of the path: the declared regions, with every gap between them, before the
/// first or after the last given to one catch-all region.
fn partition(declared: &[[f64; 2]]) -> Result<Partition, SpaceError> {
    if declared.is_empty() {
        return Err(invalid("declare at least one region"));
    }
    let mut cursor = 0.0;
    for &[lower, upper] in declared {
        if !(lower.is_finite()
            && upper.is_finite()
            && cursor <= lower
            && lower < upper
            && upper <= 100.0)
        {
            return Err(invalid(
                "regions must be increasing, non-overlapping ranges with lower < upper between 0 and 100",
            ));
        }
        cursor = upper;
    }
    let catch_all = declared.len() as u32 + 1;
    let mut lowers = Vec::new();
    let mut gaps = Vec::new();
    let mut cursor = 0.0;
    for (index, &[lower, upper]) in declared.iter().enumerate() {
        if lower > cursor {
            lowers.push((cursor, catch_all));
            gaps.push([cursor, lower]);
        }
        lowers.push((lower, index as u32 + 1));
        cursor = upper;
    }
    if cursor < 100.0 {
        lowers.push((cursor, catch_all));
        gaps.push([cursor, 100.0]);
    }
    let mut regions = declared
        .iter()
        .enumerate()
        .map(|(index, &range)| SpaceMapRegion {
            region: index as u32 + 1,
            ranges: vec![range],
            catch_all: false,
        })
        .collect::<Vec<_>>();
    // Votes need a slot for the catch-all even when no gap uses it.
    regions.push(SpaceMapRegion {
        region: catch_all,
        ranges: gaps,
        catch_all: true,
    });
    Ok((lowers, regions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_fills_gaps_with_one_catch_all_as_r_does() {
        let (lowers, regions) = partition(&[[10.0, 30.0], [50.0, 70.0]]).unwrap();
        assert_eq!(
            lowers,
            [(0.0, 3), (10.0, 1), (30.0, 3), (50.0, 2), (70.0, 3)]
        );
        assert_eq!(
            regions[2].ranges,
            [[0.0, 10.0], [30.0, 50.0], [70.0, 100.0]]
        );
        // Touching regions that span the path leave the catch-all empty.
        let (lowers, regions) = partition(&[[0.0, 50.0], [50.0, 100.0]]).unwrap();
        assert_eq!(lowers, [(0.0, 1), (50.0, 2)]);
        assert!(regions[2].ranges.is_empty());
        for bad in [
            &[[30.0, 10.0]][..],
            &[[10.0, 50.0], [40.0, 60.0]],
            &[[50.0, 70.0], [10.0, 30.0]],
            &[[90.0, 101.0]],
            &[],
        ] {
            assert!(partition(bad).is_err(), "{bad:?}");
        }
    }
}
