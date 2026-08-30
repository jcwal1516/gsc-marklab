use crate::validation::is_lower_hex_sha256 as sha;

use serde::Serialize;
use std::collections::HashSet;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct TrainingRegion {
    pub region_id: String,
    pub patient_id: String,
    pub site_id: String,
    pub split: String,
    pub domain: String,
    pub provenance_sha256: String,
    pub embedding: Vec<f64>,
}
#[derive(Clone, Debug)]
pub struct QueryRegion {
    pub region_id: String,
    pub patient_id: String,
    pub site_id: String,
    pub domain: String,
    pub provenance_sha256: String,
    pub embedding: Vec<f64>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalLeakagePolicy {
    ExcludeSamePatient,
    ExcludeSamePatientAndSite,
}
impl RetrievalLeakagePolicy {
    pub fn parse(value: &str) -> Result<Self, RegionRetrievalError> {
        match value {
            "exclude_same_patient" => Ok(Self::ExcludeSamePatient),
            "exclude_same_patient_and_site" => Ok(Self::ExcludeSamePatientAndSite),
            _ => Err(RegionRetrievalError::Invalid(
                "retrieval leakage policy is invalid".into(),
            )),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RegionRetrievalIndexArtifact {
    pub format: &'static str,
    pub version: u32,
    pub search: &'static str,
    pub approximation_recall_against_exact: f64,
    pub metric: &'static str,
    pub training_region_count: u32,
    pub feature_names: Vec<String>,
    pub training_mean: Vec<f64>,
    pub training_population_sd: Vec<f64>,
    pub domain: String,
    pub provenance_sha256: String,
    #[serde(skip)]
    regions: Vec<TrainingRegion>,
    #[serde(skip)]
    standardized: Vec<Vec<f64>>,
    #[serde(skip)]
    median_nearest: f64,
}
#[derive(Clone, Debug, Serialize)]
pub struct RegionRetrievalMatch {
    pub rank: u32,
    pub region_id: String,
    pub patient_id: String,
    pub site_id: String,
    pub distance: f64,
    pub component_squared_contributions: Vec<f64>,
}
#[derive(Clone, Debug, Serialize)]
pub struct RegionRetrievalResult {
    pub format: &'static str,
    pub version: u32,
    pub index: RegionRetrievalIndexArtifact,
    pub query_region_id: String,
    pub leakage_policy: RetrievalLeakagePolicy,
    pub k: u32,
    pub eligible_candidate_count: u32,
    pub ood_score: f64,
    pub matches: Vec<RegionRetrievalMatch>,
    pub claim_status: &'static str,
}
#[derive(Debug, Error)]
pub enum RegionRetrievalError {
    #[error("invalid region retrieval: {0}")]
    Invalid(String),
}

pub fn build_region_retrieval_index(
    mut regions: Vec<TrainingRegion>,
    feature_names: Vec<String>,
    maximum_visits: u64,
) -> Result<RegionRetrievalIndexArtifact, RegionRetrievalError> {
    if !(4..=100_000).contains(&regions.len()) || !(2..=128).contains(&feature_names.len()) {
        return Err(RegionRetrievalError::Invalid(
            "retrieval dimensions are invalid".into(),
        ));
    }
    let mut names = HashSet::new();
    if feature_names
        .iter()
        .any(|n| !n.starts_with("embedding_") || !names.insert(n))
    {
        return Err(RegionRetrievalError::Invalid(
            "retrieval feature names are invalid".into(),
        ));
    }
    regions.sort_by(|a, b| a.region_id.cmp(&b.region_id));
    let mut ids = HashSet::new();
    for r in &regions {
        if r.region_id.is_empty()
            || r.patient_id.is_empty()
            || r.site_id.is_empty()
            || r.split != "train"
            || !ids.insert(r.region_id.as_str())
            || r.embedding.len() != feature_names.len()
            || r.embedding.iter().any(|v| !v.is_finite())
            || !sha(&r.provenance_sha256)
        {
            return Err(RegionRetrievalError::Invalid(
                "training region is invalid".into(),
            ));
        }
    }
    let domain = regions[0].domain.clone();
    let provenance = regions[0].provenance_sha256.clone();
    if regions
        .iter()
        .any(|r| r.domain != domain || r.provenance_sha256 != provenance)
    {
        return Err(RegionRetrievalError::Invalid(
            "training domain/provenance is mixed".into(),
        ));
    }
    let n = regions.len();
    let d = feature_names.len();
    let work = (n as u64 * (n as u64 - 1) + n as u64) * d as u64;
    if work > maximum_visits || maximum_visits > 250_000_000 {
        return Err(RegionRetrievalError::Invalid(
            "retrieval work exceeds its bound".into(),
        ));
    }
    let mut mean = vec![0.; d];
    for r in &regions {
        for (s, v) in mean.iter_mut().zip(&r.embedding) {
            *s += v;
        }
    }
    for v in &mut mean {
        *v /= n as f64;
    }
    let mut sd = vec![0.; d];
    for r in &regions {
        for ((s, v), m) in sd.iter_mut().zip(&r.embedding).zip(&mean) {
            *s += (v - m).powi(2);
        }
    }
    for v in &mut sd {
        *v = (*v / n as f64).sqrt();
        if *v <= 1e-14 || !v.is_finite() {
            return Err(RegionRetrievalError::Invalid(
                "every retrieval feature must vary".into(),
            ));
        }
    }
    let standardized = regions
        .iter()
        .map(|r| {
            r.embedding
                .iter()
                .zip(&mean)
                .zip(&sd)
                .map(|((v, m), s)| (v - m) / s)
                .collect()
        })
        .collect::<Vec<Vec<f64>>>();
    let mut nearest = Vec::new();
    for i in 0..n {
        nearest.push(
            (0..n)
                .filter(|j| *j != i)
                .map(|j| dist(&standardized[i], &standardized[j]))
                .fold(f64::INFINITY, f64::min),
        );
    }
    nearest.sort_by(f64::total_cmp);
    let mid = n / 2;
    let median = if n.is_multiple_of(2) {
        nearest[mid - 1] + (nearest[mid] - nearest[mid - 1]) / 2.
    } else {
        nearest[mid]
    };
    if median <= 1e-14 {
        return Err(RegionRetrievalError::Invalid(
            "nearest-distance reference is degenerate".into(),
        ));
    }
    Ok(RegionRetrievalIndexArtifact {
        format: "marklab.region_retrieval_index",
        version: 1,
        search: "exact",
        approximation_recall_against_exact: 1.,
        metric: "training_standardized_euclidean",
        training_region_count: n as u32,
        feature_names,
        training_mean: mean,
        training_population_sd: sd,
        domain,
        provenance_sha256: provenance,
        regions,
        standardized,
        median_nearest: median,
    })
}
pub fn retrieve_analogous_regions(
    index: RegionRetrievalIndexArtifact,
    query: QueryRegion,
    k: u32,
    policy: RetrievalLeakagePolicy,
) -> Result<RegionRetrievalResult, RegionRetrievalError> {
    if k == 0
        || query.embedding.len() != index.feature_names.len()
        || query.embedding.iter().any(|v| !v.is_finite())
        || query.domain != index.domain
        || query.provenance_sha256 != index.provenance_sha256
    {
        return Err(RegionRetrievalError::Invalid(
            "query is incompatible".into(),
        ));
    }
    let q = query
        .embedding
        .iter()
        .zip(&index.training_mean)
        .zip(&index.training_population_sd)
        .map(|((v, m), s)| (v - m) / s)
        .collect::<Vec<_>>();
    let mut candidates = index
        .regions
        .iter()
        .zip(&index.standardized)
        .filter(|(r, _)| {
            r.patient_id != query.patient_id
                && (policy != RetrievalLeakagePolicy::ExcludeSamePatientAndSite
                    || r.site_id != query.site_id)
        })
        .map(|(r, v)| {
            let c = q
                .iter()
                .zip(v)
                .map(|(a, b)| (a - b).powi(2))
                .collect::<Vec<_>>();
            (r, c.iter().sum::<f64>().sqrt(), c)
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| {
        a.1.total_cmp(&b.1)
            .then_with(|| a.0.region_id.cmp(&b.0.region_id))
    });
    if candidates.len() < k as usize {
        return Err(RegionRetrievalError::Invalid(
            "too few eligible candidates".into(),
        ));
    }
    let eligible = candidates.len() as u32;
    let ood = candidates[0].1 / index.median_nearest;
    let matches = candidates
        .into_iter()
        .take(k as usize)
        .enumerate()
        .map(|(i, (r, d, c))| RegionRetrievalMatch {
            rank: i as u32 + 1,
            region_id: r.region_id.clone(),
            patient_id: r.patient_id.clone(),
            site_id: r.site_id.clone(),
            distance: d,
            component_squared_contributions: c,
        })
        .collect();
    Ok(RegionRetrievalResult {
        format: "marklab.region_retrieval",
        version: 1,
        index,
        query_region_id: query.region_id,
        leakage_policy: policy,
        k,
        eligible_candidate_count: eligible,
        ood_score: ood,
        matches,
        claim_status: "analogous_under_frozen_metric_not_biologically_identical",
    })
}
fn dist(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}
