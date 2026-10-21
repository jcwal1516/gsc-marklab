#!/usr/bin/env python3
"""Preserve the existing openly licensed Aperio validation fixture outside the checkout.

The required digest comes from tests/fixtures/wsi/manifest.json. Verification is streamed
once during download, as required by the existing public WSI test contract.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import urllib.request


BASE = "https://openslide.cs.cmu.edu/download/openslide-testdata/"
KEY = "Aperio/JP2K-33003-1.svs"
LICENSE = "Free to use and distribute, with or without modification"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path, help="fresh external dataset directory")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    output = args.out.resolve()
    if output == root or root in output.parents:
        raise ValueError("public raw inputs must remain outside the checkout")
    if output.exists():
        raise ValueError("raw dataset directory must be new")
    manifest = json.loads((root / "tests/fixtures/wsi/manifest.json").read_text())
    # Reuse the reviewed fixture's exact identity; do not create a new hash contract.
    entries = manifest["fixtures"]
    fixture = next(entry for entry in entries if entry["name"] == "public_aperio_jp2k_svs")
    with urllib.request.urlopen(BASE + "index.json", timeout=60) as response:
        metadata = json.loads(response.read(1024 * 1024))
    source = metadata[KEY]
    if source["license"] != "distributable" or source["sha256"] != fixture["sha256"]:
        raise ValueError("publisher license or existing required fixture identity changed")
    size = int(source["size"])
    if not 0 < size <= 80 * 1024 * 1024:
        raise ValueError("public fixture exceeds the reviewed download bound")
    output.mkdir(parents=True)
    path = output / Path(KEY).name
    count, digest = 0, hashlib.sha256()
    with urllib.request.urlopen(BASE + KEY, timeout=120) as response, path.open("xb") as stream:
        while chunk := response.read(1024 * 1024):
            count += len(chunk)
            if count > size:
                raise ValueError("source exceeds the publisher-declared size")
            digest.update(chunk)
            stream.write(chunk)
    if count != size or digest.hexdigest() != fixture["sha256"]:
        raise ValueError("download fails the existing public WSI fixture contract")
    with (output / "source.json").open("x") as stream:
        json.dump({"url": BASE + KEY, "license": LICENSE,
                   "license_source": BASE + "Aperio/", "bytes": count}, stream, indent=2)
        stream.write("\n")
    print(f"Preserved {path}; {count} bytes; existing fixture identity verified")


if __name__ == "__main__":
    main()
