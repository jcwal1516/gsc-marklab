use rstar::{primitives::GeomWithData, RTree};

use crate::{
    common::stats::mean_all_finite,
    errors::{MarklabError, Result},
};

type IndexedPoint = GeomWithData<[f64; 2], usize>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Neighbor {
    pub index: usize,
    pub distance_um: f64,
}

#[derive(Clone, Debug)]
pub struct SpatialIndex2D {
    points: Box<[[f64; 2]]>,
    tree: RTree<IndexedPoint>,
}

impl SpatialIndex2D {
    pub fn new(x: &[f64], y: &[f64]) -> Result<Self> {
        if x.len() != y.len() {
            return Err(MarklabError::Geometry(format!(
                "spatial index coordinate lengths differ: x={}, y={}",
                x.len(),
                y.len()
            )));
        }

        Self::from_points(
            x.iter()
                .copied()
                .zip(y.iter().copied())
                .map(|(x, y)| [x, y]),
        )
    }

    pub(crate) fn from_points(points: impl IntoIterator<Item = [f64; 2]>) -> Result<Self> {
        let points = points
            .into_iter()
            .enumerate()
            .map(|(index, point)| {
                let [x, y] = point;
                if !x.is_finite() || !y.is_finite() {
                    return Err(MarklabError::Geometry(format!(
                        "spatial index point {index} must have finite coordinates"
                    )));
                }
                Ok(point)
            })
            .collect::<Result<Box<[_]>>>()?;
        let tree = RTree::bulk_load(
            points
                .iter()
                .copied()
                .enumerate()
                .map(|(index, point)| IndexedPoint::new(point, index))
                .collect(),
        );

        Ok(Self { points, tree })
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn nearest_neighbor(&self, index: usize) -> Result<Option<Neighbor>> {
        Ok(self.k_nearest(index, 1)?.into_iter().next())
    }

    pub fn k_nearest(&self, index: usize, k: usize) -> Result<Vec<Neighbor>> {
        let query = self.point(index)?;
        if k == 0 || self.len() < 2 {
            return Ok(Vec::new());
        }

        let requested = k.min(self.len() - 1);
        let mut candidates = Vec::with_capacity(requested);
        let mut cutoff_distance_2 = None;
        for (point, distance_2) in self.tree.nearest_neighbor_iter_with_distance_2(query) {
            if point.data == index {
                continue;
            }
            if cutoff_distance_2.is_some_and(|cutoff| distance_2 > cutoff) {
                break;
            }

            candidates.push(self.neighbor(point.data, query));
            if candidates.len() == requested {
                cutoff_distance_2 = Some(distance_2);
            }
        }
        sort_neighbors(&mut candidates);
        candidates.truncate(requested);
        Ok(candidates)
    }

    pub fn within_radius(&self, index: usize, radius_um: f64) -> Result<Vec<Neighbor>> {
        let query = self.point(index)?;
        validate_radius(radius_um)?;
        Ok(self.neighbors_within_radius(query, radius_um, Some(index)))
    }

    pub fn points_within_radius(&self, x: f64, y: f64, radius_um: f64) -> Result<Vec<Neighbor>> {
        if !x.is_finite() || !y.is_finite() {
            return Err(MarklabError::Geometry(
                "spatial query point must have finite coordinates".into(),
            ));
        }
        validate_radius(radius_um)?;
        Ok(self.neighbors_within_radius([x, y], radius_um, None))
    }

    fn point(&self, index: usize) -> Result<[f64; 2]> {
        self.points.get(index).copied().ok_or_else(|| {
            MarklabError::Geometry(format!(
                "spatial query index {index} is out of bounds for {} points",
                self.len()
            ))
        })
    }

    fn neighbors_within_radius(
        &self,
        query: [f64; 2],
        radius_um: f64,
        excluded_index: Option<usize>,
    ) -> Vec<Neighbor> {
        let radius_2 = radius_um * radius_um;
        let mut neighbors = self
            .tree
            .locate_within_distance(query, radius_2)
            .filter(|point| Some(point.data) != excluded_index)
            .map(|point| self.neighbor(point.data, query))
            .filter(|neighbor| neighbor.distance_um <= radius_um)
            .collect::<Vec<_>>();
        sort_neighbors(&mut neighbors);
        neighbors
    }

    fn neighbor(&self, index: usize, query: [f64; 2]) -> Neighbor {
        let point = self.points[index];
        Neighbor {
            index,
            distance_um: (point[0] - query[0]).hypot(point[1] - query[1]),
        }
    }
}

pub fn mean_nearest_neighbor_distance(x: &[f64], y: &[f64]) -> Option<f64> {
    let index = SpatialIndex2D::new(x, y).ok()?;
    if index.len() < 2 {
        return None;
    }
    mean_all_finite((0..index.len()).map(|point| {
        index
            .nearest_neighbor(point)
            .ok()
            .flatten()
            .map(|neighbor| neighbor.distance_um)
            .unwrap_or(f64::NAN)
    }))
}

fn validate_radius(radius_um: f64) -> Result<()> {
    if !radius_um.is_finite() || radius_um < 0.0 {
        return Err(MarklabError::Geometry(
            "spatial query radius must be finite and non-negative".into(),
        ));
    }
    Ok(())
}

fn sort_neighbors(neighbors: &mut [Neighbor]) {
    neighbors.sort_by(|left, right| {
        left.distance_um
            .total_cmp(&right.distance_um)
            .then_with(|| left.index.cmp(&right.index))
    });
}
