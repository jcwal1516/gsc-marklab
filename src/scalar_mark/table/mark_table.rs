use super::*;

/// Stable `CellId` rows with row-aligned typed mark columns.
pub struct MarkTable {
    pub(super) cell_ids: Box<[CellId]>,
    pub(super) columns: Box<[ScalarMarkColumn]>,
    pub(super) cell_id_text_bytes: usize,
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
        if columns.len() > 256 {
            return Err(DeclaredScalarInputError::analysis(
                crate::MarklabError::Validation(format!(
                    "mark table has {} columns; maximum is 256",
                    columns.len()
                )),
            ));
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

    pub(in crate::scalar_mark) fn binary_column(
        &self,
    ) -> Result<&ScalarMarkColumn, DeclaredScalarInputError> {
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

    pub(in crate::scalar_mark) fn probability_column(
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

    pub(in crate::scalar_mark) fn cell_id_text_bytes(&self) -> usize {
        self.cell_id_text_bytes
    }
}
