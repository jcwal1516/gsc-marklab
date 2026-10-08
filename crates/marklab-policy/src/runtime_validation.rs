use std::{
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use crate::PolicyError;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationScenario {
    pub scenario_id: String,
    pub truth: f64,
    pub observation_standard_deviation: f64,
    pub sample_size: usize,
    pub repetitions: usize,
    pub seed_namespace: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeValidationSpec {
    pub parallel_partitions: usize,
    pub alpha: f64,
    pub calibration_scenarios: Vec<CalibrationScenario>,
    pub benchmark_sizes: Vec<usize>,
    pub benchmark_repetitions: usize,
    pub benchmark_seed: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct BayesianHmcFitDiagnosticSpec {
    pub posterior_mean: f64,
    pub analytic_mean: f64,
    pub posterior_standard_deviation: f64,
    pub draw_count: usize,
    pub acceptance_rate: f64,
    pub maximum_absolute_energy_error: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ParallelReductionEvidence {
    pub partition_count: usize,
    pub ordering: &'static str,
    pub seed_derivation: &'static str,
    pub arithmetic_promise: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiagnosticCheck {
    pub check_id: &'static str,
    pub status: &'static str,
    pub value: f64,
    pub threshold: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct FitDiagnostics {
    pub fit_kind: &'static str,
    pub checks: Vec<DiagnosticCheck>,
    pub unavailable_not_applicable: Vec<&'static str>,
    pub overall_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct CalibrationResult {
    pub scenario_id: String,
    pub repetitions: usize,
    pub successes: usize,
    pub failures: usize,
    pub failure_rate: f64,
    pub bias: f64,
    pub rmse: f64,
    pub interval_coverage: f64,
    pub interval_coverage_wilson_95: [f64; 2],
    pub rejection_rate: f64,
    pub rejection_rate_wilson_95: [f64; 2],
    pub rate_interpretation: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct BenchmarkResult {
    pub size: usize,
    pub repetitions: usize,
    pub output_sensitive_items: usize,
    pub expected_checksum: u64,
    pub observed_checksum: u64,
    pub checksum_verified: bool,
    pub median_plan_construction_nanoseconds: u64,
    pub median_observed_evaluation_nanoseconds: u64,
    pub median_one_step_nanoseconds: u64,
    pub median_full_inference_nanoseconds: u64,
    pub median_result_assembly_nanoseconds: u64,
    pub artifact_persistence: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeValidationResult {
    pub format: &'static str,
    pub version: u32,
    pub parallel_reduction: ParallelReductionEvidence,
    pub fit_diagnostics: FitDiagnostics,
    pub calibration: Vec<CalibrationResult>,
    pub benchmark: Vec<BenchmarkResult>,
    pub benchmark_scope: &'static str,
    pub claim_status: &'static str,
}

pub fn deterministic_parallel_reduce<T, R, Map, Reduce>(
    items: &[T],
    partition_count: usize,
    seed_namespace: u64,
    map: Map,
    reduce: Reduce,
) -> Result<R, PolicyError>
where
    T: Sync,
    R: Send,
    Map: Fn(&T, u64) -> R + Sync,
    Reduce: Fn(R, R) -> R + Sync,
{
    if items.is_empty() || partition_count == 0 || partition_count > items.len() {
        return Err(PolicyError::Invalid(
            "parallel reduction requires nonempty items and partitions in 1..=item count".into(),
        ));
    }
    let width = items.len().div_ceil(partition_count);
    let map_ref = &map;
    let reduce_ref = &reduce;
    thread::scope(|scope| {
        let handles = (0..partition_count)
            .filter_map(|partition| {
                let start = partition * width;
                let end = items.len().min(start + width);
                (start < end).then(|| {
                    let slice = &items[start..end];
                    scope.spawn(move || {
                        let mut mapped = slice.iter().enumerate().map(|(offset, item)| {
                            map_ref(item, derived_seed(seed_namespace, start + offset))
                        });
                        let first = mapped.next().expect("validated nonempty partition");
                        mapped.fold(first, reduce_ref)
                    })
                })
            })
            .collect::<Vec<_>>();
        let mut outputs = Vec::with_capacity(handles.len());
        for handle in handles {
            outputs.push(
                handle
                    .join()
                    .map_err(|_| PolicyError::Invalid("parallel map partition panicked".into()))?,
            );
        }
        let mut outputs = outputs.into_iter();
        let first = outputs
            .next()
            .expect("validated item and partition counts produce output");
        Ok(outputs.fold(first, reduce_ref))
    })
}

pub fn run_runtime_validation(
    spec: RuntimeValidationSpec,
    fit: BayesianHmcFitDiagnosticSpec,
) -> Result<RuntimeValidationResult, PolicyError> {
    validate_spec(&spec, fit)?;
    let fit_diagnostics = diagnose_hmc(fit);
    let calibration = spec
        .calibration_scenarios
        .iter()
        .map(|scenario| calibrate(scenario, spec.alpha, spec.parallel_partitions))
        .collect::<Result<Vec<_>, _>>()?;
    let benchmark = spec
        .benchmark_sizes
        .iter()
        .map(|&size| {
            benchmark_sum(
                size,
                spec.benchmark_repetitions,
                spec.parallel_partitions,
                spec.benchmark_seed,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RuntimeValidationResult {
        format: "marklab.runtime_validation_and_scaling",
        version: 1,
        parallel_reduction: ParallelReductionEvidence {
            partition_count: spec.parallel_partitions,
            ordering: "fixed_contiguous_partitions_then_partition_index",
            seed_derivation: "splitmix64_namespace_plus_global_item_index",
            arithmetic_promise: "bitwise_for_integer_checksum_tolerance_stable_for_f64_calibration",
        },
        fit_diagnostics,
        calibration,
        benchmark,
        benchmark_scope: "stable_integer_sum_smoke_equivalent_work_not_general_algorithm_scaling",
        claim_status: "synthetic_runtime_validation_and_smoke_scaling_only",
    })
}

fn validate_spec(
    spec: &RuntimeValidationSpec,
    fit: BayesianHmcFitDiagnosticSpec,
) -> Result<(), PolicyError> {
    let valid_scenarios = spec.calibration_scenarios.iter().all(|scenario| {
        !scenario.scenario_id.is_empty()
            && scenario.scenario_id.len() <= 128
            && scenario.truth.is_finite()
            && scenario.observation_standard_deviation.is_finite()
            && scenario.observation_standard_deviation > 0.0
            && (4..=100_000).contains(&scenario.sample_size)
            && (100..=100_000).contains(&scenario.repetitions)
            && spec.parallel_partitions <= scenario.repetitions
    });
    if !(1..=64).contains(&spec.parallel_partitions)
        || !spec.alpha.is_finite()
        || !(0.0..0.5).contains(&spec.alpha)
        || !(1..=32).contains(&spec.calibration_scenarios.len())
        || !valid_scenarios
        || !(1..=16).contains(&spec.benchmark_sizes.len())
        || spec
            .benchmark_sizes
            .iter()
            .any(|size| !(100..=1_000_000).contains(size) || *size < spec.parallel_partitions)
        || !(1..=20).contains(&spec.benchmark_repetitions)
        || !fit.posterior_mean.is_finite()
        || !fit.analytic_mean.is_finite()
        || !fit.posterior_standard_deviation.is_finite()
        || fit.posterior_standard_deviation <= 0.0
        || fit.draw_count < 500
        || !fit.acceptance_rate.is_finite()
        || !fit.maximum_absolute_energy_error.is_finite()
    {
        return Err(PolicyError::Invalid(
            "runtime validation requires bounded HMC diagnostics, scenarios, partitions, and benchmark sizes"
                .into(),
        ));
    }
    Ok(())
}

fn diagnose_hmc(fit: BayesianHmcFitDiagnosticSpec) -> FitDiagnostics {
    let mean_error = (fit.posterior_mean - fit.analytic_mean).abs();
    let checks = vec![
        DiagnosticCheck {
            check_id: "analytic_posterior_mean",
            status: if mean_error < 0.05 {
                "passed"
            } else {
                "failed"
            },
            value: mean_error,
            threshold: "absolute_error<0.05",
        },
        DiagnosticCheck {
            check_id: "hmc_acceptance",
            status: if (0.6..=1.0).contains(&fit.acceptance_rate) {
                "passed"
            } else {
                "failed"
            },
            value: fit.acceptance_rate,
            threshold: "[0.6,1]",
        },
        DiagnosticCheck {
            check_id: "hamiltonian_energy_error",
            status: if fit.maximum_absolute_energy_error < 0.2 {
                "passed"
            } else {
                "failed"
            },
            value: fit.maximum_absolute_energy_error,
            threshold: "maximum_absolute<0.2",
        },
    ];
    let passed = checks.iter().all(|check| check.status == "passed");
    FitDiagnostics {
        fit_kind: "bayesian_hmc",
        checks,
        unavailable_not_applicable: vec![
            "rhat_single_chain_not_applicable",
            "tree_depth_fixed_step_hmc_not_applicable",
            "point_process_diagnostics_not_applicable",
            "predictive_diagnostics_not_applicable",
            "causal_diagnostics_not_applicable",
        ],
        overall_status: if passed { "passed" } else { "failed" },
    }
}

#[derive(Clone)]
struct CalibrationFit {
    estimate: f64,
    lower: f64,
    upper: f64,
    rejected_zero: bool,
}

fn calibrate(
    scenario: &CalibrationScenario,
    alpha: f64,
    partitions: usize,
) -> Result<CalibrationResult, PolicyError> {
    let repetitions = (0..scenario.repetitions).collect::<Vec<_>>();
    let standard_error =
        scenario.observation_standard_deviation / (scenario.sample_size as f64).sqrt();
    let critical = inverse_standard_normal(1.0 - alpha / 2.0);
    let fits = deterministic_parallel_reduce(
        &repetitions,
        partitions,
        scenario.seed_namespace,
        |_, seed| {
            let mut generator = SplitMix64::new(seed);
            let sum = (0..scenario.sample_size)
                .map(|_| standard_normal(&mut generator))
                .sum::<f64>();
            let estimate = scenario.truth
                + scenario.observation_standard_deviation * sum / scenario.sample_size as f64;
            vec![CalibrationFit {
                estimate,
                lower: estimate - critical * standard_error,
                upper: estimate + critical * standard_error,
                rejected_zero: estimate.abs() > critical * standard_error,
            }]
        },
        |mut left: Vec<CalibrationFit>, mut right: Vec<CalibrationFit>| {
            left.append(&mut right);
            left
        },
    )?;
    let estimates = fits.iter().map(|fit| fit.estimate).collect::<Vec<_>>();
    let bias = estimates.iter().sum::<f64>() / estimates.len() as f64 - scenario.truth;
    let rmse = (estimates
        .iter()
        .map(|estimate| (estimate - scenario.truth).powi(2))
        .sum::<f64>()
        / estimates.len() as f64)
        .sqrt();
    let covered = fits
        .iter()
        .filter(|fit| fit.lower <= scenario.truth && scenario.truth <= fit.upper)
        .count();
    let rejected = fits.iter().filter(|fit| fit.rejected_zero).count();
    Ok(CalibrationResult {
        scenario_id: scenario.scenario_id.clone(),
        repetitions: scenario.repetitions,
        successes: scenario.repetitions,
        failures: 0,
        failure_rate: 0.0,
        bias,
        rmse,
        interval_coverage: covered as f64 / scenario.repetitions as f64,
        interval_coverage_wilson_95: wilson(covered, scenario.repetitions),
        rejection_rate: rejected as f64 / scenario.repetitions as f64,
        rejection_rate_wilson_95: wilson(rejected, scenario.repetitions),
        rate_interpretation: if scenario.truth == 0.0 {
            "type_i_error"
        } else {
            "power"
        },
    })
}

fn benchmark_sum(
    size: usize,
    repetitions: usize,
    partitions: usize,
    seed: u64,
) -> Result<BenchmarkResult, PolicyError> {
    let mut plan_times = Vec::new();
    let mut observed_times = Vec::new();
    let mut step_times = Vec::new();
    let mut full_times = Vec::new();
    let mut assembly_times = Vec::new();
    let mut expected = 0_u64;
    let mut observed = 0_u64;
    for repetition in 0..=repetitions {
        let start = Instant::now();
        let values = (0..size)
            .map(|index| derived_seed(seed ^ repetition as u64, index))
            .collect::<Vec<_>>();
        let plan = start.elapsed();
        let start = Instant::now();
        expected = values.iter().copied().fold(0_u64, u64::wrapping_add);
        let evaluation = start.elapsed();
        let start = Instant::now();
        let _one_step = values[..values.len().div_ceil(partitions)]
            .iter()
            .copied()
            .fold(0_u64, u64::wrapping_add);
        let step = start.elapsed();
        let start = Instant::now();
        observed = deterministic_parallel_reduce(
            &values,
            partitions,
            seed,
            |value, _| *value,
            u64::wrapping_add,
        )?;
        let full = start.elapsed();
        let start = Instant::now();
        let _assembled = (size, expected, observed, expected == observed);
        let assembly = start.elapsed();
        if repetition > 0 {
            plan_times.push(plan);
            observed_times.push(evaluation);
            step_times.push(step);
            full_times.push(full);
            assembly_times.push(assembly);
        }
    }
    Ok(BenchmarkResult {
        size,
        repetitions,
        output_sensitive_items: size,
        expected_checksum: expected,
        observed_checksum: observed,
        checksum_verified: expected == observed,
        median_plan_construction_nanoseconds: median_ns(&mut plan_times),
        median_observed_evaluation_nanoseconds: median_ns(&mut observed_times),
        median_one_step_nanoseconds: median_ns(&mut step_times),
        median_full_inference_nanoseconds: median_ns(&mut full_times),
        median_result_assembly_nanoseconds: median_ns(&mut assembly_times),
        artifact_persistence: "performed_transactionally_by_cli_not_self_timed",
    })
}

fn median_ns(values: &mut [Duration]) -> u64 {
    values.sort_unstable();
    u64::try_from(values[values.len() / 2].as_nanos())
        .unwrap_or(u64::MAX)
        .max(1)
}

fn wilson(successes: usize, repetitions: usize) -> [f64; 2] {
    let z = 1.959_963_984_540_054;
    let n = repetitions as f64;
    let proportion = successes as f64 / n;
    let denominator = 1.0 + z * z / n;
    let center = (proportion + z * z / (2.0 * n)) / denominator;
    let half =
        z * (proportion * (1.0 - proportion) / n + z * z / (4.0 * n * n)).sqrt() / denominator;
    [center - half, center + half]
}

fn derived_seed(namespace: u64, index: usize) -> u64 {
    let index = u64::try_from(index).unwrap_or(u64::MAX);
    mix64(namespace ^ index.wrapping_mul(0x9e37_79b9_7f4a_7c15))
}

fn mix64(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let bits = mix64(self.0) >> 11;
        (bits as f64 + 0.5) / ((1_u64 << 53) as f64)
    }
}

fn standard_normal(generator: &mut SplitMix64) -> f64 {
    (-2.0 * generator.uniform().ln()).sqrt()
        * (2.0 * std::f64::consts::PI * generator.uniform()).cos()
}

fn inverse_standard_normal(probability: f64) -> f64 {
    const P_LOW: f64 = 0.024_25;
    const P_HIGH: f64 = 1.0 - P_LOW;
    if probability < P_LOW {
        let q = (-2.0 * probability.ln()).sqrt();
        (((((-0.007_784_894_002_430_293 * q - 0.322_396_458_041_136_5) * q
            - 2.400_758_277_161_838)
            * q
            - 2.549_732_539_343_734)
            * q
            + 4.374_664_141_464_968)
            * q
            + 2.938_163_982_698_783)
            / ((((0.007_784_695_709_041_462 * q + 0.322_467_129_070_039_8) * q
                + 2.445_134_137_142_996)
                * q
                + 3.754_408_661_907_416)
                * q
                + 1.0)
    } else if probability > P_HIGH {
        -inverse_standard_normal(1.0 - probability)
    } else {
        let q = probability - 0.5;
        let r = q * q;
        (((((-39.696_830_286_653_76 * r + 220.946_098_424_520_5) * r - 275.928_510_446_968_7) * r
            + 138.357_751_867_269)
            * r
            - 30.664_798_066_147_16)
            * r
            + 2.506_628_277_459_239)
            * q
            / (((((-54.476_098_798_224_06 * r + 161.585_836_858_040_9) * r
                - 155.698_979_859_886_6)
                * r
                + 66.801_311_887_719_72)
                * r
                - 13.280_681_552_885_72)
                * r
                + 1.0)
    }
}
