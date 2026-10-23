use crate::codecs::embedding_envelope::read_rows;

use std::{
    borrow::Cow,
    collections::{BTreeMap, HashSet},
    io,
};

use marklab_embeddings::analysis::{
    test_embedding_spatial_dependence, EmbeddingDistanceBin, EmbeddingEnvelopeError,
    EmbeddingEnvelopeRow, EmbeddingSpatialCurveFunction, EmbeddingSpatialDependenceResult,
    EmbeddingSpatialDependenceSpec,
};
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;
use serde::{Deserialize, Serialize};

use super::cli_schema::{
    EmbeddingSpatialDependenceEnvelopeLimits, EmbeddingSpatialDependenceEnvelopeProjectArgs,
};
use super::{
    bayes,
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES,
};

const INPUT_KIND: &str = "application/vnd.marklab.source.embedding-envelope-rows-csv;version=1";
const BINS_KIND: &str = "application/vnd.marklab.source.distance-bins-csv;version=1";
const OUTPUT_KIND: &str =
    "application/vnd.marklab.embedding-spatial-dependence-envelope+json;version=1";
const IMPLEMENTATION_IDENTITY: &str =
    "marklab-project-embedding-spatial-dependence-envelope-node-v1";

pub(super) fn run(args: EmbeddingSpatialDependenceEnvelopeProjectArgs) -> Result<(), CliError> {
    let EmbeddingSpatialDependenceEnvelopeProjectArgs {
        project: project_path,
        input: input_path,
        bins: bins_path,
        curve,
        permutations,
        alpha,
        seed,
        limits,
        out: output_path,
    } = args;
    let EmbeddingSpatialDependenceEnvelopeLimits {
        maximum_points,
        maximum_dimension,
        maximum_pair_visits,
        memory_budget_mib,
    } = limits;
    const LABEL: &str = "embedding envelope sources";
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| CliError::Input("--memory-budget-mib is too large".into()))?;
    if maximum_points == 0
        || maximum_dimension == 0
        || maximum_pair_visits == 0
        || memory_bytes == 0
    {
        return Err(CliError::Input(
            "embedding envelope point, dimension, pair-work, and memory limits must be positive"
                .into(),
        ));
    }
    let curve_function = EmbeddingSpatialCurveFunction::parse(&curve).map_err(map_error)?;
    let input_bytes = read_bounded(&input_path, memory_bytes)?;
    let bins_bytes = read_bounded(&bins_path, memory_bytes)?;
    let source_bytes = input_bytes
        .len()
        .checked_add(bins_bytes.len())
        .ok_or_else(|| CliError::Input("embedding envelope source-byte total overflowed".into()))?;
    if source_bytes > memory_bytes {
        return Err(CliError::Input(
            "embedding envelope source files exceed the retained-memory budget".into(),
        ));
    }
    let sources = vec![
        Source::from_bytes(input_path, INPUT_KIND, &input_bytes)?,
        Source::from_bytes(bins_path, BINS_KIND, &bins_bytes)?,
    ];
    let (rows, feature_names) = read_rows(&input_bytes)?;
    let mut bins = bayes::embedding_spatial::read_bins(&bins_bytes)?;
    let requirements = preflight(
        &rows,
        &feature_names,
        &mut bins,
        permutations,
        alpha,
        limits,
    )?;
    let retained = retained_bytes(
        source_bytes,
        &rows,
        &feature_names,
        &bins,
        permutations,
        requirements.unordered_pairs,
    )?;
    if retained > memory_bytes {
        return Err(CliError::Input(format!(
            "embedding envelope retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    Source::ensure_all_unchanged(&sources, LABEL)?;
    drop(input_bytes);
    drop(bins_bytes);

    let input_sha256 = sources[0].artifact().digest().to_string();
    let bins_sha256 = sources[1].artifact().digest().to_string();
    let analysis = EmbeddingSpatialDependenceSpec {
        rows,
        feature_names,
        bins,
        curve_function,
        permutations,
        alpha,
        seed,
        maximum_pair_visits,
    };
    let (analysis, contract) = EmbeddingSpatialEnvelope::new(
        analysis,
        requirements,
        input_sha256,
        bins_sha256,
        curve,
        limits,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        analysis,
        EmbeddingSpatialEnvelope::execute,
        |analysis, output| analysis.validate(output).map_err(NodeError::decode),
    )?;

    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.embedding_spatial_dependence_envelope", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "embedding-spatial-dependence-envelope",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn read_bounded(path: &std::path::Path, memory_bytes: usize) -> Result<Vec<u8>, CliError> {
    let maximum = (MAXIMUM_INPUT_BYTES as usize).min(memory_bytes);
    crate::input_file::read_regular_file(
        path,
        maximum as u64,
        &format!(
            "embedding envelope source must be a regular file within {maximum} bytes: {}",
            path.display()
        ),
    )
}

#[derive(Clone)]
struct Requirements {
    unordered_pairs: u64,
    pair_visits: u64,
    component_operations: u64,
    stratum_count: usize,
    bin_counts: Vec<u64>,
}

fn preflight(
    rows: &[EmbeddingEnvelopeRow],
    feature_names: &[String],
    bins: &mut [EmbeddingDistanceBin],
    permutations: u32,
    alpha: f64,
    limits: EmbeddingSpatialDependenceEnvelopeLimits,
) -> Result<Requirements, CliError> {
    let EmbeddingSpatialDependenceEnvelopeLimits {
        maximum_points,
        maximum_dimension,
        maximum_pair_visits,
        ..
    } = limits;
    if rows.len() > maximum_points {
        return Err(CliError::Input(format!(
            "point count exceeds maximum_points {maximum_points}"
        )));
    }
    if feature_names.len() > maximum_dimension {
        return Err(CliError::Input(format!(
            "embedding dimension exceeds maximum_dimension {maximum_dimension}"
        )));
    }
    let stratum_count = validate_input(rows, feature_names, permutations, alpha)?;
    validate_bins(bins)?;

    let mut groups = BTreeMap::<&str, Vec<&EmbeddingEnvelopeRow>>::new();
    for row in rows {
        groups
            .entry(row.permutation_stratum.as_str())
            .or_default()
            .push(row);
    }
    let unordered_pairs = groups.values().try_fold(0_u64, |total, rows| {
        let count = u64::try_from(rows.len())
            .map_err(|_| CliError::Input("embedding envelope pair count exceeds u64".into()))?;
        let pairs = count
            .checked_mul(count.saturating_sub(1))
            .and_then(|value| value.checked_div(2))
            .ok_or_else(|| CliError::Input("embedding envelope pair count overflowed".into()))?;
        total
            .checked_add(pairs)
            .ok_or_else(|| CliError::Input("embedding envelope pair count overflowed".into()))
    })?;
    let pair_visits = unordered_pairs
        .checked_mul(u64::from(permutations) + 1)
        .ok_or_else(|| CliError::Input("embedding envelope pair work overflowed".into()))?;
    if pair_visits > maximum_pair_visits {
        return Err(CliError::Input(format!(
            "{pair_visits} observed/permuted pair visits exceed maximum_pair_visits {maximum_pair_visits}"
        )));
    }
    let component_operations = pair_visits
        .checked_mul(feature_names.len() as u64)
        .ok_or_else(|| CliError::Input("embedding envelope component work overflowed".into()))?;
    if component_operations > 250_000_000 {
        return Err(CliError::Input(format!(
            "{component_operations} embedding component operations exceed the fixed ceiling 250000000"
        )));
    }

    let mut bin_counts = vec![0_u64; bins.len()];
    for group in groups.values() {
        for (left, left_row) in group.iter().enumerate() {
            for right_row in &group[(left + 1)..] {
                let distance =
                    (left_row.x_um - right_row.x_um).hypot(left_row.y_um - right_row.y_um);
                if !distance.is_finite() {
                    return Err(CliError::Backend(
                        "a spatial pair distance is non-finite".into(),
                    ));
                }
                if let Some(index) = bins.iter().position(|bin| {
                    distance >= bin.lower_um
                        && (distance < bin.upper_um
                            || (bin.upper_inclusive && distance <= bin.upper_um))
                }) {
                    bin_counts[index] += 1;
                }
            }
        }
    }
    Ok(Requirements {
        unordered_pairs,
        pair_visits,
        component_operations,
        stratum_count,
        bin_counts,
    })
}

fn validate_input(
    rows: &[EmbeddingEnvelopeRow],
    feature_names: &[String],
    permutations: u32,
    alpha: f64,
) -> Result<usize, CliError> {
    if !(8..=10_000).contains(&rows.len())
        || !(2..=128).contains(&feature_names.len())
        || !(20..=10_000).contains(&permutations)
        || !alpha.is_finite()
        || alpha <= 0.0
        || alpha >= 1.0
        || (f64::from(permutations) + 1.0) * alpha < 1.0
    {
        return Err(CliError::Input(
            "envelope dimensions, alpha, permutations, or resource bound are invalid".into(),
        ));
    }
    let mut features = HashSet::new();
    if feature_names.iter().any(|name| {
        name.is_empty()
            || name.trim() != name
            || !name.starts_with("embedding_")
            || !features.insert(name.as_str())
    }) {
        return Err(CliError::Input(
            "envelope feature names must be unique exact embedding_* names".into(),
        ));
    }
    let mut identifiers = HashSet::new();
    let mut strata = BTreeMap::<&str, usize>::new();
    for row in rows {
        if row.object_id.is_empty()
            || row.object_id.trim() != row.object_id
            || !identifiers.insert(row.object_id.as_str())
            || row.permutation_stratum.is_empty()
            || row.permutation_stratum.trim() != row.permutation_stratum
            || !row.x_um.is_finite()
            || !row.y_um.is_finite()
            || row.embedding.len() != feature_names.len()
            || row.embedding.iter().any(|value| !value.is_finite())
        {
            return Err(CliError::Input(
                "envelope rows require unique exact identities/strata, finite coordinates, and complete finite vectors"
                    .into(),
            ));
        }
        *strata.entry(row.permutation_stratum.as_str()).or_default() += 1;
    }
    if strata.values().any(|count| *count < 2) {
        return Err(CliError::Input(
            "every permutation stratum requires at least two rows".into(),
        ));
    }
    Ok(strata.len())
}

fn validate_bins(bins: &mut [EmbeddingDistanceBin]) -> Result<(), CliError> {
    if !(1..=256).contains(&bins.len()) {
        return Err(CliError::Input("distance bin count is invalid".into()));
    }
    let mut identifiers = HashSet::new();
    for index in 0..bins.len() {
        if bins[index].bin_id.is_empty()
            || !identifiers.insert(bins[index].bin_id.as_str())
            || !bins[index].lower_um.is_finite()
            || !bins[index].upper_um.is_finite()
            || bins[index].lower_um < 0.0
            || bins[index].lower_um >= bins[index].upper_um
            || (index > 0 && bins[index].lower_um.to_bits() != bins[index - 1].upper_um.to_bits())
        {
            return Err(CliError::Input(
                "distance bins must be unique, contiguous, and increasing".into(),
            ));
        }
    }
    let final_bin = bins.len() - 1;
    for (index, bin) in bins.iter_mut().enumerate() {
        bin.upper_inclusive = index == final_bin;
    }
    Ok(())
}

fn retained_bytes(
    source_bytes: usize,
    rows: &[EmbeddingEnvelopeRow],
    feature_names: &[String],
    bins: &[EmbeddingDistanceBin],
    permutations: u32,
    unordered_pairs: u64,
) -> Result<usize, CliError> {
    let points = rows.len();
    let dimension = feature_names.len();
    let pairs = usize::try_from(unordered_pairs)
        .map_err(|_| CliError::Input("embedding envelope pair memory exceeds usize".into()))?;
    let curve_count = usize::try_from(permutations)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| CliError::Input("embedding envelope curve count overflowed".into()))?;
    let curve_cells = curve_count
        .checked_mul(bins.len())
        .ok_or_else(|| CliError::Input("embedding envelope curve memory overflowed".into()))?;

    // The node owns one parsed analysis and execution clones it. The algorithm additionally
    // retains every pair plan, index maps, all null curves, and the ERL curve/rank workspaces.
    let analysis = source_bytes
        .checked_add(points.saturating_mul(dimension).saturating_mul(8))
        .and_then(|value| value.checked_add(points.saturating_mul(256)))
        .and_then(|value| value.checked_add(dimension.saturating_mul(128)))
        .and_then(|value| value.checked_add(bins.len().saturating_mul(512)))
        .ok_or_else(|| CliError::Input("embedding envelope analysis memory overflowed".into()))?;
    let pair_plans = pairs
        .checked_mul(3 * std::mem::size_of::<usize>())
        .ok_or_else(|| CliError::Input("embedding envelope pair-plan memory overflowed".into()))?;
    let index_workspaces = points
        .checked_mul(6 * std::mem::size_of::<usize>())
        .and_then(|value| {
            value.checked_add(curve_count.saturating_mul(8 * std::mem::size_of::<usize>()))
        })
        .ok_or_else(|| CliError::Input("embedding envelope index memory overflowed".into()))?;
    let curve_workspaces = curve_cells
        .checked_mul(6 * std::mem::size_of::<f64>())
        .and_then(|value| {
            value.checked_add(
                curve_count
                    .saturating_mul(4)
                    .saturating_mul(std::mem::size_of::<Vec<f64>>()),
            )
        })
        .and_then(|value| value.checked_add(bins.len().saturating_mul(16 * 8)))
        .ok_or_else(|| CliError::Input("embedding envelope ERL memory overflowed".into()))?;
    analysis
        .checked_mul(2)
        .and_then(|value| value.checked_add(pair_plans))
        .and_then(|value| value.checked_add(index_workspaces))
        .and_then(|value| value.checked_add(curve_workspaces))
        .ok_or_else(|| {
            CliError::Input("embedding envelope retained-memory estimate overflowed".into())
        })
}

struct EmbeddingSpatialEnvelope {
    analysis: EmbeddingSpatialDependenceSpec,
    requirements: Requirements,
    input_sha256: String,
    bins_sha256: String,
}

impl EmbeddingSpatialEnvelope {
    fn new(
        analysis: EmbeddingSpatialDependenceSpec,
        requirements: Requirements,
        input_sha256: String,
        bins_sha256: String,
        curve: String,
        limits: EmbeddingSpatialDependenceEnvelopeLimits,
    ) -> Result<(Self, NodeContract), CliError> {
        let EmbeddingSpatialDependenceEnvelopeLimits {
            maximum_points,
            maximum_dimension,
            memory_budget_mib,
            ..
        } = limits;
        let alpha_bits = analysis.alpha.to_bits().to_string();
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-embedding-spatial-envelope-configuration-v1".as_slice(),
            curve.as_bytes(),
            alpha_bits.as_bytes(),
            analysis.permutations.to_string().as_bytes(),
            analysis.seed.to_string().as_bytes(),
            maximum_points.to_string().as_bytes(),
            maximum_dimension.to_string().as_bytes(),
            analysis.maximum_pair_visits.to_string().as_bytes(),
            memory_budget_mib.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "serial;complete-vector-within-stratum-random-labeling;physical-distance-bins;canonical-erl;curve={curve};permutations={};alpha_bits={alpha_bits};seed={};maximum_points={maximum_points};maximum_dimension={maximum_dimension};maximum_pair_visits={};memory_budget_mib={memory_budget_mib}",
            analysis.permutations, analysis.seed, analysis.maximum_pair_visits
        )
        .into_bytes();
        Ok((
            Self {
                analysis,
                requirements,
                input_sha256,
                bins_sha256,
            },
            NodeContract {
                id: "embedding-spatial-dependence-envelope",
                kind: "embedding_spatial_dependence_envelope",
                version: 1,
                configuration_digest,
                execution_policy: Cow::Owned(execution_policy),
                implementation_identity: IMPLEMENTATION_IDENTITY,
                output_kind: OUTPUT_KIND,
                codec: Codec::ExactFloatJson,
            },
        ))
    }

    fn validate(&self, output: &Output) -> io::Result<()> {
        if output.input_sha256 != self.input_sha256
            || output.bins_sha256 != self.bins_sha256
            || output.format != "marklab.embedding_spatial_dependence_envelope"
            || output.version != 1
            || output.coordinate_unit != "micrometer"
            || output.curve_function != curve_name(self.analysis.curve_function)
            || output.permutation_policy != "complete_embedding_rows_within_declared_strata"
            || output.feature_names != self.analysis.feature_names
            || output.object_count as usize != self.analysis.rows.len()
            || output.stratum_count as usize != self.requirements.stratum_count
            || output.permutations != self.analysis.permutations
            || output.alpha.to_bits() != self.analysis.alpha.to_bits()
            || output.seed != self.analysis.seed
            || output.pair_visits != self.requirements.pair_visits
            || output.pair_visits > self.analysis.maximum_pair_visits
            || self.requirements.component_operations > 250_000_000
            || output.curve.len() != self.analysis.bins.len()
            || !positive_unit_interval(output.p_global)
            || !positive_unit_interval(output.erl_depth)
            || !positive_unit_interval(output.critical_depth)
        {
            return Err(invalid("decoded embedding envelope identity differs"));
        }
        let scaled_p = output.p_global * (f64::from(output.permutations) + 1.0);
        if !scaled_p.is_finite()
            || scaled_p < 1.0 - 1e-9
            || (scaled_p - scaled_p.round()).abs() > 1e-9
        {
            return Err(invalid("decoded embedding envelope global p-value differs"));
        }
        for (index, ((row, bin), expected_count)) in output
            .curve
            .iter()
            .zip(&self.analysis.bins)
            .zip(&self.requirements.bin_counts)
            .enumerate()
        {
            let eligible = *expected_count > 0;
            if row.bin_id != bin.bin_id
                || row.lower_um.to_bits() != bin.lower_um.to_bits()
                || row.upper_um.to_bits() != bin.upper_um.to_bits()
                || row.upper_inclusive != (index + 1 == self.analysis.bins.len())
                || row.pair_count != *expected_count
                || row.inference_eligible != eligible
                || row.observed.is_some() != eligible
                || row.lower.is_some() != eligible
                || row.upper.is_some() != eligible
                || row
                    .observed
                    .is_some_and(|value| !value.is_finite() || value < 0.0)
                || row
                    .lower
                    .is_some_and(|value| !value.is_finite() || value < 0.0)
                || row
                    .upper
                    .is_some_and(|value| !value.is_finite() || value < 0.0)
                || matches!((row.lower, row.upper), (Some(lower), Some(upper)) if lower > upper)
            {
                return Err(invalid("decoded embedding envelope bin differs"));
            }
        }
        Ok(())
    }

    fn execute(&self) -> Result<Output, NodeError> {
        let result = test_embedding_spatial_dependence(self.analysis.clone())
            .map_err(NodeError::execution)?;
        Ok(Output::from_result(
            self.input_sha256.clone(),
            self.bins_sha256.clone(),
            result,
        ))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Output {
    input_sha256: String,
    bins_sha256: String,
    format: String,
    version: u32,
    coordinate_unit: String,
    curve_function: String,
    permutation_policy: String,
    feature_names: Vec<String>,
    object_count: u32,
    stratum_count: u32,
    permutations: u32,
    alpha: f64,
    seed: u64,
    pair_visits: u64,
    p_global: f64,
    erl_depth: f64,
    critical_depth: f64,
    curve: Vec<CurveRow>,
}

impl Output {
    fn from_result(
        input_sha256: String,
        bins_sha256: String,
        result: EmbeddingSpatialDependenceResult,
    ) -> Self {
        Self {
            input_sha256,
            bins_sha256,
            format: result.format.into(),
            version: result.version,
            coordinate_unit: result.coordinate_unit.into(),
            curve_function: curve_name(result.curve_function).into(),
            permutation_policy: result.permutation_policy.into(),
            feature_names: result.feature_names,
            object_count: result.object_count,
            stratum_count: result.stratum_count,
            permutations: result.permutations,
            alpha: result.alpha,
            seed: result.seed,
            pair_visits: result.pair_visits,
            p_global: result.p_global,
            erl_depth: result.erl_depth,
            critical_depth: result.critical_depth,
            curve: result
                .curve
                .into_iter()
                .map(|row| CurveRow {
                    bin_id: row.bin_id,
                    lower_um: row.lower_um,
                    upper_um: row.upper_um,
                    upper_inclusive: row.upper_inclusive,
                    pair_count: row.pair_count,
                    observed: row.observed,
                    lower: row.lower,
                    upper: row.upper,
                    inference_eligible: row.inference_eligible,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CurveRow {
    bin_id: String,
    lower_um: f64,
    upper_um: f64,
    upper_inclusive: bool,
    pair_count: u64,
    observed: Option<f64>,
    lower: Option<f64>,
    upper: Option<f64>,
    inference_eligible: bool,
}

fn curve_name(curve: EmbeddingSpatialCurveFunction) -> &'static str {
    match curve {
        EmbeddingSpatialCurveFunction::VectorSemivariogram => "vector_semivariogram",
    }
}

fn map_error(error: EmbeddingEnvelopeError) -> CliError {
    match error {
        EmbeddingEnvelopeError::Invalid(message) => CliError::Input(message),
        EmbeddingEnvelopeError::Numeric(message) => CliError::Backend(message),
    }
}

fn positive_unit_interval(value: f64) -> bool {
    value.is_finite() && value > 0.0 && value <= 1.0
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
