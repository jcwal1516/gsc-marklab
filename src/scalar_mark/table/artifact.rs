use super::{identity, *};

impl MarkTable {
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
                ScalarMarkColumnValues::Assay { declaration, .. } => {
                    declaration.measurement_status()
                }
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
    pub(in crate::scalar_mark) fn declarations(
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
        let binary_modality = binary_column
            .compatibility_semantics
            .map(|(modality, _)| modality);
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
                ) && column.compatibility_semantics.map(|(modality, _)| modality)
                    != binary_modality
                {
                    return Err(DeclaredScalarInputError::ThresholdModalityMismatch);
                }
                Some(declaration.clone())
            }
            None => None,
        };
        Ok((binary_declaration.clone(), probability))
    }

    pub(in crate::scalar_mark) fn semantic_artifact_ids(
        &self,
    ) -> Result<Box<[ArtifactId]>, DeclaredScalarInputError> {
        let mut ids = Vec::with_capacity(self.columns.len() + 1);
        let mut assay_ids = Vec::new();
        for column in &self.columns {
            match &column.values {
                ScalarMarkColumnValues::Assay { declaration, .. } => {
                    assay_ids.push(declaration.provenance_artifact_id())
                }
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
        // Assay channels may share one acquisition/processing provenance record. Legacy
        // specialized roles retain their stricter distinct-role contract above.
        for id in assay_ids {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        Ok(ids.into_boxed_slice())
    }

    pub(crate) fn declared_artifact_ref(
        &self,
        slide_id: &SlideId,
        frame_id: &CoordinateFrameId,
    ) -> Result<ArtifactRef, DeclaredScalarInputError> {
        identity::declared_artifact_ref(self, slide_id, frame_id)
    }
}
