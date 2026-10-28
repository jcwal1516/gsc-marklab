//! Ensembles of census variables and the options cisMI and transMI share: `depth`,
//! `bootstraps`, `max_bins`, `all`, `alo` and `not` (R `measure_cisMI` and `measure_transMI`).

use std::collections::BTreeSet;

use super::census::check_retained;
use super::information::{binomial, combinations};
use super::input::VariableSpec;
use super::{invalid, resource, SpaceError, SpaceInput};

pub(crate) struct EnsembleOptions<'a> {
    pub(crate) depth: usize,
    pub(crate) bootstraps: usize,
    pub(crate) max_bins: usize,
    pub(crate) all: &'a [String],
    pub(crate) alo: &'a [String],
    pub(crate) not: &'a [String],
}

impl EnsembleOptions<'_> {
    /// Checks shared by every ensemble analysis (R's `stop` conditions and Marklab's bounds).
    pub(crate) fn validate(&self, input: &SpaceInput) -> Result<(), SpaceError> {
        if self.depth == 0 {
            return Err(invalid("depth must be at least 1"));
        }
        if self.bootstraps < 2 || self.bootstraps > input.limits.maximum_bootstraps {
            return Err(resource(format!(
                "bootstraps must be between 2 and {}",
                input.limits.maximum_bootstraps
            )));
        }
        if !(2..=255).contains(&self.max_bins) {
            return Err(invalid("max_bins must be between 2 and 255"));
        }
        for name in self.all.iter().chain(self.alo).chain(self.not) {
            if input.variable_index(name).is_none() {
                return Err(invalid(format!("{name} is not a census variable")));
            }
        }
        let required = self.all.len() + usize::from(!self.alo.is_empty());
        if required > self.depth {
            return Err(invalid(
                "the number of required variables exceeds ensemble depth",
            ));
        }
        Ok(())
    }

    /// The candidate variables left after `not` and, when `all` plus one `alo` slot fill the
    /// depth, after dropping every variable outside them, as R does.
    pub(crate) fn active(
        &self,
        input: &SpaceInput,
        candidates: Vec<(usize, VariableSpec)>,
    ) -> Vec<(usize, VariableSpec)> {
        let not = self.not.iter().collect::<BTreeSet<_>>();
        let required = self.all.len() + usize::from(!self.alo.is_empty());
        let keep_only = (required == self.depth)
            .then(|| self.all.iter().chain(self.alo).collect::<BTreeSet<_>>());
        candidates
            .into_iter()
            .filter(|(index, _)| {
                let name = &input.variable_descriptions[*index].name;
                !not.contains(name) && keep_only.as_ref().is_none_or(|keep| keep.contains(name))
            })
            .collect()
    }
}

/// Positions of the requested census radii, all when none are named.
pub(crate) fn selected_radii(input: &SpaceInput, radii: &[f64]) -> Result<Vec<usize>, SpaceError> {
    if radii.is_empty() {
        return Ok((0..input.census.radii_um.len()).collect());
    }
    let mut positions = radii
        .iter()
        .map(|radius| {
            input
                .census
                .radii_um
                .iter()
                .position(|known| known == radius)
                .ok_or_else(|| invalid(format!("{radius} um is not a census radius")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    positions.sort_unstable();
    positions.dedup();
    Ok(positions)
}

/// Positions of the requested specimens, all when none are named.
pub(crate) fn selected_specimens(
    input: &SpaceInput,
    specimens: &[String],
) -> Result<Vec<usize>, SpaceError> {
    let selected = specimens.iter().collect::<BTreeSet<_>>();
    for id in &selected {
        if !input.specimen_ids().any(|known| known == id.as_str()) {
            return Err(invalid(format!("unknown specimen {id}")));
        }
    }
    Ok(input
        .specimens
        .iter()
        .enumerate()
        .filter(|(_, specimen)| selected.is_empty() || selected.contains(&specimen.id))
        .map(|(position, _)| position)
        .collect())
}

/// Ensembles reported at each size: every combination below `depth`, and at `depth` those
/// satisfying `all` and `alo` (R applies the constraints only at full depth).
pub(crate) struct Ensembles {
    pub(crate) variable_count: usize,
    pub(crate) depth: usize,
    /// `by_size[size - 1]`: flattened sorted variable positions.
    by_size: Vec<Vec<u16>>,
}

impl Ensembles {
    pub(crate) fn new(
        variable_count: usize,
        options: &EnsembleOptions,
        names: &[String],
        input: &SpaceInput,
        retained: usize,
    ) -> Result<Self, SpaceError> {
        let depth = options.depth;
        let position = |name: &String| names.iter().position(|known| known == name);
        let all = options.all.iter().filter_map(position).collect::<Vec<_>>();
        let alo = options.alo.iter().filter_map(position).collect::<Vec<_>>();
        // Like R's combn, every combination up to `depth` is enumerated before `all` and
        // `alo` filter the largest size; the limit bounds that enumeration.
        let enumerated = (1..=depth).fold(0_u64, |sum, size| {
            sum.saturating_add(binomial(variable_count, size))
        });
        if enumerated > input.limits.maximum_ensembles as u64 {
            return Err(resource(format!(
                "{enumerated} enumerated ensembles exceed the maximum of {}",
                input.limits.maximum_ensembles
            )));
        }
        // The tables hold every enumerated ensemble, the largest size briefly twice.
        let table_bytes = enumerated.saturating_mul(2 * depth as u64 * size_of::<u16>() as u64);
        check_retained(
            input,
            retained.saturating_add(usize::try_from(table_bytes).unwrap_or(usize::MAX)),
        )?;
        let mut by_size = (1..depth)
            .map(|size| combinations(variable_count, size))
            .collect::<Vec<_>>();
        let mut largest = combinations(variable_count, depth);
        if options.all.len() > all.len() {
            // A required variable is absent from this specimen: no full-depth ensemble qualifies.
            largest.clear();
        }
        if !options.alo.is_empty() && alo.is_empty() {
            largest.clear();
        }
        let largest = largest
            .chunks_exact(depth)
            .filter(|ensemble| {
                all.iter()
                    .all(|&required| ensemble.contains(&(required as u16)))
                    && (alo.is_empty() || alo.iter().any(|&one| ensemble.contains(&(one as u16))))
            })
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        by_size.push(largest);
        Ok(Self {
            variable_count,
            depth,
            by_size,
        })
    }

    pub(crate) fn count(&self, size: usize) -> usize {
        self.by_size[size - 1].len() / size
    }

    pub(crate) fn total(&self) -> usize {
        (1..=self.depth).map(|size| self.count(size)).sum()
    }

    pub(crate) fn table_bytes(&self) -> usize {
        self.by_size.iter().map(Vec::len).sum::<usize>() * size_of::<u16>()
    }

    pub(crate) fn ensemble(&self, size: usize, index: usize) -> &[u16] {
        &self.by_size[size - 1][index * size..(index + 1) * size]
    }

    /// An upper estimate of the bytes one radius's reported ensembles occupy, each holding
    /// its variable names and a `record`-byte summary, plus a range per variable.
    pub(crate) fn result_bytes(&self, names: &[String], record: usize) -> usize {
        let longest = names.iter().map(String::len).max().unwrap_or(0);
        let per_ensemble =
            record + self.depth * (size_of::<String>() + longest) + size_of::<(usize, usize)>();
        self.total()
            .saturating_mul(per_ensemble)
            .saturating_add(names.len().saturating_mul(size_of::<[f64; 2]>()))
    }
}
