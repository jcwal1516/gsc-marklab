//! Disc queries over a specimen's cells in the plane or, for tables with `z_um`, in space, as
//! R's `census_table` queries X, Y and Z (Z is 0 for planar tables).

use rstar::{primitives::GeomWithData, RTree};

use super::{invalid, SpaceError};
use crate::geom::spatial_index::SpatialIndex2D;

type VolumePoint = GeomWithData<[f64; 3], usize>;

pub(crate) enum CellIndex {
    Planar(SpatialIndex2D),
    Volume {
        points: Box<[[f64; 3]]>,
        tree: RTree<VolumePoint>,
    },
}

impl CellIndex {
    /// Index finite coordinates; `z_um` makes the index three-dimensional.
    pub(crate) fn new(
        x_um: &[f64],
        y_um: &[f64],
        z_um: Option<&[f64]>,
    ) -> Result<Self, SpaceError> {
        let Some(z_um) = z_um else {
            return SpatialIndex2D::new(x_um, y_um)
                .map(Self::Planar)
                .map_err(|error| invalid(format!("spatial index: {error}")));
        };
        let points = x_um
            .iter()
            .zip(y_um)
            .zip(z_um)
            .map(|((&x, &y), &z)| [x, y, z])
            .collect::<Box<[_]>>();
        let tree = RTree::bulk_load(
            points
                .iter()
                .enumerate()
                .map(|(index, &point)| VolumePoint::new(point, index))
                .collect(),
        );
        Ok(Self::Volume { points, tree })
    }

    /// Visit every other cell within `radius_um` of cell `index`, inclusive.
    pub(crate) fn visit_within_radius(
        &self,
        index: usize,
        radius_um: f64,
        mut visit: impl FnMut(usize),
    ) -> Result<(), SpaceError> {
        match self {
            Self::Planar(index_2d) => index_2d
                .visit_within_radius(index, radius_um, |neighbor| visit(neighbor.index))
                .map_err(|error| invalid(format!("disc query failed: {error}"))),
            Self::Volume { points, tree } => {
                let query = points[index];
                for point in tree.locate_within_distance(query, radius_um * radius_um) {
                    let other = points[point.data];
                    // FNN's distance: the root of the summed squared differences.
                    let distance = ((other[0] - query[0]).powi(2)
                        + (other[1] - query[1]).powi(2)
                        + (other[2] - query[2]).powi(2))
                    .sqrt();
                    if point.data != index && distance <= radius_um {
                        visit(point.data);
                    }
                }
                Ok(())
            }
        }
    }
}
