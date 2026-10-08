use std::collections::{BTreeMap, BTreeSet};

use crate::{ContentDigest, WorkflowError};

/// Stable, path-safe workflow node identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeId(String);

impl NodeId {
    /// Validate a non-empty 1-128 byte ID using `[A-Za-z0-9_.-]`.
    pub fn new(value: impl Into<String>) -> Result<Self, WorkflowError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        {
            return Err(WorkflowError::InvalidNodeId { value });
        }
        Ok(Self(value))
    }

    /// Borrow the validated ID text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Versioned node specification and dependency declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeSpec {
    id: NodeId,
    kind: String,
    version: u32,
    dependencies: Vec<NodeId>,
}

impl NodeSpec {
    /// Validate a versioned node specification and canonicalize dependency order.
    pub fn new(
        id: NodeId,
        kind: impl Into<String>,
        version: u32,
        mut dependencies: Vec<NodeId>,
    ) -> Result<Self, WorkflowError> {
        let kind = kind.into();
        if kind.is_empty()
            || kind.len() > 128
            || !kind
                .as_bytes()
                .iter()
                .all(|byte| (0x21..=0x7e).contains(byte))
            || version == 0
        {
            return Err(WorkflowError::InvalidNodeSpec {
                node_id: id.0,
                reason: "kind must be 1-128 visible ASCII bytes and version must be positive"
                    .into(),
            });
        }
        dependencies.sort();
        if dependencies.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(WorkflowError::DuplicateDependency { node_id: id.0 });
        }
        Ok(Self {
            id,
            kind,
            version,
            dependencies,
        })
    }

    /// Node identity.
    pub fn id(&self) -> &NodeId {
        &self.id
    }

    /// Stable node-kind identifier.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Positive node implementation version.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Sorted direct dependencies.
    pub fn dependencies(&self) -> &[NodeId] {
        &self.dependencies
    }

    /// Deterministic digest of ID, kind, version, and sorted dependencies.
    pub fn digest(&self) -> ContentDigest {
        let version = self.version.to_be_bytes();
        let mut fields = vec![
            b"marklab-node-spec-v1".to_vec(),
            self.id.as_str().as_bytes().to_vec(),
            self.kind.as_bytes().to_vec(),
            version.to_vec(),
        ];
        for dependency in &self.dependencies {
            fields.push(dependency.as_str().as_bytes().to_vec());
        }
        ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
    }
}

/// A dependency-valid, acyclic workflow graph.
pub struct WorkflowGraph {
    nodes: BTreeMap<NodeId, NodeSpec>,
}

impl WorkflowGraph {
    /// Build a graph after rejecting duplicate IDs, invalid edges, and cycles.
    pub fn new(nodes: impl IntoIterator<Item = NodeSpec>) -> Result<Self, WorkflowError> {
        let mut by_id = BTreeMap::new();
        for node in nodes {
            let id = node.id.clone();
            if by_id.insert(id.clone(), node).is_some() {
                return Err(WorkflowError::DuplicateNode { node_id: id });
            }
        }

        for node in by_id.values() {
            for dependency in &node.dependencies {
                if dependency == &node.id {
                    return Err(WorkflowError::SelfDependency {
                        node_id: node.id.clone(),
                    });
                }
                if !by_id.contains_key(dependency) {
                    return Err(WorkflowError::MissingDependency {
                        node_id: node.id.clone(),
                        dependency: dependency.clone(),
                    });
                }
            }
        }

        validate_acyclic(&by_id)?;
        Ok(Self { nodes: by_id })
    }

    /// Find a validated node specification by ID.
    pub fn node(&self, id: &NodeId) -> Option<&NodeSpec> {
        self.nodes.get(id)
    }
}

fn validate_acyclic(nodes: &BTreeMap<NodeId, NodeSpec>) -> Result<(), WorkflowError> {
    let mut remaining_dependencies = nodes
        .iter()
        .map(|(id, spec)| (id.clone(), spec.dependencies.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<NodeId, Vec<NodeId>>::new();
    for (id, spec) in nodes {
        for dependency in &spec.dependencies {
            dependents
                .entry(dependency.clone())
                .or_default()
                .push(id.clone());
        }
    }
    let mut ready = remaining_dependencies
        .iter()
        .filter_map(|(id, count)| (*count == 0).then_some(id.clone()))
        .collect::<BTreeSet<_>>();
    let mut visited = 0;
    while let Some(id) = ready.pop_first() {
        visited += 1;
        if let Some(children) = dependents.get(&id) {
            for child in children {
                let count = remaining_dependencies
                    .get_mut(child)
                    .expect("validated dependent must exist");
                *count -= 1;
                if *count == 0 {
                    ready.insert(child.clone());
                }
            }
        }
    }
    if visited == nodes.len() {
        Ok(())
    } else {
        let cycle_nodes = remaining_dependencies
            .into_iter()
            .filter_map(|(id, count)| (count > 0).then_some(id))
            .collect();
        Err(WorkflowError::Cycle { nodes: cycle_nodes })
    }
}
