use std::{collections::HashSet, path::PathBuf};

use marklab_bayes::{
    build_region_retrieval_index, retrieve_analogous_regions, QueryRegion, RegionRetrievalResult,
    RetrievalLeakagePolicy, TrainingRegion,
};
use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};
use serde::{Deserialize, Serialize};

use super::{
    bayes, native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES, PROJECT_LEDGER_RECORDS,
    PROJECT_RECORD_BYTES,
};

const TRAINING_KIND: &str =
    "application/vnd.marklab.source.region-retrieval-training+csv;version=1";
const QUERY_KIND: &str = "application/vnd.marklab.source.region-retrieval-query+csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.region-retrieval-codec+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-region-retrieval-node-v1";

pub(super) fn run(
    project_path: PathBuf,
    training_path: PathBuf,
    query_path: PathBuf,
    k: u32,
    leakage_policy: String,
    maximum_component_candidate_visits: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let training_before = source_artifact(&training_path, TRAINING_KIND)?;
    let query_before = source_artifact(&query_path, QUERY_KIND)?;
    let prepared = bayes::retrieval::prepare_inputs(&training_path, &query_path)?;
    let training_after = source_artifact(&training_path, TRAINING_KIND)?;
    let query_after = source_artifact(&query_path, QUERY_KIND)?;
    if training_before != training_after || query_before != query_after {
        return Err(BayesCliError::Input(
            "region retrieval source changed while the durable input was prepared".into(),
        ));
    }
    let policy = RetrievalLeakagePolicy::parse(&leakage_policy)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(training_before.clone())
        .and_then(|()| project.register_reference(query_before.clone()))
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = RegionRetrievalProjectNode::new(
        training_path,
        query_path,
        [training_before, query_before],
        prepared,
        k,
        policy,
        maximum_component_candidate_visits,
    )?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let execution = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.region_retrieval", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project region-retrieval cache_status={cache_status}");
    Ok(())
}

struct RegionRetrievalProjectNode {
    spec: NodeSpec,
    training_path: PathBuf,
    query_path: PathBuf,
    input_artifacts: [ArtifactRef; 2],
    training_sha256: String,
    query_sha256: String,
    training: Vec<TrainingRegion>,
    feature_names: Vec<String>,
    query: QueryRegion,
    k: u32,
    leakage_policy: RetrievalLeakagePolicy,
    maximum_component_candidate_visits: u64,
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
}

impl RegionRetrievalProjectNode {
    #[allow(clippy::too_many_arguments)]
    fn new(
        training_path: PathBuf,
        query_path: PathBuf,
        input_artifacts: [ArtifactRef; 2],
        prepared: bayes::retrieval::PreparedRegionRetrievalInputs,
        k: u32,
        leakage_policy: RetrievalLeakagePolicy,
        maximum_component_candidate_visits: u64,
    ) -> Result<Self, BayesCliError> {
        validate_prepared(
            &prepared.training,
            &prepared.feature_names,
            &prepared.query,
            k,
            leakage_policy,
            maximum_component_candidate_visits,
        )?;
        if prepared.training_sha256 != input_artifacts[0].digest().to_string()
            || prepared.query_sha256 != input_artifacts[1].digest().to_string()
        {
            return Err(BayesCliError::Input(
                "region retrieval parsed bytes differ from durable source identities".into(),
            ));
        }
        let policy_name = policy_name(leakage_policy);
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-region-retrieval-configuration-v1".as_slice(),
            k.to_string().as_bytes(),
            policy_name.as_bytes(),
            maximum_component_candidate_visits.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "training-standardized-exact-euclidean;leakage={policy_name};k={k};maximum_component_candidate_visits={maximum_component_candidate_visits}"
        )
        .into_bytes();
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("region-retrieval")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "region_retrieval",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            training_path,
            query_path,
            input_artifacts,
            training_sha256: prepared.training_sha256,
            query_sha256: prepared.query_sha256,
            training: prepared.training,
            feature_names: prepared.feature_names,
            query: prepared.query,
            k,
            leakage_policy,
            maximum_component_candidate_visits,
            configuration_digest,
            execution_policy,
        })
    }

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn validate_document(&self, document: &RegionRetrievalDocument) -> Result<(), String> {
        if document.training_sha256 != self.training_sha256
            || document.query_sha256 != self.query_sha256
            || document.format != "marklab.region_retrieval"
            || document.version != 1
            || document.query_region_id != self.query.region_id
            || document.leakage_policy != policy_name(self.leakage_policy)
            || document.k != self.k
            || document.claim_status != "analogous_under_frozen_metric_not_biologically_identical"
            || document.index.format != "marklab.region_retrieval_index"
            || document.index.version != 1
            || document.index.search != "exact"
            || document.index.approximation_recall_against_exact.to_bits() != 1.0_f64.to_bits()
            || document.index.metric != "training_standardized_euclidean"
            || document.index.training_region_count != self.training.len() as u32
            || document.index.feature_names != self.feature_names
            || document.index.domain != self.query.domain
            || document.index.provenance_sha256 != self.query.provenance_sha256
            || document.index.training_mean.len() != self.feature_names.len()
            || document.index.training_population_sd.len() != self.feature_names.len()
            || document.matches.len() != self.k as usize
            || !document.ood_score.is_finite()
            || document.ood_score < 0.0
        {
            return Err("decoded region retrieval identity or dimensions differ".into());
        }
        let (mean, standard_deviation, _) = standardize(&self.training)?;
        if let Some(error) =
            value_difference("mean", &document.index.training_mean, &mean).or_else(|| {
                value_difference(
                    "population_sd",
                    &document.index.training_population_sd,
                    &standard_deviation,
                )
            })
        {
            return Err(error);
        }
        let standardized = apply_standardization(
            &self.training,
            &document.index.training_mean,
            &document.index.training_population_sd,
        );
        let query = self
            .query
            .embedding
            .iter()
            .zip(&document.index.training_mean)
            .zip(&document.index.training_population_sd)
            .map(|((value, center), scale)| (value - center) / scale)
            .collect::<Vec<_>>();
        let mut candidates = self
            .training
            .iter()
            .zip(&standardized)
            .filter(|(region, _)| eligible(region, &self.query, self.leakage_policy))
            .map(|(region, values)| {
                let contributions = query
                    .iter()
                    .zip(values)
                    .map(|(left, right)| (left - right).powi(2))
                    .collect::<Vec<_>>();
                let distance = contributions.iter().sum::<f64>().sqrt();
                (region, distance, contributions)
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.0.region_id.cmp(&right.0.region_id))
        });
        if document.eligible_candidate_count != candidates.len() as u32 {
            return Err("decoded region retrieval eligible count differs".into());
        }
        for (index, (observed, expected)) in
            document.matches.iter().zip(candidates.iter()).enumerate()
        {
            if observed.rank != index as u32 + 1
                || observed.region_id != expected.0.region_id
                || observed.patient_id != expected.0.patient_id
                || observed.site_id != expected.0.site_id
            {
                return Err(format!(
                    "decoded region retrieval match identity differs at rank {}",
                    index + 1
                ));
            }
            if !derived_equal(observed.distance, expected.1) {
                return Err(format!(
                    "decoded region retrieval match distance differs at rank {}: observed={:.17e} expected={:.17e} ulps={}",
                    index + 1,
                    observed.distance,
                    expected.1,
                    observed.distance.to_bits().abs_diff(expected.1.to_bits())
                ));
            }
            if let Some(error) = derived_value_difference(
                &format!("match_rank_{}_contribution", index + 1),
                &observed.component_squared_contributions,
                &expected.2,
            ) {
                return Err(error);
            }
        }
        let median_nearest = median_nearest(&standardized)?;
        let expected_ood = candidates[0].1 / median_nearest;
        if !derived_equal(document.ood_score, expected_ood) {
            return Err("decoded region retrieval OOD score differs".into());
        }
        Ok(())
    }
}

impl WorkflowNode for RegionRetrievalProjectNode {
    type Output = RegionRetrievalDocument;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let training =
            source_artifact(&self.training_path, TRAINING_KIND).map_err(NodeError::input)?;
        let query = source_artifact(&self.query_path, QUERY_KIND).map_err(NodeError::input)?;
        self.input_artifacts[0]
            .verify_identity(training.digest(), training.byte_len())
            .and_then(|()| {
                self.input_artifacts[1].verify_identity(query.digest(), query.byte_len())
            })
            .map_err(NodeError::input)
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.configuration_digest,
            execution_policy: &self.execution_policy,
            implementation_identity: IMPLEMENTATION_IDENTITY,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        let index = build_region_retrieval_index(
            self.training.clone(),
            self.feature_names.clone(),
            self.maximum_component_candidate_visits,
        )
        .map_err(NodeError::execution)?;
        let result =
            retrieve_analogous_regions(index, self.query.clone(), self.k, self.leakage_policy)
                .map_err(NodeError::execution)?;
        let document = RegionRetrievalDocument::from_result(
            self.training_sha256.clone(),
            self.query_sha256.clone(),
            result,
        );
        self.validate_document(&document)
            .map_err(|error| NodeError::execution(std::io::Error::other(error)))?;
        Ok(document)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(&RegionRetrievalCodecDocument::from_document(output))
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let document = serde_json::from_slice::<RegionRetrievalCodecDocument>(bytes)
            .map_err(NodeError::decode)?
            .into_document();
        self.validate_document(&document)
            .map_err(|error| NodeError::decode(std::io::Error::other(error)))?;
        Ok(document)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}

fn validate_prepared(
    training: &[TrainingRegion],
    feature_names: &[String],
    query: &QueryRegion,
    k: u32,
    leakage_policy: RetrievalLeakagePolicy,
    maximum_visits: u64,
) -> Result<(), BayesCliError> {
    if !(4..=100_000).contains(&training.len())
        || !(2..=128).contains(&feature_names.len())
        || k == 0
        || maximum_visits == 0
        || maximum_visits > 250_000_000
        || query.embedding.len() != feature_names.len()
        || query.embedding.iter().any(|value| !value.is_finite())
        || !is_sha256(&query.provenance_sha256)
    {
        return Err(BayesCliError::Input(
            "durable region retrieval dimensions or controls are invalid".into(),
        ));
    }
    let mut names = HashSet::new();
    let mut ids = HashSet::new();
    if feature_names
        .iter()
        .any(|name| !name.starts_with("embedding_") || !names.insert(name))
        || training.iter().any(|region| {
            region.region_id.is_empty()
                || region.patient_id.is_empty()
                || region.site_id.is_empty()
                || region.split != "train"
                || region.domain != query.domain
                || region.provenance_sha256 != query.provenance_sha256
                || !ids.insert(region.region_id.as_str())
                || region.embedding.len() != feature_names.len()
                || region.embedding.iter().any(|value| !value.is_finite())
        })
    {
        return Err(BayesCliError::Input(
            "durable region retrieval inputs are invalid".into(),
        ));
    }
    let eligible_count = training
        .iter()
        .filter(|region| eligible(region, query, leakage_policy))
        .count();
    if eligible_count < k as usize {
        return Err(BayesCliError::Input(
            "durable region retrieval has too few eligible candidates".into(),
        ));
    }
    Ok(())
}

fn eligible(
    region: &TrainingRegion,
    query: &QueryRegion,
    leakage_policy: RetrievalLeakagePolicy,
) -> bool {
    region.patient_id != query.patient_id
        && (leakage_policy != RetrievalLeakagePolicy::ExcludeSamePatientAndSite
            || region.site_id != query.site_id)
}

fn standardize(training: &[TrainingRegion]) -> Result<(Vec<f64>, Vec<f64>, Vec<Vec<f64>>), String> {
    let dimensions = training[0].embedding.len();
    let mut mean = vec![0.0; dimensions];
    for region in training {
        for (sum, value) in mean.iter_mut().zip(&region.embedding) {
            *sum += value;
        }
    }
    for value in &mut mean {
        *value /= training.len() as f64;
    }
    let mut standard_deviation = vec![0.0; dimensions];
    for region in training {
        for ((sum, value), center) in standard_deviation
            .iter_mut()
            .zip(&region.embedding)
            .zip(&mean)
        {
            *sum += (value - center).powi(2);
        }
    }
    for value in &mut standard_deviation {
        *value = (*value / training.len() as f64).sqrt();
        if !value.is_finite() || *value <= 1e-14 {
            return Err("decoded region retrieval training scale is invalid".into());
        }
    }
    let standardized = apply_standardization(training, &mean, &standard_deviation);
    Ok((mean, standard_deviation, standardized))
}

fn apply_standardization(
    training: &[TrainingRegion],
    mean: &[f64],
    standard_deviation: &[f64],
) -> Vec<Vec<f64>> {
    training
        .iter()
        .map(|region| {
            region
                .embedding
                .iter()
                .zip(mean)
                .zip(standard_deviation)
                .map(|((value, center), scale)| (value - center) / scale)
                .collect()
        })
        .collect()
}

fn median_nearest(standardized: &[Vec<f64>]) -> Result<f64, String> {
    let mut nearest = Vec::with_capacity(standardized.len());
    for left in 0..standardized.len() {
        nearest.push(
            (0..standardized.len())
                .filter(|right| *right != left)
                .map(|right| euclidean(&standardized[left], &standardized[right]))
                .fold(f64::INFINITY, f64::min),
        );
    }
    nearest.sort_by(f64::total_cmp);
    let middle = nearest.len() / 2;
    let median = if nearest.len().is_multiple_of(2) {
        nearest[middle - 1] + (nearest[middle] - nearest[middle - 1]) / 2.0
    } else {
        nearest[middle]
    };
    if !median.is_finite() || median <= 1e-14 {
        return Err("decoded region retrieval nearest-distance reference is invalid".into());
    }
    Ok(median)
}

fn euclidean(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| (left - right).powi(2))
        .sum::<f64>()
        .sqrt()
}

fn value_difference(name: &str, observed: &[f64], expected: &[f64]) -> Option<String> {
    if observed.len() != expected.len() {
        return Some(format!(
            "decoded region retrieval {name} length differs: observed={} expected={}",
            observed.len(),
            expected.len()
        ));
    }
    observed
        .iter()
        .zip(expected)
        .enumerate()
        .find(|(_, (left, right))| !within_one_ulp(**left, **right))
        .map(|(index, (left, right))| {
            format!(
                "decoded region retrieval {name}[{index}] differs: observed={left:.17e} expected={right:.17e} observed_bits={:016x} expected_bits={:016x}",
                left.to_bits(),
                right.to_bits()
            )
        })
}

fn derived_value_difference(name: &str, observed: &[f64], expected: &[f64]) -> Option<String> {
    if observed.len() != expected.len() {
        return Some(format!(
            "decoded region retrieval {name} length differs: observed={} expected={}",
            observed.len(),
            expected.len()
        ));
    }
    observed
        .iter()
        .zip(expected)
        .enumerate()
        .find(|(_, (left, right))| !derived_equal(**left, **right))
        .map(|(index, (left, right))| {
            format!(
                "decoded region retrieval {name}[{index}] differs: observed={left:.17e} expected={right:.17e}"
            )
        })
}

fn within_one_ulp(left: f64, right: f64) -> bool {
    left == right
        || (left.is_finite()
            && right.is_finite()
            && left.is_sign_negative() == right.is_sign_negative()
            && left.to_bits().abs_diff(right.to_bits()) <= 1)
}

fn derived_equal(left: f64, right: f64) -> bool {
    left.is_finite()
        && right.is_finite()
        && (left - right).abs() <= 64.0 * f64::EPSILON * left.abs().max(right.abs()).max(1.0)
}

fn policy_name(policy: RetrievalLeakagePolicy) -> &'static str {
    match policy {
        RetrievalLeakagePolicy::ExcludeSamePatient => "exclude_same_patient",
        RetrievalLeakagePolicy::ExcludeSamePatientAndSite => "exclude_same_patient_and_site",
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RegionRetrievalDocument {
    training_sha256: String,
    query_sha256: String,
    format: String,
    version: u32,
    index: RegionRetrievalIndexDocument,
    query_region_id: String,
    leakage_policy: String,
    k: u32,
    eligible_candidate_count: u32,
    ood_score: f64,
    matches: Vec<RegionRetrievalMatchDocument>,
    claim_status: String,
}

impl RegionRetrievalDocument {
    fn from_result(
        training_sha256: String,
        query_sha256: String,
        result: RegionRetrievalResult,
    ) -> Self {
        let index = result.index;
        Self {
            training_sha256,
            query_sha256,
            format: result.format.into(),
            version: result.version,
            index: RegionRetrievalIndexDocument {
                format: index.format.into(),
                version: index.version,
                search: index.search.into(),
                approximation_recall_against_exact: index.approximation_recall_against_exact,
                metric: index.metric.into(),
                training_region_count: index.training_region_count,
                feature_names: index.feature_names,
                training_mean: index.training_mean,
                training_population_sd: index.training_population_sd,
                domain: index.domain,
                provenance_sha256: index.provenance_sha256,
            },
            query_region_id: result.query_region_id,
            leakage_policy: policy_name(result.leakage_policy).into(),
            k: result.k,
            eligible_candidate_count: result.eligible_candidate_count,
            ood_score: result.ood_score,
            matches: result
                .matches
                .into_iter()
                .map(|matched| RegionRetrievalMatchDocument {
                    rank: matched.rank,
                    region_id: matched.region_id,
                    patient_id: matched.patient_id,
                    site_id: matched.site_id,
                    distance: matched.distance,
                    component_squared_contributions: matched.component_squared_contributions,
                })
                .collect(),
            claim_status: result.claim_status.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RegionRetrievalIndexDocument {
    format: String,
    version: u32,
    search: String,
    approximation_recall_against_exact: f64,
    metric: String,
    training_region_count: u32,
    feature_names: Vec<String>,
    training_mean: Vec<f64>,
    training_population_sd: Vec<f64>,
    domain: String,
    provenance_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RegionRetrievalMatchDocument {
    rank: u32,
    region_id: String,
    patient_id: String,
    site_id: String,
    distance: f64,
    component_squared_contributions: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RegionRetrievalCodecDocument {
    training_sha256: String,
    query_sha256: String,
    format: String,
    version: u32,
    index: RegionRetrievalIndexCodecDocument,
    query_region_id: String,
    leakage_policy: String,
    k: u32,
    eligible_candidate_count: u32,
    ood_score_bits: u64,
    matches: Vec<RegionRetrievalMatchCodecDocument>,
    claim_status: String,
}

impl RegionRetrievalCodecDocument {
    fn from_document(document: &RegionRetrievalDocument) -> Self {
        Self {
            training_sha256: document.training_sha256.clone(),
            query_sha256: document.query_sha256.clone(),
            format: document.format.clone(),
            version: document.version,
            index: RegionRetrievalIndexCodecDocument {
                format: document.index.format.clone(),
                version: document.index.version,
                search: document.index.search.clone(),
                approximation_recall_against_exact_bits: document
                    .index
                    .approximation_recall_against_exact
                    .to_bits(),
                metric: document.index.metric.clone(),
                training_region_count: document.index.training_region_count,
                feature_names: document.index.feature_names.clone(),
                training_mean_bits: bits(&document.index.training_mean),
                training_population_sd_bits: bits(&document.index.training_population_sd),
                domain: document.index.domain.clone(),
                provenance_sha256: document.index.provenance_sha256.clone(),
            },
            query_region_id: document.query_region_id.clone(),
            leakage_policy: document.leakage_policy.clone(),
            k: document.k,
            eligible_candidate_count: document.eligible_candidate_count,
            ood_score_bits: document.ood_score.to_bits(),
            matches: document
                .matches
                .iter()
                .map(|matched| RegionRetrievalMatchCodecDocument {
                    rank: matched.rank,
                    region_id: matched.region_id.clone(),
                    patient_id: matched.patient_id.clone(),
                    site_id: matched.site_id.clone(),
                    distance_bits: matched.distance.to_bits(),
                    component_squared_contribution_bits: bits(
                        &matched.component_squared_contributions,
                    ),
                })
                .collect(),
            claim_status: document.claim_status.clone(),
        }
    }

    fn into_document(self) -> RegionRetrievalDocument {
        RegionRetrievalDocument {
            training_sha256: self.training_sha256,
            query_sha256: self.query_sha256,
            format: self.format,
            version: self.version,
            index: RegionRetrievalIndexDocument {
                format: self.index.format,
                version: self.index.version,
                search: self.index.search,
                approximation_recall_against_exact: f64::from_bits(
                    self.index.approximation_recall_against_exact_bits,
                ),
                metric: self.index.metric,
                training_region_count: self.index.training_region_count,
                feature_names: self.index.feature_names,
                training_mean: values(self.index.training_mean_bits),
                training_population_sd: values(self.index.training_population_sd_bits),
                domain: self.index.domain,
                provenance_sha256: self.index.provenance_sha256,
            },
            query_region_id: self.query_region_id,
            leakage_policy: self.leakage_policy,
            k: self.k,
            eligible_candidate_count: self.eligible_candidate_count,
            ood_score: f64::from_bits(self.ood_score_bits),
            matches: self
                .matches
                .into_iter()
                .map(|matched| RegionRetrievalMatchDocument {
                    rank: matched.rank,
                    region_id: matched.region_id,
                    patient_id: matched.patient_id,
                    site_id: matched.site_id,
                    distance: f64::from_bits(matched.distance_bits),
                    component_squared_contributions: values(
                        matched.component_squared_contribution_bits,
                    ),
                })
                .collect(),
            claim_status: self.claim_status,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RegionRetrievalIndexCodecDocument {
    format: String,
    version: u32,
    search: String,
    approximation_recall_against_exact_bits: u64,
    metric: String,
    training_region_count: u32,
    feature_names: Vec<String>,
    training_mean_bits: Vec<u64>,
    training_population_sd_bits: Vec<u64>,
    domain: String,
    provenance_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RegionRetrievalMatchCodecDocument {
    rank: u32,
    region_id: String,
    patient_id: String,
    site_id: String,
    distance_bits: u64,
    component_squared_contribution_bits: Vec<u64>,
}

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|value| value.to_bits()).collect()
}

fn values(bits: Vec<u64>) -> Vec<f64> {
    bits.into_iter().map(f64::from_bits).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        derived_equal, within_one_ulp, RegionRetrievalCodecDocument, RegionRetrievalDocument,
        RegionRetrievalIndexDocument, RegionRetrievalMatchDocument,
    };

    #[test]
    fn decoded_transform_accepts_one_ulp_but_not_two() {
        let value = 0.118_797_598_885_150_87_f64;
        assert!(within_one_ulp(value, f64::from_bits(value.to_bits() - 1)));
        assert!(!within_one_ulp(value, f64::from_bits(value.to_bits() - 2)));
        assert!(derived_equal(value, f64::from_bits(value.to_bits() - 4)));
        assert!(!derived_equal(value, value + 1e-10));
    }

    #[test]
    fn float_bit_codec_is_byte_canonical_for_real_scale_values() {
        let contribution = f64::from_bits(0x3f47_2acf_b95b_728c);
        let document = RegionRetrievalDocument {
            training_sha256: "a".repeat(64),
            query_sha256: "b".repeat(64),
            format: "marklab.region_retrieval".into(),
            version: 1,
            index: RegionRetrievalIndexDocument {
                format: "marklab.region_retrieval_index".into(),
                version: 1,
                search: "exact".into(),
                approximation_recall_against_exact: 1.0,
                metric: "training_standardized_euclidean".into(),
                training_region_count: 4,
                feature_names: vec!["embedding_0".into(), "embedding_1".into()],
                training_mean: vec![0.118_797_598_885_150_87, -0.25],
                training_population_sd: vec![0.5, 1.5],
                domain: "tumor".into(),
                provenance_sha256: "c".repeat(64),
            },
            query_region_id: "q".into(),
            leakage_policy: "exclude_same_patient".into(),
            k: 1,
            eligible_candidate_count: 3,
            ood_score: 0.75,
            matches: vec![RegionRetrievalMatchDocument {
                rank: 1,
                region_id: "r".into(),
                patient_id: "p".into(),
                site_id: "s".into(),
                distance: contribution.sqrt(),
                component_squared_contributions: vec![contribution, 0.0],
            }],
            claim_status: "analogous_under_frozen_metric_not_biologically_identical".into(),
        };
        let first =
            serde_json::to_vec(&RegionRetrievalCodecDocument::from_document(&document)).unwrap();
        let decoded = serde_json::from_slice::<RegionRetrievalCodecDocument>(&first)
            .unwrap()
            .into_document();
        let second =
            serde_json::to_vec(&RegionRetrievalCodecDocument::from_document(&decoded)).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            decoded.matches[0].component_squared_contributions[0].to_bits(),
            contribution.to_bits()
        );
    }
}
