# What Marklab can do

Marklab measures how cells are arranged in tissue. This page lists each workflow, the
question it answers and how mature it is. `marklab --help` lists every command, and
`marklab <command> --help` shows the options.

**Core** workflows are the original, most heavily tested part of Marklab, and their output
format ([result format 0.3](result-format-0.3.md)) is stable. A weekly CI job runs 1,000
shuffled-label datasets through the `analyze` and `multimodal` engines to check that false
positives stay at or below the stated rate. **Experimental** workflows
work and are tested against known answers, but have not been validated on real studies,
and their outputs may still change.

## One tissue section

**`marklab analyze`** (core) takes cells with a yes/no label, or a probability, and tests
whether the labeled cells are arranged differently than if the labels were shuffled at
random among the same cells. It reports:

- whether labeled cells sit closer to each other than chance predicts (clustering) or
  farther apart (dispersion), across a range of distances;
- whether the pattern has a preferred direction;
- the size scales at which labeled cells concentrate, from a spectrum of the pattern;
- maps of the regions where labeled cells are over-represented.

**`marklab classical`** and **`marklab nearest-space`** (core) apply standard
point-pattern statistics to all retained cells, ignoring the label: Ripley's K and L
functions, nearest-neighbor distances and empty-space distances, each compared with
completely random placement. See the [classical workflow](classical-spatial-workflow.md).

## Comparing sections

**`marklab prepost`** (core) compares two analyzed sections, such as one taken before
treatment and one after. It describes how each measurement changed. The two sections are
different pieces of tissue, so this is not a test of what happened to the same cells.

**`marklab multimodal`** analyzes an H&E and an IHC section of the same tissue together.
It aligns them, measures how different cell types sit relative to each other, and fits
factor models across the two stains.

## Many patients

**`marklab cohort`** compares groups of patients. The patient is the unit of evidence: a
patient with 10,000 cells counts once, the same as a patient with 500. It supports
paired, blocked and clustered designs, bootstrap intervals, Max-T correction across many
measurements, and equivalence tests against a margin you set in advance.

**`marklab study`** (experimental) runs a whole multiplex panel, with several markers
measured per cell, across slides and patients, from cell tables to a patient-level report.
It can resume after an interruption, and a thin Python client reads AnnData files. See
[multiplex studies](multiplex-study.md).

## Pathology maps and studies

**`marklab pathology maps`** (experimental) measures how marker levels and cell density
change with distance from annotated structures, maps the mix of cell types across the
tissue, and finds local hotspots and outliers for a single marker. It uses annotations and
measurements you supply; it does not draw annotations itself. See
[pathology maps](pathology-maps.md).

**`marklab pathology composition`, `spatial-study` and `scan`** (experimental) compare
cell-type composition and density between patients, fit spatial curves, and scan for
regions enriched in a cell type. They have been checked on 23 synthetic scenarios but not
yet calibrated on real studies, and some statistics for sparse cell types are not
available yet. See [pathology studies](pathology-studies.md).

## Bayesian models

**`marklab bayes`** and **`marklab project`** (experimental) fit Bayesian models with
pinned PyMC and NumPyro environments: patient and slide hierarchies, spatial
point-process models (log-Gaussian Cox processes), Gaussian processes, spatial regression
(CAR, SAR, BYM) and checks of how much a conclusion depends on the prior. Every fit reports
its sampling diagnostics (divergences, R-hat, effective sample size). A fit that does not
converge is published as non-converged instead of being presented as a result. `project`
saves finished fits, so re-running a pipeline skips work that is already done.

[Normal-mean prior sensitivity](normal-mean-prior-sensitivity.md) runs entirely in Rust;
the other models need the [Python environment](python-backends.md).

## Embeddings and tissue structure

Marklab reads feature vectors produced by image models (embeddings) for cells, patches,
regions and slides, checks that they line up with the cell coordinates, and summarizes
how they vary across the tissue. It also measures tissue interfaces and compartments,
such as how cells sit along a tumor–stroma boundary and how fragmented a compartment is.

## Research workflows

3-D tissue (`spatial3d`), graphs (`graph`), topology (`topology`), time series
(`longitudinal`), causal designs (`causal`) and simulation (`simulate`, `neural`) are
research tools. They are useful for method development and are less tested than the
workflows above.

## What every workflow guarantees

- **Reproducible**: the same input, settings and seed give the same result.
- **No silent zeros**: a measurement that cannot be computed (too few cells, region too
  small) is reported as unavailable with the reason.
- **Bounded**: commands have limits on work and memory and stop with an error instead of
  running out of memory.
- **All or nothing**: an output directory appears only when the run has finished.

## Before relying on a result

- A significant pattern shows that the labels are not randomly arranged. It does not
  explain why.
- A non-significant difference does not show that two groups are the same. Use an
  equivalence test with a margin chosen in advance.
- When comparing patients, count patients, not cells.
- The spread of patient scores describes how patients vary. It is not a confidence
  interval for the average.
- `marklab backend doctor` checks that the Python environment is installed correctly. It
  does not check whether a model suits your data.

## Recent corrections

- **September 2026.** Fixes to the anisotropy permutation test, physical-scale
  eligibility, vector predictive scoring, paired embedding posterior predictions and graph
  spectral-band features. Affected joint-model and sparse-graph results are now labeled
  version 2; recompute anything produced with version 1.
- **`bayes advanced-cluster`** reports a k-means summary and a simple binary Gibbs fit. It
  is not a full cluster model. Version 1 mislabeled some outputs as posterior quantities;
  version 2 fixes the labels.
- The gridding-sensitivity check for FFT-based statistics uses a fixed 25% tolerance as a
  rough guide. It is not a formal agreement test.
- A real-data pilot of the joint location–embedding model did not converge. Treat its
  output as diagnostic only.

Windows binaries are built for each release but are not yet tested end to end.
