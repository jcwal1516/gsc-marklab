#!/usr/bin/env python3

import hashlib
import json
import math
import sys

import numpy as np
import scipy
import skimage
from scipy import ndimage
from skimage.measure import euler_number, perimeter_crofton
from skimage.morphology import disk


class ContractError(Exception):
    pass


def functionals(mask, pixel_size, connectivity, directions):
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
    if skimage.__version__ != "0.26.0" or scipy.__version__ != "1.18.1":
        raise ContractError("scikit-image or SciPy version drift")
    if sys.version_info[:2] != (3, 12):
        raise ContractError("Python version drift")
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.skimage_raster_morphology_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    mask = np.asarray(request["mask"], dtype=bool)
    if mask.ndim != 2 or min(mask.shape) < 3 or mask.size > 4_000_000:
        raise ContractError("mask dimensions are invalid")
    pixel_size = float(request["pixel_size_um"])
    connectivity = request["connectivity"]
    directions = request["crofton_directions"]
    if not math.isfinite(pixel_size) or pixel_size <= 0 or connectivity not in (4, 8) or directions not in (2, 4):
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
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "representation": "binary_raster",
        "pixel_size_um": pixel_size,
        "baseline": baseline,
        "curves": curves,
        "claim_status": "experimental_supplied_binary_raster",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
        print(f"morphology worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
