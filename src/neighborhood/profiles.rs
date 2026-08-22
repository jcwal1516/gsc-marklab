use std::collections::{BTreeMap, BTreeSet};

use crate::{
    comparison::curves::max_abs_standardized_difference,
    errors::{MarklabError, Result},
    multimodal::{cells::FusedCell, labels::primary_label},
    output::{
        CurveComparisonAvailability, CurveComparisonResult, LabelFraction, TerritoryFeature,
        TerritoryProfile,
    },
};

pub fn territory_profiles(
    territories: &[TerritoryFeature],
    cells: &[FusedCell],
    buffer_um: f64,
) -> Result<Vec<TerritoryProfile>> {
    validate_buffer(buffer_um)?;
    validate_cells(cells)?;

    territories
        .iter()
        .enumerate()
        .map(|(territory_id, territory)| {
            profile_for_territory(territory_id, territory, cells, buffer_um)
        })
        .collect()
}

pub fn compare_territory_profiles(
    profiles: &[TerritoryProfile],
    margin: Option<f64>,
) -> Result<Vec<CurveComparisonResult>> {
    validate_margin(margin)?;
    validate_profiles(profiles)?;

    let mut tests = Vec::new();
    for left_index in 0..profiles.len() {
        for right_index in (left_index + 1)..profiles.len() {
            let left = &profiles[left_index];
            let right = &profiles[right_index];
            let labels = profile_labels(left, right);
            if labels.is_empty() || !has_profile_data(left, right) {
                tests.push(no_profile_data_result(left, right, margin));
                continue;
            }

            let left_vector = profile_vector(left, &labels);
            let right_vector = profile_vector(right, &labels);
            let statistic = max_abs_standardized_difference(&left_vector, &right_vector)?;
            let within_margin = margin.map(|margin| statistic <= margin);

            tests.push(CurveComparisonResult {
                comparison_name: format!(
                    "territory_{}_vs_{}",
                    left.territory_id, right.territory_id
                ),
                method: crate::output::CurveComparisonMethod::DescriptiveMargin,
                metric: "max_abs_standardized_difference".into(),
                availability: CurveComparisonAvailability::Available,
                statistic: Some(statistic),
                unavailable_reason: None,
                pooled_bin_p_value: None,
                margin,
                within_margin,
                interpretation: interpretation_for(statistic, margin, within_margin),
            });
        }
    }
    Ok(tests)
}

fn profile_for_territory(
    territory_id: usize,
    territory: &TerritoryFeature,
    cells: &[FusedCell],
    buffer_um: f64,
) -> Result<TerritoryProfile> {
    validate_territory(territory_id, territory)?;

    let inclusion_radius_um = territory.radius_um + buffer_um;
    if !inclusion_radius_um.is_finite() || inclusion_radius_um < 0.0 {
        return Err(MarklabError::Validation(format!(
            "territory {territory_id} inclusion radius must be finite and non-negative"
        )));
    }
    let inclusion_radius_sq = inclusion_radius_um * inclusion_radius_um;
    if !inclusion_radius_sq.is_finite() {
        return Err(MarklabError::Validation(format!(
            "territory {territory_id} squared inclusion radius is not finite"
        )));
    }
    let mut counts = BTreeMap::<&str, usize>::new();
    let mut known_cell_count = 0usize;
    let mut max_registration_error_um = 0.0_f64;

    for cell in cells {
        let dx = cell.x_um_registered - territory.center_x_um;
        let dy = cell.y_um_registered - territory.center_y_um;
        if dx.mul_add(dx, dy * dy) > inclusion_radius_sq {
            continue;
        }

        if let Some(registration_error_um) = cell.registration_error_um {
            max_registration_error_um = max_registration_error_um.max(registration_error_um);
        }

        if let Some(label) = primary_label(cell) {
            *counts.entry(label).or_insert(0) += 1;
            known_cell_count += 1;
        }
    }

    let cell_type_fractions = counts
        .into_iter()
        .map(|(label, count)| LabelFraction {
            label: label.to_owned(),
            fraction: if known_cell_count == 0 {
                0.0
            } else {
                count as f64 / known_cell_count as f64
            },
            count,
        })
        .collect();

    Ok(TerritoryProfile {
        territory_id,
        cell_type_fractions,
        enrichment: Vec::new(),
        cross_curves: Vec::new(),
        below_registration_resolution: territory.radius_um < 2.0 * max_registration_error_um,
    })
}

fn validate_buffer(buffer_um: f64) -> Result<()> {
    if !buffer_um.is_finite() || buffer_um < 0.0 {
        return Err(MarklabError::Config(
            "territory profile buffer must be finite and non-negative".into(),
        ));
    }
    Ok(())
}

fn validate_territory(index: usize, territory: &TerritoryFeature) -> Result<()> {
    if !territory.center_x_um.is_finite()
        || !territory.center_y_um.is_finite()
        || !territory.radius_um.is_finite()
        || territory.radius_um < 0.0
    {
        return Err(MarklabError::Validation(format!(
            "territory {index} must have finite center coordinates and non-negative finite radius"
        )));
    }
    Ok(())
}

fn validate_cells(cells: &[FusedCell]) -> Result<()> {
    for (index, cell) in cells.iter().enumerate() {
        if !cell.x_um_registered.is_finite() || !cell.y_um_registered.is_finite() {
            return Err(MarklabError::Schema(format!(
                "fused cell {index} ({}) has non-finite registered coordinates",
                cell.source_cell_id
            )));
        }
        if let Some(registration_error_um) = cell.registration_error_um {
            if !registration_error_um.is_finite() || registration_error_um < 0.0 {
                return Err(MarklabError::Schema(format!(
                    "fused cell {index} ({}) has invalid registration_error_um",
                    cell.source_cell_id
                )));
            }
        }
    }
    Ok(())
}

fn validate_margin(margin: Option<f64>) -> Result<()> {
    match margin {
        Some(margin) if !margin.is_finite() || margin < 0.0 => Err(MarklabError::Config(
            "territory profile comparison margin must be finite and non-negative".into(),
        )),
        _ => Ok(()),
    }
}

fn validate_profiles(profiles: &[TerritoryProfile]) -> Result<()> {
    for profile in profiles {
        let mut labels = BTreeSet::new();
        for fraction in &profile.cell_type_fractions {
            if !labels.insert(fraction.label.as_str()) {
                return Err(MarklabError::Schema(format!(
                    "territory profile {} has duplicate cell-type label {}",
                    profile.territory_id, fraction.label
                )));
            }
            if !fraction.fraction.is_finite() || !(0.0..=1.0).contains(&fraction.fraction) {
                return Err(MarklabError::Schema(format!(
                    "territory profile {} has invalid fraction for cell-type label {}",
                    profile.territory_id, fraction.label
                )));
            }
        }
    }
    Ok(())
}

fn profile_labels(left: &TerritoryProfile, right: &TerritoryProfile) -> Vec<String> {
    left.cell_type_fractions
        .iter()
        .chain(right.cell_type_fractions.iter())
        .map(|fraction| fraction.label.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn profile_vector(profile: &TerritoryProfile, labels: &[String]) -> Vec<f64> {
    let fractions = profile
        .cell_type_fractions
        .iter()
        .map(|fraction| (fraction.label.as_str(), fraction.fraction))
        .collect::<BTreeMap<_, _>>();
    labels
        .iter()
        .map(|label| fractions.get(label.as_str()).copied().unwrap_or(0.0))
        .collect()
}

fn has_profile_data(left: &TerritoryProfile, right: &TerritoryProfile) -> bool {
    left.cell_type_fractions
        .iter()
        .chain(right.cell_type_fractions.iter())
        .any(|fraction| fraction.count > 0 && fraction.fraction.is_finite())
}

fn no_profile_data_result(
    left: &TerritoryProfile,
    right: &TerritoryProfile,
    margin: Option<f64>,
) -> CurveComparisonResult {
    CurveComparisonResult {
        comparison_name: format!("territory_{}_vs_{}", left.territory_id, right.territory_id),
        method: crate::output::CurveComparisonMethod::DescriptiveMargin,
        metric: "max_abs_standardized_difference".into(),
        availability: CurveComparisonAvailability::InsufficientData,
        statistic: None,
        unavailable_reason: Some("no known cell-type labels are available for this territory pair".into()),
        pooled_bin_p_value: None,
        margin,
        within_margin: None,
        interpretation: "insufficient territory profile data: no known cell-type labels are available for this pair; a descriptive margin assessment is unavailable".into(),
    }
}

fn interpretation_for(statistic: f64, margin: Option<f64>, within_margin: Option<bool>) -> String {
    match (margin, within_margin) {
        (Some(_), Some(true)) => {
            "territory cell-type profile distance is within the requested descriptive margin".into()
        }
        (Some(_), Some(false)) => {
            "territory cell-type profile distance is outside the requested descriptive margin"
                .into()
        }
        _ if statistic > 0.0 => {
            "territory cell-type profiles differ by the reported descriptive statistic".into()
        }
        _ => "territory cell-type profiles have no observed descriptive difference".into(),
    }
}
