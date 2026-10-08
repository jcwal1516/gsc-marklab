mod error;
mod frame;
mod registry;
mod serial;
mod transform;

pub use error::CoordinateError;
pub use frame::{
    Coordinate2D, Coordinate3D, CoordinateFrame, CoordinateSpace, CoordinateUnit,
    ImageCoordinateConvention, SpatialAxis, SpatialDimension,
};
pub use registry::CoordinateRegistry;
pub use serial::{SerialSection, SerialSectionPlacement, SerialSectionSeries, SerialSectionStatus};
pub use transform::{FrameTransform, TransformMatrix, UncertaintyReference};
