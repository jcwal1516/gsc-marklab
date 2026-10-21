"""Request validation and result output shared by the workers.

A worker reads one JSON request from stdin, validates it against an exact contract, and writes
one canonical JSON result (no NaN or infinity, compact separators) to stdout. On failure it
prints one line to stderr and exits with status 2. The Rust caller computes the same file
digests that ``worker_digests`` computes and places them in the request's backend block.
"""

from __future__ import annotations

import hashlib
import json
import math
import sys
from collections.abc import Callable, Collection
from pathlib import Path
from typing import Any, NoReturn

from . import versions

MIB = 1024 * 1024

DEFAULT_MAX_OUTPUT_BYTES = 16 * MIB
"""Output ceiling for workers whose contract sets none; equals the Rust transport limit."""

CONTRACT_ERRORS = (KeyError, TypeError, ValueError)
"""Failures that contract-checking workers report; other exceptions propagate unchanged."""

BACKEND_KEYS = frozenset(
    {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"}
)


class ContractError(ValueError):
    """The request, a derived input, or a result violates the worker contract."""


def exact(value: Any, expected: Any, path: str) -> None:
    """Require ``value == expected``."""
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def obj(value: Any, keys: Collection[str], path: str) -> dict[str, Any]:
    """Require a JSON object whose field names are exactly ``keys``."""
    if not isinstance(value, dict) or set(value) != set(keys):
        actual = set(value) if isinstance(value, dict) else set()
        missing = sorted(set(keys) - actual)
        unknown = sorted(actual - set(keys))
        raise ContractError(f"{path} fields differ: missing={missing}, unknown={unknown}")
    return value


def number(value: Any, path: str, low: float = -math.inf, high: float = math.inf) -> float:
    """Require a finite JSON number (not a boolean) in ``[low, high]``; return it as a float."""
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    result = float(value)
    if not math.isfinite(result):
        raise ContractError(f"{path} must be finite")
    if not low <= result <= high:
        raise ContractError(f"{path} must be in [{low}, {high}]")
    return result


def integer(value: Any, path: str, low: int, high: int) -> int:
    """Require a JSON integer (not a boolean) in ``[low, high]``."""
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{path} must be an integer in [{low}, {high}]")
    return value


def sha256_digest(value: Any, path: str) -> str:
    """Require a lowercase hexadecimal SHA-256 digest."""
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ContractError(f"{path} must be a lowercase SHA-256 digest")
    return value


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def file_sha256(path: str | Path) -> str:
    return sha256_hex(Path(path).read_bytes())


def worker_digests(worker_file: str | Path) -> tuple[str, str]:
    """Return the SHA-256 of ``uv.lock`` beside the worker and of the worker file itself."""
    worker = Path(worker_file)
    return file_sha256(worker.with_name("uv.lock")), file_sha256(worker)


def python_version() -> str:
    return f"{sys.version_info.major}.{sys.version_info.minor}"


def require_versions(**observed: str) -> None:
    """Refuse to run unless Python and each named package match ``versions.PINS``.

    Keywords name ``versions.PINS`` entries and give the version found at run time, for
    example ``require_versions(numpyro=numpyro.__version__, jax=jax.__version__)``.
    """
    if python_version() != versions.PYTHON:
        raise ContractError(
            f"backend version drift: Python expected {versions.PYTHON}, found {python_version()}"
        )
    for package, found in observed.items():
        expected = versions.PINS[package]
        if found != expected:
            raise ContractError(
                f"backend version drift: {package} expected {expected}, found {found}"
            )


def validate_backend(
    value: Any,
    path: str,
    *,
    name: str,
    version: str,
    lock_sha256: str,
    worker_sha256: str,
) -> dict[str, Any]:
    """Validate a request backend block against this worker's identity."""
    backend = obj(value, BACKEND_KEYS, path)
    exact(backend["name"], name, f"{path}.name")
    exact(backend["version"], version, f"{path}.version")
    exact(backend["python_version"], versions.PYTHON, f"{path}.python_version")
    exact(backend["environment_lock_sha256"], lock_sha256, f"{path}.environment_lock_sha256")
    exact(backend["worker_sha256"], worker_sha256, f"{path}.worker_sha256")
    return backend


def backend_identity(name: str, version: str, lock_sha256: str, worker_sha256: str) -> dict:
    """The backend block a worker reports in its result."""
    return {
        "name": name,
        "version": version,
        "python_version": python_version(),
        "environment_lock_sha256": lock_sha256,
        "worker_sha256": worker_sha256,
    }


def derive_seed(domain: str, *parts: object, width: int = 4) -> int:
    """Derive a sampler seed from a request seed.

    The seed is the first ``width`` bytes, read little-endian, of the SHA-256 of ``domain`` and
    ``parts`` (formatted with ``str``) joined by NUL characters. ``domain`` names the worker and
    contract version so that seeds are independent across workers.
    """
    message = "\0".join([domain, *(str(part) for part in parts)])
    return int.from_bytes(hashlib.sha256(message.encode()).digest()[:width], "little")


def read_request(max_bytes: int) -> bytes:
    """Read the raw request from stdin, rejecting empty input and input above ``max_bytes``."""
    raw = sys.stdin.buffer.read(max_bytes + 1)
    if not raw:
        raise ContractError("request is empty")
    if len(raw) > max_bytes:
        raise ContractError(f"request exceeds the {max_bytes}-byte input limit")
    return raw


def write_result(
    result: Any,
    *,
    max_bytes: int = DEFAULT_MAX_OUTPUT_BYTES,
    sort_keys: bool = True,
    newline: bool = True,
) -> None:
    """Write ``result`` as compact JSON, refusing NaN, infinity, and output above ``max_bytes``.

    ``max_bytes`` bounds every byte written, including the optional trailing newline.
    """
    encoded = json.dumps(result, allow_nan=False, sort_keys=sort_keys, separators=(",", ":"))
    payload = (encoded + "\n" if newline else encoded).encode()
    if len(payload) > max_bytes:
        raise ContractError(f"result exceeds the {max_bytes}-byte output limit")
    sys.stdout.flush()
    sys.stdout.buffer.write(payload)
    sys.stdout.buffer.flush()


def run_worker(
    main: Callable[[], int | None],
    failure_label: str,
    *,
    errors: tuple[type[BaseException], ...] = (Exception,),
) -> NoReturn:
    """Run ``main`` and exit with its status.

    An exception of a type in ``errors`` is reported on stderr as one line prefixed with
    ``failure_label`` and ends the process with status 2. Other exceptions propagate.
    """
    try:
        status = main()
    except errors as error:
        print(f"{failure_label}: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
    raise SystemExit(status)
