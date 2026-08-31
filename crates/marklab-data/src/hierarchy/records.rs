use std::collections::HashMap;

use marklab_core::{HierarchyId, HierarchyKind};
use thiserror::Error;

use crate::{HierarchyNode, RepeatedMeasureSet, ReplicationRole, ReplicationRoleKind};

use super::{
    construction::{kind_index, lineage_contains},
    HIERARCHY_KIND_COUNT,
};

/// Immutable, validated object hierarchy for one analysis bundle.
#[derive(Debug)]
pub struct CohortHierarchy {
    pub(super) nodes: Box<[HierarchyNode]>,
    pub(super) index: HashMap<HierarchyId, usize>,
    pub(super) parent_indices: Box<[Option<usize>]>,
    pub(super) resolved_biological_units: Box<[Option<usize>]>,
    pub(super) biological_parent_indices: Box<[Option<usize>]>,
    pub(super) repeated_measure_sets: Box<[RepeatedMeasureSet]>,
    pub(super) summary: CohortDesignSummary,
}

impl CohortHierarchy {
    /// Whether the exact typed identity is present.
    pub fn contains(&self, id: &HierarchyId) -> bool {
        self.index.contains_key(id)
    }

    /// Return an immutable node declaration by typed identity.
    pub fn node(&self, id: &HierarchyId) -> Option<&HierarchyNode> {
        self.index.get(id).map(|index| &self.nodes[*index])
    }

    /// Resolve the nearest explicitly declared biological unit for an object.
    ///
    /// Use [`Self::belongs_to_biological_unit`] when selecting an enclosing
    /// declared level, such as a patient above a biological specimen.
    pub fn resolved_biological_unit(&self, id: &HierarchyId) -> Option<&HierarchyId> {
        let index = *self.index.get(id)?;
        self.resolved_biological_units[index].map(|source| self.nodes[source].id())
    }

    /// Whether an object belongs to the named biological unit at any declared level.
    ///
    /// Returns `false` when either ID is absent or the second ID is not declared
    /// as a biological unit. No level is selected implicitly.
    pub fn belongs_to_biological_unit(
        &self,
        id: &HierarchyId,
        biological_unit: &HierarchyId,
    ) -> bool {
        let Some(index) = self.index.get(id).copied() else {
            return false;
        };
        let Some(unit_index) = self.index.get(biological_unit).copied() else {
            return false;
        };
        if !matches!(
            self.nodes[unit_index].replication_role(),
            ReplicationRole::BiologicalUnit
        ) {
            return false;
        }
        lineage_contains(
            self.resolved_biological_units[index],
            unit_index,
            &self.biological_parent_indices,
        )
    }

    /// Explicit repeated-measure declarations in input order.
    pub fn repeated_measure_sets(&self) -> &[RepeatedMeasureSet] {
        &self.repeated_measure_sets
    }

    /// Factual counts and declared design characteristics.
    pub fn design_summary(&self) -> &CohortDesignSummary {
        &self.summary
    }

    /// Return the explicit parent of one object, if present.
    pub fn parent(&self, id: &HierarchyId) -> Option<&HierarchyId> {
        let index = *self.index.get(id)?;
        self.parent_indices[index].map(|parent| self.nodes[parent].id())
    }
}

/// Factual hierarchy and declared-design counts; this is not an inferential result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CohortDesignSummary {
    pub(super) object_counts: [usize; HIERARCHY_KIND_COUNT],
    pub(super) biological_unit_count: usize,
    pub(super) biological_subsample_count: usize,
    pub(super) technical_replicate_count: usize,
    pub(super) active_site_count: usize,
    pub(super) pair_set_count: usize,
    pub(super) repeated_set_count: usize,
    pub(super) multicore_biological_unit_count: usize,
    pub(super) multiregion_biological_unit_count: usize,
}

impl CohortDesignSummary {
    /// Number of objects declared for a kind.
    pub fn object_count(&self, kind: HierarchyKind) -> usize {
        self.object_counts[kind_index(kind)]
    }

    /// Number of objects explicitly declared as biological units.
    pub fn biological_unit_count(&self) -> usize {
        self.biological_unit_count
    }

    /// Number of objects explicitly declared as within-unit biological subsamples.
    pub fn biological_subsample_count(&self) -> usize {
        self.biological_subsample_count
    }

    /// Number of objects explicitly declared as technical replicates.
    pub fn technical_replicate_count(&self) -> usize {
        self.technical_replicate_count
    }

    /// Enrollment/cohort sites reached by at least one declared biological unit.
    pub fn active_site_count(&self) -> usize {
        self.active_site_count
    }

    /// Whether declared biological units reach more than one enrollment/cohort site.
    pub fn is_multisite(&self) -> bool {
        self.active_site_count > 1
    }

    /// Number of explicit repeated-measure sets with exactly two observations.
    pub fn pair_set_count(&self) -> usize {
        self.pair_set_count
    }

    /// Number of explicit repeated-measure sets with more than two observations.
    pub fn repeated_set_count(&self) -> usize {
        self.repeated_set_count
    }

    /// Biological units resolving at least two declared cores.
    pub fn multicore_biological_unit_count(&self) -> usize {
        self.multicore_biological_unit_count
    }

    /// Biological units resolving at least two non-nested regions.
    pub fn multiregion_biological_unit_count(&self) -> usize {
        self.multiregion_biological_unit_count
    }
}

/// Why an explicit biological-source relation is invalid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BiologicalSourceError {
    /// The source identity is not present.
    Missing,
    /// A node names itself as its source.
    SelfReference,
    /// The source exists but is not declared `BiologicalUnit`.
    NotBiological,
}

/// Why a shape-valid repeated-measure set cannot join this hierarchy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepeatedMeasureLinkError {
    /// The named biological unit does not exist.
    MissingBiologicalUnit,
    /// The named source exists but is not declared biological.
    SourceNotBiological,
    /// The same biological unit already owns a set of this observation kind.
    DuplicateSet,
    /// One observation does not exist.
    MissingObservation {
        /// Missing observation identity.
        observation: HierarchyId,
    },
    /// One observation already belongs to another declared set.
    DuplicateObservation {
        /// Reused observation identity.
        observation: HierarchyId,
    },
    /// One observation resolves to a different or absent biological unit.
    ObservationDoesNotResolve {
        /// Mismatched observation identity.
        observation: HierarchyId,
        /// Unit actually resolved by the hierarchy, if any.
        resolved: Option<HierarchyId>,
    },
}

/// Typed identity, parentage, source, and design-validation failures.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum HierarchyError {
    /// The bundle declares no biological unit at any level.
    #[error("cohort hierarchy must declare at least one biological unit")]
    NoBiologicalUnit,
    /// The same typed identity was declared more than once without a conflict.
    #[error("duplicate hierarchy identity {id}")]
    DuplicateId {
        /// Duplicated identity.
        id: HierarchyId,
    },
    /// The same identity was assigned two different parents.
    #[error("conflicting parents for {id}: {first_parent:?} versus {second_parent:?}")]
    ConflictingParent {
        /// Conflicting identity.
        id: HierarchyId,
        /// Parent in the first declaration.
        first_parent: Option<HierarchyId>,
        /// Parent in the later declaration.
        second_parent: Option<HierarchyId>,
    },
    /// The same identity was assigned two different replication roles.
    #[error("conflicting replication roles for {id}")]
    ConflictingReplicationRole {
        /// Conflicting identity.
        id: HierarchyId,
    },
    /// A required parent is omitted or a referenced parent is absent.
    #[error("missing parent for {child}: {parent:?}")]
    MissingParent {
        /// Child lacking a valid parent.
        child: HierarchyId,
        /// Missing referenced parent, or `None` when the relation was omitted.
        parent: Option<HierarchyId>,
    },
    /// The child and parent kinds do not form a supported relation.
    #[error("unsupported hierarchy nesting: {child} under {parent}")]
    UnsupportedNesting {
        /// Child identity.
        child: HierarchyId,
        /// Unsupported parent identity.
        parent: HierarchyId,
    },
    /// The compact parent graph contains a cycle.
    #[error("hierarchy contains a parent cycle involving {nodes:?}")]
    Cycle {
        /// Cycle nodes in deterministic traversal order.
        nodes: Vec<HierarchyId>,
    },
    /// This object kind cannot carry the declared replication role.
    #[error("unsupported replication role {role:?} for {node}")]
    UnsupportedReplicationRole {
        /// Object carrying the unsupported role.
        node: HierarchyId,
        /// Unsupported role category.
        role: ReplicationRoleKind,
    },
    /// A subsample or technical replicate has an invalid biological source.
    #[error("invalid biological source {biological_source} for {node}: {reason:?}")]
    InvalidBiologicalSource {
        /// Source-bearing node.
        node: HierarchyId,
        /// Invalid source identity.
        biological_source: HierarchyId,
        /// Exact source failure.
        reason: BiologicalSourceError,
    },
    /// A terminal observation branch has no declared biological unit.
    #[error("observation {observation} has no resolved biological unit")]
    UnresolvedBiologicalUnit {
        /// Unresolved terminal observation.
        observation: HierarchyId,
    },
    /// A repeated-measure declaration is inconsistent with the hierarchy.
    #[error("invalid repeated-measure set for {biological_unit}: {reason:?}")]
    InvalidRepeatedMeasureSet {
        /// Declared biological unit.
        biological_unit: HierarchyId,
        /// Exact membership/source failure.
        reason: RepeatedMeasureLinkError,
    },
}
