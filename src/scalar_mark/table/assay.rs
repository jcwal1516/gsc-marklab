use std::collections::BTreeSet;

use super::*;

/// One named multiplex assay channel with declared, content-bound provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct AssayMarkDeclaration {
    mark_id: ScalarMarkId,
    label: String,
    unit: String,
    measurement_status: MeasurementStatus,
    provenance_artifact_id: ArtifactId,
}

impl AssayMarkDeclaration {
    /// Declare a bounded channel label and exact assay unit; no conversion is inferred.
    ///
    /// Per-cell derived aggregates and empty/untrimmed/control-containing labels are rejected.
    /// The provenance identity binds evidence; construction does not independently verify an assay.
    pub fn new(
        mark_id: ScalarMarkId,
        label: impl Into<String>,
        unit: impl Into<String>,
        measurement_status: MeasurementStatus,
        provenance_artifact_id: ArtifactId,
    ) -> Result<Self, DeclaredScalarInputError> {
        let label = label.into();
        let unit = unit.into();
        if !bounded_label(&label, 256) || !bounded_label(&unit, 128) {
            return Err(invalid(
                "label and unit must be bounded trimmed non-control text",
            ));
        }
        if measurement_status == MeasurementStatus::DerivedSummary {
            return Err(DeclaredScalarInputError::UnsupportedPerCellMeasurementStatus);
        }
        Ok(Self {
            mark_id,
            label,
            unit,
            measurement_status,
            provenance_artifact_id,
        })
    }

    /// Stable channel identity.
    pub fn mark_id(&self) -> &ScalarMarkId {
        &self.mark_id
    }
    /// Display label, distinct from stable identity.
    pub fn label(&self) -> &str {
        &self.label
    }
    /// Exact unit, including any assay-specific normalization declared by the caller.
    pub fn unit(&self) -> &str {
        &self.unit
    }
    /// Measured versus imported/morphology-predicted status.
    pub fn measurement_status(&self) -> MeasurementStatus {
        self.measurement_status
    }
    /// Content-bound provenance reference.
    pub fn provenance_artifact_id(&self) -> ArtifactId {
        self.provenance_artifact_id
    }
}

/// Row-aligned assay observations. `None` is explicitly unavailable, never an observed zero.
#[derive(Clone, Debug, PartialEq)]
pub enum AssayMarkValues {
    /// Finite signed quantitative observations in the declaration's exact units.
    Continuous(Vec<Option<f64>>),
    /// Independently declared binary observations with unit `unitless`.
    Binary(Vec<Option<bool>>),
    /// Nominal codes with unit `categorical`; codes have no numeric distance interpretation.
    Categorical {
        /// Unique ordered codebook, bounded to 256 labels of at most 128 bytes each.
        levels: Vec<String>,
        /// Nullable zero-based codebook indices.
        values: Vec<Option<u32>>,
    },
}

impl AssayMarkValues {
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Continuous(values) => values.len(),
            Self::Binary(values) => values.len(),
            Self::Categorical { values, .. } => values.len(),
        }
    }

    fn has_missing(&self) -> bool {
        match self {
            Self::Continuous(values) => values.iter().any(Option::is_none),
            Self::Binary(values) => values.iter().any(Option::is_none),
            Self::Categorical { values, .. } => values.iter().any(Option::is_none),
        }
    }
}

impl ScalarMarkColumn {
    /// Construct a multiplex protein channel with explicit row availability.
    ///
    /// Values must be finite; binary/category units and category indices are checked. Signed
    /// quantitative values are allowed (e.g. declared background subtraction). Signed zero is
    /// canonicalized. Row, column and whole-study memory limits are checked by the owning caller.
    pub fn assay(
        declaration: AssayMarkDeclaration,
        mut values: AssayMarkValues,
    ) -> Result<Self, DeclaredScalarInputError> {
        match &mut values {
            AssayMarkValues::Continuous(observations) => {
                for (row, value) in observations.iter_mut().enumerate() {
                    if let Some(value) = value {
                        if !value.is_finite() {
                            return Err(invalid(format!(
                                "nonfinite quantitative observation at row {row}"
                            )));
                        }
                        if *value == 0.0 {
                            *value = 0.0;
                        }
                    }
                }
            }
            AssayMarkValues::Binary(_) if declaration.unit() != "unitless" => {
                return Err(DeclaredScalarInputError::UnitMismatch)
            }
            AssayMarkValues::Binary(_) => {}
            AssayMarkValues::Categorical { levels, values } => {
                if declaration.unit() != "categorical" {
                    return Err(DeclaredScalarInputError::UnitMismatch);
                }
                if levels.is_empty()
                    || levels.len() > 256
                    || levels.iter().any(|level| !bounded_label(level, 128))
                    || levels.iter().collect::<BTreeSet<_>>().len() != levels.len()
                {
                    return Err(invalid(
                        "categorical codebook must contain 1..=256 unique bounded labels",
                    ));
                }
                if let Some(row) = values
                    .iter()
                    .position(|value| value.is_some_and(|code| code as usize >= levels.len()))
                {
                    return Err(invalid(format!(
                        "categorical observation at row {row} is outside the codebook"
                    )));
                }
            }
        }
        let missingness = if values.has_missing() {
            MissingnessPolicy::Allowed
        } else {
            MissingnessPolicy::NotPermitted
        };
        Ok(Self {
            values: ScalarMarkColumnValues::Assay {
                declaration,
                values,
            },
            compatibility_semantics: None,
            missingness,
        })
    }
}

impl MarkTable {
    /// Borrow a channel by its exact identity without selecting an implicit default endpoint.
    pub fn assay_column(
        &self,
        mark_id: &ScalarMarkId,
    ) -> Option<(&AssayMarkDeclaration, &AssayMarkValues)> {
        self.columns.iter().find_map(|column| match &column.values {
            ScalarMarkColumnValues::Assay {
                declaration,
                values,
            } if declaration.mark_id() == mark_id => Some((declaration, values)),
            _ => None,
        })
    }
}

fn bounded_label(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn invalid(reason: impl Into<String>) -> DeclaredScalarInputError {
    DeclaredScalarInputError::analysis(crate::MarklabError::Validation(format!(
        "invalid assay column: {}",
        reason.into()
    )))
}
