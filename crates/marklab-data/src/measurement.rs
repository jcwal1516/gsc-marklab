/// How a scientific value was obtained, independent of whether it is present or usable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeasurementStatus {
    /// Directly measured by the declared assay or acquisition modality.
    Measured,
    /// Imported prediction produced outside the current morphology pipeline.
    ImportedPrediction,
    /// Prediction or representation produced from morphology.
    MorphologyPrediction,
    /// Deterministic summary derived from lower-level values.
    DerivedSummary,
}
