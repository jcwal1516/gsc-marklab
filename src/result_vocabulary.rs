//! Typed finite vocabularies used by the existing spatial and marked result formats.

use serde::{Deserialize, Serialize};

macro_rules! result_enum {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[doc = concat!("Finite result vocabulary for `", stringify!($name), "`.")]
        #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
        pub enum $name {
            $(#[doc = concat!("Serialized as `", $wire, "`.")]
            #[serde(rename = $wire)]
            $variant),+
        }
        impl $name {
            /// Exact spelling in existing JSON and tabular result formats.
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $wire),+ }
            }
        }
    };
}

result_enum!(EdgeCorrection {
    StandardBorderReducedSample => "standard_border_reduced_sample",
    StandardBorderRadiusPlusBandwidth => "standard_border_radius_plus_bandwidth",
    Isotropic => "isotropic",
    Translation => "translation",
    StandardBorderInverseIntensityRatio => "standard_border_inverse_intensity_ratio",
    StandardBorderRadiusPlusPairBandwidthInverseIntensityRatio => "standard_border_radius_plus_pair_bandwidth_inverse_intensity_ratio",
    NoneFixedObservedLocations => "none_fixed_observed_locations",
    TranslationExactPolygonTypeSpecificInverseIntensity => "translation_exact_polygon_type_specific_inverse_intensity",
    IsotropicVisibleArcTypeSpecificInverseIntensity => "isotropic_visible_arc_type_specific_inverse_intensity",
    StandardBorderRadiusPlusPairBandwidthTypeSpecificInverseIntensityRatio => "standard_border_radius_plus_pair_bandwidth_type_specific_inverse_intensity_ratio",
});

result_enum!(MarkNullModel {
    RandomLabeling => "random_labeling",
});

result_enum!(InhomogeneousNullModel {
    RefittedGriddedInhomogeneousBinomialExperimental => "refitted_gridded_inhomogeneous_binomial_experimental",
    FixedGriddedInhomogeneousBinomial => "fixed_gridded_inhomogeneous_binomial",
    FixedBinaryCompartmentCountsUniformWithinExactPartition => "fixed_binary_compartment_counts_uniform_within_exact_partition",
    IndependentRefittedTypeSpecificInhomogeneousBinomialExperimental => "independent_refitted_type_specific_inhomogeneous_binomial_experimental",
    IndependentFixedGriddedTypeSpecificInhomogeneousBinomial => "independent_fixed_gridded_type_specific_inhomogeneous_binomial",
});

result_enum!(LikelihoodNullModel {
    NoneFixedParameterLikelihoodEvaluation => "none_fixed_parameter_likelihood_evaluation",
});

result_enum!(SpatialRandomizationUnit {
    WholeLocationPatternConditionedOnCount => "whole_location_pattern_conditioned_on_count",
    WholeLocationPatternConditionedOnBinaryCompartmentCounts => "whole_location_pattern_conditioned_on_binary_compartment_counts",
    IndependentSourceAndTargetLocationPatternsConditionedOnTypeCounts => "independent_source_and_target_location_patterns_conditioned_on_type_counts",
});

result_enum!(MarkPermutationUnit {
    CompleteCategoricalRow => "complete_categorical_row",
    CompleteContinuousMarkRow => "complete_continuous_mark_row",
    CompleteProbabilityRow => "complete_probability_row",
});

result_enum!(MultiplicityCorrection {
    SeparateComponentwiseErlFamilies => "separate_componentwise_erl_families",
    SingleWeightedKErlFamily => "single_weighted_k_erl_family",
    SingleConnectionErlFamily => "single_connection_erl_family",
    SingleCorrelationErlFamily => "single_correlation_erl_family",
    SingleStepMaxAbsOverAllLocations => "single_step_max_abs_over_all_locations",
});

result_enum!(MarkNormalization {
    GlobalArithmeticMeanProduct => "global_arithmetic_mean_product",
    ExpectedPositivePositivePairsPerEligibleDirectedPair => "expected_positive_positive_pairs_per_eligible_directed_pair",
    ExpectedSourceTargetProbabilityMassPerDirectedRadiusPair => "expected_source_target_probability_mass_per_directed_radius_pair",
});

result_enum!(ProbabilityContributionMode {
    ExactExpectedContributions => "exact_expected_contributions",
});

result_enum!(SpatialExecutionMode {
    Exact => "exact",
});

result_enum!(DuplicatePointPolicy {
    Reject => "reject",
});

result_enum!(BoundaryDistanceOwner {
    ObservationWindow2d => "observation_window_2d",
});

result_enum!(ArcMeasureOwner {
    ObservationWindow2dSegmentCirclePartition => "observation_window_2d_segment_circle_partition",
});

result_enum!(OverlapMeasureOwner {
    ObservationWindow2dExactBooleanIntersection => "observation_window_2d_exact_boolean_intersection",
});

result_enum!(PairTraversal {
    ExactStreamingOrderedPairs => "exact_streaming_ordered_pairs",
    ExactStreamingUnorderedPairsWithTwoDirectedArcWeights => "exact_streaming_unordered_pairs_with_two_directed_arc_weights",
    ExactStreamingUnorderedPairsWithBothOrderedWeights => "exact_streaming_unordered_pairs_with_both_ordered_weights",
    ExactStreamingUnorderedCompactSupportPairsWithTwoDirectedArcWeights => "exact_streaming_unordered_compact_support_pairs_with_two_directed_arc_weights",
    ExactStreamingUnorderedCompactSupportPairsWithBothOrderedWeights => "exact_streaming_unordered_compact_support_pairs_with_both_ordered_weights",
});

result_enum!(WindowCoordinateUnit {
    Micrometre => "micrometre",
});

result_enum!(WindowCoordinateFrame {
    ExistingPatternPhysicalXy => "existing_pattern_physical_xy",
});

result_enum!(WindowMembershipPolicy {
    ClosedWindow => "closed_window",
});

result_enum!(IntensityEstimator {
    GaussianKernel => "gaussian_kernel",
    PiecewiseConstantBinaryCompartment => "piecewise_constant_binary_compartment",
});

result_enum!(IntensityKernel {
    IsotropicGaussian2d => "isotropic_gaussian_2d",
});

result_enum!(IntensityBoundaryCorrection {
    DeterministicCellCenterQuadrature => "deterministic_cell_center_quadrature",
    ExactCompartmentArea => "exact_compartment_area",
});

result_enum!(BandwidthSelectionMethod {
    LeaveOneOutLogLikelihood => "leave_one_out_log_likelihood",
});

result_enum!(BandwidthSelectionObjective {
    MaximizeMeanLogLeaveOneOutEventIntensity => "maximize_mean_log_leave_one_out_event_intensity",
});

result_enum!(BandwidthTieBreak {
    SmallestBandwidthUm => "smallest_bandwidth_um",
});

result_enum!(ProbeSamplingMethod {
    FixedCellCentredRectangularGrid => "fixed_cell_centred_rectangular_grid",
});

result_enum!(NearestNeighborOwner {
    SpatialIndex2d => "spatial_index_2d",
});

result_enum!(VariogramInferenceStatus {
    ObservedOnlyNoNull => "observed_only_no_null",
});

#[derive(Deserialize, Serialize)]
#[serde(
    remote = "marklab_cohort::InferenceAlternative",
    rename_all = "snake_case"
)]
pub(crate) enum AlternativeWire {
    Less,
    Greater,
    TwoSided,
}

result_enum!(WeightFunction {
    ProductOverGlobalArithmeticMeanSquared => "product_over_global_arithmetic_mean_squared",
});

result_enum!(ProbabilityRowNullModel {
    CompleteProbabilityRowsWithoutReplacement => "complete_probability_rows_without_replacement",
});

result_enum!(VariogramMultiplicity {
    TwoSidedExtremeRankLengthGlobalEnvelope => "two_sided_extreme_rank_length_global_envelope",
});

result_enum!(CompartmentInterfaceEventPolicy {
    Reject => "reject",
});

result_enum!(MoranStatisticalUnit {
    CompleteMultivariateMarkRow => "complete_multivariate_mark_row",
});

result_enum!(MoranPopulationClaim {
    WithinSpecimenFieldDiagnosticOnly => "within_specimen_field_diagnostic_only",
});

result_enum!(MoranNullModel {
    CompleteRowsRandomlyLabeledWithinDeclaredStrata => "complete_rows_randomly_labeled_within_declared_strata",
});

result_enum!(IppStatisticalUnit {
    OneObservedPointPattern => "one_observed_point_pattern",
});

result_enum!(FiniteResultPolicy {
    RejectNonFinite => "reject_non_finite",
});

result_enum!(IppClaimStatus {
    ExperimentalFixedLikelihoodNoPopulationInference => "experimental_fixed_likelihood_no_population_inference",
});

/// Intensity training policy, preserving its existing string representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntensityCrossFit {
    /// Leave one event out and correct the training intensity by n/(n−1).
    LeaveOneOut,
    /// Leave one event out within its declared compartment.
    LeaveOneOutWithinCompartment,
    /// Balanced folds in stable Cell ID order.
    BalancedCellIdRank { folds: usize },
}

impl Serialize for IntensityCrossFit {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::LeaveOneOut => serializer.serialize_str("leave_one_out_n_over_n_minus_one"),
            Self::LeaveOneOutWithinCompartment => {
                serializer.serialize_str("leave_one_out_within_compartment")
            }
            Self::BalancedCellIdRank { folds } => {
                serializer.serialize_str(&format!("balanced_cell_id_rank_{folds}_fold"))
            }
        }
    }
}

impl<'de> Deserialize<'de> for IntensityCrossFit {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        match value.as_str() {
            "leave_one_out_n_over_n_minus_one" => Ok(Self::LeaveOneOut),
            "leave_one_out_within_compartment" => Ok(Self::LeaveOneOutWithinCompartment),
            _ => {
                let digits = value
                    .strip_prefix("balanced_cell_id_rank_")
                    .and_then(|value| value.strip_suffix("_fold"));
                let folds = digits.and_then(|value| value.parse::<usize>().ok());
                match (digits, folds) {
                    (Some(digits), Some(folds)) if folds >= 2 && digits == folds.to_string() => {
                        Ok(Self::BalancedCellIdRank { folds })
                    }
                    _ => Err(serde::de::Error::custom(
                        "unknown intensity cross-fitting policy",
                    )),
                }
            }
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(remote = "marklab_cohort::MaxTCorrection", rename_all = "snake_case")]
pub(crate) enum MaxTCorrectionWire {
    #[serde(rename = "single_step_max_t")]
    SingleStep,
    #[serde(rename = "step_down_max_t")]
    StepDown,
}
