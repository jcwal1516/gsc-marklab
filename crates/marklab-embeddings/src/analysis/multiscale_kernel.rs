use std::collections::HashSet;

use serde::Serialize;
use thiserror::Error;

use marklab_numerics::summation::{FiniteNeumaierError, FiniteNeumaierSum};

/// Embedding summary for one sample at one spatial scale.
#[derive(Clone, Debug)]
pub struct MultiscaleEmbeddingSummary {
    /// Stable sample identity.
    pub sample_id: String,
    /// Spatial summary scale, in micrometres.
    pub scale_um: f64,
    /// Finite embedding vector in declared feature order.
    pub embedding: Vec<f64>,
}

/// Prespecified nonnegative weight of one spatial scale.
#[derive(Clone, Debug)]
pub struct MultiscaleKernelWeight {
    /// Spatial summary scale, in micrometres.
    pub scale_um: f64,
    /// Nonnegative contribution weight of this spatial scale.
    pub weight: f64,
}

/// Similarity kernel evaluated separately at each spatial scale.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiscaleBaseKernel {
    /// Ordinary vector inner product.
    Linear,
    /// Cosine of the angle between the input vectors.
    Cosine,
    /// Gaussian radial-basis kernel with the declared bandwidth.
    Rbf,
    /// Exponential L1-distance kernel with the declared bandwidth.
    Laplacian,
}

impl MultiscaleBaseKernel {
    /// Parse a documented kernel wire spelling.
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

/// Two sample identities, their multiscale summaries, and fixed kernel weights.
#[derive(Clone, Debug)]
pub struct MultiscaleEmbeddingKernelSpec {
    /// Embedding summaries for the compared samples and declared scales.
    pub summaries: Vec<MultiscaleEmbeddingSummary>,
    /// Feature names in vector order.
    pub feature_names: Vec<String>,
    /// Prespecified contribution weights for each spatial scale.
    pub weights: Vec<MultiscaleKernelWeight>,
    /// Identity of the first compared sample.
    pub sample_a: String,
    /// Identity of the second compared sample.
    pub sample_b: String,
    /// Similarity kernel applied at each scale.
    pub base_kernel: MultiscaleBaseKernel,
    /// Fixed positive kernel bandwidth when required by the kernel family.
    pub kernel_scale: Option<f64>,
    /// Maximum number of feature-component and scale evaluations.
    pub maximum_component_scale_visits: u64,
}

/// Raw kernel value and weighted contribution of one spatial scale.
#[derive(Clone, Debug, Serialize)]
pub struct MultiscaleKernelScaleResult {
    /// Spatial summary scale, in micrometres.
    pub scale_um: f64,
    /// Nonnegative contribution weight of this spatial scale.
    pub weight: f64,
    /// Unweighted similarity at this scale.
    pub raw_kernel: f64,
    /// Similarity multiplied by the normalized scale weight.
    pub contribution: f64,
}

/// Total similarity after dropping one scale and renormalizing the remaining weights.
#[derive(Clone, Debug, Serialize)]
pub struct MultiscaleKernelSensitivity {
    /// Spatial scale omitted in this sensitivity calculation.
    pub dropped_scale_um: f64,
    /// Weighted total after renormalizing the retained scale weights.
    pub renormalized_total: f64,
}

/// Weighted multiscale similarity and drop-one-scale sensitivity.
#[derive(Clone, Debug, Serialize)]
pub struct MultiscaleEmbeddingKernelResult {
    /// Versioned document-format identifier.
    pub format: &'static str,
    /// Version of the recorded document format.
    pub version: u32,
    /// Identity of the first compared sample.
    pub sample_a: String,
    /// Identity of the second compared sample.
    pub sample_b: String,
    /// Feature names in vector order.
    pub feature_names: Vec<String>,
    /// Similarity kernel applied at each scale.
    pub base_kernel: MultiscaleBaseKernel,
    /// Fixed positive kernel bandwidth when required by the kernel family.
    pub kernel_scale: Option<f64>,
    /// Recorded normalization convention for the declared scale weights.
    pub scale_weight_policy: &'static str,
    /// Weighted total kernel similarity.
    pub total: f64,
    /// Kernel values and contributions in scale order.
    pub scales: Vec<MultiscaleKernelScaleResult>,
    /// Sensitivity of the total to omitting each scale in turn.
    pub drop_one_scale_sensitivity: Vec<MultiscaleKernelSensitivity>,
}

/// Invalid multiscale kernel input or non-finite arithmetic.
#[derive(Debug, Error)]
pub enum MultiscaleKernelError {
    /// The supplied specification or embedding data is invalid.
    #[error("invalid multiscale embedding kernel: {0}")]
    Invalid(String),
    /// A numerical contribution or accumulation is non-finite.
    #[error("multiscale embedding kernel exceeded its numeric range")]
    Numeric,
}

impl From<FiniteNeumaierError> for MultiscaleKernelError {
    fn from(_: FiniteNeumaierError) -> Self {
        Self::Numeric
    }
}

type StableSum = FiniteNeumaierSum;

/// Compare two multiscale embeddings with fixed scale weights and bounded numerical work.
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
        .enumerate()
        .map(|(dropped_index, dropped)| {
            let mut remaining_weight = StableSum::default();
            let mut remaining_contribution = StableSum::default();
            for (index, retained) in scales.iter().enumerate() {
                if index != dropped_index {
                    remaining_weight.add(retained.weight)?;
                    remaining_contribution.add(retained.contribution)?;
                }
            }
            let renormalized_total = remaining_contribution.total()? / remaining_weight.total()?;
            if !renormalized_total.is_finite() {
                return Err(MultiscaleKernelError::Numeric);
            }
            Ok(MultiscaleKernelSensitivity {
                dropped_scale_um: dropped.scale_um,
                renormalized_total,
            })
        })
        .collect::<Result<Vec<_>, MultiscaleKernelError>>()?;
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
    // Scale each vector independently so cosine remains invariant to magnitude and
    // neither squaring components nor multiplying squared norms can overflow.
    let cosine_scales = if kernel == MultiscaleBaseKernel::Cosine {
        let left_scale = left.iter().map(|value| value.abs()).fold(0.0_f64, f64::max);
        let right_scale = right
            .iter()
            .map(|value| value.abs())
            .fold(0.0_f64, f64::max);
        if left_scale == 0.0 || right_scale == 0.0 {
            return Err(MultiscaleKernelError::Invalid(
                "cosine summary has zero norm".into(),
            ));
        }
        Some((left_scale, right_scale))
    } else {
        None
    };
    let mut dot = StableSum::default();
    let mut left_sq = StableSum::default();
    let mut right_sq = StableSum::default();
    let mut distance_sq = StableSum::default();
    let mut distance_l1 = StableSum::default();
    for (left, right) in left.iter().zip(right) {
        let (left, right) = if let Some((left_scale, right_scale)) = cosine_scales {
            (left / left_scale, right / right_scale)
        } else {
            (*left, *right)
        };
        dot.add(left * right)?;
        left_sq.add(left * left)?;
        right_sq.add(right * right)?;
        distance_sq.add((left - right) * (left - right))?;
        distance_l1.add((left - right).abs())?;
    }
    let value = match kernel {
        MultiscaleBaseKernel::Linear => dot.total()?,
        MultiscaleBaseKernel::Cosine => {
            let denominator = left_sq.total()?.sqrt() * right_sq.total()?.sqrt();
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
