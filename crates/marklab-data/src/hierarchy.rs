const HIERARCHY_KIND_COUNT: usize = 11;

mod construction;
mod records;

pub use records::{
    BiologicalSourceError, CohortDesignSummary, CohortHierarchy, HierarchyError,
    RepeatedMeasureLinkError,
};

#[cfg(test)]
mod tests;
