use marklab_workflow::ContentDigest;

use crate::geom::spatial_index::SpatialIndex2D;

use super::{GlobalMoranError, GlobalMoranWeightPolicy};

pub(super) struct RadiusWeights {
    pub(super) edges: Vec<(usize, usize)>,
    degrees: Vec<usize>,
    policy: GlobalMoranWeightPolicy,
    pub(super) digest: ContentDigest,
}

impl RadiusWeights {
    pub(super) fn build(
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

    pub(super) fn evaluate(&self, values: &[f64]) -> Result<f64, GlobalMoranError> {
        if values.windows(2).all(|pair| pair[0] == pair[1]) {
            return Err(GlobalMoranError::ZeroVariance);
        }
        let mean = compensated_sum(values.iter().copied()) / values.len() as f64;
        let centered = values.iter().map(|value| value - mean).collect::<Vec<_>>();
        let denominator = compensated_sum(centered.iter().map(|value| value * value));
        if !denominator.is_finite() || denominator <= 0.0 {
            return Err(GlobalMoranError::NumericalFailure);
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

    pub(super) fn evaluate_geary(&self, values: &[f64]) -> Result<f64, GlobalMoranError> {
        if values.windows(2).all(|pair| pair[0] == pair[1]) {
            return Err(GlobalMoranError::ZeroVariance);
        }
        let mean = compensated_sum(values.iter().copied()) / values.len() as f64;
        let denominator = compensated_sum(values.iter().map(|value| {
            let centered = value - mean;
            centered * centered
        }));
        if !denominator.is_finite() || denominator <= 0.0 {
            return Err(GlobalMoranError::NumericalFailure);
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
