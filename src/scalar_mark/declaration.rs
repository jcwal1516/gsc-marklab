use marklab_data::MeasurementStatus;
use marklab_workflow::ArtifactId;

use super::DeclaredScalarInputError;

const MARK_ID_MAX_BYTES: usize = 128;
const MARK_LABEL_MAX_BYTES: usize = 256;

/// Stable identifier for one declared scalar mark.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScalarMarkId(String);

impl ScalarMarkId {
    /// Validate the bounded version-one ASCII token grammar.
    pub fn new(value: impl Into<String>) -> Result<Self, DeclaredScalarInputError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= MARK_ID_MAX_BYTES
            && value.bytes().enumerate().all(|(index, byte)| match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => true,
                b'.' | b'_' | b':' | b'+' | b'-' => index != 0,
                _ => false,
            });
        if !valid {
            return Err(DeclaredScalarInputError::InvalidMarkId);
        }
        Ok(Self(value))
    }

    /// Borrow the exact stable identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Scalar value kind selected by one current marked-analysis endpoint family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarMarkValueKind {
    /// Required binary zero/one mark.
    Binary,
    /// Dense finite probability in the closed unit interval.
    Probability,
}

impl ScalarMarkValueKind {
    pub(super) fn wire_name(self) -> &'static str {
        match self {
            Self::Binary => "binary",
            Self::Probability => "probability",
        }
    }
}

/// Exact comparator used to derive a binary mark from a probability mark.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbabilityThresholdComparator {
    /// Probability must be strictly greater than the threshold.
    GreaterThan,
    /// Probability must be greater than or equal to the threshold.
    GreaterThanOrEqual,
}

impl ProbabilityThresholdComparator {
    pub(super) fn wire_name(self) -> &'static str {
        match self {
            Self::GreaterThan => "greater_than",
            Self::GreaterThanOrEqual => "greater_than_or_equal",
        }
    }

    pub(super) fn evaluate(self, probability: f32, threshold: f32) -> bool {
        match self {
            Self::GreaterThan => probability > threshold,
            Self::GreaterThanOrEqual => probability >= threshold,
        }
    }
}

/// Version-one origin of the required binary mark.
#[derive(Clone, Debug, PartialEq)]
pub enum BinaryMarkOrigin {
    /// Binary values were declared independently; no threshold relationship is claimed.
    Independent,
    /// Binary values are exactly derived from the named probability mark.
    Thresholded {
        /// Stable identity of the source probability mark.
        probability_mark_id: ScalarMarkId,
        /// Exact comparator applied row by row.
        comparator: ProbabilityThresholdComparator,
        /// Finite canonical `f32` threshold in the closed unit interval.
        threshold: f32,
        /// Exact threshold-evidence artifact identity.
        threshold_provenance_artifact_id: ArtifactId,
    },
}

impl BinaryMarkOrigin {
    pub(super) fn wire_name(&self) -> &'static str {
        match self {
            Self::Independent => "independent",
            Self::Thresholded { .. } => "thresholded",
        }
    }
}

/// Required unitless binary mark declaration for the compatibility engine.
#[derive(Clone, Debug, PartialEq)]
pub struct BinaryMarkDeclaration {
    pub(super) mark_id: ScalarMarkId,
    pub(super) label: String,
    pub(super) measurement_status: MeasurementStatus,
    pub(super) provenance_artifact_id: ArtifactId,
    pub(super) origin: BinaryMarkOrigin,
}

impl BinaryMarkDeclaration {
    /// Declare an independently supplied binary mark without threshold semantics.
    pub fn independent(
        mark_id: ScalarMarkId,
        label: impl Into<String>,
        measurement_status: MeasurementStatus,
        provenance_artifact_id: ArtifactId,
    ) -> Result<Self, DeclaredScalarInputError> {
        Self::build(
            mark_id,
            label.into(),
            measurement_status,
            provenance_artifact_id,
            BinaryMarkOrigin::Independent,
        )
    }

    /// Declare a binary mark exactly thresholded from one probability mark.
    #[allow(clippy::too_many_arguments)]
    pub fn thresholded(
        mark_id: ScalarMarkId,
        label: impl Into<String>,
        measurement_status: MeasurementStatus,
        provenance_artifact_id: ArtifactId,
        probability_mark_id: ScalarMarkId,
        comparator: ProbabilityThresholdComparator,
        threshold: f32,
        threshold_provenance_artifact_id: Option<ArtifactId>,
    ) -> Result<Self, DeclaredScalarInputError> {
        if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
            return Err(DeclaredScalarInputError::InvalidThreshold);
        }
        let threshold_provenance_artifact_id = threshold_provenance_artifact_id
            .ok_or(DeclaredScalarInputError::ThresholdProvenanceMissing)?;
        Self::build(
            mark_id,
            label.into(),
            measurement_status,
            provenance_artifact_id,
            BinaryMarkOrigin::Thresholded {
                probability_mark_id,
                comparator,
                threshold,
                threshold_provenance_artifact_id,
            },
        )
    }

    fn build(
        mark_id: ScalarMarkId,
        label: String,
        measurement_status: MeasurementStatus,
        provenance_artifact_id: ArtifactId,
        origin: BinaryMarkOrigin,
    ) -> Result<Self, DeclaredScalarInputError> {
        validate_label(&label)?;
        validate_per_cell_status(measurement_status)?;
        Ok(Self {
            mark_id,
            label,
            measurement_status,
            provenance_artifact_id,
            origin,
        })
    }

    /// Stable binary mark identifier.
    pub fn mark_id(&self) -> &ScalarMarkId {
        &self.mark_id
    }

    /// Exact display label required to match config 0.2.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// How this per-cell binary value was obtained.
    pub fn measurement_status(&self) -> MeasurementStatus {
        self.measurement_status
    }

    /// Exact C-03 provenance artifact identity.
    pub fn provenance_artifact_id(&self) -> ArtifactId {
        self.provenance_artifact_id
    }

    /// Independent or thresholded origin declaration.
    pub fn origin(&self) -> &BinaryMarkOrigin {
        &self.origin
    }
}

/// Optional dense unitless probability mark declaration.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbabilityMarkDeclaration {
    pub(super) mark_id: ScalarMarkId,
    pub(super) measurement_status: MeasurementStatus,
    pub(super) provenance_artifact_id: ArtifactId,
}

impl ProbabilityMarkDeclaration {
    /// Declare a dense probability column and its exact provenance artifact.
    pub fn new(
        mark_id: ScalarMarkId,
        measurement_status: MeasurementStatus,
        provenance_artifact_id: ArtifactId,
    ) -> Result<Self, DeclaredScalarInputError> {
        validate_per_cell_status(measurement_status)?;
        Ok(Self {
            mark_id,
            measurement_status,
            provenance_artifact_id,
        })
    }

    /// Stable probability mark identifier.
    pub fn mark_id(&self) -> &ScalarMarkId {
        &self.mark_id
    }

    /// How this per-cell probability was obtained.
    pub fn measurement_status(&self) -> MeasurementStatus {
        self.measurement_status
    }

    /// Exact C-03 provenance artifact identity.
    pub fn provenance_artifact_id(&self) -> ArtifactId {
        self.provenance_artifact_id
    }
}

/// Runtime-only declaration and endpoint-routing summary for one marked analysis.
#[derive(Clone, Debug, PartialEq)]
pub struct DeclaredMarkUse {
    pub(super) binary_mark: BinaryMarkDeclaration,
    pub(super) probability_mark: Option<ProbabilityMarkDeclaration>,
    pub(super) structure_factor_value_kind: ScalarMarkValueKind,
}

impl DeclaredMarkUse {
    /// Required binary mark used for counts and every non-spectrum current endpoint.
    pub fn binary_mark(&self) -> &BinaryMarkDeclaration {
        &self.binary_mark
    }

    /// Probability mark used by structure-factor spectra, when configured.
    pub fn probability_mark(&self) -> Option<&ProbabilityMarkDeclaration> {
        self.probability_mark.as_ref()
    }

    /// Value kind used only for pooled/component structure-factor spectra and value permutations.
    pub fn structure_factor_value_kind(&self) -> ScalarMarkValueKind {
        self.structure_factor_value_kind
    }

    /// Value kind used for all other current marked-analysis endpoint families.
    pub fn other_endpoint_value_kind(&self) -> ScalarMarkValueKind {
        ScalarMarkValueKind::Binary
    }
}

fn validate_label(label: &str) -> Result<(), DeclaredScalarInputError> {
    if label.is_empty()
        || label.len() > MARK_LABEL_MAX_BYTES
        || label.trim() != label
        || label.chars().any(char::is_control)
    {
        return Err(DeclaredScalarInputError::InvalidMarkLabel);
    }
    Ok(())
}

fn validate_per_cell_status(status: MeasurementStatus) -> Result<(), DeclaredScalarInputError> {
    if status == MeasurementStatus::DerivedSummary {
        return Err(DeclaredScalarInputError::UnsupportedPerCellMeasurementStatus);
    }
    Ok(())
}

pub(super) fn measurement_status_name(status: MeasurementStatus) -> &'static str {
    match status {
        MeasurementStatus::Measured => "measured",
        MeasurementStatus::ImportedPrediction => "imported_prediction",
        MeasurementStatus::MorphologyPrediction => "morphology_prediction",
        MeasurementStatus::DerivedSummary => "derived_summary",
    }
}
