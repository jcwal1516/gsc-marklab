# Python backend installation and admission

Native analyses require no Python environment. Methods implemented by the pinned Python workers
require Python 3.12 and the existing environment lock in `workers/python/uv.lock`.

Release archives include the worker sources, their lock, and project metadata beside the binary.
Extract the complete archive. Keep `workers/python` together; individual workers can use shared
geometry and statistics modules. Python environments are installed separately because they contain
platform-specific executables and native packages.

## Install in a writable extracted bundle or source checkout

Run these commands from the bundle or checkout root with uv available:

```sh
export MARKLAB_RUNTIME_ROOT="$(pwd)"
export UV_PROJECT_ENVIRONMENT="$MARKLAB_RUNTIME_ROOT/target/pymc-venv"
uv sync --project "$MARKLAB_RUNTIME_ROOT/workers/python" --locked --python 3.12
export MARKLAB_PYTHON="$UV_PROJECT_ENVIRONMENT/bin/python"
./marklab backend doctor
```

In a source checkout, use `target/debug/marklab backend doctor` after building. On Windows,
the interpreter is `Scripts/python.exe` under the virtual environment; set the corresponding
absolute `MARKLAB_PYTHON` path and invoke `marklab.exe`.

uv's [locked synchronization](https://docs.astral.sh/uv/concepts/projects/sync/) keeps the committed
environment definition authoritative. Marklab does not install or upgrade packages automatically.

For a read-only installation, create the environment in a writable location by setting
`UV_PROJECT_ENVIRONMENT` before synchronization. Set `MARKLAB_PYTHON` to that environment's
interpreter and `MARKLAB_BACKEND_CACHE` to a writable absolute directory.

## Resolution and failure behavior

| Setting | Behavior |
|---|---|
| `MARKLAB_RUNTIME_ROOT` | Absolute directory containing `workers/python/uv.lock` and `pyproject.toml`. An invalid explicit override fails without falling back. |
| No runtime-root override | Prefer workers beside the executable. Permit the build checkout only while the executable is inside that checkout's `target` directory. |
| `MARKLAB_PYTHON` | Absolute interpreter path. Otherwise use the platform-specific `target/pymc-venv` interpreter under the resolved runtime root. |
| `MARKLAB_BACKEND_CACHE` | Absolute writable PyTensor compilation-cache directory. Defaults to `target/pymc-cache` under the runtime root. |
| `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION` | Retains the existing prohibition on starting a worker. Verified durable cache hits can still replay. |

`backend doctor` checks Python 3.12, installed direct package versions against their exact project
pins, and their presence in the lock. It reports the selected paths. It does not certify numerical
accuracy, GPU support, every transitive package, or platform support. Each analysis worker retains
its additional package, schema, digest, resource, and diagnostic admission checks.

Source preparation and durable replay resolve and verify assets independently of interpreter
availability. A cache miss requires a working admitted interpreter; a verified hit does not.
Keep exact source inputs, worker assets, and configuration available for replay.

Scientific workers start with a cleared environment, fixed Python hash seed and thread controls.
Explicit `-P -s -B` flags exclude current/script and user import paths and disable bytecode writes;
ambient `PYTHONPATH` and `PYTHONHOME` are not inherited. Backend-specific compilation settings remain
owned by their existing runners. The corrected PyMC diagnostic extraction counts per-transition
divergence flags rather than summing cumulative counters. These changes retain diagnostic thresholds;
fresh fits may differ from historical runs whose declared hash seed was ignored.

Runtime relocation and backend-free replay have a focused integration test. Packaging Windows or
Linux binaries does not itself establish that every locked Python package or scientific backend is
supported on those platforms; backend admission and integration evidence remain necessary.
