# Task contract — MAINT-AI-SLOP-01 structural consolidation

Status: completed

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

## Wave B structural ownership

Wave B repairs confirmed responsibility boundaries after Wave A's shared primitives are stable:

- split the graph crate root, Bayesian embedding-spatial estimands, multimodal backend adapters, and
  the CellViT artifact-graph integration suite by existing scientific workflow ownership;
- split cohort input parsing and output shaping from command dispatch, while keeping every existing
  error string and validation order stable;
- consolidate byte-identical family input readers and exclusive JSON publication mechanics;
- remove redundant standalone CLI parser shells once the canonical command enums can own dispatch;
- split durable project orchestration only after the concurrent project implementation releases
  those files, and preserve each workflow's typed node, codec, cache identity, and replay behavior.

Line count is evidence for review, not the reason for a split. A new module must own a named
workflow, physical format, transaction, or validation responsibility. This task does not introduce a
generic backend registry, universal runner, macro-generated node framework, or catch-all helper
module merely to make the source shorter.

## Confirmed deletion targets

- `src/bin/marklab/multimodal_model.rs`: eleven independent adapters share one dispatch file and
  repeat exact worker-asset loading; split by adapter and retain one narrow asset owner.
- `src/bin/marklab/bayes.rs` and `src/bin/marklab.rs`: standalone parser shells mirror canonical
  command variants; remove the duplicate shells after dispatch characterization.
- `src/bin/marklab/{bayes,causal,longitudinal,numerics,policy,spatial3d}.rs` and cohort publication:
  repeated exclusive pretty-JSON transactions should use one binary-private owner.
- `src/bin/marklab/topology.rs`: witness lifecycle, worker-process control, direct topology commands,
  and publication are separate responsibilities and should not remain in one file.
- `src/bin/marklab/project.rs`: durable runner families and typed nodes need cohesive modules, but
  this file remains read-only until the concurrent implementation task has committed its work.
- `tests/cellvit_embedding_artifact_graph.rs`: shared fixtures, physical hostility, domain workflows,
  and graph-binding cases need separate test modules without deleting any case.

Ponytail review estimate before Wave B implementation: `net: -900 to -1,500 lines possible`, mostly
from exact CLI mechanics and parser-shell duplication. Boundary-only moves are accepted only where
they materially improve ownership and test isolation.

## Second-pass clone audit

A normalized cross-file scan over tracked production Rust found the remaining long exact clones and
separated real duplication from deliberate schema and re-export repetition:

- twelve workflow nodes carried two byte-identical observation-window artifact encoders (eight
  frame-bound references and four complete geometry descriptors); these have one immediate shared
  owner and are in scope for consolidation;
- Arrow and Parquet multiscale preflight families repeat footer admission, row-group traversal,
  page-declaration checks, and schema mechanics in blocks of roughly 35–83 normalized lines; these
  require format-specific shared owners without weakening their distinct domain checks;
- Bayesian command adapters repeat large argument schemas, and durable project commands mirror some
  of them; consolidation is deferred while `src/bin/marklab/project.rs` remains under active
  concurrent implementation ownership;
- crate-root re-export lists, typed result codecs with different post-decode invariants, and similar
  scientific validators are not deduplicated merely because their syntax is close.

The scan is advisory rather than a deletion quota. Shared code is introduced only where serialized
bytes, error behavior, and invariants are already identical and current production callers exercise
the exact same operation.

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

## Completed consolidation

The audit landed as 70 audit-owned local commits, interleaved with the active implementation task.
It consolidated exact numerical, validation, publication, reader, physical-format, allocation,
availability, and observation-window mechanics, and split oversized owners along existing Bayes,
CLI, embeddings, project, workflow, graph, data, causal, geometry, spatial, registration, topology,
multimodal, cohort, scalar-mark, and integration-test responsibilities. No public API, schema,
result-format, dependency, seed, resource policy, or scientific formula was intentionally changed.

The final integration repair found two maintenance defects rather than scientific regressions:

- seven durable project CLI tests hard-coded the default four-feature provenance vector and failed
  under `--all-features`; one shared test owner now derives the exact enabled feature set;
- scientific modules CLI-gated internal encoder re-exports, violating the workspace dependency
  contract; CLI adapters now import crate-private workflow encoders directly, and the CLI-only
  binary records its required `cli` feature at the binary boundary.

The remaining long exact clones are deliberate or currently owned elsewhere: independent numerical
oracles, domain-specific typed validators, workflow constructors whose error ordering is part of
their contract, isolated hostile-format fixtures, and the active durable project command surface.
Large remaining files were not split solely for line count.

## Executed evidence

- Focused affected suites passed for data hierarchy, project storage/catalogs, workflow, graph,
  causal, Bayes, embeddings CSV/Parquet, global Moran, and classical spatial behavior.
- The final repaired integration set passed 9/9 tests, including the workspace contract and all
  seven all-feature provenance cases.
- `cargo +1.96.0 fmt --all --check` passed.
- `cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings` passed.
- `cargo +1.96.0 nextest run --locked --workspace --all-features` passed 1,598/1,598 tests with
  26 skipped.
- `cargo +1.96.0 test --locked --workspace --doc --all-features` passed; the workspace currently
  contains no doctests.
- `cargo +1.96.0 check --locked --workspace --no-default-features` passed without warnings.
- `cargo +nightly fuzz check` passed for the specialized parser-boundary compile evidence; its
  generated lockfile refresh was discarded because dependency and lockfile changes are out of scope.

The canonical formatting gate passed before the concurrent task added
`tests/top1_pair_mixing_bounds_workflow.rs`. A final rerun then reported formatting differences only
in that untracked, concurrently owned test. An explicit Rustfmt check over every audit-owned changed
Rust file and `git diff --check` both passed; this task did not rewrite the other task's file.

Before the repair, one canonical run stopped on a SAR diagnostic assertion that passed immediately
in isolation and in both later parallel runs. A subsequent diagnostic no-fail-fast run executed all
1,596 then-current tests: 1,588 passed and eight stale integration assertions failed, directly
driving the repair above. The final exact canonical run was fully green and emitted no process-leak
classification.

Concurrent staging caused two known mixed-commit captures without data loss: `ab065d1` contains the
Parquet reader split plus already-staged implementation files, and `e0db144` captured the one-line
CLI-required feature provenance repair in `src/bin/marklab/project.rs`. History was not rewritten.
The concurrently owned `STATUS.md`, roadmap, decisions, canonical-symbol ledger, manifest, README,
workers, and unrelated tests were left for their owning task.

## Non-goals

No dependency or lockfile change, public API, new result/schema version, backend, generalized plugin
framework, scientific formula change, benchmark claim, master-plan rewrite, remote action, push,
publication, deployment, or history rewrite.
