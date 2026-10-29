#![forbid(unsafe_code)]

mod fgw_common;
mod fused_gromov;
mod partial_fused_gromov;
mod partial_transport;
mod transport;
mod unbalanced_fused_gromov;

pub use fused_gromov::{
    FgwInitializationResult, FgwPlanEntry, FgwResources, FgwSupport, FusedGromovWassersteinSpec,
    FusedGromovWassersteinWorkerRequest, FusedGromovWassersteinWorkerResult,
};
pub use partial_fused_gromov::{
    PartialFgwFit, PartialFgwResources, PartialFusedGromovWassersteinSpec,
    PartialFusedGromovWassersteinWorkerRequest, PartialFusedGromovWassersteinWorkerResult,
};
pub use partial_transport::{
    PartialTransportOptimizer, PartialTransportPlanEntry, PartialTransportResources,
    PartialTransportSpec, PartialTransportWorkerRequest, PartialTransportWorkerResult,
};
pub use transport::{
    entropic_soft_assignment, sinkhorn_ot, unbalanced_sinkhorn, EntropicSoftAssignmentResult,
    EntropicSoftAssignmentSensitivity, EntropicSoftAssignmentSpec, SinkhornResult, SinkhornSpec,
    TransportError, TransportMass, TransportPlanEntry, UnbalancedSinkhornResult,
    UnbalancedSinkhornSpec,
};
pub use unbalanced_fused_gromov::{
    UnbalancedFgwFit, UnbalancedFgwObjective, UnbalancedFgwResources,
    UnbalancedFusedGromovWassersteinSpec, UnbalancedFusedGromovWassersteinWorkerRequest,
    UnbalancedFusedGromovWassersteinWorkerResult,
};
