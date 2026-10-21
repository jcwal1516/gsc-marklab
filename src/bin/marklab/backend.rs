use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum BackendCommand {
    /// Verify asset paths, Python 3.12, and the locked direct backend packages.
    Doctor,
}

pub(crate) fn execute(command: BackendCommand) -> marklab::Result<()> {
    run(command).map_err(super::bayes::into_marklab_error)
}

pub(crate) fn run(command: BackendCommand) -> Result<(), super::bayes::CliError> {
    let BackendCommand::Doctor = command;
    let root = marklab::backend::python_backend_assets_root()?;
    let interpreter = marklab::backend::python_backend_interpreter(&root)?;
    let result = super::bayes::run_worker(&root, "marklab_backend_doctor.py", b"{}", 30)?;
    println!("Runtime assets: {}", root.display());
    println!("Interpreter: {}", interpreter.display());
    println!(
        "Compilation cache: {}",
        marklab::backend::python_backend_cache(&root)?.display()
    );
    print!(
        "{}",
        String::from_utf8(result).map_err(|error| {
            super::bayes::CliError::Backend(format!("doctor returned invalid UTF-8: {error}"))
        })?
    );
    Ok(())
}
