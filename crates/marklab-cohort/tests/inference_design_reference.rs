use marklab_cohort::{
    InferenceAlternative, InferenceAnalysisLevel, InferenceDesign, InferenceDesignError,
    InferenceMultiplicity, InferenceNullFamily, InferencePermutationUnit,
};

#[test]
fn typed_design_preserves_exact_blocks_and_deterministic_whole_units() {
    let design = InferenceDesign::stratified_random_labeling(
        &[0_u32, 0, 1, 1],
        19,
        20260826,
        InferenceAlternative::Greater,
    )
    .expect("stratified design");

    assert_eq!(design.analysis_level(), InferenceAnalysisLevel::Cell);
    assert_eq!(
        design.null_family(),
        InferenceNullFamily::StratifiedRandomLabeling
    );
    assert_eq!(
        design.permutation_unit(),
        InferencePermutationUnit::CompleteScalarMark
    );
    assert_eq!(design.multiplicity(), InferenceMultiplicity::SingleEndpoint);
    assert_eq!(design.block_count(), 2);
    assert_eq!(design.unit_count(), 4);
    assert_eq!(design.permutations(), 19);
    assert_eq!(design.seed(), 20260826);
    assert_eq!(design.alternative(), InferenceAlternative::Greater);
    assert_eq!(design.permuted_indices(7), design.permuted_indices(7));

    for replicate in 0..19 {
        let permutation = design
            .permuted_indices(replicate)
            .expect("declared replicate");
        assert_eq!(permutation.len(), 4);
        assert!(permutation[..2].iter().all(|index| *index < 2));
        assert!(permutation[2..].iter().all(|index| *index >= 2));
    }
    assert_eq!(
        design.permuted_indices(19),
        Err(InferenceDesignError::ReplicateOutOfRange {
            replicate: 19,
            permutations: 19,
        })
    );
}

#[test]
fn patient_design_rejects_partial_or_nonexchangeable_blocks() {
    assert_eq!(
        InferenceDesign::patient_label_permutation(
            &[Some("north".into()), None],
            19,
            7,
            InferenceAlternative::TwoSided,
        ),
        Err(InferenceDesignError::PartialBlockDeclaration)
    );
    assert_eq!(
        InferenceDesign::stratified_random_labeling(
            &[0_u32, 1, 2],
            19,
            7,
            InferenceAlternative::TwoSided,
        ),
        Err(InferenceDesignError::NoExchangeableUnits)
    );
}
