pub(crate) mod analysis;
mod types;

pub use analysis::analyze_compartment_interface_profile;
pub use types::{
    CompartmentInterfaceCell, CompartmentInterfaceError, CompartmentInterfaceLimits,
    CompartmentInterfaceProfile, CompartmentInterfaceSummary,
};
