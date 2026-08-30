# Task contract — MAINT-AI-SLOP-01 structural consolidation

Status: active

Date: 2026-08-30

Parent requirements: repository maintenance only; no new master-plan coverage.

## User outcome

Remove confirmed duplicated mechanics and repair poor file boundaries without changing Marklab's
scientific behavior, public APIs, CLI surface, result-format 0.3, physical schemas, deterministic
seeds, finite-result policy, resource ceilings, transaction semantics, or durable cache correctness.

## Preserved concurrent surface

The pre-existing modified and untracked files present when this task began belong to the interrupted
implementation task. They are read-only for this task, including `Cargo.toml`, `README.md`, the
modified implementation ledgers, `src/bin/marklab/project.rs`, the durable GUDHI tests, the gastric
worker/test, `docs/assets/**`, and the untracked CRC outcome worker/tests. This task stages and commits
only its own allowlisted files.

## Wave A ownership

- Bayesian numerical consolidation: private `marklab-bayes` validation, dense-linear-algebra, and
  Matérn owners plus exact duplicate callers and focused tests.
- Root/cohort scientific consolidation: canonical zero and compensated summation, the shared
  scalar–embedding covariance kernel, two-group permutation validation, and focused behavior tests.
- Columnar physical consolidation: identical Arrow footer/write mechanics, Parquet preflight
  primitives, identical budget gates, and their duplicated columnar test/fuzz support.

Agents receive disjoint file allowlists. No agent edits shared ledgers, manifests, `Cargo.toml`,
public exports, result types, schemas, or the active implementation surface.

## Behavior-preservation protocol

- Establish focused green baselines before production edits. These are behavior-neutral refactors;
  any post-edit failure is a regression rather than an assertion to weaken.
- Add small reference or differential coverage before deleting duplicate numerical mechanics.
- Preserve serialized bytes, digest identities, error precedence, exact resource boundaries, test
  inventories, hostile-input cases, and independent scientific oracles.
- Keep domain-specific errors, schemas, and result wrappers at their current owners. Shared code is
  private and owns only byte-identical semantics already used by multiple immediate callers.
- At each cohesive milestone, run focused/affected tests, inspect the diff and status, and create a
  local commit containing only this task's files. Do not push or publish.

## Stabilization

After the three Wave A milestones, run the canonical major-checkpoint formatting, Clippy, all-feature
workspace tests, doctests, and no-default gates once. Run specialized fuzz evidence only because the
columnar parser boundary changes. Update status/ledgers only after production behavior is green and
only after the interrupted task's documentation ownership is reconciled.

## Non-goals

No dependency or lockfile change, public API, new result/schema version, backend, generalized plugin
framework, scientific formula change, benchmark claim, master-plan rewrite, remote action, push,
publication, deployment, or history rewrite.
