# Task contract — BAY-JOINT-MARK-01 joint location–categorical-mark model construction

Status: complete

Date: 2026-08-25

Parent requirements: BAY-01, BAY-PP, BAY-MM, WS-43, BAY-MULTITYPE-PAP-01.

## User outcome

`marklab bayes build-joint-location-mark-model` builds a typed exact-grid joint point-location and
conditional categorical-mark model artifact with an explicit random-labeling comparison requirement.

## Frozen behavior

- consume unique finite typed points with location covariate/offset, mark covariate, and declared
  neighborhood effect inside an exact half-open rectangle plus one complete regular midpoint location
  grid; require 2–16 marks with at least two observations each and a supplied reference mark;
- location component is the IC-0062 exact event-sum/minus-grid-integral log-linear process with Normal
  fixed-effect priors; mark component is reference-category softmax over mark covariate,
  neighborhood effect, and independent mark-specific latent-field declarations with positive prior
  scales and explicit identifiability constraints;
- retain exact points/grid/counts/types/reference/units/prior/model identities and declare comparison
  against the nested zero-neighborhood/zero-mark-field random-labeling model mandatory;
- this is model construction only: no posterior, fitted coupling, causal neighborhood effect, or
  biological claim.

## Validation and claims

A two-mark four-point `2x2` fixture must preserve exact mark counts, reference mark, location grid,
joint-likelihood factorization, softmax identifiability, and mandatory random-labeling comparison.
