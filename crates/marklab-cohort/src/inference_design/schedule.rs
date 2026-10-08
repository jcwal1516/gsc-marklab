use super::{declarations::valid_block_text, *};

impl InferenceDesign {
    pub(crate) fn declare_complete_endpoint_family_max_t(mut self) -> Self {
        self.multiplicity = InferenceMultiplicity::CompleteEndpointFamilyMaxT;
        self
    }

    pub(crate) fn declare_ordered_family_gatekeeping_max_t(mut self) -> Self {
        self.multiplicity = InferenceMultiplicity::OrderedFamilyGatekeepingMaxT;
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

    /// Declare whole multivariate-row random labeling within dense exact strata.
    ///
    /// The complete feature vector moves as one unit. The resulting design owns
    /// a single-step maximum-statistic family over the caller's prespecified
    /// local endpoints.
    pub fn stratified_multivariate_random_labeling_max_t(
        strata: &[u32],
        permutations: usize,
        seed: u64,
    ) -> Result<Self, InferenceDesignError> {
        let mut by_stratum = BTreeMap::<u32, Vec<usize>>::new();
        for (index, stratum) in strata.iter().copied().enumerate() {
            by_stratum.entry(stratum).or_default().push(index);
        }
        let mut design = Self::build(
            InferenceAnalysisLevel::Cell,
            InferenceNullFamily::StratifiedRandomLabeling,
            InferencePermutationUnit::CompleteMultivariateMark,
            by_stratum.into_values().collect(),
            strata.len(),
            permutations,
            seed,
            STRATIFIED_RANDOM_LABELING_NAMESPACE ^ 0x6d75_6c74_6976_6172,
            InferenceAlternative::TwoSided,
        )?;
        design.multiplicity = InferenceMultiplicity::CompleteEndpointFamilyMaxT;
        Ok(design)
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
