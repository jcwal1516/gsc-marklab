use marklab_core::CoordinateFrameId;

use super::CoordinateError;

/// Semantic spatial axis carried by a coordinate frame.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SpatialAxis {
    /// X spatial axis.
    X,
    /// Y spatial axis.
    Y,
    /// Z spatial axis.
    Z,
}

/// Supported spatial dimensionality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialDimension {
    /// Two spatial axes.
    Two,
    /// Three spatial axes.
    Three,
}

/// Unit shared by the spatial axes of one coordinate frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinateUnit {
    /// Continuous pixel or voxel coordinates.
    Pixel,
    /// Nanometres.
    Nanometer,
    /// Micrometres.
    Micrometer,
    /// Millimetres.
    Millimeter,
}

impl CoordinateUnit {
    /// Whether this is a physical length unit rather than a pixel unit.
    pub fn is_physical(self) -> bool {
        !matches!(self, Self::Pixel)
    }
}

/// Relationship between integer image indices and continuous pixel coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageCoordinateConvention {
    /// The sample centered at index zero has continuous coordinate zero.
    PixelCenterAtInteger,
    /// Integer coordinates denote pixel corners; the first center is at 0.5.
    PixelCornerAtInteger,
}

/// Whether a frame describes image pixels or physical space.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinateSpace {
    /// Image coordinates with an explicit index-to-continuous convention.
    Image(ImageCoordinateConvention),
    /// Physical spatial coordinates.
    Physical,
}

/// One explicitly named, ordered 2-D or 3-D spatial coordinate frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoordinateFrame {
    id: CoordinateFrameId,
    axes: Box<[SpatialAxis]>,
    dimension: SpatialDimension,
    unit: CoordinateUnit,
    space: CoordinateSpace,
}

/// A finite two-dimensional coordinate whose values follow its frame's axis order.
#[derive(Clone, Debug, PartialEq)]
pub struct Coordinate2D {
    frame: CoordinateFrameId,
    values: [f64; 2],
}

impl Coordinate2D {
    pub(crate) fn new(frame: CoordinateFrameId, values: [f64; 2]) -> Self {
        Self { frame, values }
    }

    /// Frame in which the coordinate is expressed.
    pub fn frame(&self) -> &CoordinateFrameId {
        &self.frame
    }

    /// Coordinate values in the frame's exact declared axis order.
    pub fn values(&self) -> &[f64; 2] {
        &self.values
    }
}

/// A finite three-dimensional coordinate whose values follow its frame's axis order.
#[derive(Clone, Debug, PartialEq)]
pub struct Coordinate3D {
    frame: CoordinateFrameId,
    values: [f64; 3],
}

impl Coordinate3D {
    pub(crate) fn new(frame: CoordinateFrameId, values: [f64; 3]) -> Self {
        Self { frame, values }
    }

    /// Frame in which the coordinate is expressed.
    pub fn frame(&self) -> &CoordinateFrameId {
        &self.frame
    }

    /// Coordinate values in the frame's exact declared axis order.
    pub fn values(&self) -> &[f64; 3] {
        &self.values
    }
}

impl CoordinateFrame {
    /// Validate an explicit frame without inferring axes, unit, or pixel convention.
    pub fn new(
        id: CoordinateFrameId,
        axes: Vec<SpatialAxis>,
        unit: CoordinateUnit,
        space: CoordinateSpace,
    ) -> Result<Self, CoordinateError> {
        let dimension = validate_axes(&id, &axes)?;
        let units_agree = match space {
            CoordinateSpace::Image(_) => unit == CoordinateUnit::Pixel,
            CoordinateSpace::Physical => unit.is_physical(),
        };
        if !units_agree {
            return Err(CoordinateError::UnitSpaceMismatch {
                frame: id,
                unit,
                space,
            });
        }
        Ok(Self {
            id,
            axes: axes.into_boxed_slice(),
            dimension,
            unit,
            space,
        })
    }

    /// Typed frame identity.
    pub fn id(&self) -> &CoordinateFrameId {
        &self.id
    }

    /// Spatial axes in the exact declared coordinate order.
    pub fn axes(&self) -> &[SpatialAxis] {
        &self.axes
    }

    /// Two- or three-dimensional spatial arity.
    pub fn dimension(&self) -> SpatialDimension {
        self.dimension
    }

    /// Unit shared by all declared spatial axes.
    pub fn unit(&self) -> CoordinateUnit {
        self.unit
    }

    /// Image or physical coordinate-space semantics.
    pub fn space(&self) -> CoordinateSpace {
        self.space
    }
}

fn validate_axes(
    frame: &CoordinateFrameId,
    axes: &[SpatialAxis],
) -> Result<SpatialDimension, CoordinateError> {
    let has_exactly_once = |axis| axes.iter().filter(|observed| **observed == axis).count() == 1;
    match axes.len() {
        2 if has_exactly_once(SpatialAxis::X) && has_exactly_once(SpatialAxis::Y) => {
            Ok(SpatialDimension::Two)
        }
        3 if has_exactly_once(SpatialAxis::X)
            && has_exactly_once(SpatialAxis::Y)
            && has_exactly_once(SpatialAxis::Z) =>
        {
            Ok(SpatialDimension::Three)
        }
        _ => Err(CoordinateError::InvalidAxisSet {
            frame: frame.clone(),
        }),
    }
}
