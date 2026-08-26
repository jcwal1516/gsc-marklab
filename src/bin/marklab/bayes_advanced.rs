use std::path::PathBuf;

use clap::{Parser, Subcommand};
use marklab_topology::sha256_hex;
use serde_json::Value;

use super::topology::{publish_json, read_input, read_required, run_worker, TopologyCliError};

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct AdvancedBayesCli {
    #[command(subcommand)]
    command: BayesTopLevel,
}

#[derive(Debug, Subcommand)]
enum BayesTopLevel {
    Bayes {
        #[command(subcommand)]
        command: AdvancedBayesCommand,
    },
}

#[derive(Debug, Subcommand)]
enum AdvancedBayesCommand {
    HmcNormal {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    AdvancedCluster {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SpdeSuite {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run_cli() -> Result<(), TopologyCliError> {
    match AdvancedBayesCli::parse().command {
        BayesTopLevel::Bayes {
            command: AdvancedBayesCommand::HmcNormal { input, out },
        } => run("hmc_normal", input, out),
        BayesTopLevel::Bayes {
            command: AdvancedBayesCommand::AdvancedCluster { input, out },
        } => run("advanced_cluster", input, out),
        BayesTopLevel::Bayes {
            command: AdvancedBayesCommand::SpdeSuite { input, out },
        } => run("spde_suite", input, out),
    }
}

fn run(mode: &str, input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let input_bytes = read_input(&input)?;
    let spec: Value = serde_json::from_slice(&input_bytes)?;
    let timeout = spec
        .get("timeout_seconds")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            TopologyCliError::Input(
                "advanced Bayesian input requires integer timeout_seconds".into(),
            )
        })?;
    if !(1..=3_600).contains(&timeout) || !spec.is_object() {
        return Err(TopologyCliError::Input(
            "advanced Bayesian timeout must be between 1 and 3600 seconds".into(),
        ));
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let worker_path = repository.join("workers/python/marklab_scipy_advanced_bayes_worker.py");
    let lock = read_required(&repository.join("workers/python/uv.lock"))?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format":"marklab.scipy_advanced_bayes_request", "version":1, "mode":mode,
        "backend":{"name":"numpy_scipy_advanced_bayes","scipy_version":"1.18.1","numpy_version":"2.4.6","python_version":"3.12","license":"BSD-3-Clause","environment_lock_sha256":sha256_hex(&lock),"worker_sha256":sha256_hex(&worker)},
        "spec":spec, "input_sha256":sha256_hex(&input_bytes)
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(&repository, &worker_path, &request_bytes, timeout)?;
    let result: Value = serde_json::from_slice(&response)?;
    let expected = match mode {
        "hmc_normal" => "marklab.fixed_step_hmc_normal_mean",
        "advanced_cluster" => "marklab.advanced_cluster_and_gibbs_models",
        _ => "marklab.rectangular_spde_suite",
    };
    if result["format"] != expected
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
    {
        return Err(TopologyCliError::Backend(
            "advanced Bayesian result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

pub(crate) fn into_marklab_error(error: TopologyCliError) -> marklab::MarklabError {
    marklab::MarklabError::Validation(error.to_string())
}
