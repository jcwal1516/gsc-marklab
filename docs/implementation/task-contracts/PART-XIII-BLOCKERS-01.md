# Task contract — PART-XIII-BLOCKERS-01 residual unified execution/validation prerequisites

Status: resolved after explicit user unpause on 2026-08-25

Date: 2026-08-25

Parent requirements: Part XIII §§122, 124–126, 129; FND-07, BACK-01, VAL-01, PERF-01.

`ExecuteAlgorithm` depends directly on the durable project/run/descriptor/artifact transaction layer
owned by `PLAT-DUR-01`. The full-program mandate explicitly pauses that task and forbids advancing,
testing, or fixing it. Existing individual CLIs are not relabelled as the unified descriptor-driven
transaction.

`DeterministicParallelReduce` requires an immediate production algorithm caller that fixes item
identity/order, partition count/boundaries, map output, associative reduction semantics, seed
derivation, thread-count behavior, and bitwise-versus-tolerance promise. No current Part XIII
caller owns those choices; a generic closure helper would be unused proof infrastructure. IC-0136
owns sequential numerical stability only.

`DiagnoseFittedModel` requires the canonical algorithm descriptor and typed fit/diagnostic families
from unified execution. Current Bayes, point-process, predictive, approximate, and causal workflows
already retain specialized diagnostics, but no one common fit schema can truthfully infer missing
Rhat/ESS/PPC/calibration/OOD/overlap/error-bound fields.

`RunCalibrationSuite` requires executable typed algorithm/scenario interfaces plus admitted
generator/truth/interval/decision schemas. Numerous specialized SBC, simulator, inference, and exact
oracle suites exist; the complete §127 catalogue still includes blocked general windows/topology,
multimodal factors, serial deformation, clone histories, and real domain-shift/noise models. A report
aggregator without those generators is not the pseudocode function.

`BenchmarkAlgorithmScaling` requires a method-specific equivalent-work workload generator, checksum,
sizes, repetitions, phase boundaries, output-sensitive counts, and applicable memory instrumentation.
Repository policy explicitly forbids generic or per-helper benchmarks without a concrete performance
question. Existing specialized ledgers remain authoritative; no benchmark is invented here.

Resume only when PLAT-DUR-01 is explicitly unpaused for unified execution, an actual parallel
algorithm needs fixed reduction semantics, typed descriptor/fit/scenario schemas have production
callers, or a method-specific performance acceptance criterion triggers the benchmark harness.

## Resolution

The user explicitly superseded the pause. EXEC-ALGORITHM-01 extracts the proven durable typed-node
transaction with `project classical` as its immediate caller. RUNTIME-VALIDATION-01 uses fixed-order
parallel reduction in Gaussian calibration, consumes IC-0196 HMC diagnostics, and runs a checksum-
verified equivalent-work smoke scaling workload. The original exclusions remain claim/scope limits.
