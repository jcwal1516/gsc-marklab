use serde_json::{Map, Value};

use crate::errors::{MarklabError, Result};

use super::RESULT_FORMAT_VERSION;

pub(super) fn marked_document(value: Value) -> Result<Value> {
    let mut root = object(value, "result document")?;
    let analysis = object_mut(
        required_mut(&mut root, "analysis", "result document")?,
        "analysis",
    )?;
    let kind = analysis
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| MarklabError::Schema("0.2 analysis.kind must be a string".into()))?;
    if kind != "marked_pattern" {
        return Err(MarklabError::Schema(format!(
            "0.2 to 0.3 conversion supports marked_pattern documents only; found {kind}"
        )));
    }
    let result = object_mut(
        required_mut(analysis, "result", "analysis")?,
        "analysis.result",
    )?;
    migrate_marked_result(result)?;
    root.insert(
        "format_version".into(),
        Value::String(RESULT_FORMAT_VERSION.into()),
    );
    Ok(Value::Object(root))
}

fn migrate_marked_result(result: &mut Map<String, Value>) -> Result<()> {
    migrate_window(result)?;
    migrate_primary_endpoint(result)?;
    rename(result, "pair_correlation", "mark_pair_covariance")?;
    migrate_pair_curve(result)?;
    migrate_multiscale_summary(result)?;
    migrate_scale_energy(result)?;
    migrate_residual_territories(result)?;
    migrate_diagnostics(result)?;
    migrate_interpretation(result)?;
    remove_legacy_multimodal_placeholders(result)?;
    remove_legacy_curve_comparisons(result)?;
    add_component_mode_selection(result);
    result
        .entry("spectrum_null_sensitivity")
        .or_insert_with(not_applicable_section);
    Ok(())
}

fn migrate_window(result: &mut Map<String, Value>) -> Result<()> {
    let window = object_mut(
        required_mut(result, "window", "marked result")?,
        "marked result window",
    )?;
    rename(window, "l_eff_um", "analysis_effective_length_um")
}

fn migrate_primary_endpoint(result: &mut Map<String, Value>) -> Result<()> {
    let endpoint = object_mut(
        required_mut(result, "primary_endpoint", "marked result")?,
        "primary endpoint",
    )?;
    let null = endpoint
        .get("null")
        .and_then(Value::as_str)
        .ok_or_else(|| MarklabError::Schema("0.2 primary endpoint null must be a string".into()))?;
    if null != "fixed_position_random_labeling" {
        return Err(MarklabError::Schema(format!(
            "cannot safely convert 0.2 spectrum null {null}; rerun the original input"
        )));
    }
    Ok(())
}

fn migrate_pair_curve(result: &mut Map<String, Value>) -> Result<()> {
    let points = match result.remove("pair_correlation_curve") {
        Some(Value::Array(points)) => points,
        None => {
            result
                .entry("mark_pair_covariance_curve")
                .or_insert_with(|| Value::Array(Vec::new()));
            return Ok(());
        }
        Some(_) => {
            return Err(MarklabError::Schema(
                "0.2 pair_correlation_curve must be an array".into(),
            ));
        }
    };
    let mut converted = Vec::with_capacity(points.len());
    for point in points {
        let mut point = object(point, "0.2 pair-correlation point")?;
        let pair_count = point.get("count").and_then(Value::as_u64).ok_or_else(|| {
            MarklabError::Schema("0.2 pair point count must be an integer".into())
        })?;
        let value = point
            .remove("value")
            .ok_or_else(|| MarklabError::Schema("0.2 pair point value is required".into()))?;
        point.insert(
            "covariance".into(),
            if pair_count == 0 { Value::Null } else { value },
        );
        rename(&mut point, "count", "pair_count")?;
        converted.push(Value::Object(point));
    }
    result.insert("mark_pair_covariance_curve".into(), Value::Array(converted));
    Ok(())
}

fn migrate_multiscale_summary(result: &mut Map<String, Value>) -> Result<()> {
    let mut section = result
        .remove("wavelet")
        .unwrap_or_else(not_applicable_section);
    if let Some(value) = available_object_mut(&mut section, "0.2 wavelet summary")? {
        for (old, new) in [
            ("fine_variance_fraction", "local_difference_energy_fraction"),
            ("intermediate_variance_fraction", "residual_energy_fraction"),
            ("coarse_variance_fraction", "block_mean_variance_fraction"),
            (
                "coarse_to_fine_ratio",
                "block_mean_to_local_difference_ratio",
            ),
            (
                "coarse_variance_fraction_p_value",
                "block_mean_variance_fraction_p_value",
            ),
        ] {
            rename(value, old, new)?;
        }
    }
    result.insert("multiscale_residual".into(), section);
    Ok(())
}

fn migrate_scale_energy(result: &mut Map<String, Value>) -> Result<()> {
    rename(result, "scalogram", "scale_energy")?;
    let points = match result.remove("scalogram_curve") {
        Some(Value::Array(points)) => points,
        None => {
            result
                .entry("scale_energy_curve")
                .or_insert_with(|| Value::Array(Vec::new()));
            return Ok(());
        }
        Some(_) => {
            return Err(MarklabError::Schema(
                "0.2 scalogram_curve must be an array".into(),
            ));
        }
    };
    let mut converted = Vec::with_capacity(points.len());
    for point in points {
        let mut point = object(point, "0.2 scale-energy point")?;
        if let Some(Value::String(band)) = point.get_mut("band") {
            *band = match band.as_str() {
                "fine" => "local_difference".into(),
                "intermediate" => "residual".into(),
                "coarse" => "block_mean".into(),
                current => current.to_owned(),
            };
        }
        converted.push(Value::Object(point));
    }
    result.insert("scale_energy_curve".into(), Value::Array(converted));
    Ok(())
}

fn migrate_residual_territories(result: &mut Map<String, Value>) -> Result<()> {
    let mut section = result
        .remove("wavelet_territories")
        .unwrap_or_else(not_applicable_section);
    if let Some(values) = available_array_mut(&mut section, "0.2 residual territories")? {
        for territory in values {
            let territory = object_mut(territory, "0.2 residual territory")?;
            rename(territory, "scale_um", "analysis_scale_um")?;
            rename(territory, "z_or_power", "residual_score")?;
            rename(territory, "supporting_cells", "supporting_marked_cells")?;
            territory.remove("qc_overlap_fraction");
        }
    }
    result.insert("residual_territories".into(), section);
    Ok(())
}

fn migrate_diagnostics(result: &mut Map<String, Value>) -> Result<()> {
    let Some(section) = result.get_mut("diagnostics") else {
        return Ok(());
    };
    let Some(diagnostics) = available_object_mut(section, "0.2 diagnostics")? else {
        return Ok(());
    };
    rename(diagnostics, "beta_binomial", "beta_posterior_groups")?;
    if let Some(Value::Object(summary)) = diagnostics.get_mut("beta_posterior_groups") {
        if summary.get("diagnostic_name").and_then(Value::as_str) == Some("beta_binomial_v1") {
            summary.insert(
                "diagnostic_name".into(),
                Value::String("beta_posterior_group_summary_v1".into()),
            );
        }
    }
    Ok(())
}

fn migrate_interpretation(result: &mut Map<String, Value>) -> Result<()> {
    let interpretation = object_mut(
        required_mut(result, "interpretation", "marked result")?,
        "interpretation",
    )?;
    if let Some(Value::String(class)) = interpretation.get_mut("class") {
        *class = match class.as_str() {
            "coarse_clustered" => "coarse_excess".into(),
            "low_k_suppressed_or_dispersed" => "low_frequency_suppression".into(),
            current => current.to_owned(),
        };
    }
    Ok(())
}

fn remove_legacy_multimodal_placeholders(result: &mut Map<String, Value>) -> Result<()> {
    for field in ["registration", "fused_cell_summary"] {
        if let Some(section) = result.remove(field) {
            ensure_unavailable_section(&section, field)?;
        }
    }
    for field in [
        "neighborhood_enrichment",
        "cross_interaction_curves",
        "territory_profiles",
        "territory_comparisons",
    ] {
        if let Some(section) = result.remove(field) {
            ensure_empty_or_unavailable_section(&section, field)?;
        }
    }
    Ok(())
}

fn remove_legacy_curve_comparisons(result: &mut Map<String, Value>) -> Result<()> {
    if let Some(value) = result.remove("prepost_curve_tests") {
        let tests = value.as_array().ok_or_else(|| {
            MarklabError::Schema("0.2 prepost_curve_tests must be an array".into())
        })?;
        if !tests.is_empty() {
            return Err(MarklabError::Schema(
                "cannot safely convert populated 0.2 curve tests because zero may mean unavailable; rerun the original comparison".into(),
            ));
        }
    }
    Ok(())
}

fn add_component_mode_selection(result: &mut Map<String, Value>) {
    let has_components = result
        .get_mut("component_results")
        .and_then(|section| section.get("value"))
        .and_then(Value::as_array)
        .is_some_and(|values| !values.is_empty());
    if !has_components
        && result
            .get("component_results")
            .and_then(|section| section.get("status"))
            .and_then(Value::as_str)
            == Some("available")
    {
        result.insert("component_results".into(), not_applicable_section());
    }
    let mode = if has_components { "both" } else { "pooled" };
    result.insert(
        "component_mode_selection".into(),
        serde_json::json!({
            "requested": mode,
            "selected": mode,
            "reason": "inferred during 0.2 migration from populated component results"
        }),
    );
}

fn ensure_unavailable_section(section: &Value, field: &str) -> Result<()> {
    if section.get("status").and_then(Value::as_str) == Some("available") {
        return Err(MarklabError::Schema(format!(
            "cannot convert populated 0.2 marked-result {field}; rerun the original input as a multimodal analysis"
        )));
    }
    Ok(())
}

fn ensure_empty_or_unavailable_section(section: &Value, field: &str) -> Result<()> {
    if section.get("status").and_then(Value::as_str) != Some("available") {
        return Ok(());
    }
    if section
        .get("value")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
    {
        return Ok(());
    }
    Err(MarklabError::Schema(format!(
        "cannot convert populated 0.2 marked-result {field}; rerun the original input"
    )))
}

fn available_object_mut<'a>(
    section: &'a mut Value,
    context: &str,
) -> Result<Option<&'a mut Map<String, Value>>> {
    if section.get("status").and_then(Value::as_str) != Some("available") {
        return Ok(None);
    }
    Ok(Some(object_mut(
        section
            .get_mut("value")
            .ok_or_else(|| MarklabError::Schema(format!("{context} value is required")))?,
        context,
    )?))
}

fn available_array_mut<'a>(
    section: &'a mut Value,
    context: &str,
) -> Result<Option<&'a mut Vec<Value>>> {
    if section.get("status").and_then(Value::as_str) != Some("available") {
        return Ok(None);
    }
    let values = section
        .get_mut("value")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| MarklabError::Schema(format!("{context} value must be an array")))?;
    Ok(Some(values))
}

fn rename(map: &mut Map<String, Value>, old: &str, new: &str) -> Result<()> {
    if map.contains_key(old) && map.contains_key(new) {
        return Err(MarklabError::Schema(format!(
            "0.2 document contains both {old} and {new}"
        )));
    }
    if let Some(value) = map.remove(old) {
        map.insert(new.into(), value);
    }
    Ok(())
}

fn object(value: Value, context: &str) -> Result<Map<String, Value>> {
    match value {
        Value::Object(object) => Ok(object),
        _ => Err(MarklabError::Schema(format!("{context} must be an object"))),
    }
}

fn object_mut<'a>(value: &'a mut Value, context: &str) -> Result<&'a mut Map<String, Value>> {
    value
        .as_object_mut()
        .ok_or_else(|| MarklabError::Schema(format!("{context} must be an object")))
}

fn required_mut<'a>(
    map: &'a mut Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<&'a mut Value> {
    map.get_mut(field)
        .ok_or_else(|| MarklabError::Schema(format!("{context}.{field} is required")))
}

fn not_applicable_section() -> Value {
    serde_json::json!({"status": "not_applicable"})
}
