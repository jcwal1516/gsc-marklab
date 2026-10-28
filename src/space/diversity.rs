//! Composition and diversity of specimens and of their parts (R `alpha_diversity` and
//! `beta_diversity`, for cell tables). R counts each object's pixels, or sums each scalar image,
//! over an image or within each parent object; here cells are counted, or scalars summed, over
//! the specimen or within each parent class or pattern region.

use serde::{Deserialize, Serialize};

use super::ensembles::selected_specimens;
use super::input::{Specimen, VariableSpec, NO_CLASS};
use super::map::check_map;
use super::{
    invalid, SpaceError, SpaceInput, SpaceMapResult, SPACE_FORMAT_VERSION, SPACE_REFERENCE_COMMIT,
};

pub const SPACE_DIVERSITY_FORMAT: &str = "marklab.space_diversity_result";

/// R replaces zero shares by this probability over the number of constituents before taking
/// divergences.
const TINY_PROBABILITY: f64 = 0.000_001;

/// What a composition is made of (R `img_type`).
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpaceConstituents {
    /// Cells of each class of one object channel (R "O").
    Objects { channel: String },
    /// Each independent scalar summed over cells (R "S"); values must not be negative.
    Scalars,
}

/// The parent objects whose compositions are compared (R `beta_diversity`).
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpaceDiversityParents {
    /// The whole specimen only (R `alpha_diversity`).
    None,
    /// The classes of one object channel.
    Channel { channel: String },
    /// The regions of a `space map` result, as R measures `map_pattern`'s regions.
    Map,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceDiversityParameters {
    pub constituents: SpaceConstituents,
    pub parents: SpaceDiversityParents,
    /// Specimens; empty for all.
    pub specimens: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceDiversityParent {
    /// The parent class, or the region number.
    pub parent: String,
    /// The parent's cells; R weights each parent by its pixels.
    pub cell_count: usize,
    /// Share of each constituent within the parent; all 0 when it has none.
    pub percentages: Vec<f64>,
    /// Shannon entropy of the shares (R `E`).
    pub alpha_bits: f64,
    /// KL divergence of the shares from the weighted mean composition; absent for a parent
    /// without constituents, which R leaves out of beta diversity.
    pub divergence_bits: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceDiversitySpecimen {
    pub specimen_id: String,
    pub cell_count: usize,
    /// The whole specimen's composition (R `alpha_diversity`).
    pub percentages: Vec<f64>,
    pub alpha_bits: f64,
    /// Each parent's composition, in class or region order.
    pub parents: Vec<SpaceDiversityParent>,
    /// The parents' divergences averaged with weights proportional to their cells; absent
    /// without parents.
    pub beta_bits: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceDiversityResult {
    pub format: String,
    pub version: u32,
    pub reference_commit: String,
    pub source_kind: String,
    pub parameters: SpaceDiversityParameters,
    /// Constituent names: the channel's classes, or the scalar channels.
    pub constituents: Vec<String>,
    pub specimens: Vec<SpaceDiversitySpecimen>,
}

/// Measure the composition and diversity of each selected specimen; `map` supplies the parents
/// when they are a map's regions.
pub fn space_diversity(
    input: &SpaceInput,
    parameters: &SpaceDiversityParameters,
    map: Option<&SpaceMapResult>,
) -> Result<SpaceDiversityResult, SpaceError> {
    let (constituents, layer) = constituent_layer(input, &parameters.constituents)?;
    let parents = match (&parameters.parents, map) {
        (SpaceDiversityParents::None, None) => Parents::None,
        (SpaceDiversityParents::Channel { channel }, None) => {
            let map = object_channel(input, channel)?;
            Parents::Channel {
                map,
                names: input.object_channels[map].classes.clone(),
            }
        }
        (SpaceDiversityParents::Map, Some(result)) => {
            check_map(result)?;
            Parents::Map(result)
        }
        (SpaceDiversityParents::Map, None) => {
            return Err(invalid("map parents need a map result"));
        }
        (_, Some(_)) => return Err(invalid("a map result is used only with map parents")),
    };
    let specimens = selected_specimens(input, &parameters.specimens)?
        .into_iter()
        .map(|position| {
            let specimen = &input.specimens[position];
            let (whole, alpha_bits) = alpha(specimen, &layer)?;
            let (parents, beta_bits) = match parents.of(specimen)? {
                Some((names, assignment)) => {
                    let (parents, beta) = beta(specimen, &layer, &names, &assignment)?;
                    (parents, Some(beta))
                }
                None => (Vec::new(), None),
            };
            Ok(SpaceDiversitySpecimen {
                specimen_id: specimen.id.clone(),
                cell_count: specimen.cell_ids.len(),
                percentages: whole,
                alpha_bits,
                parents,
                beta_bits,
            })
        })
        .collect::<Result<Vec<_>, SpaceError>>()?;
    Ok(SpaceDiversityResult {
        format: SPACE_DIVERSITY_FORMAT.into(),
        version: SPACE_FORMAT_VERSION,
        reference_commit: SPACE_REFERENCE_COMMIT.into(),
        source_kind: input.source_kind.into(),
        parameters: parameters.clone(),
        constituents,
        specimens,
    })
}

/// How each cell adds to the constituents.
enum Layer {
    /// An object channel with this many classes.
    Objects { map: usize, classes: usize },
    /// Scalar channels, by position in the specimen's scalars.
    Scalars(Vec<usize>),
}

impl Layer {
    fn width(&self) -> usize {
        match self {
            Layer::Objects { classes, .. } => *classes,
            Layer::Scalars(scalars) => scalars.len(),
        }
    }

    /// Add `row`'s contribution to `totals`.
    fn add(&self, specimen: &Specimen, row: usize, totals: &mut [f64]) {
        match self {
            Layer::Objects { map, .. } => {
                let class = specimen.classes[*map][row];
                if class != NO_CLASS {
                    totals[class as usize] += 1.0;
                }
            }
            Layer::Scalars(scalars) => {
                for (total, &scalar) in totals.iter_mut().zip(scalars) {
                    *total += specimen.scalars[scalar][row];
                }
            }
        }
    }
}

fn constituent_layer(
    input: &SpaceInput,
    constituents: &SpaceConstituents,
) -> Result<(Vec<String>, Layer), SpaceError> {
    match constituents {
        SpaceConstituents::Objects { channel } => {
            let map = object_channel(input, channel)?;
            let classes = input.object_channels[map].classes.clone();
            let layer = Layer::Objects {
                map,
                classes: classes.len(),
            };
            Ok((classes, layer))
        }
        SpaceConstituents::Scalars => {
            let (names, scalars): (Vec<_>, Vec<_>) = input
                .variables
                .iter()
                .zip(&input.variable_descriptions)
                .filter_map(|(spec, description)| match *spec {
                    VariableSpec::Independent { scalar } => {
                        Some((description.name.clone(), scalar))
                    }
                    _ => None,
                })
                .unzip();
            if names.is_empty() {
                return Err(invalid(
                    "scalar compositions need independent scalars; R measures none for linked scalars",
                ));
            }
            for specimen in &input.specimens {
                for (name, &scalar) in names.iter().zip(&scalars) {
                    if specimen.scalars[scalar].iter().any(|value| *value < 0.0) {
                        return Err(invalid(format!(
                            "scalar {name} is negative in specimen {}; compositions need values of at least 0",
                            specimen.id
                        )));
                    }
                }
            }
            Ok((names, Layer::Scalars(scalars)))
        }
    }
}

fn object_channel(input: &SpaceInput, channel: &str) -> Result<usize, SpaceError> {
    input
        .object_channels
        .iter()
        .position(|known| known.id == channel)
        .ok_or_else(|| invalid(format!("{channel} is not an object channel of the request")))
}

/// The parents' names and each cell's parent.
type Assignment = (Vec<String>, Vec<Option<usize>>);

enum Parents<'a> {
    None,
    Channel { map: usize, names: Vec<String> },
    Map(&'a SpaceMapResult),
}

impl Parents<'_> {
    /// The parents' names and each cell's parent, or `None` without parents.
    fn of(&self, specimen: &Specimen) -> Result<Option<Assignment>, SpaceError> {
        match self {
            Parents::None => Ok(None),
            Parents::Channel { map, names } => Ok(Some((
                names.clone(),
                specimen.classes[*map]
                    .iter()
                    .map(|&class| (class != NO_CLASS).then_some(class as usize))
                    .collect(),
            ))),
            Parents::Map(result) => {
                let mapped = result
                    .specimens
                    .iter()
                    .find(|mapped| mapped.specimen_id == specimen.id)
                    .ok_or_else(|| {
                        invalid(format!("the map does not cover specimen {}", specimen.id))
                    })?;
                if mapped.cell_ids != specimen.cell_ids {
                    return Err(invalid(format!(
                        "the map's cells of specimen {} differ from the request's",
                        specimen.id
                    )));
                }
                Ok(Some((
                    result
                        .regions
                        .iter()
                        .map(|region| region.region.to_string())
                        .collect(),
                    mapped
                        .regions
                        .iter()
                        .map(|region| region.map(|region| region as usize - 1))
                        .collect(),
                )))
            }
        }
    }
}

/// Shares of each constituent and their Shannon entropy in bits (R `alpha_diversity`).
fn alpha(specimen: &Specimen, layer: &Layer) -> Result<(Vec<f64>, f64), SpaceError> {
    let mut totals = vec![0.0; layer.width()];
    for row in 0..specimen.cell_ids.len() {
        layer.add(specimen, row, &mut totals);
    }
    let sum = totals.iter().sum::<f64>();
    if sum <= 0.0 {
        return Err(invalid(format!(
            "specimen {} has no cells or scalar values to compose",
            specimen.id
        )));
    }
    let shares = totals.iter().map(|total| total / sum).collect::<Vec<_>>();
    Ok((percentages(&shares), entropy(&shares)))
}

/// Each parent's composition and its divergence from the weighted mean composition (R
/// `beta_diversity`).
fn beta(
    specimen: &Specimen,
    layer: &Layer,
    names: &[String],
    assignment: &[Option<usize>],
) -> Result<(Vec<SpaceDiversityParent>, f64), SpaceError> {
    let width = layer.width();
    let mut totals = vec![0.0; names.len() * width];
    let mut cells = vec![0_usize; names.len()];
    for (row, parent) in assignment.iter().enumerate() {
        if let Some(parent) = *parent {
            cells[parent] += 1;
            layer.add(
                specimen,
                row,
                &mut totals[parent * width..(parent + 1) * width],
            );
        }
    }
    let shares = totals
        .chunks_exact(width)
        .map(|row| {
            let sum = row.iter().sum::<f64>();
            (sum > 0.0).then(|| row.iter().map(|total| total / sum).collect::<Vec<_>>())
        })
        .collect::<Vec<_>>();
    let weight_sum = shares
        .iter()
        .zip(&cells)
        .filter(|(share, _)| share.is_some())
        .map(|(_, &count)| count as f64)
        .sum::<f64>();
    if weight_sum == 0.0 {
        return Err(invalid(format!(
            "no parent of specimen {} has any constituent",
            specimen.id
        )));
    }
    let weight = |parent: usize| cells[parent] as f64 / weight_sum;
    let mut mean = vec![0.0; width];
    for (parent, share) in shares.iter().enumerate() {
        if let Some(share) = share {
            for (mean, value) in mean.iter_mut().zip(share) {
                *mean += value * weight(parent);
            }
        }
    }
    let tiny = TINY_PROBABILITY / width as f64;
    let mean = regularized(&normalized(&mean), tiny);
    let mut beta_bits = 0.0;
    let parents = shares
        .iter()
        .enumerate()
        .map(|(parent, share)| {
            let divergence = share.as_ref().map(|share| {
                regularized(share, tiny)
                    .iter()
                    .zip(&mean)
                    .map(|(value, mean)| value * (value / mean).log2())
                    .sum::<f64>()
            });
            if let Some(divergence) = divergence {
                beta_bits += divergence * weight(parent);
            }
            SpaceDiversityParent {
                parent: names[parent].clone(),
                cell_count: cells[parent],
                percentages: share
                    .as_ref()
                    .map_or_else(|| vec![0.0; width], |share| percentages(share)),
                alpha_bits: share.as_ref().map_or(0.0, |share| entropy(share)),
                divergence_bits: divergence,
            }
        })
        .collect();
    Ok((parents, beta_bits))
}

fn normalized(values: &[f64]) -> Vec<f64> {
    let sum = values.iter().sum::<f64>();
    values.iter().map(|value| value / sum).collect()
}

/// R: zero shares become `tiny`, and the shares are renormalized, when any is zero.
fn regularized(shares: &[f64], tiny: f64) -> Vec<f64> {
    if shares.iter().all(|share| *share != 0.0) {
        return shares.to_vec();
    }
    normalized(
        &shares
            .iter()
            .map(|&share| if share == 0.0 { tiny } else { share })
            .collect::<Vec<_>>(),
    )
}

fn entropy(shares: &[f64]) -> f64 {
    -shares
        .iter()
        .filter(|share| **share > 0.0)
        .map(|share| share * share.log2())
        .sum::<f64>()
}

fn percentages(shares: &[f64]) -> Vec<f64> {
    shares.iter().map(|share| 100.0 * share).collect()
}
