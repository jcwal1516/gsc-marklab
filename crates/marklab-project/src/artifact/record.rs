use std::collections::BTreeMap;

use super::{
    identity::prepare_artifact_identity, ArtifactDraft, ArtifactId, ArtifactLocator,
    ArtifactRecord, ArtifactRecordError, ArtifactSchema, TableManifest,
};
use crate::{ArtifactRef, ContentDigest};

impl ArtifactDraft {
    /// Build a validated location-free declaration with stable artifact identity.
    pub fn new(
        content: ArtifactRef,
        schema: ArtifactSchema,
        table: Option<TableManifest>,
        dependencies: Vec<ArtifactId>,
        semantic_metadata: BTreeMap<String, String>,
    ) -> Result<Self, ArtifactRecordError> {
        let (dependencies, semantic_digest, id) = prepare_artifact_identity(
            &content,
            &schema,
            table.as_ref(),
            dependencies,
            &semantic_metadata,
        )?;
        Ok(Self {
            id,
            semantic_digest,
            content,
            schema,
            table,
            dependencies,
            semantic_metadata,
        })
    }

    /// Schema- and content-bound identity that publication must preserve.
    pub fn id(&self) -> ArtifactId {
        self.id
    }

    /// Location-independent digest of the semantic declaration.
    pub fn semantic_digest(&self) -> ContentDigest {
        self.semantic_digest
    }

    /// Exact encoded-byte reference that the publication callback must reproduce.
    pub fn content(&self) -> &ArtifactRef {
        &self.content
    }

    /// Versioned semantic schema.
    pub fn schema(&self) -> &ArtifactSchema {
        &self.schema
    }

    /// Optional exact native-table declaration.
    pub fn table(&self) -> Option<&TableManifest> {
        self.table.as_ref()
    }

    /// Sorted direct artifact dependencies.
    pub fn dependencies(&self) -> &[ArtifactId] {
        &self.dependencies
    }

    /// Sorted bounded semantic metadata.
    pub fn semantic_metadata(&self) -> &BTreeMap<String, String> {
        &self.semantic_metadata
    }

    pub(crate) fn located_record(&self, locator: ArtifactLocator) -> ArtifactRecord {
        ArtifactRecord {
            id: self.id,
            semantic_digest: self.semantic_digest,
            content: self.content.clone(),
            schema: self.schema.clone(),
            table: self.table.clone(),
            dependencies: self.dependencies.clone(),
            semantic_metadata: self.semantic_metadata.clone(),
            locations: vec![locator],
        }
    }
}

impl ArtifactRecord {
    /// Build a validated record and compute its location-independent identity.
    pub fn new(
        content: ArtifactRef,
        schema: ArtifactSchema,
        table: Option<TableManifest>,
        dependencies: Vec<ArtifactId>,
        semantic_metadata: BTreeMap<String, String>,
        mut locations: Vec<ArtifactLocator>,
    ) -> Result<Self, ArtifactRecordError> {
        let (dependencies, semantic_digest, id) = prepare_artifact_identity(
            &content,
            &schema,
            table.as_ref(),
            dependencies,
            &semantic_metadata,
        )?;
        if locations.is_empty() || locations.len() > 16 {
            return Err(ArtifactRecordError::InvalidLocationCount {
                observed: locations.len(),
            });
        }
        locations.sort();
        for pair in locations.windows(2) {
            if pair[0].store_id == pair[1].store_id {
                return Err(ArtifactRecordError::DuplicateStoreLocator {
                    store_id: pair[0].store_id.0.clone(),
                });
            }
        }
        Ok(Self {
            id,
            semantic_digest,
            content,
            schema,
            table,
            dependencies,
            semantic_metadata,
            locations,
        })
    }

    /// Schema- and content-bound artifact identity.
    pub fn id(&self) -> ArtifactId {
        self.id
    }

    /// Location-independent digest of the semantic declaration.
    pub fn semantic_digest(&self) -> ContentDigest {
        self.semantic_digest
    }

    /// Exact encoded-byte reference.
    pub fn content(&self) -> &ArtifactRef {
        &self.content
    }

    /// Versioned semantic schema.
    pub fn schema(&self) -> &ArtifactSchema {
        &self.schema
    }

    /// Optional exact native-table declaration.
    pub fn table(&self) -> Option<&TableManifest> {
        self.table.as_ref()
    }

    /// Sorted direct artifact dependencies.
    pub fn dependencies(&self) -> &[ArtifactId] {
        &self.dependencies
    }

    /// Sorted bounded semantic metadata.
    pub fn semantic_metadata(&self) -> &BTreeMap<String, String> {
        &self.semantic_metadata
    }

    /// Canonically sorted replica locations, at most one per store.
    pub fn locations(&self) -> &[ArtifactLocator] {
        &self.locations
    }

    pub(crate) fn merge_locations(
        &mut self,
        incoming: &[ArtifactLocator],
    ) -> Result<bool, ArtifactRecordError> {
        let mut merged = self.locations.clone();
        let mut changed = false;
        for locator in incoming {
            match merged.binary_search_by(|existing| existing.store_id.cmp(&locator.store_id)) {
                Ok(index) if merged[index] == *locator => {}
                Ok(_) => {
                    return Err(ArtifactRecordError::ConflictingStoreLocator {
                        store_id: locator.store_id.0.clone(),
                    });
                }
                Err(index) => {
                    if merged.len() == 16 {
                        return Err(ArtifactRecordError::InvalidLocationCount { observed: 17 });
                    }
                    merged.insert(index, locator.clone());
                    changed = true;
                }
            }
        }
        if changed {
            self.locations = merged;
        }
        Ok(changed)
    }

    pub(crate) fn same_semantics(&self, other: &Self) -> bool {
        self.id == other.id
            && self.semantic_digest == other.semantic_digest
            && self.content == other.content
            && self.schema == other.schema
            && self.table == other.table
            && self.dependencies == other.dependencies
            && self.semantic_metadata == other.semantic_metadata
    }
}
