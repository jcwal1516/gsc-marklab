use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::TopologyError;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectivityPointInput {
    pub id: String,
    pub coordinates_um: [f64; 2],
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectivityWindow {
    pub minimum_um: [f64; 2],
    pub maximum_um: [f64; 2],
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectivityTransitionSpec {
    pub points: Vec<ConnectivityPointInput>,
    pub radii_um: Vec<f64>,
    pub window: ConnectivityWindow,
    pub boundary_tolerance_um: f64,
    pub maximum_pairs: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConnectivityCurvePoint {
    pub radius_um: f64,
    pub component_count: usize,
    pub largest_fraction: f64,
    pub susceptibility: f64,
    pub spans_left_right: bool,
    pub spans_bottom_top: bool,
    pub edges_added_cumulative: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConnectivityTransitionResult {
    pub format: &'static str,
    pub version: u32,
    pub points: Vec<ConnectivityPointInput>,
    pub radii_um: Vec<f64>,
    pub distance: &'static str,
    pub curves: Vec<ConnectivityCurvePoint>,
    pub critical_radius_um: Option<f64>,
    pub critical_radius_rule: &'static str,
    pub pair_evaluations: u64,
    pub finite_size_caveat: &'static str,
    pub claim_status: &'static str,
}

pub fn connectivity_transition(
    mut spec: ConnectivityTransitionSpec,
) -> Result<ConnectivityTransitionResult, TopologyError> {
    if !(2..=100_000).contains(&spec.points.len())
        || spec.radii_um.is_empty()
        || spec.radii_um.len() > 4_096
        || spec
            .radii_um
            .iter()
            .any(|radius| !radius.is_finite() || *radius < 0.0)
        || spec.radii_um.windows(2).any(|pair| pair[0] >= pair[1])
        || !spec.boundary_tolerance_um.is_finite()
        || spec.boundary_tolerance_um < 0.0
        || spec.maximum_pairs == 0
    {
        return Err(TopologyError::Invalid(
            "connectivity points, radii, tolerance, or pair bound are invalid".into(),
        ));
    }
    if spec
        .window
        .minimum_um
        .iter()
        .chain(&spec.window.maximum_um)
        .any(|value| !value.is_finite())
        || (0..2).any(|axis| spec.window.minimum_um[axis] >= spec.window.maximum_um[axis])
    {
        return Err(TopologyError::Invalid(
            "connectivity window must be finite and positive".into(),
        ));
    }
    spec.points.sort_by(|left, right| left.id.cmp(&right.id));
    for (index, point) in spec.points.iter().enumerate() {
        if point.id.trim().is_empty()
            || point.id.trim() != point.id
            || (index > 0 && spec.points[index - 1].id == point.id)
            || point.coordinates_um.iter().any(|value| !value.is_finite())
            || (0..2).any(|axis| {
                point.coordinates_um[axis] < spec.window.minimum_um[axis]
                    || point.coordinates_um[axis] > spec.window.maximum_um[axis]
            })
        {
            return Err(TopologyError::Invalid(
                "connectivity points require unique exact IDs inside the window".into(),
            ));
        }
    }
    let pair_evaluations = (spec.points.len() as u64)
        .checked_mul(spec.points.len() as u64 - 1)
        .and_then(|value| value.checked_div(2))
        .ok_or_else(|| TopologyError::Invalid("connectivity pair count overflow".into()))?;
    if pair_evaluations > spec.maximum_pairs {
        return Err(TopologyError::Invalid(format!(
            "connectivity pairs {pair_evaluations} exceed caller maximum {}",
            spec.maximum_pairs
        )));
    }
    let maximum_radius = *spec.radii_um.last().expect("nonempty radii");
    let mut events = Vec::new();
    for left in 0..spec.points.len() {
        for right in (left + 1)..spec.points.len() {
            let dx = spec.points[left].coordinates_um[0] - spec.points[right].coordinates_um[0];
            let dy = spec.points[left].coordinates_um[1] - spec.points[right].coordinates_um[1];
            let distance = dx.hypot(dy);
            if distance <= maximum_radius {
                events.push((distance, left, right));
            }
        }
    }
    events.sort_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    let boundaries = spec
        .points
        .iter()
        .map(|point| {
            [
                point.coordinates_um[0] - spec.window.minimum_um[0] <= spec.boundary_tolerance_um,
                spec.window.maximum_um[0] - point.coordinates_um[0] <= spec.boundary_tolerance_um,
                point.coordinates_um[1] - spec.window.minimum_um[1] <= spec.boundary_tolerance_um,
                spec.window.maximum_um[1] - point.coordinates_um[1] <= spec.boundary_tolerance_um,
            ]
        })
        .collect::<Vec<_>>();
    let mut union = UnionFind::new(spec.points.len());
    let mut cursor = 0;
    let mut curves = Vec::with_capacity(spec.radii_um.len());
    for &radius in &spec.radii_um {
        while cursor < events.len() && events[cursor].0 <= radius {
            union.join(events[cursor].1, events[cursor].2);
            cursor += 1;
        }
        let mut components = BTreeMap::<usize, Vec<usize>>::new();
        for node in 0..spec.points.len() {
            components.entry(union.find(node)).or_default().push(node);
        }
        let largest = components.values().map(Vec::len).max().unwrap_or(0);
        let mut component_sizes = components.values().map(Vec::len).collect::<Vec<_>>();
        component_sizes.sort_unstable_by(|left, right| right.cmp(left));
        let susceptibility = component_sizes
            .iter()
            .skip(1)
            .map(|size| size.pow(2) as f64)
            .sum::<f64>()
            / spec.points.len() as f64;
        let spans_left_right = components.values().any(|component| {
            component.iter().any(|node| boundaries[*node][0])
                && component.iter().any(|node| boundaries[*node][1])
        });
        let spans_bottom_top = components.values().any(|component| {
            component.iter().any(|node| boundaries[*node][2])
                && component.iter().any(|node| boundaries[*node][3])
        });
        curves.push(ConnectivityCurvePoint {
            radius_um: radius,
            component_count: components.len(),
            largest_fraction: largest as f64 / spec.points.len() as f64,
            susceptibility,
            spans_left_right,
            spans_bottom_top,
            edges_added_cumulative: cursor,
        });
    }
    let critical_radius_um = curves
        .iter()
        .find(|point| point.spans_left_right || point.spans_bottom_top)
        .map(|point| point.radius_um);
    Ok(ConnectivityTransitionResult {
        format: "marklab.connectivity_transition",
        version: 1,
        points: spec.points,
        radii_um: spec.radii_um,
        distance: "euclidean_um",
        curves,
        critical_radius_um,
        critical_radius_rule: "first_declared_boundary_spanning_radius",
        pair_evaluations,
        finite_size_caveat:
            "single finite point pattern; cohort inference requires patient summaries",
        claim_status: "experimental_finite_size_connectivity",
    })
}

struct UnionFind {
    parent: Vec<usize>,
    size: Vec<usize>,
}

impl UnionFind {
    fn new(count: usize) -> Self {
        Self {
            parent: (0..count).collect(),
            size: vec![1; count],
        }
    }

    fn find(&mut self, mut node: usize) -> usize {
        while self.parent[node] != node {
            self.parent[node] = self.parent[self.parent[node]];
            node = self.parent[node];
        }
        node
    }

    fn join(&mut self, left: usize, right: usize) {
        let mut left_root = self.find(left);
        let mut right_root = self.find(right);
        if left_root == right_root {
            return;
        }
        if self.size[left_root] < self.size[right_root] {
            std::mem::swap(&mut left_root, &mut right_root);
        }
        self.parent[right_root] = left_root;
        self.size[left_root] += self.size[right_root];
    }
}
