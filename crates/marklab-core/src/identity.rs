use std::{fmt, sync::Arc};

use thiserror::Error;

/// Maximum UTF-8 byte length of one identity value.
pub const MAX_ID_BYTES: usize = 255;

/// The domain kind carried by a typed hierarchy identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HierarchyKind {
    /// Cohort or enrollment site.
    Site,
    /// Patient or other declared biological subject.
    Patient,
    /// Globally unique timepoint object, not a reusable label.
    Timepoint,
    /// Biospecimen.
    Specimen,
    /// Tissue block.
    Block,
    /// Physical or logical slide.
    Slide,
    /// Tissue section mounted on a slide.
    Section,
    /// Tissue core, including a TMA donor core.
    Core,
    /// Spatial region; regions may be nested.
    Region,
    /// Single cell.
    Cell,
    /// Image or feature patch.
    Patch,
}

impl fmt::Display for HierarchyKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Site => "site",
            Self::Patient => "patient",
            Self::Timepoint => "timepoint",
            Self::Specimen => "specimen",
            Self::Block => "block",
            Self::Slide => "slide",
            Self::Section => "section",
            Self::Core => "core",
            Self::Region => "region",
            Self::Cell => "cell",
            Self::Patch => "patch",
        })
    }
}

macro_rules! define_id {
    ($name:ident, $kind:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Arc<str>);

        impl $name {
            /// Validate and retain one opaque identity value.
            pub fn new(value: impl AsRef<str>) -> Result<Self, IdentityError> {
                validate_id(HierarchyKind::$kind, value.as_ref()).map(Self)
            }

            /// Borrow the exact validated identity text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

define_id!(
    PatientId,
    Patient,
    "A patient or declared biological-subject identity."
);
define_id!(SiteId, Site, "A cohort or enrollment-site identity.");
define_id!(SpecimenId, Specimen, "A biospecimen identity.");
define_id!(
    TimepointId,
    Timepoint,
    "A globally unique timepoint-object identity."
);
define_id!(BlockId, Block, "A tissue-block identity.");
define_id!(SlideId, Slide, "A physical or logical slide identity.");
define_id!(SectionId, Section, "A tissue-section identity.");
define_id!(CoreId, Core, "A tissue-core identity.");
define_id!(RegionId, Region, "A spatial-region identity.");
define_id!(CellId, Cell, "A single-cell identity.");
define_id!(PatchId, Patch, "An image or feature-patch identity.");

/// Any supported hierarchy identity with its kind preserved.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HierarchyId {
    /// Site identity.
    Site(SiteId),
    /// Patient identity.
    Patient(PatientId),
    /// Timepoint identity.
    Timepoint(TimepointId),
    /// Specimen identity.
    Specimen(SpecimenId),
    /// Block identity.
    Block(BlockId),
    /// Slide identity.
    Slide(SlideId),
    /// Section identity.
    Section(SectionId),
    /// Core identity.
    Core(CoreId),
    /// Region identity.
    Region(RegionId),
    /// Cell identity.
    Cell(CellId),
    /// Patch identity.
    Patch(PatchId),
}

impl HierarchyId {
    /// Return this identity's domain kind.
    pub fn kind(&self) -> HierarchyKind {
        match self {
            Self::Site(_) => HierarchyKind::Site,
            Self::Patient(_) => HierarchyKind::Patient,
            Self::Timepoint(_) => HierarchyKind::Timepoint,
            Self::Specimen(_) => HierarchyKind::Specimen,
            Self::Block(_) => HierarchyKind::Block,
            Self::Slide(_) => HierarchyKind::Slide,
            Self::Section(_) => HierarchyKind::Section,
            Self::Core(_) => HierarchyKind::Core,
            Self::Region(_) => HierarchyKind::Region,
            Self::Cell(_) => HierarchyKind::Cell,
            Self::Patch(_) => HierarchyKind::Patch,
        }
    }

    /// Borrow the exact validated identity text without its kind.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Site(value) => value.as_str(),
            Self::Patient(value) => value.as_str(),
            Self::Timepoint(value) => value.as_str(),
            Self::Specimen(value) => value.as_str(),
            Self::Block(value) => value.as_str(),
            Self::Slide(value) => value.as_str(),
            Self::Section(value) => value.as_str(),
            Self::Core(value) => value.as_str(),
            Self::Region(value) => value.as_str(),
            Self::Cell(value) => value.as_str(),
            Self::Patch(value) => value.as_str(),
        }
    }
}

impl fmt::Display for HierarchyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.kind(), self.as_str())
    }
}

macro_rules! impl_hierarchy_id_from {
    ($id:ty, $variant:ident) => {
        impl From<$id> for HierarchyId {
            fn from(value: $id) -> Self {
                Self::$variant(value)
            }
        }
    };
}

impl_hierarchy_id_from!(SiteId, Site);
impl_hierarchy_id_from!(PatientId, Patient);
impl_hierarchy_id_from!(TimepointId, Timepoint);
impl_hierarchy_id_from!(SpecimenId, Specimen);
impl_hierarchy_id_from!(BlockId, Block);
impl_hierarchy_id_from!(SlideId, Slide);
impl_hierarchy_id_from!(SectionId, Section);
impl_hierarchy_id_from!(CoreId, Core);
impl_hierarchy_id_from!(RegionId, Region);
impl_hierarchy_id_from!(CellId, Cell);
impl_hierarchy_id_from!(PatchId, Patch);

/// Rejection reasons for one opaque typed identity.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum IdentityError {
    /// The value is empty or consists only of whitespace.
    #[error("{kind} ID must not be blank")]
    Blank {
        /// Rejected identity kind.
        kind: HierarchyKind,
    },
    /// The value exceeds the fixed UTF-8 byte bound.
    #[error("{kind} ID has {byte_len} UTF-8 bytes, exceeding {MAX_ID_BYTES}")]
    TooLong {
        /// Rejected identity kind.
        kind: HierarchyKind,
        /// Observed UTF-8 byte length.
        byte_len: usize,
    },
    /// The value contains at least one Unicode control character.
    #[error("{kind} ID must not contain control characters")]
    ControlCharacter {
        /// Rejected identity kind.
        kind: HierarchyKind,
    },
    /// The value changes when surrounding Unicode whitespace is trimmed.
    #[error("{kind} ID must not contain surrounding whitespace")]
    SurroundingWhitespace {
        /// Rejected identity kind.
        kind: HierarchyKind,
    },
}

fn validate_id(kind: HierarchyKind, value: &str) -> Result<Arc<str>, IdentityError> {
    if value.trim().is_empty() {
        return Err(IdentityError::Blank { kind });
    }
    if value.len() > MAX_ID_BYTES {
        return Err(IdentityError::TooLong {
            kind,
            byte_len: value.len(),
        });
    }
    if value.chars().any(char::is_control) {
        return Err(IdentityError::ControlCharacter { kind });
    }
    if value.trim() != value {
        return Err(IdentityError::SurroundingWhitespace { kind });
    }
    Ok(Arc::from(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_share_text_without_losing_kind() {
        let patient = HierarchyId::from(PatientId::new("same").expect("patient"));
        let cell = HierarchyId::from(CellId::new("same").expect("cell"));
        assert_ne!(patient, cell);
        assert_eq!(patient.to_string(), "patient:same");
        assert_eq!(cell.to_string(), "cell:same");
    }
}
