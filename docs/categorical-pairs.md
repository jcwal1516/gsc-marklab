# Directed categorical pairs

`marklab project categorical-pair` measures the spatial relationship from one declared
histologic-compartment level to another on a single section. The previous version already
reported shell-wise mark-connection and cumulative directed cross-K. Result version 2 adds
the corresponding cross-L values, theoretical L curve, and transformed L envelopes.
Reversing `--source-level` and `--target-level` is a distinct analysis because the border
correction applies to source centers.

## Command

```bash
marklab project categorical-pair \
  --project out/categorical-pair-project \
  --cells cells.csv \
  --mask window.geojson \
  --out out/neoplastic-to-inflammatory.json \
  --source-level Neoplastic \
  --target-level Inflammatory \
  --radii-um 10,20,30,40 \
  --permutations 999 \
  --seed 123456789 \
  --alpha 0.05 \
  --memory-budget-mib 512 \
  --max-pair-visits 100000000 \
  --max-null-pair-evaluations 99900000000
```

The source and target labels must be different levels in the
`histologic_compartment` column. Radii are positive, strictly increasing micrometre
values. All arguments are required. The project directory stores durable inputs and
cached workflow results; `--out` must name a new JSON file.

## Estimators

For observation-window area `A`, radius `r`, `n_j` target cells, `m_i(r)` source cells
at least `r` from every boundary, and `N_ij(r)` ordered source-to-target pairs no farther
than `r`, the cumulative estimator is

```text
K_ij(r) = A N_ij(r) / (m_i(r) n_j)
L_ij(r) = sqrt(K_ij(r) / pi)
```

This is the standard reduced-sample border estimator. Under homogeneous independent
labeling, the reference curves are `K_ij(r) = pi r²` and `L_ij(r) = r`. Cross-K/L is
undefined at a radius with no boundary-eligible source center; the JSON uses `null`
rather than a numerical placeholder.

The mark-connection estimator keeps its shell-wise definition. For the shell ending at
`r`, `connection_probability` is the number of boundary-eligible ordered source-to-target
pairs divided by all boundary-eligible ordered pairs in that shell. A shell with no pairs
has status `no_pairs_in_shell` and a null connection value. Cross-K/L remains cumulative
even when the corresponding connection shell is empty.

## Random-labeling inference

The null model permutes complete categorical rows over the fixed cell locations while
preserving the observed level counts. Marklab evaluates separate two-sided extreme-rank-
length (ERL) families for the mark-connection curve and the cross-K curve. `alpha` is the
family-wise level within each named family. Cross-L is a monotone transform of cross-K,
so its lower and upper envelopes are transformed from the cross-K envelope and it does
not receive a second p-value.

A radius enters a family only when the observed curve and every permutation have a value
there. The result records eligibility per radius and `eligible_radius_count` per family.
The seed, requested permutation count, and exact pair-evaluation count are also recorded.

## Result contract

The output is a `marklab.categorical-pair/2` document. Version 2 preserves the version 1
shell counts, connection fields, cross-K values, theoretical cross-K curve, cross-K
eligibility and envelopes, and `inference.cross_k`. It adds these curve fields:

- `cross_l` and `theoretical_cross_l`;
- `lower_cross_l` and `upper_cross_l`.

The top-level `inference.cross_k` object continues to hold the cross-K ERL global p-value,
observed and critical depths, and eligible-radius count. The mark-connection result remains
in `inference.connection`. This versioned document is separate from Marklab result format
0.4.

## Reference comparison

The reference runner in
`tests/fixtures/inhomogeneous_spatial/compare_spatstat.R` compares both directions on the
published `spatstat.data` amacrine and betacells patterns with `spatstat` `Kcross` using
border correction. The [`Kcross` documentation](https://rdrr.io/cran/spatstat.explore/man/Kcross.html)
accepts a supplied radius vector, while the lower-level
[`compileK` contract](https://rdrr.io/cran/spatstat.explore/man/compileK.html) calls for a
finely and equally spaced sequence. Supplying
only the 17 output positions (zero plus 16 requested radii) produced 2.90%--6.15% scaled
cross-K errors and 0.661--2.247 micrometre cross-L errors because pair counts and border
risk sets were assigned on intervals one requested radius wide.

Before the successful rerun, the reference policy was fixed at 1,024 equal subdivisions
per requested-radius interval: 16,385 points, with spacing 0.008081 micrometres for
amacrine and 0.009155 micrometres for betacells. The runner independently recomputes the
integer source eligibility and directed pair counts from `bdist.points` and `crossdist`,
then requires the fine-grid `Kcross` numerator and denominator to recover those counts
within `1e-9`. It also writes the untouched 513-point `Kcross` default-grid curves for
each direction to `spatstat-default-grid.csv`; those default spacings are 0.323242 and
0.366211 micrometres. This establishes radius/bin alignment without changing the
scientific agreement tolerances.

The prespecified tolerances are `1e-10` scaled error for cross-K and `1e-10` micrometres
absolute error for cross-L. In the successful run,
the four directed cross-K errors are at most `4.21e-16` and the cross-L errors at most
`4.27e-14` micrometres. Translation and isotropic estimates are retained as descriptive
differences because they use different edge corrections.

The combined public point-pattern runner reproduced these results alongside homogeneous
and inhomogeneous K/L checks.

This checks numerical agreement for homogeneous cross-K/L on two published animal
retinal point patterns. It does not validate an inhomogeneous cross-K/L estimator,
patient-level inference, clinical interpretation, or fitness for diagnosis.
