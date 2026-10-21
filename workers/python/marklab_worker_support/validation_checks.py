"""Rows of the synthetic validation reports produced by the validation-suite workers."""

from __future__ import annotations

from typing import Any


def entry(
    validation_id: str, status: str, metric: str, value: Any, threshold: Any, evidence: str
) -> dict[str, Any]:
    """One report row: the check, its status, the measured value, and the evidence used."""
    return {
        "validation_id": validation_id,
        "status": status,
        "metric": metric,
        "value": value,
        "threshold": threshold,
        "evidence": evidence,
    }


def pass_fail(passed: bool) -> str:
    """The row status of a check that either passed or failed."""
    return "passed" if passed else "failed"
