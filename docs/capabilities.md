# What Marklab can do

Marklab measures how cells are arranged in tissue. This page lists each workflow, the
question it answers and how mature it is. `marklab --help` lists every command, and
`marklab <command> --help` shows the options.

Evidence tiers describe demonstrated scientific use, independently of software age or
file-format stability:

- **Research:** a synthetic known-answer check and documented assumptions and limitations.
- **Experimental:** Research evidence, agreement with an independent implementation or
  analytic result, and a public real-data demonstration of the particular method.
- **Core:** Experimental evidence, calibration across several tissue shapes and label
  prevalences (size and, where applicable, power), a public cohort with real patient groups,
  and a stable output format.

No workflow currently meets all Core requirements. The earlier Core label on this page
meant the oldest, most tested software; it did not establish cohort validation. A passing
fit or synthetic test alone cannot promote a method. Existing serialized `experimental`,
`research_only`, `established` and `validated` policy values use the older, caller-supplied
policy vocabulary; they are not certificates that this evidence ladder has been met.

## Method coverage

This inventory was checked against the command tree, workspace crates and linked upstream
documentation on 10 October 2026. **Implemented** means the stated calculation exists,
not that a named package has been reproduced. **Interoperable** requires a tested data
path. **Planned** identifies missing work. **Out of scope** states a deliberate boundary.
Reference names identify intended comparisons, not unexecuted agreement claims.

Upstream model choice is independent of this inventory. CellViT is one supported
example, and its normalized bundle is an optional pipeline-specific adapter. The
shared cell-table and spatial-vector inputs accept outputs mapped from other models;
they do not require that CellViT bundle or its fixed calibration. This does not imply
automatic parsing of every model's native export files.

| Method or data path | Marklab status | Reference implementation | Evidence and remaining boundary |
|---|---|---|---|
| Homogeneous K/L, border correction | Implemented | [spatstat](https://spatstat.org/resources/spatstatQuickref.pdf), [pointpats](https://pysal.org/pointpats/api.html) | Public amacrine/betacells agreement: scaled K error ≤4.05e-16 and L error ≤2.85e-14 µm; both limits 1e-10. [Estimator and grid conventions](reference-point-patterns.md). Fixed-count CSR differs from label shuffling. |
| Translation and isotropic K/L | Implemented | spatstat `Kest` | Public amacrine/betacells agreement: scaled K error ≤8.51e-15 and L error ≤8.53e-14 µm; both limits 1e-10. [Earlier failures and rectangle correction](reference-point-patterns.md) are retained. |
| Inhomogeneous K/L | Implemented | spatstat `Kinhom` | Two published retinal recordings (all cells, then each of two cell types: six cases), 96 radii: matched-intensity error ≤2.59e-11 against 1e-10 tolerance. [Scope](pathology-studies.md#reference-comparisons-on-published-cells). |
| Homogeneous, inhomogeneous and cross-type g | Implemented; reference agreement for homogeneous g and directed cross-g only | spatstat `pcf`; independent pair sums using public edge weights | [Homogeneous translation/isotropic g](reference-point-patterns.md#pair-correlation-agreement) agrees with analytic `pcf.ppp` within 3.40e-15 scaled error. Directed homogeneous/inhomogeneous cross-g pair sums agree within 1.91e-15 (limits 1e-10); the inhomogeneous check supplies Marklab's own intensities. Discretized `pcfcross` package curves fail exact equality: up to 1.16e-3 on the package grid and 1.07e-6 with 1,024 subdivisions, shrinking with refinement. No single-type inhomogeneous g comparison exists. Standard-border variants retain Python known-answer scope. |
| Directed homogeneous cross-type K/L | Implemented | spatstat `Kcross` | Four public amacrine/betacells direction checks: scaled K error ≤4.21e-16 and L error ≤4.27e-14 µm, both within 1e-10. Sparse-grid failure and package defaults retained; [conventions](categorical-pairs.md). Inhomogeneous cross-K is out of scope here: its type-specific intensity model and normalization need separate reference agreement; the homogeneous contract cannot be reused unchanged. |
| G, F and J | Implemented | spatstat `Fest` / `Gest`; matched finite J ratio | [Public amacrine/betacells](reference-point-patterns.md#empty-space-nearest-neighbour-and-j-agreement): exact reduced-sample F/G agreement on a declared 64×64 probe grid and polygon border distances, and exact J ratio at supported radii. This is not a direct `Jest` or arbitrary-probe claim. |
| ERL global envelopes | Implemented | [GET](https://stat.ethz.ch/CRAN/web/packages/GET/GET.pdf) | Executed GET 1.0-9 tied-curve oracle: exact bounds, depths and p=0.8. `pathology spatial-study` exports its within-slide null curves; GET reproduces the plug-in L-r p-values (error ≤2.08e-17) and bounds (≤5.08e-15) on six public retinal cases, all p = 1/40, and one synthetic case with p = 0.25 ([details](pathology-studies.md#reference-comparisons-on-published-cells)). Refit, g, cross-g and compartment null curves are exported but not compared. |
| Gibbs: Strauss/Geyer; Cox: Thomas/Matérn/LGCP | Implemented (Research) | spatstat, PyMC/NumPyro | Model-specific synthetic/backend checks. The [Bayesian Thomas cluster model](bayesian-thomas-cluster.md) adds known-answer, recovery and 200-replicate SBC checks and a descriptive spatstat `kppm` comparison on public data, where real-data fits mixed slowly. Binary Gibbs heuristics and pseudolikelihood are not general continuous-space Gibbs fits. |
| Neighborhood enrichment / interaction permutation | Categorical radius workflow implemented; [imcRtools comparison](categorical-enrichment.md#comparison-with-imcrtools-and-spiat) run | [Squidpy](https://squidpy.readthedocs.io/en/latest/api.html); exact fixed-count moments checked against small-graph answers and Cliff–Ord | [1,107 CODEX cells](categorical-enrichment.md): exact observed counts, analytic scaled error ≤2.85e-16 (limit 1e-12). Monte Carlo z-score agreement is unassessed. imcRtools 1.16.0 `testInteractions` (classic, histoCAT-style) on 3 public spots: identical graphs, per-cell neighbor statistic within 3.6e-15 for 1,163 ordered pairs, permutation p-values within 0.0158 (independent 9,999-draw streams). Its `histocat` and `patch` variants were not compared. Registered multimodal/stratified nulls retain separate Research scope. |
| Distance-dependent cell-type co-occurrence | Implemented | Squidpy `gr.co_occurrence` | [Four-radius CODEX comparison](categorical-cooccurrence.md): exact directed counts and native formula values; Squidpy error ≤3.56e-15 under its fixed floating-point bound. Edge-incidence normalization; no inferential or scimap-pipeline parity claim. |
| Global Moran/Geary and local Moran | Implemented | Squidpy, PySAL/ESDA | Global binary-radius Moran/Geary have exact PySAL isolate checks and [70-field CODEX agreement](validation-schurch.md) ≤2.50e-15. Local Moran remains Research: unconditional shuffling and population variance differ from default conditional LISA. |
| Graph centrality | Group degree, group closeness and average clustering implemented; individual betweenness out of scope | Squidpy / NetworkX | [19 CODEX cell groups](group-centrality.md), exact graph counts and native score error ≤3.34e-16 under 512 eps64 × (1 + absolute reference). These group summaries address cell-class graph position; individual betweenness is a separate path-flow statistic. |
| scimap spatial distance/count/interaction/aggregate | Native scimap pipeline out of scope | [scimap](https://scimap-doc.readthedocs.io/en/latest/tools/) | The listed native point-pattern and neighborhood primitives answer related questions. scimap's graph construction, aggregation and interaction-test conventions form a separate pipeline; normalized AnnData import does not establish that pipeline's parity. |
| Giotto spatial networks, enrichment and domains | Native Giotto objects and domain discovery out of scope | [Giotto Suite](https://giottosuite.com/reference/) | Its spatial-expression/network object model and domain discovery are separate from Marklab's declared cell-table statistics. Ligand–receptor mechanism inference additionally requires expression and a declared interaction database. |
| Patient-group comparison of slide summaries | Implemented; spicyR model out of scope | SciPy/NumPy; adjacent [spicyR](https://www.bioconductor.org/packages/devel/bioc/vignettes/spicyR/inst/doc/spicyR.html) | Two-group Max-T agrees on [35 CODEX](validation-schurch.md) patients (nine neighborhood fractions and four Moran/Geary endpoints) and [100 IMC](validation-jackson.md) patients (13 metacluster fractions: composition, not spatial arrangement). spicyR's cell-count weighting and response model target a different estimator; substituting Max-T is not a reproduction of that model. |
| Immune/tumor distance, mixing and neighborhoods | Declared compartment metrics implemented; [SPIAT mixing-score comparison](categorical-enrichment.md#comparison-with-imcrtools-and-spiat) run | Independent Python/GEOS and analytic geometry; adjacent [SPIAT](https://www.bioconductor.org/packages/release/bioc/html/SPIAT.html) | Radius-mixing counts use a Python oracle; contact/fragmentation use GEOS fixtures; signed interface distances have exact rectangle answers. These synthetic checks do not establish public-cohort performance. SPIAT 1.12.1 mixing scores equal the categorical radius counts' ratios exactly for 33 public cell-type pairs (3 more are undefined, where SPIAT reports 0); SPIAT's distance metrics have not been compared. |
| Epithelial architecture and invasive front | Front derivation, distance bands, fragmentation, lumen containment, contour curvature, skeleton branching, ITBCC-style budding hotspot, nuclear shape and spot/cell attachment implemented (Research) | Analytic geometry; adjacent [Bertillot et al. preprint](https://doi.org/10.1101/2025.08.11.669722) and [ICCR/ITBCC budding definition](https://pmc.ncbi.nlm.nih.gov/articles/PMC8820778/) | A square gland, its lumen, fragments and front offsets have exact areas, distances and turning angles; bar, plus, T and ring skeletons have exact branch and end counts; hotspot counts match hand-computed chords; spot covers are exact for inside, split and off-tissue disks. Regions and classes are declared upstream; Marklab does not segment or classify. No public real-data demonstration or agreement with a pathologist-drawn front yet. |
| Multiview contextual effects | MISTy model fitting out of scope | [MISTy / mistyR](https://master.bioconductor.org/packages/release/bioc/manuals/mistyR/man/mistyR.pdf) | Fitting and evaluating learned view contributions requires a declared multiview training/held-out design. Existing factors/regressions do not implement this decomposition. |
| Learned niches | Native niche-model fitting out of scope | [CellCharter](https://cellcharter.readthedocs.io/en/stable/graph.html), [UTAG](https://www.nature.com/articles/s41592-022-01657-2), [BANKSY](https://prabhakarlab.github.io/Banksy/) | Their feature aggregation, cross-image clustering and preprocessing are distinct models. Marklab consumes declared categorical observations; this does not reproduce the models or validate a predicted niche as biological ground truth. |
| Recurring cellular neighborhoods | Interoperable through published labels; clustering out of scope | [Schürch et al.](https://pmc.ncbi.nlm.nih.gov/articles/PMC7479520/) | The original nine neighborhood labels flow through the [Schürch patient analysis](validation-schurch.md). Native radius composition is not the published kNN-composition clustering. |
| QuPath GeoJSON and measurements | Interoperable for classified PUMA nucleus annotations; general measurements out of scope for this profile | [QuPath](https://qupath.readthedocs.io/en/stable/docs/tutorials/exporting_measurements.html) | [Biological PUMA ROI](qupath-import.md): 168 exact IDs/classes, centroid error ≤2.85e-14 µm against an independent shoelace calculation (limit 1e-9). Explicit calibration; no generic detection/cell measurement claim. |
| CellViT cells and embeddings | Optional normalized NPY and bundle adapters implemented; native `cells.json`/graph outputs out of scope for the current profile | [CellViT-Inference](https://github.com/TIO-IKIM/CellViT-Inference), [STHELAR](cellvit-import.md) | 128 real CellViT-SAM-H vectors pass exact NPY/columnar checks. Full normalized v1 CSV/manifest admission has fixed calibration and synthetic fixtures only; this is an adapter-specific evidence limit, not a prerequisite for other models. Native outputs have different layouts and need separately bound source image calibration and metadata. |
| HoVer-Net nucleus output | Native JSON adapter planned | [HoVer-Net](https://github.com/vqdang/hover_net) | Native `mag` can be null or nominal objective power, and does not bind physical MPP. An adapter needs a small reviewed native biological output with independent calibration; none has been found yet. The PUMA QuPath file is a corrected re-export, not a native HoVer-Net fixture. A search on 10 October 2026 found native output on TCGA slides (Waldron lab bucket, slide MPP independently available from GDC), but with no stated licence and no recorded HoVer-Net version or checkpoint, so it is not used. In whole-slide mode HoVer-Net's `bbox` is offset with swapped axes (`infer/wsi.py`), so an adapter must use `centroid` and `contour`. |
| HistomicsTK, StarDist, Cellpose, Mesmer, InstanSeg | Native project/model formats and segmentation out of scope | Upstream object/measurement exports | These tools produce distinct masks, objects and measurement exports. Use explicit calibrated cell-table mapping after upstream processing; it is not an automatic adapter for each tool, and Marklab does not run segmentation or train image models. |
| MCMICRO and Steinbock tables | Steinbock and MCMICRO quantification interoperable | [MCMICRO](https://mcmicro.org/parameters/specs.html), [Steinbock](https://bodenmillergroup.github.io/steinbock/latest/cli/export/) | [Steinbock paired-CSV adapter](steinbock-import.md) has exact Object joins, explicit axes/calibration, malformed-file tests and an upstream mock-fixture file check. The [MCMICRO adapter](mcmicro-import.md) reads MCQuant tables with explicit calibration; on nf-core's public tonsil test output (1,621 cells) its centroids and areas match the segmentation mask exactly and the OME-XML scale is applied. That output came from MCMICRO's component tools with an unrecorded MCQuant version; legacy column names are not admitted. |
| TIAToolbox and WSI viewers | Native viewer/project state out of scope | TIAToolbox / QuPath | Pixel reading is tested separately. Viewer annotations, slide databases and framework objects are distinct stateful formats, not implicit extensions of the region reader. |
| UNI, CONCH, Virchow, Prov-GigaPath, H-optimus embeddings | Generic vector/NPY inputs and declared-model cell provenance implemented; native model-export profiles out of scope | Upstream feature matrices/coordinates | NPY CLI and verified Arrow/Parquet checks cover non-CellViT dimensions. Tensor layout, pooling, normalization, coordinates and weights are producer declarations. CellViT file evidence does not validate other encoders or imply their native exports load directly. |
| ABMIL, CLAM, TransMIL slide models | Architectures and native model artifacts out of scope | Upstream scores/embeddings | Patient evaluation accepts explicitly declared endpoint values. That is not a native model-output adapter or evidence that MIL attention identifies a cell-level mechanism. |
| AnnData `.h5ad` | Interoperable: bounded in-memory profile | [AnnData](https://anndata.readthedocs.io/en/stable/fileformat-prose.html) 0.12.4 | Python client and sparse round trips, including [1,107 observed Schürch cells](validation-schurch.md) in a derived H5AD with exact identities/values/coordinates. Physical window/patient/group declarations required; no backed/lazy arrays or universal AnnData schema claim. |
| SpatialData / OME-Zarr | Bounded STHELAR table extraction example; general ingestion out of scope | [SpatialData](https://spatialdata.scverse.org/en/latest/tutorials/notebooks/notebooks/examples/models1.html), [OME-NGFF](https://ngff.openmicroscopy.org/specifications/index.html) | The [STHELAR script](cellvit-import.md) preserves selected published Zarr arrays and joins exact cell IDs. It is not a general image/label/point transform adapter; physical units require a separately justified source contract. |
| OME-TIFF metadata and WSI pixels | Pixel reader implemented; OME-XML mapping out of scope | [`wsi-rs` / OME-TIFF](https://ome-model.readthedocs.io/en/stable/ome-tiff/) | [Public Aperio SVS pixels](formats.md#public-image-check) agree with OpenSlide within the existing lossy-codec limits. A codec does not establish OME axes, plane or physical-calibration semantics; no OME-XML adapter or WSI writer is provided. |
| CSV and Parquet cell tables | Read/write implemented | CSV / Arrow-Parquet | Synthetic round trips/parser tests; the calibrated [Schürch CSV](validation-schurch.md) feeds all 70 multiplex fields. The native CSV/Parquet round trip preserves 1,107 observed cells exactly; [format scope](formats.md). |
| Arrow IPC / Parquet embedding artifacts | Internal storage implemented | Arrow / Parquet | Exact actual-file transport of 163,840 published CellViT values and 128 IDs, with explicitly synthetic test provenance. Strict physical profile deliberately rejects arbitrary pyarrow artifacts; not a universal interchange format or verified checkpoint claim. |
| Structure factor and hyperuniformity | Mark spectra implemented (Research: synthetic checks only); [unmarked structure factor with `H` and `α`](structure-descriptors.md#structure-factor-and-hyperuniformity-numerics-hyperuniformity) implemented (Research) | [structure-factor](https://for-a-few-dpps-more.github.io/structure-factor/) | Scattering intensity at the allowed wavevectors of a fully observed rectangle. Five public CODEX squares agree with structure-factor 2.2.1 (`S` ≤3.5e-13, `H` ≤2.2e-8, `α` ≤3.7e-6) after a first run failed on a last-bin boundary convention. No uncertainty for `H` or `α`; irregular tissue windows are not supported. Fixed-label spectra do not establish unmarked hyperuniformity. |
| Minkowski functionals | Implemented | scikit-image | Area/perimeter/Euler-characteristic worker/oracle tests; pixel connectivity and resolution must be declared. |
| Lacunarity and fractal dimension | [Box-counting dimension and gliding-box lacunarity](structure-descriptors.md#box-counting-dimension-and-lacunarity-numerics-lacunarity-fractal-dimension) implemented (Research) | [`lacunaritycovariance::gblemp`](https://cran.r-project.org/package=lacunaritycovariance); box-counting definitions | Declared mask, pixel size, grid origin and box sizes. 40 public CODEX cell masks agree with gblemp 1.1-9 (lacunarity error ≤8.3e-13) and an independent R box count (regression error ≤1.1e-15). The dimension is a descriptive slope over declared scales without uncertainty, not evidence that tissue is fractal. |
| Landy–Szalay two-point estimator | [Landy–Szalay ξ against a declared random catalog](structure-descriptors.md#landyszalay-two-point-correlation-numerics-landy-szalay) implemented (Research) | [TreeCorr](https://rmjarvis.github.io/TreeCorr/) (Corrfunc did not build here) | The caller supplies the random catalog that samples the tissue. Pair counts on 15 public CODEX patterns are identical to TreeCorr 5.1.4 and ξ matches the 1993 normalization exactly; TreeCorr's own `n²/2` normalization differs by up to 0.19 for 38 cells. No uncertainty is reported. |
| Diversity indices | Shannon entropy/effective class count implemented; other ecological indices out of scope | Independent Python composition formula; adjacent [vegan](https://search.r-project.org/CRAN/refmans/vegan/html/diversity.html) | Soft-class composition compares against the checked-in Python oracle, including effective class count 2.8387205126507515. This synthetic profile does not establish native vegan parity or validate biological diversity estimates. |
| Morisita dispersion, ecological co-occurrence/niche models | [Morisita dispersion](structure-descriptors.md#morisita-quadrat-dispersion-numerics-morisita-dispersion) implemented (Research); niche models planned | `vegan::dispindmorisita` / ecological count models | Declared quadrat width and origin; only whole-tissue quadrats count. 30 public CODEX cases agree with vegan 2.7-6: identical counts, index error ≤2.0e-16, p-value ≤5.8e-15. Ecological association/niche models need a specified abundance/environment model; categorical composition alone supplies neither. |
| Kulldorff-style Bernoulli scan | Stratified circular scan implemented; general SaTScan parity out of scope | Exact enumeration and [Kulldorff's likelihood ratio](https://www.tandfonline.com/doi/abs/10.1080/03610929708831995) | The six-cell test enumerates all 20 fixed-count allocations, checks the score within 1e-12 and the Monte Carlo p-value against exact 2/20 within 0.025. Public-data validation remains absent; this is Research evidence. |
| Variograms | Implemented | [SciKit-GStat](https://mmaelicke.github.io/scikit-gstat/) | [Public two-marker empirical variogram](variograms.md): 1,107 CODEX cells, exact bin counts, scaled error ≤3.98e-15 (limit 1e-12). Weighted/projected/scalar-permutation variants retain separate Research scope. |
| Kriging | [Ordinary kriging](structure-descriptors.md#ordinary-kriging-numerics-ordinary-kriging) with a declared variogram and leave-one-block-out checks implemented (Research) | [SciKit-GStat](https://mmaelicke.github.io/scikit-gstat/reference/kriging.html) | 600 public CODEX cells: predictions agree with SciKit-GStat 1.0.23 within 4.2e-14 and variances within 4.9e-15 (limit 1e-8). The variogram fit and the constant-mean/stationarity assumptions remain the user's; the held-out summary checks them. |
| Persistent homology | Implemented via pinned backend (Research) | GUDHI 3.13.0 | Alpha/witness persistence, diagrams and stability tests; no cohort biomarker claim. GUDHI is the implementation, so no independent persistence comparison exists yet. |
| Mapper | [Mapper graph of cell features](structure-descriptors.md#mapper-graph-of-cell-features-numerics-mapper-graph) implemented (Research) | [KeplerMapper](https://kepler-mapper.scikit-tda.org/en/latest/) | One-dimensional lens, uniform interval cover, single-linkage clusters at a declared threshold. 12 graphs on public CODEX cells are identical to KeplerMapper 2.1.0 with DBSCAN (`min_samples=1`). No stability or significance assessment; multi-dimensional lenses are not implemented. |
| Optimal transport between patterns | Implemented | Python Optimal Transport | Balanced/unbalanced Sinkhorn, partial/full fused Gromov–Wasserstein and [unbalanced fused Gromov–Wasserstein with KL-relaxed marginals](research-workflows.md#transport-alignment-bayes--fused-gromov-wasserstein); analytic and independent-optimizer known answers for the unbalanced form; biological correspondence unvalidated. |
| Graph motifs and communities | Motifs implemented; native community fitting out of scope | NetworkX / Leiden | Typed triangles have exhaustive known-answer checks; spectra/Hodge have analytic checks. Community fitting adds an objective, resolution and cross-sample label-alignment problem; no learned-community or niche claim is made. |
| Bond-orientational order | [k-atic order ψ_k](structure-descriptors.md#bond-orientational-order-numerics-bond-orientational-order) implemented (Research); Steinhardt 3-D order out of scope | [freud](https://freud.readthedocs.io/en/latest/modules/order.html) | Declared nearest-count or radius neighbors and a border rule. On 734 public tumor cells, double-precision ψ₆ on freud 3.6.1's neighbor lists agrees within 2.2e-16; freud's own single-precision values differ by up to 7.0e-4, failing the prespecified single-precision bound in 5 of 6 cases. Crystal ordering and tissue-cell anisotropy answer different questions. |
| Information-theoretic dependence | Entropy implemented; [mutual information between two marks](structure-descriptors.md#mutual-information-between-two-marks-numerics-mutual-information) implemented (Research) | [scikit-learn MI](https://scikit-learn.org/stable/modules/generated/sklearn.feature_selection.mutual_info_regression.html) | KSG estimates agree with scikit-learn 1.9.1 in 59 of 60 public cases (≤3.6e-14); one failed by 1.07e-5 where scikit-learn's tie-breaking noise splits a tied value. Under autocorrelated independent fields the variogram-matching null held size (0.040–0.065 at ranges 10–200 µm) while ordinary permutation rejected up to 0.89. Calibration covers one spot's geometry and Gaussian fields; entropy/covariance/kernel dependence are not claimed as MI. |
| 3-D, causal, longitudinal, simulation and SBI | Implemented research families | GUDHI, SciPy, PyMC/NumPyro, analytic examples | Research tier; see [research workflows](research-workflows.md). No independent reference comparison or public real-data demonstration yet. |

**Planned** rows are missing work, not deliberate boundaries: ecological niche models,
a native HoVer-Net adapter, and comparison with SPIAT's distance metrics.

## Command maturity

Each row applies to the named command **and every subcommand**, unless a more specific row
overrides it. Cached `project` routes inherit their method's evidence. Research is a ceiling
where an individual model still lacks a complete public demonstration; a command named
`validate` or `agreement` does not certify every model or configuration it can run.

| Command family | Tier | Evidence and limit |
|---|---|---|
| `analyze`, `batch` | Research | Known-answer smoke tests and [shuffle calibration](validation-methodology.md#scheduled-calibration) of the shipped primary endpoint: no evidence of size above 5% (observed 3.9%–6.2%) across five layouts and three prevalences, power ≥ 95% for small foci. One clone covering a quarter of a small window was detected in 0.6% of replicates. No independent implementation agreement or public patient cohort. Batch adds no scientific evidence. |
| `classical` | Experimental for observed rectangular border, translation and isotropic K/L; Research for CSR inference | Hand-counted K/L and public spatstat agreement; the observed-curve comparison does not calibrate the CSR test or its null distribution. |
| `nearest-space` | Experimental for the demonstrated rectangular observed F/G/J curves; Research for inference | SciPy known answers and exact public amacrine/betacells reduced-sample F/G and matched-J agreement; no patient cohort or multi-shape calibration claim. |
| `prepost` | Research | Axis/descriptive-change tests; not a paired-patient causal or equivalence claim. |
| `multimodal` | Research | Synthetic registration/neighborhood/factor tests and [shuffle calibration](validation-methodology.md#scheduled-calibration) of the lymphocyte/MMR enrichment under all four source-section null models: conservative size (1.2%–4.2%) on two layouts and two prevalences, full power on the immune-association scenario. No validated paired H&E/IHC cohort. |
| `cohort` | Experimental for two-group `max-t`; Research for other designs | Patient-unit exact/synthetic checks plus independent real-group Max-T agreement on Schürch and Jackson. Other designs are not promoted by these two-group examples. |
| `study` | Experimental for `binary_symmetric_including_isolates` Moran/Geary with two-group Max-T; Research for other weight policies | Synthetic/oracle, durable-resume and [Schürch multiplex cohort](validation-schurch.md) checks, which used only that weight policy; no multi-shape calibration or clinical validation. |
| `pathology maps`, `pathology composition`, `pathology scan` | Research | Hand/exhaustive examples and synthetic calibration; public demonstration pending. |
| `pathology architecture` | Research | Exact square, offset, lumen, turning-angle, skeleton and hotspot checks; no real-data, pathologist-front or pathologist budding comparison. |
| `pathology spatial-study` | Research overall; Experimental for fixed-bandwidth rectangular L-minus-r | Published retinal K/L agreement and analytic intensity check. Patient groups, g/cross-g, irregular windows and fitted-null inference not covered. |
| `bayes` | Research | Model-specific analytic, worker, SBC and backend checks. Nonconverged fits remain diagnostic only. No blanket model calibration claim. |
| `bayes vector-semivariogram` | Experimental for the unweighted observed curve | Hand-computed/rotation tests and [public SciKit-GStat agreement](variograms.md); this descriptive command does not fit a Bayesian model. Weighted and inferential extensions remain Research. |
| `project` | Inherits the underlying method; Research unless specifically promoted | Durable transaction/identity/resume and method tests; caching does not promote scientific maturity. |
| `project categorical-pair` | Experimental for directed homogeneous cross-K/L with standard border correction; Research otherwise | [Public spatstat `Kcross` agreement](categorical-pairs.md) in four directions; translation/isotropic variants and inhomogeneous cross-K are not covered. |
| `graph` | Research | Matrix/graph oracles and synthetic signal/permutation tests; no tissue-cohort validation. [Research workflows](research-workflows.md#graphs-graph). |
| `graph group-centrality` | Experimental for the three observed group scores | Hand/disconnected graph tests and [public Squidpy/NetworkX agreement](group-centrality.md); no inferential or cohort-biomarker claim. |
| `topology` | Research | GUDHI/scikit-image and known topology/stability tests; tissue biomarker interpretation unvalidated. |
| `spatial3d` | Research | Cuboid/voxel/registration and known 3-D answers; real serial-section patient claims unvalidated. |
| `simulate` | Research | Solver/conservation/bound/seeded-process tests; no fitted mechanistic tissue validation. |
| `neural` | Research | Synthetic point-process/generator/SBI checks; no image-model training or real-cohort performance claim. |
| `causal` | Research | Analytic/synthetic identification and sensitivity controls; observational tissue association is not causation. |
| `longitudinal` | Research | Analytic/synthetic filter/smoother tests; no repeated-patient clinical validation. |
| `registration` | Research | Synthetic deformation/landmark checks; deformation alone does not establish biological change. |
| `numerics` | Research | Known numerical answers and finite/resource checks; local Moran has its own null conventions. |
| `numerics categorical-co-occurrence` | Experimental for observed radius-dependent scores | Hand-counted duplicate/boundary cases and [public Squidpy agreement](categorical-cooccurrence.md); no calibrated null or patient test. |
| `numerics categorical-enrichment` | Experimental for observed counts/exact null descriptors; Research for Monte Carlo inference | Hand-counted fixed-margin cases and [public count/moment agreement](categorical-enrichment.md). Monte Carlo z-score package agreement and multi-shape calibration remain unassessed. |
| `inspect-slide`, `extract-region` | Experimental for the tested Aperio region; Research ceiling for other codecs | Public-file/OpenSlide pixel agreement plus codec fixtures and bounded reads; no statistical inference or universal scanner/OME guarantee. |
| `smoke` | Research evidence generator | Production synthetic controls; smoke success is not calibration. |
| `backend doctor` | Research ceiling; utility | Locked-package/runtime checks; not model suitability or numerical accuracy. |
| `policy` | Research ceiling; utility | Evaluates declared flags; does not independently verify evidence. |
| `profile-plan`, `help` | Research ceiling; utility | Profiling instructions/help only; no scientific claim. |

## Work priorities

1. **Calibration:** `analyze` and `multimodal` now have size and power across several layouts,
   prevalences and null models. Next are stratified permutation, the default 999
   permutations, and the pathology workflows. Unsupported cases remain support tests, not
   size or power evidence. Fitted-null and irregular-window inference stay scoped until
   checked.
2. **Planned methods:** the Planned rows above, each with a concrete tissue question, a
   reference comparison and documented failure modes before promotion beyond Research.
3. **Research families:** independent reference comparisons and public real-data
   demonstrations for the [research workflows](research-workflows.md).

## One tissue section

**`marklab analyze`** (Research) takes cells with a yes/no label, or a probability, and tests
whether the labeled cells are arranged differently than if the labels were shuffled at
random among the same cells. It reports:

- whether labeled cells sit closer to each other than chance predicts (clustering) or
  farther apart (dispersion), across a range of distances;
- whether the pattern has a preferred direction;
- the size scales at which labeled cells concentrate, from a spectrum of the pattern;
- maps of the regions where labeled cells are over-represented.

**`marklab classical`** and **`marklab nearest-space`** apply standard point-pattern
statistics to all retained cells, ignoring the label: Ripley's K and L functions,
nearest-neighbor distances and empty-space distances, each compared with completely
random placement. The demonstrated rectangular observed curves are Experimental;
their inferential extensions remain Research. See the
[classical workflow](classical-spatial-workflow.md).

The directed categorical cross-K/L workflow has [public-data spatstat agreement](categorical-pairs.md)
and is Experimental for its documented standard border correction.

## Comparing sections

**`marklab prepost`** (Research) compares two analyzed sections, such as one taken before
treatment and one after. It describes how each measurement changed. The two sections are
different pieces of tissue, so this is not a test of what happened to the same cells.

**`marklab multimodal`** (Research) analyzes an H&E and an IHC section of the same tissue together.
It aligns them, measures how different cell types sit relative to each other, and fits
factor models across the two stains. Its spatial factor can use a dense Gaussian-process prior
or a mesh-based SPDE prior; the two recover the same synthetic factor
([SPDE spatial factor](python-backends.md#spde-spatial-factor)).

## Many patients

**`marklab cohort`** (Experimental for two-group Max-T; Research for other designs) compares
groups of patients. The patient is the unit of evidence: a
patient with 10,000 cells counts once, the same as a patient with 500. It supports
paired, blocked and clustered designs, bootstrap intervals, Max-T correction across many
measurements, and equivalence tests against a margin you set in advance.

**`marklab study`** (Experimental) runs a whole multiplex panel, with several markers
measured per cell, across slides and patients, from cell tables to a patient-level report.
It can resume after an interruption, and a thin Python client reads AnnData files. See
[multiplex studies](multiplex-study.md).

## Pathology maps and studies

**`marklab pathology maps`** (Research) measures how marker levels and cell density
change with distance from annotated structures, maps the mix of cell types across the
tissue, and finds local hotspots and outliers for a single marker. It uses annotations and
measurements you supply; it does not draw annotations itself. See
[pathology maps](pathology-maps.md).

**`marklab pathology architecture`** (Research) derives the advancing tumor front from
declared epithelium polygons and reports glandular, solid and fragmented epithelium,
fragmentation, lumens, curvature, branching, tumor buds and nuclear shape in distance
bands from that front, with an ITBCC-style budding hotspot. Region
polygons and classes come from you; it does not segment tissue. See
[pathology architecture](pathology-architecture.md).

**`marklab pathology composition`, `spatial-study` and `scan`** (Research) compare
cell-type composition and density between patients, fit spatial curves, and scan for
regions enriched in a cell type. They have been checked on 23 synthetic scenarios but not
yet calibrated on real studies. Sparse cell types can use a support rule declared before
analysis, with synthetic size and power evidence. See
[pathology studies](pathology-studies.md#sparse-cell-types).

## Bayesian models

**`marklab bayes`** and **`marklab project`** (Research) fit Bayesian models with
pinned PyMC and NumPyro environments: patient and slide hierarchies, spatial
point-process models (log-Gaussian Cox processes), Gaussian processes, spatial regression
(CAR, SAR, BYM) and checks of how much a conclusion depends on the prior. Every fit reports
its sampling diagnostics (divergences, R-hat, effective sample size). A fit that does not
converge is published as non-converged instead of being presented as a result. `project`
saves finished fits, so re-running a pipeline skips work that is already done.

`bayes advanced-cluster` fits a [Bayesian Thomas cluster model](bayesian-thomas-cluster.md):
latent parent cells sampled by birth–death MCMC, cluster size and spread pooled across
patients, and simulation-based calibration through `bayes advanced-cluster-sbc`.

[Normal-mean prior sensitivity](normal-mean-prior-sensitivity.md) runs entirely in Rust;
the other models need the [Python environment](python-backends.md).

`bayes anisotropic-gp-3d` requires pairwise squared displacements that can distinguish
the three axis length scales. Variation on each axis alone is insufficient; planar
data can still satisfy this condition. Admission accounts for constructing and
factoring the joint prediction covariance for every posterior draw.

The existing `posterior_mean_anisotropy_metric` field is a plug-in metric computed
from the posterior mean lengths: `diag(1 / mean(length_scale_axis)^2)`. It is not an
average of the per-draw inverse-square lengths, whose posterior moments need not
be finite under the current half-normal priors. Its units are inverse square
micrometres, so it cannot be used directly as the dimensionless anisotropy matrix
for physical-radius K analyses. The legacy key and result shape are retained.

The crossed/nested Gaussian hierarchy's `variance_partition` reports posterior mean
shares of random-effect and residual variance, averaged over the supplied observation
rows. The calculation includes the effects of sum-to-zero centering and squared
standardized exposure for random slopes. It excludes fixed effects and is neither an
R-squared nor the sample variance of the fitted random-effect values.

`bayes normal-mean-sbc` checks sampled posterior ranks against their discrete-uniform
distribution. Its coverage diagnostic uses analytic 95% posterior intervals, so the
coverage target stays 95% even when few posterior draws are requested.

## Embeddings and tissue structure

Marklab reads feature vectors produced by image models (embeddings) for cells, patches,
regions and slides, checks that they line up with the cell coordinates, and summarizes
how they vary across the tissue. It also measures tissue interfaces and compartments,
such as how cells sit along a tumor–stroma boundary and how fragmented a compartment is.

## Research workflows

3-D tissue (`spatial3d`), graphs (`graph`), topology (`topology`), time series
(`longitudinal`), causal designs (`causal`) and simulation (`simulate`, `neural`) are
research tools. They are useful for method development and are less tested than the
workflows above. [Research workflows](research-workflows.md) describes what each family
measures, its assumptions and what it does not establish.

`spatial3d clone-models` fits patient intercepts jointly with the clone A-minus-B contrast
and residual scale. Contrast standard errors account for the free nuisance parameters;
when the residual SD reaches its existing numerical floor, they are conditional on that
floor. Unidentifiable contrasts and fits at the contrast bound produce an error.
`patient_feature_means` still contains the observed patient means, not fitted intercepts.

## Execution contracts

- **Reproducible**: fixed input, settings, seed, implementation and numerical environment
  support replay. Cross-platform bitwise identity is not established for every backend.
- **No silent zeros**: a measurement that cannot be computed (too few cells, region too
  small) is reported as unavailable with the reason.
- **Bounded**: commands have limits on work and memory and stop with an error instead of
  running out of memory.
- **Transactional directories**: study/analysis publication exposes a complete output
  directory only after the run succeeds. File-producing utilities have their own contracts.

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

## Known limitations

- **`bayes advanced-cluster`** mixes slowly on dense cell aggregates. On three public B-cell
  spots, four chains of 8,000 draws converged for one of four fits under the default policy,
  and the single-scale Thomas model under-predicted close neighbours. Non-converged fits are
  published as such and are diagnostic only. See the
  [Bayesian Thomas cluster model](bayesian-thomas-cluster.md).
- The gridding-sensitivity check for FFT-based statistics uses a fixed 25% tolerance as a
  rough guide. It is not a formal agreement test.
- A real-data pilot of the joint location–embedding model did not converge. Treat its
  output as diagnostic only.
- Linux is the CI reference platform for the Python backends. Changes that have not yet
  run in CI have macOS evidence only; macOS results cannot substitute for Linux.

Windows binaries are built for each release but are not yet tested end to end.
