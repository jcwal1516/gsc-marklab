use std::collections::BTreeMap;

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

    pub(super) fn conditional_expectations(
        &self,
        values: &[f64],
        strata: &[u32],
    ) -> Result<(f64, f64), GlobalMoranError> {
        if values.len() != self.degrees.len() || strata.len() != values.len() {
            return Err(GlobalMoranError::RowCountMismatch {
                expected: self.degrees.len(),
                observed: values.len().min(strata.len()),
            });
        }
        let mean = compensated_sum(values.iter().copied()) / values.len() as f64;
        let centered = values.iter().map(|value| value - mean).collect::<Vec<_>>();
        let denominator = compensated_sum(centered.iter().map(|value| value * value));
        if !denominator.is_finite() || denominator <= 0.0 {
            return Err(GlobalMoranError::NumericalFailure);
        }
        let mut moments = BTreeMap::<u32, BlockMoments>::new();
        for (&stratum, &value) in strata.iter().zip(&centered) {
            moments.entry(stratum).or_default().add(value)?;
        }
        let expected_moran_numerator = compensated_sum(self.edges.iter().map(|(left, right)| {
            let left_moments = &moments[&strata[*left]];
            let right_moments = &moments[&strata[*right]];
            let product = if strata[*left] == strata[*right] {
                left_moments.distinct_product_mean()
            } else {
                left_moments.mean() * right_moments.mean()
            };
            self.edge_weight(*left) * product
        }));
        let expected_geary_numerator = compensated_sum(self.edges.iter().map(|(left, right)| {
            let left_moments = &moments[&strata[*left]];
            let right_moments = &moments[&strata[*right]];
            let squared_difference = if strata[*left] == strata[*right] {
                left_moments.distinct_squared_difference_mean()
            } else {
                left_moments.mean_square() + right_moments.mean_square()
                    - 2.0 * left_moments.mean() * right_moments.mean()
            };
            self.edge_weight(*left) * squared_difference.max(0.0)
        }));
        let weight_sum = self.weight_sum(values.len());
        let moran = values.len() as f64 * expected_moran_numerator / (weight_sum * denominator);
        let geary = (values.len() as f64 - 1.0) * expected_geary_numerator
            / (2.0 * weight_sum * denominator);
        if moran.is_finite() && geary.is_finite() {
            Ok((moran, geary))
        } else {
            Err(GlobalMoranError::NumericalFailure)
        }
    }

    fn edge_weight(&self, source: usize) -> f64 {
        match self.policy {
            GlobalMoranWeightPolicy::BinarySymmetric => 1.0,
            GlobalMoranWeightPolicy::RowStandardized => 1.0 / self.degrees[source] as f64,
        }
    }

    fn weight_sum(&self, point_count: usize) -> f64 {
        match self.policy {
            GlobalMoranWeightPolicy::BinarySymmetric => self.edges.len() as f64,
            GlobalMoranWeightPolicy::RowStandardized => point_count as f64,
        }
    }
}

#[derive(Default)]
struct BlockMoments {
    count: usize,
    sum: f64,
    sum_correction: f64,
    square_sum: f64,
    square_correction: f64,
}

impl BlockMoments {
    fn add(&mut self, value: f64) -> Result<(), GlobalMoranError> {
        self.count = self
            .count
            .checked_add(1)
            .ok_or(GlobalMoranError::SizeOverflow)?;
        compensated_add(&mut self.sum, &mut self.sum_correction, value);
        compensated_add(
            &mut self.square_sum,
            &mut self.square_correction,
            value * value,
        );
        Ok(())
    }

    fn mean(&self) -> f64 {
        self.sum / self.count as f64
    }

    fn mean_square(&self) -> f64 {
        self.square_sum / self.count as f64
    }

    fn distinct_product_mean(&self) -> f64 {
        debug_assert!(self.count > 1);
        (self.sum * self.sum - self.square_sum) / (self.count * (self.count - 1)) as f64
    }

    fn distinct_squared_difference_mean(&self) -> f64 {
        debug_assert!(self.count > 1);
        2.0 * (self.square_sum - self.sum * self.sum / self.count as f64) / (self.count - 1) as f64
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

fn compensated_add(sum: &mut f64, correction: &mut f64, value: f64) {
    let adjusted = value - *correction;
    let next = *sum + adjusted;
    *correction = (next - *sum) - adjusted;
    *sum = next;
}

fn usize_to_u64(value: usize) -> Result<u64, GlobalMoranError> {
    u64::try_from(value).map_err(|_| GlobalMoranError::SizeOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditional_expectations_match_all_four_within_block_labelings() {
        let values = [1.0, 2.0, 8.0, 9.0];
        let strata = [0_u32, 0, 1, 1];
        let permutations = [[0, 1, 2, 3], [0, 1, 3, 2], [1, 0, 2, 3], [1, 0, 3, 2]];

        for policy in [
            GlobalMoranWeightPolicy::BinarySymmetric,
            GlobalMoranWeightPolicy::RowStandardized,
        ] {
            let weights = RadiusWeights::build(&[0.0, 1.0, 2.0, 3.0], &[0.0; 4], 1.1, policy, 6)
                .expect("line weights");
            let expected_moran = permutations
                .iter()
                .map(|order| {
                    weights
                        .evaluate(&order.map(|row| values[row]))
                        .expect("Moran statistic")
                })
                .sum::<f64>()
                / permutations.len() as f64;
            let expected_geary = permutations
                .iter()
                .map(|order| {
                    weights
                        .evaluate_geary(&order.map(|row| values[row]))
                        .expect("Geary statistic")
                })
                .sum::<f64>()
                / permutations.len() as f64;

            assert!(
                (weights
                    .conditional_expectations(&values, &strata)
                    .expect("conditional expectations")
                    .0
                    - expected_moran)
                    .abs()
                    <= 1.0e-14
            );
            assert!(
                (weights
                    .conditional_expectations(&values, &strata)
                    .expect("conditional expectations")
                    .1
                    - expected_geary)
                    .abs()
                    <= 1.0e-14
            );
        }
    }
}
