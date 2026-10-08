use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use crate::{MarklabError, Result};

pub(super) fn batch_output_path(root: &Path, raw_id: &str) -> Result<PathBuf> {
    let id = raw_id.trim();
    let mut components = Path::new(id).components();
    let is_single_normal_component = matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
        && !id.contains('/')
        && !id.contains('\\');
    if !is_single_normal_component {
        return Err(MarklabError::Validation(
            "batch manifest id must be one non-empty path component without separators, '.' or '..'"
                .into(),
        ));
    }

    let target = root.join(id);
    if let Ok(metadata) = fs::symlink_metadata(&target) {
        if metadata.file_type().is_symlink() {
            return Err(MarklabError::Validation(format!(
                "batch output target may not be a symbolic link: {}",
                target.display()
            )));
        }
        if root.exists() {
            let canonical_root =
                fs::canonicalize(root).map_err(|source| MarklabError::io(root, source))?;
            let canonical_target =
                fs::canonicalize(&target).map_err(|source| MarklabError::io(&target, source))?;
            if !canonical_target.starts_with(&canonical_root) {
                return Err(MarklabError::Validation(format!(
                    "batch output target escapes the configured root: {}",
                    target.display()
                )));
            }
        }
    }
    Ok(target)
}

#[cfg(test)]
mod batch_output_path_tests {
    use std::path::Path;

    use super::batch_output_path;

    #[test]
    fn rejects_unsafe_batch_output_ids() {
        let root = Path::new("safe-output");
        for id in [
            "",
            "   ",
            ".",
            "..",
            "../escape",
            "a/b",
            "a\\b",
            "/absolute",
        ] {
            assert!(batch_output_path(root, id).is_err(), "accepted {id:?}");
        }
    }

    #[test]
    fn trims_and_accepts_one_normal_batch_output_component() {
        assert_eq!(
            batch_output_path(Path::new("safe-output"), "  case_001_post  ").expect("valid id"),
            Path::new("safe-output").join("case_001_post")
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_existing_symlink_batch_output_target() {
        use std::{fs, os::unix::fs::symlink};

        let directory = tempfile::tempdir().expect("temp dir");
        let root = directory.path().join("output");
        let outside = directory.path().join("outside");
        fs::create_dir_all(&root).expect("output root");
        fs::create_dir_all(&outside).expect("outside dir");
        symlink(&outside, root.join("case_001")).expect("symlink");

        assert!(batch_output_path(&root, "case_001").is_err());
    }
}
