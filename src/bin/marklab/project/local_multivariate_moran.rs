use std::borrow::Cow;

use super::is_sha256;
use marklab::autocorrelation::{LocalMultivariateMoranLimits, LocalMultivariateMoranResult};
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;

use super::cli_schema::LocalMultivariateMoranProjectArgs;
use super::{
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_RESULT_BYTES,
};
use crate::local_multivariate::{self, PreparedLocalMultivariateMoran};

const INPUT_KIND: &str = "application/vnd.marklab.source.local-multivariate-points+csv;version=1";
const WINDOW_KIND: &str = "application/vnd.marklab.source.observation-window+geojson;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.local-multivariate-moran+json;version=2";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-local-multivariate-moran-node-v2";

pub(super) fn run(args: LocalMultivariateMoranProjectArgs) -> Result<(), CliError> {
    let LocalMultivariateMoranProjectArgs {
        project: project_path,
        input: input_path,
        window: window_path,
        radius_um,
        permutations,
        seed,
        maximum_points,
        maximum_dimension,
        maximum_directed_edges,
        maximum_permutation_edge_evaluations,
        memory_budget_mib,
        out: output_path,
    } = args;
    const LABEL: &str = "local multivariate Moran sources";
    let memory_budget_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| CliError::Input("--memory-budget-mib is too large".into()))?;
    let sources = Source::identify_all([input_path, window_path], &[INPUT_KIND, WINDOW_KIND])?;
    let prepared =
        local_multivariate::prepare(sources[0].path(), sources[1].path(), memory_budget_bytes)
            .map_err(|error| CliError::Input(error.to_string()))?;
    Source::ensure_all_unchanged(&sources, LABEL)?;
    let (analysis, contract) = LocalMultivariateMoran::new(
        prepared,
        radius_um,
        permutations,
        seed,
        LocalMultivariateMoranLimits {
            maximum_points,
            maximum_dimension,
            maximum_directed_edges,
            maximum_permutation_edge_evaluations,
            memory_budget_bytes,
        },
        memory_budget_mib,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        analysis,
        LocalMultivariateMoran::execute,
        |analysis, output| analysis.validate_decoded(output),
    )?;

    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.local_multivariate_moran", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "local-multivariate-moran",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

struct LocalMultivariateMoran {
    prepared: PreparedLocalMultivariateMoran,
    radius_um: f64,
    permutations: usize,
    seed: u64,
    limits: LocalMultivariateMoranLimits,
}

impl LocalMultivariateMoran {
    fn new(
        prepared: PreparedLocalMultivariateMoran,
        radius_um: f64,
        permutations: usize,
        seed: u64,
        limits: LocalMultivariateMoranLimits,
        memory_budget_mib: usize,
    ) -> Result<(Self, NodeContract), CliError> {
        let LocalMultivariateMoranLimits {
            maximum_points,
            maximum_dimension,
            maximum_directed_edges,
            maximum_permutation_edge_evaluations,
            ..
        } = limits;
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-local-multivariate-moran-configuration-v1".as_slice(),
            prepared.source_schema.as_bytes(),
            radius_um.to_bits().to_be_bytes().as_slice(),
            permutations.to_string().as_bytes(),
            seed.to_string().as_bytes(),
            maximum_points.to_string().as_bytes(),
            maximum_dimension.to_string().as_bytes(),
            maximum_directed_edges.to_string().as_bytes(),
            maximum_permutation_edge_evaluations.to_string().as_bytes(),
            memory_budget_mib.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "serial;source_schema={};complete-multivariate-rows-within-strata;row-standardized-radius-weights;single-step-max-abs;permutations={permutations};seed={seed};maximum_points={maximum_points};maximum_dimension={maximum_dimension};maximum_directed_edges={maximum_directed_edges};maximum_permutation_edge_evaluations={maximum_permutation_edge_evaluations};memory_budget_mib={memory_budget_mib}",
            prepared.source_schema
        )
        .into_bytes();
        Ok((
            Self {
                prepared,
                radius_um,
                permutations,
                seed,
                limits,
            },
            NodeContract {
                id: "local-multivariate-moran",
                kind: "local_multivariate_moran",
                version: 1,
                configuration_digest,
                execution_policy: Cow::Owned(execution_policy),
                implementation_identity: IMPLEMENTATION_IDENTITY,
                output_kind: OUTPUT_KIND,
                codec: Codec::Json,
            },
        ))
    }

    fn validate_decoded(&self, result: &LocalMultivariateMoranResult) -> Result<(), NodeError> {
        let expected_stratum_count = self
            .prepared
            .points
            .iter()
            .map(|point| point.permutation_stratum.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len();
        let maximum = result
            .locations
            .iter()
            .map(|row| row.statistic.abs())
            .fold(0.0_f64, f64::max);
        let denominator = self.permutations as f64 + 1.0;
        if result.format != "marklab.local_multivariate_moran"
            || result.version != 1
            || result.statistical_unit != marklab::autocorrelation::MoranStatisticalUnit::CompleteMultivariateMarkRow
            || result.population_claim != marklab::autocorrelation::MoranPopulationClaim::WithinSpecimenFieldDiagnosticOnly
            || result.null != marklab::autocorrelation::MoranNullModel::CompleteRowsRandomlyLabeledWithinDeclaredStrata
            || result.multiplicity != marklab::marks::MultiplicityCorrection::SingleStepMaxAbsOverAllLocations
            || result.feature_names != self.prepared.feature_names
            || result.feature_means.len() != self.prepared.feature_names.len()
            || result.feature_standard_deviations.len() != self.prepared.feature_names.len()
            || result.feature_means.iter().any(|value| !value.is_finite())
            || result
                .feature_standard_deviations
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
            || result.radius_um.to_bits() != self.radius_um.to_bits()
            || result.point_count != self.prepared.points.len()
            || result.dimension != self.prepared.feature_names.len()
            || result.stratum_count != expected_stratum_count
            || result.permutations_requested != self.permutations
            || result.seed != self.seed
            || result.window_sha256 != self.prepared.window.descriptor().logical_digest.to_string()
            || !is_sha256(&result.weights_sha256)
            || result.directed_edge_count > self.limits.maximum_directed_edges
            || result.permutation_edge_evaluations
                != result.directed_edge_count.saturating_mul(self.permutations)
            || result.permutation_edge_evaluations > self.limits.maximum_permutation_edge_evaluations
            || result.retained_memory_bytes > self.limits.memory_budget_bytes
            || result.locations.len() != self.prepared.points.len()
            || result
                .locations
                .iter()
                .zip(&self.prepared.points)
                .any(|(row, point)| {
                    row.point_id != point.point_id
                        || !equal_within_one_ulp(row.x_um, point.x_um)
                        || !equal_within_one_ulp(row.y_um, point.y_um)
                })
            || !result.global_max_abs_statistic.is_finite()
            || !equal_within_one_ulp(result.global_max_abs_statistic, maximum)
            || !result.global_max_abs_p_value.is_finite()
            || !(0.0..=1.0).contains(&result.global_max_abs_p_value)
            || !permutation_probability(result.global_max_abs_p_value, denominator)
            || result.locations.iter().any(|row| {
                !row.statistic.is_finite()
                    || !row.raw_p_value.is_finite()
                    || !row.adjusted_p_value.is_finite()
                    || row.raw_p_value > row.adjusted_p_value
                    || !(0.0..=1.0).contains(&row.raw_p_value)
                    || !(0.0..=1.0).contains(&row.adjusted_p_value)
                    || !permutation_probability(row.raw_p_value, denominator)
                    || !permutation_probability(row.adjusted_p_value, denominator)
            })
        {
            return Err(NodeError::decode(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "decoded local multivariate Moran result does not match its cache-bound request",
            )));
        }
        Ok(())
    }

    fn execute(&self) -> Result<LocalMultivariateMoranResult, NodeError> {
        let result = local_multivariate::evaluate(
            &self.prepared,
            self.radius_um,
            self.permutations,
            self.seed,
            self.limits,
        )
        .map_err(NodeError::execution)?;
        local_multivariate::canonical_result_codec(result).map_err(NodeError::execution)
    }
}

fn permutation_probability(value: f64, denominator: f64) -> bool {
    let count = value * denominator;
    (count - count.round()).abs() <= 1e-9 && count >= 1.0 && count <= denominator
}

fn equal_within_one_ulp(left: f64, right: f64) -> bool {
    if !left.is_finite()
        || !right.is_finite()
        || left.is_sign_negative() != right.is_sign_negative()
    {
        return left.to_bits() == right.to_bits();
    }
    left.to_bits().abs_diff(right.to_bits()) <= 1
}
