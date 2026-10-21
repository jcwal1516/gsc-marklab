"""Read-only check of the installed Python runtime against its direct package pins."""

import re
import sys
import tomllib
from importlib.metadata import version
from pathlib import Path


def main():
    if sys.version_info[:2] != (3, 12):
        raise ValueError("the pinned backend requires Python 3.12")
    root = Path(__file__).resolve().parent
    project = tomllib.loads((root / "pyproject.toml").read_text())
    lock = tomllib.loads((root / "uv.lock").read_text())
    for requirement in project["project"]["dependencies"]:
        match = re.fullmatch(r"([A-Za-z0-9_.-]+)(?:\[[A-Za-z0-9_,.-]+\])?==([^ ;]+)", requirement)
        if match is None:
            raise ValueError(f"backend requirement is not an exact supported pin: {requirement}")
        name, expected = match.groups()
        if not any(
            package["name"] == name and package["version"] == expected
            for package in lock["package"]
        ):
            raise ValueError(f"{name}=={expected} is absent from the environment lock")
        observed = version(name)
        if observed != expected:
            raise ValueError(f"{name} version drift: expected {expected}, observed {observed}")
    pin_count = len(project["project"]["dependencies"])
    print(f"Python 3.12 found; {pin_count} direct package pins verified")
    print(
        "Individual analysis workers additionally validate their own package, schema, "
        "and resource contracts."
    )


if __name__ == "__main__":
    main()
