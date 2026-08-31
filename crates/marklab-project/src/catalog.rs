use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
    str::FromStr,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::artifact::{
    ArtifactId, ArtifactKey, ArtifactLocator, ArtifactRecord, ArtifactRecordError, ArtifactSchema,
    StoreId, TableColumn, TableColumnType, TableFormat, TableManifest, TableManifestError,
    TableScalarType,
};
use crate::{ArtifactRef, ContentDigest, ProjectError};

const CATALOG_FORMAT: &str = "marklab.artifact_catalog";
const CATALOG_VERSION: u32 = 1;
const MAX_CATALOG_BYTES: usize = 16 * 1024 * 1024;
const MAX_CATALOG_ARTIFACTS: usize = 100_000;

mod codec;
mod error;
mod model;
mod validation;

pub use error::ArtifactCatalogError;
pub use model::ArtifactCatalog;
