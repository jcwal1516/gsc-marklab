#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Minimal typed DAG and single-node local scheduler for Marklab.

mod error;
mod execution;
mod graph;
mod node;
mod scheduler;

pub use error::WorkflowError;
pub use execution::{execute_algorithm, execute_algorithm_with_store, ExecuteAlgorithmError};
pub use graph::{NodeId, NodeSpec, WorkflowGraph};
pub use node::{CacheKeyMaterial, NodeError, WorkflowNode};
pub use scheduler::{CacheStatus, LocalScheduler, NodeRun, SchedulerLimits};

#[cfg(test)]
mod tests;
