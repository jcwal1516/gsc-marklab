use marklab_core::{HierarchyId, PatientId, RegionId};

use crate::{HierarchyNode, ReplicationRole};

use super::{CohortHierarchy, HierarchyError};

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
