"""The ``marklab.pymc_worker_request`` envelope shared by the hierarchical PyMC workers.

The count, ordinal, hurdle, crossed/nested, and meta-analysis workers accept the same request
envelope (format, version, PyMC backend identity, and a Bayesian model IR header); most also
share the ``sampling`` and ``diagnostic_policy`` blocks parsed here. Each adds its own data and
``resources``. The NumPyro agreement and SBC workers reuse the PyMC workers' ``validate``.
"""

from __future__ import annotations

import unicodedata
from collections.abc import Collection
from typing import Any

from . import versions
from .nuts_contract import UNBOUNDED_COUNT, nuts_policy, nuts_sampling
from .protocol import exact, obj, validate_backend


def valid_name(value: Any) -> bool:
    """A nonempty identifier of at most 128 characters without surrounding whitespace or
    control characters."""
    return (
        isinstance(value, str)
        and bool(value)
        and len(value) <= 128
        and value == value.strip()
        and not any(unicodedata.category(character) == "Cc" for character in value)
    )


def validate_envelope(
    request: Any,
    lock_digest: str,
    worker_digest: str,
    data_keys: Collection[str],
    model_keys: Collection[str],
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Check the request envelope, PyMC backend identity, and model IR header.

    ``data_keys`` name the request's data fields (for example ``patients``, or ``site_ids`` and
    ``patients``). Returns the request and its model object.
    """
    keys = {"format", "version", "backend", "model", "sampling", "resources", "diagnostic_policy"}
    request = obj(request, keys | set(data_keys), "request")
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "backend",
        name="pymc",
        version=versions.PYMC,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    model = obj(request["model"], model_keys, "model")
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    exact(model["version"], 1, "model.version")
    return request, model


def validate_sampling(value: Any, *, bounded_target_accept: bool = False) -> dict[str, Any]:
    """Parse the NUTS ``sampling`` block; the target acceptance is range-checked on request."""
    return nuts_sampling(value, "sampling", bounded_target_accept=bounded_target_accept)


def validate_policy(value: Any) -> dict[str, Any]:
    """Parse the ``diagnostic_policy`` block; thresholds are applied as given."""
    return nuts_policy(
        value,
        "policy",
        prior_draw_range=(1, 100_000),
        count_ceiling=UNBOUNDED_COUNT,
        check_thresholds=False,
    )
