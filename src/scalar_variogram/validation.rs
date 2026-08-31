use std::collections::BTreeMap;

use marklab_workflow::ContentDigest;

use super::{ScalarVariogramBin, ScalarVariogramError};
use crate::{
    geom::window::ObservationWindow2D,
    scalar_mark::{DeclaredScalarPatternInput, ScalarMarkId},
};

pub(super) fn has_distinct_values(values: &[f64]) -> bool {
    values
        .first()
        .is_some_and(|first| values[1..].iter().any(|value| value != first))
}

pub(super) fn has_stratified_distinct_values(values: &[f64], strata: &[u32]) -> bool {
    if values.len() != strata.len() {
        return false;
    }
    let mut first_by_stratum = BTreeMap::<u32, f64>::new();
    for (value, stratum) in values.iter().zip(strata) {
        match first_by_stratum.get(stratum) {
            Some(first) if first != value => return true,
            Some(_) => {}
            None => {
                first_by_stratum.insert(*stratum, *value);
            }
        }
    }
    false
}

pub(super) fn validate_bins(bins: &[ScalarVariogramBin]) -> Result<(), ScalarVariogramError> {
    if bins.is_empty()
        || bins.iter().any(|bin| {
            !bin.lower_um.is_finite()
                || !bin.upper_um.is_finite()
                || bin.lower_um < 0.0
                || bin.upper_um <= bin.lower_um
        })
        || bins
            .windows(2)
            .any(|pair| pair[0].upper_um.to_bits() != pair[1].lower_um.to_bits())
    {
        return Err(ScalarVariogramError::InvalidBins);
    }
    Ok(())
}

pub(super) fn validate_points(
    x: &[f64],
    y: &[f64],
    window: &ObservationWindow2D,
) -> Result<(), ScalarVariogramError> {
    if x.len() != y.len() {
        return Err(ScalarVariogramError::PointShapeMismatch {
            x_rows: x.len(),
            y_rows: y.len(),
        });
    }
    let mut coordinates = BTreeMap::new();
    for row in 0..x.len() {
        if !window.contains(x[row], y[row]) {
            return Err(ScalarVariogramError::PointOutsideWindow { row });
        }
        let key = (canonical_bits(x[row]), canonical_bits(y[row]));
        if let Some(first_row) = coordinates.insert(key, row) {
            return Err(ScalarVariogramError::DuplicatePoint {
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

pub(super) fn normalize_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

pub(crate) fn pair_plan_digest(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    declared_input_digest: ContentDigest,
    bins: &[ScalarVariogramBin],
) -> ContentDigest {
    let pattern = input.pattern();
    let mut fields = vec![
        b"marklab-scalar-variogram-pair-plan-v1".to_vec(),
        input.coordinate_frame_id().as_str().as_bytes().to_vec(),
        window.descriptor().logical_digest.as_bytes().to_vec(),
        mark_id.as_str().as_bytes().to_vec(),
        declared_input_digest.as_bytes().to_vec(),
    ];
    for ((cell_id, x), y) in input
        .cell_ids()
        .iter()
        .zip(&pattern.x_um)
        .zip(&pattern.y_um)
    {
        fields.push(cell_id.as_str().as_bytes().to_vec());
        fields.push(x.to_bits().to_be_bytes().to_vec());
        fields.push(y.to_bits().to_be_bytes().to_vec());
    }
    for bin in bins {
        fields.push(bin.lower_um.to_bits().to_be_bytes().to_vec());
        fields.push(bin.upper_um.to_bits().to_be_bytes().to_vec());
    }
    ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
}
