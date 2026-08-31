use std::collections::HashSet;

use marklab::{
    CoordinateFrame, CoordinateFrameId, CoordinateRegistry, CoordinateSpace, CoordinateUnit,
    PatientId, SectionId, SerialSectionSeries, SpatialAxis, SpecimenId, TimepointId,
};
use marklab_data::{SerialSection, SerialSectionPlacement, SerialSectionStatus, UncertaintyId};
use marklab_spatial3d::{
    voxel_window_k3d, K3dCorrection, Point3DInput, VoxelWindow3dInput, VoxelWindowK3dResult,
    VoxelWindowK3dSpec,
};
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

#[path = "spatial3d_registered/longitudinal.rs"]
pub(crate) mod longitudinal;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SectionStatusInput {
    Observed,
    Missing,
    Distorted,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SectionUncertaintyInput {
    uncertainty_id: String,
    conservative_radius_um: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SectionPointInput {
    id: String,
    coordinates_um: [f64; 2],
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisteredSectionInput {
    section_id: String,
    ordinal: u32,
    z_center_um: f64,
    thickness_um: f64,
    status: SectionStatusInput,
    source_frame_id: Option<String>,
    placement_coefficients: Option<[f64; 6]>,
    uncertainty: Option<SectionUncertaintyInput>,
    points: Vec<SectionPointInput>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisteredSerialVoxelKSpec {
    patient_id: String,
    specimen_id: String,
    timepoint_id: String,
    anatomical_site: String,
    volume_frame_id: String,
    sections: Vec<RegisteredSectionInput>,
    window: VoxelWindow3dInput,
    anisotropy_matrix: Option<[[f64; 3]; 3]>,
    radii_um: Vec<f64>,
    correction: K3dCorrection,
    maximum_unordered_pairs: u64,
    maximum_boundary_face_checks: u64,
    maximum_translation_voxel_pair_checks: u64,
    memory_budget_mib: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RegisteredSectionSummary {
    section_id: String,
    ordinal: u32,
    z_center_um: f64,
    thickness_um: f64,
    status: SectionStatusInput,
    source_frame_id: Option<String>,
    point_count: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedRegisteredSerialVoxelK {
    patient_id: String,
    specimen_id: String,
    timepoint_id: String,
    anatomical_site: String,
    volume_frame_id: String,
    sections: Vec<RegisteredSectionSummary>,
    missing_section_count: usize,
    analysis: VoxelWindowK3dSpec,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct RegisteredSerialVoxelKResult {
    format: &'static str,
    version: u32,
    patient_id: String,
    specimen_id: String,
    timepoint_id: String,
    anatomical_site: String,
    volume_frame_id: String,
    section_count: usize,
    observed_section_count: usize,
    missing_section_count: usize,
    sections: Vec<RegisteredSectionSummary>,
    analysis: VoxelWindowK3dResult,
    statistical_unit: &'static str,
    assumptions: [&'static str; 4],
    claim_status: &'static str,
}

impl<'de> Deserialize<'de> for RegisteredSerialVoxelKResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Owned {
            format: String,
            version: u32,
            patient_id: String,
            specimen_id: String,
            timepoint_id: String,
            anatomical_site: String,
            volume_frame_id: String,
            section_count: usize,
            observed_section_count: usize,
            missing_section_count: usize,
            sections: Vec<RegisteredSectionSummary>,
            analysis: VoxelWindowK3dResult,
            statistical_unit: String,
            assumptions: [String; 4],
            claim_status: String,
        }
        let owned = Owned::deserialize(deserializer)?;
        let assumptions = [
            "section_placements_are_deterministic_affine_maps_in_micrometers",
            "missing_sections_are_explicit_and_have_no_points_or_occupied_voxels",
            "voxel_window_is_already_segmented_in_the_registered_volume_frame",
            "distorted_or_uncertain_sections_require_transform_draw_propagation_and_are_rejected",
        ];
        if owned.format != "marklab.registered_serial_voxel_k3d"
            || owned.statistical_unit != "one_registered_specimen_volume_at_one_patient_timepoint"
            || owned.assumptions != assumptions
            || owned.claim_status != "registered_serial_geometry_descriptive_only"
        {
            return Err(serde::de::Error::custom(
                "unexpected registered serial voxel K result identity",
            ));
        }
        Ok(Self {
            format: "marklab.registered_serial_voxel_k3d",
            version: owned.version,
            patient_id: owned.patient_id,
            specimen_id: owned.specimen_id,
            timepoint_id: owned.timepoint_id,
            anatomical_site: owned.anatomical_site,
            volume_frame_id: owned.volume_frame_id,
            section_count: owned.section_count,
            observed_section_count: owned.observed_section_count,
            missing_section_count: owned.missing_section_count,
            sections: owned.sections,
            analysis: owned.analysis,
            statistical_unit: "one_registered_specimen_volume_at_one_patient_timepoint",
            assumptions,
            claim_status: "registered_serial_geometry_descriptive_only",
        })
    }
}

impl RegisteredSerialVoxelKResult {
    pub(crate) fn validate_for_prepared(
        &self,
        prepared: &PreparedRegisteredSerialVoxelK,
    ) -> Result<(), RegisteredSerialVoxelKError> {
        self.analysis
            .validate_for_spec(&prepared.analysis)
            .map_err(invalid)?;
        if self.version != 1
            || self.patient_id != prepared.patient_id
            || self.specimen_id != prepared.specimen_id
            || self.timepoint_id != prepared.timepoint_id
            || self.anatomical_site != prepared.anatomical_site
            || self.volume_frame_id != prepared.volume_frame_id
            || self.section_count != prepared.sections.len()
            || self.observed_section_count
                != prepared.sections.len() - prepared.missing_section_count
            || self.missing_section_count != prepared.missing_section_count
            || self.sections != prepared.sections
        {
            return Err(RegisteredSerialVoxelKError::Invalid(
                "restored registered serial voxel K result differs from its prepared identity"
                    .into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub(crate) enum RegisteredSerialVoxelKError {
    #[error("invalid registered serial 3-D input: {0}")]
    Invalid(String),
    #[error("invalid registered serial 3-D JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub(crate) fn prepare(
    bytes: &[u8],
) -> Result<PreparedRegisteredSerialVoxelK, RegisteredSerialVoxelKError> {
    let spec: RegisteredSerialVoxelKSpec = serde_json::from_slice(bytes)?;
    prepare_spec(spec)
}

fn prepare_spec(
    spec: RegisteredSerialVoxelKSpec,
) -> Result<PreparedRegisteredSerialVoxelK, RegisteredSerialVoxelKError> {
    let patient = PatientId::new(&spec.patient_id).map_err(invalid)?;
    let specimen = SpecimenId::new(&spec.specimen_id).map_err(invalid)?;
    let timepoint = TimepointId::new(&spec.timepoint_id).map_err(invalid)?;
    if spec.anatomical_site.trim().is_empty() || spec.anatomical_site.trim() != spec.anatomical_site
    {
        return Err(RegisteredSerialVoxelKError::Invalid(
            "anatomical_site must be nonempty without surrounding whitespace".into(),
        ));
    }
    let volume_frame_id = CoordinateFrameId::new(&spec.volume_frame_id).map_err(invalid)?;
    let volume_frame = CoordinateFrame::new(
        volume_frame_id.clone(),
        vec![SpatialAxis::X, SpatialAxis::Y, SpatialAxis::Z],
        CoordinateUnit::Micrometer,
        CoordinateSpace::Physical,
    )
    .map_err(invalid)?;
    if spec.sections.is_empty() {
        return Err(RegisteredSerialVoxelKError::Invalid(
            "registered serial input requires at least one section".into(),
        ));
    }

    let mut frames = vec![volume_frame];
    let mut serial_sections = Vec::with_capacity(spec.sections.len());
    let mut section_summaries = Vec::with_capacity(spec.sections.len());
    let mut embedded_inputs = Vec::new();
    let mut point_ids = HashSet::new();
    let mut missing_section_count = 0_usize;
    for section in &spec.sections {
        let section_id = SectionId::new(&section.section_id).map_err(invalid)?;
        let (status, placement) = match section.status {
            SectionStatusInput::Missing => {
                missing_section_count += 1;
                if section.source_frame_id.is_some()
                    || section.placement_coefficients.is_some()
                    || section.uncertainty.is_some()
                    || !section.points.is_empty()
                {
                    return Err(RegisteredSerialVoxelKError::Invalid(format!(
                        "missing section {} must not carry a frame, placement, uncertainty, or points",
                        section.section_id
                    )));
                }
                (SerialSectionStatus::Missing, None)
            }
            SectionStatusInput::Distorted => {
                let uncertainty = section.uncertainty.as_ref().ok_or_else(|| {
                    RegisteredSerialVoxelKError::Invalid(format!(
                        "distorted section {} lacks uncertainty identity and radius",
                        section.section_id
                    ))
                })?;
                UncertaintyId::new(&uncertainty.uncertainty_id).map_err(invalid)?;
                if !uncertainty.conservative_radius_um.is_finite()
                    || uncertainty.conservative_radius_um < 0.0
                {
                    return Err(RegisteredSerialVoxelKError::Invalid(format!(
                        "distorted section {} has an invalid conservative uncertainty radius",
                        section.section_id
                    )));
                }
                return Err(RegisteredSerialVoxelKError::Invalid(format!(
                    "distorted section {} requires transform-draw propagation; nominal voxel K refuses to ignore its uncertainty",
                    section.section_id
                )));
            }
            SectionStatusInput::Observed => {
                if section.uncertainty.is_some() {
                    return Err(RegisteredSerialVoxelKError::Invalid(format!(
                        "observed section {} must be declared distorted before attaching registration uncertainty",
                        section.section_id
                    )));
                }
                let source_text = section.source_frame_id.as_ref().ok_or_else(|| {
                    RegisteredSerialVoxelKError::Invalid(format!(
                        "observed section {} lacks source_frame_id",
                        section.section_id
                    ))
                })?;
                let source_id = CoordinateFrameId::new(source_text).map_err(invalid)?;
                frames.push(
                    CoordinateFrame::new(
                        source_id.clone(),
                        vec![SpatialAxis::X, SpatialAxis::Y],
                        CoordinateUnit::Micrometer,
                        CoordinateSpace::Physical,
                    )
                    .map_err(invalid)?,
                );
                let coefficients = section.placement_coefficients.ok_or_else(|| {
                    RegisteredSerialVoxelKError::Invalid(format!(
                        "observed section {} lacks placement_coefficients",
                        section.section_id
                    ))
                })?;
                (
                    SerialSectionStatus::Observed,
                    Some(
                        SerialSectionPlacement::new(source_id, coefficients, None)
                            .map_err(invalid)?,
                    ),
                )
            }
        };
        serial_sections.push(
            SerialSection::new(
                section_id,
                section.ordinal,
                section.z_center_um,
                section.thickness_um,
                status,
                placement,
            )
            .map_err(invalid)?,
        );
        section_summaries.push(RegisteredSectionSummary {
            section_id: section.section_id.clone(),
            ordinal: section.ordinal,
            z_center_um: section.z_center_um,
            thickness_um: section.thickness_um,
            status: section.status,
            source_frame_id: section.source_frame_id.clone(),
            point_count: section.points.len(),
        });
    }

    let series =
        SerialSectionSeries::new(volume_frame_id.clone(), serial_sections).map_err(invalid)?;
    let registry =
        CoordinateRegistry::new(frames, Vec::new(), Vec::new(), vec![series]).map_err(invalid)?;
    validate_section_window_planes(&spec.sections, &spec.window)?;
    for section in &spec.sections {
        if !matches!(section.status, SectionStatusInput::Observed) {
            continue;
        }
        let source_id = CoordinateFrameId::new(
            section
                .source_frame_id
                .as_ref()
                .expect("observed source validated"),
        )
        .map_err(invalid)?;
        let section_id = SectionId::new(&section.section_id).map_err(invalid)?;
        for point in &section.points {
            if !point_ids.insert(point.id.as_str()) {
                return Err(RegisteredSerialVoxelKError::Invalid(format!(
                    "duplicate registered point ID: {}",
                    point.id
                )));
            }
            let coordinate = registry
                .coordinate_2d(&source_id, point.coordinates_um)
                .map_err(invalid)?;
            let embedded = registry
                .embed_serial_section(&section_id, &coordinate)
                .map_err(invalid)?;
            embedded_inputs.push(Point3DInput {
                id: point.id.clone(),
                coordinates: *embedded.values(),
            });
        }
    }
    if embedded_inputs.len() < 2 {
        return Err(RegisteredSerialVoxelKError::Invalid(
            "registered serial K requires at least two embedded points".into(),
        ));
    }
    Ok(PreparedRegisteredSerialVoxelK {
        patient_id: patient.as_str().to_owned(),
        specimen_id: specimen.as_str().to_owned(),
        timepoint_id: timepoint.as_str().to_owned(),
        anatomical_site: spec.anatomical_site,
        volume_frame_id: volume_frame_id.as_str().to_owned(),
        sections: section_summaries,
        missing_section_count,
        analysis: VoxelWindowK3dSpec {
            points: embedded_inputs,
            window: spec.window,
            coordinate_unit: marklab_spatial3d::CoordinateUnit::Micrometer,
            anisotropy_matrix: spec.anisotropy_matrix,
            radii_um: spec.radii_um,
            correction: spec.correction,
            maximum_unordered_pairs: spec.maximum_unordered_pairs,
            maximum_boundary_face_checks: spec.maximum_boundary_face_checks,
            maximum_translation_voxel_pair_checks: spec.maximum_translation_voxel_pair_checks,
            memory_budget_mib: spec.memory_budget_mib,
        },
    })
}

pub(crate) fn execute(
    prepared: &PreparedRegisteredSerialVoxelK,
) -> Result<RegisteredSerialVoxelKResult, RegisteredSerialVoxelKError> {
    let analysis = voxel_window_k3d(prepared.analysis.clone()).map_err(invalid)?;
    Ok(RegisteredSerialVoxelKResult {
        format: "marklab.registered_serial_voxel_k3d",
        version: 1,
        patient_id: prepared.patient_id.clone(),
        specimen_id: prepared.specimen_id.clone(),
        timepoint_id: prepared.timepoint_id.clone(),
        anatomical_site: prepared.anatomical_site.clone(),
        volume_frame_id: prepared.volume_frame_id.clone(),
        section_count: prepared.sections.len(),
        observed_section_count: prepared.sections.len() - prepared.missing_section_count,
        missing_section_count: prepared.missing_section_count,
        sections: prepared.sections.clone(),
        analysis,
        statistical_unit: "one_registered_specimen_volume_at_one_patient_timepoint",
        assumptions: [
            "section_placements_are_deterministic_affine_maps_in_micrometers",
            "missing_sections_are_explicit_and_have_no_points_or_occupied_voxels",
            "voxel_window_is_already_segmented_in_the_registered_volume_frame",
            "distorted_or_uncertain_sections_require_transform_draw_propagation_and_are_rejected",
        ],
        claim_status: "registered_serial_geometry_descriptive_only",
    })
}

fn validate_section_window_planes(
    sections: &[RegisteredSectionInput],
    window: &VoxelWindow3dInput,
) -> Result<(), RegisteredSerialVoxelKError> {
    let mut occupied_by_z = vec![false; window.dimensions[2] as usize];
    let mut declared_by_z = vec![false; window.dimensions[2] as usize];
    for voxel in &window.occupied_voxels {
        if let Some(value) = occupied_by_z.get_mut(voxel[2] as usize) {
            *value = true;
        }
    }
    for section in sections {
        if (section.thickness_um - window.voxel_size[2]).abs() > 1.0e-12 {
            return Err(RegisteredSerialVoxelKError::Invalid(format!(
                "section {} thickness does not equal the registered volume Z voxel size",
                section.section_id
            )));
        }
        let index = (section.z_center_um - window.origin[2]) / window.voxel_size[2] - 0.5;
        let rounded = index.round();
        if !index.is_finite()
            || (index - rounded).abs() > 1.0e-12
            || rounded < 0.0
            || rounded >= f64::from(window.dimensions[2])
        {
            return Err(RegisteredSerialVoxelKError::Invalid(format!(
                "section {} center is not aligned to a registered volume Z plane",
                section.section_id
            )));
        }
        let occupied = occupied_by_z[rounded as usize];
        if std::mem::replace(&mut declared_by_z[rounded as usize], true) {
            return Err(RegisteredSerialVoxelKError::Invalid(format!(
                "more than one section is assigned to registered volume Z plane {rounded}"
            )));
        }
        if matches!(section.status, SectionStatusInput::Missing) == occupied {
            return Err(RegisteredSerialVoxelKError::Invalid(format!(
                "section {} status disagrees with occupancy of its registered volume plane",
                section.section_id
            )));
        }
    }
    if declared_by_z.iter().any(|declared| !declared) {
        return Err(RegisteredSerialVoxelKError::Invalid(
            "every registered volume Z plane must have an observed, missing, or distorted section declaration"
                .into(),
        ));
    }
    Ok(())
}

fn invalid(error: impl std::fmt::Display) -> RegisteredSerialVoxelKError {
    RegisteredSerialVoxelKError::Invalid(error.to_string())
}
