use std::collections::{BTreeMap, BTreeSet};

use marklab_cohort::{InferenceAlternative, InferenceDesign};
use marklab_data::{CoordinateFrameId, MeasurementStatus};
use marklab_workflow::ContentDigest;
use thiserror::Error;

use crate::{
    geom::{spatial_index::SpatialIndex2D, window::ObservationWindow2D},
    scalar_mark::{DeclaredScalarPatternInput, ScalarMarkId},
};

/// Explicit adjacency normalization for global Moran's I.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GlobalMoranWeightPolicy {
    /// Every directed radius-neighbour edge has weight one.
    BinarySymmetric,
    /// Each outgoing radius-neighbour row sums to one.
    RowStandardized,
}

impl GlobalMoranWeightPolicy {
    fn wire_name(self) -> &'static str {
        match self {
            Self::BinarySymmetric => "binary_symmetric",
            Self::RowStandardized => "row_standardized",
        }
    }
}

/// Prespecified tail for the global Moran random-labeling test.
pub type GlobalMoranAlternative = InferenceAlternative;

/// Conditioning used by the admitted method-specific random-labeling design.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GlobalMoranConditioning {
    /// Permute complete scalar values across all rows.
    None,
    /// Permute only within the exact `histologic_compartment` row codes.
    HistologicCompartment,
}

/// Explicit random-labeling controls accepted by global Moran's I.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalMoranDesign {
    conditioning: GlobalMoranConditioning,
    permutations: usize,
    seed: u64,
    alternative: GlobalMoranAlternative,
}

impl GlobalMoranDesign {
    /// Declare unstratified whole-value random labeling.
    pub fn random_labeling(
        permutations: usize,
        seed: u64,
        alternative: GlobalMoranAlternative,
    ) -> Result<Self, GlobalMoranError> {
        Self::new(
            GlobalMoranConditioning::None,
            permutations,
            seed,
            alternative,
        )
    }

    /// Declare whole-value random labeling within exact histologic compartments.
    pub fn histologic_compartment_random_labeling(
        permutations: usize,
        seed: u64,
        alternative: GlobalMoranAlternative,
    ) -> Result<Self, GlobalMoranError> {
        Self::new(
            GlobalMoranConditioning::HistologicCompartment,
            permutations,
            seed,
            alternative,
        )
    }

    fn new(
        conditioning: GlobalMoranConditioning,
        permutations: usize,
        seed: u64,
        alternative: GlobalMoranAlternative,
    ) -> Result<Self, GlobalMoranError> {
        if permutations == 0 {
            return Err(GlobalMoranError::InvalidDesign(
                "global Moran inference requires at least one permutation".into(),
            ));
        }
        Ok(Self {
            conditioning,
            permutations,
            seed,
            alternative,
        })
    }

    /// Declared conditioning policy.
    pub fn conditioning(&self) -> GlobalMoranConditioning {
        self.conditioning
    }

    /// Requested deterministic permutation count.
    pub fn permutations(&self) -> usize {
        self.permutations
    }

    /// Base seed before method-domain separation.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Prespecified alternative.
    pub fn alternative(&self) -> GlobalMoranAlternative {
        self.alternative
    }
}

/// Explicit resource ceilings for one global Moran workflow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GlobalMoranLimits {
    /// Maximum point rows.
    pub maximum_points: usize,
    /// Maximum directed neighbour edges retained once.
    pub maximum_directed_edges: usize,
    /// Maximum directed-edge evaluations across all permutations.
    pub maximum_permutation_edge_evaluations: usize,
}

impl GlobalMoranLimits {
    /// Validate positive point, edge, and permutation-work limits.
    pub fn new(
        maximum_points: usize,
        maximum_directed_edges: usize,
        maximum_permutation_edge_evaluations: usize,
    ) -> Result<Self, GlobalMoranError> {
        if maximum_points == 0
            || maximum_directed_edges == 0
            || maximum_permutation_edge_evaluations == 0
        {
            return Err(GlobalMoranError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_points,
            maximum_directed_edges,
            maximum_permutation_edge_evaluations,
        })
    }
}

/// Complete typed result of one global Moran random-labeling workflow.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobalMoranResult {
    /// Stable continuous mark identity.
    pub mark_id: ScalarMarkId,
    /// Column-wide measured or predicted status.
    pub measurement_status: MeasurementStatus,
    /// Exact physical coordinate frame shared by points and window.
    pub coordinate_frame_id: CoordinateFrameId,
    /// Fixed neighbour radius in micrometres.
    pub radius_um: f64,
    /// Explicit adjacency normalization.
    pub weight_policy: GlobalMoranWeightPolicy,
    /// Content identity of the exact ordered directed edge plan and policy.
    pub weights_digest: ContentDigest,
    /// Number of point rows.
    pub point_count: usize,
    /// Number of retained directed edges.
    pub directed_edge_count: usize,
    /// Number of exchangeability strata.
    pub stratum_count: usize,
    /// Typed categorical mark used for conditioning, absent when unstratified.
    pub conditioning_mark_id: Option<ScalarMarkId>,
    /// Measurement status of the conditioning mark, absent when unstratified.
    pub conditioning_measurement_status: Option<MeasurementStatus>,
    /// Observed global Moran's I.
    pub statistic: f64,
    /// Random-labeling expectation `-1/(n-1)`.
    pub null_expectation: f64,
    /// Inclusive-plus-one permutation p-value.
    pub p_value: f64,
    /// Requested permutations.
    pub permutations_requested: usize,
    /// Attempted permutations.
    pub permutations_attempted: usize,
    /// Successfully completed permutations.
    pub permutations_completed: usize,
    /// Exact base seed.
    pub seed: u64,
    /// Prespecified alternative.
    pub alternative: GlobalMoranAlternative,
    /// Explicit random-labeling conditioning.
    pub conditioning: GlobalMoranConditioning,
}

/// Prespecified tail for the global Geary random-labeling test.
pub type GlobalGearyAlternative = GlobalMoranAlternative;

/// Explicit random-labeling controls accepted by global Geary's C.
pub type GlobalGearyDesign = GlobalMoranDesign;

/// Explicit resource ceilings for one global Geary workflow.
pub type GlobalGearyLimits = GlobalMoranLimits;

/// Complete typed result of one global Geary random-labeling workflow.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobalGearyResult {
    /// Stable continuous mark identity.
    pub mark_id: ScalarMarkId,
    /// Column-wide measured or predicted status.
    pub measurement_status: MeasurementStatus,
    /// Exact physical coordinate frame shared by points and window.
    pub coordinate_frame_id: CoordinateFrameId,
    /// Fixed neighbour radius in micrometres.
    pub radius_um: f64,
    /// Explicit adjacency normalization.
    pub weight_policy: GlobalMoranWeightPolicy,
    /// Content identity of the exact ordered directed edge plan and policy.
    pub weights_digest: ContentDigest,
    /// Number of point rows.
    pub point_count: usize,
    /// Number of retained directed edges.
    pub directed_edge_count: usize,
    /// Number of exchangeability strata.
    pub stratum_count: usize,
    /// Typed categorical mark used for conditioning, absent when unstratified.
    pub conditioning_mark_id: Option<ScalarMarkId>,
    /// Measurement status of the conditioning mark, absent when unstratified.
    pub conditioning_measurement_status: Option<MeasurementStatus>,
    /// Observed global Geary's C.
    pub statistic: f64,
    /// Random-labeling expectation, exactly one.
    pub null_expectation: f64,
    /// Inclusive-plus-one permutation p-value.
    pub p_value: f64,
    /// Requested permutations.
    pub permutations_requested: usize,
    /// Attempted permutations.
    pub permutations_attempted: usize,
    /// Successfully completed permutations.
    pub permutations_completed: usize,
    /// Exact base seed.
    pub seed: u64,
    /// Prespecified alternative.
    pub alternative: GlobalGearyAlternative,
    /// Explicit random-labeling conditioning.
    pub conditioning: GlobalMoranConditioning,
}

/// Compute global Moran's I and a deterministic whole-value random-labeling test.
#[allow(clippy::too_many_arguments)]
pub fn global_moran_permutation(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    radius_um: f64,
    weight_policy: GlobalMoranWeightPolicy,
    design: &GlobalMoranDesign,
    limits: GlobalMoranLimits,
) -> Result<GlobalMoranResult, GlobalMoranError> {
    if !radius_um.is_finite() || radius_um <= 0.0 {
        return Err(GlobalMoranError::InvalidRadius);
    }
    let frame = window
        .coordinate_frame_id()
        .ok_or(GlobalMoranError::UnboundObservationWindow)?;
    if frame != input.coordinate_frame_id() {
        return Err(GlobalMoranError::CoordinateFrameMismatch {
            expected: input.coordinate_frame_id().clone(),
            observed: frame.clone(),
        });
    }
    let pattern = input.pattern();
    let row_count = pattern.len();
    if row_count < 3 {
        return Err(GlobalMoranError::InsufficientPoints {
            observed: row_count,
        });
    }
    if row_count > limits.maximum_points {
        return Err(GlobalMoranError::PointLimitExceeded {
            observed: row_count,
            maximum: limits.maximum_points,
        });
    }
    for (row, (&x, &y)) in pattern.x_um.iter().zip(&pattern.y_um).enumerate() {
        if !window.contains(x, y) {
            return Err(GlobalMoranError::PointOutsideWindow { row });
        }
    }
    reject_duplicate_points(&pattern.x_um, &pattern.y_um)?;

    let table = input
        .mark_table()
        .ok_or(GlobalMoranError::TypedMarkTableRequired)?;
    let values = table.continuous_values(mark_id).ok_or_else(|| {
        GlobalMoranError::ContinuousMarkMissing {
            mark_id: mark_id.clone(),
        }
    })?;
    if values.len() != row_count {
        return Err(GlobalMoranError::RowCountMismatch {
            expected: row_count,
            observed: values.len(),
        });
    }
    let measurement_status = table.measurement_status(mark_id).ok_or_else(|| {
        GlobalMoranError::ContinuousMarkMissing {
            mark_id: mark_id.clone(),
        }
    })?;
    let values = values
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    let weights = RadiusWeights::build(
        &pattern.x_um,
        &pattern.y_um,
        radius_um,
        weight_policy,
        limits.maximum_directed_edges,
    )?;
    let work = weights
        .edges
        .len()
        .checked_mul(design.permutations)
        .ok_or(GlobalMoranError::SizeOverflow)?;
    if work > limits.maximum_permutation_edge_evaluations {
        return Err(GlobalMoranError::PermutationWorkExceeded {
            observed: work,
            maximum: limits.maximum_permutation_edge_evaluations,
        });
    }
    let (compartments, conditioning_mark_id, conditioning_measurement_status) =
        match design.conditioning {
            GlobalMoranConditioning::None => (None, None, None),
            GlobalMoranConditioning::HistologicCompartment => {
                let compartment_id = ScalarMarkId::new("histologic_compartment")
                    .map_err(|error| GlobalMoranError::InvalidDesign(error.to_string()))?;
                let compartments = table
                    .categorical_values(&compartment_id)
                    .ok_or(GlobalMoranError::MissingCompartmentStratum)?;
                let status = table
                    .measurement_status(&compartment_id)
                    .ok_or(GlobalMoranError::MissingCompartmentStratum)?;
                (Some(compartments), Some(compartment_id), Some(status))
            }
        };
    let inference_design = compile_inference_design(compartments, design, &values)?;
    let statistic = weights.evaluate(&values)?;
    let null_expectation = -1.0 / (row_count as f64 - 1.0);
    let mut extreme = 0_usize;
    for replicate in 0..design.permutations {
        let permuted = inference_design
            .permuted_indices(replicate)
            .map_err(|error| GlobalMoranError::InvalidDesign(error.to_string()))?
            .iter()
            .map(|source| values[*source])
            .collect::<Vec<_>>();
        let candidate = weights.evaluate(&permuted)?;
        extreme += usize::from(match design.alternative {
            GlobalMoranAlternative::Less => candidate <= statistic,
            GlobalMoranAlternative::Greater => candidate >= statistic,
            GlobalMoranAlternative::TwoSided => {
                (candidate - null_expectation).abs() >= (statistic - null_expectation).abs()
            }
        });
    }
    let p_value = (extreme as f64 + 1.0) / (design.permutations as f64 + 1.0);
    if !p_value.is_finite() || !(0.0..=1.0).contains(&p_value) {
        return Err(GlobalMoranError::NumericalFailure);
    }

    Ok(GlobalMoranResult {
        mark_id: mark_id.clone(),
        measurement_status,
        coordinate_frame_id: frame.clone(),
        radius_um,
        weight_policy,
        weights_digest: weights.digest,
        point_count: row_count,
        directed_edge_count: weights.edges.len(),
        stratum_count: inference_design.block_count(),
        conditioning_mark_id,
        conditioning_measurement_status,
        statistic,
        null_expectation,
        p_value,
        permutations_requested: design.permutations,
        permutations_attempted: design.permutations,
        permutations_completed: design.permutations,
        seed: design.seed,
        alternative: design.alternative,
        conditioning: design.conditioning,
    })
}

/// Compute global Geary's C and a deterministic whole-value random-labeling test.
#[allow(clippy::too_many_arguments)]
pub fn global_geary_permutation(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    radius_um: f64,
    weight_policy: GlobalMoranWeightPolicy,
    design: &GlobalGearyDesign,
    limits: GlobalGearyLimits,
) -> Result<GlobalGearyResult, GlobalGearyError> {
    if !radius_um.is_finite() || radius_um <= 0.0 {
        return Err(GlobalMoranError::InvalidRadius);
    }
    let frame = window
        .coordinate_frame_id()
        .ok_or(GlobalMoranError::UnboundObservationWindow)?;
    if frame != input.coordinate_frame_id() {
        return Err(GlobalMoranError::CoordinateFrameMismatch {
            expected: input.coordinate_frame_id().clone(),
            observed: frame.clone(),
        });
    }
    let pattern = input.pattern();
    let row_count = pattern.len();
    if row_count < 3 {
        return Err(GlobalMoranError::InsufficientPoints {
            observed: row_count,
        });
    }
    if row_count > limits.maximum_points {
        return Err(GlobalMoranError::PointLimitExceeded {
            observed: row_count,
            maximum: limits.maximum_points,
        });
    }
    for (row, (&x, &y)) in pattern.x_um.iter().zip(&pattern.y_um).enumerate() {
        if !window.contains(x, y) {
            return Err(GlobalMoranError::PointOutsideWindow { row });
        }
    }
    reject_duplicate_points(&pattern.x_um, &pattern.y_um)?;

    let table = input
        .mark_table()
        .ok_or(GlobalMoranError::TypedMarkTableRequired)?;
    let values = table.continuous_values(mark_id).ok_or_else(|| {
        GlobalMoranError::ContinuousMarkMissing {
            mark_id: mark_id.clone(),
        }
    })?;
    if values.len() != row_count {
        return Err(GlobalMoranError::RowCountMismatch {
            expected: row_count,
            observed: values.len(),
        });
    }
    let measurement_status = table.measurement_status(mark_id).ok_or_else(|| {
        GlobalMoranError::ContinuousMarkMissing {
            mark_id: mark_id.clone(),
        }
    })?;
    let values = values
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    let weights = RadiusWeights::build(
        &pattern.x_um,
        &pattern.y_um,
        radius_um,
        weight_policy,
        limits.maximum_directed_edges,
    )?;
    let work = weights
        .edges
        .len()
        .checked_mul(design.permutations)
        .ok_or(GlobalMoranError::SizeOverflow)?;
    if work > limits.maximum_permutation_edge_evaluations {
        return Err(GlobalMoranError::PermutationWorkExceeded {
            observed: work,
            maximum: limits.maximum_permutation_edge_evaluations,
        });
    }
    let (compartments, conditioning_mark_id, conditioning_measurement_status) =
        match design.conditioning {
            GlobalMoranConditioning::None => (None, None, None),
            GlobalMoranConditioning::HistologicCompartment => {
                let compartment_id = ScalarMarkId::new("histologic_compartment")
                    .map_err(|error| GlobalMoranError::InvalidDesign(error.to_string()))?;
                let compartments = table
                    .categorical_values(&compartment_id)
                    .ok_or(GlobalMoranError::MissingCompartmentStratum)?;
                let status = table
                    .measurement_status(&compartment_id)
                    .ok_or(GlobalMoranError::MissingCompartmentStratum)?;
                (Some(compartments), Some(compartment_id), Some(status))
            }
        };
    let inference_design = compile_inference_design(compartments, design, &values)?;
    let statistic = weights.evaluate_geary(&values)?;
    let null_expectation = 1.0;
    let mut extreme = 0_usize;
    for replicate in 0..design.permutations {
        let permuted = inference_design
            .permuted_indices(replicate)
            .map_err(|error| GlobalMoranError::InvalidDesign(error.to_string()))?
            .iter()
            .map(|source| values[*source])
            .collect::<Vec<_>>();
        let candidate = weights.evaluate_geary(&permuted)?;
        extreme += usize::from(match design.alternative {
            GlobalGearyAlternative::Less => candidate <= statistic,
            GlobalGearyAlternative::Greater => candidate >= statistic,
            GlobalGearyAlternative::TwoSided => {
                (candidate - null_expectation).abs() >= (statistic - null_expectation).abs()
            }
        });
    }
    let p_value = (extreme as f64 + 1.0) / (design.permutations as f64 + 1.0);
    if !p_value.is_finite() || !(0.0..=1.0).contains(&p_value) {
        return Err(GlobalMoranError::NumericalFailure);
    }

    Ok(GlobalGearyResult {
        mark_id: mark_id.clone(),
        measurement_status,
        coordinate_frame_id: frame.clone(),
        radius_um,
        weight_policy,
        weights_digest: weights.digest,
        point_count: row_count,
        directed_edge_count: weights.edges.len(),
        stratum_count: inference_design.block_count(),
        conditioning_mark_id,
        conditioning_measurement_status,
        statistic,
        null_expectation,
        p_value,
        permutations_requested: design.permutations,
        permutations_attempted: design.permutations,
        permutations_completed: design.permutations,
        seed: design.seed,
        alternative: design.alternative,
        conditioning: design.conditioning,
    })
}

struct RadiusWeights {
    edges: Vec<(usize, usize)>,
    degrees: Vec<usize>,
    policy: GlobalMoranWeightPolicy,
    digest: ContentDigest,
}

impl RadiusWeights {
    fn build(
        x: &[f64],
        y: &[f64],
        radius_um: f64,
        policy: GlobalMoranWeightPolicy,
        maximum_directed_edges: usize,
    ) -> Result<Self, GlobalMoranError> {
        let index = SpatialIndex2D::new(x, y)
            .map_err(|error| GlobalMoranError::Geometry(error.to_string()))?;
        let mut edges = Vec::new();
        let mut degrees = Vec::with_capacity(index.len());
        for row in 0..index.len() {
            let neighbors = index
                .within_radius(row, radius_um)
                .map_err(|error| GlobalMoranError::Geometry(error.to_string()))?;
            if neighbors.is_empty() {
                return Err(GlobalMoranError::IsolatedPoint { row });
            }
            degrees.push(neighbors.len());
            for neighbor in neighbors {
                if edges.len() == maximum_directed_edges {
                    return Err(GlobalMoranError::DirectedEdgeLimitExceeded {
                        maximum: maximum_directed_edges,
                    });
                }
                edges.push((row, neighbor.index));
            }
        }
        let mut digest_fields = Vec::<Vec<u8>>::with_capacity(edges.len() + 3);
        digest_fields.push(b"marklab-global-moran-radius-weights-v1".to_vec());
        digest_fields.push(radius_um.to_bits().to_be_bytes().to_vec());
        digest_fields.push(policy.wire_name().as_bytes().to_vec());
        for (left, right) in &edges {
            let mut edge = Vec::with_capacity(16);
            edge.extend_from_slice(&usize_to_u64(*left)?.to_be_bytes());
            edge.extend_from_slice(&usize_to_u64(*right)?.to_be_bytes());
            digest_fields.push(edge);
        }
        let digest = ContentDigest::from_framed(digest_fields.iter().map(Vec::as_slice));
        Ok(Self {
            edges,
            degrees,
            policy,
            digest,
        })
    }

    fn evaluate(&self, values: &[f64]) -> Result<f64, GlobalMoranError> {
        let mean = compensated_sum(values.iter().copied()) / values.len() as f64;
        let centered = values.iter().map(|value| value - mean).collect::<Vec<_>>();
        let denominator = compensated_sum(centered.iter().map(|value| value * value));
        if !denominator.is_finite() || denominator <= 0.0 {
            return Err(GlobalMoranError::ZeroVariance);
        }
        let numerator = compensated_sum(self.edges.iter().map(|(left, right)| {
            let weight = match self.policy {
                GlobalMoranWeightPolicy::BinarySymmetric => 1.0,
                GlobalMoranWeightPolicy::RowStandardized => 1.0 / self.degrees[*left] as f64,
            };
            weight * centered[*left] * centered[*right]
        }));
        let weight_sum = match self.policy {
            GlobalMoranWeightPolicy::BinarySymmetric => self.edges.len() as f64,
            GlobalMoranWeightPolicy::RowStandardized => values.len() as f64,
        };
        let statistic = values.len() as f64 * numerator / (weight_sum * denominator);
        if statistic.is_finite() {
            Ok(statistic)
        } else {
            Err(GlobalMoranError::NumericalFailure)
        }
    }

    fn evaluate_geary(&self, values: &[f64]) -> Result<f64, GlobalMoranError> {
        let mean = compensated_sum(values.iter().copied()) / values.len() as f64;
        let denominator = compensated_sum(values.iter().map(|value| {
            let centered = value - mean;
            centered * centered
        }));
        if !denominator.is_finite() || denominator <= 0.0 {
            return Err(GlobalMoranError::ZeroVariance);
        }
        let numerator = compensated_sum(self.edges.iter().map(|(left, right)| {
            let weight = match self.policy {
                GlobalMoranWeightPolicy::BinarySymmetric => 1.0,
                GlobalMoranWeightPolicy::RowStandardized => 1.0 / self.degrees[*left] as f64,
            };
            let difference = values[*left] - values[*right];
            weight * difference * difference
        }));
        let weight_sum = match self.policy {
            GlobalMoranWeightPolicy::BinarySymmetric => self.edges.len() as f64,
            GlobalMoranWeightPolicy::RowStandardized => values.len() as f64,
        };
        let statistic = (values.len() as f64 - 1.0) * numerator / (2.0 * weight_sum * denominator);
        if statistic.is_finite() {
            Ok(statistic)
        } else {
            Err(GlobalMoranError::NumericalFailure)
        }
    }
}

fn compile_inference_design(
    compartments: Option<&[u32]>,
    design: &GlobalMoranDesign,
    values: &[f64],
) -> Result<InferenceDesign, GlobalMoranError> {
    match design.conditioning {
        GlobalMoranConditioning::None => {
            if values
                .iter()
                .map(|value| value.to_bits())
                .collect::<BTreeSet<_>>()
                .len()
                < 2
            {
                return Err(GlobalMoranError::DegenerateNull);
            }
            InferenceDesign::random_labeling(
                values.len(),
                design.permutations,
                design.seed,
                design.alternative,
            )
            .map_err(|error| GlobalMoranError::InvalidDesign(error.to_string()))
        }
        GlobalMoranConditioning::HistologicCompartment => {
            let compartments = compartments.ok_or(GlobalMoranError::MissingCompartmentStratum)?;
            if compartments.len() != values.len() {
                return Err(GlobalMoranError::RowCountMismatch {
                    expected: values.len(),
                    observed: compartments.len(),
                });
            }
            let mut by_compartment = BTreeMap::<u32, Vec<usize>>::new();
            for (row, compartment) in compartments.iter().copied().enumerate() {
                by_compartment.entry(compartment).or_default().push(row);
            }
            if by_compartment.len() < 2 {
                return Err(GlobalMoranError::DegenerateNull);
            }
            let has_exchangeable_values = by_compartment.values().any(|block| {
                block.len() > 1
                    && block
                        .iter()
                        .map(|row| values[*row].to_bits())
                        .collect::<BTreeSet<_>>()
                        .len()
                        > 1
            });
            if !has_exchangeable_values {
                return Err(GlobalMoranError::DegenerateNull);
            }
            InferenceDesign::stratified_random_labeling(
                compartments,
                design.permutations,
                design.seed,
                design.alternative,
            )
            .map_err(|error| GlobalMoranError::InvalidDesign(error.to_string()))
        }
    }
}

fn reject_duplicate_points(x: &[f64], y: &[f64]) -> Result<(), GlobalMoranError> {
    let mut rows = BTreeMap::new();
    for (row, (&x, &y)) in x.iter().zip(y).enumerate() {
        let key = (canonical_bits(x), canonical_bits(y));
        if let Some(first_row) = rows.insert(key, row) {
            return Err(GlobalMoranError::DuplicatePoint {
                first_row,
                second_row: row,
            });
        }
    }
    Ok(())
}

fn canonical_bits(value: f64) -> u64 {
    if value == 0.0 {
        0.0_f64.to_bits()
    } else {
        value.to_bits()
    }
}

fn compensated_sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for value in values {
        let adjusted = value - correction;
        let next = sum + adjusted;
        correction = (next - sum) - adjusted;
        sum = next;
    }
    sum
}

fn usize_to_u64(value: usize) -> Result<u64, GlobalMoranError> {
    u64::try_from(value).map_err(|_| GlobalMoranError::SizeOverflow)
}

/// Invalid typed input, design, resource, geometry, or numerical state for global Moran's I.
pub type GlobalMoranError = GlobalSpatialAutocorrelationError;

/// Invalid typed input, design, resource, geometry, or numerical state for global Geary's C.
pub type GlobalGearyError = GlobalSpatialAutocorrelationError;

/// Shared failure states of the admitted global spatial-autocorrelation workflows.
#[derive(Debug, Error, PartialEq)]
pub enum GlobalSpatialAutocorrelationError {
    /// One or more resource ceilings are zero.
    #[error("global spatial-autocorrelation resource limits must be positive")]
    InvalidResourceLimit,
    /// Method-specific inference controls are invalid.
    #[error("invalid global spatial-autocorrelation design: {0}")]
    InvalidDesign(String),
    /// Neighbour radius is non-finite or non-positive.
    #[error("global spatial-autocorrelation neighbour radius must be finite and positive")]
    InvalidRadius,
    /// Exact observation window has no installed frame binding.
    #[error("global spatial autocorrelation requires a coordinate-frame-bound observation window")]
    UnboundObservationWindow,
    /// Point and window frames differ.
    #[error(
        "global spatial-autocorrelation frame mismatch: expected {expected}, observed {observed}"
    )]
    CoordinateFrameMismatch {
        /// Point/mark input frame.
        expected: CoordinateFrameId,
        /// Window frame.
        observed: CoordinateFrameId,
    },
    /// Fewer than three point rows were supplied.
    #[error("global spatial autocorrelation requires at least three points; observed {observed}")]
    InsufficientPoints {
        /// Observed row count.
        observed: usize,
    },
    /// Point rows exceed the explicit limit.
    #[error("global spatial autocorrelation has {observed} points; maximum is {maximum}")]
    PointLimitExceeded {
        /// Observed rows.
        observed: usize,
        /// Explicit maximum.
        maximum: usize,
    },
    /// A point is outside the exact observation window.
    #[error("global spatial-autocorrelation point row {row} lies outside the observation window")]
    PointOutsideWindow {
        /// Outside row.
        row: usize,
    },
    /// Two rows have the same physical coordinate and no duplicate policy was declared.
    #[error("global spatial-autocorrelation duplicate point rows {first_row} and {second_row}")]
    DuplicatePoint {
        /// First duplicate row.
        first_row: usize,
        /// Second duplicate row.
        second_row: usize,
    },
    /// Input was not constructed from the typed mark table adapter.
    #[error("global spatial autocorrelation requires a typed MarkTable input")]
    TypedMarkTableRequired,
    /// Requested continuous mark is absent.
    #[error("global spatial-autocorrelation continuous mark {mark_id:?} is absent")]
    ContinuousMarkMissing {
        /// Missing mark identity.
        mark_id: ScalarMarkId,
    },
    /// An aligned row vector has the wrong length.
    #[error("global spatial-autocorrelation row count mismatch: expected {expected}, observed {observed}")]
    RowCountMismatch {
        /// Required rows.
        expected: usize,
        /// Observed rows.
        observed: usize,
    },
    /// Spatial index or query construction failed.
    #[error("global spatial-autocorrelation geometry failed: {0}")]
    Geometry(String),
    /// At least one point has no neighbour under the declared radius.
    #[error(
        "global spatial-autocorrelation point row {row} is isolated under the declared weights"
    )]
    IsolatedPoint {
        /// Isolated row.
        row: usize,
    },
    /// Directed edge count exceeds the explicit limit.
    #[error("global spatial-autocorrelation directed-edge count exceeds {maximum}")]
    DirectedEdgeLimitExceeded {
        /// Explicit maximum.
        maximum: usize,
    },
    /// Permutation-by-edge work exceeds the explicit limit.
    #[error("global spatial-autocorrelation permutation work is {observed}; maximum is {maximum}")]
    PermutationWorkExceeded {
        /// Required directed-edge evaluations.
        observed: usize,
        /// Explicit maximum.
        maximum: usize,
    },
    /// Histologic-compartment conditioning was requested but not supplied.
    #[error("global spatial-autocorrelation histologic_compartment stratum is missing")]
    MissingCompartmentStratum,
    /// No declared stratum contains exchangeable distinct values.
    #[error("global spatial-autocorrelation random-labeling null is degenerate")]
    DegenerateNull,
    /// Continuous values have zero variance.
    #[error("global spatial-autocorrelation continuous mark has zero variance")]
    ZeroVariance,
    /// Checked count or work arithmetic overflowed.
    #[error("global spatial-autocorrelation size arithmetic overflow")]
    SizeOverflow,
    /// Finite inputs produced a non-finite statistic or p-value.
    #[error("global spatial autocorrelation produced a non-finite numerical result")]
    NumericalFailure,
}
