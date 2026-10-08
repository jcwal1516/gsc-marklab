use super::*;

impl MarkTable {
    fn validate_legacy_column_counts(&self) -> Result<(), DeclaredScalarInputError> {
        let binary_count = self
            .columns
            .iter()
            .filter(|column| matches!(&column.values, ScalarMarkColumnValues::Binary { .. }))
            .count();
        if binary_count != 1 {
            return Err(DeclaredScalarInputError::BinaryColumnCountMismatch);
        }
        let probability_count = self
            .columns
            .iter()
            .filter(|column| matches!(&column.values, ScalarMarkColumnValues::Probability { .. }))
            .count();
        if probability_count > 1 {
            return Err(DeclaredScalarInputError::ProbabilityColumnCountMismatch);
        }
        let continuous_count = self
            .columns
            .iter()
            .filter(|column| matches!(&column.values, ScalarMarkColumnValues::Continuous { .. }))
            .count();
        if continuous_count > 1 {
            return Err(DeclaredScalarInputError::ContinuousColumnCountMismatch);
        }
        let categorical_count = self
            .columns
            .iter()
            .filter(|column| matches!(&column.values, ScalarMarkColumnValues::Categorical { .. }))
            .count();
        if categorical_count > 1 {
            return Err(DeclaredScalarInputError::CategoricalColumnCountMismatch);
        }
        let simplex_count = self
            .columns
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
        let ordinal_count = self
            .columns
            .iter()
            .filter(|column| matches!(&column.values, ScalarMarkColumnValues::Ordinal { .. }))
            .count();
        if ordinal_count > 1 {
            return Err(DeclaredScalarInputError::OrdinalColumnCountMismatch);
        }
        let vector_ref_count = self
            .columns
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
        Ok(())
    }

    pub(in crate::scalar_mark) fn validate_for_pattern(
        &self,
        pattern: &Pattern,
    ) -> Result<(), DeclaredScalarInputError> {
        self.validate_legacy_column_counts()?;
        if self
            .columns
            .iter()
            .any(|column| matches!(&column.values, ScalarMarkColumnValues::Assay { .. }))
        {
            return Err(DeclaredScalarInputError::analysis(
                crate::MarklabError::Validation(
                    "legacy Pattern projection requires explicitly selected compatibility columns"
                        .into(),
                ),
            ));
        }
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

    pub(in crate::scalar_mark) fn validate_provenance(
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
                ScalarMarkColumnValues::Assay { .. } => {
                    return Err(DeclaredScalarInputError::analysis(
                        crate::MarklabError::Validation(
                            "assay provenance requires the panel application boundary".into(),
                        ),
                    ))
                }
                ScalarMarkColumnValues::Binary { .. }
                | ScalarMarkColumnValues::Probability { .. } => {}
            }
        }
        Ok(())
    }
}
