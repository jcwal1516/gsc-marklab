# Python backends

Most of Marklab runs in Rust and needs no Python. The Bayesian models and a few other
methods run as Python scripts ("workers") using PyMC, NumPyro and related packages. They
need Python 3.12 and the exact package versions locked in `workers/python/uv.lock`, so
results do not change when a package releases a new version.
[Normal-mean prior sensitivity](normal-mean-prior-sensitivity.md) is one Bayesian method
that runs natively without Python.

Release downloads include the worker scripts, the lock file and project metadata next to
the `marklab` binary. Extract the whole archive and keep `workers/python` together, because
workers share some modules. The Python environment itself is installed separately, because
it contains platform-specific compiled packages.

## Install

From the root of the extracted release or the source checkout, with
[uv](https://docs.astral.sh/uv/) installed:

```sh
export MARKLAB_RUNTIME_ROOT="$(pwd)"
export UV_PROJECT_ENVIRONMENT="$MARKLAB_RUNTIME_ROOT/target/pymc-venv"
uv sync --project "$MARKLAB_RUNTIME_ROOT/workers/python" --locked --python 3.12
export MARKLAB_PYTHON="$UV_PROJECT_ENVIRONMENT/bin/python"
./marklab backend doctor
```

In a source checkout, run `target/debug/marklab backend doctor` after building. On Windows
the interpreter is `Scripts/python.exe` inside the environment; set `MARKLAB_PYTHON` to
its absolute path and run `marklab.exe`.

`uv sync --locked` installs exactly the versions in the lock file. Marklab never installs
or upgrades packages itself.

If the installation directory is read-only, set `UV_PROJECT_ENVIRONMENT` to a writable
location before `uv sync`, then set `MARKLAB_PYTHON` to that environment's interpreter and
`MARKLAB_BACKEND_CACHE` to a writable absolute directory.

## Settings

| Variable | Effect |
|---|---|
| `MARKLAB_RUNTIME_ROOT` | Absolute path to the directory containing `workers/python/uv.lock` and `pyproject.toml`. If set but invalid, Marklab stops with an error instead of looking elsewhere. |
| (not set) | Marklab uses the workers next to its own executable. In a source checkout this works only while the executable is inside that checkout's `target` directory. |
| `MARKLAB_PYTHON` | Absolute path to the Python interpreter. Default: `target/pymc-venv` under the runtime root. |
| `MARKLAB_BACKEND_CACHE` | Absolute, writable directory for compiled model code. Default: `target/pymc-cache` under the runtime root. |
| `MARKLAB_BACKEND_PATH` | Absolute directories, separated like `PATH`, that workers search for executables such as PyTensor's C++ compiler. Default: `/usr/bin:/bin:/usr/sbin:/sbin`. Set it when the compiler lives elsewhere (Nix, conda, Homebrew-only GCC); PyMC workers stop with an error if they find no compiler. |
| `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION` | Never start a Python worker. Saved results from earlier project runs can still be reused. |

## Checking the installation

`marklab backend doctor` checks that the interpreter is Python 3.12 and that each directly
required package is installed at its pinned version and listed in the lock file. It also
prints the paths in use. It does not check numerical accuracy, GPU support, every indirect
dependency or platform support; each worker runs its own checks on packages, inputs, limits
and diagnostics when it starts.

Saved project results can be reused without a working interpreter: Marklab checks the
inputs, worker scripts and shared `marklab_worker_support` sources before returning a
cached result. Changing any shared source invalidates the cached Python-backed results.
Running a new fit needs a working, checked interpreter. Keep the original inputs, worker
files and configuration if you want to reuse saved results later.

## How workers run

Workers start with an empty environment, a fixed Python hash seed and one thread per
numerical library. They run with `python -P -s -B`, which keeps the current directory, the
worker directory and the user's site-packages off the import path and stops Python writing
bytecode files. `PYTHONPATH` and `PYTHONHOME` from your shell are not passed on.

PyMC workers also import `marklab_worker_support.pytensor_config`, which sets two PyTensor options:

- **Reproducible arithmetic.** Numba's `fastmath` is turned off. With it on, freshly
  compiled and cached model code can round differently, so the same seed could give
  different draws depending on what was already cached. Turning it off costs a little
  speed.
- **macOS linker compatibility.** PyTensor passes `-ld64` to the linker on every macOS 15
  or later. Recent Xcode linkers reject that flag, which breaks every compiled model. The
  flag is removed only when a test link with it fails.

PyMC divergence diagnostics count the divergent transitions themselves, not cumulative
counters. Fits made before these changes, including those made when the hash seed setting
was ignored, may differ slightly from new fits; diagnostic thresholds are unchanged.

## Adaptive SPDE convergence

The adaptive polygon-window SPDE commands fit a penalized one-factor spatial field.
They first update field and loadings in blocks, then use L-BFGS-B with the remaining
caller-declared iteration budget. Both phases count toward `maximum_iterations`.
These are within-specimen Research diagnostics, not calibrated patient effects.

L-BFGS-B can report success because the relative objective change is small even if
the gradient is still too large. The adaptive solver uses `ftol = 10 * eps64`
(`factr=10`) and keeps `gtol=1e-7`; it independently requires a finite gradient
infinity norm no greater than `2e-4`. The objective, priors, mesh, acceptance threshold
and result field definitions are unchanged. The fixed holed-window regression formerly
stopped at gradient `3.299134399128434e-4` after 442 iterations; the stricter solve
reached `3.620771430057346e-5` after 506 iterations on the development host, within
the declared 1,000-iteration budget. Exact optimizer paths can vary by numerical platform.
See [SciPy's two stopping criteria](https://docs.scipy.org/doc/scipy/reference/optimize.minimize-lbfgsb.html).

An exhausted or prematurely terminated solve that misses the gradient requirement
fails. Marklab does not widen the limit or publish that state as a successful fit.

## SPDE spatial factor

`multimodal spatial-latent-factor` uses a dense Matérn-3/2 Gaussian-process prior for its one
spatial factor by default. With `"latent_field": {"kind": "spde_alpha_two",
"mesh_resolution": n, "mesh_extension_um": e}` and `"matern_nu": 1.0` it uses the stochastic
partial differential equation (SPDE) prior of Lindgren, Rue and Lindström (2011) instead: a
Matérn field with smoothness 1 represented on a regular `n × n` triangular mesh over the
regions' bounding box widened by `e` µm, with mass-lumped finite-element precision
`Q = τ²(κ⁴C + 2κ²G + GC̃⁻¹G)`, unit marginal variance `τ² = 1/(4πκ²)` and practical range
`√8/κ`, which receives the `HalfNormal(length_scale_prior_um)` prior. The factor at the regions
is `A w` with `A` the barycentric projection from mesh vertices. The mesh extension moves the
natural boundary away from the regions; near the mesh edge the field variance is not exactly
one. The result reports `kernel.family = "spde_matern"`, the mesh size and
`posterior_range_um`; without the option the request and result are unchanged.

The finite-element assembly, rectangular mesh, projection and one-factor MAP fit live in the
shared `marklab_worker_support.fem` module, also used by the `bayes` SPDE commands.

Evidence, with thresholds fixed before the comparisons ran:

- `multimodal validate` entry `spde_factor_recovery` (previously
  `blocked_missing_mesh_owner`): 48 seeded regions in a 1000 µm square, a factor drawn from a
  Matérn-3/2 GP (length scale 300 µm) and four loadings with noise SD 0.25. MAP fits under the
  dense generating prior and under an SPDE prior on a 256-vertex mesh, with range equal to the
  dense kernel's 0.1-correlation distance, must each correlate at least 0.9 with the truth and
  at least 0.95 with each other. For seed 59 the correlations are 0.980 (SPDE–truth), 0.983
  (dense–truth) and 0.992 (SPDE–dense).
- `tests/multimodal_spatial_latent_factor_cli.rs` fits 36 regions in 2-D with both priors by
  PyMC NUTS. The SPDE and dense posterior-mean factors must each correlate at least 0.9 with the
  generating field and at least 0.9 with each other. On the development host both fits were
  complete without divergences (rank R-hat 1.014 dense, 1.021 SPDE); the correlations were
  0.99991 (SPDE–truth), 0.99991 (dense–truth) and 0.99995 (SPDE–dense), and the masked-entry
  RMSE 0.016 for both. The generating field has no added noise, so these correlations are
  near their ceiling.

These are synthetic recovery checks. The smoothness differs between the two priors (1 versus
3/2), so they show recovery of a common smooth factor, not equal priors, and they do not
validate either prior on tissue.

## Platforms

An integration test moves an installed runtime to a new location and checks that it still
runs a fit and reuses saved results without Python. Building Windows or Linux binaries does
not by itself show that every locked Python package works on that platform; that needs the
doctor check and the backend integration tests on the platform.
