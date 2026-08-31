use super::*;

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
    /// Prespecified ordered families opened serially, each controlled by local Max-T.
    OrderedFamilyGatekeepingMaxT,
}

/// Complete exact blocked-permutation schedule shared by current production methods.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InferenceDesign {
    pub(super) analysis_level: InferenceAnalysisLevel,
    pub(super) null_family: InferenceNullFamily,
    pub(super) permutation_unit: InferencePermutationUnit,
    pub(super) blocks: Vec<Vec<usize>>,
    pub(super) unit_count: usize,
    pub(super) permutations: usize,
    pub(super) seed: u64,
    pub(super) seed_namespace: u64,
    pub(super) alternative: InferenceAlternative,
    pub(super) multiplicity: InferenceMultiplicity,
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
pub(super) fn valid_block_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.trim() == value
        && !value.chars().any(char::is_control)
}
