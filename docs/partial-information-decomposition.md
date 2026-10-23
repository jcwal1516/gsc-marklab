# Discrete partial information decomposition

`marklab numerics partial-information-decomposition` separates empirical information about
one categorical target into redundancy, two unique contributions, and synergy from exactly
two categorical sources. The implemented redundancy definition is **Williams–Beer I_min**;
other PID definitions can give different answers.

```sh
marklab numerics partial-information-decomposition \
  --input examples/partial-information-decomposition.json \
  --out out/partial-information-decomposition.json
```

Supply a strict version-1 `marklab.partial_information_decomposition_request` with
`estimator: "williams_beer_i_min"`, the target and two source declarations (`name`, ordered
`levels`, and `definition`), and a joint count table. Each count row names the zero-based
`target`, `source_a`, and `source_b` level indices and a positive integer `count`. Duplicate
joint cells are summed exactly; missing cells have zero count. There are no pseudocounts.
The example is the XOR control: neither source alone informs the target, while their joint
state supplies one bit, reported as synergy.

For target state `t`, the source-specific information is

```text
I_spec(t; A) = KL[p(A | t) || p(A)]
R = sum_t p(t) min(I_spec(t; A), I_spec(t; B))
U_A = I(T; A) - R
U_B = I(T; B) - R
S = I(T; A,B) - I(T; A) - I(T; B) + R
```

All values use base-2 logarithms and are in bits. The result includes target entropy,
marginal and joint mutual information, the four PID terms and target-specific information.
The terms sum to joint information within floating-point tolerance; only negative roundoff
within `1e-12` bits is clipped to zero. Constant targets legitimately yield zero information.

Use `statistical_unit: "patient"` for patient counts, or
`"within_specimen_field_diagnostic"` for a descriptive within-specimen table. These
declarations do not verify independence or turn cells into patient replicates. For continuous
embeddings, prespecify a low-dimensional discretization or learn it on training patients and
record it in each variable's `definition`. This command does not bin continuous inputs.

I_min compares amounts of specific information, not whether sources convey the same content.
For two independent bits copied into a joint two-bit target, it assigns one bit to redundancy
and one to synergy. This known limitation makes a positive redundancy term insufficient
evidence of duplicated biological information. Sparse tables, binning choices and small
cohorts can dominate the estimates. The command supplies no bias correction, uncertainty,
significance test, causal interpretation or high-dimensional continuous PID estimator.

Inputs are limited to 16 MiB, at most 256 levels per variable, one million declared joint
states or count entries, and one trillion observations. The caller must additionally declare
`maximum_entries`, `maximum_states`, and `maximum_observations` in `limits`. Output publication
requires a new path and preserves existing files.
