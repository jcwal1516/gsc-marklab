//! The [`ValidationStatus`] and [`ValidationSuiteStatus`] vocabularies.

crate::vocabulary_enum! {
    /// Outcome of one entry in a validation ledger.
    pub enum ValidationStatus {
        Failed => "failed",
        NotApplicable => "not_applicable",
        NotSupported => "not_supported",
        NotVerified => "not_verified",
        NotVerifiedMissingAdmittedData => "not_verified_missing_admitted_data",
        NotVerifiedMissingProspectiveEvidence => "not_verified_missing_prospective_evidence",
        NotVerifiedUnexecutedControl => "not_verified_unexecuted_control",
        Passed => "passed",
    }
}

crate::vocabulary_enum! {
    /// Overall outcome of a validation suite, including the evidence it still lacks.
    pub enum ValidationSuiteStatus {
        FailedSyntheticControl => "failed_synthetic_control",
        PartialSyntheticAndProspectiveEvidenceRequired => "partial_synthetic_and_prospective_evidence_required",
        PartialSyntheticAndRealEvidenceRequired => "partial_synthetic_and_real_evidence_required",
        PartialWithDeclaredUnsupportedDimensions => "partial_with_declared_unsupported_dimensions",
    }
}
