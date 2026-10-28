//! How many discs give a desired coverage (R `suggest_number`, for cell tables). R divides
//! the coverage times the non-background pixels by the disc volume in pixels; here the cells
//! that carry a variable are divided by the mean number of them in a disc, over every cell that
//! may centre one.

use serde::{Deserialize, Serialize};

use super::census::eligible_seeds;
use super::ensembles::{selected_radii, selected_specimens};
use super::input::{Specimen, VariableSpec, NO_CLASS};
use super::{
    for_each_chunk, invalid, resource, SpaceError, SpaceInput, SPACE_FORMAT_VERSION,
    SPACE_REFERENCE_COMMIT,
};

pub const SPACE_SUGGEST_FORMAT: &str = "marklab.space_suggest_result";

/// R warns that coverage above this pseudo-replicates.
pub const SPACE_MAXIMUM_ADVISED_COVERAGE: f64 = 5.0;

/// Disc centres queried between checks of the work limit.
const CENTRES_PER_BATCH: usize = 1 << 16;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceSuggestParameters {
    /// Mean number of discs each counted cell should fall in (R `coverage`).
    pub coverage: f64,
    /// Census radii to suggest for; empty for all.
    pub radii_um: Vec<f64>,
    /// Specimens; empty for all.
    pub specimens: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceSuggestRadius {
    pub radius_um: f64,
    /// Counted cells per disc, averaged over every eligible centre.
    pub mean_disc_cells: f64,
    /// R: `ceiling(coverage × counted cells / disc size)`.
    pub sample_size: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceSuggestSpecimen {
    pub specimen_id: String,
    /// Cells with a class in an object channel or a nonzero independent scalar, R's
    /// non-background pixels.
    pub counted_cells: usize,
    /// Cells that may centre a disc; a sample cannot exceed them.
    pub eligible_centres: usize,
    pub radii: Vec<SpaceSuggestRadius>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceSuggestResult {
    pub format: String,
    pub version: u32,
    pub reference_commit: String,
    pub source_kind: String,
    pub parameters: SpaceSuggestParameters,
    pub specimens: Vec<SpaceSuggestSpecimen>,
}

/// Suggest each selected specimen's sample size at each selected radius.
pub fn space_suggest(
    input: &SpaceInput,
    parameters: &SpaceSuggestParameters,
) -> Result<SpaceSuggestResult, SpaceError> {
    if !(parameters.coverage.is_finite() && parameters.coverage > 0.0) {
        return Err(invalid("coverage must be positive"));
    }
    let radius_positions = if parameters.radii_um.is_empty() {
        (0..input.census.radii_um.len()).collect()
    } else {
        selected_radii(input, &parameters.radii_um)?
    };
    let mut visits = 0_u64;
    let specimens = selected_specimens(input, &parameters.specimens)?
        .into_iter()
        .map(|position| {
            let specimen = &input.specimens[position];
            let counted = counted_cells(input, specimen);
            let counted_cells = counted.iter().filter(|counted| **counted).count();
            let centres = eligible_seeds(input, specimen);
            if counted_cells == 0 || centres.is_empty() {
                return Err(invalid(format!(
                    "specimen {} has no cells with a variable or no eligible disc centres",
                    specimen.id
                )));
            }
            let radii = radius_positions
                .iter()
                .map(|&radius_position| {
                    let radius_um = input.census.radii_um[radius_position];
                    let total =
                        disc_cells(input, specimen, &counted, &centres, radius_um, &mut visits)?;
                    let mean_disc_cells = total as f64 / centres.len() as f64;
                    // Positive: centres have a class, or every cell is a centre and some
                    // cell is counted.
                    let size =
                        (parameters.coverage * counted_cells as f64 / mean_disc_cells).ceil();
                    Ok(SpaceSuggestRadius {
                        radius_um,
                        mean_disc_cells,
                        sample_size: size as usize,
                    })
                })
                .collect::<Result<Vec<_>, SpaceError>>()?;
            Ok(SpaceSuggestSpecimen {
                specimen_id: specimen.id.clone(),
                counted_cells,
                eligible_centres: centres.len(),
                radii,
            })
        })
        .collect::<Result<Vec<_>, SpaceError>>()?;
    Ok(SpaceSuggestResult {
        format: SPACE_SUGGEST_FORMAT.into(),
        version: SPACE_FORMAT_VERSION,
        reference_commit: SPACE_REFERENCE_COMMIT.into(),
        source_kind: input.source_kind.into(),
        parameters: parameters.clone(),
        specimens,
    })
}

/// Whether each cell carries a variable: a class in an object channel or a nonzero
/// independent scalar (R sums every image but the linked scalars').
fn counted_cells(input: &SpaceInput, specimen: &Specimen) -> Vec<bool> {
    let scalars = input
        .variables
        .iter()
        .filter_map(|spec| match *spec {
            VariableSpec::Independent { scalar } => Some(scalar),
            _ => None,
        })
        .collect::<Vec<_>>();
    (0..specimen.cell_ids.len())
        .map(|row| {
            specimen.classes.iter().any(|map| map[row] != NO_CLASS)
                || scalars
                    .iter()
                    .any(|&scalar| specimen.scalars[scalar][row] != 0.0)
        })
        .collect()
}

/// Counted cells within `radius_um` of each centre, the centre included, summed over centres.
fn disc_cells(
    input: &SpaceInput,
    specimen: &Specimen,
    counted: &[bool],
    centres: &[usize],
    radius_um: f64,
    visits: &mut u64,
) -> Result<u64, SpaceError> {
    let mut total = 0_u64;
    for batch in centres.chunks(CENTRES_PER_BATCH) {
        // Per centre: counted members and every member visited; a failed query marks `None`.
        let mut counts = vec![Some((0_u64, 0_u64)); batch.len()];
        let chunk = 256;
        for_each_chunk(
            &mut counts,
            chunk,
            || (),
            |(), index, slots| {
                for (offset, slot) in slots.iter_mut().enumerate() {
                    let centre = batch[index * chunk + offset];
                    let mut members = (u64::from(counted[centre]), 1_u64);
                    let query = specimen
                        .index
                        .visit_within_radius(centre, radius_um, |neighbor| {
                            members.0 += u64::from(counted[neighbor.index]);
                            members.1 += 1;
                        });
                    *slot = query.is_ok().then_some(members);
                }
            },
        );
        for count in counts {
            let (members, visited) =
                count.ok_or_else(|| invalid(format!("disc query failed at {radius_um} um")))?;
            total += members;
            *visits = visits.saturating_add(visited);
        }
        if *visits > input.limits.maximum_entropy_visits {
            return Err(resource(format!(
                "{} disc member visits exceed the maximum of {}",
                *visits, input.limits.maximum_entropy_visits
            )));
        }
    }
    Ok(total)
}
