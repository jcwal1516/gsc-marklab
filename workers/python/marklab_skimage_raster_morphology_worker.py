#!/usr/bin/env python3
"""scikit-image worker for Minkowski functionals of a binary raster under dilation and erosion."""

import json
import math
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))

import numpy as np
import scipy
import skimage
from scipy import ndimage
from skimage.measure import euler_number, perimeter_crofton
from skimage.morphology import disk

from marklab_worker_support.protocol import (
    CONTRACT_ERRORS,
    MIB,
    ContractError,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    write_result,
)


def functionals(mask, pixel_size, connectivity, directions):
    """Area, Crofton perimeter, and Euler characteristic of ``mask`` in physical units."""
    area = float(np.count_nonzero(mask)) * pixel_size**2
    perimeter = float(perimeter_crofton(mask, directions=directions)) * pixel_size
    euler = int(euler_number(mask, connectivity=1 if connectivity == 4 else 2))
    return {
        "area_um2": area,
        "perimeter_um": perimeter,
        "euler_characteristic": euler,
        "normalized_perimeter_per_sqrt_um2": perimeter / math.sqrt(area) if area > 0 else None,
        "area_convention": "foreground_pixel_count_times_pixel_area",
        "perimeter_convention": f"crofton_{directions}_directions_zero_background",
        "connectivity_convention": f"foreground_{connectivity}_connectivity",
        "uncertainty": "none_supplied",
    }


def main():
    require_versions(scikit_image=skimage.__version__, scipy=scipy.__version__)
    request_bytes = read_request(16 * MIB)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.skimage_raster_morphology_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    mask = np.asarray(request["mask"], dtype=bool)
    if mask.ndim != 2 or min(mask.shape) < 3 or mask.size > 4_000_000:
        raise ContractError("mask dimensions are invalid")
    pixel_size = float(request["pixel_size_um"])
    connectivity = request["connectivity"]
    directions = request["crofton_directions"]
    if (
        not math.isfinite(pixel_size)
        or pixel_size <= 0
        or connectivity not in (4, 8)
        or directions not in (2, 4)
    ):
        raise ContractError("raster conventions are invalid")
    radii = request["radii_um"]
    if not radii or radii != sorted(set(radii)) or radii[0] != 0.0:
        raise ContractError("radii must be unique increasing and begin at zero")
    baseline = functionals(mask, pixel_size, connectivity, directions)
    curves = []
    for radius in radii:
        pixels = radius / pixel_size
        if not math.isfinite(radius) or radius < 0 or abs(pixels - round(pixels)) > 1e-12:
            raise ContractError("radii must be nonnegative integer pixel multiples")
        pixel_radius = int(round(pixels))
        if pixel_radius == 0:
            dilated = mask
            eroded = mask
        else:
            footprint = disk(pixel_radius).astype(bool)
            dilated = ndimage.binary_dilation(mask, structure=footprint, border_value=0)
            eroded = ndimage.binary_erosion(mask, structure=footprint, border_value=0)
        curves.append(
            {
                "radius_um": radius,
                "pixel_radius": pixel_radius,
                "dilation": functionals(dilated, pixel_size, connectivity, directions),
                "erosion": functionals(eroded, pixel_size, connectivity, directions),
            }
        )
    result = {
        "format": "marklab.raster_morphology",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(request_bytes),
        "representation": "binary_raster",
        "pixel_size_um": pixel_size,
        "baseline": baseline,
        "curves": curves,
        "claim_status": "experimental_supplied_binary_raster",
    }
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "morphology worker contract error", errors=CONTRACT_ERRORS)
