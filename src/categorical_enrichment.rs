//! Fixed-count categorical edge enrichment on a declared tissue radius graph.

use serde::{Deserialize, Serialize};

use crate::{
    categorical_input::{
        prepare_categorical_input, CategoricalCellView, CategoricalChannelView,
        CategoricalInputLimits, PreparedCategoricalInput,
    },
    common::seeds::{derive_seed, SeedEndpoint},
    errors::{MarklabError, Result},
    inference::multiple_testing::benjamini_hochberg,
    neighborhood::enrichment::{enrichment_result, LabelPair},
    permutation::labels::deterministic_shuffle,
    EnrichmentStatisticUnavailableReason, NeighborhoodEnrichmentResult,
};

const MAXIMUM_INPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    format: String,
    version: u32,
    coordinate_frame_id: String,
    coordinate_unit: String,
    window: serde_json::Value,
    channel: Channel,
    cells: Vec<Cell>,
    radius_um: f64,
    permutations: usize,
    seed: u64,
    limits: Limits,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Channel {
    id: String,
    label: String,
    kind: String,
    unit: String,
    measurement_status: String,
    provenance: String,
    classes: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cell {
    id: String,
    x_um: f64,
    y_um: f64,
    class_id: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    maximum_input_bytes: usize,
    maximum_points: usize,
    maximum_classes: usize,
    maximum_edges: usize,
    maximum_permutations: usize,
    maximum_permutation_work: usize,
    maximum_retained_bytes: usize,
}

/// Exact finite-label null moments alongside the separately estimated Monte Carlo result.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CategoricalEnrichmentPairResult {
    pub analytic_null_mean: f64,
    pub analytic_null_variance: f64,
    pub analytic_z_score: Option<f64>,
    pub analytic_z_score_unavailable_reason: Option<EnrichmentStatisticUnavailableReason>,
    pub monte_carlo: NeighborhoodEnrichmentResult,
}

/// Version-1 single-stratum categorical neighborhood enrichment result.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CategoricalEnrichmentResult {
    pub format: String,
    pub version: u32,
    pub coordinate_frame_id: String,
    pub channel_id: String,
    pub classes: Vec<String>,
    pub class_counts: Vec<usize>,
    pub point_count: usize,
    pub radius_um: f64,
    pub undirected_edge_count: usize,
    pub adjacent_edge_pair_count: u64,
    pub disjoint_edge_pair_count: u64,
    pub permutations: usize,
    pub seed: u64,
    pub pairs: Vec<CategoricalEnrichmentPairResult>,
}

/// Analyze a strict version-1 categorical-enrichment JSON request.
///
/// The input has an unconditional 16 MiB cap and narrower caller-declared input,
/// memory and work limits. It supplies physical coordinates and a fixed-count
/// categorical channel; no label, coordinate or patient group is inferred.
pub fn categorical_enrichment_from_json(input: &[u8]) -> Result<CategoricalEnrichmentResult> {
    if input.len() > MAXIMUM_INPUT_BYTES {
        return Err(invalid("categorical enrichment input exceeds 16 MiB"));
    }
    let request: Request = serde_json::from_slice(input)?;
    let limits = request.limits;
    if request.format != "marklab.categorical_enrichment"
        || request.version != 1
        || !request.radius_um.is_finite()
        || request.radius_um <= 0.0
        || !(request.radius_um * request.radius_um).is_finite()
        || request.permutations == 0
        || request.permutations > limits.maximum_permutations
        || limits.maximum_input_bytes == 0
        || limits.maximum_input_bytes > MAXIMUM_INPUT_BYTES
        || input.len() > limits.maximum_input_bytes
        || limits.maximum_edges == 0
        || limits.maximum_permutation_work == 0
    {
        return Err(invalid(
            "categorical enrichment format, radius or limits are invalid",
        ));
    }
    // Typed rows and the temporary generic GeoJSON tree remain alive during
    // preparation. This deliberately conservative bound includes their storage
    // and the window serialization, before the index or algorithm buffers exist.
    let decoded_request_bytes = input
        .len()
        .checked_mul(64)
        .ok_or_else(arithmetic_overflow)?;
    if decoded_request_bytes > limits.maximum_retained_bytes {
        return Err(invalid(
            "categorical enrichment decoded request exceeds the memory budget",
        ));
    }
    let window = serde_json::to_string(&request.window)?;
    let channel = &request.channel;
    let prepared = prepare_categorical_input(
        &request.coordinate_frame_id,
        &request.coordinate_unit,
        &window,
        CategoricalChannelView {
            id: &channel.id,
            label: &channel.label,
            kind: &channel.kind,
            unit: &channel.unit,
            measurement_status: &channel.measurement_status,
            provenance: &channel.provenance,
            classes: &channel.classes,
        },
        request.cells.iter().map(|cell| CategoricalCellView {
            id: &cell.id,
            x_um: cell.x_um,
            y_um: cell.y_um,
            class_id: &cell.class_id,
        }),
        CategoricalInputLimits {
            maximum_points: limits.maximum_points,
            maximum_classes: limits.maximum_classes,
            maximum_retained_bytes: limits.maximum_retained_bytes,
        },
        input.len(),
        decoded_request_bytes,
    )
    .map_err(|error| invalid(error.to_string()))?;
    let (radius, permutations, seed) = (request.radius_um, request.permutations, request.seed);
    drop(request);
    drop(window);
    analyze(prepared, radius, permutations, seed, limits)
}

fn analyze(
    prepared: PreparedCategoricalInput,
    radius_um: f64,
    permutations: usize,
    seed: u64,
    limits: Limits,
) -> Result<CategoricalEnrichmentResult> {
    let n = prepared.class_indices.len();
    let k = prepared.classes.len();
    let pair_count = k
        .checked_add(1)
        .and_then(|next| k.checked_mul(next))
        .map(|twice| twice / 2)
        .ok_or_else(arithmetic_overflow)?;
    let possible_edges = n
        .checked_mul(n - 1)
        .map(|twice| twice / 2)
        .ok_or_else(arithmetic_overflow)?;
    let edge_capacity = limits.maximum_edges.min(possible_edges);
    let null_count = pair_count
        .checked_mul(permutations)
        .ok_or_else(arithmetic_overflow)?;
    let matrix_size = k.checked_mul(k).ok_or_else(arithmetic_overflow)?;
    let class_text = prepared.classes.iter().try_fold(0_usize, |total, label| {
        total
            .checked_add(label.len())
            .ok_or_else(arithmetic_overflow)
    })?;
    let estimated_bytes = [
        prepared.input_file_bytes,
        prepared.estimated_storage_bytes,
        bytes_for::<(usize, usize)>(edge_capacity)?,
        bytes_for::<usize>(n.checked_mul(2).ok_or_else(arithmetic_overflow)?)?,
        bytes_for::<usize>(matrix_size)?,
        bytes_for::<usize>(null_count)?,
        bytes_for::<usize>(pair_count.checked_mul(2).ok_or_else(arithmetic_overflow)?)?,
        bytes_for::<f64>(permutations)?,
        bytes_for::<CategoricalEnrichmentPairResult>(pair_count)?,
        // Every class name occurs k+1 times across the output family; an
        // additional temporary LabelPair is live while one result is assembled.
        class_text
            .checked_mul(k.checked_add(3).ok_or_else(arithmetic_overflow)?)
            .ok_or_else(arithmetic_overflow)?,
        // BH sorts a p-value vector and retains its adjusted output.
        bytes_for::<(usize, f64)>(pair_count.checked_mul(3).ok_or_else(arithmetic_overflow)?)?,
    ]
    .into_iter()
    .try_fold(0_usize, |total, bytes| {
        total.checked_add(bytes).ok_or_else(arithmetic_overflow)
    })?;
    if estimated_bytes > limits.maximum_retained_bytes {
        return Err(invalid(format!(
            "categorical enrichment retained bytes {estimated_bytes} exceed the memory budget"
        )));
    }
    let mut edges = Vec::new();
    edges
        .try_reserve_exact(edge_capacity)
        .map_err(|_| invalid("categorical edge allocation failed"))?;
    let mut degrees = vec![0_usize; n];
    for source in 0..n {
        let mut exceeded = false;
        prepared
            .index
            .visit_within_radius(source, radius_um, |neighbor| {
                if neighbor.index <= source || exceeded {
                    return;
                }
                if edges.len() == edge_capacity {
                    exceeded = true;
                    return;
                }
                edges.push((source, neighbor.index));
                degrees[source] += 1;
                degrees[neighbor.index] += 1;
            })?;
        if exceeded {
            return Err(invalid("categorical graph exceeds maximum_edges"));
        }
    }
    let work = n
        .checked_add(edges.len())
        .and_then(|permutation| permutation.checked_mul(permutations))
        .ok_or_else(arithmetic_overflow)?;
    if work > limits.maximum_permutation_work {
        return Err(invalid(format!(
            "categorical permutation work {work} exceeds its maximum"
        )));
    }
    let adjacent = degrees.iter().try_fold(0_u128, |total, degree| {
        add(total, falling(*degree as u128, 2)? / 2)
    })?;
    let disjoint = (falling(edges.len() as u128, 2)? / 2)
        .checked_sub(adjacent)
        .ok_or_else(|| invalid("categorical radius graph has inconsistent edge pairs"))?;
    drop(degrees);

    let mut pair_indices = vec![0_usize; matrix_size];
    let mut row = 0;
    for a in 0..k {
        for b in a..k {
            pair_indices[a * k + b] = row;
            pair_indices[b * k + a] = row;
            row += 1;
        }
    }
    let mut observed = vec![0_usize; pair_count];
    count_edges(
        &prepared.class_indices,
        &edges,
        &pair_indices,
        k,
        &mut observed,
    );
    let mut null_counts = Vec::new();
    null_counts
        .try_reserve_exact(null_count)
        .map_err(|_| invalid("categorical null-count allocation failed"))?;
    null_counts.resize(null_count, 0_usize);
    let mut shuffled = prepared.class_indices.to_vec();
    let mut counts = vec![0_usize; pair_count];
    for permutation in 0..permutations {
        shuffled.copy_from_slice(&prepared.class_indices);
        deterministic_shuffle(
            &mut shuffled,
            derive_seed(seed, SeedEndpoint::CategoricalEnrichment, permutation),
        );
        count_edges(&shuffled, &edges, &pair_indices, k, &mut counts);
        for (pair, count) in counts.iter().enumerate() {
            null_counts[pair * permutations + permutation] = *count;
        }
    }
    let mut pairs = Vec::with_capacity(pair_count);
    for a in 0..k {
        for b in a..k {
            let pair_index = pair_indices[a * k + b];
            let (mean, variance) = exact_null_moments(
                n,
                prepared.class_counts[a],
                prepared.class_counts[b],
                a == b,
                edges.len(),
                adjacent,
                disjoint,
            )?;
            let (z_score, unavailable) = if variance > 0.0 {
                (
                    Some((observed[pair_index] as f64 - mean) / variance.sqrt()),
                    None,
                )
            } else {
                (
                    None,
                    Some(EnrichmentStatisticUnavailableReason::ZeroNullVariance),
                )
            };
            if z_score.is_some_and(|value| !value.is_finite()) {
                return Err(MarklabError::Compute(
                    "categorical analytic z-score is nonfinite".into(),
                ));
            }
            let labels = LabelPair::new(&prepared.classes[a], &prepared.classes[b]);
            let start = pair_index * permutations;
            pairs.push(CategoricalEnrichmentPairResult {
                analytic_null_mean: mean,
                analytic_null_variance: variance,
                analytic_z_score: z_score,
                analytic_z_score_unavailable_reason: unavailable,
                monte_carlo: enrichment_result(
                    &labels,
                    observed[pair_index],
                    &null_counts[start..start + permutations],
                )?,
            });
        }
    }
    let p_values = pairs
        .iter()
        .map(|pair| {
            pair.monte_carlo
                .p_value
                .expect("enrichment computes a p-value")
        })
        .collect::<Vec<_>>();
    for (pair, q_value) in pairs.iter_mut().zip(benjamini_hochberg(&p_values)?) {
        pair.monte_carlo.q_value = Some(q_value);
    }
    Ok(CategoricalEnrichmentResult {
        format: "marklab.categorical_enrichment_result".into(),
        version: 1,
        coordinate_frame_id: prepared.coordinate_frame_id,
        channel_id: prepared.channel_id,
        classes: prepared.classes.into_vec(),
        class_counts: prepared.class_counts.into_vec(),
        point_count: n,
        radius_um,
        undirected_edge_count: edges.len(),
        adjacent_edge_pair_count: u64::try_from(adjacent).map_err(|_| arithmetic_overflow())?,
        disjoint_edge_pair_count: u64::try_from(disjoint).map_err(|_| arithmetic_overflow())?,
        permutations,
        seed,
        pairs,
    })
}

fn count_edges(
    labels: &[usize],
    edges: &[(usize, usize)],
    lookup: &[usize],
    classes: usize,
    counts: &mut [usize],
) {
    counts.fill(0);
    for &(a, b) in edges {
        counts[lookup[labels[a] * classes + labels[b]]] += 1;
    }
}

fn bytes_for<T>(count: usize) -> Result<usize> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(arithmetic_overflow)
}

fn invalid(message: impl Into<String>) -> MarklabError {
    MarklabError::Validation(message.into())
}

fn arithmetic_overflow() -> MarklabError {
    MarklabError::Validation("categorical enrichment exact null arithmetic overflowed".into())
}

fn multiply(left: u128, right: u128) -> Result<u128> {
    left.checked_mul(right).ok_or_else(arithmetic_overflow)
}

fn add(left: u128, right: u128) -> Result<u128> {
    left.checked_add(right).ok_or_else(arithmetic_overflow)
}

fn falling(count: u128, order: u128) -> Result<u128> {
    if count < order {
        return Ok(0);
    }
    (0..order).try_fold(1, |product, offset| multiply(product, count - offset))
}

fn exact_null_moments(
    point_count: usize,
    count_a: usize,
    count_b: usize,
    same_class: bool,
    edges: usize,
    adjacent: u128,
    disjoint: u128,
) -> Result<(f64, f64)> {
    if edges == 0 {
        return Ok((0.0, 0.0));
    }
    let n = point_count as u128;
    let a = count_a as u128;
    let b = count_b as u128;
    let n2 = falling(n, 2)?;
    if n2 == 0 {
        return Err(MarklabError::Compute(
            "a categorical graph edge requires two distinct cells".into(),
        ));
    }
    let p = if same_class {
        falling(a, 2)?
    } else {
        multiply(2, multiply(a, b)?)?
    };
    let mean_numerator = Wide::product(&[edges as u128, p])?;
    let mean = mean_numerator.to_f64() / n2 as f64;
    if n == 2 {
        return Ok((mean, 0.0));
    }
    let q = if same_class {
        falling(a, 3)?
    } else {
        add(multiply(a, falling(b, 2)?)?, multiply(b, falling(a, 2)?)?)?
    };
    // Use one common denominator and subtract integer numerators before any f64
    // conversion. Complete graphs must retain their exact zero variance. The products
    // exceed u128 from about 60,000 cells, so they use 256-bit integers; inputs within
    // the 16 MiB cap stay far below that range.
    let (denominator_factor, second_moment_numerator) = if n == 3 {
        (
            n - 2,
            mean_numerator
                .mul(Wide::from(n - 2))?
                .add(Wide::product(&[2, adjacent, q])?)?,
        )
    } else {
        let r = if same_class {
            falling(a, 4)?
        } else {
            multiply(4, multiply(falling(a, 2)?, falling(b, 2)?)?)?
        };
        let factor = multiply(n - 2, n - 3)?;
        (
            factor,
            mean_numerator
                .mul(Wide::from(factor))?
                .add(Wide::product(&[2, adjacent, q, n - 3])?)?
                .add(Wide::product(&[2, disjoint, r])?)?,
        )
    };
    let variance_numerator = second_moment_numerator
        .mul(Wide::from(n2))?
        .checked_sub(
            mean_numerator
                .mul(mean_numerator)?
                .mul(Wide::from(denominator_factor))?,
        )
        .ok_or_else(|| {
            MarklabError::Compute("categorical graph null variance is negative".into())
        })?;
    let variance_denominator = Wide::product(&[n2, n2, denominator_factor])?;
    let variance = variance_numerator.to_f64() / variance_denominator.to_f64();
    Ok((mean, variance))
}

/// Unsigned 256-bit integer for the exact moment numerators, as little-endian limbs.
#[derive(Clone, Copy)]
struct Wide([u64; 4]);

impl From<u128> for Wide {
    fn from(value: u128) -> Self {
        Self([value as u64, (value >> 64) as u64, 0, 0])
    }
}

impl Wide {
    fn product(factors: &[u128]) -> Result<Self> {
        factors.iter().try_fold(Self::from(1), |product, &factor| {
            product.mul(Self::from(factor))
        })
    }

    fn add(self, other: Self) -> Result<Self> {
        let mut limbs = [0; 4];
        let mut carry = 0_u128;
        for (index, limb) in limbs.iter_mut().enumerate() {
            let sum = u128::from(self.0[index]) + u128::from(other.0[index]) + carry;
            *limb = sum as u64;
            carry = sum >> 64;
        }
        if carry != 0 {
            return Err(arithmetic_overflow());
        }
        Ok(Self(limbs))
    }

    fn checked_sub(self, other: Self) -> Option<Self> {
        let mut limbs = [0; 4];
        let mut borrow = false;
        for (index, limb) in limbs.iter_mut().enumerate() {
            let (difference, first) = self.0[index].overflowing_sub(other.0[index]);
            let (difference, second) = difference.overflowing_sub(u64::from(borrow));
            *limb = difference;
            borrow = first || second;
        }
        (!borrow).then_some(Self(limbs))
    }

    fn mul(self, other: Self) -> Result<Self> {
        let mut limbs = [0_u64; 8];
        for (left_index, &left) in self.0.iter().enumerate() {
            let mut carry = 0_u128;
            for (right_index, &right) in other.0.iter().enumerate() {
                let slot = &mut limbs[left_index + right_index];
                let value = u128::from(left) * u128::from(right) + u128::from(*slot) + carry;
                *slot = value as u64;
                carry = value >> 64;
            }
            limbs[left_index + 4] = carry as u64;
        }
        if limbs[4..].iter().any(|&limb| limb != 0) {
            return Err(arithmetic_overflow());
        }
        Ok(Self([limbs[0], limbs[1], limbs[2], limbs[3]]))
    }

    /// Correctly rounded conversion.
    fn to_f64(self) -> f64 {
        let low = u128::from(self.0[0]) | (u128::from(self.0[1]) << 64);
        let high = u128::from(self.0[2]) | (u128::from(self.0[3]) << 64);
        if high == 0 {
            return low as f64;
        }
        // Keep the top 128 bits and fold every discarded bit into a sticky low bit, so
        // the u128 conversion rounds as a direct 256-bit conversion would.
        let shift = 128 - high.leading_zeros();
        let (kept, discarded) = if shift == 128 {
            (high, low)
        } else {
            (
                (high << (128 - shift)) | (low >> shift),
                low & ((1_u128 << shift) - 1),
            )
        };
        let rounded = (kept | u128::from(discarded != 0)) as f64;
        rounded * f64::from_bits((1023 + u64::from(shift)) << 52)
    }
}

#[cfg(test)]
mod tests {
    use super::exact_null_moments;

    fn assert_close(observed: f64, expected: f64) {
        let error = (observed - expected).abs() / expected.abs();
        assert!(error <= 4.0 * f64::EPSILON, "{observed} != {expected}");
    }

    // Regular graphs at whole-slide scale; the products of their moment terms exceed
    // u128. Expected values are exact rational Cliff-Ord moments computed from S0, S1
    // and S2 of the degree sequence, independently of the edge-pair formulation.
    #[test]
    fn exact_null_moments_support_whole_slide_counts() {
        let cases = [
            // 200,000 cells, degree 6, two equal classes.
            (
                200_000,
                100_000,
                100_000,
                600_000,
                3_000_000,
                179_996_700_000,
                [
                    149_999.249_996_249_97,
                    37_499.249_987_812_34,
                    300_001.500_007_500_06,
                    149_996.999_951_249_37,
                ],
            ),
            // 400,000 cells, degree 30, classes of 150,000 and 250,000.
            (
                400_000,
                150_000,
                250_000,
                6_000_000,
                174_000_000,
                17_999_823_000_000,
                [
                    843_746.484_366_210_9,
                    329_566.552_573_012_36,
                    2_812_507.031_267_578_2,
                    1_318_266.210_292_049_5,
                ],
            ),
        ];
        for (n, a, b, edges, adjacent, disjoint, expected) in cases {
            let (mean, variance) =
                exact_null_moments(n, a, a, true, edges, adjacent, disjoint).unwrap();
            assert_close(mean, expected[0]);
            assert_close(variance, expected[1]);
            let (mean, variance) =
                exact_null_moments(n, a, b, false, edges, adjacent, disjoint).unwrap();
            assert_close(mean, expected[2]);
            assert_close(variance, expected[3]);
        }
    }
}
