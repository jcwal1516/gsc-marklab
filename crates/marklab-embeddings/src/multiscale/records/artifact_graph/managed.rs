use std::convert::Infallible;

use marklab_project::{
    ArtifactCatalog, ArtifactId, ArtifactReadSeek, ArtifactRecord, LocalArtifactStore,
    VerifiedReaderError,
};

use super::{
    record::required_record, MultiscaleEmbeddingArtifactGraphError, MultiscaleEmbeddingArtifactRole,
};
use crate::{
    multiscale::json::CanonicalJsonReaderError, provenance::artifact_availability_failure,
    ArtifactAvailabilityFailure,
};

pub(super) fn require_canonical_payload_for<F>(
    store: &LocalArtifactStore,
    catalog: &ArtifactCatalog,
    role: MultiscaleEmbeddingArtifactRole,
    id: ArtifactId,
    compare: F,
) -> Result<(), MultiscaleEmbeddingArtifactGraphError>
where
    F: FnOnce(&mut dyn ArtifactReadSeek) -> Result<(), CanonicalJsonReaderError>,
{
    require_canonical_payload(store, required_record(catalog, role, id)?, role, compare)
}

pub(super) fn require_canonical_payload<F>(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    role: MultiscaleEmbeddingArtifactRole,
    compare: F,
) -> Result<(), MultiscaleEmbeddingArtifactGraphError>
where
    F: FnOnce(&mut dyn ArtifactReadSeek) -> Result<(), CanonicalJsonReaderError>,
{
    store
        .with_verified_reader(record, compare)
        .map_err(|error| canonical_reader_error(role, error))
}

fn canonical_reader_error(
    role: MultiscaleEmbeddingArtifactRole,
    error: VerifiedReaderError<CanonicalJsonReaderError>,
) -> MultiscaleEmbeddingArtifactGraphError {
    match error {
        VerifiedReaderError::Store(error) => MultiscaleEmbeddingArtifactGraphError::Unavailable {
            role,
            reason: artifact_availability_failure(&error),
        },
        VerifiedReaderError::Callback(CanonicalJsonReaderError::Mismatch) => {
            MultiscaleEmbeddingArtifactGraphError::PayloadIdentityMismatch { role }
        }
        VerifiedReaderError::Callback(CanonicalJsonReaderError::Read) => {
            MultiscaleEmbeddingArtifactGraphError::Unavailable {
                role,
                reason: ArtifactAvailabilityFailure::StoreAccess,
            }
        }
    }
}

pub(super) fn require_available(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    role: MultiscaleEmbeddingArtifactRole,
) -> Result<(), MultiscaleEmbeddingArtifactGraphError> {
    store
        .with_verified_reader(record, |_reader| Ok::<(), Infallible>(()))
        .map_err(|error| match error {
            VerifiedReaderError::Store(error) => {
                MultiscaleEmbeddingArtifactGraphError::Unavailable {
                    role,
                    reason: artifact_availability_failure(&error),
                }
            }
            VerifiedReaderError::Callback(error) => match error {},
        })
}

pub(super) fn require_available_for(
    store: &LocalArtifactStore,
    catalog: &ArtifactCatalog,
    role: MultiscaleEmbeddingArtifactRole,
    id: ArtifactId,
) -> Result<(), MultiscaleEmbeddingArtifactGraphError> {
    require_available(store, required_record(catalog, role, id)?, role)
}

#[cfg(test)]
mod tests {
    use marklab_project::VerifiedReaderError;

    use super::{canonical_reader_error, CanonicalJsonReaderError};
    use crate::{
        ArtifactAvailabilityFailure, MultiscaleEmbeddingArtifactGraphError,
        MultiscaleEmbeddingArtifactRole,
    };

    #[test]
    fn canonical_reader_io_failure_maps_to_the_redacted_store_category() {
        let error = canonical_reader_error(
            MultiscaleEmbeddingArtifactRole::SourceEntities,
            VerifiedReaderError::Callback(CanonicalJsonReaderError::Read),
        );
        assert_eq!(
            error,
            MultiscaleEmbeddingArtifactGraphError::Unavailable {
                role: MultiscaleEmbeddingArtifactRole::SourceEntities,
                reason: ArtifactAvailabilityFailure::StoreAccess,
            }
        );
    }
}
