use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::GraphError;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CellularJunctionInput {
    pub id: String,
    pub coordinates_um: [f64; 2],
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CellularInterfaceInput {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrientedCellularInterfaceInput {
    pub interface_id: String,
    pub orientation: i8,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CellularDomainInput {
    pub id: String,
    pub oriented_interfaces: Vec<OrientedCellularInterfaceInput>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellularSegmentationInput {
    pub junctions: Vec<CellularJunctionInput>,
    pub interfaces: Vec<CellularInterfaceInput>,
    pub domains: Vec<CellularDomainInput>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellularSegmentationPerturbation {
    pub id: String,
    pub segmentation: CellularSegmentationInput,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellularComplexSpec {
    pub pathology_interpretation: String,
    pub baseline: CellularSegmentationInput,
    pub segmentation_perturbations: Vec<CellularSegmentationPerturbation>,
    pub maximum_incidence_entries: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CellularComplexArtifact {
    pub complex_digest: String,
    pub topology_digest: String,
    pub cells_0: Vec<CellularJunctionInput>,
    pub cells_1: Vec<CellularInterfaceInput>,
    pub cells_2: Vec<String>,
    pub boundary_1: Vec<Vec<f64>>,
    pub boundary_2: Vec<Vec<f64>>,
    pub boundary_of_boundary_max_abs: f64,
    pub incidence_entries: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CellularPerturbationResult {
    pub id: String,
    pub complex_digest: String,
    pub topology_digest: String,
    pub topology_unchanged: bool,
    pub boundary_matrices_unchanged: bool,
    pub maximum_junction_displacement_um: Option<f64>,
    pub boundary_of_boundary_max_abs: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CellularComplexResult {
    pub format: &'static str,
    pub version: u32,
    pub pathology_interpretation: String,
    pub baseline: CellularComplexArtifact,
    pub perturbations: Vec<CellularPerturbationResult>,
    pub robustness_status: &'static str,
    pub claim_status: &'static str,
}

pub fn cellular_complex_workflow(
    spec: CellularComplexSpec,
) -> Result<CellularComplexResult, GraphError> {
    if spec.pathology_interpretation.trim().is_empty()
        || spec.pathology_interpretation.trim() != spec.pathology_interpretation
        || spec.segmentation_perturbations.is_empty()
        || spec.maximum_incidence_entries == 0
        || spec.maximum_incidence_entries > 1_000_000
    {
        return Err(GraphError::Invalid(
            "cellular complex requires an exact pathology interpretation, perturbations, and a bounded incidence budget".into(),
        ));
    }
    let baseline = build_cellular_complex(spec.baseline, spec.maximum_incidence_entries)?;
    let baseline_coordinates = baseline
        .cells_0
        .iter()
        .map(|junction| (junction.id.as_str(), junction.coordinates_um))
        .collect::<BTreeMap<_, _>>();
    let mut perturbation_ids = HashSet::new();
    let mut perturbations = Vec::with_capacity(spec.segmentation_perturbations.len());
    for perturbation in spec.segmentation_perturbations {
        if perturbation.id.trim().is_empty()
            || perturbation.id.trim() != perturbation.id
            || !perturbation_ids.insert(perturbation.id.clone())
        {
            return Err(GraphError::Invalid(
                "segmentation perturbation IDs must be unique exact strings".into(),
            ));
        }
        let candidate =
            build_cellular_complex(perturbation.segmentation, spec.maximum_incidence_entries)?;
        let topology_unchanged = candidate.topology_digest == baseline.topology_digest;
        let boundary_matrices_unchanged = candidate.boundary_1 == baseline.boundary_1
            && candidate.boundary_2 == baseline.boundary_2;
        let maximum_junction_displacement_um = topology_unchanged.then(|| {
            candidate
                .cells_0
                .iter()
                .map(|junction| {
                    let baseline = baseline_coordinates[&junction.id.as_str()];
                    let dx = junction.coordinates_um[0] - baseline[0];
                    let dy = junction.coordinates_um[1] - baseline[1];
                    dx.hypot(dy)
                })
                .fold(0.0_f64, f64::max)
        });
        perturbations.push(CellularPerturbationResult {
            id: perturbation.id,
            complex_digest: candidate.complex_digest,
            topology_digest: candidate.topology_digest,
            topology_unchanged,
            boundary_matrices_unchanged,
            maximum_junction_displacement_um,
            boundary_of_boundary_max_abs: candidate.boundary_of_boundary_max_abs,
        });
    }
    let stable = perturbations
        .iter()
        .all(|result| result.topology_unchanged && result.boundary_matrices_unchanged);
    Ok(CellularComplexResult {
        format: "marklab.cellular_complex",
        version: 1,
        pathology_interpretation: spec.pathology_interpretation,
        baseline,
        perturbations,
        robustness_status: if stable {
            "stable_for_all_declared_segmentation_perturbations"
        } else {
            "topology_changed_under_declared_segmentation_perturbation"
        },
        claim_status: "research_only_declared_compartment_interpretation",
    })
}

fn build_cellular_complex(
    mut segmentation: CellularSegmentationInput,
    maximum_incidence_entries: u64,
) -> Result<CellularComplexArtifact, GraphError> {
    validate_segmentation(&segmentation)?;
    segmentation
        .junctions
        .sort_by(|left, right| left.id.cmp(&right.id));
    segmentation
        .interfaces
        .sort_by(|left, right| left.id.cmp(&right.id));
    segmentation
        .domains
        .sort_by(|left, right| left.id.cmp(&right.id));
    for domain in &mut segmentation.domains {
        domain
            .oriented_interfaces
            .sort_by(|left, right| left.interface_id.cmp(&right.interface_id));
    }
    let node_index = segmentation
        .junctions
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let interface_index = segmentation
        .interfaces
        .iter()
        .enumerate()
        .map(|(index, interface)| (interface.id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let incidence_entries = (segmentation.junctions.len() as u64)
        .checked_mul(segmentation.interfaces.len() as u64)
        .and_then(|first| {
            (segmentation.interfaces.len() as u64)
                .checked_mul(segmentation.domains.len() as u64)
                .and_then(|second| first.checked_add(second))
        })
        .ok_or_else(|| GraphError::Invalid("cellular incidence size overflow".into()))?;
    if incidence_entries > maximum_incidence_entries {
        return Err(GraphError::Invalid(format!(
            "cellular incidence entries {incidence_entries} exceed caller maximum {maximum_incidence_entries}"
        )));
    }
    let mut boundary_1 =
        vec![vec![0.0; segmentation.interfaces.len()]; segmentation.junctions.len()];
    for (column, interface) in segmentation.interfaces.iter().enumerate() {
        let source = node_index[interface.source_id.as_str()];
        let target = node_index[interface.target_id.as_str()];
        boundary_1[source][column] = -1.0;
        boundary_1[target][column] = 1.0;
    }
    let mut boundary_2 = vec![vec![0.0; segmentation.domains.len()]; segmentation.interfaces.len()];
    for (column, domain) in segmentation.domains.iter().enumerate() {
        for oriented in &domain.oriented_interfaces {
            boundary_2[interface_index[oriented.interface_id.as_str()]][column] =
                f64::from(oriented.orientation);
        }
    }
    let boundary_of_boundary = multiply(&boundary_1, &boundary_2);
    let boundary_of_boundary_max_abs = boundary_of_boundary
        .iter()
        .flatten()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    if boundary_of_boundary_max_abs > 1e-12 {
        return Err(GraphError::Invalid(
            "cellular domain interfaces do not form oriented closed boundaries".into(),
        ));
    }
    let cells_2 = segmentation
        .domains
        .iter()
        .map(|domain| domain.id.clone())
        .collect::<Vec<_>>();
    let topology = serde_json::json!({
        "junction_ids": segmentation.junctions.iter().map(|node| &node.id).collect::<Vec<_>>(),
        "interfaces": &segmentation.interfaces,
        "domains": &segmentation.domains,
    });
    let topology_bytes =
        serde_json::to_vec(&topology).map_err(|error| GraphError::Numerical(error.to_string()))?;
    let topology_digest = format!("{:x}", Sha256::digest(topology_bytes));
    let complete = serde_json::json!({
        "topology": topology,
        "junctions": &segmentation.junctions,
        "boundary_1": &boundary_1,
        "boundary_2": &boundary_2,
    });
    let complete_bytes =
        serde_json::to_vec(&complete).map_err(|error| GraphError::Numerical(error.to_string()))?;
    Ok(CellularComplexArtifact {
        complex_digest: format!("{:x}", Sha256::digest(complete_bytes)),
        topology_digest,
        cells_0: segmentation.junctions,
        cells_1: segmentation.interfaces,
        cells_2,
        boundary_1,
        boundary_2,
        boundary_of_boundary_max_abs,
        incidence_entries,
    })
}

fn validate_segmentation(segmentation: &CellularSegmentationInput) -> Result<(), GraphError> {
    if !(3..=1_000).contains(&segmentation.junctions.len())
        || segmentation.interfaces.is_empty()
        || segmentation.domains.is_empty()
    {
        return Err(GraphError::Invalid(
            "cellular segmentation requires bounded junctions, interfaces, and domains".into(),
        ));
    }
    let mut node_ids = HashSet::new();
    for node in &segmentation.junctions {
        if node.id.trim().is_empty()
            || node.id.trim() != node.id
            || !node_ids.insert(node.id.as_str())
            || node.coordinates_um.iter().any(|value| !value.is_finite())
        {
            return Err(GraphError::Invalid(
                "cellular junctions require unique exact IDs and finite coordinates".into(),
            ));
        }
    }
    let mut interface_ids = HashSet::new();
    for interface in &segmentation.interfaces {
        if interface.id.trim().is_empty()
            || interface.id.trim() != interface.id
            || !interface_ids.insert(interface.id.as_str())
            || interface.source_id == interface.target_id
            || !node_ids.contains(interface.source_id.as_str())
            || !node_ids.contains(interface.target_id.as_str())
        {
            return Err(GraphError::Invalid(
                "cellular interfaces require unique exact IDs and distinct known endpoints".into(),
            ));
        }
    }
    let mut domain_ids = HashSet::new();
    for domain in &segmentation.domains {
        let mut seen = HashSet::new();
        if domain.id.trim().is_empty()
            || domain.id.trim() != domain.id
            || !domain_ids.insert(domain.id.as_str())
            || domain.oriented_interfaces.len() < 3
            || domain.oriented_interfaces.iter().any(|oriented| {
                !matches!(oriented.orientation, -1 | 1)
                    || !interface_ids.contains(oriented.interface_id.as_str())
                    || !seen.insert(oriented.interface_id.as_str())
            })
        {
            return Err(GraphError::Invalid(
                "cellular domains require unique exact IDs and at least three unique signed known interfaces".into(),
            ));
        }
    }
    Ok(())
}

fn multiply(left: &[Vec<f64>], right: &[Vec<f64>]) -> Vec<Vec<f64>> {
    (0..left.len())
        .map(|row| {
            (0..right[0].len())
                .map(|column| {
                    left[row]
                        .iter()
                        .zip(right)
                        .map(|(value, right_row)| value * right_row[column])
                        .sum()
                })
                .collect()
        })
        .collect()
}
