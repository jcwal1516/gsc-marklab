use super::{
    identity::is_identifier, ArtifactId, ArtifactKey, ArtifactLocator, ArtifactRecordError,
    StoreId, MAX_ARTIFACT_KEY_BYTES, MAX_KEY_COMPONENT_BYTES, MAX_OBJECT_VERSION_BYTES,
};

impl StoreId {
    /// Validate a restricted portable store identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, ArtifactRecordError> {
        let value = value.into();
        if !is_identifier(&value) {
            return Err(ArtifactRecordError::InvalidStoreId { value });
        }
        Ok(Self(value))
    }

    /// Borrow the validated binding name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ArtifactKey {
    /// Validate a portable relative key without assigning namespace policy.
    pub fn new(value: impl Into<String>) -> Result<Self, ArtifactRecordError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_ARTIFACT_KEY_BYTES
            || value.starts_with('/')
            || value.starts_with("//")
            || value.contains('\\')
            || value.contains(':')
            || value.contains('?')
            || value.chars().any(char::is_control)
        {
            return Err(ArtifactRecordError::InvalidArtifactKey { value });
        }
        for component in value.split('/') {
            if component.is_empty()
                || component == "."
                || component == ".."
                || component.len() > MAX_KEY_COMPONENT_BYTES
            {
                return Err(ArtifactRecordError::InvalidArtifactKey { value });
            }
        }
        Ok(Self(value))
    }

    /// Borrow the normalized relative key.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn managed(id: ArtifactId) -> Self {
        let digest = id.to_string();
        Self(format!("objects/sha256/{}/{digest}", &digest[..2]))
    }

    pub(crate) fn is_reserved(&self) -> bool {
        matches!(
            self.0.split('/').next(),
            Some("objects" | ".marklab-staging" | ".marklab-quarantine" | ".marklab-store.lock")
        )
    }
}

impl ArtifactLocator {
    /// Build an external reference-only locator outside managed namespaces.
    pub fn new(
        store_id: StoreId,
        key: ArtifactKey,
        object_version: Option<String>,
    ) -> Result<Self, ArtifactRecordError> {
        if key.is_reserved() {
            return Err(ArtifactRecordError::ReservedArtifactKey { key: key.0.clone() });
        }
        Self::build(store_id, key, object_version)
    }

    pub(crate) fn managed(store_id: StoreId, id: ArtifactId) -> Self {
        Self {
            store_id,
            key: ArtifactKey::managed(id),
            object_version: None,
        }
    }

    pub(crate) fn decoded(
        store_id: StoreId,
        key: ArtifactKey,
        object_version: Option<String>,
        record_id: ArtifactId,
    ) -> Result<Self, ArtifactRecordError> {
        if key.is_reserved() && key != ArtifactKey::managed(record_id) {
            return Err(ArtifactRecordError::ReservedArtifactKey { key: key.0.clone() });
        }
        Self::build(store_id, key, object_version)
    }

    fn build(
        store_id: StoreId,
        key: ArtifactKey,
        object_version: Option<String>,
    ) -> Result<Self, ArtifactRecordError> {
        if let Some(version) = &object_version {
            if version.is_empty()
                || version.len() > MAX_OBJECT_VERSION_BYTES
                || !version.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':')
                })
            {
                return Err(ArtifactRecordError::InvalidObjectVersion {
                    value: version.clone(),
                });
            }
        }
        Ok(Self {
            store_id,
            key,
            object_version,
        })
    }

    /// Logical store binding.
    pub fn store_id(&self) -> &StoreId {
        &self.store_id
    }

    /// Store-relative key.
    pub fn key(&self) -> &ArtifactKey {
        &self.key
    }

    /// Optional non-secret object-generation provenance.
    pub fn object_version(&self) -> Option<&str> {
        self.object_version.as_deref()
    }
}
