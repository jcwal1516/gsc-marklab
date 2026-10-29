# Changelog

## Unreleased

### Scientific and serialized contracts

- Blocked `cohort covariate-permutation` and `cohort covariate-matrix-permutation` now remove block means
  before the residual permutation, which puts block fixed effects in the model. Before, residuals moved only
  within blocks but the model had no block terms. The p-value then tested the within-block contrast, while the
  reported effect, standard error and statistic were not adjusted for block, so they disagreed whenever the
  groups differed by block. Both documents are now version 2 and report `block_indicator_columns`.
  `reduced_model_columns`, `full_model_columns` and the residual degrees of freedom count the block columns. A
  blocked run needs at least one block that contains both groups. Unblocked results are unchanged.

- Result format 0.5 adds `xi_resolution` to spectrum and component summaries: whether the
  shell that sets `xi_um` lies inside the eligible range or at its lowest- or highest-`k`
  edge, and the wavelengths of its eligible neighbours. At the lowest-`k` edge the window
  may hide a longer correlation length; `report.md` says so.

- Preserve configured permutation strata when computing component spectra, including the
  unavailable result for a component with no exchangeable labels. Reject conflicting source
  cell IDs at the declared-input boundary and invalid probability rows at the direct analysis
  boundary.

- Retain `g = 0` and cross-type `g = 0` when kernel support is empty but the estimator's
  normalization is positive, for border, isotropic, translation and intensity-reweighted
  estimators. Zero estimates participate in inference. Existing result formats and legacy
  unavailable-zero documents remain readable; affected workflow revisions invalidate old caches.

- Add `numerics hyperuniformity` (`marklab.hyperuniformity` version 1): the scattering
  intensity of cells at the allowed wavevectors of a fully observed rectangle, binned means,
  the extrapolated `Ŝ(0)`, the `H` index and a power-law exponent `α`. Five public CODEX
  squares agree with structure-factor 2.2.1.

- Add `numerics landy-szalay` (`marklab.landy_szalay` version 1): the Landy–Szalay two-point
  correlation of cells against a caller-declared random catalog inside the tissue window,
  with raw DD, DR and RR counts. Counts on 15 public CODEX patterns are identical to TreeCorr
  5.1.4.

- Add `numerics mapper-graph` (`marklab.mapper_graph` version 1): the Mapper graph of cell
  feature vectors over a one-dimensional lens with a declared interval cover, single-linkage
  threshold and minimum shared cells. Twelve graphs on public CODEX cells are identical to
  KeplerMapper 2.1.0.

- Add `numerics mutual-information` (`marklab.mutual_information` version 1): the
  Kraskov–Stögbauer–Grassberger estimate between two cell marks, unclipped, with a seeded
  variogram-matching surrogate null or a block permutation null. Estimates agree with
  scikit-learn 1.9.1 in 59 of 60 public cases; one differs by 1.07e-5 through scikit-learn's
  tie-breaking noise. The variogram-matching null held size under autocorrelated fields where
  ordinary permutation rejected up to 89% of the time.

- Add `numerics lacunarity-fractal-dimension` (`marklab.lacunarity_fractal_dimension`
  version 1): box-counting dimension with its regression fit, and gliding-box lacunarity, for
  a declared binary mask, pixel size and box sizes. It agrees with
  `lacunaritycovariance::gblemp` 1.1-9 and an independent R box count on 40 public CODEX cell
  masks.

- Add `numerics bond-orientational-order` (`marklab.bond_orientational_order` version 1):
  per-cell k-atic order `ψ_k` with nearest-count or radius neighbors and a border rule. It
  matches double-precision evaluation on freud 3.6.1's neighbor lists within 2.2e-16; freud's
  own single-precision values differ by up to 7.0e-4, beyond the prespecified bound.

- Add `numerics ordinary-kriging` (`marklab.ordinary_kriging` version 1): ordinary kriging
  with a declared exponential, spherical or Gaussian variogram, a declared neighbor search and
  leave-one-block-out held-out checks (RMSE, standardized errors, 95% coverage). It agrees
  with SciKit-GStat 1.0.23 on 600 public CODEX cells.

- Add `numerics morisita-dispersion` (`marklab.morisita_dispersion` version 1): Morisita's
  index, the Smith-Gill standardized index and the Poisson-quadrat chi-square test on whole
  quadrats of a declared grid inside the tissue window. It agrees with
  `vegan::dispindmorisita` 2.7-6 on 30 public CODEX cell patterns.

- Add an optional `support_rule` (`expected_pair_count`) to `pathology spatial-study`
  designs, declared before analysis from counts and border distances. With it, `g` and
  cross-type `g` bins with eligible centers but no pairs are estimates of zero, and null
  simulations lacking eligible centers where the observed pattern has them are redrawn
  under a bounded conditional Monte Carlo test; slides failing the rule stay unavailable
  with the radius named. Designs without the rule give unchanged results. The calibration
  scenarios `sparse_cross_g_refit` and `cluster_g_refit` now supply size (0.046 among 693
  available of 1,000) and power (250/250) evidence instead of support-handling checks.

- Add an SPDE prior for the `multimodal spatial-latent-factor` factor (`latent_field.kind =
  "spde_alpha_two"`, smoothness 1): a mesh-based Lindgren–Rue–Lindström precision with
  barycentric projection to the regions. Requests without the option are byte-identical.
  Move the finite-element assembly, rectangular mesh, projection and one-factor MAP fit into
  the shared `marklab_worker_support.fem` module used by the `bayes` SPDE commands; their
  results are unchanged and their worker digest changes. The `multimodal validate`
  `spde_factor_recovery` entry now runs a recovery comparison against the dense Matérn-3/2 GP
  instead of reporting `blocked_missing_mesh_owner`.

- Add `bayes unbalanced-fused-gromov-wasserstein`: fused unbalanced Gromov–Wasserstein with
  KL-relaxed marginals through pinned POT 0.9.7.post1 (`marklab.unbalanced_fused_gromov_wasserstein`
  version 1), with three starting plans, recomputed objective terms and analytic and
  independent-optimizer known answers. The partial FGW output's `unbalanced_fgw_status`
  now reads `available_as_bayes_unbalanced_fused_gromov_wasserstein` instead of
  `blocked_no_pinned_fgw_backend_with_kl_relaxed_marginals`.

- Export each slide's observed and simulated within-slide null curves from
  `pathology spatial-study` as `null-curves.json` (format
  `marklab.pathology_spatial_null_curves` version 1), on the reported scale and masked to
  the radii the ERL envelope used. `result.json` is unchanged. GET 1.0-9 reproduces the
  exported curves' ERL p-values and bounds on six public retinal cases and one synthetic
  inhomogeneous Poisson case.

- Replace the `bayes advanced-cluster` k-means heuristic with a Bayesian Thomas cluster
  process: parents on a declared dilation of each rectangular window sampled by spatial
  birth–death MCMC, lognormal pooling of `κ`, `μ` and `σ` across patients, ArviZ R-hat and
  ESS over several chains, an exact edge-truncation share, posterior predictive checks and
  work and memory bounds checked before sampling. Non-converged fits are published as such.
  Add `bayes advanced-cluster-sbc`. The output is `marklab.bayesian_thomas_cluster_model`
  version 1; version-2 heuristic outputs are no longer produced but remain readable through
  `ClusterModelDocument`. Legacy heuristic inputs are rejected; the finite binary Gibbs
  exchange fit is kept as an optional, independent `finite_gibbs` input.

- Remove CellViT assumptions from the shared cell-embedding storage path. Version-2
  provenance declares the producer's dimension, pooling, normalization, input channels,
  modality and stain; verified sources use generic CSV/NPY labels. The original version-1
  CellViT reader and canonical bytes remain supported.
- Add bounded model-independent float32 NPY readers and `bayes vector-semivariogram
  --vectors`. Separate-NPY runs use result version 2 to bind both coordinate and vector
  sources; existing CSV-only results and the main result reader are unchanged.
- Compute the unweighted sample-covariance correction directly as `(n - 1)/n`
  while retaining scaled compensated cross products. The exact three-row oracle's
  first diagonal changes from `0.9999999999999999` to `1.0`; effective sample size
  is exactly `n`. Caller-weighted calculations and serialized field meanings are unchanged.
- Make adaptive SPDE optimization satisfy its existing gradient requirement even when
  SciPy reports a successful objective-change stop. The holed-window regression's gradient
  changes from 3.30e-4 (rejected) to 3.62e-5 (accepted), within the same 1,000-iteration
  budget and unchanged 2e-4 limit. The objective and serialized field meanings are unchanged.
- Correct rectangular translation-overlap arithmetic. The previous polygon-clipping
  path introduced avoidable error even for exact axis-aligned rectangles; the bounded
  rectangular formula now agrees with independent pair sums and spatstat. On a public
  beta-cell regression, K(18.75 µm) changes from 169.17194815987222 to
  169.1719483393978 µm². Translation cache identities are advanced so old values are
  not replayed. Nonrectangular geometry still uses the existing polygon path.
- Preserve correctly rounded floating-point values when reading JSON results and
  cache inputs. Decimal values such as `57.925000000000004` must reopen as that same
  binary64 value. Existing strict result validation and the main result reader remain.
  Durable source-node cache identities include the JSON number-parsing policy, so a
  result cached under the previous parser is recomputed once instead of replayed.
- Add the explicit `binary_symmetric_including_isolates` Moran/Geary policy. It retains
  isolated observed cells in the mean, variance and permutation population. Existing
  binary and row-standardized policies remain strict; zero-edge graphs are unavailable.
- Add directed categorical cross-L curves and transformed ERL bounds to cross-K results.
  The expanded standalone contract is `marklab.categorical-pair/2`, with a new durable
  artifact schema. Cross-K and mark-connection definitions are unchanged.
- The main result document is format 0.5. Its reader accepts 0.5; it does not convert
  0.2, 0.3 or 0.4. See [the format notes](docs/result-format-0.5.md) for changes introduced
  in this development batch, including typed metadata, permutation-count fields and
  `xi_resolution`.

### Public-data validation and interoperability

- Add `marklab spectrum preprocess` and the `marklab-microbio` crate: read one Bruker flex
  MALDI-TOF acquisition (`fid` with `acqu` or `acqus`), convert time of flight to m/z, apply
  declared steps in order (square root, Savitzky–Golay, SNIP baseline, total ion current, trim)
  and optionally sum fixed-edge m/z bins. High-precision calibration, LIFT, PSD and V1.0 CTOF2
  calibrations are rejected rather than approximated. On readBrukerFlexData 1.9.3's two example
  spectra, raw intensities match readBrukerFlexData exactly and m/z within 2.0e-12 (its root
  formula cancels; Marklab's does not), every step matches MALDIquant 1.22.3 within 2.8e-12 of
  the spectrum maximum, the 6,000-bin vectors agree with a per-bin R sum within 2.8e-12, and the
  HPC example is rejected. `--text` reads two-column spectra such as DRIAMS `raw` files.

- Add `marklab spectrum feature-table`, `marklab classifier fit` and `classifier predict`. A
  feature table is a Parquet file with an `id` column, numeric feature columns and
  `marklab.feature_table` metadata (modality, unit, provenance); `spectrum feature-table` writes one
  from binned spectra, and other modalities can write it from Python (checked with pyarrow, pandas
  and polars). `classifier fit` fits an ensemble of L2-penalised logistic regressions predicting a
  binary label (one per inner fold, scikit-learn-equivalent within 1e-7), Platt-calibrates it on
  held-out margins and calibrates class-conditional cross-conformal calls; outer cross-validation of
  that whole procedure reports AUROC, average precision, Brier score, reliability and
  false-negative, false-positive and abstention rates. `classifier predict` applies the saved model
  and refuses tables made differently.

- Compare `numerics categorical-enrichment` with imcRtools 1.16.0 `testInteractions`
  (classic) and SPIAT 1.12.1 `mixing_score_summary` on three public CODEX spots. Graphs and
  per-cell neighbor statistics agree (≤3.6e-15), permutation p-values agree within 0.0158,
  and SPIAT's interaction counts and mixing scores are reproduced exactly.

- Add `upstream_formats.mcmicro_csv_slide` to the Python client: one MCMICRO (MCQuant)
  quantification CSV with explicit micrometre scales and window. On nf-core's public tonsil
  CyCIF test output it reproduces the table exactly, and the table's centroids and areas match
  the segmentation mask. The Steinbock reader now shares the same bounded CSV reader; its
  messages are unchanged.

- Calibrate the shipped `analyze` primary endpoint and the `multimodal` enrichment by shuffle
  experiments across layouts, prevalences and all four multimodal null models, with size
  and power criteria fixed in advance and a weekly scheduled run. The earlier control
  reused one permutation seed, so it measured a single reference set rather than the test.
  The experiment also shows that one clone covering a quarter of a small window is
  rarely detected by the low-k primary endpoint.
- Add bounded categorical co-occurrence curves with public Squidpy agreement on
  1,107 CODEX cells at four physical radii, preserving exact class-pair counts.
- Add categorical neighborhood enrichment with exact fixed-count graph-null moments
  and separate deterministic Monte Carlo estimates. Public counts agree with Squidpy;
  exact moments agree with the same formulas evaluated separately in rational arithmetic,
  and a whole-slide regression checks them against the textbook Cliff–Ord form. The
  exact calculation uses 256-bit intermediates; it previously overflowed from about
  60,000 cells. Simulation z-score parity is unassessed.
- Add group degree, closeness and average clustering on physical-radius graphs, with
  public Squidpy/NetworkX agreement and explicit disconnected-graph conventions.
- Validate patient-level two-group Max-T on the Schürch colorectal CODEX cohort
  (35 patients) and curated Jackson breast-cancer IMC cohort (100 patients), with
  independent SciPy/NumPy calculations. Deterministic quantities use fixed limits;
  adjusted p-values use a Monte Carlo error limit that replaced a fixed 0.03 after review.
- Exercise the CSV-to-multiplex workflow on 138,567 Schürch cells in 70 fields. Preserve
  and document the preceding resource-limit and isolated-cell support failures.
- Add explicit calibrated cell-table import to the Python client, preserving source cell
  IDs, channel declarations, missingness and patient membership.
- Check native CSV/Parquet and derived sparse AnnData interchange on observed Schürch
  cells, Steinbock paired tables on its upstream mock dataset, and public Aperio JPEG2000
  pixels against OpenSlide. Each profile has explicit limits; these do not imply universal
  compatibility with every upstream export or image codec.
- Import classified single-ring QuPath nucleus annotations with explicit calibration,
  using a public PUMA melanoma ROI to check identities, classifications and centroids.
  Reject duplicate JSON fields and unsupported geometry/object profiles.
- Check 128 published STHELAR CellViT-SAM-H vectors through native NPY, Arrow IPC and
  Parquet file paths, preserving all 163,840 values exactly. The codec test uses explicitly
  synthetic provenance; the source does not supply a verified checkpoint identity for the
  strict normalized CellViT bundle.
- Compare rectangular homogeneous K/L and g with spatstat on public retinal patterns,
  and compare directed cross-g with independent pair sums. Exact equality with the
  package's discretized cross-g curves fails the fixed tolerance and remains documented.
- Check public reduced-sample F/G curves and their finite J ratio against spatstat with
  explicit probe and border conventions. Exact agreement applies to the demonstrated
  rectangular observed curves; it does not calibrate their inferential extensions.
- Validate the unweighted observed vector variogram against SciKit-GStat on 1,107
  Schürch cells. Weighted and inferential variants retain separate Research scope.
- Pin the R/spatstat/GET reference environment and configure the scheduled calibration
  workflow to run it; that job has not yet run in CI. Add directed public-data cross-K/L comparisons, default-package comparisons,
  and an explicit fixed-count leave-one-out intensity derivation.
- Replace age-based maturity labels with an evidence ladder. Promote only demonstrated
  configurations; distinguish unsupported-input tests from size and power calibration.
  Mark unimplemented methods as Planned rather than out of scope, and document the
  Research-tier command families in `docs/research-workflows.md`.

### Engineering in the unreleased development batch

- Name the spectrum and classifier commands for what they do rather than for MALDI-TOF
  susceptibility: `microbio preprocess` and `microbio feature-table` are now `spectrum preprocess`
  and `spectrum feature-table`, which takes `--modality` and `--unit`; `microbio fit-ast` and
  `predict-ast` are now `classifier fit` and `classifier predict`. Their configuration names a
  `label_column`, `positive` and `negative` label values and an optional `subset` instead of an
  antibiotic, resistant and susceptible categories and a species, and their reports give
  false-negative and false-positive rates instead of very major and major errors. A feature table's
  unit may be any name. Models are `marklab.classifier` files; `marklab.ast-model` files must be
  refitted. [Pipelines](examples/README.md) maps the susceptibility terms and shows the command
  sequences.
- The `cohort` commands read input tables of up to 512 MiB, raised from 16 MiB, so tests over
  thousands of endpoints or features fit. Each command's work limits are unchanged; `cohort
  max-t`, for example, still allows at most 10^8 patient-endpoint-permutation evaluations. A
  table is read whole: a 216 MB `max-t` table of 100,000 patients and 60 endpoints peaked at
  0.70 GB resident memory in a debug build. `cohort max-t-calibration` and the `project
  cohort-*` commands keep their own input limits.
- Pull-request CI runs the backend tests in six parallel shards and leaves out the Bayesian
  calibration, prior-sensitivity and PyMC/NumPyro agreement tests (nextest profiles `pr` and
  `calibration` in `.config/nextest.toml`). Those run weekly in the Backend calibration
  workflow. The backend job had taken about three hours on two test threads.
- Organize scientific implementations and worker contracts into domain crates, expose
  namespaced Rust APIs, and parse CLI commands through a shared typed command tree.
- Consolidate durable project nodes, input validation, worker execution, timeout bounds,
  finite-result metadata and columnar artifact handling.
- Keep failed Bayesian diagnostics visible, use each quantity's Monte Carlo error in
  cross-backend comparisons, and pin workflow actions and Python lint tooling.
- Strengthen Linux-sensitive PyMC tests. Local macOS evidence does not verify Linux;
  Docker is unavailable on the development host and the branch still needs CI after push.
- Add this changelog and software citation metadata. No release, tag or publication has
  been created by these changes.
