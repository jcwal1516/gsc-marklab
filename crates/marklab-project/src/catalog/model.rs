use super::{
    validation::{validate_graph, validate_record_dependencies},
    *,
};

/// Validated in-memory artifact catalog with deterministic immutable snapshots.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ArtifactCatalog {
    pub(super) artifacts: BTreeMap<ArtifactId, ArtifactRecord>,
}

impl ArtifactCatalog {
    /// Create an empty catalog.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a catalog while allowing dependency records in any input order.
    pub fn from_records(
        records: impl IntoIterator<Item = ArtifactRecord>,
    ) -> Result<Self, ArtifactCatalogError> {
        let mut artifacts = BTreeMap::new();
        for record in records {
            let id = record.id();
            if artifacts.insert(id, record).is_some() {
                return Err(ArtifactCatalogError::DuplicateArtifact { artifact: id });
            }
            if artifacts.len() > MAX_CATALOG_ARTIFACTS {
                return Err(ArtifactCatalogError::TooManyArtifacts {
                    observed: artifacts.len(),
                    maximum: MAX_CATALOG_ARTIFACTS,
                });
            }
        }
        validate_graph(&artifacts)?;
        Ok(Self { artifacts })
    }

    /// Register one dependency-complete record or deterministically add replicas.
    ///
    /// The operation is failure-atomic. A new record must name only artifacts
    /// already present; use [`Self::from_records`] for a forward-referenced batch.
    pub fn register(&mut self, record: ArtifactRecord) -> Result<(), ArtifactCatalogError> {
        let id = record.id();
        if let Some(existing) = self.artifacts.get(&id) {
            if !existing.same_semantics(&record) {
                return Err(ArtifactCatalogError::ConflictingArtifact { artifact: id });
            }
            let mut merged = existing.clone();
            merged.merge_locations(record.locations())?;
            self.artifacts.insert(id, merged);
            return Ok(());
        }
        if self.artifacts.len() == MAX_CATALOG_ARTIFACTS {
            return Err(ArtifactCatalogError::TooManyArtifacts {
                observed: self.artifacts.len() + 1,
                maximum: MAX_CATALOG_ARTIFACTS,
            });
        }
        validate_record_dependencies(&self.artifacts, &record)?;
        self.artifacts.insert(id, record);
        Ok(())
    }

    /// Find a record by its exact schema-bound ID.
    pub fn get(&self, id: ArtifactId) -> Option<&ArtifactRecord> {
        self.artifacts.get(&id)
    }

    /// Whether a schema-bound ID is declared in this catalog.
    pub fn contains(&self, id: ArtifactId) -> bool {
        self.artifacts.contains_key(&id)
    }

    /// Number of unique schema-bound artifacts.
    pub fn len(&self) -> usize {
        self.artifacts.len()
    }

    /// Whether no schema-bound artifacts are present.
    pub fn is_empty(&self) -> bool {
        self.artifacts.is_empty()
    }

    /// Iterate in canonical artifact-ID order.
    pub fn iter(&self) -> impl Iterator<Item = (&ArtifactId, &ArtifactRecord)> {
        self.artifacts.iter()
    }
}
