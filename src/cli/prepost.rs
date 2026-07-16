use super::*;

pub(super) fn run(pre: PathBuf, post: PathBuf, out: PathBuf) -> Result<()> {
    let pre_result = ResultDocument::from_json(&fs::read_to_string(pre)?)?.into_marked_pattern()?;
    let post_result =
        ResultDocument::from_json(&fs::read_to_string(post)?)?.into_marked_pattern()?;
    let delta = compare_prepost(&pre_result, &post_result);

    fs::create_dir_all(&out)?;
    fs::write(
        out.join("prepost.json"),
        serde_json::to_string_pretty(&delta)?,
    )?;

    Ok(())
}
