use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "marklab")]
struct BackendCli {
    #[command(subcommand)]
    command: BackendTopLevel,
}

#[derive(Subcommand)]
enum BackendTopLevel {
    /// Inspect the installed scientific backend environment.
    Backend {
        #[command(subcommand)]
        command: BackendCommand,
    },
}

#[derive(Subcommand)]
enum BackendCommand {
    /// Verify asset paths, Python 3.12, and the locked direct backend packages.
    Doctor,
}

pub(crate) fn cli_route() -> super::command_tree::Route {
    super::command_tree::Route::new::<BackendCli>(|| run().map_err(super::bayes::into_marklab_error))
}

fn run() -> Result<(), super::bayes::BayesCliError> {
    let BackendTopLevel::Backend { command: BackendCommand::Doctor } = BackendCli::parse().command;
    let root = marklab::python_backend_assets_root()?;
    let interpreter = marklab::python_backend_interpreter(&root)?;
    let result = super::bayes::run_worker(&root, "marklab_backend_doctor.py", b"{}", 30)?;
    println!("Runtime assets: {}", root.display());
    println!("Interpreter: {}", interpreter.display());
    println!("Compilation cache: {}", marklab::python_backend_cache(&root)?.display());
    print!("{}", String::from_utf8(result).map_err(|error| {
        super::bayes::BayesCliError::Backend(format!("doctor returned invalid UTF-8: {error}"))
    })?);
    Ok(())
}
