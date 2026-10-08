use std::collections::HashSet;

use marklab_core::{CoordinateFrameId, SectionId, UncertaintyId};

use super::CoordinateError;

/// Observation state for one retained serial-section entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SerialSectionStatus {
    /// Tissue was observed and has an explicit placement.
    Observed,
    /// The ordinal is retained as an explicit gap with no placement.
    Missing,
    /// Tissue was observed with distortion represented by referenced uncertainty.
    Distorted,
}

/// An explicit affine placement from intrinsic 2-D section coordinates to volume X/Y.
#[derive(Clone, Debug, PartialEq)]
pub struct SerialSectionPlacement {
    source_frame: CoordinateFrameId,
    coefficients: [f64; 6],
    uncertainty: Option<UncertaintyId>,
}

impl SerialSectionPlacement {
    /// Validate a row-major 2-by-3 placement matrix.
    ///
    /// Rows are semantic volume X then Y, non-translation columns follow the
    /// source frame's declared axis order, and translations use the volume unit.
    pub fn new(
        source_frame: CoordinateFrameId,
        coefficients: [f64; 6],
        uncertainty: Option<UncertaintyId>,
    ) -> Result<Self, CoordinateError> {
        if let Some(index) = coefficients
            .iter()
            .position(|coefficient| !coefficient.is_finite())
        {
            return Err(CoordinateError::NonFiniteSerialSectionPlacementCoefficient { index });
        }
        Ok(Self {
            source_frame,
            coefficients,
            uncertainty,
        })
    }

    /// Intrinsic physical 2-D source frame.
    pub fn source_frame(&self) -> &CoordinateFrameId {
        &self.source_frame
    }

    /// Row-major 2-by-3 placement coefficients.
    pub fn coefficients(&self) -> &[f64; 6] {
        &self.coefficients
    }

    /// Optional placement uncertainty expressed in the target volume frame.
    pub fn uncertainty(&self) -> Option<&UncertaintyId> {
        self.uncertainty.as_ref()
    }

    pub(crate) fn apply_to_semantic_xy(&self, values: [f64; 2]) -> [f64; 2] {
        [
            self.coefficients[0] * values[0]
                + self.coefficients[1] * values[1]
                + self.coefficients[2],
            self.coefficients[3] * values[0]
                + self.coefficients[4] * values[1]
                + self.coefficients[5],
        ]
    }
}

/// One ordered serial-section observation or explicit gap.
#[derive(Clone, Debug, PartialEq)]
pub struct SerialSection {
    id: SectionId,
    ordinal: u32,
    z_center: f64,
    thickness: f64,
    status: SerialSectionStatus,
    placement: Option<SerialSectionPlacement>,
}

impl SerialSection {
    /// Validate finite section geometry and its status/placement combination.
    pub fn new(
        id: SectionId,
        ordinal: u32,
        z_center: f64,
        thickness: f64,
        status: SerialSectionStatus,
        placement: Option<SerialSectionPlacement>,
    ) -> Result<Self, CoordinateError> {
        if !z_center.is_finite() {
            return Err(CoordinateError::InvalidSerialSectionZ { section: id });
        }
        if !thickness.is_finite() || thickness <= 0.0 {
            return Err(CoordinateError::InvalidSerialSectionThickness { section: id });
        }

        let state_is_valid = match (status, placement.as_ref()) {
            (SerialSectionStatus::Observed, Some(_)) | (SerialSectionStatus::Missing, None) => true,
            (SerialSectionStatus::Distorted, Some(placement)) => placement.uncertainty().is_some(),
            _ => false,
        };
        if !state_is_valid {
            return Err(CoordinateError::SerialSectionStateMismatch {
                section: id,
                status,
            });
        }

        Ok(Self {
            id,
            ordinal,
            z_center,
            thickness,
            status,
            placement,
        })
    }

    /// Section identity from the cohort hierarchy.
    pub fn id(&self) -> &SectionId {
        &self.id
    }

    /// Input-ordered section ordinal.
    pub fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Signed section-center Z in the target volume unit.
    pub fn z_center(&self) -> f64 {
        self.z_center
    }

    /// Positive section thickness in the target volume unit.
    pub fn thickness(&self) -> f64 {
        self.thickness
    }

    /// Observation state.
    pub fn status(&self) -> SerialSectionStatus {
        self.status
    }

    /// Explicit placement when tissue is present.
    pub fn placement(&self) -> Option<&SerialSectionPlacement> {
        self.placement.as_ref()
    }
}

/// A non-empty ordered series owned by one physical 3-D volume frame.
#[derive(Clone, Debug, PartialEq)]
pub struct SerialSectionSeries {
    volume_frame: CoordinateFrameId,
    sections: Vec<SerialSection>,
}

impl SerialSectionSeries {
    /// Validate local identity, ordinal, and Z ordering in declaration order.
    pub fn new(
        volume_frame: CoordinateFrameId,
        sections: Vec<SerialSection>,
    ) -> Result<Self, CoordinateError> {
        if sections.is_empty() {
            return Err(CoordinateError::EmptySerialSectionSeries {
                volume: volume_frame,
            });
        }

        let mut section_ids = HashSet::with_capacity(sections.len());
        for (index, section) in sections.iter().enumerate() {
            if !section_ids.insert(section.id().clone()) {
                return Err(CoordinateError::DuplicateSerialSection {
                    section: section.id().clone(),
                });
            }
            if index == 0 {
                continue;
            }
            let previous = &sections[index - 1];
            let Some(expected) = previous.ordinal().checked_add(1) else {
                return Err(CoordinateError::SerialSectionOrdinalOverflow {
                    section: previous.id().clone(),
                });
            };
            if section.ordinal() != expected {
                return Err(CoordinateError::NonConsecutiveSerialSectionOrdinal {
                    section: section.id().clone(),
                    expected,
                    observed: section.ordinal(),
                });
            }
            if section.z_center() <= previous.z_center() {
                return Err(CoordinateError::NonIncreasingSerialSectionZ {
                    section: section.id().clone(),
                    previous_z: previous.z_center(),
                    observed_z: section.z_center(),
                });
            }
        }

        Ok(Self {
            volume_frame,
            sections,
        })
    }

    /// Physical 3-D volume-frame identity.
    pub fn volume_frame(&self) -> &CoordinateFrameId {
        &self.volume_frame
    }

    /// Sections and explicit gaps in declaration order.
    pub fn sections(&self) -> &[SerialSection] {
        &self.sections
    }
}
