use marklab_workflow::ContentDigest;

use crate::{
    classical::SpatialGeometryPlan2D, ClassicalSpatialLimits, DeclaredScalarPatternInput,
    ObservationWindow2D,
};

#[derive(Clone, Copy)]
pub(crate) struct DirectedPair {
    pub(crate) source: usize,
    pub(crate) target: usize,
    pub(crate) distance_um: f64,
}

pub(crate) struct MarkPairPlan {
    pub(crate) geometry: SpatialGeometryPlan2D,
    pub(crate) pairs: Vec<DirectedPair>,
    pub(crate) digest: ContentDigest,
    pub(crate) retained_bytes: usize,
}

#[derive(Debug)]
pub(crate) enum MarkPairPlanError {
    DirectedPairLimitExceeded { maximum: usize },
    RetainedByteLimitExceeded { required: usize, maximum: usize },
    AllocationFailed,
    SizeOverflow,
    Dependency(String),
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_mark_pair_plan(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    radii_um: &[f64],
    maximum_points: usize,
    maximum_radii: usize,
    maximum_directed_pairs: usize,
    maximum_retained_bytes: usize,
) -> Result<MarkPairPlan, MarkPairPlanError> {
    let classical_limits = ClassicalSpatialLimits::new(
        maximum_points,
        maximum_radii,
        maximum_directed_pairs,
        1,
        maximum_retained_bytes,
    )
    .map_err(dependency)?;
    let pattern = input.pattern();
    let geometry =
        SpatialGeometryPlan2D::new(&pattern.x_um, &pattern.y_um, window, classical_limits)
            .map_err(dependency)?;
    let maximum_radius = *radii_um.last().expect("validated nonempty radii");
    let mut pairs = Vec::new();
    for source in 0..pattern.len() {
        let mut failure = None;
        geometry
            .index()
            .visit_within_radius(source, maximum_radius, |neighbor| {
                retain_pair(
                    &geometry,
                    &mut pairs,
                    &mut failure,
                    source,
                    neighbor.index,
                    neighbor.distance_um,
                    maximum_directed_pairs,
                    maximum_retained_bytes,
                );
            })
            .map_err(dependency)?;
        if let Some(error) = failure {
            return Err(error);
        }
    }
    pairs.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.target.cmp(&right.target))
    });
    let digest = pair_plan_digest(geometry.logical_digest(), maximum_radius, &pairs);
    let retained_bytes = geometry
        .estimated_storage_bytes()
        .checked_add(
            pairs
                .len()
                .checked_mul(std::mem::size_of::<DirectedPair>())
                .ok_or(MarkPairPlanError::SizeOverflow)?,
        )
        .ok_or(MarkPairPlanError::SizeOverflow)?;
    if retained_bytes > maximum_retained_bytes {
        return Err(MarkPairPlanError::RetainedByteLimitExceeded {
            required: retained_bytes,
            maximum: maximum_retained_bytes,
        });
    }
    Ok(MarkPairPlan {
        geometry,
        pairs,
        digest,
        retained_bytes,
    })
}

#[allow(clippy::too_many_arguments)]
fn retain_pair(
    geometry: &SpatialGeometryPlan2D,
    pairs: &mut Vec<DirectedPair>,
    failure: &mut Option<MarkPairPlanError>,
    source: usize,
    target: usize,
    distance_um: f64,
    maximum_directed_pairs: usize,
    maximum_retained_bytes: usize,
) {
    if failure.is_some() {
        return;
    }
    if pairs.len() >= maximum_directed_pairs {
        *failure = Some(MarkPairPlanError::DirectedPairLimitExceeded {
            maximum: maximum_directed_pairs,
        });
        return;
    }
    let required = geometry.estimated_storage_bytes().saturating_add(
        pairs
            .len()
            .saturating_add(1)
            .saturating_mul(std::mem::size_of::<DirectedPair>()),
    );
    if required > maximum_retained_bytes {
        *failure = Some(MarkPairPlanError::RetainedByteLimitExceeded {
            required,
            maximum: maximum_retained_bytes,
        });
        return;
    }
    if pairs.try_reserve(1).is_err() {
        *failure = Some(MarkPairPlanError::AllocationFailed);
        return;
    }
    pairs.push(DirectedPair {
        source,
        target,
        distance_um,
    });
}

fn pair_plan_digest(
    geometry_digest: ContentDigest,
    maximum_radius: f64,
    pairs: &[DirectedPair],
) -> ContentDigest {
    // Keep the original domain string so existing categorical durable outputs remain compatible.
    let mut fields = vec![
        b"marklab-categorical-directed-pair-plan-v1".to_vec(),
        geometry_digest.as_bytes().to_vec(),
        maximum_radius.to_bits().to_be_bytes().to_vec(),
        (pairs.len() as u128).to_be_bytes().to_vec(),
    ];
    for pair in pairs {
        fields.push((pair.source as u128).to_be_bytes().to_vec());
        fields.push((pair.target as u128).to_be_bytes().to_vec());
        fields.push(pair.distance_um.to_bits().to_be_bytes().to_vec());
    }
    ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
}

fn dependency(error: impl std::fmt::Display) -> MarkPairPlanError {
    MarkPairPlanError::Dependency(error.to_string())
}
