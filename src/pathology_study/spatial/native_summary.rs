//! Typed study input from native estimators. JSON is retained only for published detail fields.

use super::{native, Failure};
use serde::Serialize;
use serde_json::Value;

pub(super) struct CurvePoint {
    pub(super) value: Option<f64>,
    pub(super) eligible: bool,
    pub(super) eligible_centers: usize,
}

pub(super) struct NativeSummary {
    pub(super) curve: Vec<CurvePoint>,
    pub(super) inference: native::InhomogeneousSpatialInference,
    pub(super) charged_work: usize,
    pub(super) estimated_storage_bytes: usize,
    pub(super) intensity: Value,
    pub(super) support: Value,
}

trait StudyPoint: Serialize {
    fn study_point(&self) -> CurvePoint;
}

impl StudyPoint for native::InhomogeneousSpatialPoint {
    fn study_point(&self) -> CurvePoint {
        CurvePoint {
            value: self.l,
            eligible: self.inference_eligible,
            eligible_centers: self.eligible_centers,
        }
    }
}

impl StudyPoint for native::InhomogeneousPairCorrelationPoint {
    fn study_point(&self) -> CurvePoint {
        CurvePoint {
            value: self.g,
            eligible: self.inference_eligible,
            eligible_centers: self.eligible_centers,
        }
    }
}

impl StudyPoint for native::InhomogeneousCategoricalCrossPairCorrelationPoint {
    fn study_point(&self) -> CurvePoint {
        CurvePoint {
            value: self.cross_g,
            eligible: self.inference_eligible,
            eligible_centers: self.eligible_source_centers,
        }
    }
}

impl NativeSummary {
    fn new(
        curve: &[impl StudyPoint],
        intensity: impl Serialize,
        inference: native::InhomogeneousSpatialInference,
        work: [usize; 2],
        estimated_storage_bytes: usize,
    ) -> Result<Self, Failure> {
        let charged_work = work
            .into_iter()
            .try_fold(inference.null_draws, usize::checked_add)
            .ok_or_else(|| Failure::Fatal("native spatial work overflow".into()))?;
        Ok(Self {
            curve: curve.iter().map(StudyPoint::study_point).collect(),
            inference,
            charged_work,
            estimated_storage_bytes,
            intensity: serde_json::to_value(intensity)
                .map_err(|error| Failure::Fatal(error.to_string()))?,
            support: serde_json::to_value(curve)
                .map_err(|error| Failure::Fatal(error.to_string()))?,
        })
    }
}

impl TryFrom<native::InhomogeneousSpatialResult> for NativeSummary {
    type Error = Failure;
    fn try_from(result: native::InhomogeneousSpatialResult) -> Result<Self, Failure> {
        Self::new(
            &result.curve,
            result.intensity,
            result.inference,
            [result.intensity_evaluations, result.total_pair_visits],
            result.estimated_storage_bytes,
        )
    }
}

impl TryFrom<native::InhomogeneousPairCorrelationResult> for NativeSummary {
    type Error = Failure;
    fn try_from(result: native::InhomogeneousPairCorrelationResult) -> Result<Self, Failure> {
        Self::new(
            &result.curve,
            result.intensity,
            result.inference,
            [result.intensity_evaluations, result.total_pair_visits],
            result.estimated_storage_bytes,
        )
    }
}

impl TryFrom<native::PiecewiseCompartmentSpatialResult> for NativeSummary {
    type Error = Failure;
    fn try_from(result: native::PiecewiseCompartmentSpatialResult) -> Result<Self, Failure> {
        Self::new(
            &result.curve,
            result.intensity,
            result.inference,
            [result.compartment_queries, result.total_pair_visits],
            result.estimated_storage_bytes,
        )
    }
}

impl TryFrom<native::PiecewiseCompartmentPairCorrelationResult> for NativeSummary {
    type Error = Failure;
    fn try_from(
        result: native::PiecewiseCompartmentPairCorrelationResult,
    ) -> Result<Self, Failure> {
        Self::new(
            &result.curve,
            result.intensity,
            result.inference,
            [result.compartment_queries, result.total_pair_visits],
            result.estimated_storage_bytes,
        )
    }
}

impl TryFrom<native::InhomogeneousCategoricalCrossPairCorrelationResult> for NativeSummary {
    type Error = Failure;
    fn try_from(
        result: native::InhomogeneousCategoricalCrossPairCorrelationResult,
    ) -> Result<Self, Failure> {
        #[derive(Serialize)]
        struct Intensities {
            source: native::InhomogeneousIntensitySummary,
            target: native::InhomogeneousIntensitySummary,
        }
        Self::new(
            &result.curve,
            Intensities {
                source: result.source_intensity,
                target: result.target_intensity,
            },
            result.inference,
            [result.intensity_evaluations, result.total_pair_visits],
            result.estimated_storage_bytes,
        )
    }
}
