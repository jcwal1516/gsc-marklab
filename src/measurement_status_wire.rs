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

pub(crate) fn serialize<S: serde::Serializer>(
    status: &MeasurementStatus,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(name(*status))
}

pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<MeasurementStatus, D::Error> {
    let value = <String as serde::Deserialize>::deserialize(deserializer)?;
    parse(&value).ok_or_else(|| serde::de::Error::custom("unknown measurement status"))
}
