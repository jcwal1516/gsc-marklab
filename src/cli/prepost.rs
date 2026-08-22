use std::{fs, path::PathBuf};

use crate::{
    output::read_result_document_path_or_dir, prepost::compare_prepost, Result, ResultDocument,
};

pub(super) fn run(pre: PathBuf, post: PathBuf, out: PathBuf) -> Result<()> {
    let pre_result = read_result_document_path_or_dir(&pre)?.into_marked_pattern()?;
    let post_result = read_result_document_path_or_dir(&post)?.into_marked_pattern()?;
    let delta = compare_prepost(&pre_result, &post_result);
    let document = ResultDocument::marked_prepost(delta);

    fs::create_dir_all(&out)?;
    fs::write(out.join("prepost.json"), document.to_json_pretty()?)?;

    Ok(())
}
