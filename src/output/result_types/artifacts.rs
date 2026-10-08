use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ArtifactStatus {
    Written { path: PathBuf },
    Disabled,
    NotApplicable,
    InsufficientData { reason: String },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OutputManifest {
    pub result: ArtifactStatus,
    pub artifacts: BTreeMap<String, ArtifactStatus>,
}
