use std::{fs, path::Path};

use crate::{errors::Result, ResultDocument};

pub(crate) fn read_result_document_path_or_dir(path: &Path) -> Result<ResultDocument> {
    let result_path = if path.is_dir() {
        path.join("result.json")
    } else {
        path.to_path_buf()
    };
    ResultDocument::from_json(&fs::read_to_string(result_path)?)
}
