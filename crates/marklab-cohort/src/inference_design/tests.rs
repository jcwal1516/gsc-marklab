use super::*;
use crate::{derive_seed_in_namespace, shuffled_labels};

#[test]
fn shared_two_group_validation_preserves_exact_failures() {
    assert_eq!(
        validate_two_group_labels("", "B"),
        Err(CohortInferenceError::InvalidInput(
            "group labels must be non-empty".into()
        ))
    );
    assert_eq!(
        validate_two_group_labels(" A", "B"),
        Err(CohortInferenceError::InvalidInput(
            "group labels may not have surrounding whitespace".into()
        ))
    );
    assert_eq!(
        validate_two_group_labels("A", "A"),
        Err(CohortInferenceError::InvalidInput(
            "group labels must be distinct".into()
        ))
    );
    assert_eq!(
        validate_permutation_count(0),
        Err(CohortInferenceError::InvalidInput(format!(
            "permutations must be between 1 and {MAXIMUM_PERMUTATIONS}"
        )))
    );
    assert!(validate_two_group_labels("A", "B").is_ok());
    assert!(validate_permutation_count(19).is_ok());
}

#[test]
fn population_independence_preserves_whole_labels_and_exact_method_stream() {
    let labels = [true, true, false, false, true, false];
    let namespace = 0x7465_7374_5f70_6f70;
    let design = InferenceDesign::population_independence(labels.len(), 19, 20260828, namespace)
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
