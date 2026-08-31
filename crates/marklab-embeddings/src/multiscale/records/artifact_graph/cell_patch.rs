use std::{convert::Infallible, fmt};

use marklab_project::{
    ArtifactCatalog, ArtifactId, ArtifactRecord, LocalArtifactStore, VerifiedReaderError,
};
use thiserror::Error;

use crate::{
    columnar::{
        validate_patch_footprint_set_arrow_from_store,
        validate_patch_footprint_set_parquet_from_store, EmbeddingColumnarBudgets,
        MultiscaleColumnarError,
    },
    expected::ExpectedCellReaderError,
    multiscale::{
        cell_patch::{CellPatchAssignmentMode, CellPatchLink},
        context::PatchEmbeddingContext,
        expected::ExpectedPatchSet,
        footprint::PatchFootprintSet,
        json::CanonicalJsonReaderError,
        physical::{record_matches, SpatialArtifactRole},
        records::CellPatchLinkProducer,
    },
    provenance::artifact_availability_failure as availability_failure,
    ArtifactAvailabilityFailure, ExpectedCellSet,
};

/// Artifact roles admitted by independent cell-patch input validation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum CellPatchInputArtifactRole {
    /// The exact reused C-04 expected-cell set.
    ExpectedCells,
    /// The exact expected-patch set.
    ExpectedPatches,
    /// The calibrated patch context.
    PatchContext,
    /// The fully decoded patch-footprint artifact.
    PatchFootprints,
    /// The strict cell-patch producer descriptor.
    Producer,
    /// Opaque source cell-anchor coordinate bytes.
    SourceCoordinates,
    /// The producer run configuration.
    RunConfig,
    /// The producer execution environment.
    Environment,
    /// The producer converter manifest.
    Converter,
}

impl fmt::Display for CellPatchInputArtifactRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ExpectedCells => "expected cells",
            Self::ExpectedPatches => "expected patches",
            Self::PatchContext => "patch context",
            Self::PatchFootprints => "patch footprints",
            Self::Producer => "cell-patch producer",
            Self::SourceCoordinates => "source coordinates",
            Self::RunConfig => "run config",
            Self::Environment => "environment",
            Self::Converter => "converter",
        })
    }
}

/// Exact cell-patch input record, payload, physical, binding, or availability failure.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
#[non_exhaustive]
pub enum CellPatchInputArtifactGraphError {
    /// One required role is absent from the catalog.
    #[error("required {role} artifact is absent from the catalog")]
    MissingRecord {
        /// Missing role.
        role: CellPatchInputArtifactRole,
    },
    /// A required record has the wrong schema ID or version.
    #[error("required {role} artifact has the wrong schema or version")]
    SchemaMismatch {
        /// Mismatched role.
        role: CellPatchInputArtifactRole,
    },
    /// A required record has the wrong content kind.
    #[error("required {role} artifact has the wrong content kind")]
    ContentKindMismatch {
        /// Mismatched role.
        role: CellPatchInputArtifactRole,
    },
    /// A non-table role declares a table manifest.
    #[error("required {role} artifact has an unexpected table manifest")]
    UnexpectedTableManifest {
        /// Mismatched role.
        role: CellPatchInputArtifactRole,
    },
    /// The footprint table declaration differs from the exact profile.
    #[error("required patch footprints artifact has the wrong table manifest")]
    TableManifestMismatch,
    /// A required record carries forbidden semantic metadata.
    #[error("required {role} artifact semantic metadata must be empty")]
    SemanticMetadataMismatch {
        /// Mismatched role.
        role: CellPatchInputArtifactRole,
    },
    /// Direct dependencies differ from the exact role set.
    #[error("required {role} artifact has the wrong direct dependencies")]
    DependencyMismatch {
        /// Mismatched role.
        role: CellPatchInputArtifactRole,
    },
    /// Two roles illegitimately resolve to one artifact.
    #[error("cell-patch input artifact roles must be distinct")]
    RoleAlias,
    /// Decoded domain values disagree about one role.
    #[error("decoded cell-patch values disagree about the {role} binding")]
    DomainBindingMismatch {
        /// Inconsistent role.
        role: CellPatchInputArtifactRole,
    },
    /// Canonical domain bytes differ from managed content.
    #[error("required {role} artifact does not match the canonical domain payload")]
    PayloadIdentityMismatch {
        /// Mismatched role.
        role: CellPatchInputArtifactRole,
    },
    /// Full footprint raw or stock decode failed.
    #[error("patch footprint physical validation failed: {reason}")]
    FootprintPhysical {
        /// Redacted physical failure.
        reason: MultiscaleColumnarError,
    },
    /// A catalog record cannot be verified through the managed store.
    #[error("required {role} artifact is unavailable from the managed store: {reason:?}")]
    Unavailable {
        /// Unavailable role.
        role: CellPatchInputArtifactRole,
        /// Redacted availability category.
        reason: ArtifactAvailabilityFailure,
    },
}

/// Runtime-only proof of exact, vector-independent cell-patch inputs.
///
/// This proves the nine-role record graph, canonical expected-set/context/producer payloads,
/// managed integrity, and a full footprint raw-plus-stock decode. Opaque coordinate availability
/// does not prove anchor correspondence or promote a source adapter.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedCellPatchInputArtifactGraph {
    pub(crate) mode: CellPatchAssignmentMode,
    pub(crate) expected_cells_artifact_id: ArtifactId,
    pub(crate) expected_patches_artifact_id: ArtifactId,
    pub(crate) patch_context_artifact_id: ArtifactId,
    pub(crate) patch_footprints_artifact_id: ArtifactId,
    pub(crate) producer_artifact_id: ArtifactId,
    pub(crate) link_logical_digest: marklab_project::ContentDigest,
    pub(crate) assignment_count: u64,
    pub(crate) edge_count: u64,
}

impl fmt::Debug for VerifiedCellPatchInputArtifactGraph {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedCellPatchInputArtifactGraph")
            .field("mode", &self.mode)
            .field("assignment_count", &self.assignment_count)
            .field("edge_count", &self.edge_count)
            .finish_non_exhaustive()
    }
}

impl VerifiedCellPatchInputArtifactGraph {
    /// Verified assignment-row count.
    pub fn assignment_count(self) -> u64 {
        self.assignment_count
    }

    /// Verified edge-row count.
    pub fn edge_count(self) -> u64 {
        self.edge_count
    }

    /// Format-independent cell-patch logical identity.
    pub fn logical_digest(self) -> marklab_project::ContentDigest {
        self.link_logical_digest
    }

    /// Exact producer artifact identity.
    pub fn producer_artifact_id(self) -> ArtifactId {
        self.producer_artifact_id
    }
}

impl CellPatchLinkProducer {
    /// Validate the independent cell-patch input graph and fully decode its footprint artifact.
    ///
    /// # Errors
    ///
    /// Returns a role-only error for record, dependency, payload, domain, physical footprint, or
    /// managed availability drift. Opaque coordinate values, paths, IDs, and payload excerpts are
    /// never included.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_cell_patch_input_artifact_graph(
        &self,
        producer_artifact_id: ArtifactId,
        expected_cells: &ExpectedCellSet,
        expected_patches: &ExpectedPatchSet,
        context: &PatchEmbeddingContext,
        footprints: &PatchFootprintSet,
        link: &CellPatchLink,
        catalog: &ArtifactCatalog,
        store: &LocalArtifactStore,
        budgets: EmbeddingColumnarBudgets,
    ) -> Result<VerifiedCellPatchInputArtifactGraph, CellPatchInputArtifactGraphError> {
        validate_domain_bindings(
            self,
            producer_artifact_id,
            expected_cells,
            expected_patches,
            context,
            footprints,
            link,
        )?;
        let ids = role_ids(self, producer_artifact_id, link);
        validate_distinct_roles(&ids)?;
        for (role, id) in ids {
            require_profile(
                required_record(catalog, role, id)?,
                role,
                u64::try_from(footprints.row_count())
                    .map_err(|_| CellPatchInputArtifactGraphError::TableManifestMismatch)?,
            )?;
        }
        let producer_record = required_record(
            catalog,
            CellPatchInputArtifactRole::Producer,
            producer_artifact_id,
        )?;
        if producer_record.content().digest() != link.producer_content_digest() {
            return binding_mismatch(CellPatchInputArtifactRole::Producer);
        }
        validate_dependencies(self, link, catalog)?;
        require_expected_cells_payload(
            store,
            required_record(
                catalog,
                CellPatchInputArtifactRole::ExpectedCells,
                link.expected_cells_artifact_id(),
            )?,
            expected_cells,
        )?;
        require_json_payload(
            store,
            required_record(
                catalog,
                CellPatchInputArtifactRole::ExpectedPatches,
                link.expected_patches_artifact_id(),
            )?,
            CellPatchInputArtifactRole::ExpectedPatches,
            |reader| expected_patches.compare_canonical_json_reader(reader),
        )?;
        require_json_payload(
            store,
            required_record(
                catalog,
                CellPatchInputArtifactRole::PatchContext,
                link.patch_context_artifact_id(),
            )?,
            CellPatchInputArtifactRole::PatchContext,
            |reader| context.compare_canonical_json_reader(reader),
        )?;
        require_json_payload(
            store,
            required_record(
                catalog,
                CellPatchInputArtifactRole::Producer,
                producer_artifact_id,
            )?,
            CellPatchInputArtifactRole::Producer,
            |reader| self.compare_canonical_json_reader(reader),
        )?;
        for (role, id) in [
            (
                CellPatchInputArtifactRole::SourceCoordinates,
                self.source_coordinates_artifact_id(),
            ),
            (
                CellPatchInputArtifactRole::RunConfig,
                self.run_config_artifact_id(),
            ),
            (
                CellPatchInputArtifactRole::Environment,
                self.environment_artifact_id(),
            ),
            (
                CellPatchInputArtifactRole::Converter,
                self.converter_artifact_id(),
            ),
        ] {
            require_available(store, required_record(catalog, role, id)?, role)?;
        }
        let footprint_record = required_record(
            catalog,
            CellPatchInputArtifactRole::PatchFootprints,
            link.patch_footprints_artifact_id(),
        )?;
        validate_physical_footprints(
            store,
            footprint_record,
            expected_patches,
            context,
            footprints,
            budgets,
        )?;
        Ok(VerifiedCellPatchInputArtifactGraph {
            mode: link.mode(),
            expected_cells_artifact_id: link.expected_cells_artifact_id(),
            expected_patches_artifact_id: link.expected_patches_artifact_id(),
            patch_context_artifact_id: link.patch_context_artifact_id(),
            patch_footprints_artifact_id: link.patch_footprints_artifact_id(),
            producer_artifact_id,
            link_logical_digest: link.logical_digest(),
            assignment_count: u64::try_from(link.assignment_count()).map_err(|_| {
                CellPatchInputArtifactGraphError::DomainBindingMismatch {
                    role: CellPatchInputArtifactRole::ExpectedCells,
                }
            })?,
            edge_count: u64::try_from(link.edge_count()).map_err(|_| {
                CellPatchInputArtifactGraphError::DomainBindingMismatch {
                    role: CellPatchInputArtifactRole::PatchFootprints,
                }
            })?,
        })
    }
}

fn validate_domain_bindings(
    producer: &CellPatchLinkProducer,
    producer_artifact_id: ArtifactId,
    expected_cells: &ExpectedCellSet,
    expected_patches: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    link: &CellPatchLink,
) -> Result<(), CellPatchInputArtifactGraphError> {
    if link.expected_cells_logical_digest() != expected_cells.logical_digest()
        || link.assignment_count() != expected_cells.cells().len()
        || link
            .assignments()
            .iter()
            .zip(expected_cells.cells())
            .any(|(assignment, expected)| assignment.cell_id() != expected)
    {
        return binding_mismatch(CellPatchInputArtifactRole::ExpectedCells);
    }
    if link.expected_patches_artifact_id() != footprints.expected_patches_artifact_id()
        || link.expected_patches_logical_digest() != expected_patches.logical_digest()
        || footprints.expected_patches_logical_digest() != expected_patches.logical_digest()
        || expected_patches.owning_slide_id() != link.owning_slide_id()
    {
        return binding_mismatch(CellPatchInputArtifactRole::ExpectedPatches);
    }
    if link.patch_context_artifact_id() != footprints.patch_context_artifact_id()
        || link.patch_context_logical_digest() != context.logical_digest()
        || footprints.patch_context_logical_digest() != context.logical_digest()
        || context.owning_slide_id() != link.owning_slide_id()
    {
        return binding_mismatch(CellPatchInputArtifactRole::PatchContext);
    }
    if link.patch_footprints_logical_digest() != footprints.logical_digest() {
        return binding_mismatch(CellPatchInputArtifactRole::PatchFootprints);
    }
    if link.producer_artifact_id() != producer_artifact_id
        || link.mode() != producer.assignment_mode()
    {
        return binding_mismatch(CellPatchInputArtifactRole::Producer);
    }
    Ok(())
}

fn role_ids(
    producer: &CellPatchLinkProducer,
    producer_artifact_id: ArtifactId,
    link: &CellPatchLink,
) -> [(CellPatchInputArtifactRole, ArtifactId); 9] {
    [
        (
            CellPatchInputArtifactRole::ExpectedCells,
            link.expected_cells_artifact_id(),
        ),
        (
            CellPatchInputArtifactRole::ExpectedPatches,
            link.expected_patches_artifact_id(),
        ),
        (
            CellPatchInputArtifactRole::PatchContext,
            link.patch_context_artifact_id(),
        ),
        (
            CellPatchInputArtifactRole::PatchFootprints,
            link.patch_footprints_artifact_id(),
        ),
        (CellPatchInputArtifactRole::Producer, producer_artifact_id),
        (
            CellPatchInputArtifactRole::SourceCoordinates,
            producer.source_coordinates_artifact_id(),
        ),
        (
            CellPatchInputArtifactRole::RunConfig,
            producer.run_config_artifact_id(),
        ),
        (
            CellPatchInputArtifactRole::Environment,
            producer.environment_artifact_id(),
        ),
        (
            CellPatchInputArtifactRole::Converter,
            producer.converter_artifact_id(),
        ),
    ]
}

fn validate_distinct_roles(
    roles: &[(CellPatchInputArtifactRole, ArtifactId); 9],
) -> Result<(), CellPatchInputArtifactGraphError> {
    let mut ids = roles.map(|(_, id)| id);
    ids.sort_unstable();
    if ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(CellPatchInputArtifactGraphError::RoleAlias);
    }
    Ok(())
}

fn required_record(
    catalog: &ArtifactCatalog,
    role: CellPatchInputArtifactRole,
    id: ArtifactId,
) -> Result<&ArtifactRecord, CellPatchInputArtifactGraphError> {
    catalog
        .get(id)
        .ok_or(CellPatchInputArtifactGraphError::MissingRecord { role })
}

fn require_profile(
    record: &ArtifactRecord,
    role: CellPatchInputArtifactRole,
    footprint_rows: u64,
) -> Result<(), CellPatchInputArtifactGraphError> {
    if record.schema().id() != schema_id(role) || record.schema().version() != 1 {
        return Err(CellPatchInputArtifactGraphError::SchemaMismatch { role });
    }
    if !record.semantic_metadata().is_empty() {
        return Err(CellPatchInputArtifactGraphError::SemanticMetadataMismatch { role });
    }
    if role == CellPatchInputArtifactRole::PatchFootprints {
        if !matches!(
            record.content().kind(),
            "application/vnd.marklab.patch-footprint-table.v1+arrow"
                | "application/vnd.marklab.patch-footprint-table.v1+parquet"
        ) {
            return Err(CellPatchInputArtifactGraphError::ContentKindMismatch { role });
        }
        if !record_matches(record, SpatialArtifactRole::Footprint, footprint_rows) {
            return Err(CellPatchInputArtifactGraphError::TableManifestMismatch);
        }
    } else {
        if record.content().kind() != content_kind(role) {
            return Err(CellPatchInputArtifactGraphError::ContentKindMismatch { role });
        }
        if record.table().is_some() {
            return Err(CellPatchInputArtifactGraphError::UnexpectedTableManifest { role });
        }
    }
    Ok(())
}

fn schema_id(role: CellPatchInputArtifactRole) -> &'static str {
    match role {
        CellPatchInputArtifactRole::ExpectedCells => "marklab.cell_embedding_expected_cells",
        CellPatchInputArtifactRole::ExpectedPatches => "marklab.expected_patch_set",
        CellPatchInputArtifactRole::PatchContext => "marklab.patch_embedding_context",
        CellPatchInputArtifactRole::PatchFootprints => "marklab.patch_footprint_table",
        CellPatchInputArtifactRole::Producer => "marklab.cell_patch_link_producer",
        CellPatchInputArtifactRole::SourceCoordinates => "marklab.cell_anchor_source_coordinates",
        CellPatchInputArtifactRole::RunConfig => "marklab.embedding_run_config",
        CellPatchInputArtifactRole::Environment => "marklab.execution_environment",
        CellPatchInputArtifactRole::Converter => "marklab.converter_manifest",
    }
}

fn content_kind(role: CellPatchInputArtifactRole) -> &'static str {
    match role {
        CellPatchInputArtifactRole::ExpectedCells => {
            "application/vnd.marklab.embedding-expected-cells.v1"
        }
        CellPatchInputArtifactRole::ExpectedPatches => {
            "application/vnd.marklab.expected-patch-set.v1+json"
        }
        CellPatchInputArtifactRole::PatchContext => {
            "application/vnd.marklab.patch-embedding-context.v1+json"
        }
        CellPatchInputArtifactRole::Producer => {
            "application/vnd.marklab.cell-patch-link-producer.v1+json"
        }
        CellPatchInputArtifactRole::SourceCoordinates => {
            "application/vnd.marklab.cell-anchor-source-coordinates.v1+binary"
        }
        CellPatchInputArtifactRole::RunConfig
        | CellPatchInputArtifactRole::Environment
        | CellPatchInputArtifactRole::Converter => "application/json",
        CellPatchInputArtifactRole::PatchFootprints => "",
    }
}

fn validate_dependencies(
    producer: &CellPatchLinkProducer,
    link: &CellPatchLink,
    catalog: &ArtifactCatalog,
) -> Result<(), CellPatchInputArtifactGraphError> {
    for (role, id) in [
        (
            CellPatchInputArtifactRole::ExpectedCells,
            link.expected_cells_artifact_id(),
        ),
        (
            CellPatchInputArtifactRole::ExpectedPatches,
            link.expected_patches_artifact_id(),
        ),
        (
            CellPatchInputArtifactRole::PatchContext,
            link.patch_context_artifact_id(),
        ),
        (
            CellPatchInputArtifactRole::SourceCoordinates,
            producer.source_coordinates_artifact_id(),
        ),
        (
            CellPatchInputArtifactRole::RunConfig,
            producer.run_config_artifact_id(),
        ),
        (
            CellPatchInputArtifactRole::Environment,
            producer.environment_artifact_id(),
        ),
        (
            CellPatchInputArtifactRole::Converter,
            producer.converter_artifact_id(),
        ),
    ] {
        require_dependencies(required_record(catalog, role, id)?, role, &[])?;
    }
    let mut footprint_dependencies = [
        link.expected_patches_artifact_id(),
        link.patch_context_artifact_id(),
    ];
    footprint_dependencies.sort_unstable();
    require_dependencies(
        required_record(
            catalog,
            CellPatchInputArtifactRole::PatchFootprints,
            link.patch_footprints_artifact_id(),
        )?,
        CellPatchInputArtifactRole::PatchFootprints,
        &footprint_dependencies,
    )?;
    let mut producer_dependencies = [
        producer.source_coordinates_artifact_id(),
        producer.run_config_artifact_id(),
        producer.environment_artifact_id(),
        producer.converter_artifact_id(),
    ];
    producer_dependencies.sort_unstable();
    require_dependencies(
        required_record(
            catalog,
            CellPatchInputArtifactRole::Producer,
            link.producer_artifact_id(),
        )?,
        CellPatchInputArtifactRole::Producer,
        &producer_dependencies,
    )
}

fn require_dependencies(
    record: &ArtifactRecord,
    role: CellPatchInputArtifactRole,
    expected: &[ArtifactId],
) -> Result<(), CellPatchInputArtifactGraphError> {
    if record.dependencies() != expected {
        return Err(CellPatchInputArtifactGraphError::DependencyMismatch { role });
    }
    Ok(())
}

fn require_expected_cells_payload(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
) -> Result<(), CellPatchInputArtifactGraphError> {
    store
        .with_verified_reader(record, |reader| expected.compare_canonical_reader(reader))
        .map_err(|error| match error {
            VerifiedReaderError::Store(error) => CellPatchInputArtifactGraphError::Unavailable {
                role: CellPatchInputArtifactRole::ExpectedCells,
                reason: availability_failure(&error),
            },
            VerifiedReaderError::Callback(ExpectedCellReaderError::Mismatch) => {
                CellPatchInputArtifactGraphError::PayloadIdentityMismatch {
                    role: CellPatchInputArtifactRole::ExpectedCells,
                }
            }
            VerifiedReaderError::Callback(ExpectedCellReaderError::Read) => {
                CellPatchInputArtifactGraphError::Unavailable {
                    role: CellPatchInputArtifactRole::ExpectedCells,
                    reason: ArtifactAvailabilityFailure::StoreAccess,
                }
            }
        })
}

fn require_json_payload<F>(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    role: CellPatchInputArtifactRole,
    compare: F,
) -> Result<(), CellPatchInputArtifactGraphError>
where
    F: FnOnce(&mut dyn marklab_project::ArtifactReadSeek) -> Result<(), CanonicalJsonReaderError>,
{
    store
        .with_verified_reader(record, compare)
        .map_err(|error| match error {
            VerifiedReaderError::Store(error) => CellPatchInputArtifactGraphError::Unavailable {
                role,
                reason: availability_failure(&error),
            },
            VerifiedReaderError::Callback(CanonicalJsonReaderError::Mismatch) => {
                CellPatchInputArtifactGraphError::PayloadIdentityMismatch { role }
            }
            VerifiedReaderError::Callback(CanonicalJsonReaderError::Read) => {
                CellPatchInputArtifactGraphError::Unavailable {
                    role,
                    reason: ArtifactAvailabilityFailure::StoreAccess,
                }
            }
        })
}

fn require_available(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    role: CellPatchInputArtifactRole,
) -> Result<(), CellPatchInputArtifactGraphError> {
    store
        .with_verified_reader(record, |_reader| Ok::<(), Infallible>(()))
        .map_err(|error| match error {
            VerifiedReaderError::Store(error) => CellPatchInputArtifactGraphError::Unavailable {
                role,
                reason: availability_failure(&error),
            },
            VerifiedReaderError::Callback(error) => match error {},
        })
}

fn validate_physical_footprints(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), CellPatchInputArtifactGraphError> {
    let result = match record.content().kind() {
        "application/vnd.marklab.patch-footprint-table.v1+arrow" => {
            validate_patch_footprint_set_arrow_from_store(
                store, record, expected, context, footprints, budgets,
            )
            .map(|_| ())
        }
        "application/vnd.marklab.patch-footprint-table.v1+parquet" => {
            validate_patch_footprint_set_parquet_from_store(
                store, record, expected, context, footprints, budgets,
            )
            .map(|_| ())
        }
        _ => {
            return Err(CellPatchInputArtifactGraphError::ContentKindMismatch {
                role: CellPatchInputArtifactRole::PatchFootprints,
            })
        }
    };
    result.map_err(|error| match error {
        VerifiedReaderError::Store(error) => CellPatchInputArtifactGraphError::Unavailable {
            role: CellPatchInputArtifactRole::PatchFootprints,
            reason: availability_failure(&error),
        },
        VerifiedReaderError::Callback(reason) => {
            CellPatchInputArtifactGraphError::FootprintPhysical { reason }
        }
    })
}

fn binding_mismatch<T>(
    role: CellPatchInputArtifactRole,
) -> Result<T, CellPatchInputArtifactGraphError> {
    Err(CellPatchInputArtifactGraphError::DomainBindingMismatch { role })
}
