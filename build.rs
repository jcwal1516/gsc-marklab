use std::{env, path::Path, process::Command};

fn main() {
    let rustc = env::var("RUSTC")
        .ok()
        .and_then(|program| Command::new(program).arg("--version").output().ok())
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .unwrap_or_else(|| "unavailable".to_owned());
    println!("cargo:rustc-env=MARKLAB_BUILD_RUSTC={rustc}");

    let sha = git_output(["rev-parse", "HEAD"]);
    let dirty = git_dirty(Path::new("."));
    match (sha, dirty) {
        (Some(sha), Some(dirty)) => {
            println!("cargo:rustc-env=MARKLAB_BUILD_GIT_SHA={sha}");
            println!("cargo:rustc-env=MARKLAB_BUILD_GIT_DIRTY={dirty}");
        }
        _ => {
            println!("cargo:rustc-env=MARKLAB_BUILD_GIT_SHA=");
            println!("cargo:rustc-env=MARKLAB_BUILD_GIT_DIRTY=unknown");
        }
    }
}

pub(crate) fn git_dirty(repository: &Path) -> Option<bool> {
    Command::new("git")
        .current_dir(repository)
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| !output.stdout.is_empty())
}

fn git_output<const N: usize>(arguments: [&str; N]) -> Option<String> {
    Command::new("git")
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
