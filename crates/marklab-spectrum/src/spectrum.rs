use serde::Serialize;

use crate::SpectrumError;

/// A mass spectrum with strictly increasing, positive m/z values and finite, non-negative
/// intensities of equal length.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MassSpectrum {
    pub(crate) mz: Vec<f64>,
    pub(crate) intensity: Vec<f64>,
}

impl MassSpectrum {
    /// Validate and build a spectrum.
    pub fn new(mz: Vec<f64>, intensity: Vec<f64>) -> Result<Self, SpectrumError> {
        let invalid = |message: String| Err(SpectrumError::Spectrum(message));
        if mz.len() != intensity.len() {
            return invalid(format!(
                "{} m/z values but {} intensities",
                mz.len(),
                intensity.len()
            ));
        }
        if mz.is_empty() {
            return invalid("the spectrum has no points".to_owned());
        }
        if let Some(index) = mz
            .iter()
            .position(|value| !(value.is_finite() && *value > 0.0))
        {
            return invalid(format!(
                "m/z value {} at point {index} is not finite and positive",
                mz[index]
            ));
        }
        if let Some(index) = mz.windows(2).position(|pair| pair[1] <= pair[0]) {
            return invalid(format!(
                "m/z values are not strictly increasing at point {}",
                index + 1
            ));
        }
        if let Some(index) = intensity
            .iter()
            .position(|value| !(value.is_finite() && *value >= 0.0))
        {
            return invalid(format!(
                "intensity {} at point {index} is not finite and non-negative",
                intensity[index]
            ));
        }
        Ok(Self { mz, intensity })
    }

    /// m/z values in increasing order.
    pub fn mz(&self) -> &[f64] {
        &self.mz
    }

    /// Intensities in m/z order.
    pub fn intensity(&self) -> &[f64] {
        &self.intensity
    }
}
