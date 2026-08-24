# Marklab Frontier Algorithm Pseudocode Index

The canonical full document is `PSEUDOCODE_FULL.md`. The files under `pseudocode/` are exact contiguous byte slices of that document, provided so Codex and human reviewers can load the specification incrementally rather than pasting a 226 KB document into one prompt.

## Required reading order

1. `MASTER_PLAN.md`
2. `PSEUDOCODE_AUTHORITY_AND_ERRATA.md`
3. `PSEUDOCODE_GAP_REGISTER.md`
4. this index
5. the split files below in numeric order
6. `PSEUDOCODE_CROSSWALK_REVIEW.md` when reconciling scope or authority

## Integrity

Full-source SHA-256:

```text
c0508109be2a954502bb1b1989ee98936f1c1838bd25fd92fcb33f09054fda51
```

Total source lines: 6000  
Split file count: 18

| Order | File | Contents | Original lines | Lines | Bytes | SHA-256 |
|---:|---|---|---:|---:|---:|---|
| 0 | [`00_preamble.md`](pseudocode/00_preamble.md) | Preamble and scope | 1–45 | 45 | 2066 | `e157cd7e4a36509d65799c4db9a3698672aa544836508b4235417bd79deef165` |
| 1 | [`01_part_i_shared_algorithmic_substrate.md`](pseudocode/01_part_i_shared_algorithmic_substrate.md) | Shared algorithmic substrate | 46–512 | 467 | 14048 | `d392fdbb1f97e410e46954bbb8babbed5d2944bdc1a581813724813b005e0b60` |
| 2 | [`02_part_ii_cohort_valid_spatial_inference.md`](pseudocode/02_part_ii_cohort_valid_spatial_inference.md) | Cohort-valid spatial inference | 513–886 | 374 | 14324 | `8754c7a53b0ed355ffb1fc5cf0038508c1f000a51290ccee498e38eaa4e2db20` |
| 3 | [`03_part_iii_bayesian_spatial_modeling.md`](pseudocode/03_part_iii_bayesian_spatial_modeling.md) | Bayesian spatial modeling | 887–1399 | 513 | 19408 | `cec9128251ca6a668e41c64e44db040febd51ffee871db6d3d92d721b548c5e2` |
| 4 | [`04_part_iv_bayesian_point_processes.md`](pseudocode/04_part_iv_bayesian_point_processes.md) | Bayesian point processes | 1400–1770 | 371 | 14462 | `1e1383e130453c37cc422937541a0ef9d93e071a82edb535f71a7784ee8ca6a2` |
| 5 | [`05_part_v_high_dimensional_embedding_science.md`](pseudocode/05_part_v_high_dimensional_embedding_science.md) | High-dimensional embedding science | 1771–2196 | 426 | 16002 | `33574014146bcfc9d4afec224fa0e9882f9b7cc7aa6cc9e6bc605cd9bb9cffac` |
| 6 | [`06_part_vi_registration_atlas_mapping_and_transport.md`](pseudocode/06_part_vi_registration_atlas_mapping_and_transport.md) | Registration, atlas mapping, and transport | 2197–2515 | 319 | 12522 | `773421df67822615b1f96aec0f929dfda96c66356f19ba743c439d0eccf446ee` |
| 7 | [`07_part_vii_graph_and_higher_order_tissue_mathematics.md`](pseudocode/07_part_vii_graph_and_higher_order_tissue_mathematics.md) | Graph and Higher-Order Tissue Mathematics | 2516–3105 | 590 | 20625 | `4c973e32fd764d7a1968c74d187d4da0204461b97cc2aa2c00d7774b95a47470` |
| 8 | [`08_part_viii_topology_and_mathematical_morphology.md`](pseudocode/08_part_viii_topology_and_mathematical_morphology.md) | Topology and Mathematical Morphology | 3106–3388 | 283 | 10972 | `3cb7db55f642e2a360d04d57ab4b537659f532eeea4fafb48a4780531dde8666` |
| 9 | [`09_part_ix_multimodal_bayesian_models.md`](pseudocode/09_part_ix_multimodal_bayesian_models.md) | Multimodal Bayesian Models | 3389–3804 | 416 | 15052 | `a27d4084c300ccff8f7f1af6eedb64fbe16d5a45854e4720b1bed53e022e0391` |
| 10 | [`10_part_x_generative_tissue_modeling_and_simulation_based_inference.md`](pseudocode/10_part_x_generative_tissue_modeling_and_simulation_based_inference.md) | Generative Tissue Modeling and Simulation-Based Inference | 3805–4464 | 660 | 22899 | `9409aba11d1405d5245481a8c45fd8277c27971314bfc77d45829de7f5c7f1e2` |
| 11 | [`11_part_xi_3_d_longitudinal_and_evolutionary_models.md`](pseudocode/11_part_xi_3_d_longitudinal_and_evolutionary_models.md) | 3-D, Longitudinal, and Evolutionary Models | 4465–4829 | 365 | 13189 | `88187b9abe398b0fcb80a7f60f2a6b685e0f5e530315a4712bf9201a44f7dcea` |
| 12 | [`12_part_xii_causal_interference_perturbational_and_active_design_research.md`](pseudocode/12_part_xii_causal_interference_perturbational_and_active_design_research.md) | Causal, Interference, Perturbational, and Active-Design Research | 4830–5310 | 481 | 18517 | `d744a0293331c1d0d66e7484045ceeae3f662ee0ec4166f60c6d93a75d0f7bac` |
| 13 | [`13_part_xiii_unified_execution_validation_and_release_contracts.md`](pseudocode/13_part_xiii_unified_execution_validation_and_release_contracts.md) | Unified Execution, Validation, and Release Contracts | 5311–5754 | 444 | 14884 | `28a0a28683e8d4badf01abde325b53ae2b8db40abecea2ecd739af1a0aeb1fc0` |
| 14 | [`14_part_xiv_requested_function_coverage_index.md`](pseudocode/14_part_xiv_requested_function_coverage_index.md) | Requested-Function Coverage Index | 5755–5831 | 77 | 3666 | `2dbb24d416b429cb53be158dc7eb449da735844c33f47fba1173e6342f48fb08` |
| 15 | [`15_part_xv_complexity_and_scaling_summary.md`](pseudocode/15_part_xv_complexity_and_scaling_summary.md) | Complexity and Scaling Summary | 5832–5856 | 25 | 2027 | `04205e8d25f535ce7b6f6075e467706f0089416cd5f575a10b2eee5ef03d6cbc` |
| 16 | [`16_part_xvi_primary_research_basis_and_reference_implementations.md`](pseudocode/16_part_xvi_primary_research_basis_and_reference_implementations.md) | Primary Research Basis and Reference Implementations | 5857–5978 | 122 | 15435 | `4224c0cc4d0a0afbb2d7b7e9cae9ebb02c8bb5d4eb9a512661845a176d7e3c52` |
| 17 | [`17_part_xvii_implementation_reading_order.md`](pseudocode/17_part_xvii_implementation_reading_order.md) | Implementation Reading Order | 5979–6000 | 22 | 1063 | `f1fedd618024caca798d74eefcbd4542c1e2fabd6d22dde6ccc80232ebcd0c97` |

## Verification

```bash
python3 docs/implementation/verify_pseudocode_pack.py
```

The verifier checks every split-file digest and confirms that ordered concatenation is byte-identical to `PSEUDOCODE_FULL.md`.

## Codex ingestion rule

Do not paste the full pseudocode into a Codex prompt. Put these files in the repository. Tell Codex to verify the manifest and read the relevant split files from disk. For a bounded task, Codex should read only:

- the master-plan sections governing the task;
- the authority/errata file;
- the relevant pseudocode part;
- shared substrate parts on which it depends;
- the current status, canonical-symbol registry, validation ledger, and active handoff.
