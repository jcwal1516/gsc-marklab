use marklab_data::{
    CellId, CoordinateFrameId, CoordinateSpace, CoordinateUnit, HierarchyId, SlideId, SpatialAxis,
    SpatialDimension,
};
use marklab_workflow::{ArtifactId, ArtifactRef, MarklabProject};

use crate::data::Pattern;

use super::{
    declaration::{
        BinaryMarkDeclaration, BinaryMarkOrigin, DeclaredMarkUse, ProbabilityMarkDeclaration,
        ScalarMarkValueKind,
    },
    identity::{cell_ids_identity, declared_identity, DeclaredScalarIdentity},
    provenance::{semantic_artifact_ids, validate_provenance},
    DeclaredScalarInputError, MarkTable,
};

const DECLARED_INPUT_KIND: &str = "application/vnd.marklab.declared-scalar-pattern;version=1";

/// Borrowed version-one scalar marked-pattern input with exact identity and provenance bindings.
pub struct DeclaredScalarPatternInput<'a> {
    pattern: &'a Pattern,
    cell_ids: &'a [CellId],
    owning_slide_id: SlideId,
    coordinate_frame_id: CoordinateFrameId,
    binary_mark: BinaryMarkDeclaration,
    probability_mark: Option<ProbabilityMarkDeclaration>,
    scalar_identity: DeclaredScalarIdentity,
    declared_artifact_ref: ArtifactRef,
    semantic_artifact_ids: Box<[ArtifactId]>,
    mark_table: Option<&'a MarkTable>,
}

impl<'a> DeclaredScalarPatternInput<'a> {
    /// Validate exact row, hierarchy, frame, scalar-value, and provenance bindings.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project: &MarklabProject,
        pattern: &'a Pattern,
        cell_ids: &'a [CellId],
        owning_slide_id: SlideId,
        coordinate_frame_id: CoordinateFrameId,
        binary_mark: BinaryMarkDeclaration,
        probability_mark: Option<ProbabilityMarkDeclaration>,
        maximum_rows: usize,
        maximum_cell_id_text_bytes: usize,
    ) -> Result<Self, DeclaredScalarInputError> {
        validate_shape_and_resource_bounds(
            pattern,
            cell_ids,
            maximum_rows,
            maximum_cell_id_text_bytes,
        )?;
        validate_pattern_columns(pattern)?;
        validate_probability(pattern, probability_mark.as_ref())?;
        validate_frame(project, &coordinate_frame_id)?;
        validate_hierarchy(project, cell_ids, &owning_slide_id, pattern)?;
        validate_threshold_source(&binary_mark, probability_mark.as_ref())?;
        let semantic_artifact_ids = semantic_artifact_ids(&binary_mark, probability_mark.as_ref())?;
        validate_provenance(project, &binary_mark, probability_mark.as_ref())?;
        validate_threshold_rows(pattern, &binary_mark)?;

        let (cell_ids_logical_digest, _) = cell_ids_identity(cell_ids)?;
        let (declared_input_logical_digest, byte_len) = declared_identity(
            cell_ids,
            &owning_slide_id,
            &coordinate_frame_id,
            &binary_mark,
            probability_mark.as_ref(),
        )?;
        let declared_artifact_ref =
            ArtifactRef::new(DECLARED_INPUT_KIND, declared_input_logical_digest, byte_len)
                .map_err(|_| DeclaredScalarInputError::LogicalIdentityOverflow)?;
        let scalar_identity = DeclaredScalarIdentity::new(
            cell_ids.len(),
            cell_ids_logical_digest,
            owning_slide_id.clone(),
            coordinate_frame_id.clone(),
            declared_input_logical_digest,
        );
        Ok(Self {
            pattern,
            cell_ids,
            owning_slide_id,
            coordinate_frame_id,
            binary_mark,
            probability_mark,
            scalar_identity,
            declared_artifact_ref,
            semantic_artifact_ids,
            mark_table: None,
        })
    }

    /// Bind one row-aligned typed mark table to the unchanged compatibility Pattern.
    pub fn from_mark_table(
        project: &MarklabProject,
        pattern: &'a Pattern,
        mark_table: &'a MarkTable,
        owning_slide_id: SlideId,
        coordinate_frame_id: CoordinateFrameId,
    ) -> Result<Self, DeclaredScalarInputError> {
        mark_table.validate_for_pattern(pattern)?;
        let (binary_mark, probability_mark) = mark_table.declarations()?;
        // Preserve the endpoint identity of the selected binary/probability estimand;
        // the separate table artifact below binds every retained column and value.
        let mut input = Self::new(
            project,
            pattern,
            mark_table.cell_ids(),
            owning_slide_id,
            coordinate_frame_id,
            binary_mark,
            probability_mark,
            mark_table.cell_ids().len().max(1),
            mark_table.cell_id_text_bytes().max(1),
        )?;
        mark_table.validate_provenance(project)?;
        input.semantic_artifact_ids = mark_table.semantic_artifact_ids()?;
        input.declared_artifact_ref =
            mark_table.declared_artifact_ref(&input.owning_slide_id, &input.coordinate_frame_id)?;
        input.mark_table = Some(mark_table);
        Ok(input)
    }

    /// Unchanged compatibility Pattern borrowed by the declared workflow.
    pub fn pattern(&self) -> &Pattern {
        self.pattern
    }

    /// Strictly increasing CellIds aligned one-to-one with Pattern rows.
    pub fn cell_ids(&self) -> &[CellId] {
        self.cell_ids
    }

    /// Explicit owning slide shared by every declared cell.
    pub fn owning_slide_id(&self) -> &SlideId {
        &self.owning_slide_id
    }

    /// Exact physical `[X,Y]` micrometre frame asserted for Pattern coordinates.
    pub fn coordinate_frame_id(&self) -> &CoordinateFrameId {
        &self.coordinate_frame_id
    }

    /// Required binary mark declaration.
    pub fn binary_mark(&self) -> &BinaryMarkDeclaration {
        &self.binary_mark
    }

    /// Optional dense probability mark declaration.
    pub fn probability_mark(&self) -> Option<&ProbabilityMarkDeclaration> {
        self.probability_mark.as_ref()
    }

    /// Typed mark table backing this input, when constructed through the table adapter.
    pub fn mark_table(&self) -> Option<&MarkTable> {
        self.mark_table
    }

    /// Compact row, slide, frame, declaration, status, and provenance identity.
    pub(crate) fn scalar_identity(&self) -> &DeclaredScalarIdentity {
        &self.scalar_identity
    }

    /// Versioned reference-only declared-input identity used as a workflow input.
    pub(crate) fn declared_artifact_ref(&self) -> Result<ArtifactRef, DeclaredScalarInputError> {
        self.verify_identity()?;
        Ok(self.declared_artifact_ref.clone())
    }

    /// Exact semantic artifact roles in binary, probability, threshold order.
    pub(crate) fn semantic_artifact_ids(&self) -> &[ArtifactId] {
        &self.semantic_artifact_ids
    }

    /// Recompute the borrowed declared identity before scheduler cache lookup.
    fn verify_identity(&self) -> Result<(), DeclaredScalarInputError> {
        let (digest, byte_len) = declared_identity(
            self.cell_ids,
            &self.owning_slide_id,
            &self.coordinate_frame_id,
            &self.binary_mark,
            self.probability_mark.as_ref(),
        )?;
        if digest != self.scalar_identity.declared_input_logical_digest() {
            return Err(DeclaredScalarInputError::DeclaredIdentityMismatch);
        }
        let expected = match self.mark_table {
            Some(mark_table) => mark_table
                .declared_artifact_ref(&self.owning_slide_id, &self.coordinate_frame_id)?,
            None => ArtifactRef::new(DECLARED_INPUT_KIND, digest, byte_len)
                .map_err(|_| DeclaredScalarInputError::LogicalIdentityOverflow)?,
        };
        if expected.digest() != self.declared_artifact_ref.digest()
            || expected.byte_len() != self.declared_artifact_ref.byte_len()
            || expected.kind() != self.declared_artifact_ref.kind()
        {
            return Err(DeclaredScalarInputError::DeclaredIdentityMismatch);
        }
        Ok(())
    }

    /// Revalidate project-owned frame, hierarchy, and provenance before node input cataloging.
    pub(crate) fn revalidate_project(
        &self,
        project: &MarklabProject,
    ) -> Result<(), DeclaredScalarInputError> {
        validate_frame(project, &self.coordinate_frame_id)?;
        validate_hierarchy(project, self.cell_ids, &self.owning_slide_id, self.pattern)?;
        let semantic_artifact_ids = match self.mark_table {
            Some(mark_table) => {
                mark_table.validate_for_pattern(self.pattern)?;
                mark_table.validate_provenance(project)?;
                mark_table.semantic_artifact_ids()?
            }
            None => semantic_artifact_ids(&self.binary_mark, self.probability_mark.as_ref())?,
        };
        if semantic_artifact_ids.as_ref() != self.semantic_artifact_ids.as_ref() {
            return Err(DeclaredScalarInputError::DeclaredIdentityMismatch);
        }
        if self.mark_table.is_none() {
            validate_provenance(project, &self.binary_mark, self.probability_mark.as_ref())?;
        }
        self.verify_identity()
    }

    /// Validate config 0.2 label/value routing and return the runtime-only summary.
    pub(crate) fn mark_use_for_config(
        &self,
        configured_mark_label: &str,
        use_probabilistic_marks: bool,
    ) -> Result<DeclaredMarkUse, DeclaredScalarInputError> {
        if configured_mark_label != self.binary_mark.label {
            return Err(DeclaredScalarInputError::ConfigurationMarkLabelMismatch);
        }
        if use_probabilistic_marks {
            if self.probability_mark.is_none() {
                return Err(DeclaredScalarInputError::ConfigurationValueKindMismatch);
            }
        } else if self.probability_mark.is_some()
            || !matches!(self.binary_mark.origin, BinaryMarkOrigin::Independent)
        {
            return Err(DeclaredScalarInputError::ConfigurationValueKindMismatch);
        }
        Ok(DeclaredMarkUse {
            binary_mark: self.binary_mark.clone(),
            probability_mark: self.probability_mark.clone(),
            structure_factor_value_kind: if use_probabilistic_marks {
                ScalarMarkValueKind::Probability
            } else {
                ScalarMarkValueKind::Binary
            },
        })
    }
}

fn validate_shape_and_resource_bounds(
    pattern: &Pattern,
    cell_ids: &[CellId],
    maximum_rows: usize,
    maximum_cell_id_text_bytes: usize,
) -> Result<(), DeclaredScalarInputError> {
    if cell_ids.len() != pattern.len() {
        return Err(DeclaredScalarInputError::CellIdCountMismatch {
            expected: pattern.len(),
            observed: cell_ids.len(),
        });
    }
    if maximum_rows == 0 || maximum_cell_id_text_bytes == 0 {
        return Err(DeclaredScalarInputError::InvalidResourceLimit);
    }
    if cell_ids.len() > maximum_rows {
        return Err(DeclaredScalarInputError::RowCountExceeded {
            observed: cell_ids.len(),
            maximum: maximum_rows,
        });
    }
    let required = cell_ids
        .iter()
        .try_fold(0_usize, |total, id| total.checked_add(id.as_str().len()))
        .ok_or(DeclaredScalarInputError::LogicalIdentityOverflow)?;
    if required > maximum_cell_id_text_bytes {
        return Err(DeclaredScalarInputError::CellIdTextBudgetExceeded {
            required,
            maximum: maximum_cell_id_text_bytes,
        });
    }
    Ok(())
}

fn validate_pattern_columns(pattern: &Pattern) -> Result<(), DeclaredScalarInputError> {
    let expected = pattern.len();
    for (column, observed) in [
        ("x_um", pattern.x_um.len()),
        ("y_um", pattern.y_um.len()),
        ("valid", pattern.valid.len()),
    ] {
        require_column_len(column, expected, observed)?;
    }
    for (row, (x, y)) in pattern.x_um.iter().zip(pattern.y_um.iter()).enumerate() {
        if !x.is_finite() || !y.is_finite() {
            return Err(DeclaredScalarInputError::NonFiniteCoordinate { row });
        }
    }
    if let Some(row) = pattern.valid.iter().position(|value| *value != 1) {
        return Err(DeclaredScalarInputError::UnsupportedMissingScalarRow { row });
    }
    if let Some(row) = pattern.mark.iter().position(|value| *value > 1) {
        return Err(DeclaredScalarInputError::InvalidBinaryMark { row });
    }
    for (column, observed) in [
        (
            "tumor_probability",
            pattern
                .tumor_probability
                .as_ref()
                .map(|values| values.len()),
        ),
        (
            "nucleus_area_um2",
            pattern.nucleus_area_um2.as_ref().map(|values| values.len()),
        ),
        (
            "component_id",
            pattern.component_id.as_ref().map(|values| values.len()),
        ),
        ("qc_bin", pattern.qc_bin.as_ref().map(|values| values.len())),
        (
            "local_dab_od",
            pattern.local_dab_od.as_ref().map(|values| values.len()),
        ),
        (
            "local_hematoxylin_od",
            pattern
                .local_hematoxylin_od
                .as_ref()
                .map(|values| values.len()),
        ),
    ] {
        if let Some(observed) = observed {
            require_column_len(column, expected, observed)?;
        }
    }
    for values in pattern.categorical_strata.values() {
        require_column_len("categorical_strata", expected, values.len())?;
    }
    Ok(())
}

fn require_column_len(
    column: &'static str,
    expected: usize,
    observed: usize,
) -> Result<(), DeclaredScalarInputError> {
    if observed != expected {
        return Err(DeclaredScalarInputError::PatternColumnLengthMismatch {
            column,
            expected,
            observed,
        });
    }
    Ok(())
}

fn validate_probability(
    pattern: &Pattern,
    declaration: Option<&ProbabilityMarkDeclaration>,
) -> Result<(), DeclaredScalarInputError> {
    if pattern.mark_prob.is_some() != declaration.is_some() {
        return Err(DeclaredScalarInputError::ProbabilityDeclarationMismatch);
    }
    let Some(values) = pattern.mark_prob.as_deref() else {
        return Ok(());
    };
    if values.len() != pattern.len() {
        return Err(DeclaredScalarInputError::ProbabilityLengthMismatch {
            expected: pattern.len(),
            observed: values.len(),
        });
    }
    if let Some(row) = values
        .iter()
        .position(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
    {
        return Err(DeclaredScalarInputError::InvalidProbability { row });
    }
    Ok(())
}

fn validate_frame(
    project: &MarklabProject,
    frame_id: &CoordinateFrameId,
) -> Result<(), DeclaredScalarInputError> {
    let registry = project
        .coordinate_registry()
        .ok_or(DeclaredScalarInputError::CoordinateRegistryMissing)?;
    let frame = registry
        .frame(frame_id)
        .ok_or(DeclaredScalarInputError::CoordinateFrameMismatch)?;
    if frame.dimension() != SpatialDimension::Two
        || frame.space() != CoordinateSpace::Physical
        || frame.unit() != CoordinateUnit::Micrometer
        || frame.axes() != [SpatialAxis::X, SpatialAxis::Y]
    {
        return Err(DeclaredScalarInputError::CoordinateFrameMismatch);
    }
    Ok(())
}

fn validate_hierarchy(
    project: &MarklabProject,
    cell_ids: &[CellId],
    slide_id: &SlideId,
    pattern: &Pattern,
) -> Result<(), DeclaredScalarInputError> {
    let hierarchy = project
        .hierarchy()
        .ok_or(DeclaredScalarInputError::CohortHierarchyMissing)?;
    let slide = HierarchyId::from(slide_id.clone());
    if !hierarchy.contains(&slide)
        || pattern
            .meta
            .slide_id
            .as_deref()
            .is_some_and(|value| value != slide_id.as_str())
    {
        return Err(DeclaredScalarInputError::OwningSlideMismatch);
    }
    for (row, cell_id) in cell_ids.iter().enumerate() {
        if row > 0 && cell_ids[row - 1] >= *cell_id {
            return Err(DeclaredScalarInputError::NonCanonicalCellIds { row });
        }
        let mut current = HierarchyId::from(cell_id.clone());
        if !hierarchy.contains(&current) {
            return Err(DeclaredScalarInputError::CellHierarchyMismatch { row });
        }
        let mut owned = current == slide;
        while !owned {
            let Some(parent) = hierarchy.parent(&current) else {
                break;
            };
            owned = parent == &slide;
            current = parent.clone();
        }
        if !owned {
            return Err(DeclaredScalarInputError::CellHierarchyMismatch { row });
        }
    }
    Ok(())
}

fn validate_threshold_source(
    binary: &BinaryMarkDeclaration,
    probability: Option<&ProbabilityMarkDeclaration>,
) -> Result<(), DeclaredScalarInputError> {
    let BinaryMarkOrigin::Thresholded {
        probability_mark_id,
        ..
    } = &binary.origin
    else {
        return Ok(());
    };
    let Some(probability) = probability else {
        return Err(DeclaredScalarInputError::ThresholdSourceMismatch);
    };
    if probability.mark_id != *probability_mark_id
        || probability.measurement_status != binary.measurement_status
    {
        return Err(DeclaredScalarInputError::ThresholdSourceMismatch);
    }
    Ok(())
}

fn validate_threshold_rows(
    pattern: &Pattern,
    binary: &BinaryMarkDeclaration,
) -> Result<(), DeclaredScalarInputError> {
    let BinaryMarkOrigin::Thresholded {
        comparator,
        threshold,
        ..
    } = binary.origin
    else {
        return Ok(());
    };
    let probabilities = pattern
        .mark_prob
        .as_deref()
        .ok_or(DeclaredScalarInputError::ThresholdSourceMismatch)?;
    for (row, (&observed, &probability)) in pattern.mark.iter().zip(probabilities).enumerate() {
        let expected = u8::from(comparator.evaluate(probability, threshold));
        if observed != expected {
            return Err(DeclaredScalarInputError::ThresholdBindingMismatch { row });
        }
    }
    Ok(())
}
