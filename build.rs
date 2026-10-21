use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    let rustc = env::var("RUSTC")
        .ok()
        .and_then(|program| Command::new(program).arg("--version").output().ok())
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .unwrap_or_else(|| "unavailable".to_owned());
    println!("cargo:rustc-env=MARKLAB_BUILD_RUSTC={rustc}");
    println!("cargo:rerun-if-env-changed=RUSTC");

    let package = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("set by Cargo"));
    // A copy vendored inside another repository must not record that repository's state.
    let in_checkout = repository_root(&package).is_some_and(|root| same_directory(&root, &package));
    if in_checkout {
        // Editing any tracked file can flip the dirty flag; commits, checkouts and staging move
        // the SHA. Outside a checkout, emitting nothing keeps Cargo's default package scan.
        if let Some(files) = git_output(&package, ["ls-files", "-z"]) {
            for file in files.split('\0').filter(|file| !file.is_empty()) {
                println!("cargo:rerun-if-changed={file}");
            }
        }
        for input in [".git/HEAD", ".git/logs/HEAD", ".git/index"] {
            if package.join(input).exists() {
                println!("cargo:rerun-if-changed={input}");
            }
        }
    }
    let sha = in_checkout
        .then(|| git_output(&package, ["rev-parse", "HEAD"]))
        .flatten();
    match (sha, git_dirty(&package)) {
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

fn repository_root(directory: &Path) -> Option<PathBuf> {
    git_output(directory, ["rev-parse", "--show-toplevel"]).map(PathBuf::from)
}

fn same_directory(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn git_output<const N: usize>(directory: &Path, arguments: [&str; N]) -> Option<String> {
    Command::new("git")
        .current_dir(directory)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
