use std::{fs, path::Path};

use marklab_bayes::{sha256_hex, NumpyroBetaBinomialGroupGenderSlideSbcWorkerRequest, SbcControls};

use super::cli_schema::BetaBinomialGroupGenderSlideHierarchySbcArgs;
use super::{beta_binomial_group_gender_slide_hierarchy, cross_backend, CliError};

const WORKER_IDENTITY_PREFIX: &[u8] =
    b"marklab.numpyro_beta_binomial_group_gender_slide_hierarchy_sbc_worker_identity.v1\0";

pub(super) fn run(args: BetaBinomialGroupGenderSlideHierarchySbcArgs) -> Result<(), CliError> {
    let BetaBinomialGroupGenderSlideHierarchySbcArgs {
        model,
        replicates,
        minimum_rank_uniformity_p_value,
        minimum_coverage_90,
        maximum_coverage_90,
        out: output_path,
    } = args;
    let prepared = beta_binomial_group_gender_slide_hierarchy::prepare(model)?;
    let timeout_seconds = prepared.timeout_seconds;
    cross_backend::sbc(timeout_seconds, &output_path, |identity| {
        Ok(NumpyroBetaBinomialGroupGenderSlideSbcWorkerRequest::new(
            prepared.request,
            SbcControls {
                replicates,
                minimum_rank_uniformity_p_value,
                minimum_coverage: minimum_coverage_90,
                maximum_coverage: maximum_coverage_90,
            },
            identity.environment_lock_sha256,
            identity.worker_sha256,
            timeout_seconds,
        )?)
    })
}

pub(super) fn identity(
    repository: &Path,
) -> Result<super::worker_process::WorkerIdentity, CliError> {
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| CliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory
        .join("marklab_numpyro_beta_binomial_group_gender_slide_hierarchy_sbc_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| CliError::Io {
        path: worker_path,
        source,
    })?;
    let model_worker_path = worker_directory
        .join("marklab_numpyro_beta_binomial_group_gender_slide_hierarchy_worker.py");
    let model_worker_bytes = fs::read(&model_worker_path).map_err(|source| CliError::Io {
        path: model_worker_path,
        source,
    })?;
    let mut worker_identity = WORKER_IDENTITY_PREFIX.to_vec();
    worker_identity.extend_from_slice(&worker_bytes);
    worker_identity.push(0);
    worker_identity.extend_from_slice(&model_worker_bytes);
    Ok(super::worker_process::WorkerIdentity {
        environment_lock_sha256: sha256_hex(&lock_bytes),
        worker_sha256: sha256_hex(&worker_identity),
    })
}
