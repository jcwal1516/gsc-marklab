# Task contract — TOP-MORPH-01 raster Minkowski and morphology

Status: complete

Date: 2026-08-25

Owns `MinkowskiFunctionals2D` and `MorphologicalFunctionalCurve` for supplied binary rasters through
pinned scikit-image 0.26.0/SciPy 1.18.1. `marklab topology raster-morphology` reports pixel area,
Crofton perimeter, declared-connectivity Euler characteristic, and disk dilation/erosion curves.
The single-pixel exact area/Euler/dilation/erosion oracle passes; no segmentation validity is claimed.
