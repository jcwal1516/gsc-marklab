# SPACE: ensembles of cell types and markers

**Research** tier. `marklab space` ports SPACE (Spatial Patterning Analysis of Cellular
Ensembles) to Rust. SPACE ranks every combination ("ensemble") of up to `depth` cell types and
per-cell values by how non-randomly the combination is patterned in tissue. A pair can
co-occur or avoid each other; three or more variables can show context-dependent patterns that
no pair reveals.

The algorithms follow the R package by Edward Schrom
([github.com/eschrom/SPACE](https://github.com/eschrom/SPACE), commit
`94f0a0f9311e2ee2b406ebc45c84a4e93f2f38f8`, Apache-2.0; Schrom et al., *PNAS* 2025,
[doi:10.1073/pnas.2412146122](https://doi.org/10.1073/pnas.2412146122)). This port is a
re-implementation for cell tables. It is not affiliated with or endorsed by the SPACE authors.
The [differences from R SPACE](#differences-from-r-space) are listed below.

This release implements:
- the census;
- **cisMI**, patterning within one specimen;
- **transMI**, patterning that separates groups of specimens;
- the data behind SPACE's distribution and ranking plots;
- **patterns**: how an ensemble's variables covary along a path through its discs, and their
  enrichment against a reference;
- **maps**: each cell's region of a pattern's path.

Diversity and image inputs are planned.

```sh
marklab space describe
marklab space census --request request.json --out census.json --table census.csv
marklab space cis --request request.json --out cis.json --table cis.csv \
  --depth 3 --bootstraps 100
marklab space trans --request request.json --out trans.json --table trans.csv \
  --depth 3 --bootstraps 100
marklab space dist --request request.json --variables cell_type.T,cell_type.B --radius 30 \
  --bins 10 --out dist.json --table dist.csv
marklab space rank --result cis.json --radius 30 --out rank.json --table rank.csv \
  --variable-table rank-variables.csv
marklab space pattern --request request.json --variables cell_type.T,cell_type.B --radius 30 \
  --focal responder --reference non_responder --out pattern.json --table pattern.csv
marklab space map --pattern pattern.json --request request.json --regions 0:30,70:100 \
  --out map.json --table map.csv
```

Each output file must not already exist.

## Request

A request (`marklab.space_request`, version 1) names a source of cells, the census variables
and the census design. Unknown fields are errors. File paths in a request are resolved
relative to the request file.

### Sources

Cells come directly from inputs Marklab already reads. You do not need a SPACE-style CSV.

- **`inline`**: `channels` (as in a [multiplex study recipe](multiplex-study.md#recipe)) and
  `specimens`. Each specimen has `specimen_id`, optional `groups` (factor → level),
  `coordinate_frame_id`, a GeoJSON MultiPolygon `window`, `cell_ids`, `coordinates_um` and
  `observations` (channel → one value per cell). A categorical value is an index into the
  channel's `levels`, and `null` means unclassified. Scalars are numbers.
- **`multiplex_study_recipe`**: `path` to a [multiplex study recipe](multiplex-study.md).
  Every slide becomes a specimen and its group becomes the factor `group`. The recipe's Moran
  design is ignored.
- **`cellvit_he_csv`**: `specimens`, one entry per CellViT H&E classification table (`cell_id`,
  `x_centroid_um`, `y_centroid_um`, `predicted_class`, `class_probability`; the
  [multimodal](capabilities.md) reader), each with `path`, `window_path`, `specimen_id`,
  `coordinate_frame_id` and optional `groups`; plus an optional `minimum_class_probability`
  (rows below it are rejected). The predicted class becomes the categorical channel
  `cellvit_class`. Its classes are the normalized class names of every table, in sorted order,
  so a class means the same in each specimen. The probability becomes the continuous channel
  `class_probability`.

Every cell must lie inside its specimen's window, and cell IDs must be unique within a
specimen.

### Variables

- **`objects`**: categorical channels used as object maps. Each class with at least one cell
  in a specimen is a variable named `channel.class`. Its value in a disc is the percentage of
  the disc's classified cells that belong to that class. Unclassified cells play the part of
  SPACE's image background: they are not counted.
- **`linked_scalars`**: `{channel, class, scalar}`, SPACE's object–scalar pairs. The variable
  `channel.class_scalar` is the mean of the scalar over the disc's cells of that class, or 0
  when the disc has none.
- **`independent_scalars`**: scalar channels whose variable is the mean over every cell in
  the disc. SPACE has independent scalars only for images; this is their cell-table form.
- **`merge`**: `{channel, classes, into}` merges classes before the census (SPACE
  `merge_objects`). The merged class takes the position of the first class it absorbs.

Scalar channels used as variables must be observed on every cell. A missing value is an
error, never a zero. Scalar values must be finite with magnitude at most 1e150, so disc means
and the squared distances used in binning cannot overflow.

### Census

`census` holds:

- `radii_um`: strictly increasing;
- `sample_size`: discs per radius, drawn without replacement;
- `seed_classes`: optional; a channel-to-classes map restricting which cells may be disc
  centres (SPACE `seed_points`). When absent, any classified cell may be a centre. Channels not
  listed contribute no centres;
- `seed_cell_ids`: optional; explicit centres per specimen, one list per radius;
- `seed`.

A disc holds every cell within the radius of its centre, including the centre and cells
exactly at the radius, as R's `FNN` query does. Discs are not corrected for the window edge,
also as in SPACE.

Optional `limits` bound the following, each within a fixed hard maximum:

- cells per specimen;
- specimens;
- variables;
- disc members;
- enumerated ensembles;
- bootstraps;
- analysis work: for cisMI, ensembles × (bootstraps + 1) × discs; for transMI, each
  ensemble's discs, grid cells and divergence pairs, and its modularity evaluations;
- retained memory.

## cisMI

For each specimen and radius:

1. Draw `bootstraps` randomized censuses (SPACE `random_census`). Each object channel's disc
   members are pooled across discs, shuffled, and re-split at the original disc sizes. Each
   independent scalar is shuffled the same way, separately. Linked scalars move with their
   cell.
2. Bin every variable into `b = min(floor(((ss + 1)/10)^(1/depth)), max_bins)` equally spaced
   points over its range in the observed and all randomized censuses. `ss` is the smallest disc
   count over the analysed radii. A value goes to the nearest point; exact ties resolve as R's
   `FNN` kd-tree resolves them.
3. Compute plug-in joint entropy in bits for every ensemble of up to `depth` variables, then
   co-information by inclusion–exclusion: entropy at depth one, mutual information at two,
   interaction information above that.
4. Report:
   - `observed_mi`, `null_mean_mi` and `null_sd_mi`;
   - `cis_mi = observed − null mean` and `z_score = cis_mi / null sd`;
   - SPACE's normal-approximation `p_value_normal = 2Φ(−|Z|)`;
   - its Benjamini–Hochberg adjustment `p_adjust_bh` over every ensemble at the radius;
   - a Monte Carlo `p_value_monte_carlo = (1 + #{b: |null_b − mean| ≥ |cis_mi|})/(1 + B)` from
     the same draws.

   Each radius also reports `variable_ranges`, the `[minimum, maximum]` that spans each
   variable's grid. The normal tail uses Cody's rational approximation of `erfc`; on a dense
   grid of |Z| up to 38, its largest relative error against R's `pnorm` was 1.9e-13.

Parameters mirror R `measure_cisMI`:

| Parameter | Default | Meaning |
|---|---|---|
| `--depth` | 3 | Largest ensemble size. R has no default; 3 is the SPACE tutorial value. |
| `--bootstraps` | 100 | Randomized censuses per radius (at least 2). |
| `--all` | none | Variables every largest ensemble must contain. |
| `--alo` | none | Variables of which every largest ensemble contains at least one. |
| `--not` | none | Variables excluded everywhere. |
| `--max-bins` | 100 | Upper bound on bins per variable. |
| `--radii` | all | Census radii to analyse. |
| `--specimen` | all | Specimens to analyse. |

As in R, `all` and `alo` filter only the largest ensembles; smaller ones are still reported
and used in the subtraction. When `all` plus one `alo` slot fills the depth, every other
variable is dropped.

## transMI

transMI asks which ensembles are patterned differently in different groups of specimens, for
each grouping factor in the specimens' `groups` (for example clinical status or treatment). For
each radius:

1. Census every compared specimen and round each variable onto `b` equally spaced points over
   that specimen's own range. `b = min(floor(((ss + 1)/10)^(1/depth)), max_bins)`, where `ss` is
   the smallest disc count over the compared specimens and analysed radii.
2. For every ensemble and specimen, build the joint distribution over all `b^k` grid cells
   (R `build_dist` and `smooth_dist`):
   - cells where two or more objects of the same channel would sum to more than 100% are
     removed;
   - when some cells are empty, the observed frequencies are scaled down and the empty cells
     share a Chao–Jost estimate of the unseen probability mass (R's formula, including its
     counts of singletons and doubletons).
3. For every pair of specimens, average the Kullback–Leibler divergences in bits in both
   directions, over the cells both distributions keep.
4. Turn the divergences into similarities `1 − D/max(D)` (each specimen's similarity to itself
   is 1). Score the factor's groups by igraph's directed weighted modularity on that complete
   network. This modularity is the reported `trans_mi`.
5. Shuffle the pairwise divergences `bootstraps` times and score each shuffled network. Report:
   - `null_mean` and `null_sd`;
   - `z_score = (trans_mi − null_mean)/null_sd`, set to 0 when `null_sd` is 0, as in R;
   - SPACE's one-sided `p_value_normal = 1 − Φ(Z)`;
   - its Benjamini–Hochberg adjustment `p_adjust_bh` over every ensemble at the radius,
     separately for each factor;
   - a Monte Carlo `p_value_monte_carlo = (1 + #{b: null_b ≥ trans_mi})/(1 + B)`.

The variables are those present in any compared specimen. A class a specimen lacks is 0 in each
of its discs, as R's `standardize_censuses` makes it. Every compared specimen needs a level for
every tested factor.

Parameters mirror R `measure_transMI`, with the same defaults as cisMI:

| Parameter | Default | Meaning |
|---|---|---|
| `--depth` | 3 | Largest ensemble size. |
| `--bootstraps` | 100 | Shuffles of the pairwise divergences (at least 2). |
| `--all`, `--alo`, `--not` | none | As for cisMI. |
| `--max-bins` | 100 | Upper bound on bins per variable. R accepts it but does not apply it. |
| `--radii` | all | Census radii to analyse. |
| `--specimen` | all | Specimens to compare (at least two). |
| `--factor` | all | Grouping factors to test. |

## Distributions

`space dist` returns the data of R `plot_dist` without the plot: how one or two variables
co-occur across one specimen's discs at one radius.

- Without `--bins`, it reports each disc's values.
- With `--bins b`, it reports every combination of the variables' values and its smoothed
  probability, the first variable varying fastest. A variable with more than `b` distinct values
  is rounded onto `b` equally spaced points between its minimum and maximum in this census; one
  with fewer keeps its distinct values. Smoothing and the removal of impossible compositions
  follow transMI.
- With `--randomized`, it describes one randomized census, drawn as cisMI draws its nulls,
  instead of the observed one (R's `patch_list` argument).

| Parameter | Default | Meaning |
|---|---|---|
| `--variables` | required | One or two census variables (R `ensemble`). |
| `--radius` | required | Census radius in µm. |
| `--specimen` | the only one | Specimen to describe. |
| `--bins` | none | Rounding bins per variable, 2 to 255 (R `bin_num`). |
| `--randomized` | off | Describe a randomized census. |

The JSON also gives each variable's minimum and maximum, the axis plotted when R zooms.

## Rankings

`space rank` reads a cisMI or transMI result and returns the data of R `plot_MI_rank` and
`plot_MI_radius` without the plots.

At one radius (`--radius`), it keeps the ensembles that pass every filter and orders them by
decreasing |Z|, keeping ties in their original order:
- P at most `--p-threshold`;
- |cisMI|, or transMI itself, at least `--mi-threshold`;
- the `--depth`, `--all`, `--alo` and `--not` filters, which here apply at every ensemble size.

It then scores each variable in the kept ensembles: the sum of their |Z| over the ensembles
containing it, divided by the number kept. With `--ensemble`, it instead reports that ensemble's
statistics at every radius of the result.

| Parameter | Default | Meaning |
|---|---|---|
| `--result` | required | A `marklab.space_cis_result` or `marklab.space_trans_result` file. |
| `--radius` | none | Radius whose ensembles are ranked. |
| `--specimen` | the only one | Specimen of a cisMI result. |
| `--factor` | the only one | Grouping factor of a transMI result (R `group`). |
| `--depth` | all | Ensemble sizes to rank. |
| `--p-threshold` | 0.05 | Largest P kept (R `p_thr`). |
| `--mi-threshold` | 0.1 for cisMI, 0 for transMI | Smallest statistic kept (R `mi_thr`). |
| `--p-adjust` | true | Filter on the BH P; `false` uses the normal-approximation P (R `p_adj`). |
| `--all`, `--alo`, `--not` | none | Variables required, at least one required, or excluded. |
| `--ensemble` | none | Follow this ensemble across radii instead (R `plot_MI_radius`). |

`--table` writes the ranked ensembles, or the ensemble at each radius. `--variable-table`
writes the ranked variables.

## Patterns

`space pattern` returns the data of R `learn_pattern` without the plot. It draws a
one-dimensional path through the densest parts of an ensemble's joint distribution, orders the
discs along it, and reports how each variable changes along the path.

**Which discs.** The focal discs draw the path:
- by default, the discs of every specimen (or of `--specimen`), pooled;
- with `--focal` and `--reference`, the discs of specimens at those levels of a grouping factor.
  The factor is `--factor`, or else the one factor whose levels include all of them, as R infers
  it;
- with `--randomized-reference`, one specimen, compared with one randomized census of itself
  (R's `patch_list` argument), drawn as cisMI draws its nulls.

**The map.** A self-organizing map of `min(discs, 1000)` nodes on a chain, or a ring with
`--toroidal`, is trained on the focal discs. Marklab re-implements kohonen 3.0.13's online
algorithm from its documented and measured behaviour; kohonen is GPL, and none of its code is
used. The nodes start at randomly chosen discs, and each of `--som-reps` passes presents as
many randomly drawn discs. The learning rate falls linearly from 0.05 to 0.01, and the
neighbourhood radius falls from kohonen's default (the 2/3 quantile of the distances between
nodes) to 0. Fixed presentation orders reproduce kohonen's nodes to 1e-12 in the unit tests.

**The path.** It starts at the end of the chain nearer the origin, or for a ring where R starts
it. Each disc goes to its nearest node, and the discs are ordered by node, keeping their order
within a node. Along this order, each variable's mean and its `--conf-int` band (type-7
quantiles) are taken over a rolling window of `2 · round(w/2) + 1` discs, `w` being
`--smooth-window`. The ends are padded by drawing discs from the first and last nodes. The band's
edges are smoothed once more and widened where needed to contain the mean. Positions run from
0 to 100 in equal steps, one per focal disc.

**Enrichment** against a reference: each disc is weighted by how close it lies to its node,
`(√k − d/100)/√k` for `k` variables at Euclidean distance `d`. The log ratio of the focal and
reference weighted frequencies at each node is then smoothed along the path like the curves.
A node without reference discs takes the largest finite ratio. Without a reference, enrichment
is 1 everywhere, as in R.

**Normalization.** `--obj-norm` (objects) and `--scl-norm` (linked and independent scalars)
multiply curves by 100 divided by the largest upper band edge: that of each curve
(`individual`) or of all curves of that kind (`collective`). The rescaled curves are reported
beside the raw ones with the scale used.

| Parameter | Default | Meaning |
|---|---|---|
| `--variables` | required | The ensemble's variables. |
| `--radius` | required | Census radius in µm. |
| `--specimen` | all | Specimens whose discs are used. |
| `--factor` | inferred | Grouping factor holding the focal and reference levels (R `group`). |
| `--focal`, `--reference` | none | Levels whose discs draw the path, and the reference. |
| `--randomized-reference` | off | Compare one specimen with a randomized census of itself. |
| `--conf-int` | 0.95 | Coverage of the rolling band. |
| `--obj-norm` | `none` | `none`, `individual` or `collective` (R `obj_norm`: NULL, "ind", "all"). |
| `--scl-norm` | `collective` | As `--obj-norm`, for scalars (R `scl_norm`). |
| `--som-reps` | 50 | Training passes over the focal discs. |
| `--toroidal` | off | Train a ring instead of a chain. |
| `--smooth-window` | 100 | Discs smoothed together along the path. |
| `--sub-sample` | off | Subsample the focal discs, and the reference discs, to 10,000 each. |

The JSON gives the trained nodes in path order and, for each variable, the curves at every
position. `--table` writes one row per position and variable.

## Mapping patterns onto cells

`space map` assigns cells to regions of a learned pattern's path, which R `map_pattern` does
for image pixels. It reads a pattern result and a request that declares the pattern's
variables and radius, usually the same request.

- `--regions` lists increasing, non-overlapping `lower:upper` ranges of path position, numbered
  1, 2, and so on. Any gaps before, between or after them form one more region, numbered last,
  as in R.
- Each disc of each mapped specimen goes to the nearest point on the pattern's mean curves
  (the first on ties), and so to that point's region. It votes for that region in every cell it
  contains, with weight `1 − (d/max d + 0.0001)`, `max d` being the largest such distance over
  the mapped discs.
- Each cell joins the region with the most votes, the first on ties. A cell whose best vote
  is below R's background vote of 0.000001, such as one that no disc contains, has no region.

| Parameter | Default | Meaning |
|---|---|---|
| `--pattern` | required | A `marklab.space_pattern_result` file. |
| `--request` | required | Request whose specimens are mapped. |
| `--regions` | required | Path ranges, for example `0:30,70:100` (R `region_bounds`). |
| `--specimen` | all | Specimens to map. |

The census follows the request: with the pattern's own request, the discs are the ones the
pattern was learned from. `--table` writes one row per cell (`specimen_id, cell_id, region`).

## Interpreting the results

- The randomized censuses keep each disc's size and each layer's overall composition. They
  break where cells sit relative to one another, so cisMI measures any departure from a
  well-mixed tissue, including large-scale regional structure. It is not a test of direct
  cell–cell interaction.
- **Each layer is shuffled separately.** A per-cell association between an object class and
  an independent scalar, or between two categorical channels, therefore counts as
  co-patterning, as pixel co-localization does in SPACE images. To treat a value purely as a
  property of a class, link it to that class instead.
- Discs overlap and can share cells. The normal-approximation P values assume an approximately
  normal null and are best read as a ranking. The Monte Carlo P value cannot fall below
  `1/(1 + B)`.
- **Rare variables are unstable.** Each grid spans the variable's range over the observed and
  all randomized censuses, so a variable present in very few discs has a grid set by the most
  extreme randomized value. Rerunning with another seed can move its observed MI between
  discrete values, and its ensembles with it. R SPACE behaves the same way. Check
  `variable_ranges`, and treat ensembles of variables present in only a few discs as unstable,
  or merge or exclude those classes.
- cisMI results describe one specimen. They are not patient-level inference.
- **A pattern's path is an ordering, not a distance.** Positions are ranks of discs along the
  map, so equal steps along the path need not be equal changes in composition. Different seeds
  train different maps; the curves agree in shape, not position by position.
- **transMI compares distributions, not values.** Each specimen is rounded on its own range,
  so transMI responds to the shape of an ensemble's distribution over discs. Two specimens with
  the same shape at different overall levels look alike.
- **transMI needs several specimens per group.** The null shuffles only the
  `n(n − 1)/2` pairwise divergences, so with few specimens the null is coarse. In the synthetic
  test, six specimens split cleanly into two groups of three gave Z near 3. The Monte Carlo P
  value cannot fall below `1/(1 + B)`, and the shuffled networks need not be equally likely
  under any model of the specimens.

## Differences from R SPACE

- **Monte Carlo P value added.** `p_value_monte_carlo` is added alongside SPACE's
  normal-approximation P value.
- **Degenerate nulls.** When every randomized census gives the same co-information, the
  status is `degenerate_null`. R reports Z = ±Inf and P = 0 when the observed value differs.
  Marklab keeps P = 0, so BH matches R, but reports no Z, so results stay finite.
- **Too few discs is an error.** With too few discs for two bins per variable, R silently uses
  one bin and reports MI = 0. Marklab stops and states the needed count (`10·2^depth − 1`).
- **No `B/(1.5n)` term.** R adds `B/(1.5n)` to every entropy. It is the same constant for the
  observed and every randomized census and cancels exactly in CisMI, Z and P, so Marklab
  omits it. `observed_mi` is therefore the plain plug-in value.
- **Empty discs in multi-channel nulls.** With several object channels, each randomized
  channel keeps the original disc's count of classified cells. R moves discs that are empty in
  one channel to the end of that channel's randomized table.
- **transMI defects fixed.** R's `measure_transMI` takes each specimen's grid maximum from the
  wrong census (`sub_max[[j]]`, indexed by ensemble size, instead of `sub_max[[k]]`). It also
  collapses the drop list to a single value when `all` and `alo` fill the depth. Marklab uses
  each specimen's own range, and drops variables as cisMI does. The comparison with R uses a
  copy of `measure_transMI` with both defects fixed.
- **transMI compositions.** R removes cells where variables whose names contain `O1` (or `O2`,
  …) sum above 100%. That pattern also matches linked scalars and, for example, `O10` under
  `O1`. Marklab sums only object percentages of the same channel.
- **transMI `max_bins`.** R ignores `max_bins` in transMI; Marklab applies it, as in cisMI.
- **transMI `ss`.** R takes the smallest disc count over every radius in the censuses; Marklab
  uses the analysed radii, as cisMI does. With all radii analysed they agree.
- **Identical distributions.** When every pairwise divergence is zero, R divides zero by zero
  and reports `NaN`. Marklab treats every pair as fully similar, giving `trans_mi = 0` and a
  degenerate null.
- **transMI null draws.** R draws a fresh shuffle for each factor; Marklab scores every factor
  on the same shuffles of an ensemble's divergences. Each factor's null distribution is
  unchanged.
- **Plot data, not plots.** `dist` and `rank` return the data of `plot_dist`, `plot_MI_rank`
  and `plot_MI_radius`. Colours, point sizes and the zoom option are not ported. R replaces a P
  value of 0 in a ranking by a quadratic extrapolation from Z for display; Marklab reports the P
  value as computed.
- **Distribution axes.** When a variable has no more distinct values than bins, R lays an
  equally spaced grid between its minimum and maximum and matches values to it exactly, so
  values off that grid drop out. Marklab uses the distinct values themselves. R also mislabels
  the two axes when the variables are given in the opposite order to its census columns;
  Marklab keeps the order given.
- **Unbounded Z in rankings.** Where a cisMI null was degenerate and the observed value differs,
  R's Z is infinite: the ensemble ranks first and every variable in it gets an infinite score.
  Marklab ranks those ensembles first too, reports no Z, and gives each variable the count of
  such ensembles (`unbounded_ensembles`) beside its finite score. Variables with any such
  ensemble rank first.
- **Pattern padding from one disc.** R pads the ends of the path with `sample()`, which draws
  from `1:x` instead of repeating `x` when the first or last node holds a single disc; Marklab
  repeats the disc's value.
- **Pattern normalization by zero.** When the largest upper band edge of a normalized kind is
  0 or below, R divides by it; Marklab leaves those curves unscaled and reports no scale.
- **Enrichment off SPACE's scale.** The enrichment weights assume every variable lies between 0
  and 100. When a disc lies farther than that from its node, R's weights turn negative and the
  log ratio can be undefined; Marklab reports an error.
- **Maps for cell tables.** R maps patterns onto image pixels; Marklab maps them onto cells,
  each disc voting for its members. When every disc lies on the curves (`max d` = 0), R divides
  zero by zero; every disc then votes with weight 0.9999.
- **Variables come from the specimen.** Variables are the declared classes present in the
  specimen. R's columns are the classes present in the sampled discs, and a class present in
  one radius's discs but absent from another's becomes `NA` there. Unknown names in `all`,
  `alo` and `not` are errors; R ignores them.

## Validation

- **Unit tests:**
  - binning against R's formula, including floating-point cube roots;
  - nearest-point rounding against `FNN::knnx.index`. The tie emulation matched FNN on 285,694
    generated cases, including every grid midpoint;
  - lexicographic ensembles, and entropy and co-information by hand: a copy gives 1 bit, XOR
    gives −1 bit of three-way co-information;
  - the map's training against kohonen's nodes for fixed presentation orders on a chain and a
    ring (1e-12), kohonen's default radii, and R's path orientation;
  - rolling means and quantiles against `zoo::rollapply`, bit for bit where zoo's running sum
    rounds differently from per-window means;
  - the partition of the path into regions and the catch-all.
- **Integration tests (`tests/space.rs`):**
  - hand-computed census values, including the inclusive radius and unclassified cells;
  - a striped pattern detected against random labels;
  - R's `all`/`alo`/`not` rules and the explicit errors;
  - the CellViT CSV source giving the same census as the same cells declared inline, and
    several tables sharing one class list;
  - the repository's synthetic multiplex recipe loading every slide as a specimen with its
    group, and refusing a scalar with missing values;
  - the scalar magnitude bound and the census retained-memory limit;
  - transMI separating three striped from three well-mixed specimens but not an unrelated
    batch factor, and its input errors;
  - Chao–Jost smoothing, compositional removal and constant variables computed by hand from
    R's `smooth_dist`, and modularity against an `igraph::modularity` value;
  - identical cisMI and transMI results at one and four threads;
  - distributions: disc values equal to the census, probabilities summing to 1 with impossible
    compositions removed, distinct-value axes, reproducible randomized censuses, and errors;
  - rankings: R's filters, |Z| order and variable scores recomputed from a cisMI result read
    back from JSON, the across-radii view, and a transMI result;
  - patterns: a path from pure-B to pure-A discs across stripes with the band containing the
    mean, normalization, enrichment of the pure ends against a randomized census, group designs
    with the inferred factor, and errors, including variables off SPACE's scale;
  - maps: stripe interiors assigned to the expected regions, the catch-all region, and region
    errors;
  - identical patterns and maps at one and four threads;
  - CLI outputs that refuse to overwrite.
- **R oracle:** `tests/fixtures/space/compare_space.R` compares the census and cisMI with R
  SPACE, sourced from the pinned commit. The data is SPACE's own tutorial cell table
  (`HuLN_Object1_ObjTable_Types.csv`: 9,757 cells, 20 classes). The design used radii 10 and
  20 µm, 1,000 discs at R's sampled centres, depth 3 and 1,000 bootstraps; R 4.5.2 package
  versions are pinned. On 2026-10-28:
  - **Census:** every value equals R `census_table` (maximum error 0).
  - **Observed co-information:** for all 1,350 ensembles at each radius, it equals R's
    `round_column`, `entropy` and inclusion–exclusion on Marklab's reported grids within
    1.4e-15.
  - **P values:** the BH adjustment matches `p.adjust` within 2.3e-16, and the normal P matches
    `2 * pnorm(-|Z|)` within 1.3e-13.
  - **Randomized censuses at 10 µm:** the Spearman correlation of Z is 0.9954 between Marklab
    and R, against 0.9956 between two R runs. No ensemble differs by more than 4 Monte Carlo
    standard errors.
  - **Randomized censuses at 20 µm:** results depend on the grid regime of rare classes. Two R
    runs differed beyond 4 standard errors on 12.6% of ensembles. Every CisMI from both R runs
    lies within Marklab's range over 20 null seeds, widened by 4 standard errors.
  - **Revised criteria:** the comparison criteria were revised twice after failures that traced
    to this grid effect (objects 1 and 20). The script header records the original criteria,
    the failures and the evidence.
  - **Speed:** Marklab's cisMI took 0.77 s on 12 cores; R `measure_cisMI` took 138 s and 170 s
    with 11 workers.

- **R oracle for transMI:** `tests/fixtures/space/compare_space_trans.R` splits the same
  tutorial table into a 3 × 2 grid of tiles. That gives six specimens of 1,098 to 2,047 cells,
  with factors `column` (3 levels) and `row` (2 levels). The design used radius 10 µm, 1,000
  discs per tile, depth 3 (4 bins, 1,350 ensembles) and 1,000 bootstraps. R ran a copy of
  `measure_transMI` with its two defects fixed. All criteria were fixed before the comparison
  and passed on the first run, 2026-10-28:
  - **transMI:** for every ensemble and factor, it equals the modularity from R's
    `round_column`, `build_dist`, `smooth_dist`, KL and `igraph::modularity`, and the fixed
    `measure_transMI`, within 6.0e-16.
  - **P values:** the BH adjustment matches `p.adjust` within 2.2e-16, and the normal P matches
    `pnorm(Z, lower.tail = FALSE)` within 2.0e-15.
  - **Shuffled null:** the Spearman correlation of Z is 0.9980 between Marklab and R, against
    0.9979 between two R runs. The mean absolute Z difference is 0.046 for both pairs.
  - **Speed:** Marklab's transMI took 0.06 s; R `measure_transMI` took 634 s and 580 s with 11
    workers.

- **R oracle for the plot data:** `tests/fixtures/space/compare_space_views.R` runs R's
  `plot_dist`, `plot_MI_rank` and `plot_MI_radius` on the tutorial census at radii 10 and 20 µm
  (1,000 discs at R's centres). R's ranking functions receive Marklab's cisMI (depth 2, 200
  bootstraps) and transMI results (3 × 2 tiles, 300 discs each, depth 2, 100 bootstraps) as
  their input tables. All criteria were fixed before the comparison. The first run stopped
  inside R's plotting code (ggplot2 4.0.3), before any ranking was compared, so the script now
  skips drawing. It passed on 2026-10-28:
  - **Distributions:** at 4, 10 and 25 bins, for one object and for the two most frequent
    objects, every value and probability equals R's (maximum error 0), with the same cells
    removed. Unrounded disc values equal R's census.
  - **Rankings:** with R's defaults, `depth = 2`, a `not` filter and an `all` filter, the
    kept cisMI ensembles (8, 6, 4 and 19), their order and the variable scores match R; so does
    the ensemble followed across both radii. For transMI, no ensemble passes R's defaults on
    either side, and with `p_thr = 1` the same 210 ensembles are kept in the same order.

- **R oracle for patterns:** `tests/fixtures/space/compare_space_pattern.R` runs R's
  `learn_pattern` with its defaults on the six tiles of the transMI oracle (radius 10 µm, 1,000
  discs per tile) for the three most frequent objects. It uses two designs: column "left"
  against column "right" (2,000 discs each), and all six tiles pooled (6,000 discs). All
  criteria were fixed before the comparison, and they passed on 2026-10-28:
  - **After the map:** with kohonen's call replaced by Marklab's trained nodes, R's curves,
    bands and enrichment equal Marklab's wherever R's padding draws do not enter. The first
    design matches bit for bit; the pooled design within 1.4e-13, the rounding R's different
    padding values leave in zoo's running sum. Marklab's starting radius equals kohonen's
    default (423).
  - **The map:** over five seeds each, the mean quantization error is 2.374 against kohonen's
    2.392, and the topographic error 0.45% against 0.86%.
  - **End to end:** Marklab's curves differ from R's by 3.2 percentage points on average,
    within the criterion of 1.5 times the 2.3 between consecutive R seeds, plus 0.5. Marklab's
    own seeds differ by 2.3 consecutively and 3.5 over all pairs.
  - **Speed:** the Marklab command, census included, took 1.3–1.6 s; R `learn_pattern` took
    3.0–3.9 s. The second run, under other load, took 2.7–3.8 s and 5.5–6.9 s.
  - The first run also passed, but matched only within 6.5e-13: Marklab averaged each window
    afresh, while zoo computes `rollapply(x, w, mean)` as a running sum. Marklab now uses the
    running sum, and the second run gave the figures above.

  `space map` has no R counterpart for cell tables, so it is checked by the integration tests
  only.

  The R and SPACE data stay outside the repository; each script's header gives the download
  and run commands.
