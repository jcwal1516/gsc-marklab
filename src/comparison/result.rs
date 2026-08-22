use crate::output::{CurveComparisonAvailability, CurveComparisonMethod, CurveComparisonResult};

const STANDARDIZED_DIFFERENCE_METRIC: &str = "max_abs_standardized_difference";

/// Typed producer state for a curve comparison before output projection.
///
/// The variants prevent producers from combining a pooled-bin p-value with a
/// descriptive margin or from attaching a numeric statistic to unavailable
/// data. `into_output` is the single result-DTO construction path.
pub(crate) enum CurveComparisonAnalysis {
    #[cfg(any(feature = "cli", test))]
    PooledBin {
        comparison_name: String,
        statistic: f64,
        p_value: f64,
        interpretation: String,
    },
    DescriptiveMargin {
        comparison_name: String,
        statistic: f64,
        margin: Option<f64>,
        within_margin: Option<bool>,
        interpretation: String,
    },
    InsufficientData {
        comparison_name: String,
        method: CurveComparisonMethod,
        metric: String,
        margin: Option<f64>,
        reason: String,
    },
}

impl CurveComparisonAnalysis {
    #[cfg(any(feature = "cli", test))]
    pub(crate) fn pooled_bin(
        comparison_name: &str,
        statistic: f64,
        p_value: f64,
        interpretation: String,
    ) -> Self {
        Self::PooledBin {
            comparison_name: comparison_name.to_owned(),
            statistic,
            p_value,
            interpretation,
        }
    }

    pub(crate) fn descriptive_margin(
        comparison_name: &str,
        statistic: f64,
        margin: Option<f64>,
        within_margin: Option<bool>,
        interpretation: String,
    ) -> Self {
        Self::DescriptiveMargin {
            comparison_name: comparison_name.to_owned(),
            statistic,
            margin,
            within_margin,
            interpretation,
        }
    }

    pub(crate) fn insufficient_data(
        comparison_name: &str,
        method: CurveComparisonMethod,
        metric: &str,
        margin: Option<f64>,
        reason: String,
    ) -> Self {
        Self::InsufficientData {
            comparison_name: comparison_name.to_owned(),
            method,
            metric: metric.to_owned(),
            margin,
            reason,
        }
    }

    pub(crate) fn into_output(self) -> CurveComparisonResult {
        let (
            comparison_name,
            method,
            metric,
            availability,
            statistic,
            unavailable_reason,
            pooled_bin_p_value,
            margin,
            within_margin,
            interpretation,
        ) = match self {
            #[cfg(any(feature = "cli", test))]
            Self::PooledBin {
                comparison_name,
                statistic,
                p_value,
                interpretation,
            } => (
                comparison_name,
                CurveComparisonMethod::PooledBinPermutation,
                STANDARDIZED_DIFFERENCE_METRIC.to_owned(),
                CurveComparisonAvailability::Available,
                Some(statistic),
                None,
                Some(p_value),
                None,
                None,
                interpretation,
            ),
            Self::DescriptiveMargin {
                comparison_name,
                statistic,
                margin,
                within_margin,
                interpretation,
            } => (
                comparison_name,
                CurveComparisonMethod::DescriptiveMargin,
                STANDARDIZED_DIFFERENCE_METRIC.to_owned(),
                CurveComparisonAvailability::Available,
                Some(statistic),
                None,
                None,
                margin,
                within_margin,
                interpretation,
            ),
            Self::InsufficientData {
                comparison_name,
                method,
                metric,
                margin,
                reason,
            } => (
                comparison_name,
                method,
                metric,
                CurveComparisonAvailability::InsufficientData,
                None,
                Some(reason.clone()),
                None,
                margin,
                None,
                reason,
            ),
        };

        CurveComparisonResult {
            comparison_name,
            method,
            metric,
            availability,
            statistic,
            unavailable_reason,
            pooled_bin_p_value,
            margin,
            within_margin,
            interpretation,
        }
    }
}
