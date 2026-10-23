# Cahn–Hilliard phase-field simulation

`marklab simulate phase-field` evolves a conserved scalar order parameter on a periodic
two-dimensional grid. It is a bounded research simulator for domain separation, coarsening and
interface relaxation under a declared free-energy model. It does not add cell growth, reactions,
advection or treatment effects.

The model is

```text
∂φ/∂t = M ∇²μ
μ = A(φ³ - φ) - κ∇²φ
F[φ] = ∫ [A(φ² - 1)²/4 + κ|∇φ|²/2] dx.
```

Here `φ` is the dimensionless phase field, `M` is `mobility_um2_per_time`, `A` is
`bulk_energy_scale`, and `κ` is `gradient_energy_coefficient_um2`. Positive `A` gives minima at
`φ = -1` and `φ = 1`; positive `κ` penalizes sharp interfaces. Parameters are hypotheses and are
not fitted by this command.

The solver uses the eigenvalues of the periodic five-point finite-difference Laplacian in a
stabilized semi-implicit discrete Fourier step. The zero Fourier mode is unchanged, so the
discrete spatial mean and mass are conserved. MarkLab evaluates the matching forward-difference
discrete free energy after every step and stops if it increases beyond floating-point tolerance.
It also rejects non-finite fields, initial `|φ| > 2`, evolved `|φ| > 4`, mean drift, an excessive
stabilized update number, work above `--maximum-cell-steps`, declared work above 50 million cell
steps, grids above 65,536 cells, and requests retaining more than 10,001 summaries. `--time-step`
must not exceed `--final-time`; reducing it is the remedy for an energy-increase rejection.
Derived cell area, domain extents, inverse-square Laplacian scales and worst-case stabilized
update terms must also remain finite and nondegenerate. Every initial, evolved and final snapshot
is checked field by field before it can affect an energy maximum or be published. The output path
is created exclusively; an existing file is rejected and preserved.

The [example input](../examples/phase-field.json) is strict JSON:

```json
{
  "grid_x": 4,
  "grid_y": 4,
  "spacing_x_um": 1.0,
  "spacing_y_um": 1.0,
  "initial_phase_row_major": [
    0.2, -0.2, 0.2, -0.2,
    0.2, -0.2, 0.2, -0.2,
    0.2, -0.2, 0.2, -0.2,
    0.2, -0.2, 0.2, -0.2
  ],
  "mobility_um2_per_time": 1.0,
  "bulk_energy_scale": 1.0,
  "gradient_energy_coefficient_um2": 0.5
}
```

Run it with:

```sh
marklab simulate phase-field \
  --input examples/phase-field.json \
  --final-time 0.1 \
  --time-step 0.01 \
  --record-every-steps 1 \
  --maximum-cell-steps 1000 \
  --out phase-field-result.json
```

The result contains the initial and final fields, trajectory summaries, mean/mass conservation,
bulk and gradient energy, and solver diagnostics. The trajectory records scalar summaries at the
requested interval rather than a full field at every step, keeping output bounded.

This workflow can explore idealized phase separation or generate synthetic fields for downstream
method tests. Energy decay and mass conservation validate the declared numerical model; they do
not show that tissue follows Cahn–Hilliard dynamics, identify biological phases or forecast tumor
evolution.
