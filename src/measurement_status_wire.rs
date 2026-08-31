use marklab_data::MeasurementStatus;

pub(crate) fn name(status: MeasurementStatus) -> &'static str {
    match status {
        MeasurementStatus::Measured => "measured",
        MeasurementStatus::ImportedPrediction => "imported_prediction",
        MeasurementStatus::MorphologyPrediction => "morphology_prediction",
        MeasurementStatus::DerivedSummary => "derived_summary",
    }
}

pub(crate) fn parse(value: &str) -> Option<MeasurementStatus> {
    match value {
        "measured" => Some(MeasurementStatus::Measured),
        "imported_prediction" => Some(MeasurementStatus::ImportedPrediction),
        "morphology_prediction" => Some(MeasurementStatus::MorphologyPrediction),
        "derived_summary" => Some(MeasurementStatus::DerivedSummary),
        _ => None,
    }
}
