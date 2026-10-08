#[derive(Clone, Debug, PartialEq)]
pub(crate) struct F64Matrix {
    rows: usize,
    columns: usize,
    values: Vec<f64>,
}

impl F64Matrix {
    pub(crate) fn zeros(rows: usize, columns: usize) -> Option<Self> {
        if rows == 0 || columns == 0 {
            return None;
        }
        let len = rows.checked_mul(columns)?;
        Some(Self {
            rows,
            columns,
            values: vec![0.0; len],
        })
    }

    pub(crate) fn from_rows(rows: &[Vec<f64>]) -> Option<Self> {
        let columns = rows.first()?.len();
        if columns == 0 || rows.iter().any(|row| row.len() != columns) {
            return None;
        }
        let mut matrix = Self::zeros(rows.len(), columns)?;
        for (target, source) in matrix.iter_rows_mut().zip(rows) {
            target.copy_from_slice(source);
        }
        Some(matrix)
    }

    pub(crate) fn row_count(&self) -> usize {
        self.rows
    }

    pub(crate) fn column_count(&self) -> usize {
        self.columns
    }

    pub(crate) fn row(&self, row: usize) -> Option<&[f64]> {
        let start = row.checked_mul(self.columns)?;
        self.values.get(start..start.checked_add(self.columns)?)
    }

    pub(crate) fn row_mut(&mut self, row: usize) -> Option<&mut [f64]> {
        let start = row.checked_mul(self.columns)?;
        self.values.get_mut(start..start.checked_add(self.columns)?)
    }

    pub(crate) fn iter_rows(&self) -> impl ExactSizeIterator<Item = &[f64]> {
        self.values.chunks_exact(self.columns)
    }

    pub(crate) fn iter_rows_mut(&mut self) -> impl ExactSizeIterator<Item = &mut [f64]> {
        self.values.chunks_exact_mut(self.columns)
    }

    pub(crate) fn values(&self) -> &[f64] {
        &self.values
    }

    #[cfg(feature = "parallel")]
    pub(crate) fn values_mut(&mut self) -> &mut [f64] {
        &mut self.values
    }

    #[cfg(test)]
    pub(crate) fn estimated_storage_bytes(&self) -> usize {
        self.values
            .capacity()
            .saturating_mul(std::mem::size_of::<f64>())
    }
}

#[cfg(test)]
mod tests {
    use super::F64Matrix;

    #[test]
    fn checked_contiguous_matrix_has_stable_rows() {
        assert!(F64Matrix::zeros(usize::MAX, 2).is_none());
        let mut matrix = F64Matrix::zeros(2, 3).expect("matrix");
        matrix
            .row_mut(1)
            .expect("row")
            .copy_from_slice(&[1.0, 2.0, 3.0]);

        assert_eq!(matrix.row_count(), 2);
        assert_eq!(matrix.column_count(), 3);
        assert_eq!(matrix.values(), &[0.0, 0.0, 0.0, 1.0, 2.0, 3.0]);
        assert_eq!(matrix.estimated_storage_bytes(), 6 * 8);
        assert!(matrix.row(2).is_none());
    }
}
