//! SPACE: Spatial Patterning Analysis of Cellular Ensembles, on cell tables.
//!
//! A native port of the R package SPACE (Schrom et al., PNAS 2025,
//! doi 10.1073/pnas.2412146122; github.com/eschrom/SPACE at commit
//! `94f0a0f9311e2ee2b406ebc45c84a4e93f2f38f8`, Apache-2.0). A census samples discs of
//! fixed radius around seed cells and records, per disc, the percentage of each object
//! class, mean scalar values on linked classes and mean independent scalars. cisMI ranks
//! every ensemble of up to `depth` variables by co-information relative to censuses whose
//! cells were shuffled between discs. See `docs/space.md` for the method, parameters and
//! the documented deviations from the R code.

mod binning;
mod census;
mod cis;
mod dist;
mod distribution;
mod diversity;
mod ensembles;
mod index;
mod information;
mod input;
mod map;
mod normal;
mod patches;
mod pattern;
mod rank;
mod raster;
mod som;
mod suggest;
mod trans;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use census::{
    space_census, SpaceCensusNeighborhood, SpaceCensusRadius, SpaceCensusResult,
    SpaceCensusSpecimen, SPACE_CENSUS_FORMAT,
};
pub use cis::{
    space_cis, SpaceCisEnsemble, SpaceCisParameters, SpaceCisRadius, SpaceCisResult,
    SpaceCisSpecimen, SpaceNullStatus, SPACE_CIS_FORMAT,
};
pub use dist::{
    space_dist, SpaceDistAxis, SpaceDistCell, SpaceDistCensus, SpaceDistParameters,
    SpaceDistResult, SpaceDistValues, SpaceDistVariable, SPACE_DIST_FORMAT,
};
pub use diversity::{
    space_diversity, SpaceConstituents, SpaceDiversityParameters, SpaceDiversityParent,
    SpaceDiversityParents, SpaceDiversityResult, SpaceDiversitySpecimen, SPACE_DIVERSITY_FORMAT,
};
pub use input::{
    load_space_input, space_input_from_json, SpaceCensusDesign, SpaceInput, SpaceLimits,
    SPACE_REQUEST_FORMAT,
};
pub use map::{
    load_space_map_result, load_space_pattern_result, space_map, SpaceMapParameters,
    SpaceMapRegion, SpaceMapResult, SpaceMapSpecimen, SPACE_MAP_FORMAT,
};
pub use pattern::{
    space_pattern, SpaceNormalization, SpacePatternCurve, SpacePatternParameters,
    SpacePatternReference, SpacePatternResult, SPACE_PATTERN_FORMAT,
};
pub use rank::{
    load_space_mi_result, space_mi_result_from_json, space_rank, SpaceMiResult,
    SpaceRadiusStatistic, SpaceRankParameters, SpaceRankResult, SpaceRankView, SpaceRankedEnsemble,
    SpaceRankedVariable, SPACE_RANK_FORMAT,
};
pub use suggest::{
    space_suggest, SpaceSuggestParameters, SpaceSuggestRadius, SpaceSuggestResult,
    SpaceSuggestSpecimen, SPACE_MAXIMUM_ADVISED_COVERAGE, SPACE_SUGGEST_FORMAT,
};
pub use trans::{
    space_trans, SpaceTransEnsemble, SpaceTransFactor, SpaceTransParameters, SpaceTransRadius,
    SpaceTransResult, SpaceTransSpecimen, SpaceTransStatistic, SPACE_TRANS_FORMAT,
};

/// Version of every SPACE request and result format.
pub const SPACE_FORMAT_VERSION: u32 = 1;

/// The R SPACE revision whose algorithms these commands port.
pub const SPACE_REFERENCE_COMMIT: &str = "94f0a0f9311e2ee2b406ebc45c84a4e93f2f38f8";

/// What a census variable measures in each disc.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpaceVariableKind {
    /// Percentage of the disc's classified cells (in one categorical channel) of one class.
    Object,
    /// Mean of a scalar channel over the disc's cells of one class; zero when none are present.
    LinkedScalar,
    /// Mean of a scalar channel over every cell in the disc.
    IndependentScalar,
}

/// One named census variable.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceVariable {
    pub name: String,
    pub kind: SpaceVariableKind,
    /// The categorical channel for objects and linked scalars.
    pub channel: Option<String>,
    pub class: Option<String>,
    /// The scalar channel for linked and independent scalars.
    pub scalar: Option<String>,
}

/// Rejected SPACE input, resource request or file.
#[derive(Debug, Error, PartialEq)]
pub enum SpaceError {
    #[error("invalid SPACE input: {0}")]
    Invalid(String),
    #[error("SPACE resource limit exceeded: {0}")]
    Resource(String),
    #[error("SPACE input file failed: {0}")]
    Io(String),
}

fn invalid(message: impl Into<String>) -> SpaceError {
    SpaceError::Invalid(message.into())
}

fn resource(message: impl Into<String>) -> SpaceError {
    SpaceError::Resource(message.into())
}

fn label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn worker_threads() -> usize {
    #[cfg(feature = "parallel")]
    {
        rayon::current_num_threads()
    }
    #[cfg(not(feature = "parallel"))]
    {
        1
    }
}

/// Apply `operation(state, chunk_index, chunk)` to consecutive chunks, in parallel when the
/// `parallel` feature is enabled. Each chunk's result depends only on its index, so output is
/// identical at any thread count.
fn for_each_chunk<T, S>(
    data: &mut [T],
    chunk: usize,
    init: impl Fn() -> S + Sync + Send,
    operation: impl Fn(&mut S, usize, &mut [T]) + Sync + Send,
) where
    T: Send,
{
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        data.par_chunks_mut(chunk)
            .enumerate()
            .for_each_init(init, |state, (index, slice)| operation(state, index, slice));
    }
    #[cfg(not(feature = "parallel"))]
    {
        let mut state = init();
        for (index, slice) in data.chunks_mut(chunk).enumerate() {
            operation(&mut state, index, slice);
        }
    }
}
