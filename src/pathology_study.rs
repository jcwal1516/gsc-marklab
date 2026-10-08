//! Native patient-replicated pathology studies with explicit observation support.

mod common;
mod composition;
mod publication;
mod scan;
mod spatial;

pub use composition::{analyze_pathology_composition, PathologyCompositionResult};
pub use publication::publish_pathology_composition;
pub use publication::publish_pathology_scan;
pub use publication::publish_pathology_spatial_study;
pub use scan::{analyze_pathology_scan, PathologyScanResult};
pub use spatial::{analyze_pathology_spatial_study, PathologySpatialStudyResult};
