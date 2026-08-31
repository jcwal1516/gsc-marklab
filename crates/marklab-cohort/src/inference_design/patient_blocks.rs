use super::*;

pub(crate) fn compile_blocked_population_independence(
    patient_ids: &[String],
    observed_labels: &[bool],
    assignments: &[PatientExchangeabilityBlock],
    permutations: usize,
    seed: u64,
    seed_namespace: u64,
    alternative: InferenceAlternative,
) -> Result<InferenceDesign, CohortInferenceError> {
    if patient_ids.len() != observed_labels.len() {
        return Err(CohortInferenceError::InvalidInput(
            "blocked fingerprint design requires exactly one assignment per patient".into(),
        ));
    }
    let blocks = align_patient_blocks(patient_ids, assignments, "fingerprint")?;
    let design = InferenceDesign::blocked_population_independence(
        &blocks,
        permutations,
        seed,
        seed_namespace,
        alternative,
    )
    .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?;
    if !design.blocks().iter().any(|indices| {
        indices.iter().any(|index| observed_labels[*index])
            && indices.iter().any(|index| !observed_labels[*index])
    }) {
        return Err(CohortInferenceError::InvalidInput(
            "fingerprint blocks are fully confounded with group".into(),
        ));
    }
    Ok(design)
}

pub(crate) fn align_patient_blocks(
    patient_ids: &[String],
    assignments: &[PatientExchangeabilityBlock],
    context: &str,
) -> Result<Vec<String>, CohortInferenceError> {
    if assignments.len() != patient_ids.len() {
        return Err(CohortInferenceError::InvalidInput(format!(
            "blocked {context} design requires exactly one assignment per patient"
        )));
    }
    let mut by_patient = BTreeMap::<&str, &str>::new();
    for assignment in assignments {
        if by_patient
            .insert(assignment.patient_id(), assignment.block())
            .is_some()
        {
            return Err(CohortInferenceError::InvalidInput(format!(
                "duplicate {context} block assignment for patient {}",
                assignment.patient_id()
            )));
        }
    }
    let mut blocks = Vec::new();
    blocks
        .try_reserve_exact(patient_ids.len())
        .map_err(|_| CohortInferenceError::InvalidInput("block allocation failed".into()))?;
    for patient_id in patient_ids {
        let block = by_patient.remove(patient_id.as_str()).ok_or_else(|| {
            CohortInferenceError::InvalidInput(format!(
                "missing {context} block assignment for patient {patient_id}"
            ))
        })?;
        blocks.push(block.to_owned());
    }
    if let Some(patient_id) = by_patient.keys().next() {
        return Err(CohortInferenceError::InvalidInput(format!(
            "{context} block assignment names unknown patient {patient_id}"
        )));
    }
    Ok(blocks)
}

/// Validated patient hierarchy plus one shared typed permutation schedule.
pub(crate) struct PatientPermutationDesign {
    design: InferenceDesign,
    blocked: bool,
}

impl PatientPermutationDesign {
    pub(crate) fn compile(
        records: &[ValidatedRecord],
        spec: &PatientPermutationSpec,
    ) -> Result<Self, CohortInferenceError> {
        let blocks = records
            .iter()
            .map(|record| record.block.clone())
            .collect::<Vec<_>>();
        let blocked = blocks
            .iter()
            .any(|block| block.as_deref().is_some_and(|value| !value.is_empty()));
        let design = InferenceDesign::patient_label_permutation(
            &blocks,
            spec.permutations,
            spec.seed,
            spec.alternative,
        )
        .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?;
        if blocked
            && !design.blocks().iter().any(|indices| {
                indices.iter().any(|index| records[*index].is_group_a)
                    && indices.iter().any(|index| !records[*index].is_group_a)
            })
        {
            return Err(CohortInferenceError::InvalidInput(
                "declared blocks are fully confounded with group".into(),
            ));
        }
        Ok(Self { design, blocked })
    }

    pub(crate) fn blocked(&self) -> bool {
        self.blocked
    }

    pub(crate) fn block_count(&self) -> usize {
        self.design.block_count()
    }

    #[cfg(test)]
    pub(crate) fn blocks(&self) -> &[Vec<usize>] {
        self.design.blocks()
    }

    pub(crate) fn permutations(&self) -> usize {
        self.design.permutations()
    }

    pub(crate) fn seed(&self) -> u64 {
        self.design.seed()
    }

    pub(crate) fn alternative(&self) -> PermutationAlternative {
        self.design.alternative()
    }

    pub(crate) fn permuted_labels(
        &self,
        records: &[ValidatedRecord],
        replicate: usize,
    ) -> Result<Vec<bool>, CohortInferenceError> {
        Ok(self
            .design
            .permuted_indices(replicate)
            .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?
            .iter()
            .map(|source| records[*source].is_group_a)
            .collect())
    }
}
