use std::collections::HashSet;

use serde::Serialize;
use thiserror::Error;

use crate::embedding_spatial::{FiniteNeumaierError, FiniteNeumaierSum};

#[derive(Clone, Debug)]
pub struct MultiscaleEmbeddingSummary {
    pub sample_id: String,
    pub scale_um: f64,
    pub embedding: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct MultiscaleKernelWeight {
    pub scale_um: f64,
    pub weight: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiscaleBaseKernel {
    Linear,
    Cosine,
    Rbf,
    Laplacian,
}

impl MultiscaleBaseKernel {
    pub fn parse(value: &str) -> Result<Self, MultiscaleKernelError> {
        match value {
            "linear" => Ok(Self::Linear),
            "cosine" => Ok(Self::Cosine),
            "rbf" => Ok(Self::Rbf),
            "laplacian" => Ok(Self::Laplacian),
            _ => Err(MultiscaleKernelError::Invalid(
                "base kernel must be linear, cosine, rbf, or laplacian".into(),
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct MultiscaleEmbeddingKernelSpec {
    pub summaries: Vec<MultiscaleEmbeddingSummary>,
    pub feature_names: Vec<String>,
    pub weights: Vec<MultiscaleKernelWeight>,
    pub sample_a: String,
    pub sample_b: String,
    pub base_kernel: MultiscaleBaseKernel,
    pub kernel_scale: Option<f64>,
    pub maximum_component_scale_visits: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MultiscaleKernelScaleResult {
    pub scale_um: f64,
    pub weight: f64,
    pub raw_kernel: f64,
    pub contribution: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MultiscaleKernelSensitivity {
    pub dropped_scale_um: f64,
    pub renormalized_total: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MultiscaleEmbeddingKernelResult {
    pub format: &'static str,
    pub version: u32,
    pub sample_a: String,
    pub sample_b: String,
    pub feature_names: Vec<String>,
    pub base_kernel: MultiscaleBaseKernel,
    pub kernel_scale: Option<f64>,
    pub scale_weight_policy: &'static str,
    pub total: f64,
    pub scales: Vec<MultiscaleKernelScaleResult>,
    pub drop_one_scale_sensitivity: Vec<MultiscaleKernelSensitivity>,
}

#[derive(Debug, Error)]
pub enum MultiscaleKernelError {
    #[error("invalid multiscale embedding kernel: {0}")]
    Invalid(String),
    #[error("multiscale embedding kernel exceeded its numeric range")]
    Numeric,
}

impl From<FiniteNeumaierError> for MultiscaleKernelError {
    fn from(_: FiniteNeumaierError) -> Self {
        Self::Numeric
    }
}

type StableSum = FiniteNeumaierSum;

pub fn multiscale_embedding_kernel(
    mut spec: MultiscaleEmbeddingKernelSpec,
) -> Result<MultiscaleEmbeddingKernelResult, MultiscaleKernelError> {
    if spec.sample_a.is_empty()
        || spec.sample_b.is_empty()
        || spec.sample_a == spec.sample_b
        || !(1..=128).contains(&spec.feature_names.len())
        || !(2..=64).contains(&spec.weights.len())
        || spec.maximum_component_scale_visits == 0
    {
        return Err(MultiscaleKernelError::Invalid(
            "dimensions or sample identities are invalid".into(),
        ));
    }
    let mut names = HashSet::new();
    if spec
        .feature_names
        .iter()
        .any(|name| !name.starts_with("embedding_") || !names.insert(name))
    {
        return Err(MultiscaleKernelError::Invalid(
            "feature names are invalid".into(),
        ));
    }
    if matches!(
        spec.base_kernel,
        MultiscaleBaseKernel::Rbf | MultiscaleBaseKernel::Laplacian
    ) {
        if spec
            .kernel_scale
            .is_none_or(|value| !value.is_finite() || value <= 0.0)
        {
            return Err(MultiscaleKernelError::Invalid(
                "RBF/Laplacian kernel scale is invalid".into(),
            ));
        }
    } else if spec.kernel_scale.is_some() {
        return Err(MultiscaleKernelError::Invalid(
            "linear/cosine kernels do not accept a scale".into(),
        ));
    }
    spec.weights
        .sort_by(|left, right| left.scale_um.total_cmp(&right.scale_um));
    if spec.weights.iter().any(|row| {
        !row.scale_um.is_finite()
            || row.scale_um <= 0.0
            || !row.weight.is_finite()
            || row.weight <= 0.0
    }) || spec
        .weights
        .windows(2)
        .any(|rows| rows[0].scale_um == rows[1].scale_um)
    {
        return Err(MultiscaleKernelError::Invalid(
            "scale weights are invalid".into(),
        ));
    }
    let weight_sum = spec.weights.iter().map(|row| row.weight).sum::<f64>();
    if (weight_sum - 1.0).abs() > 1e-12 {
        return Err(MultiscaleKernelError::Invalid(
            "scale weights must sum to one".into(),
        ));
    }
    let work = spec.weights.len() as u64 * spec.feature_names.len() as u64;
    if work > spec.maximum_component_scale_visits
        || spec.maximum_component_scale_visits > 250_000_000
    {
        return Err(MultiscaleKernelError::Invalid(
            "component-scale work exceeds its bound".into(),
        ));
    }
    spec.summaries.sort_by(|left, right| {
        left.sample_id
            .cmp(&right.sample_id)
            .then_with(|| left.scale_um.total_cmp(&right.scale_um))
    });
    if spec.summaries.len() != spec.weights.len() * 2 {
        return Err(MultiscaleKernelError::Invalid(
            "both samples require every weighted scale".into(),
        ));
    }
    let sample = |id: &str| {
        spec.summaries
            .iter()
            .filter(|row| row.sample_id == id)
            .collect::<Vec<_>>()
    };
    let a = sample(&spec.sample_a);
    let b = sample(&spec.sample_b);
    if a.len() != spec.weights.len() || b.len() != spec.weights.len() {
        return Err(MultiscaleKernelError::Invalid(
            "input contains unknown samples or missing scales".into(),
        ));
    }
    let mut scales = Vec::with_capacity(spec.weights.len());
    let mut total = StableSum::default();
    for (index, weight) in spec.weights.iter().enumerate() {
        if a[index].scale_um.to_bits() != weight.scale_um.to_bits()
            || b[index].scale_um.to_bits() != weight.scale_um.to_bits()
            || a[index].embedding.len() != spec.feature_names.len()
            || b[index].embedding.len() != spec.feature_names.len()
            || a[index]
                .embedding
                .iter()
                .chain(&b[index].embedding)
                .any(|value| !value.is_finite())
        {
            return Err(MultiscaleKernelError::Invalid(
                "sample scales or vectors differ".into(),
            ));
        }
        let raw_kernel = evaluate(
            &a[index].embedding,
            &b[index].embedding,
            spec.base_kernel,
            spec.kernel_scale,
        )?;
        let contribution = weight.weight * raw_kernel;
        total.add(contribution)?;
        scales.push(MultiscaleKernelScaleResult {
            scale_um: weight.scale_um,
            weight: weight.weight,
            raw_kernel,
            contribution,
        });
    }
    let total = total.total()?;
    let drop_one_scale_sensitivity = scales
        .iter()
        .map(|dropped| {
            let remaining_weight = 1.0 - dropped.weight;
            let renormalized_total = (total - dropped.contribution) / remaining_weight;
            MultiscaleKernelSensitivity {
                dropped_scale_um: dropped.scale_um,
                renormalized_total,
            }
        })
        .collect();
    Ok(MultiscaleEmbeddingKernelResult {
        format: "marklab.multiscale_embedding_kernel",
        version: 1,
        sample_a: spec.sample_a,
        sample_b: spec.sample_b,
        feature_names: spec.feature_names,
        base_kernel: spec.base_kernel,
        kernel_scale: spec.kernel_scale,
        scale_weight_policy: "caller_prespecified_positive_sum_one",
        total,
        scales,
        drop_one_scale_sensitivity,
    })
}

fn evaluate(
    left: &[f64],
    right: &[f64],
    kernel: MultiscaleBaseKernel,
    scale: Option<f64>,
) -> Result<f64, MultiscaleKernelError> {
    let mut dot = StableSum::default();
    let mut left_sq = StableSum::default();
    let mut right_sq = StableSum::default();
    let mut distance_sq = StableSum::default();
    let mut distance_l1 = StableSum::default();
    for (left, right) in left.iter().zip(right) {
        dot.add(left * right)?;
        left_sq.add(left * left)?;
        right_sq.add(right * right)?;
        distance_sq.add((left - right) * (left - right))?;
        distance_l1.add((left - right).abs())?;
    }
    let value = match kernel {
        MultiscaleBaseKernel::Linear => dot.total()?,
        MultiscaleBaseKernel::Cosine => {
            let denominator = (left_sq.total()? * right_sq.total()?).sqrt();
            if denominator <= 1e-14 {
                return Err(MultiscaleKernelError::Invalid(
                    "cosine summary has zero norm".into(),
                ));
            }
            dot.total()? / denominator
        }
        MultiscaleBaseKernel::Rbf => {
            let scale = scale.expect("validated RBF scale");
            (-distance_sq.total()? / (2.0 * scale * scale)).exp()
        }
        MultiscaleBaseKernel::Laplacian => {
            (-distance_l1.total()? / scale.expect("validated Laplacian scale")).exp()
        }
    };
    value
        .is_finite()
        .then_some(value)
        .ok_or(MultiscaleKernelError::Numeric)
}
