//! Source-identified scientific partial-transport workflow, independent of CSV/process I/O.
use super::{
    PartialTransportOptimizer, PartialTransportPlanEntry, PartialTransportSpec,
    FEASIBILITY_TOLERANCE,
};
use crate::BayesError;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
#[path = "solver.rs"]
mod solver;

/// Native scientific implementation identity, independent of Python reference provenance.
#[derive(Debug, Serialize)]
pub struct NativePartialTransportBackend {
    pub name: &'static str,
    pub version: &'static str,
    pub implementation_sha256: String,
}
/// Native version-2 dense partial plan and unchanged objective/feasibility summaries.
#[derive(Debug, Serialize)]
pub struct PartialTransportFit {
    pub format: &'static str,
    pub version: u32,
    pub backend: NativePartialTransportBackend,
    pub request_sha256: String,
    pub optimizer: PartialTransportOptimizer,
    pub plan: Vec<PartialTransportPlanEntry>,
    pub source_marginals: Vec<f64>,
    pub target_marginals: Vec<f64>,
    pub transported_mass: f64,
    pub unmatched_source_mass: Vec<f64>,
    pub unmatched_target_mass: Vec<f64>,
    pub transport_cost: f64,
    pub entropy: f64,
    pub regularized_objective: f64,
    pub maximum_constraint_violation: f64,
    pub constraint_status: &'static str,
}

/// Minimize the fixed-mass entropic partial objective with capacity inequalities.
///
/// Preserves 1..=64 supports per side, supplied support/row-major ordering, nonnegative finite
/// capacities/costs, positive feasible mass/epsilon, 1..=3600 seconds and 16 MiB serialized output.
/// Log-domain dual coordinate updates perform at most 2000 sweeps; success requires 1e-8 absolute
/// feasibility and a 1e-12 relative/absolute primal-dual residual. Zero supports remain in output.
/// Memory is O(rows*columns), work O(sweeps*rows*columns). Invalid inputs, nonfinite arithmetic,
/// nonconvergence, size and cooperative deadline exhaustion are errors. Hard cancellation belongs
/// to the native child runtime. Unmatched mass and plan mass do not establish biological novelty
/// or cell identity.
pub fn fit_partial_transport(
    spec: PartialTransportSpec,
) -> Result<PartialTransportFit, BayesError> {
    let start = Instant::now();
    spec.validate()?;
    check_plan_identity_budget(&spec)?;
    let deadline = start + Duration::from_secs(spec.timeout_seconds);
    // The required executable identity is immutable within this process.
    static IMPLEMENTATION_SHA256: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        crate::sha256_hex(
            concat!(
                include_str!("native.rs"),
                include_str!("solver.rs"),
                include_str!("../partial_transport.rs"),
                include_str!("../transport.rs")
            )
            .as_bytes(),
        )
    });
    let implementation_sha256 = IMPLEMENTATION_SHA256.clone();
    let request_sha256 = input_identity(&implementation_sha256, &spec);
    let solved = solver::solve(&spec, deadline)?;
    let rows = spec.source.len();
    let columns = spec.target.len();
    let mut source_marginals = vec![0.; rows];
    let mut target_marginals = vec![0.; columns];
    let mut plan = Vec::with_capacity(rows * columns);
    let mut transport_cost = 0.;
    let mut entropy = 0.;
    let mut regularization = 0.;
    for (index, mass) in solved.masses.into_iter().enumerate() {
        let i = index / columns;
        let j = index % columns;
        let cost = spec.costs_row_major[index];
        source_marginals[i] += mass;
        target_marginals[j] += mass;
        transport_cost += mass * cost;
        if mass > 0. {
            let logarithm = mass.ln();
            entropy -= mass * logarithm;
            regularization += mass * (logarithm - 1.);
        }
        plan.push(PartialTransportPlanEntry {
            source_id: spec.source[i].id.clone(),
            target_id: spec.target[j].id.clone(),
            mass,
            cost,
        });
    }
    let transported_mass = source_marginals.iter().sum::<f64>();
    let unmatched_source_mass = spec
        .source
        .iter()
        .zip(&source_marginals)
        .map(|(r, m)| r.mass - m)
        .collect::<Vec<_>>();
    let unmatched_target_mass = spec
        .target
        .iter()
        .zip(&target_marginals)
        .map(|(r, m)| r.mass - m)
        .collect::<Vec<_>>();
    let maximum_constraint_violation = unmatched_source_mass
        .iter()
        .chain(&unmatched_target_mass)
        .fold(
            (transported_mass - spec.transported_mass).abs(),
            |error, left| error.max((-left).max(0.)),
        );
    let regularized_objective = transport_cost + spec.epsilon * regularization;
    if [
        transported_mass,
        transport_cost,
        entropy,
        regularized_objective,
        maximum_constraint_violation,
    ]
    .iter()
    .any(|x| !x.is_finite())
        || maximum_constraint_violation > FEASIBILITY_TOLERANCE
    {
        return Err(BayesError::InvalidSpec(
            "partial transport final quantities are nonfinite or infeasible".into(),
        ));
    }
    let fit = PartialTransportFit {
        format: "marklab.partial_ot",
        version: 2,
        backend: NativePartialTransportBackend {
            name: "marklab-rust",
            version: env!("CARGO_PKG_VERSION"),
            implementation_sha256,
        },
        request_sha256,
        optimizer: PartialTransportOptimizer {
            method: "log_domain_partial_transport_dual".into(),
            success: true,
            iterations: solved.iterations,
            message: "feasibility and primal-dual residual converged".into(),
        },
        plan,
        source_marginals,
        target_marginals,
        transported_mass,
        unmatched_source_mass,
        unmatched_target_mass,
        transport_cost,
        entropy,
        regularized_objective,
        maximum_constraint_violation,
        constraint_status: "feasible_within_tolerance",
    };
    if serde_json::to_vec(&fit)?.len() > 16 * 1024 * 1024 {
        return Err(BayesError::InvalidSpec(
            "partial transport result exceeds 16 MiB".into(),
        ));
    }
    solver::check_deadline(deadline)?;
    Ok(fit)
}
fn input_identity(implementation: &str, spec: &PartialTransportSpec) -> String {
    let mut hash = Sha256::new();
    hash.update(b"marklab.partial-transport.spec.v1\0");
    hash_text(&mut hash, implementation);
    for support in [&spec.source, &spec.target] {
        hash.update((support.len() as u64).to_le_bytes());
        for row in support {
            hash_text(&mut hash, &row.id);
            hash.update(row.mass.to_bits().to_le_bytes());
        }
    }
    for value in &spec.costs_row_major {
        hash.update(value.to_bits().to_le_bytes());
    }
    hash.update(spec.transported_mass.to_bits().to_le_bytes());
    hash.update(spec.epsilon.to_bits().to_le_bytes());
    hash.update(spec.timeout_seconds.to_le_bytes());
    format!("{:x}", hash.finalize())
}
fn hash_text(hash: &mut Sha256, text: &str) {
    hash.update((text.len() as u64).to_le_bytes());
    hash.update(text.as_bytes());
}

// Dense output repeats each source ID once per target (and conversely). Bound this expansion
// before allocating the plan or hashing arbitrarily long typed inputs. JSON escapes are counted
// without constructing a temporary escaped string; final serialization checks the other fields.
fn check_plan_identity_budget(spec: &PartialTransportSpec) -> Result<(), BayesError> {
    let mut bytes = 0_usize;
    for (support, repetitions) in [
        (&spec.source, spec.target.len()),
        (&spec.target, spec.source.len()),
    ] {
        for row in support {
            for byte in row.id.bytes() {
                let width = match byte {
                    b'"' | b'\\' | 8 | 9 | 10 | 12 | 13 => 2,
                    0..=31 => 6,
                    _ => 1,
                };
                bytes = bytes.saturating_add(width * repetitions);
                if bytes > 16 * 1024 * 1024 {
                    return Err(BayesError::InvalidSpec(
                        "partial transport plan identity expansion exceeds 16 MiB".into(),
                    ));
                }
            }
        }
    }
    Ok(())
}
