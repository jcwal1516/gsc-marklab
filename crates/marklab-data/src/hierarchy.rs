use std::collections::{HashMap, HashSet};

use marklab_core::{HierarchyId, HierarchyKind};
use thiserror::Error;

use crate::{HierarchyNode, RepeatedMeasureSet, ReplicationRole, ReplicationRoleKind};

const HIERARCHY_KIND_COUNT: usize = 11;

/// Immutable, validated object hierarchy for one analysis bundle.
#[derive(Debug)]
pub struct CohortHierarchy {
    nodes: Box<[HierarchyNode]>,
    index: HashMap<HierarchyId, usize>,
    parent_indices: Box<[Option<usize>]>,
    resolved_biological_units: Box<[Option<usize>]>,
    biological_parent_indices: Box<[Option<usize>]>,
    repeated_measure_sets: Box<[RepeatedMeasureSet]>,
    summary: CohortDesignSummary,
}

impl CohortHierarchy {
    /// Validate explicit nodes and repeated-measure declarations without inference.
    pub fn new(
        nodes: Vec<HierarchyNode>,
        repeated_measure_sets: Vec<RepeatedMeasureSet>,
    ) -> Result<Self, HierarchyError> {
        let index = index_unique_nodes(&nodes)?;
        validate_parent_presence(&nodes, &index)?;
        validate_parent_kinds(&nodes)?;
        let parent_indices = parent_indices(&nodes, &index);
        reject_cycles(&nodes, &parent_indices)?;
        validate_replication_roles(&nodes)?;
        let source_indices = validate_biological_sources(&nodes, &index)?;
        if !nodes
            .iter()
            .any(|node| matches!(node.replication_role(), ReplicationRole::BiologicalUnit))
        {
            return Err(HierarchyError::NoBiologicalUnit);
        }
        let resolved_biological_units =
            resolve_biological_units(&nodes, &parent_indices, &source_indices);
        let biological_parent_indices =
            biological_parent_indices(&nodes, &parent_indices, &resolved_biological_units);
        validate_terminal_resolution(&nodes, &parent_indices, &resolved_biological_units)?;
        validate_repeated_measure_sets(
            &nodes,
            &index,
            &resolved_biological_units,
            &biological_parent_indices,
            &repeated_measure_sets,
        )?;
        let summary = summarize(
            &nodes,
            &parent_indices,
            &resolved_biological_units,
            &biological_parent_indices,
            &repeated_measure_sets,
        );

        Ok(Self {
            nodes: nodes.into_boxed_slice(),
            index,
            parent_indices: parent_indices.into_boxed_slice(),
            resolved_biological_units: resolved_biological_units.into_boxed_slice(),
            biological_parent_indices: biological_parent_indices.into_boxed_slice(),
            repeated_measure_sets: repeated_measure_sets.into_boxed_slice(),
            summary,
        })
    }

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
    object_counts: [usize; HIERARCHY_KIND_COUNT],
    biological_unit_count: usize,
    biological_subsample_count: usize,
    technical_replicate_count: usize,
    active_site_count: usize,
    pair_set_count: usize,
    repeated_set_count: usize,
    multicore_biological_unit_count: usize,
    multiregion_biological_unit_count: usize,
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

fn index_unique_nodes(
    nodes: &[HierarchyNode],
) -> Result<HashMap<HierarchyId, usize>, HierarchyError> {
    let mut index = HashMap::<HierarchyId, usize>::with_capacity(nodes.len());
    for (position, node) in nodes.iter().enumerate() {
        if let Some(existing_position) = index.get(node.id()).copied() {
            let existing = &nodes[existing_position];
            if existing.containment_parent() != node.containment_parent() {
                return Err(HierarchyError::ConflictingParent {
                    id: node.id().clone(),
                    first_parent: existing.containment_parent().cloned(),
                    second_parent: node.containment_parent().cloned(),
                });
            }
            if existing.replication_role() != node.replication_role() {
                return Err(HierarchyError::ConflictingReplicationRole {
                    id: node.id().clone(),
                });
            }
            return Err(HierarchyError::DuplicateId {
                id: node.id().clone(),
            });
        }
        index.insert(node.id().clone(), position);
    }
    Ok(index)
}

fn validate_parent_presence(
    nodes: &[HierarchyNode],
    index: &HashMap<HierarchyId, usize>,
) -> Result<(), HierarchyError> {
    for node in nodes {
        match node.containment_parent() {
            Some(parent) if !index.contains_key(parent) => {
                return Err(HierarchyError::MissingParent {
                    child: node.id().clone(),
                    parent: Some(parent.clone()),
                });
            }
            None if requires_parent(node.id().kind()) => {
                return Err(HierarchyError::MissingParent {
                    child: node.id().clone(),
                    parent: None,
                });
            }
            Some(_) | None => {}
        }
    }
    Ok(())
}

fn validate_parent_kinds(nodes: &[HierarchyNode]) -> Result<(), HierarchyError> {
    for node in nodes {
        let Some(parent) = node.containment_parent() else {
            continue;
        };
        if !supports_parent(node.id().kind(), parent.kind()) {
            return Err(HierarchyError::UnsupportedNesting {
                child: node.id().clone(),
                parent: parent.clone(),
            });
        }
    }
    Ok(())
}

fn parent_indices(
    nodes: &[HierarchyNode],
    index: &HashMap<HierarchyId, usize>,
) -> Vec<Option<usize>> {
    nodes
        .iter()
        .map(|node| node.containment_parent().map(|parent| index[parent]))
        .collect()
}

fn reject_cycles(
    nodes: &[HierarchyNode],
    parent_indices: &[Option<usize>],
) -> Result<(), HierarchyError> {
    let mut state = vec![0_u8; nodes.len()];
    let mut path_position = vec![usize::MAX; nodes.len()];
    for start in 0..nodes.len() {
        if state[start] == 2 {
            continue;
        }
        let mut path = Vec::new();
        let mut current = start;
        loop {
            match state[current] {
                0 => {
                    state[current] = 1;
                    path_position[current] = path.len();
                    path.push(current);
                    let Some(parent) = parent_indices[current] else {
                        break;
                    };
                    current = parent;
                }
                1 => {
                    let offset = path_position[current];
                    return Err(HierarchyError::Cycle {
                        nodes: path[offset..]
                            .iter()
                            .map(|index| nodes[*index].id().clone())
                            .collect(),
                    });
                }
                2 => break,
                _ => unreachable!("cycle state is internal and bounded"),
            }
        }
        for index in path {
            state[index] = 2;
            path_position[index] = usize::MAX;
        }
    }
    Ok(())
}

fn validate_replication_roles(nodes: &[HierarchyNode]) -> Result<(), HierarchyError> {
    for node in nodes {
        let role = node.replication_role().kind();
        if !supports_replication_role(node.id().kind(), role) {
            return Err(HierarchyError::UnsupportedReplicationRole {
                node: node.id().clone(),
                role,
            });
        }
    }
    Ok(())
}

fn validate_biological_sources(
    nodes: &[HierarchyNode],
    index: &HashMap<HierarchyId, usize>,
) -> Result<Vec<Option<usize>>, HierarchyError> {
    let mut sources = Vec::with_capacity(nodes.len());
    for node in nodes {
        let Some(source) = node.replication_role().biological_source() else {
            sources.push(None);
            continue;
        };
        if source == node.id() {
            return Err(HierarchyError::InvalidBiologicalSource {
                node: node.id().clone(),
                biological_source: source.clone(),
                reason: BiologicalSourceError::SelfReference,
            });
        }
        let Some(source_index) = index.get(source).copied() else {
            return Err(HierarchyError::InvalidBiologicalSource {
                node: node.id().clone(),
                biological_source: source.clone(),
                reason: BiologicalSourceError::Missing,
            });
        };
        if !matches!(
            nodes[source_index].replication_role(),
            ReplicationRole::BiologicalUnit
        ) {
            return Err(HierarchyError::InvalidBiologicalSource {
                node: node.id().clone(),
                biological_source: source.clone(),
                reason: BiologicalSourceError::NotBiological,
            });
        }
        sources.push(Some(source_index));
    }
    Ok(sources)
}

fn resolve_biological_units(
    nodes: &[HierarchyNode],
    parent_indices: &[Option<usize>],
    source_indices: &[Option<usize>],
) -> Vec<Option<usize>> {
    let mut resolved = vec![None::<Option<usize>>; nodes.len()];
    for start in 0..nodes.len() {
        if resolved[start].is_some() {
            continue;
        }
        let mut path = Vec::new();
        let mut current = start;
        let result = loop {
            if let Some(result) = resolved[current] {
                break result;
            }
            path.push(current);
            if matches!(
                nodes[current].replication_role(),
                ReplicationRole::BiologicalUnit
            ) {
                break Some(current);
            }
            if let Some(source) = source_indices[current] {
                break Some(source);
            }
            let Some(parent) = parent_indices[current] else {
                break None;
            };
            current = parent;
        };
        for index in path {
            resolved[index] = Some(result);
        }
    }
    resolved
        .into_iter()
        .map(|value| value.expect("every node is traversed"))
        .collect()
}

fn biological_parent_indices(
    nodes: &[HierarchyNode],
    parent_indices: &[Option<usize>],
    resolved: &[Option<usize>],
) -> Vec<Option<usize>> {
    nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            if matches!(node.replication_role(), ReplicationRole::BiologicalUnit) {
                parent_indices[index].and_then(|parent| resolved[parent])
            } else {
                None
            }
        })
        .collect()
}

fn lineage_contains(
    nearest_unit: Option<usize>,
    requested_unit: usize,
    biological_parent_indices: &[Option<usize>],
) -> bool {
    let mut current = nearest_unit;
    while let Some(unit) = current {
        if unit == requested_unit {
            return true;
        }
        current = biological_parent_indices[unit];
    }
    false
}

fn validate_terminal_resolution(
    nodes: &[HierarchyNode],
    parent_indices: &[Option<usize>],
    resolved: &[Option<usize>],
) -> Result<(), HierarchyError> {
    let mut child_counts = vec![0_usize; nodes.len()];
    for parent in parent_indices.iter().flatten() {
        child_counts[*parent] += 1;
    }
    for (index, node) in nodes.iter().enumerate() {
        if child_counts[index] == 0
            && !matches!(
                node.id().kind(),
                HierarchyKind::Site | HierarchyKind::Timepoint
            )
            && resolved[index].is_none()
        {
            return Err(HierarchyError::UnresolvedBiologicalUnit {
                observation: node.id().clone(),
            });
        }
    }
    Ok(())
}

fn validate_repeated_measure_sets(
    nodes: &[HierarchyNode],
    index: &HashMap<HierarchyId, usize>,
    resolved: &[Option<usize>],
    biological_parent_indices: &[Option<usize>],
    sets: &[RepeatedMeasureSet],
) -> Result<(), HierarchyError> {
    let mut seen_sets = HashSet::<(usize, HierarchyKind)>::with_capacity(sets.len());
    let mut seen_observations = HashSet::new();
    for set in sets {
        let unit = set.biological_unit();
        let Some(unit_index) = index.get(unit).copied() else {
            return Err(invalid_repeated(
                unit,
                RepeatedMeasureLinkError::MissingBiologicalUnit,
            ));
        };
        if !matches!(
            nodes[unit_index].replication_role(),
            ReplicationRole::BiologicalUnit
        ) {
            return Err(invalid_repeated(
                unit,
                RepeatedMeasureLinkError::SourceNotBiological,
            ));
        }
        let mut observation_indices = Vec::with_capacity(set.observations().len());
        for observation in set.observations() {
            let Some(observation_index) = index.get(observation).copied() else {
                return Err(invalid_repeated(
                    unit,
                    RepeatedMeasureLinkError::MissingObservation {
                        observation: observation.clone(),
                    },
                ));
            };
            observation_indices.push(observation_index);
        }
        let observation_kind = set.observations()[0].kind();
        if !seen_sets.insert((unit_index, observation_kind)) {
            return Err(invalid_repeated(
                unit,
                RepeatedMeasureLinkError::DuplicateSet,
            ));
        }
        for (observation, observation_index) in set
            .observations()
            .iter()
            .zip(observation_indices.iter().copied())
        {
            if !seen_observations.insert(observation_index) {
                return Err(invalid_repeated(
                    unit,
                    RepeatedMeasureLinkError::DuplicateObservation {
                        observation: observation.clone(),
                    },
                ));
            }
        }
        for (observation, observation_index) in set
            .observations()
            .iter()
            .zip(observation_indices.iter().copied())
        {
            if !lineage_contains(
                resolved[observation_index],
                unit_index,
                biological_parent_indices,
            ) {
                return Err(invalid_repeated(
                    unit,
                    RepeatedMeasureLinkError::ObservationDoesNotResolve {
                        observation: observation.clone(),
                        resolved: resolved[observation_index]
                            .map(|index| nodes[index].id().clone()),
                    },
                ));
            }
        }
    }
    Ok(())
}

fn invalid_repeated(
    biological_unit: &HierarchyId,
    reason: RepeatedMeasureLinkError,
) -> HierarchyError {
    HierarchyError::InvalidRepeatedMeasureSet {
        biological_unit: biological_unit.clone(),
        reason,
    }
}

fn summarize(
    nodes: &[HierarchyNode],
    parent_indices: &[Option<usize>],
    resolved: &[Option<usize>],
    biological_parent_indices: &[Option<usize>],
    sets: &[RepeatedMeasureSet],
) -> CohortDesignSummary {
    let mut object_counts = [0_usize; HIERARCHY_KIND_COUNT];
    let mut biological_unit_count = 0;
    let mut biological_subsample_count = 0;
    let mut technical_replicate_count = 0;
    for node in nodes {
        object_counts[kind_index(node.id().kind())] += 1;
        match node.replication_role() {
            ReplicationRole::BiologicalUnit => biological_unit_count += 1,
            ReplicationRole::BiologicalSubsample { .. } => biological_subsample_count += 1,
            ReplicationRole::TechnicalReplicate { .. } => technical_replicate_count += 1,
            ReplicationRole::Structural => {}
        }
    }

    let nearest_sites = resolve_nearest_sites(nodes, parent_indices);
    let active_site_count = nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| matches!(node.replication_role(), ReplicationRole::BiologicalUnit))
        .filter_map(|(index, _)| nearest_sites[index])
        .collect::<HashSet<_>>()
        .len();

    let mut cores_per_unit = HashMap::<usize, usize>::new();
    let mut top_regions_per_unit = HashMap::<usize, usize>::new();
    for (index, node) in nodes.iter().enumerate() {
        let Some(unit) = resolved[index] else {
            continue;
        };
        let counter = match node.id().kind() {
            HierarchyKind::Core => Some(&mut cores_per_unit),
            HierarchyKind::Region
                if parent_indices[index]
                    .is_some_and(|parent| nodes[parent].id().kind() != HierarchyKind::Region) =>
            {
                Some(&mut top_regions_per_unit)
            }
            _ => None,
        };
        if let Some(counter) = counter {
            // The role-kind matrix bounds biological containment to patient -> specimen,
            // so expanding each nearest unit through its ancestry remains linear.
            let mut current = Some(unit);
            while let Some(biological_unit) = current {
                *counter.entry(biological_unit).or_default() += 1;
                current = biological_parent_indices[biological_unit];
            }
        }
    }

    CohortDesignSummary {
        object_counts,
        biological_unit_count,
        biological_subsample_count,
        technical_replicate_count,
        active_site_count,
        pair_set_count: sets
            .iter()
            .filter(|set| set.observations().len() == 2)
            .count(),
        repeated_set_count: sets
            .iter()
            .filter(|set| set.observations().len() > 2)
            .count(),
        multicore_biological_unit_count: cores_per_unit
            .values()
            .filter(|count| **count >= 2)
            .count(),
        multiregion_biological_unit_count: top_regions_per_unit
            .values()
            .filter(|count| **count >= 2)
            .count(),
    }
}

fn resolve_nearest_sites(
    nodes: &[HierarchyNode],
    parent_indices: &[Option<usize>],
) -> Vec<Option<usize>> {
    let mut resolved = vec![None::<Option<usize>>; nodes.len()];
    for start in 0..nodes.len() {
        if resolved[start].is_some() {
            continue;
        }
        let mut path = Vec::new();
        let mut current = start;
        let result = loop {
            if let Some(result) = resolved[current] {
                break result;
            }
            path.push(current);
            if nodes[current].id().kind() == HierarchyKind::Site {
                break Some(current);
            }
            let Some(parent) = parent_indices[current] else {
                break None;
            };
            current = parent;
        };
        for index in path {
            resolved[index] = Some(result);
        }
    }
    resolved
        .into_iter()
        .map(|value| value.expect("every node is traversed"))
        .collect()
}

fn requires_parent(kind: HierarchyKind) -> bool {
    !matches!(
        kind,
        HierarchyKind::Site | HierarchyKind::Patient | HierarchyKind::Slide
    )
}

fn supports_parent(child: HierarchyKind, parent: HierarchyKind) -> bool {
    match child {
        HierarchyKind::Site => false,
        HierarchyKind::Patient => parent == HierarchyKind::Site,
        HierarchyKind::Timepoint => parent == HierarchyKind::Patient,
        HierarchyKind::Specimen => {
            matches!(parent, HierarchyKind::Patient | HierarchyKind::Timepoint)
        }
        HierarchyKind::Block => parent == HierarchyKind::Specimen,
        HierarchyKind::Slide => parent == HierarchyKind::Block,
        HierarchyKind::Section => parent == HierarchyKind::Slide,
        HierarchyKind::Core => {
            matches!(parent, HierarchyKind::Slide | HierarchyKind::Section)
        }
        HierarchyKind::Region => matches!(
            parent,
            HierarchyKind::Slide
                | HierarchyKind::Section
                | HierarchyKind::Core
                | HierarchyKind::Region
        ),
        HierarchyKind::Cell | HierarchyKind::Patch => matches!(
            parent,
            HierarchyKind::Slide
                | HierarchyKind::Section
                | HierarchyKind::Core
                | HierarchyKind::Region
        ),
    }
}

fn supports_replication_role(kind: HierarchyKind, role: ReplicationRoleKind) -> bool {
    match kind {
        HierarchyKind::Site
        | HierarchyKind::Timepoint
        | HierarchyKind::Cell
        | HierarchyKind::Patch => role == ReplicationRoleKind::Structural,
        HierarchyKind::Patient => matches!(
            role,
            ReplicationRoleKind::BiologicalUnit | ReplicationRoleKind::Structural
        ),
        HierarchyKind::Specimen => true,
        HierarchyKind::Block
        | HierarchyKind::Slide
        | HierarchyKind::Section
        | HierarchyKind::Core
        | HierarchyKind::Region => matches!(
            role,
            ReplicationRoleKind::BiologicalSubsample
                | ReplicationRoleKind::TechnicalReplicate
                | ReplicationRoleKind::Structural
        ),
    }
}

fn kind_index(kind: HierarchyKind) -> usize {
    match kind {
        HierarchyKind::Site => 0,
        HierarchyKind::Patient => 1,
        HierarchyKind::Timepoint => 2,
        HierarchyKind::Specimen => 3,
        HierarchyKind::Block => 4,
        HierarchyKind::Slide => 5,
        HierarchyKind::Section => 6,
        HierarchyKind::Core => 7,
        HierarchyKind::Region => 8,
        HierarchyKind::Cell => 9,
        HierarchyKind::Patch => 10,
    }
}

#[cfg(test)]
mod tests {
    use marklab_core::{PatientId, RegionId};

    use super::*;

    fn patient(value: &str) -> HierarchyId {
        PatientId::new(value).expect("patient").into()
    }

    fn region(value: &str) -> HierarchyId {
        RegionId::new(value).expect("region").into()
    }

    #[test]
    fn nested_region_cycle_is_deterministic() {
        let a = region("a");
        let b = region("b");
        let error = CohortHierarchy::new(
            vec![
                HierarchyNode::new(patient("p"), None, ReplicationRole::BiologicalUnit),
                HierarchyNode::new(a.clone(), Some(b.clone()), ReplicationRole::Structural),
                HierarchyNode::new(b.clone(), Some(a.clone()), ReplicationRole::Structural),
            ],
            Vec::new(),
        )
        .expect_err("cycle");
        assert!(matches!(
            error,
            HierarchyError::Cycle { nodes } if nodes == vec![a, b]
        ));
    }
}
