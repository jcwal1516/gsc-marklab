//! Python backend releases pinned by `workers/python/uv.lock`.
//!
//! Workers refuse to run under any other release, and results record these versions.
//! `tests/backend_versions.rs` checks every constant against the lock file.

/// Pinned python version for the worker protocol.
pub const PYTHON_VERSION: &str = "3.12";
/// Pinned pymc version for the worker protocol.
pub const PYMC_VERSION: &str = "6.3.0";
/// Pinned pytensor version for the worker protocol.
pub const PYTENSOR_VERSION: &str = "3.2.4";
/// Pinned numpyro version for the worker protocol.
pub const NUMPYRO_VERSION: &str = "0.21.0";
/// Pinned jax version for the worker protocol.
pub const JAX_VERSION: &str = "0.11.1";
/// Pinned numpy version for the worker protocol.
pub const NUMPY_VERSION: &str = "2.4.6";
/// Pinned scipy version for the worker protocol.
pub const SCIPY_VERSION: &str = "1.18.1";
/// Pinned pot version for the worker protocol.
pub const POT_VERSION: &str = "0.9.7.post1";
/// Pinned gudhi version for the worker protocol.
pub const GUDHI_VERSION: &str = "3.13.0";
/// Pinned arviz version for the worker protocol.
pub const ARVIZ_VERSION: &str = "1.3.0";
/// Pinned arviz stats version for the worker protocol.
pub const ARVIZ_STATS_VERSION: &str = "1.3.1";
/// Pinned torch version for the worker protocol.
pub const TORCH_VERSION: &str = "2.13.0";

/// Backend identities recorded by workers that combine several packages.
/// Pinned scipy backend for the worker protocol.
pub const SCIPY_BACKEND: &str = "scipy-1.18.1";
/// Pinned numpy scipy backend for the worker protocol.
pub const NUMPY_SCIPY_BACKEND: &str = "numpy-2.4.6+scipy-1.18.1";
/// Pinned scipy pytensor backend for the worker protocol.
pub const SCIPY_PYTENSOR_BACKEND: &str = "scipy-1.18.1+pytensor-3.2.4";
/// Pinned scipy pytensor pymc backend for the worker protocol.
pub const SCIPY_PYTENSOR_PYMC_BACKEND: &str = "scipy-1.18.1+pytensor-3.2.4+pymc-6.3.0";
/// Pinned arviz backend for the worker protocol.
pub const ARVIZ_BACKEND: &str = "arviz-1.3.0+arviz-stats-1.3.1";
/// Pinned sbi backend for the worker protocol.
pub const SBI_BACKEND: &str = "sbi-0.26.1";
/// Pinned gudhi backend for the worker protocol.
pub const GUDHI_BACKEND: &str = "gudhi-3.13.0";
/// Pinned gudhi scikit image scipy backend for the worker protocol.
pub const GUDHI_SCIKIT_IMAGE_SCIPY_BACKEND: &str = "gudhi-3.13.0+scikit-image-0.26.0+scipy-1.18.1";

#[cfg(test)]
mod tests {
    use super::*;

    fn locked_version(lock: &str, package: &str) -> Option<String> {
        lock.split("[[package]]")
            .find(|entry| entry.contains(&format!("\nname = \"{package}\"\n")))
            .and_then(|entry| {
                entry
                    .lines()
                    .find_map(|line| line.strip_prefix("version = "))
            })
            .map(|version| version.trim_matches('"').to_owned())
    }

    #[test]
    fn every_pinned_backend_matches_the_worker_lock_file() {
        let lock_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../workers/python/uv.lock");
        let lock = std::fs::read_to_string(lock_path).expect("worker lock file");
        let single = [
            ("pymc", PYMC_VERSION),
            ("pytensor", PYTENSOR_VERSION),
            ("numpyro", NUMPYRO_VERSION),
            ("jax", JAX_VERSION),
            ("numpy", NUMPY_VERSION),
            ("scipy", SCIPY_VERSION),
            ("pot", POT_VERSION),
            ("gudhi", GUDHI_VERSION),
            ("arviz", ARVIZ_VERSION),
            ("arviz-stats", ARVIZ_STATS_VERSION),
            ("torch", TORCH_VERSION),
        ];
        for (package, version) in single {
            assert_eq!(
                locked_version(&lock, package).as_deref(),
                Some(version),
                "{package}"
            );
        }
        for identity in [
            SCIPY_BACKEND,
            NUMPY_SCIPY_BACKEND,
            SCIPY_PYTENSOR_BACKEND,
            SCIPY_PYTENSOR_PYMC_BACKEND,
            ARVIZ_BACKEND,
            SBI_BACKEND,
            GUDHI_BACKEND,
            GUDHI_SCIKIT_IMAGE_SCIPY_BACKEND,
        ] {
            for component in identity.split('+') {
                let (package, version) = component.rsplit_once('-').expect("name-version");
                assert_eq!(
                    locked_version(&lock, package).as_deref(),
                    Some(version),
                    "{identity}"
                );
            }
        }
        let requires_python = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../workers/python/pyproject.toml"
        ))
        .expect("worker project");
        assert!(requires_python.contains(&format!("requires-python = \">={PYTHON_VERSION},")));
    }
}
