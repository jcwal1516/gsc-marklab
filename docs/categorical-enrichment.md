# Categorical neighborhood enrichment

**Experimental** for observed counts and exact fixed-count null descriptors;
**Research** for the Monte Carlo inferential outputs.

This workflow counts undirected edges between each pair of cell classes in a fixed
Euclidean radius graph. It asks whether labels share edges more often than under a
uniform reassignment of the observed class counts to the same cells. Coordinates,
the observation window, the graph and class totals remain fixed. The null has one
global exchangeability stratum; it does not adjust for tissue compartments, spatial
gradients, patients or technical batches.

The request is strict JSON with format `marklab.categorical_enrichment`, version
`1`. It uses the same physical frame, inline observation window, categorical
channel and cell declarations as [categorical co-occurrence](categorical-cooccurrence.md).
Instead of several radii, it supplies one positive finite `radius_um`, a positive
`permutations` count and a `seed`. Explicit limits bound input bytes, points,
classes, retained edges, permutations, permutation work and retained
memory. Permutation work counts one label visit per cell plus one count visit per edge,
for every permutation; isolated cells therefore still consume this bound. Distinct cells at the same coordinate are neighbors; self-edges are
excluded. The radius boundary is inclusive.

```bash
target/debug/marklab numerics categorical-enrichment \
  --input /absolute/path/request.json \
  --out /absolute/new/path/result.json
```

The result is a separate version-1 artifact with format
`marklab.categorical_enrichment_result`. It retains frame/channel IDs, ordered
classes and class counts, radius, graph edge-pair counts, permutation count, seed,
and one row for each unordered class pair, including same-class pairs. Pair labels
are in lexical order. Existing multimodal result-format 0.4 enrichment fields keep
their definitions.

## Exact fixed-count null moments

For a particular class pair, write `X` for its undirected edge count, `M` for the
graph's number of edges, `W` for the number of unordered edge pairs sharing a node,
and `D = choose(M, 2) - W` for the number of disjoint edge pairs. For a simple graph,
`W = sum_v choose(degree(v), 2)`. If `p` is the probability that one edge has the
requested labels, `q` the probability that two adjacent edges both do, and `r` the
corresponding probability for two disjoint edges, then

```text
E[X]   = M p
Var[X] = M p + 2 W q + 2 D r - (M p)^2.
```

Let `(a)_j` denote a falling factorial and `N` the number of cells. For a same-class
pair with class count `a`, the probabilities are `(a)_2/(N)_2`, `(a)_3/(N)_3` and
`(a)_4/(N)_4`. For two different classes with counts `a` and `b`, they are
`2ab/(N)_2`, `[a(b)_2 + b(a)_2]/(N)_3`, and `4(a)_2(b)_2/(N)_4`. Terms involving
more distinct cells than exist contribute zero because the corresponding edge
pair cannot occur. This edge-indicator calculation is the fixed-count join-count
null discussed by [Moran (1948), section 6](https://doi.org/10.1111/j.2517-6161.1948.tb00012.x).

The native calculation combines checked integer terms before converting the final
mean and variance to floating point. It therefore distinguishes exact zero
variance from cancellation in `E[X²] - E[X]²`. The intermediate products use
256-bit integers, which cover every input within the 16 MiB request cap; a
regression checks whole-slide counts of up to 400,000 cells against exact
rational Cliff–Ord moments.
`analytic_null_mean` and `analytic_null_variance` are population moments over the
entire finite label-allocation space. `analytic_z_score` is
`(observed - mean) / sqrt(variance)`. Zero variance produces JSON `null` with
reason `zero_null_variance`. This standardized descriptor is not a Gaussian
p-value, a calibrated test or evidence of a biological interaction.

## Monte Carlo estimates and inference

The separately named `monte_carlo` object uses the existing enrichment result
definition: permutation-average expected edges, observed/expected ratio, sample
standard deviation (`ddof=1`) for the z-score, inclusive one-sided enrichment
p-value `(1 + number(null >= observed)) / (B + 1)`, and Benjamini–Hochberg adjusted
q-values across the declared pair family. A new domain-separated deterministic
seed stream belongs to this workflow. Existing multimodal streams are unchanged.
Every permutation shuffles complete categorical labels across all retained cells.

A finite-sample z-score can be unavailable even when the exact variance is
positive, if the sampled counts have zero variance. The result keeps these cases
separate. Depletion is not tested by the one-sided enrichment p-value. Dependence
between overlapping pair tests means BH adjustment alone does not establish FDR
control for every spatial labeling process. Neither a small p-value nor a large
z-score identifies a causal cell-cell interaction. Radius selection after looking
at results introduces an additional selection problem that this workflow does not
correct.

## Reference comparison

[`validate_neighborhood_enrichment.py`](../examples/public-cohorts/validate_neighborhood_enrichment.py)
uses the openly licensed Schürch colorectal CODEX table preserved by
[`schurch.py`](../examples/public-cohorts/schurch.py). It fixes the 1,107 non-dirt
cells and 19 classes in `reg001_A`, the published physical calibration, a 25 µm
radius, 9,999 permutations and seed 42. The resulting graph has 5,833 undirected
edges, 89,271 adjacent edge pairs and 16,919,757 disjoint edge pairs.

Observed counts must match Squidpy 1.8.3 exactly. Squidpy's symmetric directed
matrix counts a same-class edge twice; its off-diagonal count equals the native
undirected count. Exact mean, variance and standardized descriptors are compared
to the same edge-pair moment formulas evaluated separately in Python rational
arithmetic, with prespecified scaled error at most `1e-12`, using
`max(1, abs(reference))` as scale. Those formulas were derived for this workflow, so
this comparison checks the native arithmetic rather than the derivation. The derivation
is checked separately: exact answers for small path, complete and edgeless graphs in the
CLI tests, and the textbook Cliff–Ord form in terms of S0, S1 and S2 in the whole-slide
regression.

Squidpy uses its own shuffle stream and population standard deviation (`ddof=0`)
for its simulated z-score. The script reports both 9,999-permutation and default
1,000-permutation outputs, with a fixed seed and one worker. Differences from
native Monte Carlo z-scores are descriptive: agreement is explicitly **unassessed**,
with no tolerance fitted to the observed differences. Earlier Python emulations
and seed sweeps are exploratory and provide no native acceptance evidence.

```bash
target/spatial-reference-venv/bin/python \
  examples/public-cohorts/validate_neighborhood_enrichment.py \
  /absolute/path/schurch-preserved-source \
  /absolute/new/path/enrichment-validation \
  --binary /absolute/path/target/debug/marklab
```

The native run passed exact Squidpy count agreement for all 190 unordered class pairs. The maximum
scaled error across the exact moments and standardized descriptors was
`2.840887189544699e-16`, below the unchanged `1e-12` limit. The largest descriptive
Monte Carlo z-score difference was `0.3949132059626592`; it is not an agreement pass.
Squidpy emitted its zero-variance division warning, and nonfinite package z-scores are
recorded as unavailable. The public graph is one tissue field, not multi-shape calibration or a
patient-group inference study. The existing multimodal source-section and explicit
stratum nulls require their own evidence; this single-stratum comparison does not
validate them.

## Comparison with imcRtools and SPIAT

[`compare_imcrtools_spiat.R`](../tests/fixtures/interaction_tools/compare_imcrtools_spiat.R)
runs this workflow and two widely used interaction tools on three public Schürch et al. spots
(`reg001_A` to `reg003_A`, non-dirt cells, 25 µm radius, 9,999 permutations). Criteria were
fixed beforehand.

- **imcRtools 1.16.0 `testInteractions` (histoCAT-style, `method = "classic"`):** its
  statistic for an ordered class pair (A, B) is the number of B neighbors per A cell, i.e.
  this workflow's undirected count divided by `n_A` (doubled for A = B). Its `p_gt` is a
  one-sided label-permutation p-value for that statistic.
  - Graphs agree: imcRtools' directed edges are exactly twice Marklab's undirected edges
    (5,833, 8,851 and 7,454).
  - The classic statistic agrees within **3.6e-15** for all 1,163 ordered pairs.
  - The p-values come from independent streams of 9,999 draws. They differ by at most
    **0.0158** (mean 0.0019; limit 0.03), so the two permutation tests agree to Monte Carlo
    precision.
  - The `"histocat"` and `"patch"` variants average over different cell sets and were not
    compared.
- **SPIAT 1.12.1 `mixing_score_summary`:** for every ordered pair of distinct types among
  tumor cells, CD8+ T cells, CD68+CD163+ macrophages and B cells (36 pairs), its
  reference–target and reference–reference interaction counts equal this workflow's
  undirected counts. The mixing score (their ratio) and normalized mixing score are
  identical in all 33 defined cases. In the other 3, B cells in `reg002_A` have no B–B pairs,
  so the ratio is undefined; SPIAT reports 0 there.

Both packages ran as external oracles from a separate R library. SpatialExperiment, EBImage,
sf and terra needed ImageMagick 7.1.2-31, fftw 3.3.10, udunits 2.2.28, fribidi 1.0.16 and
GDAL 3.12.4. These were built from source under `target/native-prefix` because the
machine's Homebrew GDAL could not load.
