#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InternalControlState {
    Valid(String),
    Invalid(String),
    Missing,
}

impl InternalControlState {
    pub(crate) fn from_optional(value: Option<String>) -> Self {
        let Some(value) = value else {
            return Self::Missing;
        };
        let trimmed = value.trim();
        if trimmed.is_empty() {
            Self::Missing
        } else if trimmed.eq_ignore_ascii_case("valid") {
            Self::Valid(trimmed.to_owned())
        } else {
            Self::Invalid(trimmed.to_owned())
        }
    }

    pub(crate) fn is_valid(&self) -> bool {
        matches!(self, Self::Valid(_))
    }

    pub(crate) fn label(&self) -> Option<&str> {
        match self {
            Self::Valid(label) | Self::Invalid(label) => Some(label),
            Self::Missing => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ArtifactFlags {
    artifact: Option<bool>,
    edge_artifact: Option<bool>,
    fold_artifact: Option<bool>,
}

impl ArtifactFlags {
    pub(crate) fn new(
        artifact: Option<bool>,
        edge_artifact: Option<bool>,
        fold_artifact: Option<bool>,
    ) -> Self {
        Self {
            artifact,
            edge_artifact,
            fold_artifact,
        }
    }

    pub(crate) fn is_available(self) -> bool {
        self.artifact.is_some() || self.edge_artifact.is_some() || self.fold_artifact.is_some()
    }

    pub(crate) fn is_excluded(self) -> bool {
        self.artifact.unwrap_or(false)
            || self.edge_artifact.unwrap_or(false)
            || self.fold_artifact.unwrap_or(false)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct NonviableFlags {
    necrosis: Option<bool>,
    therapy_effect: Option<bool>,
}

impl NonviableFlags {
    pub(crate) fn new(necrosis: Option<bool>, therapy_effect: Option<bool>) -> Self {
        Self {
            necrosis,
            therapy_effect,
        }
    }

    pub(crate) fn is_available(self) -> bool {
        self.necrosis.is_some() || self.therapy_effect.is_some()
    }

    pub(crate) fn is_excluded(self) -> bool {
        self.necrosis.unwrap_or(false) || self.therapy_effect.unwrap_or(false)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DecodedCellRow {
    pub x_um: f64,
    pub y_um: f64,
    pub mark: u8,
    pub mark_probability: Option<f32>,
    pub tumor_probability: Option<f32>,
    pub nucleus_area_um2: Option<f32>,
    pub case_id: String,
    pub timepoint: String,
    pub protein: String,
    pub internal_control: Option<InternalControlState>,
    pub slide_id: Option<String>,
    pub section_id: Option<String>,
    pub stain_batch: Option<String>,
    pub block_id: Option<String>,
    pub region_id: Option<String>,
    pub slide_region: Option<String>,
    pub histologic_compartment: Option<String>,
    pub valid_tumor: bool,
    pub valid_ihc: bool,
    pub artifact_flags: ArtifactFlags,
    pub nonviable_flags: NonviableFlags,
    pub qc_bin: Option<u16>,
    pub component_id: Option<u32>,
    pub local_dab_od: Option<f32>,
    pub local_hematoxylin_od: Option<f32>,
}
