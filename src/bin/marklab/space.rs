use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use clap::Subcommand;
use marklab::space::{
    load_space_input, space_census, space_cis, space_trans, SpaceCensusResult, SpaceCisParameters,
    SpaceCisResult, SpaceError, SpaceNullStatus, SpaceTransParameters, SpaceTransResult,
};
use marklab::MarklabError;

use super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError};

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
            require_new(&out, table.as_deref())?;
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
            require_new(&out, table.as_deref())?;
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
            require_new(&out, table.as_deref())?;
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
            }
        },
        "deviations_from_r": "docs/space.md#differences-from-r-space",
        "claim_ceiling": "exploratory ranking of spatial co-patterning; randomized censuses do not model tissue structure, and overlapping discs are not independent",
        "guide": "docs/space.md"
    })
}

fn require_new(out: &Path, table: Option<&Path>) -> marklab::Result<()> {
    for path in std::iter::once(out).chain(table) {
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

/// One row per disc; variables absent from a specimen are zero (R `standardize_censuses`).
fn census_table(result: &SpaceCensusResult) -> marklab::Result<Vec<u8>> {
    let names = result
        .variables
        .iter()
        .map(|variable| variable.name.as_str())
        .filter(|name| {
            result
                .specimens
                .iter()
                .any(|specimen| specimen.variables.iter().any(|known| known == name))
        })
        .collect::<Vec<_>>();
    let mut writer = csv::Writer::from_writer(Vec::new());
    let mut header = vec![
        "specimen_id",
        "radius_um",
        "seed_cell_id",
        "x_um",
        "y_um",
        "member_count",
    ];
    header.extend(&names);
    writer.write_record(&header)?;
    for specimen in &result.specimens {
        let columns = names
            .iter()
            .map(|name| specimen.variables.iter().position(|known| known == name))
            .collect::<Vec<_>>();
        for radius in &specimen.radii {
            for disc in &radius.discs {
                let mut record = vec![
                    specimen.specimen_id.clone(),
                    number(radius.radius_um),
                    disc.seed_cell_id.clone(),
                    number(disc.x_um),
                    number(disc.y_um),
                    disc.member_count.to_string(),
                ];
                record.extend(
                    columns.iter().map(|column| {
                        column.map_or_else(|| "0".into(), |c| number(disc.values[c]))
                    }),
                );
                writer.write_record(&record)?;
            }
        }
    }
    writer
        .into_inner()
        .map_err(|error| MarklabError::Compute(format!("CSV buffer: {error}")))
}

fn cis_table(result: &SpaceCisResult) -> marklab::Result<Vec<u8>> {
    let depth = result.parameters.depth;
    let mut writer = csv::Writer::from_writer(Vec::new());
    let mut header = vec!["specimen_id".to_owned(), "radius_um".into(), "depth".into()];
    header.extend((1..=depth).map(|position| format!("v{position}")));
    header.extend(
        [
            "observed_mi",
            "null_mean_mi",
            "null_sd_mi",
            "cis_mi",
            "z_score",
            "status",
            "p_value_normal",
            "p_adjust_bh",
            "p_value_monte_carlo",
        ]
        .map(String::from),
    );
    writer.write_record(&header)?;
    for specimen in &result.specimens {
        for radius in &specimen.radii {
            for ensemble in &radius.ensembles {
                let mut record = vec![
                    specimen.specimen_id.clone(),
                    number(radius.radius_um),
                    ensemble.variables.len().to_string(),
                ];
                record.extend((0..depth).map(|position| {
                    ensemble
                        .variables
                        .get(position)
                        .cloned()
                        .unwrap_or_default()
                }));
                record.extend([
                    number(ensemble.observed_mi),
                    number(ensemble.null_mean_mi),
                    number(ensemble.null_sd_mi),
                    number(ensemble.cis_mi),
                    ensemble.z_score.map(number).unwrap_or_default(),
                    status(ensemble.status),
                    number(ensemble.p_value_normal),
                    number(ensemble.p_adjust_bh),
                    number(ensemble.p_value_monte_carlo),
                ]);
                writer.write_record(&record)?;
            }
        }
    }
    writer
        .into_inner()
        .map_err(|error| MarklabError::Compute(format!("CSV buffer: {error}")))
}

fn trans_table(result: &SpaceTransResult) -> marklab::Result<Vec<u8>> {
    let depth = result.parameters.depth;
    let mut writer = csv::Writer::from_writer(Vec::new());
    let mut header = vec!["radius_um".to_owned(), "depth".into()];
    header.extend((1..=depth).map(|position| format!("v{position}")));
    header.extend(
        [
            "factor",
            "trans_mi",
            "null_mean",
            "null_sd",
            "z_score",
            "status",
            "p_value_normal",
            "p_adjust_bh",
            "p_value_monte_carlo",
        ]
        .map(String::from),
    );
    writer.write_record(&header)?;
    for radius in &result.radii {
        for ensemble in &radius.ensembles {
            for statistic in &ensemble.factors {
                let mut record = vec![
                    number(radius.radius_um),
                    ensemble.variables.len().to_string(),
                ];
                record.extend((0..depth).map(|position| {
                    ensemble
                        .variables
                        .get(position)
                        .cloned()
                        .unwrap_or_default()
                }));
                record.extend([
                    statistic.factor.clone(),
                    number(statistic.trans_mi),
                    number(statistic.null_mean),
                    number(statistic.null_sd),
                    number(statistic.z_score),
                    status(statistic.status),
                    number(statistic.p_value_normal),
                    number(statistic.p_adjust_bh),
                    number(statistic.p_value_monte_carlo),
                ]);
                writer.write_record(&record)?;
            }
        }
    }
    writer
        .into_inner()
        .map_err(|error| MarklabError::Compute(format!("CSV buffer: {error}")))
}

fn status(status: SpaceNullStatus) -> String {
    match status {
        SpaceNullStatus::Available => "available".into(),
        SpaceNullStatus::DegenerateNull => "degenerate_null".into(),
    }
}

/// Shortest round-trip decimal, as in the JSON result.
fn number(value: f64) -> String {
    format!("{value:?}")
}
