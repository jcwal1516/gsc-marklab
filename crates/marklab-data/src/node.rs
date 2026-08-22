use std::collections::HashSet;

use marklab_core::{HierarchyId, HierarchyKind};
use thiserror::Error;

/// Declared relationship of one hierarchy object to biological replication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplicationRole {
    /// A declared eligible biological unit; downstream inference still selects its unit level.
    BiologicalUnit,
    /// A within-unit biological subsample, such as a donor TMA core.
    BiologicalSubsample {
        /// Explicit biological unit from which the subsample derives.
        biological_source: HierarchyId,
    },
    /// A repeated technical measurement that is never an independent biological unit.
    TechnicalReplicate {
        /// Explicit biological unit measured by this replicate.
        biological_source: HierarchyId,
    },
    /// A structural or contextual object that inherits the nearest declared source.
    Structural,
}

/// Stable category of a replication role, excluding any referenced source ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplicationRoleKind {
    /// An eligible biological unit.
    BiologicalUnit,
    /// A within-unit biological subsample.
    BiologicalSubsample,
    /// A technical replicate.
    TechnicalReplicate,
    /// A structural or contextual object.
    Structural,
}

impl ReplicationRole {
    /// Return this role's category without exposing or comparing its source ID.
    pub fn kind(&self) -> ReplicationRoleKind {
        match self {
            Self::BiologicalUnit => ReplicationRoleKind::BiologicalUnit,
            Self::BiologicalSubsample { .. } => ReplicationRoleKind::BiologicalSubsample,
            Self::TechnicalReplicate { .. } => ReplicationRoleKind::TechnicalReplicate,
            Self::Structural => ReplicationRoleKind::Structural,
        }
    }

    pub(crate) fn biological_source(&self) -> Option<&HierarchyId> {
        match self {
            Self::BiologicalSubsample { biological_source }
            | Self::TechnicalReplicate { biological_source } => Some(biological_source),
            Self::BiologicalUnit | Self::Structural => None,
        }
    }
}

/// One explicitly identified hierarchy object and its compact parent relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HierarchyNode {
    id: HierarchyId,
    containment_parent: Option<HierarchyId>,
    role: ReplicationRole,
}

impl HierarchyNode {
    /// Declare a node without deriving any field from its ID text.
    pub fn new(
        id: HierarchyId,
        containment_parent: Option<HierarchyId>,
        role: ReplicationRole,
    ) -> Self {
        Self {
            id,
            containment_parent,
            role,
        }
    }

    /// Typed object identity.
    pub fn id(&self) -> &HierarchyId {
        &self.id
    }

    /// Explicit containment or lineage parent, if this kind may be a root.
    pub fn containment_parent(&self) -> Option<&HierarchyId> {
        self.containment_parent.as_ref()
    }

    /// Explicit biological, subsample, technical, or structural role.
    pub fn replication_role(&self) -> &ReplicationRole {
        &self.role
    }
}

/// Explicit repeated observations belonging to one declared biological unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepeatedMeasureSet {
    biological_unit: HierarchyId,
    observations: Box<[HierarchyId]>,
}

impl RepeatedMeasureSet {
    /// Validate the shape of a repeated-measure declaration.
    ///
    /// Existence and biological-source resolution are validated when the set is
    /// installed into a `CohortHierarchy`.
    pub fn new(
        biological_unit: HierarchyId,
        observations: Vec<HierarchyId>,
    ) -> Result<Self, RepeatedMeasureError> {
        if observations.len() < 2 {
            return Err(RepeatedMeasureError::TooFewObservations {
                observed: observations.len(),
            });
        }
        let expected = observations[0].kind();
        if !supports_repeated_observation(expected) {
            return Err(RepeatedMeasureError::UnsupportedObservationKind { kind: expected });
        }
        let mut unique = HashSet::with_capacity(observations.len());
        for observation in &observations {
            if observation.kind() != expected {
                return Err(RepeatedMeasureError::MixedObservationKinds {
                    expected,
                    observed: observation.kind(),
                });
            }
            if !unique.insert(observation.clone()) {
                return Err(RepeatedMeasureError::DuplicateObservation {
                    observation: observation.clone(),
                });
            }
        }
        Ok(Self {
            biological_unit,
            observations: observations.into_boxed_slice(),
        })
    }

    /// Biological unit shared by the declared observations.
    pub fn biological_unit(&self) -> &HierarchyId {
        &self.biological_unit
    }

    /// Distinct, same-kind observation IDs in deterministic declaration order.
    ///
    /// The retained order has no temporal or condition meaning; those semantics
    /// require future explicit metadata rather than interpretation of ID text.
    pub fn observations(&self) -> &[HierarchyId] {
        &self.observations
    }
}

/// Shape errors detected before a repeated-measure set sees a hierarchy.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RepeatedMeasureError {
    /// A repeated set must contain at least two observations.
    #[error("repeated-measure set has {observed} observation(s); at least two are required")]
    TooFewObservations {
        /// Observed member count.
        observed: usize,
    },
    /// One observation appears more than once in the set.
    #[error("repeated-measure set repeats observation {observation}")]
    DuplicateObservation {
        /// Repeated observation ID.
        observation: HierarchyId,
    },
    /// Observation kinds differ within one set.
    #[error("repeated-measure observations must all be {expected}, observed {observed}")]
    MixedObservationKinds {
        /// Kind established by the first member.
        expected: HierarchyKind,
        /// Conflicting member kind.
        observed: HierarchyKind,
    },
    /// This object kind cannot be a repeated observation in C-01.
    #[error("{kind} cannot be a repeated-measure observation")]
    UnsupportedObservationKind {
        /// Unsupported object kind.
        kind: HierarchyKind,
    },
}

fn supports_repeated_observation(kind: HierarchyKind) -> bool {
    matches!(
        kind,
        HierarchyKind::Specimen
            | HierarchyKind::Slide
            | HierarchyKind::Section
            | HierarchyKind::Core
            | HierarchyKind::Region
    )
}
