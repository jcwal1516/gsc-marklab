use std::collections::BTreeMap;

use marklab_workflow::{ArtifactId, ArtifactRecord, ContentDigest, MarklabProject};

use super::{
    declaration::{
        measurement_status_name, BinaryMarkDeclaration, BinaryMarkOrigin,
        HistologicCompartmentMarkDeclaration, NucleusAreaUm2MarkDeclaration,
        ProbabilityMarkDeclaration, ProbabilitySimplexMarkDeclaration,
        ProbabilityThresholdComparator, ScalarMarkValueKind,
    },
    DeclaredScalarInputError,
};

const MARK_SCHEMA: &str = "marklab.scalar_mark_provenance";
const THRESHOLD_SCHEMA: &str = "marklab.scalar_threshold_provenance";
const SCHEMA_VERSION: u32 = 1;

pub(super) fn semantic_artifact_ids(
    binary: &BinaryMarkDeclaration,
    probability: Option<&ProbabilityMarkDeclaration>,
) -> Result<Box<[ArtifactId]>, DeclaredScalarInputError> {
    let mut ids = Vec::with_capacity(3);
    ids.push(binary.provenance_artifact_id);
    if let Some(probability) = probability {
        ids.push(probability.provenance_artifact_id);
    }
    if let BinaryMarkOrigin::Thresholded {
        threshold_provenance_artifact_id,
        ..
    } = binary.origin
    {
        ids.push(threshold_provenance_artifact_id);
    }
    let mut distinct = ids.clone();
    distinct.sort_unstable();
    if distinct.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(DeclaredScalarInputError::DuplicateArtifactRole);
    }
    Ok(ids.into_boxed_slice())
}

pub(super) fn validate_provenance(
    project: &MarklabProject,
    binary: &BinaryMarkDeclaration,
    probability: Option<&ProbabilityMarkDeclaration>,
) -> Result<(), DeclaredScalarInputError> {
    let binary_dependencies = match &binary.origin {
        BinaryMarkOrigin::Independent => Vec::new(),
        BinaryMarkOrigin::Thresholded {
            threshold_provenance_artifact_id,
            ..
        } => {
            let probability =
                probability.ok_or(DeclaredScalarInputError::ThresholdSourceMismatch)?;
            vec![
                probability.provenance_artifact_id,
                *threshold_provenance_artifact_id,
            ]
        }
    };
    require_record(
        project,
        binary.provenance_artifact_id,
        MARK_SCHEMA,
        &binary_metadata(binary),
        &binary_dependencies,
    )?;
    if let Some(probability) = probability {
        require_record(
            project,
            probability.provenance_artifact_id,
            MARK_SCHEMA,
            &probability_metadata(probability),
            &[],
        )?;
    }
    if let BinaryMarkOrigin::Thresholded {
        probability_mark_id,
        comparator,
        threshold,
        threshold_provenance_artifact_id,
    } = &binary.origin
    {
        let probability = probability.ok_or(DeclaredScalarInputError::ThresholdSourceMismatch)?;
        require_record(
            project,
            *threshold_provenance_artifact_id,
            THRESHOLD_SCHEMA,
            &threshold_metadata(
                binary.mark_id.as_str(),
                probability_mark_id.as_str(),
                *comparator,
                *threshold,
            ),
            &[probability.provenance_artifact_id],
        )?;
    }
    Ok(())
}

pub(crate) fn validate_nucleus_area_um2_provenance(
    project: &MarklabProject,
    declaration: &NucleusAreaUm2MarkDeclaration,
) -> Result<(), DeclaredScalarInputError> {
    require_record(
        project,
        declaration.provenance_artifact_id(),
        MARK_SCHEMA,
        &nucleus_area_um2_metadata(declaration),
        &[],
    )
}

pub(crate) fn validate_histologic_compartment_provenance(
    project: &MarklabProject,
    declaration: &HistologicCompartmentMarkDeclaration,
) -> Result<(), DeclaredScalarInputError> {
    require_record(
        project,
        declaration.provenance_artifact_id(),
        MARK_SCHEMA,
        &histologic_compartment_metadata(declaration),
        &[],
    )
}

pub(crate) fn validate_probability_simplex_provenance(
    project: &MarklabProject,
    declaration: &ProbabilitySimplexMarkDeclaration,
) -> Result<(), DeclaredScalarInputError> {
    require_record(
        project,
        declaration.provenance_artifact_id(),
        MARK_SCHEMA,
        &probability_simplex_metadata(declaration),
        &[],
    )
}

fn require_record(
    project: &MarklabProject,
    artifact: ArtifactId,
    schema_id: &str,
    metadata: &BTreeMap<String, String>,
    dependencies: &[ArtifactId],
) -> Result<(), DeclaredScalarInputError> {
    let record = project
        .artifact_record(artifact)
        .ok_or(DeclaredScalarInputError::ProvenanceRecordMissing { artifact })?;
    let mut expected_dependencies = dependencies.to_vec();
    expected_dependencies.sort_unstable();
    if !record_matches(record, schema_id, metadata, &expected_dependencies) {
        return Err(DeclaredScalarInputError::ProvenanceProfileMismatch { artifact });
    }
    Ok(())
}

fn record_matches(
    record: &ArtifactRecord,
    schema_id: &str,
    metadata: &BTreeMap<String, String>,
    dependencies: &[ArtifactId],
) -> bool {
    record.schema().id() == schema_id
        && record.schema().version() == SCHEMA_VERSION
        && record.table().is_none()
        && record.semantic_metadata() == metadata
        && record.dependencies() == dependencies
}

fn binary_metadata(binary: &BinaryMarkDeclaration) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("mark_id".into(), binary.mark_id.as_str().into()),
        ("mark_label".into(), binary.label.clone()),
        (
            "measurement_status".into(),
            measurement_status_name(binary.measurement_status).into(),
        ),
        ("origin".into(), binary.origin.wire_name().into()),
        ("unit".into(), "unitless".into()),
        (
            "value_kind".into(),
            ScalarMarkValueKind::Binary.wire_name().into(),
        ),
    ])
}

fn probability_metadata(probability: &ProbabilityMarkDeclaration) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("mark_id".into(), probability.mark_id.as_str().into()),
        (
            "measurement_status".into(),
            measurement_status_name(probability.measurement_status).into(),
        ),
        ("unit".into(), "unitless".into()),
        (
            "value_kind".into(),
            ScalarMarkValueKind::Probability.wire_name().into(),
        ),
    ])
}

fn nucleus_area_um2_metadata(
    declaration: &NucleusAreaUm2MarkDeclaration,
) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("mark_id".into(), declaration.mark_id().as_str().into()),
        ("mark_label".into(), declaration.label().into()),
        (
            "measurement_status".into(),
            measurement_status_name(declaration.measurement_status()).into(),
        ),
        ("modality".into(), "morphology".into()),
        ("unit".into(), "square_micrometer".into()),
        ("value_kind".into(), "continuous".into()),
    ])
}

fn histologic_compartment_metadata(
    declaration: &HistologicCompartmentMarkDeclaration,
) -> BTreeMap<String, String> {
    let levels_digest =
        ContentDigest::from_framed(declaration.levels().iter().map(|level| level.as_bytes()));
    BTreeMap::from([
        ("levels_digest".into(), levels_digest.to_string()),
        (
            "levels_count".into(),
            declaration.levels().len().to_string(),
        ),
        ("mark_id".into(), declaration.mark_id().as_str().into()),
        ("mark_label".into(), declaration.label().into()),
        (
            "measurement_status".into(),
            measurement_status_name(declaration.measurement_status()).into(),
        ),
        ("modality".into(), "histology".into()),
        ("unit".into(), "categorical".into()),
        ("value_kind".into(), "categorical".into()),
    ])
}

fn probability_simplex_metadata(
    declaration: &ProbabilitySimplexMarkDeclaration,
) -> BTreeMap<String, String> {
    let levels_digest =
        ContentDigest::from_framed(declaration.levels().iter().map(|level| level.as_bytes()));
    BTreeMap::from([
        ("levels_digest".into(), levels_digest.to_string()),
        (
            "levels_count".into(),
            declaration.levels().len().to_string(),
        ),
        ("mark_id".into(), declaration.mark_id().as_str().into()),
        ("mark_label".into(), declaration.label().into()),
        (
            "measurement_status".into(),
            measurement_status_name(declaration.measurement_status()).into(),
        ),
        ("modality".into(), "morphology".into()),
        ("unit".into(), "probability_simplex".into()),
        ("value_kind".into(), "probability_simplex".into()),
    ])
}

fn threshold_metadata(
    binary_mark_id: &str,
    probability_mark_id: &str,
    comparator: ProbabilityThresholdComparator,
    threshold: f32,
) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("binary_mark_id".into(), binary_mark_id.into()),
        ("comparator".into(), comparator.wire_name().into()),
        ("probability_mark_id".into(), probability_mark_id.into()),
        (
            "threshold_f32_bits".into(),
            format!("{:08x}", threshold.to_bits()),
        ),
        ("unit".into(), "probability".into()),
    ])
}
