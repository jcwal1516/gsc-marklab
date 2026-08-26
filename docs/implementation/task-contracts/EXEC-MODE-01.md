# Task contract — EXEC-MODE-01 backend/resource/accuracy mode selection

Status: complete

Date: 2026-08-25

Parent requirements: Part XIII §123 `SelectExecutionMode`, BACK-01, REL-01.

`marklab policy select-mode` consumes canonical ordered unique mode descriptors, available backend
IDs, positive data size/memory/runtime budgets, optional explicit mode request, optional nonnegative
maximum error, and an approximation-approval flag. Each mode declares exact/approximate kind,
required backend, checked base-plus-per-item memory/runtime estimates, and optional validated maximum
error. Approximate modes require a finite nonnegative validated error; exact modes have error zero.

Every candidate retains backend/resource/accuracy/approval feasibility and one rejection reason.
An explicit request evaluates only that mode and returns a typed unsupported/backend/resource/
accuracy/approximation-not-permitted result. Automatic selection scans scientific preference order,
selects the first fully permitted candidate, may continue past an unapproved approximation to a
later exact mode, and otherwise returns approximation approval required or resource limit exceeded.
It never silently approves approximation.

The oracle makes exact mode exceed memory while an approximate mode is feasible: without approval
the result is `approximation_not_permitted`; with approval it selects approximation and retains both
assessments. This is policy planning, not backend execution or runtime benchmarking.
