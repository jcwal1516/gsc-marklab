use super::{publish_json, BayesCliError, MAXIMUM_INPUT_BYTES};
use clap::{Args, Parser, Subcommand};
use marklab_bayes::{
    ordinal_site_heldout_comparison, OrdinalGroupSitePatientData, OrdinalSiteHeldoutComparison,
    OrdinalSiteHeldoutSpec,
};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::PathBuf};
#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct Cli {
    #[command(subcommand)]
    command: Top,
}
#[derive(Debug, Subcommand)]
enum Top {
    Bayes {
        #[command(subcommand)]
        command: Command,
    },
}
#[derive(Debug, Subcommand)]
enum Command {
    OrdinalSiteHeldoutComparison(Options),
}
#[derive(Debug, Args)]
struct Options {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    reference_group: String,
    #[arg(long)]
    comparison_group: String,
    #[arg(long)]
    ordered_levels: String,
    #[arg(long)]
    smoothing: f64,
    #[arg(long)]
    optimizer_tolerance: f64,
    #[arg(long)]
    maximum_optimizer_evaluations: u64,
    #[arg(long)]
    out: PathBuf,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    patient_id: String,
    site_id: String,
    group: String,
    outcome: String,
}
pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Cli {
        command:
            Top::Bayes {
                command: Command::OrdinalSiteHeldoutComparison(a),
            },
    } = Cli::parse();
    let prepared = prepare(
        a.input,
        a.reference_group,
        a.comparison_group,
        a.ordered_levels.split(',').map(str::to_owned).collect(),
        a.smoothing,
        a.optimizer_tolerance,
        a.maximum_optimizer_evaluations,
    )?;
    publish_json(&a.out, &execute(&prepared)?)
}
pub(crate) struct PreparedOrdinalSiteHeldout {
    pub(crate) input_path: PathBuf,
    pub(crate) spec: OrdinalSiteHeldoutSpec,
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    ordered_levels: Vec<String>,
    smoothing: f64,
    optimizer_tolerance: f64,
    maximum_optimizer_evaluations: u64,
) -> Result<PreparedOrdinalSiteHeldout, BayesCliError> {
    let metadata = fs::metadata(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "ordinal heldout input exceeds 16 MiB".into(),
        ));
    }
    let bytes = fs::read(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    let levels = ordered_levels
        .iter()
        .enumerate()
        .map(|(i, x)| (x.as_str(), i as u32))
        .collect::<BTreeMap<_, _>>();
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(bytes.as_slice());
    let patients = reader
        .deserialize::<Row>()
        .map(|row| -> Result<_, BayesCliError> {
            let row = row?;
            let outcome_code = levels.get(row.outcome.as_str()).copied().ok_or_else(|| {
                BayesCliError::Input(format!("outcome {:?} is not ordered", row.outcome))
            })?;
            Ok(OrdinalGroupSitePatientData {
                patient_id: row.patient_id,
                site_id: row.site_id,
                group: row.group,
                outcome_code,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PreparedOrdinalSiteHeldout {
        input_path,
        spec: OrdinalSiteHeldoutSpec {
            reference_group,
            comparison_group,
            ordered_levels,
            smoothing,
            optimizer_tolerance,
            maximum_optimizer_evaluations,
            patients,
        },
    })
}
pub(crate) fn execute(
    prepared: &PreparedOrdinalSiteHeldout,
) -> Result<OrdinalSiteHeldoutComparison, BayesCliError> {
    ordinal_site_heldout_comparison(prepared.spec.clone()).map_err(BayesCliError::from)
}
