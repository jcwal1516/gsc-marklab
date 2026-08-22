use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    output::read_result_document_path_or_dir, prepost::compare_multimodal_prepost, MarklabError,
    MultimodalResult, Result, ResultDocument,
};

use super::super::{
    batch_output_path, HeInputFormat, MultimodalAnalyzeRequest, MultimodalManifestRow,
};

pub(in crate::cli) fn prepost(pre: PathBuf, post: PathBuf, out: PathBuf) -> Result<()> {
    let pre_result = read_analysis_result_path_or_dir(&pre)?;
    let post_result = read_analysis_result_path_or_dir(&post)?;
    if pre_result.registration.value().is_none() && pre_result.fused_cell_summary.value().is_none()
    {
        bail!("multimodal prepost requires a multimodal pre result");
    }
    if post_result.registration.value().is_none()
        && post_result.fused_cell_summary.value().is_none()
    {
        bail!("multimodal prepost requires a multimodal post result");
    }

    let delta = compare_multimodal_prepost(&pre_result, &post_result);
    let document = ResultDocument::multimodal_prepost(delta);
    fs::create_dir_all(&out)?;
    fs::write(out.join("prepost.json"), document.to_json_pretty()?)?;
    super::analyze::write_pretty_json(
        &out.join("pre_result_summary.json"),
        &pre_result.fused_cell_summary,
    )?;
    super::analyze::write_pretty_json(
        &out.join("post_result_summary.json"),
        &post_result.fused_cell_summary,
    )?;

    Ok(())
}

pub(in crate::cli) fn batch(manifest: PathBuf, out: PathBuf) -> Result<()> {
    let mut reader = csv::Reader::from_path(&manifest)?;
    for (index, row) in reader.deserialize::<MultimodalManifestRow>().enumerate() {
        let row_number = index + 2;
        let row = row?;
        let row_out = batch_output_path(&out, &row.id).map_err(|error| {
            MarklabError::Validation(format!(
                "{} row {}: {error}",
                manifest.display(),
                row_number
            ))
        })?;
        if option_path_is_present(&row.pre) || option_path_is_present(&row.post) {
            let pre = required_manifest_path(&manifest, row_number, "pre", &row.pre)?;
            let post = required_manifest_path(&manifest, row_number, "post", &row.post)?;
            prepost(pre, post, row_out)?;
        } else {
            let he_cells =
                required_manifest_path(&manifest, row_number, "he_cells", &row.he_cells)?;
            let ihc_cells =
                required_manifest_path(&manifest, row_number, "ihc_cells", &row.ihc_cells)?;
            let landmarks =
                required_manifest_path(&manifest, row_number, "landmarks", &row.landmarks)?;
            let config = required_manifest_path(&manifest, row_number, "config", &row.config)?;
            let case_id = row
                .case_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    MarklabError::Validation(format!(
                        "{} row {}: case_id is required",
                        manifest.display(),
                        row_number
                    ))
                })?
                .to_owned();
            let timepoint = row
                .timepoint
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    MarklabError::Validation(format!(
                        "{} row {}: timepoint is required",
                        manifest.display(),
                        row_number
                    ))
                })?
                .to_owned();
            let protein = row
                .protein
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    MarklabError::Validation(format!(
                        "{} row {}: protein is required",
                        manifest.display(),
                        row_number
                    ))
                })?
                .to_owned();
            super::analyze::run(MultimodalAnalyzeRequest {
                he_cells,
                ihc_cells,
                landmarks,
                config,
                out: row_out,
                case_id,
                timepoint,
                protein,
                he_format: HeInputFormat::HeCsv,
                cellvit_min_probability: 0.5,
            })?;
        }
    }
    Ok(())
}

fn required_manifest_path(
    manifest: &Path,
    row_number: usize,
    field: &str,
    value: &Option<PathBuf>,
) -> Result<PathBuf> {
    match value {
        Some(path) if !path.as_os_str().is_empty() => Ok(path.clone()),
        _ => bail!(
            "{} row {}: {} is required",
            manifest.display(),
            row_number,
            field
        ),
    }
}

fn option_path_is_present(value: &Option<PathBuf>) -> bool {
    value
        .as_ref()
        .is_some_and(|path| !path.as_os_str().is_empty())
}

fn read_analysis_result_path_or_dir(path: &Path) -> Result<MultimodalResult> {
    read_result_document_path_or_dir(path)?.into_multimodal()
}
