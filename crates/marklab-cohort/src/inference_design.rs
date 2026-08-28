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

/// One exact patient-to-exchangeability-block assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatientExchangeabilityBlock {
    patient_id: String,
    block: String,
}

impl PatientExchangeabilityBlock {
    /// Validate bounded nonempty patient and block labels.
    pub fn new(
        patient_id: impl Into<String>,
        block: impl Into<String>,
    ) -> Result<Self, InferenceDesignError> {
        let patient_id = patient_id.into();
        let block = block.into();
        if !valid_block_text(&patient_id) {
            return Err(InferenceDesignError::InvalidPatientIdentity);
        }
        if !valid_block_text(&block) {
            return Err(InferenceDesignError::InvalidBlockLabel);
        }
        Ok(Self { patient_id, block })
    }

    /// Exact patient identity.
    pub fn patient_id(&self) -> &str {
        &self.patient_id
    }

    /// Exact exchangeability-block label.
    pub fn block(&self) -> &str {
        &self.block
    }
}

/// Biological or observational level at which exchangeable units are declared.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InferenceAnalysisLevel {
    /// One complete per-cell mark value is the exchangeable unit.
    Cell,
    /// One whole patient label is the exchangeable unit.
    Patient,
    /// One declared cluster containing one or more patients.
    Cluster,
}

/// Admitted null families used by current patient and cell-mark callers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InferenceNullFamily {
    /// Whole scalar values move freely across fixed cell locations.
    RandomLabeling,
    /// Whole scalar values move only within exact declared cell strata.
    StratifiedRandomLabeling,
    /// Whole patient group labels move within exact declared patient blocks.
    PatientLabelPermutation,
    /// Whole patient labels move under population independence.
    PopulationIndependence,
    /// Complete within-patient differences receive independent signs.
    PairedSignFlip,
    /// Patients are sampled first, then specimens within each sampled patient occurrence.
    HierarchicalBootstrap,
    /// Whole cluster group labels move under cluster-level independence.
    ClusterLabelPermutation,
    /// Reduced-model residuals move as whole cluster values conditional on fixed covariates.
    ClusterCovariateResidualPermutation,
    /// Complete reduced-model residual vectors receive independent subject-level signs.
    SubjectResidualSignSymmetry,
    /// Reduced-model residuals move as whole patient values conditional on fixed covariates.
    CovariateConditionalResidualPermutation,
}

/// Atomic unit moved by one permutation schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InferencePermutationUnit {
    /// One complete scalar mark; coordinates and scalar components never split.
    CompleteScalarMark,
    /// One whole patient group label.
    PatientLabel,
    /// One complete condition-B-minus-condition-A patient difference.
    CompletePatientPairDifference,
    /// One complete condition-B-minus-condition-A endpoint vector for one patient pair.
    CompletePatientPairDifferenceVector,
    /// One patient occurrence followed by its complete nested-specimen draw.
    PatientThenNestedSpecimen,
    /// One complete cluster endpoint summary.
    CompleteClusterEndpoint,
    /// One complete scalar reduced-model residual belonging to one cluster.
    CompleteClusterResidual,
    /// One complete subject residual vector across every retained visit.
    CompleteSubjectResidualVector,
    /// One complete scalar reduced-model residual belonging to one patient.
    CompletePatientResidual,
}

/// Multiplicity family currently owned by the shared design.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InferenceMultiplicity {
    /// One prespecified endpoint with no multiplicity adjustment.
    SingleEndpoint,
    /// One prespecified complete endpoint family controlled by a maximum statistic.
    CompleteEndpointFamilyMaxT,
}

/// Complete exact blocked-permutation schedule shared by current production methods.
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
    pub(crate) fn declare_complete_endpoint_family_max_t(mut self) -> Self {
        self.multiplicity = InferenceMultiplicity::CompleteEndpointFamilyMaxT;
        self
    }

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

    /// Declare one method-owned unblocked whole-patient population-independence schedule.
    pub(crate) fn population_independence(
        unit_count: usize,
        permutations: usize,
        seed: u64,
        seed_namespace: u64,
    ) -> Result<Self, InferenceDesignError> {
        Self::population_independence_with_alternative(
            unit_count,
            permutations,
            seed,
            seed_namespace,
            InferenceAlternative::Greater,
        )
    }

    pub(crate) fn population_independence_with_alternative(
        unit_count: usize,
        permutations: usize,
        seed: u64,
        seed_namespace: u64,
        alternative: InferenceAlternative,
    ) -> Result<Self, InferenceDesignError> {
        Self::build(
            InferenceAnalysisLevel::Patient,
            InferenceNullFamily::PopulationIndependence,
            InferencePermutationUnit::PatientLabel,
            vec![(0..unit_count).collect()],
            unit_count,
            permutations,
            seed,
            seed_namespace,
            alternative,
        )
    }

    pub(crate) fn blocked_population_independence(
        blocks: &[String],
        permutations: usize,
        seed: u64,
        seed_namespace: u64,
        alternative: InferenceAlternative,
    ) -> Result<Self, InferenceDesignError> {
        let mut by_block = BTreeMap::<&str, Vec<usize>>::new();
        for (index, block) in blocks.iter().enumerate() {
            if !valid_block_text(block) {
                return Err(InferenceDesignError::InvalidBlockLabel);
            }
            by_block.entry(block).or_default().push(index);
        }
        Self::build(
            InferenceAnalysisLevel::Patient,
            InferenceNullFamily::PopulationIndependence,
            InferencePermutationUnit::PatientLabel,
            by_block.into_values().collect(),
            blocks.len(),
            permutations,
            seed,
            seed_namespace,
            alternative,
        )
    }

    /// Declare independent sign symmetry for complete subject residual vectors.
    pub(crate) fn subject_residual_sign_symmetry(
        subject_count: usize,
        permutations: usize,
        seed: u64,
        seed_namespace: u64,
    ) -> Result<Self, InferenceDesignError> {
        Self::build(
            InferenceAnalysisLevel::Patient,
            InferenceNullFamily::SubjectResidualSignSymmetry,
            InferencePermutationUnit::CompleteSubjectResidualVector,
            vec![(0..subject_count).collect()],
            subject_count,
            permutations,
            seed,
            seed_namespace,
            InferenceAlternative::TwoSided,
        )
    }

    pub(crate) fn covariate_conditional_residual_permutation(
        patient_count: usize,
        permutations: usize,
        seed: u64,
        seed_namespace: u64,
        alternative: InferenceAlternative,
    ) -> Result<Self, InferenceDesignError> {
        Self::build(
            InferenceAnalysisLevel::Patient,
            InferenceNullFamily::CovariateConditionalResidualPermutation,
            InferencePermutationUnit::CompletePatientResidual,
            vec![(0..patient_count).collect()],
            patient_count,
            permutations,
            seed,
            seed_namespace,
            alternative,
        )
    }

    pub(crate) fn blocked_covariate_conditional_residual_permutation(
        blocks: &[String],
        permutations: usize,
        seed: u64,
        seed_namespace: u64,
        alternative: InferenceAlternative,
    ) -> Result<Self, InferenceDesignError> {
        let mut by_block = BTreeMap::<&str, Vec<usize>>::new();
        for (index, block) in blocks.iter().enumerate() {
            if !valid_block_text(block) {
                return Err(InferenceDesignError::InvalidBlockLabel);
            }
            by_block.entry(block).or_default().push(index);
        }
        Self::build(
            InferenceAnalysisLevel::Patient,
            InferenceNullFamily::CovariateConditionalResidualPermutation,
            InferencePermutationUnit::CompletePatientResidual,
            by_block.into_values().collect(),
            blocks.len(),
            permutations,
            seed,
            seed_namespace,
            alternative,
        )
    }

    pub(crate) fn paired_sign_flip(
        pair_count: usize,
        permutations: usize,
        seed: u64,
        seed_namespace: u64,
        alternative: InferenceAlternative,
    ) -> Result<Self, InferenceDesignError> {
        Self::build(
            InferenceAnalysisLevel::Patient,
            InferenceNullFamily::PairedSignFlip,
            InferencePermutationUnit::CompletePatientPairDifference,
            vec![(0..pair_count).collect()],
            pair_count,
            permutations,
            seed,
            seed_namespace,
            alternative,
        )
    }

    pub(crate) fn paired_vector_sign_flip(
        pair_count: usize,
        permutations: usize,
        seed: u64,
        seed_namespace: u64,
    ) -> Result<Self, InferenceDesignError> {
        Self::build(
            InferenceAnalysisLevel::Patient,
            InferenceNullFamily::PairedSignFlip,
            InferencePermutationUnit::CompletePatientPairDifferenceVector,
            vec![(0..pair_count).collect()],
            pair_count,
            permutations,
            seed,
            seed_namespace,
            InferenceAlternative::TwoSided,
        )
    }

    pub(crate) fn hierarchical_bootstrap(
        specimen_counts: &[usize],
        replicates: usize,
        seed: u64,
        seed_namespace: u64,
    ) -> Result<Self, InferenceDesignError> {
        let mut blocks = Vec::with_capacity(specimen_counts.len());
        let mut offset = 0usize;
        for count in specimen_counts {
            if *count == 0 {
                return Err(InferenceDesignError::EmptyHierarchicalBlock);
            }
            let end = offset
                .checked_add(*count)
                .ok_or(InferenceDesignError::InvalidBlockMembership)?;
            blocks.push((offset..end).collect());
            offset = end;
        }
        Self::build(
            InferenceAnalysisLevel::Patient,
            InferenceNullFamily::HierarchicalBootstrap,
            InferencePermutationUnit::PatientThenNestedSpecimen,
            blocks,
            offset,
            replicates,
            seed,
            seed_namespace,
            InferenceAlternative::TwoSided,
        )
    }

    pub(crate) fn cluster_label_permutation(
        cluster_count: usize,
        permutations: usize,
        seed: u64,
        seed_namespace: u64,
        alternative: InferenceAlternative,
    ) -> Result<Self, InferenceDesignError> {
        Self::build(
            InferenceAnalysisLevel::Cluster,
            InferenceNullFamily::ClusterLabelPermutation,
            InferencePermutationUnit::CompleteClusterEndpoint,
            vec![(0..cluster_count).collect()],
            cluster_count,
            permutations,
            seed,
            seed_namespace,
            alternative,
        )
    }

    pub(crate) fn cluster_covariate_residual_permutation(
        cluster_count: usize,
        permutations: usize,
        seed: u64,
        seed_namespace: u64,
        alternative: InferenceAlternative,
    ) -> Result<Self, InferenceDesignError> {
        Self::build(
            InferenceAnalysisLevel::Cluster,
            InferenceNullFamily::ClusterCovariateResidualPermutation,
            InferencePermutationUnit::CompleteClusterResidual,
            vec![(0..cluster_count).collect()],
            cluster_count,
            permutations,
            seed,
            seed_namespace,
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
        if (null_family == InferenceNullFamily::HierarchicalBootstrap && blocks.len() < 2)
            || (null_family != InferenceNullFamily::HierarchicalBootstrap
                && !blocks.iter().any(|block| block.len() > 1))
        {
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
        if matches!(
            self.permutation_unit,
            InferencePermutationUnit::CompletePatientPairDifference
                | InferencePermutationUnit::CompletePatientPairDifferenceVector
                | InferencePermutationUnit::CompleteSubjectResidualVector
                | InferencePermutationUnit::PatientThenNestedSpecimen
        ) {
            return Err(InferenceDesignError::UnsupportedIndexOperation);
        }
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

    /// Deterministic ±1 sign for every complete subject residual vector.
    pub(crate) fn subject_residual_signs(
        &self,
        replicate: usize,
    ) -> Result<Box<[i8]>, InferenceDesignError> {
        if self.null_family != InferenceNullFamily::SubjectResidualSignSymmetry
            || self.permutation_unit != InferencePermutationUnit::CompleteSubjectResidualVector
        {
            return Err(InferenceDesignError::UnsupportedSignOperation);
        }
        self.independent_signs(replicate)
    }

    pub(crate) fn paired_difference_signs(
        &self,
        replicate: usize,
    ) -> Result<Box<[i8]>, InferenceDesignError> {
        if self.null_family != InferenceNullFamily::PairedSignFlip
            || self.permutation_unit != InferencePermutationUnit::CompletePatientPairDifference
        {
            return Err(InferenceDesignError::UnsupportedPairedSignOperation);
        }
        self.independent_signs(replicate)
    }

    pub(crate) fn paired_difference_vector_signs(
        &self,
        replicate: usize,
    ) -> Result<Box<[i8]>, InferenceDesignError> {
        if self.null_family != InferenceNullFamily::PairedSignFlip
            || self.permutation_unit
                != InferencePermutationUnit::CompletePatientPairDifferenceVector
        {
            return Err(InferenceDesignError::UnsupportedPairedSignOperation);
        }
        self.independent_signs(replicate)
    }

    fn independent_signs(&self, replicate: usize) -> Result<Box<[i8]>, InferenceDesignError> {
        if replicate >= self.permutations {
            return Err(InferenceDesignError::ReplicateOutOfRange {
                replicate,
                permutations: self.permutations,
            });
        }
        let mut state = splitmix64(splitmix64(self.seed ^ self.seed_namespace) ^ replicate as u64);
        Ok((0..self.unit_count)
            .map(|subject| {
                state = splitmix64(state ^ subject as u64);
                if state & 1 == 0 {
                    1
                } else {
                    -1
                }
            })
            .collect::<Vec<_>>()
            .into_boxed_slice())
    }

    pub(crate) fn hierarchical_resample_indices(
        &self,
        replicate: usize,
    ) -> Result<Box<[usize]>, InferenceDesignError> {
        if self.null_family != InferenceNullFamily::HierarchicalBootstrap
            || self.permutation_unit != InferencePermutationUnit::PatientThenNestedSpecimen
        {
            return Err(InferenceDesignError::UnsupportedHierarchicalOperation);
        }
        if replicate >= self.permutations {
            return Err(InferenceDesignError::ReplicateOutOfRange {
                replicate,
                permutations: self.permutations,
            });
        }
        let maximum = self
            .blocks
            .len()
            .checked_mul(self.blocks.iter().map(Vec::len).max().unwrap_or(0))
            .ok_or(InferenceDesignError::InvalidBlockMembership)?;
        let mut sampled = Vec::with_capacity(maximum);
        let mut state = splitmix64(splitmix64(self.seed ^ self.seed_namespace) ^ replicate as u64);
        let mut draw_index = 0usize;
        for _ in 0..self.blocks.len() {
            state = splitmix64(state ^ draw_index as u64);
            draw_index += 1;
            let specimens = &self.blocks[state as usize % self.blocks.len()];
            for _ in 0..specimens.len() {
                state = splitmix64(state ^ draw_index as u64);
                draw_index += 1;
                sampled.push(specimens[state as usize % specimens.len()]);
            }
        }
        Ok(sampled.into_boxed_slice())
    }

    pub(super) fn blocks(&self) -> &[Vec<usize>] {
        &self.blocks
    }
}

/// Failure to compile one explicit blocked-permutation schedule.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum InferenceDesignError {
    /// Patient identity is empty, untrimmed, contains controls, or exceeds the bound.
    #[error("patient exchangeability identity must be 1-256 trimmed non-control bytes")]
    InvalidPatientIdentity,
    /// Block label is empty, untrimmed, contains controls, or exceeds the bound.
    #[error("exchangeability block must be 1-256 trimmed non-control bytes")]
    InvalidBlockLabel,
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
    /// A declared patient contains no nested specimens.
    #[error("hierarchical bootstrap patient blocks must contain at least one specimen")]
    EmptyHierarchicalBlock,
    /// Index permutation was requested from a sign-symmetry design.
    #[error("index permutation is unavailable for a subject-residual sign-symmetry design")]
    UnsupportedIndexOperation,
    /// Paired-difference signs were requested from another design.
    #[error("paired-difference signs require a paired sign-flip design")]
    UnsupportedPairedSignOperation,
    /// Hierarchical draws were requested from another design.
    #[error("hierarchical draws require a patient-then-nested-specimen bootstrap design")]
    UnsupportedHierarchicalOperation,
    /// Subject residual signs were requested from a design with another randomization unit.
    #[error("subject residual signs require a subject-residual sign-symmetry design")]
    UnsupportedSignOperation,
    /// Requested replicate is outside the declared schedule.
    #[error("inference replicate {replicate} is outside 0..{permutations}")]
    ReplicateOutOfRange {
        /// Requested zero-based replicate.
        replicate: usize,
        /// Declared replicate count.
        permutations: usize,
    },
}

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

fn valid_block_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.trim() == value
        && !value.chars().any(char::is_control)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{derive_seed_in_namespace, shuffled_labels};

    #[test]
    fn population_independence_preserves_whole_labels_and_exact_method_stream() {
        let labels = [true, true, false, false, true, false];
        let namespace = 0x7465_7374_5f70_6f70;
        let design =
            InferenceDesign::population_independence(labels.len(), 19, 20260828, namespace)
                .expect("population-independence design");
        assert_eq!(design.analysis_level(), InferenceAnalysisLevel::Patient);
        assert_eq!(
            design.null_family(),
            InferenceNullFamily::PopulationIndependence
        );
        assert_eq!(
            design.permutation_unit(),
            InferencePermutationUnit::PatientLabel
        );
        for replicate in 0..design.permutations() {
            let observed = design
                .permuted_indices(replicate)
                .expect("declared replicate")
                .iter()
                .map(|source| labels[*source])
                .collect::<Vec<_>>();
            let expected = shuffled_labels(
                &labels,
                derive_seed_in_namespace(20260828, namespace, replicate),
            );
            assert_eq!(observed, expected);
            assert_eq!(
                observed.iter().filter(|label| **label).count(),
                labels.iter().filter(|label| **label).count()
            );
        }
    }

    #[test]
    fn subject_residual_signs_match_the_former_whole_subject_stream() {
        let namespace = 0x7265_7065_6174_666c;
        let design = InferenceDesign::subject_residual_sign_symmetry(4, 19, 20260825, namespace)
            .expect("subject residual design");
        assert_eq!(design.analysis_level(), InferenceAnalysisLevel::Patient);
        assert_eq!(
            design.null_family(),
            InferenceNullFamily::SubjectResidualSignSymmetry
        );
        assert_eq!(
            design.permutation_unit(),
            InferencePermutationUnit::CompleteSubjectResidualVector
        );
        assert_eq!(design.alternative(), InferenceAlternative::TwoSided);

        for replicate in 0..design.permutations() {
            let mut state = splitmix64(splitmix64(20260825 ^ namespace) ^ replicate as u64);
            let expected = (0..4)
                .map(|subject| {
                    state = splitmix64(state ^ subject as u64);
                    if state & 1 == 0 {
                        1
                    } else {
                        -1
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(
                design
                    .subject_residual_signs(replicate)
                    .expect("declared replicate")
                    .as_ref(),
                expected
            );
        }
        assert_eq!(
            design.subject_residual_signs(19),
            Err(InferenceDesignError::ReplicateOutOfRange {
                replicate: 19,
                permutations: 19,
            })
        );
        assert_eq!(
            design.permuted_indices(0),
            Err(InferenceDesignError::UnsupportedIndexOperation)
        );
        let labels = InferenceDesign::random_labeling(4, 19, 1, InferenceAlternative::TwoSided)
            .expect("random-labeling design");
        assert_eq!(
            labels.subject_residual_signs(0),
            Err(InferenceDesignError::UnsupportedSignOperation)
        );
    }

    #[test]
    fn hierarchical_bootstrap_preserves_patient_then_specimen_units() {
        let design =
            InferenceDesign::hierarchical_bootstrap(&[2, 3], 19, 20260828, 0x6869_6572_5f62_6f6f)
                .expect("hierarchical design");
        assert_eq!(
            design.null_family(),
            InferenceNullFamily::HierarchicalBootstrap
        );
        assert_eq!(
            design.permutation_unit(),
            InferencePermutationUnit::PatientThenNestedSpecimen
        );
        assert_eq!(design.block_count(), 2);
        assert_eq!(design.unit_count(), 5);
        for replicate in 0..design.permutations() {
            let draw = design
                .hierarchical_resample_indices(replicate)
                .expect("declared replicate");
            assert!((4..=6).contains(&draw.len()));
            assert!(draw.iter().all(|index| *index < 5));
        }
        assert_eq!(
            design.permuted_indices(0),
            Err(InferenceDesignError::UnsupportedIndexOperation)
        );
        assert_eq!(
            InferenceDesign::hierarchical_bootstrap(&[1, 0], 9, 1, 2),
            Err(InferenceDesignError::EmptyHierarchicalBlock)
        );
    }
}
