use crate::{NodeId, NodeSpec, WorkflowError, WorkflowGraph};

#[test]
fn graph_rejects_cycles_and_missing_dependencies() {
    let a = NodeId::new("a").expect("a");
    let b = NodeId::new("b").expect("b");
    let a_spec = NodeSpec::new(a.clone(), "test", 1, vec![b.clone()]).expect("a spec");
    let b_spec = NodeSpec::new(b.clone(), "test", 1, vec![a]).expect("b spec");
    assert!(matches!(
        WorkflowGraph::new([a_spec, b_spec]),
        Err(WorkflowError::Cycle { .. })
    ));

    let missing = NodeSpec::new(NodeId::new("root").expect("root"), "test", 1, vec![b])
        .expect("missing spec");
    assert!(matches!(
        WorkflowGraph::new([missing]),
        Err(WorkflowError::MissingDependency { .. })
    ));
}
