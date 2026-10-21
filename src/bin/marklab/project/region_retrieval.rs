use std::{borrow::Cow, collections::HashSet};

use super::is_sha256;
use marklab_prediction::{
    build_region_retrieval_index, retrieve_analogous_regions, QueryRegion, RegionRetrievalResult,
    RetrievalLeakagePolicy, TrainingRegion,
};
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;
use serde::{Deserialize, Serialize};

use super::cli_schema::RegionRetrievalProjectArgs;
use super::{
    bayes,
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_RESULT_BYTES,
};
use marklab_bayes::ClaimStatus;

const TRAINING_KIND: &str =
    "application/vnd.marklab.source.region-retrieval-training+csv;version=1";
const QUERY_KIND: &str = "application/vnd.marklab.source.region-retrieval-query+csv;version=1";
/// v2: results are stored with the shared exact-float codec.
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-region-retrieval-node-v2";

pub(super) fn run(args: RegionRetrievalProjectArgs) -> Result<(), CliError> {
    let RegionRetrievalProjectArgs {
        project: project_path,
        training: training_path,
        query: query_path,
        k,
        leakage_policy,
        maximum_component_candidate_visits,
        out: output_path,
    } = args;
    const LABEL: &str = "region retrieval sources";
    let sources = Source::identify_all([training_path, query_path], &[TRAINING_KIND, QUERY_KIND])?;
    let prepared = bayes::retrieval::prepare_inputs(sources[0].path(), sources[1].path())?;
    Source::ensure_all_unchanged(&sources, LABEL)?;
    if prepared.training_sha256 != sources[0].artifact().digest().to_string()
        || prepared.query_sha256 != sources[1].artifact().digest().to_string()
    {
        return Err(CliError::Input(
            "region retrieval parsed bytes differ from durable source identities".into(),
        ));
    }
    let policy = RetrievalLeakagePolicy::parse(&leakage_policy)
        .map_err(|error| CliError::Input(error.to_string()))?;
    let (retrieval, contract) =
        RegionRetrieval::new(prepared, k, policy, maximum_component_candidate_visits)?;
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        retrieval,
        RegionRetrieval::execute,
        |retrieval, document| {
            retrieval
                .validate_document(document)
                .map_err(|error| NodeError::decode(std::io::Error::other(error)))
        },
    )?;
    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.region_retrieval", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "region-retrieval",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

struct RegionRetrieval {
    training_sha256: String,
    query_sha256: String,
    training: Vec<TrainingRegion>,
    feature_names: Vec<String>,
    query: QueryRegion,
    k: u32,
    leakage_policy: RetrievalLeakagePolicy,
    maximum_component_candidate_visits: u64,
}

impl RegionRetrieval {
    fn new(
        prepared: bayes::retrieval::PreparedRegionRetrievalInputs,
        k: u32,
        leakage_policy: RetrievalLeakagePolicy,
        maximum_component_candidate_visits: u64,
    ) -> Result<(Self, NodeContract), CliError> {
        validate_prepared(
            &prepared.training,
            &prepared.feature_names,
            &prepared.query,
            k,
            leakage_policy,
            maximum_component_candidate_visits,
        )?;
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
        Ok((
            Self {
                training_sha256: prepared.training_sha256,
                query_sha256: prepared.query_sha256,
                training: prepared.training,
                feature_names: prepared.feature_names,
                query: prepared.query,
                k,
                leakage_policy,
                maximum_component_candidate_visits,
            },
            NodeContract {
                id: "region-retrieval",
                kind: "region_retrieval",
                version: 1,
                configuration_digest,
                execution_policy: Cow::Owned(execution_policy),
                implementation_identity: IMPLEMENTATION_IDENTITY,
                output_kind: "application/vnd.marklab.region-retrieval+json;version=2",
                codec: Codec::ExactFloatJson,
            },
        ))
    }

    fn execute(&self) -> Result<RegionRetrievalDocument, NodeError> {
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

    fn validate_document(&self, document: &RegionRetrievalDocument) -> Result<(), String> {
        if document.training_sha256 != self.training_sha256
            || document.query_sha256 != self.query_sha256
            || document.format != "marklab.region_retrieval"
            || document.version != 1
            || document.query_region_id != self.query.region_id
            || document.leakage_policy != policy_name(self.leakage_policy)
            || document.k != self.k
            || document.claim_status
                != ClaimStatus::AnalogousUnderFrozenMetricNotBiologicallyIdentical
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
        let (mean, standard_deviation) = standardization(&self.training)?;
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

fn validate_prepared(
    training: &[TrainingRegion],
    feature_names: &[String],
    query: &QueryRegion,
    k: u32,
    leakage_policy: RetrievalLeakagePolicy,
    maximum_visits: u64,
) -> Result<(), CliError> {
    if !(4..=100_000).contains(&training.len())
        || !(2..=128).contains(&feature_names.len())
        || k == 0
        || maximum_visits == 0
        || maximum_visits > 250_000_000
        || query.embedding.len() != feature_names.len()
        || query.embedding.iter().any(|value| !value.is_finite())
        || !is_sha256(&query.provenance_sha256)
    {
        return Err(CliError::Input(
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
        return Err(CliError::Input(
            "durable region retrieval inputs are invalid".into(),
        ));
    }
    let eligible_count = training
        .iter()
        .filter(|region| eligible(region, query, leakage_policy))
        .count();
    if eligible_count < k as usize {
        return Err(CliError::Input(
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

fn standardization(training: &[TrainingRegion]) -> Result<(Vec<f64>, Vec<f64>), String> {
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
    Ok((mean, standard_deviation))
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
    claim_status: ClaimStatus,
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
            claim_status: result.claim_status,
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

#[cfg(test)]
mod tests {
    use super::{derived_equal, within_one_ulp};

    #[test]
    fn decoded_transform_accepts_one_ulp_but_not_two() {
        let value = 0.118_797_598_885_150_87_f64;
        assert!(within_one_ulp(value, f64::from_bits(value.to_bits() - 1)));
        assert!(!within_one_ulp(value, f64::from_bits(value.to_bits() - 2)));
        assert!(derived_equal(value, f64::from_bits(value.to_bits() - 4)));
        assert!(!derived_equal(value, value + 1e-10));
    }
}
