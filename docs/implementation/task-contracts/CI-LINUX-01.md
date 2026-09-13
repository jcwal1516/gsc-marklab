# CI-LINUX-01 — Restore Linux CI

The Gaussian crossed/nested hierarchy's NUTS tree-depth budget increases from 11
to 12. The existing real-backend regression repeatedly reaches depth 11 despite
finite posterior draws, zero divergences, and adequate effective sample sizes.
The additional level permits longer trajectories within the existing iteration,
observation, output-size, and worker-timeout bounds. Diagnostic admission remains
unchanged, including zero permitted tree-depth hits and the existing R-hat limit.
The request already records this budget in its diagnostic policy, so durable
execution identity continues to distinguish different sampler policies.

The CI repair also restores readable directory handles for Linux durability
barriers, gates the CSV-dependent test on CSV, places the CLI smoke test in the
integration suite, selects the native GNU target for fuzz checks, and permits
600 seconds for the existing hierarchy regression. Backend CI collects all test
failures in one run. No dependency, scientific model, schema, or assertion changes
are required.
