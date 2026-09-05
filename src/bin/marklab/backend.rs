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
    /// Private typed native execution boundary for the grouped-conformal application.
    #[command(hide = true)]
    NativeGroupedConformal,
    /// Private native patient-OOF calibration application.
    #[command(hide = true)]
    NativePredictionCalibration,
}

pub(crate) fn cli_route() -> super::command_tree::Route {
    super::command_tree::Route::new::<BackendCli>(|| {
        run().map_err(super::bayes::into_marklab_error)
    })
}

fn run() -> Result<(), super::bayes::BayesCliError> {
    let BackendTopLevel::Backend { command } = BackendCli::parse().command;
    match command {
        BackendCommand::Doctor => doctor(),
        BackendCommand::NativeGroupedConformal => native(|input| {
            marklab::grouped_conformal::execute_native_request(input)
                .map_err(|e| super::bayes::BayesCliError::Input(e.to_string()))
        }),
        BackendCommand::NativePredictionCalibration => native(|input| {
            marklab::prediction_calibration::execute_native_request(input)
                .map_err(|e| super::bayes::BayesCliError::Input(e.to_string()))
        }),
    }
}

fn native(
    execute: impl FnOnce(Vec<u8>) -> Result<Vec<u8>, super::bayes::BayesCliError>,
) -> Result<(), super::bayes::BayesCliError> {
    use super::bayes::BayesCliError;
    use std::io::{Read, Write};
    // The ordinary CSV is bounded to 16 MiB; JSON escaping plus row keys may expand it.
    const INPUT_LIMIT: u64 = 128 * 1024 * 1024;
    let mut input = Vec::new();
    std::io::stdin()
        .take(INPUT_LIMIT + 1)
        .read_to_end(&mut input)
        .map_err(|error| BayesCliError::Input(format!("native request read failed: {error}")))?;
    if input.len() as u64 > INPUT_LIMIT {
        return Err(BayesCliError::Input(
            "native request exceeds 128 MiB".into(),
        ));
    }
    let bytes = execute(input)?;
    std::io::stdout()
        .lock()
        .write_all(&bytes)
        .map_err(|error| BayesCliError::Input(format!("native result write failed: {error}")))?;
    Ok(())
}

fn doctor() -> Result<(), super::bayes::BayesCliError> {
    let root = marklab::python_backend_assets_root()?;
    let interpreter = marklab::python_backend_interpreter(&root)?;
    let result = super::bayes::run_worker(&root, "marklab_backend_doctor.py", b"{}", 30)?;
    println!("Runtime assets: {}", root.display());
    println!("Interpreter: {}", interpreter.display());
    println!(
        "Compilation cache: {}",
        marklab::python_backend_cache(&root)?.display()
    );
    print!(
        "{}",
        String::from_utf8(result).map_err(|error| {
            super::bayes::BayesCliError::Backend(format!("doctor returned invalid UTF-8: {error}"))
        })?
    );
    Ok(())
}
