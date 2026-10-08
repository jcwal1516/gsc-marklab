use crate::LongitudinalError;

#[derive(Clone, Debug)]
pub(crate) struct Matrix {
    rows: usize,
    cols: usize,
    values: Vec<f64>,
}

impl Matrix {
    pub(crate) fn from_rows(
        values: &[Vec<f64>],
        rows: usize,
        cols: usize,
        name: &str,
    ) -> Result<Self, LongitudinalError> {
        if values.len() != rows || values.iter().any(|row| row.len() != cols) {
            return Err(LongitudinalError::Invalid(format!(
                "{name} must have shape {rows} by {cols}"
            )));
        }
        if values.iter().flatten().any(|value| !value.is_finite()) {
            return Err(LongitudinalError::Invalid(format!(
                "{name} must contain only finite values"
            )));
        }
        Ok(Self {
            rows,
            cols,
            values: values.iter().flatten().copied().collect(),
        })
    }

    pub(crate) fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            values: vec![0.0; rows * cols],
        }
    }

    pub(crate) fn identity(size: usize) -> Self {
        let mut result = Self::zeros(size, size);
        for index in 0..size {
            result.set(index, index, 1.0);
        }
        result
    }

    pub(crate) fn rows(&self) -> usize {
        self.rows
    }

    pub(crate) fn get(&self, row: usize, col: usize) -> f64 {
        self.values[row * self.cols + col]
    }

    pub(crate) fn set(&mut self, row: usize, col: usize, value: f64) {
        self.values[row * self.cols + col] = value;
    }

    pub(crate) fn transpose(&self) -> Self {
        let mut result = Self::zeros(self.cols, self.rows);
        for row in 0..self.rows {
            for col in 0..self.cols {
                result.set(col, row, self.get(row, col));
            }
        }
        result
    }

    pub(crate) fn add(&self, other: &Self) -> Self {
        let values = self
            .values
            .iter()
            .zip(&other.values)
            .map(|(left, right)| left + right)
            .collect();
        Self {
            rows: self.rows,
            cols: self.cols,
            values,
        }
    }

    pub(crate) fn sub(&self, other: &Self) -> Self {
        let values = self
            .values
            .iter()
            .zip(&other.values)
            .map(|(left, right)| left - right)
            .collect();
        Self {
            rows: self.rows,
            cols: self.cols,
            values,
        }
    }

    pub(crate) fn mul(&self, other: &Self) -> Self {
        let mut result = Self::zeros(self.rows, other.cols);
        for row in 0..self.rows {
            for inner in 0..self.cols {
                let left = self.get(row, inner);
                for col in 0..other.cols {
                    let index = row * other.cols + col;
                    result.values[index] += left * other.get(inner, col);
                }
            }
        }
        result
    }

    pub(crate) fn mul_vec(&self, vector: &[f64]) -> Vec<f64> {
        (0..self.rows)
            .map(|row| {
                (0..self.cols)
                    .map(|col| self.get(row, col) * vector[col])
                    .sum()
            })
            .collect()
    }

    pub(crate) fn selected_rows(&self, indices: &[usize]) -> Self {
        let mut result = Self::zeros(indices.len(), self.cols);
        for (target_row, &source_row) in indices.iter().enumerate() {
            for col in 0..self.cols {
                result.set(target_row, col, self.get(source_row, col));
            }
        }
        result
    }

    pub(crate) fn principal_submatrix(&self, indices: &[usize]) -> Self {
        let mut result = Self::zeros(indices.len(), indices.len());
        for (target_row, &source_row) in indices.iter().enumerate() {
            for (target_col, &source_col) in indices.iter().enumerate() {
                result.set(target_row, target_col, self.get(source_row, source_col));
            }
        }
        result
    }

    pub(crate) fn symmetrized(mut self) -> Self {
        for row in 0..self.rows {
            for col in 0..row {
                let value = 0.5 * (self.get(row, col) + self.get(col, row));
                self.set(row, col, value);
                self.set(col, row, value);
            }
        }
        self
    }

    pub(crate) fn into_rows(self) -> Vec<Vec<f64>> {
        self.values.chunks(self.cols).map(<[f64]>::to_vec).collect()
    }

    pub(crate) fn validate_symmetric_positive_definite(
        &self,
        name: &str,
    ) -> Result<(), LongitudinalError> {
        if self.rows != self.cols {
            return Err(LongitudinalError::Invalid(format!("{name} must be square")));
        }
        for row in 0..self.rows {
            for col in 0..row {
                let scale = self
                    .get(row, col)
                    .abs()
                    .max(self.get(col, row).abs())
                    .max(1.0);
                if (self.get(row, col) - self.get(col, row)).abs() > 1.0e-10 * scale {
                    return Err(LongitudinalError::Invalid(format!(
                        "{name} must be symmetric"
                    )));
                }
            }
        }
        self.cholesky(name).map(|_| ())
    }

    pub(crate) fn validate_symmetric_positive_semidefinite(
        &self,
        name: &str,
    ) -> Result<(), LongitudinalError> {
        if self.rows != self.cols {
            return Err(LongitudinalError::Invalid(format!("{name} must be square")));
        }
        let scale = (0..self.rows)
            .map(|index| self.get(index, index).abs())
            .fold(1.0_f64, f64::max);
        let tolerance = 1.0e-12 * scale;
        for row in 0..self.rows {
            for col in 0..row {
                if (self.get(row, col) - self.get(col, row)).abs() > 1.0e-10 * scale {
                    return Err(LongitudinalError::Invalid(format!(
                        "{name} must be symmetric"
                    )));
                }
            }
        }
        let mut lower = Self::identity(self.rows);
        let mut diagonal = vec![0.0; self.rows];
        for pivot in 0..self.rows {
            let cross = (0..pivot)
                .map(|inner| lower.get(pivot, inner).powi(2) * diagonal[inner])
                .sum::<f64>();
            let residual_diagonal = self.get(pivot, pivot) - cross;
            if residual_diagonal < -tolerance {
                return Err(LongitudinalError::Invalid(format!(
                    "{name} is not positive semidefinite"
                )));
            }
            diagonal[pivot] = if residual_diagonal.abs() <= tolerance {
                0.0
            } else {
                residual_diagonal
            };
            for row in (pivot + 1)..self.rows {
                let residual = self.get(row, pivot)
                    - (0..pivot)
                        .map(|inner| {
                            lower.get(row, inner) * lower.get(pivot, inner) * diagonal[inner]
                        })
                        .sum::<f64>();
                if diagonal[pivot] == 0.0 {
                    if residual.abs() > tolerance {
                        return Err(LongitudinalError::Invalid(format!(
                            "{name} is not positive semidefinite"
                        )));
                    }
                    lower.set(row, pivot, 0.0);
                } else {
                    lower.set(row, pivot, residual / diagonal[pivot]);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn cholesky(&self, name: &str) -> Result<Cholesky, LongitudinalError> {
        let mut lower = Self::zeros(self.rows, self.rows);
        for row in 0..self.rows {
            for col in 0..=row {
                let cross: f64 = (0..col)
                    .map(|inner| lower.get(row, inner) * lower.get(col, inner))
                    .sum();
                if row == col {
                    let diagonal = self.get(row, row) - cross;
                    if !diagonal.is_finite() || diagonal <= 0.0 {
                        return Err(LongitudinalError::Numerical(format!(
                            "{name} is not positive definite"
                        )));
                    }
                    lower.set(row, col, diagonal.sqrt());
                } else {
                    lower.set(row, col, (self.get(row, col) - cross) / lower.get(col, col));
                }
            }
        }
        Ok(Cholesky { lower })
    }
}

pub(crate) struct Cholesky {
    lower: Matrix,
}

impl Cholesky {
    pub(crate) fn solve_vector(&self, right: &[f64]) -> Vec<f64> {
        let size = self.lower.rows;
        let mut forward = vec![0.0; size];
        for row in 0..size {
            let cross: f64 = (0..row)
                .map(|col| self.lower.get(row, col) * forward[col])
                .sum();
            forward[row] = (right[row] - cross) / self.lower.get(row, row);
        }
        let mut result = vec![0.0; size];
        for row in (0..size).rev() {
            let cross: f64 = ((row + 1)..size)
                .map(|col| self.lower.get(col, row) * result[col])
                .sum();
            result[row] = (forward[row] - cross) / self.lower.get(row, row);
        }
        result
    }

    pub(crate) fn solve_matrix(&self, right: &Matrix) -> Matrix {
        let mut result = Matrix::zeros(right.rows, right.cols);
        for col in 0..right.cols {
            let column = (0..right.rows)
                .map(|row| right.get(row, col))
                .collect::<Vec<_>>();
            let solved = self.solve_vector(&column);
            for (row, value) in solved.into_iter().enumerate() {
                result.set(row, col, value);
            }
        }
        result
    }

    pub(crate) fn log_determinant(&self) -> f64 {
        2.0 * (0..self.lower.rows)
            .map(|index| self.lower.get(index, index).ln())
            .sum::<f64>()
    }
}
