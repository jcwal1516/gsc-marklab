//! Two-column text spectra: m/z and intensity per line, as in DRIAMS `raw` and `preprocessed`
//! files and MALDIquant text exports.

use std::path::Path;

use crate::{read::read_capped, MassSpectrum, SpectrumError, MAXIMUM_BRUKER_TIME_POINTS};

const MAXIMUM_TEXT_BYTES: u64 = 128 * 1024 * 1024;

/// Read a text spectrum file; see [`parse_text_spectrum`].
pub fn read_text_spectrum(path: &Path) -> Result<MassSpectrum, SpectrumError> {
    parse_text_spectrum(&read_capped(path, MAXIMUM_TEXT_BYTES)?)
}

/// Parse a text spectrum. Lines starting with `#` and blank lines are ignored, and the first
/// remaining line may be a header of two non-numeric fields. Every other line must hold exactly
/// two finite numbers, m/z and a non-negative intensity, separated by whitespace, with m/z
/// strictly increasing.
pub fn parse_text_spectrum(bytes: &[u8]) -> Result<MassSpectrum, SpectrumError> {
    let text = String::from_utf8_lossy(bytes);
    let (mut mz, mut intensity) = (Vec::new(), Vec::new());
    let mut header_allowed = true;
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() != 2 {
            return Err(SpectrumError::Text(format!(
                "line {}: expected 2 columns, found {}",
                index + 1,
                fields.len()
            )));
        }
        match (fields[0].parse::<f64>(), fields[1].parse::<f64>()) {
            (Ok(m), Ok(i)) if m.is_finite() && i.is_finite() => {
                mz.push(m);
                intensity.push(i);
            }
            (Err(_), Err(_)) if header_allowed => {}
            _ => {
                return Err(SpectrumError::Text(format!(
                    "line {}: {line:?} is not two finite numbers",
                    index + 1
                )))
            }
        }
        header_allowed = false;
        if mz.len() > MAXIMUM_BRUKER_TIME_POINTS {
            return Err(SpectrumError::Text(format!(
                "more than {MAXIMUM_BRUKER_TIME_POINTS} points"
            )));
        }
    }
    MassSpectrum::new(mz, intensity).map_err(|error| SpectrumError::Text(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_driams_style_spectra_with_comments_and_a_header() {
        let text = "#  /data/0_E3/1/1SLin/fid\r\n#  001c791c\r\n\
                    \"mass.myspec..1...\" \"intensity.myspec..1...\"\r\n\
                    1958.51386932871 2348\r\n\r\n1958.92950438406 2.5e-3\r\n";
        let spectrum = parse_text_spectrum(text.as_bytes()).unwrap();
        assert_eq!(spectrum.mz(), &[1958.51386932871, 1958.92950438406]);
        assert_eq!(spectrum.intensity(), &[2348.0, 0.0025]);
    }

    #[test]
    fn malformed_text_spectra_are_rejected() {
        for (text, expected) in [
            ("1 2 3\n", "line 1: expected 2 columns"),
            ("mass intensity\n1 2\nmass intensity\n", "line 3"),
            ("1 nan\n", "line 1"),
            ("2 1\n1 1\n", "strictly increasing"),
            ("1 -1\n", "non-negative"),
            ("# only a comment\nmass intensity\n", "no points"),
        ] {
            let error = parse_text_spectrum(text.as_bytes())
                .expect_err(text)
                .to_string();
            assert!(error.contains(expected), "{text:?}: {error}");
        }
    }
}
