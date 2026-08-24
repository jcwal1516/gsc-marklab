# Marklab active roadmap

Last updated: 2026-08-24

Authority: this file is the bounded dependency-ordered execution view derived from `PROGRAM_TRACKER.md` and the immutable `MASTER_PLAN.md`. It does not replace the master plan. Non-goals apply only to this increment; every excluded future capability remains represented in `PROGRAM_TRACKER.md`.

The complete end-to-end classical spatial-pathology workflow reached its major checkpoint on 2026-08-24. Its preserved contract and evidence are in `task-contracts/WS-30-KL-01.md`, `ROADMAP_HISTORY.md`, and the implementation ledgers. The next promoted outcome closes the durable execution prerequisite required before Marklab admits external numerical backends. Work remains single-agent and behavior-first. Broad gates run only at the next major checkpoint.

## Outcome 1 — Durable replayable classical project execution

Tracker state: active.

Master-plan IDs advanced: FND-07, PLAT-01, WF-01, WS-11, and WS-12.

Concrete workflow delivered: a user runs the completed classical workflow inside a named local Marklab project, receives the same strict scientific result bundle, can terminate and rerun in a separate process, and observes a verified durable cache hit rather than recomputation. Marklab persists a human-readable project head, append-only execution ledger, and content-addressed result object; validates exact inputs and current implementation/config identity on every replay; and recovers or rejects incomplete/tampered state without inventing success.

Existing prerequisites:

- the completed `marklab classical` input, geometry, K/L, conditional-CSR, result, report, and failure-atomic output workflow;
- the existing strict artifact references/catalog, local capability-confined artifact store, SHA-256 identities, typed node/spec/cache key, local scheduler, and in-memory `MarklabProject` success semantics;
- existing project catalog canonical JSON and local-store publication/recovery rules;
- result format 0.3 remains unchanged and the classical result remains `marklab.classical_spatial` version one.

Exact accepted inputs:

- CLI: `marklab project classical` with required `--project`, `--cells`, `--mask`, `--out`, `--r-max-um`, `--r-steps`, `--simulations`, `--seed`, `--alpha`, `--memory-budget-mib`, `--max-pair-visits`, and `--max-csr-draws` arguments;
- project path: one local directory path whose root, manifest, ledger, staging paths, and object store are not symbolic links and remain confined beneath the declared project root;
- scientific inputs and limits: exactly the completed classical command contract, with source bytes and canonical window/config identities reverified before miss or hit;
- durable state: either no existing project, one canonical supported project-head version with an append-only canonical ledger, or recoverable recognized staging state; unknown versions, malformed records, digest drift, conflicting heads, path escape, and fabricated success are errors;
- resources: positive project-manifest, ledger-record/count, object-byte, scheduler-output, and scientific limits; no unbounded replay scan or object materialization.

Observable outputs:

- the unchanged three-file classical run bundle at `--out`, including cache status and identity that truthfully report a cross-process miss or hit;
- one canonical human-readable project head naming project ID, schema version, exact artifact references, current execution-ledger tail, and object-store policy without duplicating scientific source data;
- one append-only execution ledger whose successful record binds node/spec, implementation, complete input/config/resource identities, cache key, canonical output artifact, result schema, timestamps/order metadata, and terminal disposition;
- one content-addressed canonical private node-output object published through the existing local artifact-store durability/integrity boundary;
- deterministic recovery reporting for recognized incomplete staging records/objects, with no successful ledger entry before object durability and head advancement;
- typed first-run miss, separate-process verified hit, changed-input/config/resource miss, stale-implementation miss, tampered-object failure, malformed-ledger failure, and interrupted-run recovery outcomes.

Explicit non-goals for this increment only:

- no backend/plugin registry, Python/R/Stan/container execution, remote scheduler, server, collaboration, authentication, UI, notebook binding, cloud store, or deployment;
- no arbitrary untyped task runner, general multi-node execution, parallel DAG scheduler, distributed cache, workflow marketplace, or user code execution;
- no new scientific statistic, estimator, null, randomization unit, result-format 0.3 field, general result schema, or change to classical numerics/claims;
- no source-data copy into project metadata and no claim that a durable cache object is a scientific receipt or external producer authentication.

Focused acceptance tests:

1. A first process executes the hand-oracle classical fixture as a miss; a second process using the same project and exact inputs returns a verified hit with identical typed result and result bundle and no scientific execution.
2. Any cells, window, radius, null, seed, alpha, simulation, implementation, codec, feature/resource policy, or scheduler-output-limit change produces a distinct cache identity and a miss; row ordering allowed by the point-pattern contract preserves scientific results while source artifact identity remains exact.
3. Project head and ledger codecs are canonical fixed points, reject unknown fields/versions/noncanonical encodings, enforce record/count/byte limits, and never duplicate source matrices or geometry payloads.
4. Missing, truncated, appended, replaced, or digest-mismatched cache objects fail integrity before replay; symlink roots/intermediates/leaves and path escapes are rejected without following them.
5. Injected failures before object publication, after object publication, before ledger append, and before head advancement either leave the prior durable state intact or produce one deterministic recoverable staging report; no partial path is visible as success.
6. Existing in-memory project/workflow tests, `marklab classical`, legacy `marklab analyze`, result 0.3, catalog/store recovery, no-default, and feature boundaries remain unchanged.

Major-checkpoint exit condition: cross-process miss/hit and every failure/recovery point pass focused oracles; the final diff has one durable project-head owner, one append-only execution-ledger owner, and one existing content-addressed store owner; formatting, affected integration/packages, warning-denied Clippy, no-default, strict docs, relevant feature/CLI suites, and the full workspace suite run once and pass; ledgers and `PROGRAM_TRACKER.md` are updated; `BACK-01`/`WS-13` are re-evaluated and the next multi-backend outcome is promoted; a coherent local checkpoint commit is created without push or publication.

## Multi-backend continuation rule

After this durable prerequisite closes, the next backend outcome must follow the master plan’s integrated multi-backend policy. Marklab will own pinned environment/license/security manifests, typed input/output schemas, diagnostics normalization, deterministic controls, artifact identities, and workflow provenance. Established Python, R, Stan, GPU, or specialized backends may perform advanced numerical work. A native Rust port is not presumed and requires measured scientific, operational, portability, or performance evidence against the established backend.
