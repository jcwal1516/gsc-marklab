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
| `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION` | Never start a Python worker. Saved results from earlier project runs can still be reused. |

## Checking the installation

`marklab backend doctor` checks that the interpreter is Python 3.12 and that each directly
required package is installed at its pinned version and listed in the lock file. It also
prints the paths in use. It does not check numerical accuracy, GPU support, every indirect
dependency or platform support; each worker runs its own checks on packages, inputs, limits
and diagnostics when it starts.

Saved project results can be reused without a working interpreter: Marklab checks the
inputs and worker files and returns the cached result. Running a new fit needs a working,
checked interpreter. Keep the original inputs, worker files and configuration if you want
to reuse saved results later.

## How workers run

Workers start with an empty environment, a fixed Python hash seed and one thread per
numerical library. They run with `python -P -s -B`, which keeps the current directory, the
worker directory and the user's site-packages off the import path and stops Python writing
bytecode files. `PYTHONPATH` and `PYTHONHOME` from your shell are not passed on.

PyMC workers also import `marklab_pytensor_config.py`, which sets two PyTensor options:

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

## Platforms

An integration test moves an installed runtime to a new location and checks that it still
runs a fit and reuses saved results without Python. Building Windows or Linux binaries does
not by itself show that every locked Python package works on that platform; that needs the
doctor check and the backend integration tests on the platform.
