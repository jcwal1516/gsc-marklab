use super::OrdinalSiteHeldoutComparisonModelArgs;
use super::{publish_json, CliError, MAXIMUM_INPUT_BYTES};
use clap::Args;
use marklab_bayes::{
    ordinal_site_heldout_comparison, OrdinalGroupSitePatientData, OrdinalSiteHeldoutComparison,
    OrdinalSiteHeldoutSpec,
};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::PathBuf};
#[derive(Debug, Args)]
pub(crate) struct OrdinalSiteHeldoutComparisonArgs {
    #[command(flatten)]
    pub(super) model: OrdinalSiteHeldoutComparisonModelArgs,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    patient_id: String,
    site_id: String,
    group: String,
    outcome: String,
}
pub(super) fn run(a: OrdinalSiteHeldoutComparisonArgs) -> Result<(), CliError> {
    let prepared = prepare(a.model)?;
    publish_json(&a.out, &execute(&prepared)?)
}
pub(crate) struct PreparedOrdinalSiteHeldout {
    pub(crate) spec: OrdinalSiteHeldoutSpec,
}
pub(crate) fn prepare(
    model: OrdinalSiteHeldoutComparisonModelArgs,
) -> Result<PreparedOrdinalSiteHeldout, CliError> {
    let OrdinalSiteHeldoutComparisonModelArgs {
        input: input_path,
        reference_group,
        comparison_group,
        ordered_levels,
        smoothing,
        optimizer_tolerance,
        maximum_optimizer_evaluations,
    } = model;

    let metadata = fs::metadata(&input_path).map_err(|source| CliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CliError::Input(
            "ordinal heldout input exceeds 16 MiB".into(),
        ));
    }
    let bytes = fs::read(&input_path).map_err(|source| CliError::Io {
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
        .map(|row| -> Result<_, CliError> {
            let row = row?;
            let outcome_code = levels.get(row.outcome.as_str()).copied().ok_or_else(|| {
                CliError::Input(format!("outcome {:?} is not ordered", row.outcome))
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
) -> Result<OrdinalSiteHeldoutComparison, CliError> {
    ordinal_site_heldout_comparison(prepared.spec.clone()).map_err(CliError::from)
}
