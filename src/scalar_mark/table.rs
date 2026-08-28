use marklab_data::{CellId, CoordinateFrameId, MeasurementStatus, SlideId};
use marklab_embeddings::{CellEmbeddingArtifact, CellEmbeddingTable};
use marklab_workflow::{ArtifactId, ArtifactRef, ContentDigest, MarklabProject};

use crate::data::Pattern;

use super::identity::cell_ids_identity;
use super::{
    declaration::{
        BinaryMarkDeclaration, BinaryMarkOrigin, HistologicCompartmentMarkDeclaration,
        NucleusAreaUm2MarkDeclaration, OrdinalMarkDeclaration, ProbabilityMarkDeclaration,
        ProbabilitySimplexMarkDeclaration, VectorArtifactRefMarkDeclaration,
    },
    provenance::{
        validate_histologic_compartment_provenance, validate_nucleus_area_um2_provenance,
        validate_provenance,
    },
    DeclaredScalarInputError, ScalarMarkId,
};

mod identity;

/// Bounded modality vocabulary needed by the current scalar-mark caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarMarkModality {
    /// Histology-derived categorical annotation.
    Histology,
    /// Immunohistochemistry-derived scalar observation or prediction.
    Immunohistochemistry,
    /// Morphology-derived scalar observation or prediction.
    Morphology,
}

impl ScalarMarkModality {
    fn wire_name(self) -> &'static str {
        match self {
            Self::Histology => "histology",
            Self::Immunohistochemistry => "immunohistochemistry",
            Self::Morphology => "morphology",
        }
    }
}

/// Bounded scalar units needed by the current compatibility columns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarMarkUnit {
    /// Nominal category code with an explicit ordered codebook.
    Categorical,
    /// Dimensionless binary or probability value.
    Unitless,
    /// Square micrometres for the existing nucleus-area column.
    SquareMicrometer,
    /// Complete dimensionless class-probability vector.
    ProbabilitySimplex,
    /// Ordered categorical code with no interval-scale interpretation.
    Ordinal,
    /// Dimensionless high-dimensional morphology vector stored in a verified artifact.
    EmbeddingVector,
}

impl ScalarMarkUnit {
    fn wire_name(self) -> &'static str {
        match self {
            Self::Categorical => "categorical",
            Self::Unitless => "unitless",
            Self::SquareMicrometer => "square_micrometer",
            Self::ProbabilitySimplex => "probability_simplex",
            Self::Ordinal => "ordinal",
            Self::EmbeddingVector => "embedding_vector",
        }
    }
}

/// Explicit column-wide missingness contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingnessPolicy {
    /// Every row must contain a value.
    NotPermitted,
    /// Missing values are admissible.
    ///
    /// The compatibility Pattern adapter rejects this for materialized scalar columns. A vector
    /// artifact reference may use it because its verified embedding table owns explicit row status.
    Allowed,
}

impl MissingnessPolicy {
    fn wire_name(self) -> &'static str {
        match self {
            Self::NotPermitted => "not_permitted",
            Self::Allowed => "allowed",
        }
    }
}

/// One typed scalar value column or row-bound vector artifact aligned to every `MarkTable` row.
#[derive(Clone, Debug, PartialEq)]
pub struct ScalarMarkColumn {
    values: ScalarMarkColumnValues,
    modality: ScalarMarkModality,
    unit: ScalarMarkUnit,
    missingness: MissingnessPolicy,
}

#[derive(Clone, Debug, PartialEq)]
enum ScalarMarkColumnValues {
    Binary {
        declaration: BinaryMarkDeclaration,
        values: Box<[u8]>,
    },
    Probability {
        declaration: ProbabilityMarkDeclaration,
        values: Box<[f32]>,
    },
    Continuous {
        declaration: NucleusAreaUm2MarkDeclaration,
        values: Box<[f32]>,
    },
    Categorical {
        declaration: HistologicCompartmentMarkDeclaration,
        values: Box<[u32]>,
    },
    ProbabilitySimplex {
        declaration: ProbabilitySimplexMarkDeclaration,
        row_count: usize,
        values: Box<[f32]>,
    },
    Ordinal {
        declaration: OrdinalMarkDeclaration,
        values: Box<[u32]>,
    },
    VectorArtifactRef {
        declaration: VectorArtifactRefMarkDeclaration,
        artifact: Box<CellEmbeddingArtifact>,
        row_count: usize,
        cell_ids_logical_digest: ContentDigest,
    },
}

impl ScalarMarkColumn {
    /// Construct a dense binary column with explicit column-wide semantics.
    pub fn binary(
        declaration: BinaryMarkDeclaration,
        modality: ScalarMarkModality,
        unit: ScalarMarkUnit,
        missingness: MissingnessPolicy,
        values: impl Into<Box<[u8]>>,
    ) -> Result<Self, DeclaredScalarInputError> {
        if unit != ScalarMarkUnit::Unitless {
            return Err(DeclaredScalarInputError::UnitMismatch);
        }
        let values = values.into();
        if let Some(row) = values.iter().position(|value| *value > 1) {
            return Err(DeclaredScalarInputError::InvalidBinaryMark { row });
        }
        Ok(Self {
            values: ScalarMarkColumnValues::Binary {
                declaration,
                values,
            },
            modality,
            unit,
            missingness,
        })
    }

    /// Construct a dense probability column with explicit column-wide semantics.
    pub fn probability(
        declaration: ProbabilityMarkDeclaration,
        modality: ScalarMarkModality,
        unit: ScalarMarkUnit,
        missingness: MissingnessPolicy,
        values: impl Into<Box<[f32]>>,
    ) -> Result<Self, DeclaredScalarInputError> {
        if unit != ScalarMarkUnit::Unitless {
            return Err(DeclaredScalarInputError::UnitMismatch);
        }
        let values = values.into();
        if let Some(row) = values
            .iter()
            .position(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
        {
            return Err(DeclaredScalarInputError::InvalidProbability { row });
        }
        Ok(Self {
            values: ScalarMarkColumnValues::Probability {
                declaration,
                values,
            },
            modality,
            unit,
            missingness,
        })
    }

    /// Construct the current finite positive nucleus-area continuous column.
    pub fn continuous(
        declaration: NucleusAreaUm2MarkDeclaration,
        modality: ScalarMarkModality,
        unit: ScalarMarkUnit,
        missingness: MissingnessPolicy,
        values: impl Into<Box<[f32]>>,
    ) -> Result<Self, DeclaredScalarInputError> {
        if modality != ScalarMarkModality::Morphology || unit != ScalarMarkUnit::SquareMicrometer {
            return Err(DeclaredScalarInputError::UnitMismatch);
        }
        let values = values.into();
        if let Some(row) = values
            .iter()
            .position(|value| !value.is_finite() || *value <= 0.0)
        {
            return Err(DeclaredScalarInputError::InvalidContinuousValue { row });
        }
        Ok(Self {
            values: ScalarMarkColumnValues::Continuous {
                declaration,
                values,
            },
            modality,
            unit,
            missingness,
        })
    }

    /// Construct the current dense histologic-compartment categorical column.
    pub fn histologic_compartment(
        declaration: HistologicCompartmentMarkDeclaration,
        modality: ScalarMarkModality,
        unit: ScalarMarkUnit,
        missingness: MissingnessPolicy,
        values: impl Into<Box<[u32]>>,
    ) -> Result<Self, DeclaredScalarInputError> {
        if modality != ScalarMarkModality::Histology || unit != ScalarMarkUnit::Categorical {
            return Err(DeclaredScalarInputError::UnitMismatch);
        }
        let values = values.into();
        if let Some((row, code)) = values.iter().copied().enumerate().find(|(_, code)| {
            usize::try_from(*code).map_or(true, |code| code >= declaration.levels().len())
        }) {
            return Err(DeclaredScalarInputError::InvalidCategoricalValue {
                row,
                code,
                level_count: declaration.levels().len(),
            });
        }
        Ok(Self {
            values: ScalarMarkColumnValues::Categorical {
                declaration,
                values,
            },
            modality,
            unit,
            missingness,
        })
    }

    /// Construct complete contiguous probability-simplex rows without renormalizing values.
    pub fn probability_simplex(
        declaration: ProbabilitySimplexMarkDeclaration,
        modality: ScalarMarkModality,
        unit: ScalarMarkUnit,
        missingness: MissingnessPolicy,
        rows: Vec<Vec<f32>>,
    ) -> Result<Self, DeclaredScalarInputError> {
        if modality != ScalarMarkModality::Morphology || unit != ScalarMarkUnit::ProbabilitySimplex
        {
            return Err(DeclaredScalarInputError::UnitMismatch);
        }
        let width = declaration.levels().len();
        let row_count = rows.len();
        let value_count = row_count
            .checked_mul(width)
            .ok_or(DeclaredScalarInputError::LogicalIdentityOverflow)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(value_count)
            .map_err(|_| DeclaredScalarInputError::LogicalIdentityOverflow)?;
        for (row, current) in rows.into_iter().enumerate() {
            if current.len() != width {
                return Err(DeclaredScalarInputError::InvalidProbabilitySimplexShape {
                    row,
                    expected: width,
                    observed: current.len(),
                });
            }
            let sum = current.iter().map(|value| f64::from(*value)).sum::<f64>();
            if current
                .iter()
                .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
                || !sum.is_finite()
                || (sum - 1.0).abs() > 1e-5
            {
                return Err(DeclaredScalarInputError::InvalidProbabilitySimplexRow { row, sum });
            }
            values.extend(current);
        }
        Ok(Self {
            values: ScalarMarkColumnValues::ProbabilitySimplex {
                declaration,
                row_count,
                values: values.into_boxed_slice(),
            },
            modality,
            unit,
            missingness,
        })
    }

    /// Construct one complete measured-IHC ordinal column without interval arithmetic.
    pub fn ordinal(
        declaration: OrdinalMarkDeclaration,
        modality: ScalarMarkModality,
        unit: ScalarMarkUnit,
        missingness: MissingnessPolicy,
        values: impl Into<Box<[u32]>>,
    ) -> Result<Self, DeclaredScalarInputError> {
        if unit != ScalarMarkUnit::Ordinal {
            return Err(DeclaredScalarInputError::UnitMismatch);
        }
        if modality != ScalarMarkModality::Immunohistochemistry {
            return Err(DeclaredScalarInputError::OrdinalModalityMismatch);
        }
        let values = values.into();
        if let Some((row, code)) = values
            .iter()
            .copied()
            .enumerate()
            .find(|(_, code)| *code as usize >= declaration.levels().len())
        {
            return Err(DeclaredScalarInputError::InvalidOrdinalValue {
                row,
                code,
                level_count: declaration.levels().len(),
            });
        }
        Ok(Self {
            values: ScalarMarkColumnValues::Ordinal {
                declaration,
                values,
            },
            modality,
            unit,
            missingness,
        })
    }

    /// Bind one verified cell-embedding artifact to exact ordered MarkTable rows.
    ///
    /// The matrix remains owned by the existing embedding table/artifact store; this column retains
    /// metadata and row identity only.
    pub fn vector_artifact_ref(
        declaration: VectorArtifactRefMarkDeclaration,
        modality: ScalarMarkModality,
        unit: ScalarMarkUnit,
        missingness: MissingnessPolicy,
        table: &CellEmbeddingTable,
        artifact: CellEmbeddingArtifact,
    ) -> Result<Self, DeclaredScalarInputError> {
        if unit != ScalarMarkUnit::EmbeddingVector {
            return Err(DeclaredScalarInputError::UnitMismatch);
        }
        let row_count = table.row_count();
        let row_count_u64 = u64::try_from(row_count)
            .map_err(|_| DeclaredScalarInputError::LogicalIdentityOverflow)?;
        let qc = table.qc_summary();
        if qc != artifact.qc_summary()
            || row_count_u64 != artifact.row_count()
            || table.dimension() != artifact.dimension()
            || qc.logical_digest() != artifact.logical_digest()
        {
            return Err(DeclaredScalarInputError::VectorArtifactBindingMismatch);
        }
        if missingness == MissingnessPolicy::NotPermitted && qc.present_count() != qc.row_count() {
            return Err(DeclaredScalarInputError::VectorArtifactMissingnessMismatch);
        }
        let (cell_ids_logical_digest, _) = cell_ids_identity(table.cell_ids())?;
        Ok(Self {
            values: ScalarMarkColumnValues::VectorArtifactRef {
                declaration,
                artifact: Box::new(artifact),
                row_count,
                cell_ids_logical_digest,
            },
            modality,
            unit,
            missingness,
        })
    }

    fn mark_id(&self) -> &ScalarMarkId {
        match &self.values {
            ScalarMarkColumnValues::Binary { declaration, .. } => declaration.mark_id(),
            ScalarMarkColumnValues::Probability { declaration, .. } => declaration.mark_id(),
            ScalarMarkColumnValues::Continuous { declaration, .. } => declaration.mark_id(),
            ScalarMarkColumnValues::Categorical { declaration, .. } => declaration.mark_id(),
            ScalarMarkColumnValues::ProbabilitySimplex { declaration, .. } => declaration.mark_id(),
            ScalarMarkColumnValues::Ordinal { declaration, .. } => declaration.mark_id(),
            ScalarMarkColumnValues::VectorArtifactRef { declaration, .. } => declaration.mark_id(),
        }
    }

    fn len(&self) -> usize {
        match &self.values {
            ScalarMarkColumnValues::Binary { values, .. } => values.len(),
            ScalarMarkColumnValues::Probability { values, .. } => values.len(),
            ScalarMarkColumnValues::Continuous { values, .. } => values.len(),
            ScalarMarkColumnValues::Categorical { values, .. } => values.len(),
            ScalarMarkColumnValues::ProbabilitySimplex { row_count, .. } => *row_count,
            ScalarMarkColumnValues::Ordinal { values, .. } => values.len(),
            ScalarMarkColumnValues::VectorArtifactRef { row_count, .. } => *row_count,
        }
    }

    fn missingness(&self) -> MissingnessPolicy {
        self.missingness
    }
}

/// Stable `CellId` rows with row-aligned typed mark columns.
pub struct MarkTable {
    cell_ids: Box<[CellId]>,
    columns: Box<[ScalarMarkColumn]>,
    cell_id_text_bytes: usize,
}

impl MarkTable {
    /// Construct a bounded row-aligned table and validate every retained value or artifact binding.
    pub fn new(
        cell_ids: Vec<CellId>,
        columns: Vec<ScalarMarkColumn>,
        maximum_rows: usize,
        maximum_cell_id_text_bytes: usize,
    ) -> Result<Self, DeclaredScalarInputError> {
        if maximum_rows == 0 || maximum_cell_id_text_bytes == 0 {
            return Err(DeclaredScalarInputError::InvalidResourceLimit);
        }
        if cell_ids.len() > maximum_rows {
            return Err(DeclaredScalarInputError::RowCountExceeded {
                observed: cell_ids.len(),
                maximum: maximum_rows,
            });
        }
        for row in 1..cell_ids.len() {
            if cell_ids[row - 1] >= cell_ids[row] {
                return Err(DeclaredScalarInputError::NonCanonicalCellIds { row });
            }
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
        if columns.is_empty() {
            return Err(DeclaredScalarInputError::EmptyMarkTable);
        }
        for column in &columns {
            if column.len() != cell_ids.len() {
                return Err(DeclaredScalarInputError::MarkColumnLengthMismatch {
                    expected: cell_ids.len(),
                    observed: column.len(),
                });
            }
        }
        let (cell_ids_logical_digest, _) = cell_ids_identity(&cell_ids)?;
        if columns.iter().any(|column| {
            matches!(
                &column.values,
                ScalarMarkColumnValues::VectorArtifactRef {
                    cell_ids_logical_digest: observed,
                    ..
                } if *observed != cell_ids_logical_digest
            )
        }) {
            return Err(DeclaredScalarInputError::VectorArtifactCellIdentityMismatch);
        }
        let mut mark_ids = columns
            .iter()
            .map(ScalarMarkColumn::mark_id)
            .collect::<Vec<_>>();
        mark_ids.sort_unstable();
        if mark_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DeclaredScalarInputError::DuplicateMarkId);
        }
        let binary_count = columns
            .iter()
            .filter(|column| matches!(&column.values, ScalarMarkColumnValues::Binary { .. }))
            .count();
        if binary_count != 1 {
            return Err(DeclaredScalarInputError::BinaryColumnCountMismatch);
        }
        let probability_count = columns
            .iter()
            .filter(|column| matches!(&column.values, ScalarMarkColumnValues::Probability { .. }))
            .count();
        if probability_count > 1 {
            return Err(DeclaredScalarInputError::ProbabilityColumnCountMismatch);
        }
        let continuous_count = columns
            .iter()
            .filter(|column| matches!(&column.values, ScalarMarkColumnValues::Continuous { .. }))
            .count();
        if continuous_count > 1 {
            return Err(DeclaredScalarInputError::ContinuousColumnCountMismatch);
        }
        let categorical_count = columns
            .iter()
            .filter(|column| matches!(&column.values, ScalarMarkColumnValues::Categorical { .. }))
            .count();
        if categorical_count > 1 {
            return Err(DeclaredScalarInputError::CategoricalColumnCountMismatch);
        }
        let simplex_count = columns
            .iter()
            .filter(|column| {
                matches!(
                    &column.values,
                    ScalarMarkColumnValues::ProbabilitySimplex { .. }
                )
            })
            .count();
        if simplex_count > 1 {
            return Err(DeclaredScalarInputError::ProbabilitySimplexColumnCountMismatch);
        }
        let ordinal_count = columns
            .iter()
            .filter(|column| matches!(&column.values, ScalarMarkColumnValues::Ordinal { .. }))
            .count();
        if ordinal_count > 1 {
            return Err(DeclaredScalarInputError::OrdinalColumnCountMismatch);
        }
        let vector_ref_count = columns
            .iter()
            .filter(|column| {
                matches!(
                    &column.values,
                    ScalarMarkColumnValues::VectorArtifactRef { .. }
                )
            })
            .count();
        if vector_ref_count > 1 {
            return Err(DeclaredScalarInputError::VectorArtifactRefColumnCountMismatch);
        }
        let table = Self {
            cell_ids: cell_ids.into_boxed_slice(),
            columns: columns.into_boxed_slice(),
            cell_id_text_bytes: required,
        };
        table.semantic_artifact_ids()?;
        Ok(table)
    }

    /// Ordered stable row identities.
    pub fn cell_ids(&self) -> &[CellId] {
        &self.cell_ids
    }

    /// Borrow one exact dense continuous column by stable mark identity.
    pub fn continuous_values(&self, mark_id: &ScalarMarkId) -> Option<&[f32]> {
        self.columns.iter().find_map(|column| match &column.values {
            ScalarMarkColumnValues::Continuous {
                declaration,
                values,
            } if declaration.mark_id() == mark_id => Some(values.as_ref()),
            _ => None,
        })
    }

    /// Borrow one exact dense probability column by stable mark identity.
    pub fn probability_values(&self, mark_id: &ScalarMarkId) -> Option<&[f32]> {
        self.columns.iter().find_map(|column| match &column.values {
            ScalarMarkColumnValues::Probability {
                declaration,
                values,
            } if declaration.mark_id() == mark_id => Some(values.as_ref()),
            _ => None,
        })
    }

    /// Borrow one exact dense categorical column by stable mark identity.
    pub fn categorical_values(&self, mark_id: &ScalarMarkId) -> Option<&[u32]> {
        self.columns.iter().find_map(|column| match &column.values {
            ScalarMarkColumnValues::Categorical {
                declaration,
                values,
            } if declaration.mark_id() == mark_id => Some(values.as_ref()),
            _ => None,
        })
    }

    /// Borrow the exact ordered level codebook of one categorical column.
    pub fn categorical_levels(&self, mark_id: &ScalarMarkId) -> Option<&[String]> {
        self.columns.iter().find_map(|column| match &column.values {
            ScalarMarkColumnValues::Categorical { declaration, .. }
                if declaration.mark_id() == mark_id =>
            {
                Some(declaration.levels())
            }
            _ => None,
        })
    }

    /// Borrow one complete row-major simplex value buffer by stable mark identity.
    pub fn probability_simplex_values(&self, mark_id: &ScalarMarkId) -> Option<&[f32]> {
        self.columns.iter().find_map(|column| match &column.values {
            ScalarMarkColumnValues::ProbabilitySimplex {
                declaration,
                values,
                ..
            } if declaration.mark_id() == mark_id => Some(values.as_ref()),
            _ => None,
        })
    }

    /// Borrow the ordered class codebook for one simplex column.
    pub fn probability_simplex_levels(&self, mark_id: &ScalarMarkId) -> Option<&[String]> {
        self.columns.iter().find_map(|column| match &column.values {
            ScalarMarkColumnValues::ProbabilitySimplex { declaration, .. }
                if declaration.mark_id() == mark_id =>
            {
                Some(declaration.levels())
            }
            _ => None,
        })
    }

    /// Borrow one exact dense ordinal code column by stable mark identity.
    pub fn ordinal_values(&self, mark_id: &ScalarMarkId) -> Option<&[u32]> {
        self.columns.iter().find_map(|column| match &column.values {
            ScalarMarkColumnValues::Ordinal {
                declaration,
                values,
            } if declaration.mark_id() == mark_id => Some(values.as_ref()),
            _ => None,
        })
    }

    /// Borrow the exact ordered level codebook for one ordinal mark.
    pub fn ordinal_levels(&self, mark_id: &ScalarMarkId) -> Option<&[String]> {
        self.columns.iter().find_map(|column| match &column.values {
            ScalarMarkColumnValues::Ordinal { declaration, .. }
                if declaration.mark_id() == mark_id =>
            {
                Some(declaration.levels())
            }
            _ => None,
        })
    }

    /// Return the compact verified embedding artifact for one exact vector mark.
    pub fn vector_artifact_ref(&self, mark_id: &ScalarMarkId) -> Option<CellEmbeddingArtifact> {
        self.columns.iter().find_map(|column| match &column.values {
            ScalarMarkColumnValues::VectorArtifactRef {
                declaration,
                artifact,
                ..
            } if declaration.mark_id() == mark_id => Some(**artifact),
            _ => None,
        })
    }

    pub(crate) fn single_vector_artifact_ref(
        &self,
    ) -> Option<(&VectorArtifactRefMarkDeclaration, CellEmbeddingArtifact)> {
        self.columns.iter().find_map(|column| match &column.values {
            ScalarMarkColumnValues::VectorArtifactRef {
                declaration,
                artifact,
                ..
            } => Some((declaration, **artifact)),
            _ => None,
        })
    }

    /// Column-wide measurement status for one exact typed mark.
    pub fn measurement_status(&self, mark_id: &ScalarMarkId) -> Option<MeasurementStatus> {
        self.columns
            .iter()
            .find(|column| column.mark_id() == mark_id)
            .map(|column| match &column.values {
                ScalarMarkColumnValues::Binary { declaration, .. } => {
                    declaration.measurement_status()
                }
                ScalarMarkColumnValues::Probability { declaration, .. } => {
                    declaration.measurement_status()
                }
                ScalarMarkColumnValues::Continuous { declaration, .. } => {
                    declaration.measurement_status()
                }
                ScalarMarkColumnValues::Categorical { declaration, .. } => {
                    declaration.measurement_status()
                }
                ScalarMarkColumnValues::ProbabilitySimplex { declaration, .. } => {
                    declaration.measurement_status()
                }
                ScalarMarkColumnValues::Ordinal { declaration, .. } => {
                    declaration.measurement_status()
                }
                ScalarMarkColumnValues::VectorArtifactRef { declaration, .. } => {
                    declaration.measurement_status()
                }
            })
    }

    pub(super) fn binary_column(&self) -> Result<&ScalarMarkColumn, DeclaredScalarInputError> {
        let mut columns = self
            .columns
            .iter()
            .filter(|column| matches!(&column.values, ScalarMarkColumnValues::Binary { .. }));
        let column = columns
            .next()
            .ok_or(DeclaredScalarInputError::BinaryColumnCountMismatch)?;
        if columns.next().is_some() {
            return Err(DeclaredScalarInputError::BinaryColumnCountMismatch);
        }
        Ok(column)
    }

    pub(super) fn probability_column(
        &self,
    ) -> Result<Option<&ScalarMarkColumn>, DeclaredScalarInputError> {
        let mut columns = self
            .columns
            .iter()
            .filter(|column| matches!(&column.values, ScalarMarkColumnValues::Probability { .. }));
        let column = columns.next();
        if columns.next().is_some() {
            return Err(DeclaredScalarInputError::ProbabilityColumnCountMismatch);
        }
        Ok(column)
    }

    pub(super) fn validate_for_pattern(
        &self,
        pattern: &Pattern,
    ) -> Result<(), DeclaredScalarInputError> {
        if self.cell_ids.len() != pattern.len() {
            return Err(DeclaredScalarInputError::CellIdCountMismatch {
                expected: pattern.len(),
                observed: self.cell_ids.len(),
            });
        }
        if self.columns.iter().any(|column| {
            column.missingness() != MissingnessPolicy::NotPermitted
                && !matches!(
                    &column.values,
                    ScalarMarkColumnValues::VectorArtifactRef { .. }
                )
        }) {
            return Err(DeclaredScalarInputError::UnsupportedMissingnessPolicy);
        }
        let binary = self.binary_column()?;
        let ScalarMarkColumnValues::Binary { values, .. } = &binary.values else {
            unreachable!()
        };
        if let Some(row) = values
            .iter()
            .zip(&pattern.mark)
            .position(|(left, right)| left != right)
        {
            return Err(DeclaredScalarInputError::PatternMarkValueMismatch { row });
        }
        let probability = self.probability_column()?;
        match (probability, pattern.mark_prob.as_deref()) {
            (None, None) => {}
            (
                Some(ScalarMarkColumn {
                    values: ScalarMarkColumnValues::Probability { values, .. },
                    ..
                }),
                Some(pattern_values),
            ) => {
                if values.len() != pattern_values.len() {
                    return Err(DeclaredScalarInputError::ProbabilityLengthMismatch {
                        expected: pattern_values.len(),
                        observed: values.len(),
                    });
                }
                if let Some(row) = values
                    .iter()
                    .zip(pattern_values)
                    .position(|(left, right)| left.to_bits() != right.to_bits())
                {
                    return Err(DeclaredScalarInputError::PatternMarkValueMismatch { row });
                }
            }
            _ => return Err(DeclaredScalarInputError::ProbabilityDeclarationMismatch),
        }
        let continuous = self
            .columns
            .iter()
            .filter_map(|column| match &column.values {
                ScalarMarkColumnValues::Continuous { values, .. } => Some(values.as_ref()),
                _ => None,
            })
            .collect::<Vec<_>>();
        if continuous.len() > 1 {
            return Err(DeclaredScalarInputError::ContinuousColumnCountMismatch);
        }
        match (continuous.first(), pattern.nucleus_area_um2.as_deref()) {
            (None, None) => {}
            (Some(values), Some(pattern_values)) => {
                if values.len() != pattern_values.len() {
                    return Err(DeclaredScalarInputError::MarkColumnLengthMismatch {
                        expected: pattern_values.len(),
                        observed: values.len(),
                    });
                }
                if let Some(row) = values
                    .iter()
                    .zip(pattern_values)
                    .position(|(left, right)| left.to_bits() != right.to_bits())
                {
                    return Err(DeclaredScalarInputError::PatternMarkValueMismatch { row });
                }
            }
            _ => return Err(DeclaredScalarInputError::ContinuousDeclarationMismatch),
        }
        let categorical = self
            .columns
            .iter()
            .filter_map(|column| match &column.values {
                ScalarMarkColumnValues::Categorical { values, .. } => Some(values.as_ref()),
                _ => None,
            })
            .collect::<Vec<_>>();
        if categorical.len() > 1 {
            return Err(DeclaredScalarInputError::CategoricalColumnCountMismatch);
        }
        match (
            categorical.first(),
            pattern
                .categorical_strata
                .get("histologic_compartment")
                .map(Box::as_ref),
        ) {
            (None, None) => {}
            (Some(values), Some(pattern_values)) if *values == pattern_values => {}
            _ => return Err(DeclaredScalarInputError::CategoricalDeclarationMismatch),
        }
        if let Some(levels) = pattern
            .categorical_stratum_levels
            .get("histologic_compartment")
        {
            let declared = self.columns.iter().find_map(|column| match &column.values {
                ScalarMarkColumnValues::Categorical { declaration, .. } => {
                    Some(declaration.levels())
                }
                _ => None,
            });
            if declared != Some(levels.as_ref()) {
                return Err(DeclaredScalarInputError::CategoricalDeclarationMismatch);
            }
        }
        Ok(())
    }

    pub(super) fn declarations(
        &self,
    ) -> Result<(BinaryMarkDeclaration, Option<ProbabilityMarkDeclaration>), DeclaredScalarInputError>
    {
        let binary_column = self.binary_column()?;
        let ScalarMarkColumnValues::Binary {
            declaration: binary_declaration,
            ..
        } = &binary_column.values
        else {
            unreachable!()
        };
        let binary_modality = binary_column.modality;
        let probability = match self.probability_column()? {
            Some(column) => {
                let ScalarMarkColumnValues::Probability { declaration, .. } = &column.values else {
                    unreachable!()
                };
                if matches!(
                    declaration.measurement_status(),
                    MeasurementStatus::DerivedSummary
                ) {
                    return Err(DeclaredScalarInputError::UnsupportedPerCellMeasurementStatus);
                }
                if matches!(
                    binary_declaration.origin(),
                    BinaryMarkOrigin::Thresholded { .. }
                ) && column.modality != binary_modality
                {
                    return Err(DeclaredScalarInputError::ThresholdModalityMismatch);
                }
                Some(declaration.clone())
            }
            None => None,
        };
        Ok((binary_declaration.clone(), probability))
    }

    pub(super) fn validate_provenance(
        &self,
        project: &MarklabProject,
    ) -> Result<(), DeclaredScalarInputError> {
        let (binary, probability) = self.declarations()?;
        validate_provenance(project, &binary, probability.as_ref())?;
        for column in &self.columns {
            match &column.values {
                ScalarMarkColumnValues::Continuous { declaration, .. } => {
                    validate_nucleus_area_um2_provenance(project, declaration)?
                }
                ScalarMarkColumnValues::Categorical { declaration, .. } => {
                    validate_histologic_compartment_provenance(project, declaration)?
                }
                ScalarMarkColumnValues::ProbabilitySimplex { declaration, .. } => {
                    super::provenance::validate_probability_simplex_provenance(
                        project,
                        declaration,
                    )?
                }
                ScalarMarkColumnValues::Ordinal { declaration, .. } => {
                    super::provenance::validate_ordinal_provenance(project, declaration)?
                }
                ScalarMarkColumnValues::VectorArtifactRef { .. } => {}
                ScalarMarkColumnValues::Binary { .. }
                | ScalarMarkColumnValues::Probability { .. } => {}
            }
        }
        Ok(())
    }

    pub(super) fn semantic_artifact_ids(
        &self,
    ) -> Result<Box<[ArtifactId]>, DeclaredScalarInputError> {
        let mut ids = Vec::with_capacity(self.columns.len() + 1);
        for column in &self.columns {
            match &column.values {
                ScalarMarkColumnValues::Binary { declaration, .. } => {
                    ids.push(declaration.provenance_artifact_id());
                    if let BinaryMarkOrigin::Thresholded {
                        threshold_provenance_artifact_id,
                        ..
                    } = declaration.origin()
                    {
                        ids.push(*threshold_provenance_artifact_id);
                    }
                }
                ScalarMarkColumnValues::Probability { declaration, .. } => {
                    ids.push(declaration.provenance_artifact_id())
                }
                ScalarMarkColumnValues::Continuous { declaration, .. } => {
                    ids.push(declaration.provenance_artifact_id())
                }
                ScalarMarkColumnValues::Categorical { declaration, .. } => {
                    ids.push(declaration.provenance_artifact_id())
                }
                ScalarMarkColumnValues::ProbabilitySimplex { declaration, .. } => {
                    ids.push(declaration.provenance_artifact_id())
                }
                ScalarMarkColumnValues::Ordinal { declaration, .. } => {
                    ids.push(declaration.provenance_artifact_id())
                }
                ScalarMarkColumnValues::VectorArtifactRef { artifact, .. } => ids.extend([
                    artifact.embedding_artifact_id(),
                    artifact.expected_cells_artifact_id(),
                    artifact.row_link_artifact_id(),
                    artifact.provenance_artifact_id(),
                ]),
            }
        }
        let mut distinct = ids.clone();
        distinct.sort_unstable();
        if distinct.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DeclaredScalarInputError::DuplicateArtifactRole);
        }
        Ok(ids.into_boxed_slice())
    }

    pub(super) fn declared_artifact_ref(
        &self,
        slide_id: &SlideId,
        frame_id: &CoordinateFrameId,
    ) -> Result<ArtifactRef, DeclaredScalarInputError> {
        identity::declared_artifact_ref(self, slide_id, frame_id)
    }

    pub(super) fn cell_id_text_bytes(&self) -> usize {
        self.cell_id_text_bytes
    }
}
