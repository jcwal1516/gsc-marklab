use std::{fs, path::PathBuf};

use crate::{
    synthetic_smoke::{
        run_multimodal_synthetic_smoke, run_synthetic_smoke, MultimodalSyntheticSmokeSummary,
    },
    MarklabError, Result,
};

pub(super) fn run(suite: &str, replicates: usize, out: PathBuf) -> Result<()> {
    fs::create_dir_all(&out)?;
    match suite {
        "synthetic" => {
            let summary = run_synthetic_smoke(replicates)?;
            fs::write(
                out.join("smoke.json"),
                serde_json::to_string_pretty(&summary)?,
            )?;
            let failed = summary
                .results
                .iter()
                .filter_map(|(name, result)| (!result.passed).then_some(name.as_str()))
                .collect::<Vec<_>>();
            if !failed.is_empty() {
                bail!(
                    "synthetic generator smoke check failed for: {}",
                    failed.join(", ")
                );
            }
        }
        "multimodal" => {
            let summary = run_multimodal_synthetic_smoke(replicates, 123)?;
            fs::write(
                out.join("smoke.json"),
                serde_json::to_string_pretty(&summary)?,
            )?;
            let failed = failed_multimodal_generators(&summary);
            if !failed.is_empty() {
                bail!(
                    "multimodal synthetic generator smoke check failed for: {}",
                    failed.join(", ")
                );
            }
        }
        _ => bail!("--suite must be synthetic or multimodal"),
    }
    Ok(())
}

fn failed_multimodal_generators(summary: &MultimodalSyntheticSmokeSummary) -> Vec<String> {
    summary
        .results
        .iter()
        .filter(|(_name, result)| !result.passed)
        .map(|(name, _result)| name.clone())
        .collect()
}
