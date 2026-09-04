"""Read-only admission of the installed Python runtime's direct package requirements."""

from importlib.metadata import version
from pathlib import Path
import re
import sys
import tomllib


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
        if not any(package["name"] == name and package["version"] == expected for package in lock["package"]):
            raise ValueError(f"{name}=={expected} is absent from the environment lock")
        observed = version(name)
        if observed != expected:
            raise ValueError(f"{name} version drift: expected {expected}, observed {observed}")
    print(f"Python 3.12 admitted; {len(project['project']['dependencies'])} direct package pins verified")
    print("Individual analysis workers additionally validate their own package, schema, and resource contracts.")


if __name__ == "__main__":
    main()
