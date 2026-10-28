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

This release implements the census and **cisMI** (patterning within one specimen). transMI
(patterns that separate groups of specimens), pattern learning and mapping, diversity and
image inputs are planned.

```sh
marklab space describe
marklab space census --request request.json --out census.json --table census.csv
marklab space cis --request request.json --out cis.json --table cis.csv \
  --depth 3 --bootstraps 100
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
- **`cellvit_he_csv`**: `path` to a CellViT H&E classification table (`cell_id`,
  `x_centroid_um`, `y_centroid_um`, `predicted_class`, `class_probability`; the
  [multimodal](capabilities.md) reader), plus `window_path`, `specimen_id`,
  `coordinate_frame_id`, optional `groups` and `minimum_class_probability` (rows below it are
  rejected). The predicted class becomes the categorical channel `cellvit_class`, with
  normalized class names in sorted order. The probability becomes the continuous channel
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
error, never a zero.

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
- joint-entropy work (ensembles × (bootstraps + 1) × discs);
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
- Results describe one specimen. They are not patient-level inference.

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
    gives −1 bit of three-way co-information.
- **Integration tests (`tests/space.rs`):**
  - hand-computed census values, including the inclusive radius and unclassified cells;
  - a striped pattern detected against random labels;
  - R's `all`/`alo`/`not` rules and the explicit errors;
  - the CellViT CSV and multiplex-recipe sources giving the same census as inline input;
  - identical results at one and four threads;
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

  The R and SPACE data stay outside the repository; the script's header gives the download
  and run commands.
