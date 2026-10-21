use std::fs;

use marklab_bayes::{compare_psis_loo_models, sha256_hex, PsisLooComparisonInput, PsisLooResult};

use super::cli_schema::CompareModelsArgs;
use super::{publish_json, CliError, MAXIMUM_INPUT_BYTES};

pub(super) fn run(args: CompareModelsArgs) -> Result<(), CliError> {
    let CompareModelsArgs { input, out } = args;
    let input_paths = input;
    let output_path = out;
    let mut inputs = Vec::new();
    for path in input_paths {
        let metadata = fs::metadata(&path).map_err(|source| CliError::Io {
            path: path.clone(),
            source,
        })?;
        if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
            return Err(CliError::Input(
                "Bayesian comparison inputs must be regular files within the 16 MiB limit".into(),
            ));
        }
        let bytes = fs::read(&path).map_err(|source| CliError::Io {
            path: path.clone(),
            source,
        })?;
        let result: PsisLooResult = serde_json::from_slice(&bytes)?;
        inputs.push(PsisLooComparisonInput {
            path: path.display().to_string(),
            artifact_sha256: sha256_hex(&bytes),
            result,
        });
    }
    publish_json(&output_path, &compare_psis_loo_models(inputs)?)
}
