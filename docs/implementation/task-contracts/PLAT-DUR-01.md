# Task contract — PLAT-DUR-01 durable classical project replay

Status: complete on 2026-08-26 after explicit user unpause; checkpoint commit intentionally omitted

Date: 2026-08-24

Parent requirements: FND-07, PLAT-01, WF-01, WS-11, WS-12.

## User outcome

`marklab project classical` runs the completed classical spatial-pathology workflow in one named
local project. A first process executes and durably records a miss. A later process with exact
inputs, configuration, resource policy, implementation, and codec identity verifies the recorded
object and replays it through the existing scheduler as a hit without executing the statistic.
The ordinary `marklab classical` command and result-format 0.3 remain unchanged.

## Accepted input

- One local `--project` directory plus exactly the completed classical CLI arguments.
- A missing project directory, or an existing supported canonical project head, canonical
  append-only ledger, project lock, staging intent, and existing local object-store layout.
- Positive fixed metadata/record/count/ledger limits and the caller's positive scheduler/object
  byte limit.
- A native runtime identity containing crate version, build Git availability/SHA and dirty state,
  rustc version, exact compiled feature set, and a streamed executable content reference.

Project roots and every owned control path are capability-relative and must be directories or
regular files as declared, never symlinks. Unknown versions, unknown JSON fields, noncanonical
bytes, invalid chains, head/ledger drift, conflicting cache identities, malformed timestamps,
oversized state, and unverifiable objects are errors.

## Durable format and ordering

- `project.json` is strict canonical pretty JSON with a final newline. Version one names the
  project ID, native object-store policy, current ledger count/byte length/tail digest, and exact
  latest input/output references without copying source data.
- `executions.jsonl` is an append-only sequence of strict canonical compact JSON records, one final
  newline per record. Each record binds its sequence and prior digest, node/spec, input references,
  configuration, native runtime, execution policy, scheduler output limit, cache key, result
  schema, canonical output content/artifact identity, record time, and terminal success.
- `pending-execution.json` is the sole project-level recovery intent. Object publication precedes
  intent publication; canonical intent precedes ledger append; a synced ledger precedes atomic
  head replacement; head durability precedes intent removal.
- `artifacts/` is opened by the existing `LocalArtifactStore`; no second content-addressing,
  publication, locator, verification, or artifact-staging implementation is permitted.

## Replay semantics

The current scheduler computes the sole cache key. Durable lookup considers only the exact key.
An exact record is reconstructed and the managed object is verified before bounded materialization.
The bytes are restored with the existing `MarklabProject::commit_success`; the returned reference
must equal the ledger reference. The existing scheduler then performs its normal hit decode and
canonical validation. A missing exact key is a miss. A duplicate key with different declared
identity is corruption, never a miss.

On a miss, the existing scheduler executes and commits in memory first. The durable layer reads the
verified inline bytes, constructs one schema-bound output draft, uses the existing store to publish
it, and advances intent/ledger/head in the stated order. Repeating an already committed exact
success is idempotent and does not append a second record.

## Recovery and failure atomicity

Opening a project holds its exclusive project lock and validates the entire bounded ledger.
A valid pending record is either appended and used to advance the head, used only to advance a
lagging head, or removed when head and ledger already match. Its object must verify first. An object
published before intent is an unreachable immutable object and does not fabricate success. Missing
intent with a ledger/head mismatch, conflicting intent, or malformed/tampered state is an error.

## Focused red–green evidence

1. Separate CLI processes produce miss then verified hit, identical scientific analysis, one ledger
   record, one object, and no second scientific execution.
2. Changed source bytes, canonical window, radii/null/seed/alpha/simulations, implementation,
   feature/toolchain/executable identity, codec, scheduler limit, or scientific resources miss.
3. Head, ledger, and intent are canonical fixed points and reject unknown fields/version, malformed
   JSON/newlines/digests/chains, truncation/appends, duplicate conflicting keys, and every bound.
4. Missing, replaced, truncated, appended, same-length mutated, or symlinked objects fail before
   replay. Root/control/store symlinks and non-regular paths fail without traversal fallback.
5. Faults after object publication, intent publication, ledger append, and head replacement leave
   either the prior visible success state or one deterministic recovery action; no partial record is
   exposed as a hit.
6. Existing classical node/CLI/result, project/workflow, catalog/store/recovery, analyze, no-default,
   and feature contracts retain their behavior.

## Non-goals for this increment

No backend registry or external process/Python/R/Stan/container execution; no arbitrary task runner,
multi-node DAG, remote/cloud store, UI/server/authentication; no new statistic or result 0.3 field;
no source-data copy; no generalized migration system beyond strict rejection of unsupported v1
state. These remain in the program tracker. BACK-01/WS-13 follows this prerequisite under DEC-0054.

## Exit

The complete cross-process workflow and failure/recovery cases pass; the applicable major-checkpoint
gates pass once; documentation and trackers are updated truthfully; the next backend outcome is
promoted; and one local checkpoint commit is created without remote action.
