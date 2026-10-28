use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use clap::Subcommand;
use marklab::space::{
    load_space_input, load_space_mi_result, space_census, space_cis, space_dist, space_rank,
    space_trans, SpaceCisParameters, SpaceDistParameters, SpaceError, SpaceRankParameters,
    SpaceTransParameters,
};
use marklab::MarklabError;

use super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError};

mod tables;

use tables::{census_table, cis_table, dist_table, rank_table, rank_variable_table, trans_table};

#[derive(Subcommand)]
pub(crate) enum SpaceCommand {
    /// Inspect the SPACE request profile, analyses, defaults and claim level.
    Describe,
    /// Census discs around seed cells (R census_table) and publish their contents.
    Census {
        /// marklab.space_request version 1; file sources resolve relative to it.
        #[arg(long)]
        request: PathBuf,
        /// New JSON result file.
        #[arg(long)]
        out: PathBuf,
        /// Optional new CSV with one row per disc.
        #[arg(long)]
        table: Option<PathBuf>,
    },
    /// Rank ensembles by cis mutual information within each specimen (R measure_cisMI).
    Cis {
        /// marklab.space_request version 1; file sources resolve relative to it.
        #[arg(long)]
        request: PathBuf,
        /// New JSON result file.
        #[arg(long)]
        out: PathBuf,
        /// Optional new CSV with one row per specimen, radius and ensemble.
        #[arg(long)]
        table: Option<PathBuf>,
        /// Largest ensemble size (SPACE tutorial value).
        #[arg(long, default_value_t = 3)]
        depth: usize,
        /// Randomized censuses per radius.
        #[arg(long, default_value_t = 100)]
        bootstraps: usize,
        /// Variables every largest ensemble must contain.
        #[arg(long, value_delimiter = ',')]
        all: Vec<String>,
        /// Variables of which every largest ensemble must contain at least one.
        #[arg(long, value_delimiter = ',')]
        alo: Vec<String>,
        /// Variables excluded from every ensemble.
        #[arg(long, value_delimiter = ',')]
        not: Vec<String>,
        /// Upper bound on bins per variable.
        #[arg(long, default_value_t = 100)]
        max_bins: usize,
        /// Census radii to analyse in micrometres (default: all).
        #[arg(long, value_delimiter = ',')]
        radii: Vec<f64>,
        /// Specimens to analyse (default: all).
        #[arg(long, value_delimiter = ',')]
        specimen: Vec<String>,
    },
    /// Rank ensembles by how well their patterning separates groups of specimens
    /// (R measure_transMI).
    Trans {
        /// marklab.space_request version 1; file sources resolve relative to it.
        #[arg(long)]
        request: PathBuf,
        /// New JSON result file.
        #[arg(long)]
        out: PathBuf,
        /// Optional new CSV with one row per radius, ensemble and grouping factor.
        #[arg(long)]
        table: Option<PathBuf>,
        /// Largest ensemble size (SPACE tutorial value).
        #[arg(long, default_value_t = 3)]
        depth: usize,
        /// Shuffles of the pairwise divergences per ensemble.
        #[arg(long, default_value_t = 100)]
        bootstraps: usize,
        /// Variables every largest ensemble must contain.
        #[arg(long, value_delimiter = ',')]
        all: Vec<String>,
        /// Variables of which every largest ensemble must contain at least one.
        #[arg(long, value_delimiter = ',')]
        alo: Vec<String>,
        /// Variables excluded from every ensemble.
        #[arg(long, value_delimiter = ',')]
        not: Vec<String>,
        /// Upper bound on bins per variable.
        #[arg(long, default_value_t = 100)]
        max_bins: usize,
        /// Census radii to analyse in micrometres (default: all).
        #[arg(long, value_delimiter = ',')]
        radii: Vec<f64>,
        /// Specimens to compare (default: all).
        #[arg(long, value_delimiter = ',')]
        specimen: Vec<String>,
        /// Grouping factors to test (default: every factor in the specimens' groups).
        #[arg(long, value_delimiter = ',')]
        factor: Vec<String>,
    },
    /// The values or smoothed joint distribution of one or two variables in one specimen
    /// (R plot_dist, without the plot).
    Dist {
        /// marklab.space_request version 1; file sources resolve relative to it.
        #[arg(long)]
        request: PathBuf,
        /// New JSON result file.
        #[arg(long)]
        out: PathBuf,
        /// Optional new CSV with one row per disc, or per grid cell when rounding.
        #[arg(long)]
        table: Option<PathBuf>,
        /// One or two census variables.
        #[arg(long, value_delimiter = ',', required = true)]
        variables: Vec<String>,
        /// Census radius in micrometres.
        #[arg(long)]
        radius: f64,
        /// Specimen to describe (required when the request has several).
        #[arg(long)]
        specimen: Option<String>,
        /// Rounding bins per variable; without it each disc's values are reported.
        #[arg(long)]
        bins: Option<usize>,
        /// Describe one randomized census instead of the observed one.
        #[arg(long)]
        randomized: bool,
    },
    /// Rank a cisMI or transMI result's ensembles and variables at one radius, or follow one
    /// ensemble across radii (R plot_MI_rank and plot_MI_radius, without the plots).
    Rank {
        /// A marklab.space_cis_result or marklab.space_trans_result file.
        #[arg(long)]
        result: PathBuf,
        /// New JSON result file.
        #[arg(long)]
        out: PathBuf,
        /// Optional new CSV of the ranked ensembles, or of the ensemble at each radius.
        #[arg(long)]
        table: Option<PathBuf>,
        /// Optional new CSV of the ranked variables.
        #[arg(long)]
        variable_table: Option<PathBuf>,
        /// Radius whose ensembles are ranked, in micrometres.
        #[arg(long)]
        radius: Option<f64>,
        /// Specimen of a cisMI result (required when it has several).
        #[arg(long)]
        specimen: Option<String>,
        /// Grouping factor of a transMI result (required when it tests several).
        #[arg(long)]
        factor: Option<String>,
        /// Ensemble sizes to rank (default: all).
        #[arg(long, value_delimiter = ',')]
        depth: Vec<usize>,
        /// Largest P value kept.
        #[arg(long, default_value_t = 0.05)]
        p_threshold: f64,
        /// Smallest |cisMI| or transMI kept (default 0.1 for cisMI and 0 for transMI).
        #[arg(long)]
        mi_threshold: Option<f64>,
        /// Filter on Benjamini-Hochberg P values; false uses the normal-approximation P.
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        p_adjust: bool,
        /// Variables every kept ensemble must contain.
        #[arg(long, value_delimiter = ',')]
        all: Vec<String>,
        /// Variables of which every kept ensemble must contain at least one.
        #[arg(long, value_delimiter = ',')]
        alo: Vec<String>,
        /// Variables no kept ensemble may contain.
        #[arg(long, value_delimiter = ',')]
        not: Vec<String>,
        /// Follow this ensemble across every radius instead of ranking one radius.
        #[arg(long, value_delimiter = ',')]
        ensemble: Vec<String>,
    },
}

pub(crate) fn run(command: SpaceCommand) -> marklab::Result<()> {
    match command {
        SpaceCommand::Describe => {
            println!("{}", serde_json::to_string_pretty(&describe())?);
            Ok(())
        }
        SpaceCommand::Census {
            request,
            out,
            table,
        } => {
            require_new(&out, &[table.as_deref()])?;
            let input = load_space_input(&request).map_err(space_error)?;
            let result = space_census(&input).map_err(space_error)?;
            publish(&out, &result)?;
            if let Some(table) = table {
                write_new(&table, &census_table(&result)?)?;
            }
            Ok(())
        }
        SpaceCommand::Cis {
            request,
            out,
            table,
            depth,
            bootstraps,
            all,
            alo,
            not,
            max_bins,
            radii,
            specimen,
        } => {
            require_new(&out, &[table.as_deref()])?;
            let input = load_space_input(&request).map_err(space_error)?;
            let parameters = SpaceCisParameters {
                depth,
                bootstraps,
                all,
                alo,
                not,
                max_bins,
                radii_um: radii,
                specimens: specimen,
            };
            let result = space_cis(&input, &parameters).map_err(space_error)?;
            publish(&out, &result)?;
            if let Some(table) = table {
                write_new(&table, &cis_table(&result)?)?;
            }
            Ok(())
        }
        SpaceCommand::Trans {
            request,
            out,
            table,
            depth,
            bootstraps,
            all,
            alo,
            not,
            max_bins,
            radii,
            specimen,
            factor,
        } => {
            require_new(&out, &[table.as_deref()])?;
            let input = load_space_input(&request).map_err(space_error)?;
            let parameters = SpaceTransParameters {
                depth,
                bootstraps,
                all,
                alo,
                not,
                max_bins,
                radii_um: radii,
                specimens: specimen,
                factors: factor,
            };
            let result = space_trans(&input, &parameters).map_err(space_error)?;
            publish(&out, &result)?;
            if let Some(table) = table {
                write_new(&table, &trans_table(&result)?)?;
            }
            Ok(())
        }
        SpaceCommand::Dist {
            request,
            out,
            table,
            variables,
            radius,
            specimen,
            bins,
            randomized,
        } => {
            require_new(&out, &[table.as_deref()])?;
            let input = load_space_input(&request).map_err(space_error)?;
            let parameters = SpaceDistParameters {
                variables,
                radius_um: radius,
                specimen,
                bins,
                randomized,
            };
            let result = space_dist(&input, &parameters).map_err(space_error)?;
            publish(&out, &result)?;
            if let Some(table) = table {
                write_new(&table, &dist_table(&result)?)?;
            }
            Ok(())
        }
        SpaceCommand::Rank {
            result,
            out,
            table,
            variable_table,
            radius,
            specimen,
            factor,
            depth,
            p_threshold,
            mi_threshold,
            p_adjust,
            all,
            alo,
            not,
            ensemble,
        } => {
            require_new(&out, &[table.as_deref(), variable_table.as_deref()])?;
            if variable_table.is_some() && !ensemble.is_empty() {
                return Err(MarklabError::Config(
                    "--variable-table applies to a ranking of one radius, not --ensemble".into(),
                ));
            }
            let source = load_space_mi_result(&result).map_err(space_error)?;
            let parameters = SpaceRankParameters {
                radius_um: radius,
                specimen,
                factor,
                depths: depth,
                p_threshold,
                mi_threshold,
                p_adjust,
                all,
                alo,
                not,
                ensemble,
            };
            let ranked = space_rank(&source, &parameters).map_err(space_error)?;
            publish(&out, &ranked)?;
            if let Some(table) = table {
                write_new(&table, &rank_table(&ranked)?)?;
            }
            if let Some(variable_table) = variable_table {
                write_new(&variable_table, &rank_variable_table(&ranked)?)?;
            }
            Ok(())
        }
    }
}

fn describe() -> serde_json::Value {
    serde_json::json!({
        "profile": marklab::space::SPACE_REQUEST_FORMAT,
        "version": marklab::space::SPACE_FORMAT_VERSION,
        "reference": {
            "package": "SPACE (R)",
            "repository": "https://github.com/eschrom/SPACE",
            "commit": marklab::space::SPACE_REFERENCE_COMMIT,
            "license": "Apache-2.0",
            "citation": "Schrom et al., PNAS 2025, doi:10.1073/pnas.2412146122"
        },
        "scientific_status": "research",
        "sources": ["inline", "multiplex_study_recipe", "cellvit_he_csv"],
        "variables": {
            "object": "percentage of a disc's classified cells of one class, per categorical channel",
            "linked_scalar": "mean scalar over a disc's cells of one class (SPACE OS_pairs)",
            "independent_scalar": "mean scalar over every cell in a disc (Marklab's cell-table form of SPACE's independent scalar sets)"
        },
        "analyses": {
            "census": "R census_table",
            "cis": {
                "r_function": "measure_cisMI",
                "defaults": {"depth": 3, "bootstraps": 100, "max_bins": 100},
                "p_values": ["SPACE normal approximation 2*Phi(-|Z|)", "Benjamini-Hochberg per radius", "Monte Carlo from the same randomized censuses"]
            },
            "trans": {
                "r_function": "measure_transMI",
                "defaults": {"depth": 3, "bootstraps": 100, "max_bins": 100},
                "statistic": "directed weighted modularity of each grouping factor on the specimen similarity network built from symmetric KL divergences",
                "p_values": ["SPACE normal approximation 1-Phi(Z)", "Benjamini-Hochberg per radius and factor", "Monte Carlo from the same shuffled divergences"]
            },
            "dist": {
                "r_function": "plot_dist (data only)",
                "output": "each disc's values of one or two variables, or with bins their smoothed joint distribution over every grid cell"
            },
            "rank": {
                "r_functions": ["plot_MI_rank (data only)", "plot_MI_radius (data only)"],
                "defaults": {"p_threshold": 0.05, "mi_threshold": {"cis": 0.1, "trans": 0.0}, "p_adjust": true},
                "output": "kept ensembles by decreasing |Z| and variables by aggregate |Z|, or one ensemble at every radius"
            }
        },
        "deviations_from_r": "docs/space.md#differences-from-r-space",
        "claim_ceiling": "exploratory ranking of spatial co-patterning; randomized censuses do not model tissue structure, and overlapping discs are not independent",
        "guide": "docs/space.md"
    })
}

fn require_new(out: &Path, tables: &[Option<&Path>]) -> marklab::Result<()> {
    for path in std::iter::once(out).chain(tables.iter().flatten().copied()) {
        if fs::symlink_metadata(path).is_ok() {
            return Err(MarklabError::Config(format!(
                "output already exists: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn publish(path: &Path, result: &impl serde::Serialize) -> marklab::Result<()> {
    publish_pretty_json(path, result, "space").map_err(|error| match error {
        ExclusiveJsonOutputError::OutputExists => {
            MarklabError::Config(format!("output already exists: {}", path.display()))
        }
        ExclusiveJsonOutputError::OutputMustNameFile => {
            MarklabError::Config("output must name a file".into())
        }
        ExclusiveJsonOutputError::Io { path, source } => MarklabError::Io { path, source },
        ExclusiveJsonOutputError::Json(error) => MarklabError::Json(error),
    })
}

/// Create `path` exclusively and write the whole table, removing it if the write fails.
fn write_new(path: &Path, bytes: &[u8]) -> marklab::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| MarklabError::Io {
            path: path.to_owned(),
            source,
        })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|source| {
            let _ = fs::remove_file(path);
            MarklabError::Io {
                path: path.to_owned(),
                source,
            }
        })
}

fn space_error(error: SpaceError) -> MarklabError {
    match error {
        SpaceError::Resource(message) => MarklabError::Compute(message),
        SpaceError::Invalid(message) | SpaceError::Io(message) => MarklabError::Config(message),
    }
}
