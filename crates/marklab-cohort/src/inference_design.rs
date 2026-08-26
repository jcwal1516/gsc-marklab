use std::collections::BTreeMap;

use thiserror::Error;

use super::{
    splitmix64, CohortInferenceError, PatientPermutationSpec, PermutationAlternative,
    ValidatedRecord, PATIENT_PERMUTATION_NAMESPACE,
};

const RANDOM_LABELING_NAMESPACE: u64 = 0x7261_6e64_6c61_6265;
const STRATIFIED_RANDOM_LABELING_NAMESPACE: u64 = 0x7374_7261_746c_6162;

/// Shared name for the existing less/greater/two-sided permutation alternative.
pub type InferenceAlternative = PermutationAlternative;

/// Biological or observational level at which exchangeable units are declared.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InferenceAnalysisLevel {
    /// One complete per-cell mark value is the exchangeable unit.
    Cell,
    /// One whole patient label is the exchangeable unit.
    Patient,
}

/// Admitted null family for the two current design callers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InferenceNullFamily {
    /// Whole scalar values move freely across fixed cell locations.
    RandomLabeling,
    /// Whole scalar values move only within exact declared cell strata.
    StratifiedRandomLabeling,
    /// Whole patient group labels move within exact declared patient blocks.
    PatientLabelPermutation,
}

/// Atomic unit moved by one permutation schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InferencePermutationUnit {
    /// One complete scalar mark; coordinates and scalar components never split.
    CompleteScalarMark,
    /// One whole patient group label.
    PatientLabel,
}

/// Multiplicity family currently owned by the shared design.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InferenceMultiplicity {
    /// One prespecified endpoint with no multiplicity adjustment.
    SingleEndpoint,
}

/// Complete exact blocked-permutation schedule shared by two production methods.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InferenceDesign {
    analysis_level: InferenceAnalysisLevel,
    null_family: InferenceNullFamily,
    permutation_unit: InferencePermutationUnit,
    blocks: Vec<Vec<usize>>,
    unit_count: usize,
    permutations: usize,
    seed: u64,
    seed_namespace: u64,
    alternative: InferenceAlternative,
    multiplicity: InferenceMultiplicity,
}

impl InferenceDesign {
    /// Declare unstratified whole-mark random labeling across fixed cell locations.
    pub fn random_labeling(
        unit_count: usize,
        permutations: usize,
        seed: u64,
        alternative: InferenceAlternative,
    ) -> Result<Self, InferenceDesignError> {
        Self::build(
            InferenceAnalysisLevel::Cell,
            InferenceNullFamily::RandomLabeling,
            InferencePermutationUnit::CompleteScalarMark,
            vec![(0..unit_count).collect()],
            unit_count,
            permutations,
            seed,
            RANDOM_LABELING_NAMESPACE,
            alternative,
        )
    }

    /// Declare whole-mark random labeling within dense exact stratum codes.
    pub fn stratified_random_labeling(
        strata: &[u32],
        permutations: usize,
        seed: u64,
        alternative: InferenceAlternative,
    ) -> Result<Self, InferenceDesignError> {
        let mut by_stratum = BTreeMap::<u32, Vec<usize>>::new();
        for (index, stratum) in strata.iter().copied().enumerate() {
            by_stratum.entry(stratum).or_default().push(index);
        }
        Self::build(
            InferenceAnalysisLevel::Cell,
            InferenceNullFamily::StratifiedRandomLabeling,
            InferencePermutationUnit::CompleteScalarMark,
            by_stratum.into_values().collect(),
            strata.len(),
            permutations,
            seed,
            STRATIFIED_RANDOM_LABELING_NAMESPACE,
            alternative,
        )
    }

    /// Declare whole-patient label permutation with all blocks present or all absent.
    pub fn patient_label_permutation(
        blocks: &[Option<String>],
        permutations: usize,
        seed: u64,
        alternative: InferenceAlternative,
    ) -> Result<Self, InferenceDesignError> {
        let declared = blocks
            .iter()
            .filter(|block| block.as_deref().is_some_and(|value| !value.is_empty()))
            .count();
        if declared != 0 && declared != blocks.len() {
            return Err(InferenceDesignError::PartialBlockDeclaration);
        }
        let mut by_block = BTreeMap::<&str, Vec<usize>>::new();
        for (index, block) in blocks.iter().enumerate() {
            by_block
                .entry(block.as_deref().unwrap_or_default())
                .or_default()
                .push(index);
        }
        Self::build(
            InferenceAnalysisLevel::Patient,
            InferenceNullFamily::PatientLabelPermutation,
            InferencePermutationUnit::PatientLabel,
            by_block.into_values().collect(),
            blocks.len(),
            permutations,
            seed,
            PATIENT_PERMUTATION_NAMESPACE,
            alternative,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        analysis_level: InferenceAnalysisLevel,
        null_family: InferenceNullFamily,
        permutation_unit: InferencePermutationUnit,
        blocks: Vec<Vec<usize>>,
        unit_count: usize,
        permutations: usize,
        seed: u64,
        seed_namespace: u64,
        alternative: InferenceAlternative,
    ) -> Result<Self, InferenceDesignError> {
        if unit_count == 0 {
            return Err(InferenceDesignError::NoUnits);
        }
        if permutations == 0 {
            return Err(InferenceDesignError::NoPermutations);
        }
        if !blocks.iter().any(|block| block.len() > 1) {
            return Err(InferenceDesignError::NoExchangeableUnits);
        }
        let mut observed = blocks.iter().flatten().copied().collect::<Vec<_>>();
        observed.sort_unstable();
        if observed != (0..unit_count).collect::<Vec<_>>() {
            return Err(InferenceDesignError::InvalidBlockMembership);
        }
        Ok(Self {
            analysis_level,
            null_family,
            permutation_unit,
            blocks,
            unit_count,
            permutations,
            seed,
            seed_namespace,
            alternative,
            multiplicity: InferenceMultiplicity::SingleEndpoint,
        })
    }

    /// Analysis level whose units may move.
    pub fn analysis_level(&self) -> InferenceAnalysisLevel {
        self.analysis_level
    }

    /// Exact admitted null family.
    pub fn null_family(&self) -> InferenceNullFamily {
        self.null_family
    }

    /// Atomic unit preserved by every permutation.
    pub fn permutation_unit(&self) -> InferencePermutationUnit {
        self.permutation_unit
    }

    /// Number of complete exact exchangeability blocks.
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Number of exchangeable units.
    pub fn unit_count(&self) -> usize {
        self.unit_count
    }

    /// Requested permutation count.
    pub fn permutations(&self) -> usize {
        self.permutations
    }

    /// Base deterministic seed.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Method-owned namespace mixed into every replicate seed.
    pub fn seed_namespace(&self) -> u64 {
        self.seed_namespace
    }

    /// Prespecified alternative.
    pub fn alternative(&self) -> InferenceAlternative {
        self.alternative
    }

    /// Explicit multiplicity family.
    pub fn multiplicity(&self) -> InferenceMultiplicity {
        self.multiplicity
    }

    /// Deterministic mapping from each output position to one whole source unit.
    pub fn permuted_indices(&self, replicate: usize) -> Result<Box<[usize]>, InferenceDesignError> {
        if replicate >= self.permutations {
            return Err(InferenceDesignError::ReplicateOutOfRange {
                replicate,
                permutations: self.permutations,
            });
        }
        let mut indices = (0..self.unit_count).collect::<Vec<_>>();
        let mut state = splitmix64(splitmix64(self.seed ^ self.seed_namespace) ^ replicate as u64);
        for block in &self.blocks {
            let mut shuffled = block.clone();
            for index in (1..shuffled.len()).rev() {
                state = splitmix64(state ^ index as u64);
                let other = (state % (index as u64 + 1)) as usize;
                shuffled.swap(index, other);
            }
            for (position, source) in block.iter().zip(shuffled) {
                indices[*position] = source;
            }
        }
        Ok(indices.into_boxed_slice())
    }

    pub(super) fn blocks(&self) -> &[Vec<usize>] {
        &self.blocks
    }
}

/// Failure to compile one explicit blocked-permutation schedule.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum InferenceDesignError {
    /// No units were supplied.
    #[error("inference design requires at least one unit")]
    NoUnits,
    /// No randomization replicates were requested.
    #[error("inference design requires at least one permutation")]
    NoPermutations,
    /// Patient blocks were declared for only a subset of patients.
    #[error("exchangeability blocks must be declared for every patient or no patients")]
    PartialBlockDeclaration,
    /// No block contains two units that can be exchanged.
    #[error("inference design has no exchangeable units")]
    NoExchangeableUnits,
    /// Block membership omits, duplicates, or invents a unit row.
    #[error("inference design block membership is not a complete exact partition")]
    InvalidBlockMembership,
    /// Requested replicate is outside the declared schedule.
    #[error("inference replicate {replicate} is outside 0..{permutations}")]
    ReplicateOutOfRange {
        /// Requested zero-based replicate.
        replicate: usize,
        /// Declared replicate count.
        permutations: usize,
    },
}

/// Validated patient hierarchy plus one shared typed permutation schedule.
pub(super) struct PatientPermutationDesign {
    design: InferenceDesign,
    blocked: bool,
}

impl PatientPermutationDesign {
    pub(super) fn compile(
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

    pub(super) fn blocked(&self) -> bool {
        self.blocked
    }

    pub(super) fn block_count(&self) -> usize {
        self.design.block_count()
    }

    #[cfg(test)]
    pub(super) fn blocks(&self) -> &[Vec<usize>] {
        self.design.blocks()
    }

    pub(super) fn permutations(&self) -> usize {
        self.design.permutations()
    }

    pub(super) fn seed(&self) -> u64 {
        self.design.seed()
    }

    pub(super) fn alternative(&self) -> PermutationAlternative {
        self.design.alternative()
    }

    pub(super) fn permuted_labels(
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
