use super::*;

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
    pub(super) fn wire_name(self) -> &'static str {
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
    pub(super) fn wire_name(self) -> &'static str {
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
    pub(super) fn wire_name(self) -> &'static str {
        match self {
            Self::NotPermitted => "not_permitted",
            Self::Allowed => "allowed",
        }
    }
}

/// One typed scalar value column or row-bound vector artifact aligned to every `MarkTable` row.
#[derive(Clone, Debug, PartialEq)]
pub struct ScalarMarkColumn {
    pub(super) values: ScalarMarkColumnValues,
    pub(super) compatibility_semantics: Option<(ScalarMarkModality, ScalarMarkUnit)>,
    pub(super) missingness: MissingnessPolicy,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum ScalarMarkColumnValues {
    Assay {
        declaration: AssayMarkDeclaration,
        values: AssayMarkValues,
    },
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
            compatibility_semantics: Some((modality, unit)),
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
            compatibility_semantics: Some((modality, unit)),
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
            compatibility_semantics: Some((modality, unit)),
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
            compatibility_semantics: Some((modality, unit)),
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
            compatibility_semantics: Some((modality, unit)),
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
            compatibility_semantics: Some((modality, unit)),
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
            compatibility_semantics: Some((modality, unit)),
            missingness,
        })
    }

    pub(super) fn mark_id(&self) -> &ScalarMarkId {
        match &self.values {
            ScalarMarkColumnValues::Assay { declaration, .. } => declaration.mark_id(),
            ScalarMarkColumnValues::Binary { declaration, .. } => declaration.mark_id(),
            ScalarMarkColumnValues::Probability { declaration, .. } => declaration.mark_id(),
            ScalarMarkColumnValues::Continuous { declaration, .. } => declaration.mark_id(),
            ScalarMarkColumnValues::Categorical { declaration, .. } => declaration.mark_id(),
            ScalarMarkColumnValues::ProbabilitySimplex { declaration, .. } => declaration.mark_id(),
            ScalarMarkColumnValues::Ordinal { declaration, .. } => declaration.mark_id(),
            ScalarMarkColumnValues::VectorArtifactRef { declaration, .. } => declaration.mark_id(),
        }
    }

    pub(super) fn len(&self) -> usize {
        match &self.values {
            ScalarMarkColumnValues::Assay { values, .. } => values.len(),
            ScalarMarkColumnValues::Binary { values, .. } => values.len(),
            ScalarMarkColumnValues::Probability { values, .. } => values.len(),
            ScalarMarkColumnValues::Continuous { values, .. } => values.len(),
            ScalarMarkColumnValues::Categorical { values, .. } => values.len(),
            ScalarMarkColumnValues::ProbabilitySimplex { row_count, .. } => *row_count,
            ScalarMarkColumnValues::Ordinal { values, .. } => values.len(),
            ScalarMarkColumnValues::VectorArtifactRef { row_count, .. } => *row_count,
        }
    }

    pub(super) fn missingness(&self) -> MissingnessPolicy {
        self.missingness
    }
}
