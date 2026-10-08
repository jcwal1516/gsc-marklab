use std::collections::{HashMap, HashSet};

use marklab_core::{HierarchyId, HierarchyKind};

use crate::{HierarchyNode, RepeatedMeasureSet, ReplicationRole, ReplicationRoleKind};

use super::{
    records::{
        BiologicalSourceError, CohortDesignSummary, CohortHierarchy, HierarchyError,
        RepeatedMeasureLinkError,
    },
    HIERARCHY_KIND_COUNT,
};

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

pub(super) fn lineage_contains(
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

pub(super) fn kind_index(kind: HierarchyKind) -> usize {
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
