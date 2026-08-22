use rstar::{primitives::GeomWithData, RTree};

use crate::{
    common::stats::mean_all_finite,
    errors::{MarklabError, Result},
};

type IndexedPoint = GeomWithData<[f64; 2], usize>;

#[cfg(test)]
thread_local! {
    static INDEX_BUILD_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_index_build_call_count() {
    INDEX_BUILD_CALLS.set(0);
}

#[cfg(test)]
pub(crate) fn index_build_call_count() -> usize {
    INDEX_BUILD_CALLS.get()
}

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
        #[cfg(test)]
        INDEX_BUILD_CALLS.set(INDEX_BUILD_CALLS.get() + 1);
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

    #[allow(
        dead_code,
        reason = "required deterministic materialized query; allocation-sensitive domain paths use the visitor variant"
    )]
    pub fn within_radius(&self, index: usize, radius_um: f64) -> Result<Vec<Neighbor>> {
        let mut neighbors = Vec::new();
        self.visit_within_radius(index, radius_um, |neighbor| neighbors.push(neighbor))?;
        sort_neighbors(&mut neighbors);
        Ok(neighbors)
    }

    #[allow(
        dead_code,
        reason = "required deterministic materialized query; allocation-sensitive domain paths use the visitor variant"
    )]
    pub fn points_within_radius(&self, x: f64, y: f64, radius_um: f64) -> Result<Vec<Neighbor>> {
        let mut neighbors = Vec::new();
        self.visit_points_within_radius(x, y, radius_um, |neighbor| neighbors.push(neighbor))?;
        sort_neighbors(&mut neighbors);
        Ok(neighbors)
    }

    pub(crate) fn visit_within_radius(
        &self,
        index: usize,
        radius_um: f64,
        visit: impl FnMut(Neighbor),
    ) -> Result<()> {
        let query = self.point(index)?;
        validate_radius(radius_um)?;
        self.visit_neighbors_within_radius(query, radius_um, Some(index), visit);
        Ok(())
    }

    pub(crate) fn visit_points_within_radius(
        &self,
        x: f64,
        y: f64,
        radius_um: f64,
        visit: impl FnMut(Neighbor),
    ) -> Result<()> {
        if !x.is_finite() || !y.is_finite() {
            return Err(MarklabError::Geometry(
                "spatial query point must have finite coordinates".into(),
            ));
        }
        validate_radius(radius_um)?;
        self.visit_neighbors_within_radius([x, y], radius_um, None, visit);
        Ok(())
    }

    fn point(&self, index: usize) -> Result<[f64; 2]> {
        self.points.get(index).copied().ok_or_else(|| {
            MarklabError::Geometry(format!(
                "spatial query index {index} is out of bounds for {} points",
                self.len()
            ))
        })
    }

    fn visit_neighbors_within_radius(
        &self,
        query: [f64; 2],
        radius_um: f64,
        excluded_index: Option<usize>,
        mut visit: impl FnMut(Neighbor),
    ) {
        let radius_2 = radius_um * radius_um;
        for point in self.tree.locate_within_distance(query, radius_2) {
            if Some(point.data) == excluded_index {
                continue;
            }
            let neighbor = self.neighbor(point.data, query);
            if neighbor.distance_um <= radius_um {
                visit(neighbor);
            }
        }
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
