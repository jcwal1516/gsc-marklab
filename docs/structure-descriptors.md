# Point-pattern structure descriptors

These commands describe how cells, or a mask drawn from them, are arranged in a declared
tissue window, and how two cell measurements depend on each other across it. Each is at the
**Research** tier of the [evidence ladder](capabilities.md): it has known-answer tests and a
reference comparison, documented below, but no calibrated patient-level inference and no
claim about what the arrangement or field means biologically. The Rust API is
`marklab::structure`.

## Morisita quadrat dispersion (`numerics morisita-dispersion`)

Measures whether cells of one type crowd into some parts of the tissue (clumping) or spread
evenly, by counting them in equal square quadrats.

- **Quadrats:** squares of the declared width tile the plane from the declared origin. Only
  quadrats lying wholly inside the window count, so all counted quadrats have the same tissue
  area; a quadrat touching the boundary is clipped exactly and kept only if its clipped area
  equals its full area. Cells are assigned to the half-open quadrat `[a, a + w) × [b, b + w)`;
  cells in excluded quadrats are reported as `excluded_cells`, not counted.
- **Index:** with `q` counted quadrats, counts `n_i` and `N = Σ n_i`,
  `I = q Σ n_i(n_i − 1) / (N(N − 1))`. `I ≈ 1` for random placement, `I > 1` for clumping
  and `I < 1` for regularity.
- **Standardized index and test:** the uniform and clumped indices
  `M_u = (χ²₀.₀₂₅ − q + N)/(N − 1)` and `M_c = (χ²₀.₉₇₅ − q + N)/(N − 1)` (`q − 1` degrees of
  freedom) give the Smith-Gill (1975) standardized index on `[−1, 1]`, where values above
  0.5 or below −0.5 fall outside the 95% Poisson range. The chi-square statistic
  `I(N − 1) + q − N` with `q − 1` degrees of freedom tests the Poisson-quadrat null; the upper
  tail is reported. When none of the four standardization cases applies, Marklab reports the
  standardized index as unavailable, whereas vegan returns `I` itself.
- **Assumes:** the quadrat width and origin are chosen before looking at the result. The index
  depends on them, as it does on the window: a single width describes a single scale.
- **Does not establish:** a mechanism. The chi-square test assumes independent Poisson counts;
  cells within a quadrat are not independent tissue samples, and the test is not a patient
  comparison.
- **Evidence:** hand-counted indices and degenerate inputs (unit tests); exact quadrat
  inclusion on rectangular, shifted and L-shaped windows (`tests/structure_descriptors.rs`).
  [`compare_vegan.R`](../tests/fixtures/morisita/compare_vegan.R) compares the command with
  `vegan::dispindmorisita` 2.7-6 on public Schürch et al. CODEX cells (CC BY 4.0): B cells,
  CD8⁺ T cells and CD68⁺CD163⁺ macrophages in the first ten TMA A spots, 100 µm quadrats from
  the image corner, 35 quadrats per spot, with R counting the quadrats independently. Tolerances
  fixed before the run: identical counts; index and chi-square statistic 1e-12 scaled; derived
  indices 1e-8 scaled; p-value 1e-8 absolute. All 30 cases passed, with largest errors
  2.0e-16 (index), 3.8e-16 (statistic), 3.4e-15 (derived indices) and 5.8e-15 (p-value).

## Ordinary kriging (`numerics ordinary-kriging`)

Predicts a cell-level value, such as a marker intensity, at new locations from nearby cells,
and checks those predictions on held-out tissue blocks.

- **Variogram:** declared, not fitted by this command: exponential
  `γ(h) = b + c(1 − e^{−h/a})`, spherical `b + c(1.5h/a − 0.5(h/a)³)` up to `h = a` and
  `b + c` beyond, or Gaussian `b + c(1 − e^{−h²/a²})`, with nugget `b`, partial sill `c`,
  range parameter `a` and `γ(0) = 0`. Fit it beforehand, for example to Marklab's
  [empirical variogram](variograms.md), and use the held-out check to judge it.
- **Prediction:** the observations within the declared search radius, nearest first (ties by
  input order), up to `maximum_neighbors`; fewer than `minimum_neighbors` leaves the prediction
  unavailable with the count. The ordinary-kriging system `[Γ 1; 1ᵀ 0][λ; μ] = [γ₀; 1]` gives
  the weights; the kriging variance `Σλᵢγ₀ᵢ + μ` is the error variance for a new observation and
  includes the nugget. A singular system is reported, not regularized.
- **Held-out check:** square blocks of the declared width tile the plane from the declared
  origin. Each nonempty block is left out in turn and its cells are predicted from the rest.
  The summary gives RMSE, the mean error `z − ẑ`, and the mean and mean square of the
  standardized errors `(z − ẑ)/σ`, with coverage of `ẑ ± 1.96σ`. A well-specified model has a
  mean squared standardized error near 1 and coverage near 0.95.
- **Assumes:** a constant unknown mean (no trend) and a stationary isotropic variogram within
  the search radius. Block size controls how far predictions must extrapolate; it should be
  chosen before looking at the result.
- **Does not establish:** that the field is stationary, that one variogram describes all
  patients, or anything about cells that were not measured.
- **Evidence:** closed-form unit tests (symmetric pair weights and variance; exact
  interpolation without nugget; insufficient neighbors) and a block hold-out test in which each
  block is predicted exactly by the other block's constant value
  (`tests/structure_descriptors.rs`). [`compare_skgstat.py`](../tests/fixtures/kriging/compare_skgstat.py)
  compares the command with SciKit-GStat 1.0.23 `OrdinaryKriging` on 600 public Schürch et al.
  cells (log1p CD45 intensity, spot reg001_A): an exponential variogram with SciKit-GStat
  effective range 150 µm (Marklab `a` = 50 µm), partial sill equal to the sample variance and
  nugget one tenth of it, 5–15 neighbors within 150 µm, 100 µm blocks and 300 grid targets.
  The tolerance, fixed before the first run, was 1e-8 scaled error. The first run used
  SciKit-GStat's default fit (effective range 5.9 µm), which left both programs without
  neighbors, so nothing was compared; the second omitted SciKit-GStat's `use_nugget=True`, so the
  reference dropped the nugget and the two programs fitted different models. The third run,
  with the declared model adopted by both, agreed on availability, and on predictions
  (≤4.2e-14), kriging variances (≤4.9e-15) and the held-out summary (≤6.5e-16). Its held-out
  mean squared standardized error, 1.37, and 95% coverage, 0.878, show that this declared,
  unfitted variogram understates prediction uncertainty for CD45, which is the kind of misfit
  the check is meant to expose.

## Bond-orientational order (`numerics bond-orientational-order`)

Measures how regularly the directions to a cell's neighbors repeat, for example the
six-fold arrangement of closely packed epithelium.

- **Definition:** `ψ_k(i) = (1/n_i) Σ_j e^{ikφ_ij}`, with `φ_ij` the angle of the vector from
  cell `i` to neighbor `j` measured from the x axis. `|ψ_k(i)|` is 1 when bonds repeat every
  `2π/k` and near 0 when they are disordered; its phase is the local lattice orientation.
  The summary reports the mean of `|ψ_k|` (local order) and `|mean ψ_k|` (orientation shared
  across the pattern).
- **Neighbors:** the declared number of nearest cells (distance ties at the cutoff are broken
  by input order and flagged per cell) or all cells within a declared radius.
- **Border rule:** a cell is included only when its distance to the window boundary is at least
  its farthest neighbor distance (or the radius), because unobserved cells beyond the boundary
  could change its neighbors. Excluded cells are reported, not averaged.
- **Assumes:** cell centroids represent cell positions and the neighbor rule is chosen before
  looking at the result; `k` should match the arrangement being asked about.
- **Does not establish:** a crystal-like tissue state. Cell sizes, packing and segmentation all
  affect bond angles, and there is no calibrated test against a null arrangement here.
- **Evidence:** a rotated triangular lattice gives `ψ₆ = e^{6iθ}` for every included cell and
  a square lattice gives `|ψ₄| = 1` and `|ψ₆| = 0`, with the outer ring excluded
  (`tests/structure_descriptors.rs`, within 1e-12). [`compare_freud.py`](../tests/fixtures/bond_order/compare_freud.py)
  compares the command with freud 3.6.1 `order.Hexatic` on tumor cells in three public Schürch
  et al. spots, with the 6 nearest neighbors and with all neighbors within 25 µm (six cases,
  734 compared cells). freud stores positions and results in single precision, which made the
  original 1e-12 criterion against its values unattainable; this was found when the first run
  stopped before reporting, and the criteria were revised without inspecting any comparison
  value. Result: ψ₆ evaluated in double precision on freud's own neighbor lists equals
  Marklab's within **2.2e-16** in all six cases, and neighbor counts match, so the neighbor sets
  and formula agree. Against freud's own single-precision values, the differences, up to
  **7.0e-4**, exceeded the a priori single-precision bound in five of six cases, so that part of
  the comparison **failed**. Rounding the positions to single precision changes ψ₆ by at most
  9.7e-6, so the remaining difference arises inside freud's single-precision evaluation rather
  than in Marklab.

## Box-counting dimension and lacunarity (`numerics lacunarity-fractal-dimension`)

Describes how a declared binary mask, such as pixels holding tumor cells, fills space across
scales: how many boxes it touches as boxes grow, and how unevenly its mass is spread.

- **Input:** rows of `0` and `1` (first row at the top), a pixel size in µm, a stated
  foreground definition and increasing box sizes in pixels, the largest at most the shorter
  side. Rasterizing cells or tissue into the mask is the caller's declared design.
- **Box counting:** boxes of side `s` tile the mask from its top-left pixel; boxes past the
  right or bottom edge cover only the pixels inside. `N(s)` counts boxes with any foreground.
  The dimension is the negative least-squares slope of `ln N(s)` on `ln s` over scales with
  `N > 0`, reported with the intercept and `R²`; `R²` is unavailable when `N` is the same at
  every scale.
- **Lacunarity:** gliding-box lacunarity (Allain and Cloitre 1991), `Λ(s) = E[M²] / E[M]²`
  over every `s × s` box position wholly inside the mask, with `M` the foreground pixel count.
  `Λ = 1` for a uniform mask and grows as the mass gathers into clumps and gaps. It is
  unavailable when every box is empty.
- **Assumes:** pixel size, foreground definition, grid origin and box sizes are fixed before
  looking at the result; all four change the values. Every raster pixel counts as observed:
  there is no separate observation mask, so background outside the tissue counts as empty
  and the raster should be cropped to observed tissue.
- **Does not establish:** that the mask is a fractal. A finite mask has a scaling range, and a
  straight-line fit over declared sizes is a descriptive slope, not a calibrated estimate with
  uncertainty. Masks of different size or resolution are not directly comparable.
- **Evidence:** a level-4 Sierpinski carpet gives `N = 8^{4−j}` at sizes `3^j` and dimension
  `ln 8 / ln 3`; a checkerboard gives `Λ(1) = 2` and `Λ(2) = 1`; a corner pixel shows partial
  edge boxes (`tests/structure_descriptors.rs`).
  [`compare_lacunaritycovariance.R`](../tests/fixtures/lacunarity/compare_lacunaritycovariance.R)
  rasterizes tumor cells, CD8+ T cells, B cells and CD68+CD163+ macrophages in ten public
  Schürch et al. TMA A spots onto 3.02 µm pixels (40 masks, box sizes 1–45 pixels) and compares
  lacunarity with `lacunaritycovariance::gblemp` 1.1-9 and box counts with an independent R
  count and `lm`. Tolerances fixed beforehand: identical counts; lacunarity and mean mass within
  1e-12 scaled; dimension, intercept and `R²` within 1e-10. Run 01 failed two masks with two
  foreground pixels each, whose count was constant across scales: Marklab reported `R² = 1`
  where `R²` is undefined and `lm` returns rounding noise (0.498). Marklab now reports it as
  unavailable, the script requires that, and no tolerance changed. Run 02: all 40 masks pass;
  lacunarity error ≤**8.3e-13**, mean mass ≤**2.5e-13**, regression ≤**1.1e-15**.

## Mutual information between two marks (`numerics mutual-information`)

Asks whether one cell measurement carries information about another, such as Ki67 and
β-catenin, in any form of dependence rather than only a linear one, with a null model that
respects how both measurements vary across the tissue.

- **Estimate:** Kraskov–Stögbauer–Grassberger algorithm 1 with a declared `k`, after dividing
  each mark by its population standard deviation; in nats and **unclipped**, so values just
  below zero are estimator bias near independence. A cell with `k` others at identical values
  of both marks makes the estimate undefined, and the request is rejected; tied single-mark
  values are counted and reported. Zero-inflated intensities are mostly ties, so restrict the
  cells (for example to cells with both markers above zero) as part of the declared design.
- **Null models:** cell-level permutation treats cells as exchangeable, but neighboring cells
  share their surroundings and their measurements are autocorrelated, which makes ordinary
  permutation tests reject far too often. Two nulls are offered, with seeded draws:
  `variogram_matching` (Viladomat et al. 2014; Burt et al. 2020) builds surrogates of the
  second mark that keep its empirical semivariogram over the declared lag bins and its exact
  values, while breaking any link to the first mark. Each surrogate permutes the values,
  smooths them over each cell's `k` nearest cells for the declared `k` that best fits the
  observed semivariogram, adds a matched nugget and reassigns the observed values in rank
  order. `block_permutation` permutes only within declared square blocks.
- **Output:** the estimate, null mean and standard deviation, `estimate − null mean` and the
  upper Monte Carlo p-value `(1 + #{null ≥ estimate}) / (draws + 1)`, with the variogram fit
  quality and chosen smoothing for the surrogate null.
- **Assumes:** the dependence of interest is between the marks of the same cells; the
  surrogate null assumes the second mark is a stationary field whose autocorrelation the
  declared lag bins capture.
- **Does not establish:** a causal or cell-intrinsic relationship. Shared regional context
  (tumor versus stroma) is dependence too; the null removes only what spatial autocorrelation
  alone would produce.
- **Evidence:** hand-computed values on Golomb-ruler marks, whose distinct pairwise
  differences keep floating-point rounding from splitting distance ties (`11/6` and `-7/24`
  nats); singleton blocks give `p = 1`; seeded draws are reproducible
  (`tests/structure_descriptors.rs`).
  [`compare_sklearn.py`](../tests/fixtures/mutual_information/compare_sklearn.py), with criteria
  fixed beforehand:
  - **Estimates:** 60 cases (ten public Schürch et al. spots, three marker pairs on cells
    positive for both, `k` = 3 and 5) against scikit-learn 1.9.1 `mutual_info_regression`
    (limit 1e-9). 59 agree within **3.6e-14**, including 4 where scikit-learn clips a negative
    estimate to 0 and Marklab's is negative. One case **failed** by 1.07e-5: two of its 2,537
    cells share a CD44 value, and scikit-learn's 1e-10 tie-breaking noise splits that tie;
    without the noise its internal estimator agrees with Marklab within 1.8e-15.
  - **Null calibration:** two independent Gaussian fields with exponential covariance, on
    the 1,164 cell positions of one spot, 200 replicates per scenario, 99 draws each,
    rejection at `p ≤ 0.05`; size counts as controlled at ≤0.081:

    | Field range | Variogram matching | 100 µm blocks | One block (ordinary) |
    | --- | --- | --- | --- |
    | 10 µm | 0.045 | 0.055 | 0.045 |
    | 50 µm | **0.040** | 0.18 | 0.32 |
    | 200 µm | **0.065** | 0.455 | 0.89 |
    | Power, coupling 0.4 at 50 µm | 0.795 | (0.89) | (0.93) |

    Only variogram matching controls size once the fields are autocorrelated beyond 10 µm.
    The block and ordinary permutation tests do not, so their power figures are not
    comparable. Run 01 had only the block designs; run 02 added variogram matching with
    settings fixed before it ran, and reproduced run 01's other numbers exactly.

## Mapper graph of cell features (`numerics mapper-graph`)

Summarizes the shape of cell phenotype space along a chosen axis, such as a marker program
or the distance to tumor, as a graph: branches and loops show phenotypes that split or
rejoin along that axis.

- **Cover:** the lens range `[m, M]` is split into `n` equal bins of width `b`; interval `j` is
  the closed interval centred on bin `j` with half-length `b / (2(1 − p))`, so consecutive
  intervals overlap by the fraction `p` of their length (Singh, Mémoli and Carlsson 2007).
- **Clustering and graph:** within each interval, cells whose feature vectors lie within the
  declared Euclidean distance are joined, and clusters are the connected components (single
  linkage cut at the threshold); every cluster is a node. Nodes sharing at least the declared
  number of cells are joined. The result lists intervals, nodes with members, edges with shared
  counts and the number of connected components.
- **Assumes:** the lens, the feature scaling, `n`, `p` and the threshold are declared before
  looking at the graph; each changes it. Features are used as given, so scale them first.
- **Does not establish:** that a branch or loop is real. There is no stability or significance
  assessment, and small parameter changes can split or merge nodes.
- **Evidence:** ten cells on a line give the known two-node graph and its single-cell and
  minimum-overlap variants (`tests/structure_descriptors.rs`).
  [`compare_kmapper.py`](../tests/fixtures/mapper/compare_kmapper.py) compares 12 graphs: three
  public Schürch et al. spots (1,164–1,422 cells), eight standardized arcsinh marker
  features, two lenses (first principal component; distance to the nearest tumor cell) and two
  covers (10 intervals, overlap 0.3; 15 intervals, overlap 0.5), threshold 1.5. They are
  compared with KeplerMapper 2.1.0 using `DBSCAN(eps=1.5, min_samples=1)`. Criterion fixed
  beforehand: identical node member sets and edges. Result: all 12 graphs are identical
  (198–618 nodes, 64–384 edges), with no cell within 1e-9 of an interval end.

## Landy–Szalay two-point correlation (`numerics landy-szalay`)

Measures how much more often cells sit at a given distance from each other than points
spread through the same tissue would, correcting for irregular edges and holes through a
random catalog drawn from the tissue.

- **Estimator:** for each bin `[e_b, e_{b+1})`, `ξ = (DD_n − 2 DR_n + RR_n) / RR_n` with
  `DD_n = DD / (n(n−1)/2)`, `DR_n = DR / (n n_R)` and `RR_n = RR / (n_R(n_R−1)/2)`
  (Landy and Szalay 1993); each unordered pair counts once, and coincident cells are pairs at
  distance 0. `ξ = 0` for complete spatial randomness and is positive for clustering at that
  distance. Raw counts are reported with `ξ`, which is unavailable without random pairs.
- **Random catalog:** supplied by the caller with an identifier, and checked to lie inside
  the window. It is part of the declared design: it should sample exactly the observed tissue
  (with holes, tears and any sampling mask), be several times larger than the cell count, and
  be fixed before looking at `ξ`.
- **Assumes:** cells are observed throughout the area the random catalog samples.
- **Does not establish:** significance or uncertainty. No error bars are reported: counts are
  correlated across bins, Poisson errors understate the variance of clustered patterns, and a
  spatial jackknife or bootstrap would have to be declared separately.
- **Evidence:** a three-cell example gives the hand-counted `DD`, `DR` and `RR` and `ξ = 5/3`,
  with a pair at exactly a bin edge in the upper bin (`tests/structure_descriptors.rs`).
  [`compare_treecorr.py`](../tests/fixtures/landy_szalay/compare_treecorr.py) compares 15
  public Schürch et al. cell patterns (tumor cells, CD8+ T cells and CD68+CD163+ macrophages in
  five spots, 38–873 cells, uniform random catalogs of at least 2,000 and 20 n points, bins of
  10 µm to 200 µm) with TreeCorr 5.1.4 run with `bin_slop = 0` and brute force. Criteria fixed
  beforehand: identical counts, and `ξ` within 1e-12 of the 1993 formula applied to
  TreeCorr's counts. Result: all counts identical (no coincident cells occurred) and `ξ` equal
  exactly in all 15 cases. TreeCorr's own `calculateXi` normalizes auto-pairs by `n²/2`
  instead of `n(n−1)/2` and so differs by up to **0.19** in `ξ` for the 38-cell pattern, a
  convention difference that matters for small cell counts.

## Structure factor and hyperuniformity (`numerics hyperuniformity`)

Asks whether cells are spread more evenly at large scales than a random arrangement, as
packed epithelium can be, by measuring density fluctuations at long wavelengths.

- **Structure factor:** `S(k) = |Σ_j e^{−i k·x_j}|² / N` at the allowed wavevectors
  `k = 2π (m_x / L_x, m_y / L_y)`, `m ≠ 0`, `|m_a| ≤ ⌊k_max L_a / 2π⌋` of a declared rectangle,
  where the rectangle adds no window bias (Hawat et al. 2023). Random (Poisson) patterns have
  `S ≈ 1`; hyperuniform patterns have `S → 0` as `|k| → 0`. Every wavevector's value is reported.
- **Binned spectrum and indices:** means and standard errors of `S` in declared half-open
  `|k|` bins. From the nonempty bins whose centres lie below the declared stop come `Ŝ(0)`, the
  intercept of a least-squares line, and `H = Ŝ(0) / max(S_peak, 1)`, with `S_peak` the first
  local maximum of the binned means reaching 1. Torquato (2018) calls `H ≤ 10⁻³` effectively
  hyperuniform. A least-squares fit of `S ≈ c|k|^α` over the same bins also gives `α`, whose
  value separates hyperuniformity classes.
- **Assumes:** the rectangle is fully covered by observed tissue. Empty background or lumen
  inside it reads as large-scale density fluctuation and pushes `S` up at small `k`. Choose bin
  edges away from the wavenumbers `2π|m|/L` of allowed wavevectors: a wavevector exactly on an
  edge is assigned by the half-open rule, and other software may close the last bin.
- **Does not establish:** hyperuniformity as a calibrated finding. There is no uncertainty for
  `H` or `α`, the few lowest bins carry most of the evidence, and a finite rectangle cannot show
  the `k → 0` limit.
- **Evidence:** a lattice filling the rectangle gives `S = 0` at every allowed wavevector short
  of the reciprocal lattice, and two cells 2.5 µm apart give `S = 1 + cos(k_x · 2.5)`
  (`tests/structure_descriptors.rs`).
  [`compare_structure_factor.py`](../tests/fixtures/hyperuniformity/compare_structure_factor.py)
  compares five public Schürch et al. spots (300 µm central squares, 219–715 cells, 9,024
  wavevectors to 1 rad/µm per axis) with structure-factor 2.2.1. That package declares Python
  older than 3.10 and numpy 1; it ran from its unpacked wheel with numba's `njit` replaced by
  an identity decorator and explicit wavevectors, neither touching the compared estimators.
  Tolerances fixed beforehand: `S` and bin means within 1e-9; the line, `Ŝ(0)` and `H` within
  1e-7; `α` and `c` within 1e-5.
  - Run 02 **failed**: its bin edges `4πj/300` coincided with allowed wavenumbers, and scipy
    closes the last bin where Marklab does not, so four wavevectors on the final edge changed
    that bin's mean by 2.5e-3.
  - Run 03 used edges offset by a quarter step, with no tolerance changed, and passed in all
    five spots: `S` ≤3.5e-13, bin means ≤8.9e-15, line and `H` ≤2.2e-8, `α` and `c` ≤3.7e-6.
  - The spots' `H` of 0.9–4.8 and negative `α` reflect TMA cores that do not fill the
    squares; they are not tissue findings.

  Run 01 stopped at writing its summary because of a script bug.

Ecological co-occurrence and niche models remain planned. They need a declared abundance or
environment model; quadrat counts alone do not supply one.
