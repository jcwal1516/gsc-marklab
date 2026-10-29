//! Reader for Bruker Daltonics flex-series (XMASS) acquisitions: a binary `fid` file of
//! 32-bit intensities and its `acqu` (or `acqus`) JCAMP-DX parameter file.

use std::path::Path;

use serde::Serialize;

use crate::{read::read_capped, MassSpectrum, SpectrumError};

/// Largest accepted number of time points (`TD`), so a `fid` file is at most 16 MiB.
pub const MAXIMUM_BRUKER_TIME_POINTS: usize = 4 * 1024 * 1024;
const MAXIMUM_ACQU_BYTES: u64 = 1024 * 1024;

/// Byte order of the `fid` intensities (`BYTORDA`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrukerByteOrder {
    /// `BYTORDA` 0.
    Little,
    /// `BYTORDA` 1.
    Big,
}

/// Time-of-flight operating mode (`AQOP_m`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrukerTofMode {
    /// `AQOP_m` 0.
    Linear,
    /// `AQOP_m` 1.
    Reflector,
}

/// One acquisition converted to m/z, with the acquisition facts the conversion relied on.
#[derive(Clone, Debug, PartialEq)]
pub struct BrukerAcquisition {
    /// Intensities at their calibrated m/z values; negative raw counts are set to zero.
    pub spectrum: MassSpectrum,
    /// Number of time points (`TD`).
    pub time_points: usize,
    /// Byte order of the `fid` file.
    pub byte_order: BrukerByteOrder,
    /// Number of laser shots summed into the spectrum (`NoSHOTS`).
    pub laser_shots: u64,
    /// Time-of-flight mode, when `AQOP_m` is present.
    pub tof_mode: Option<BrukerTofMode>,
    /// Number of negative raw intensities that were set to zero.
    pub negative_intensities_clipped: usize,
}

/// Read the acquisition in `spectrum_dir`, the directory holding `fid` and `acqu` (for example
/// `0_A1/1/1SLin`). `acqu` is used when present, otherwise `acqus`.
pub fn read_bruker_flex(spectrum_dir: &Path) -> Result<BrukerAcquisition, SpectrumError> {
    let acqu_path = ["acqu", "acqus"]
        .iter()
        .map(|name| spectrum_dir.join(name))
        .find(|path| path.is_file())
        .ok_or_else(|| {
            SpectrumError::Bruker(format!(
                "{} contains neither an acqu nor an acqus file",
                spectrum_dir.display()
            ))
        })?;
    let acqu = read_capped(&acqu_path, MAXIMUM_ACQU_BYTES)?;
    let fid = read_capped(
        &spectrum_dir.join("fid"),
        4 * MAXIMUM_BRUKER_TIME_POINTS as u64,
    )?;
    parse_bruker_flex(&acqu, &fid)
}

/// Parse an acquisition from the bytes of its `acqu` and `fid` files.
///
/// m/z follows the flex-series calibration `tof = ML2 + sqrt(1e12 / ML1) sqrt(m) + ML3 m` with
/// `tof = DELAY + i DW`, as in the R package readBrukerFlexData. High-precision calibration,
/// LIFT, PSD and V1.0 CTOF2 calibrations are rejected rather than approximated.
pub fn parse_bruker_flex(acqu: &[u8], fid: &[u8]) -> Result<BrukerAcquisition, SpectrumError> {
    let fields = AcquFields::parse(acqu);
    reject_unsupported(&fields)?;
    let byte_order = match fields.required("BYTORDA")? {
        "0" => BrukerByteOrder::Little,
        "1" => BrukerByteOrder::Big,
        other => return Err(bruker(format!("BYTORDA value {other:?} is not 0 or 1"))),
    };
    let time_points = fields.count("TD")?;
    if !(1..=MAXIMUM_BRUKER_TIME_POINTS).contains(&time_points) {
        return Err(bruker(format!(
            "TD = {time_points} is outside 1..={MAXIMUM_BRUKER_TIME_POINTS}"
        )));
    }
    let laser_shots = fields.count("NoSHOTS")? as u64;
    if laser_shots == 0 {
        return Err(bruker("the acquisition has no laser shots (NoSHOTS = 0)"));
    }
    let tof_mode = match fields.optional("AQOP_m")? {
        None => None,
        Some("0") => Some(BrukerTofMode::Linear),
        Some("1") => Some(BrukerTofMode::Reflector),
        Some(other) => return Err(bruker(format!("AQOP_m value {other:?} is not 0 or 1"))),
    };
    let delay = fields.finite("DELAY")?;
    let time_delta = fields.finite("DW")?;
    let ml1 = fields.finite("ML1")?;
    let ml2 = fields.finite("ML2")?;
    let ml3 = fields.finite("ML3")?;
    if time_delta <= 0.0 {
        return Err(bruker(format!("DW = {time_delta} is not positive")));
    }
    if ml1 <= 0.0 {
        return Err(bruker(format!("ML1 = {ml1} is not positive")));
    }
    if fid.len() != 4 * time_points {
        return Err(bruker(format!(
            "fid holds {} bytes but TD = {time_points} requires {}",
            fid.len(),
            4 * time_points
        )));
    }

    let mut negative_intensities_clipped = 0;
    let (counts, _) = fid.as_chunks::<4>();
    let intensity = counts
        .iter()
        .map(|&bytes| {
            let count = match byte_order {
                BrukerByteOrder::Little => i32::from_le_bytes(bytes),
                BrukerByteOrder::Big => i32::from_be_bytes(bytes),
            };
            if count < 0 {
                negative_intensities_clipped += 1;
                0.0
            } else {
                f64::from(count)
            }
        })
        .collect();
    let b = (1e12 / ml1).sqrt();
    // readBrukerFlexData's root (-b + sqrt(b² - 4 ML3 c)) / (2 ML3), rearranged to avoid
    // cancellation when ML3 is small; ML3 = 0 gives -c / b.
    let mz = (0..time_points)
        .map(|index| {
            let c = ml2 - (delay + index as f64 * time_delta);
            let root = -2.0 * c / (b + (b * b - 4.0 * ml3 * c).sqrt());
            root * root
        })
        .collect();
    let spectrum = MassSpectrum::new(mz, intensity).map_err(|error| {
        bruker(format!(
            "the calibration does not give usable m/z values ({error})"
        ))
    })?;
    Ok(BrukerAcquisition {
        spectrum,
        time_points,
        byte_order,
        laser_shots,
        tof_mode,
        negative_intensities_clipped,
    })
}

fn reject_unsupported(fields: &AcquFields) -> Result<(), SpectrumError> {
    if fields.optional("HPClUse")? == Some("yes") {
        let in_use = fields.finite("HPClBLo")? > 0.0
            && fields.finite("HPClBHi")? > 0.0
            && fields.finite("HPClOrd")? > 0.0;
        if in_use {
            return Err(bruker(
                "the acquisition uses Bruker high-precision calibration (HPC), which is not supported",
            ));
        }
    }
    if fields
        .optional("NTBCal")?
        .is_some_and(|value| value.contains("V1.0CTOF2CalibrationConstants"))
    {
        return Err(bruker(
            "V1.0CTOF2CalibrationConstants calibrations are not supported",
        ));
    }
    let nonzero = |key| -> Result<bool, SpectrumError> {
        Ok(match fields.optional(key)? {
            Some(_) => fields.finite(key)? != 0.0,
            None => false,
        })
    };
    if (nonzero("Lift1")? && nonzero("Lift2")?) || nonzero("TLift")? {
        return Err(bruker("LIFT acquisitions are not supported"));
    }
    match fields.optional("SPType")? {
        None | Some("0") => Ok(()),
        Some(code) => {
            let name = match code {
                "1" => "PSD",
                "2" => "LIFT",
                "3" => "PSDSegment",
                other => other,
            };
            Err(bruker(format!(
                "spectrum type {name} is not supported; only TOF spectra (SPType 0) are read"
            )))
        }
    }
}

fn bruker(message: impl Into<String>) -> SpectrumError {
    SpectrumError::Bruker(message.into())
}

/// `##$KEY= value` parameters of an `acqu` file. Values keep their text; enclosing `<` `>`
/// and surrounding spaces are removed. LF, CRLF and CR line endings are accepted.
struct AcquFields {
    fields: Vec<(String, String)>,
}

impl AcquFields {
    fn parse(acqu: &[u8]) -> Self {
        let text = String::from_utf8_lossy(acqu);
        let fields = text
            .split(['\r', '\n'])
            .filter_map(|line| line.strip_prefix("##$")?.split_once('='))
            .map(|(key, value)| {
                let value = value.trim();
                let value = value.strip_prefix('<').unwrap_or(value);
                let value = value.strip_suffix('>').unwrap_or(value);
                (key.to_owned(), value.trim().to_owned())
            })
            .collect();
        Self { fields }
    }

    fn optional(&self, key: &str) -> Result<Option<&str>, SpectrumError> {
        let mut matches = self.fields.iter().filter(|(name, _)| name == key);
        let first = matches.next().map(|(_, value)| value.as_str());
        if matches.next().is_some() {
            return Err(bruker(format!("{key} appears more than once in acqu")));
        }
        Ok(first)
    }

    fn required(&self, key: &str) -> Result<&str, SpectrumError> {
        self.optional(key)?
            .ok_or_else(|| bruker(format!("{key} is missing from acqu")))
    }

    /// A finite number; a decimal comma is read as a point, as readBrukerFlexData does.
    fn finite(&self, key: &str) -> Result<f64, SpectrumError> {
        let text = self.required(key)?;
        text.replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .ok_or_else(|| bruker(format!("{key} value {text:?} is not a finite number")))
    }

    fn count(&self, key: &str) -> Result<usize, SpectrumError> {
        let text = self.required(key)?;
        text.parse::<usize>().map_err(|_| {
            bruker(format!(
                "{key} value {text:?} is not a non-negative integer"
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Case name, `acqu` fields, `fid` bytes and expected error text.
    type RejectionCase<'a> = (&'a str, Vec<(&'a str, &'a str)>, Vec<u8>, &'a str);

    fn base_fields() -> Vec<(&'static str, &'static str)> {
        vec![
            ("BYTORDA", "0"),
            ("TD", "3"),
            ("DELAY", "21010"),
            ("DW", "2"),
            ("ML1", "1000000"),
            ("ML2", "10"),
            ("ML3", "0"),
            ("NoSHOTS", "200"),
            ("AQOP_m", "0"),
            ("SPType", "0"),
            ("Lift1", "0"),
            ("Lift2", "0"),
            ("TLift", "0"),
            ("HPClUse", "no"),
        ]
    }

    fn with<'a>(
        mut fields: Vec<(&'a str, &'a str)>,
        key: &'a str,
        value: &'a str,
    ) -> Vec<(&'a str, &'a str)> {
        match fields.iter_mut().find(|(name, _)| *name == key) {
            Some(field) => field.1 = value,
            None => fields.push((key, value)),
        }
        fields
    }

    fn acqu(fields: &[(&str, &str)]) -> Vec<u8> {
        let mut text = String::from("##TITLE= XMASS Parameter file\r\n##JCAMPDX= 5.0\r\n");
        for (key, value) in fields {
            text.push_str(&format!("##${key}= {value} \r\n"));
        }
        text.push_str("##END= \r\n");
        text.into_bytes()
    }

    fn fid(values: &[i32], byte_order: BrukerByteOrder) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| match byte_order {
                BrukerByteOrder::Little => value.to_le_bytes(),
                BrukerByteOrder::Big => value.to_be_bytes(),
            })
            .collect()
    }

    #[test]
    fn reads_linear_calibration_and_clips_negative_counts() {
        let acquisition = parse_bruker_flex(
            &acqu(&base_fields()),
            &fid(&[5, -7, 9], BrukerByteOrder::Little),
        )
        .unwrap();
        // ML3 = 0: m/z = (tof - ML2)^2 / (1e12 / ML1) with tof = DELAY + i DW.
        let expected = [441.0, 21002.0_f64.powi(2) / 1e6, 21004.0_f64.powi(2) / 1e6];
        for (mz, expected) in acquisition.spectrum.mz().iter().zip(expected) {
            assert!(
                (mz - expected).abs() <= 1e-12 * expected,
                "{mz} != {expected}"
            );
        }
        assert_eq!(acquisition.spectrum.intensity(), &[5.0, 0.0, 9.0]);
        assert_eq!(acquisition.negative_intensities_clipped, 1);
        assert_eq!(acquisition.time_points, 3);
        assert_eq!(acquisition.laser_shots, 200);
        assert_eq!(acquisition.tof_mode, Some(BrukerTofMode::Linear));

        // A negligible ML3 must not lose the distinct masses to cancellation.
        let nearly_linear = parse_bruker_flex(
            &acqu(&with(base_fields(), "ML3", "1e-12")),
            &fid(&[5, -7, 9], BrukerByteOrder::Little),
        )
        .unwrap();
        for (mz, expected) in nearly_linear.spectrum.mz().iter().zip(expected) {
            assert!(
                (mz - expected).abs() <= 1e-9 * expected,
                "{mz} != {expected}"
            );
        }
    }

    #[test]
    fn quadratic_calibration_inverts_the_time_of_flight_equation_big_endian() {
        // Calibration constants of readBrukerFlexData's 2010_05_19_Gibb_C8_A1 example.
        let fields = with(
            with(
                with(with(base_fields(), "BYTORDA", "1"), "DELAY", "21021"),
                "ML1",
                "2322804.10760584",
            ),
            "ML2",
            "274.099200641536",
        );
        let fields = with(fields, "ML3", "-0.00129879810255285");
        let acquisition =
            parse_bruker_flex(&acqu(&fields), &fid(&[1, 2, 70_000], BrukerByteOrder::Big)).unwrap();
        assert_eq!(acquisition.byte_order, BrukerByteOrder::Big);
        assert_eq!(acquisition.spectrum.intensity(), &[1.0, 2.0, 70_000.0]);
        let b = (1e12_f64 / 2322804.10760584).sqrt();
        for (index, mz) in acquisition.spectrum.mz().iter().enumerate() {
            let tof = 21021.0 + 2.0 * index as f64;
            let residual = -0.00129879810255285 * mz + b * mz.sqrt() + 274.099200641536 - tof;
            assert!(residual.abs() <= 1e-9 * tof, "residual {residual} at {mz}");
        }
    }

    #[test]
    fn unsupported_or_malformed_acquisitions_are_rejected() {
        let cases: Vec<RejectionCase> = vec![
            (
                "fid shorter than TD",
                base_fields(),
                fid(&[1, 2], BrukerByteOrder::Little),
                "fid holds 8 bytes",
            ),
            (
                "high-precision calibration",
                with(
                    with(
                        with(with(base_fields(), "HPClUse", "yes"), "HPClBLo", "700"),
                        "HPClBHi",
                        "3700",
                    ),
                    "HPClOrd",
                    "10",
                ),
                fid(&[1, 2, 3], BrukerByteOrder::Little),
                "high-precision calibration",
            ),
            (
                "LIFT",
                with(with(base_fields(), "Lift1", "1.5"), "Lift2", "2.5"),
                fid(&[1, 2, 3], BrukerByteOrder::Little),
                "LIFT",
            ),
            (
                "PSD spectrum type",
                with(base_fields(), "SPType", "1"),
                fid(&[1, 2, 3], BrukerByteOrder::Little),
                "PSD",
            ),
            (
                "V1.0 CTOF2 calibration",
                with(
                    base_fields(),
                    "NTBCal",
                    "<V1.0CTOF2CalibrationConstants 1 2 3>",
                ),
                fid(&[1, 2, 3], BrukerByteOrder::Little),
                "V1.0CTOF2",
            ),
            (
                "no laser shots",
                with(base_fields(), "NoSHOTS", "0"),
                fid(&[1, 2, 3], BrukerByteOrder::Little),
                "no laser shots",
            ),
            (
                "missing TD",
                base_fields()
                    .into_iter()
                    .filter(|(key, _)| *key != "TD")
                    .collect(),
                fid(&[1, 2, 3], BrukerByteOrder::Little),
                "TD is missing",
            ),
            (
                "duplicate ML1",
                {
                    let mut fields = base_fields();
                    fields.push(("ML1", "2000000"));
                    fields
                },
                fid(&[1, 2, 3], BrukerByteOrder::Little),
                "ML1 appears more than once",
            ),
            (
                "non-finite DELAY",
                with(base_fields(), "DELAY", "inf"),
                fid(&[1, 2, 3], BrukerByteOrder::Little),
                "DELAY",
            ),
            (
                "unknown byte order",
                with(base_fields(), "BYTORDA", "2"),
                fid(&[1, 2, 3], BrukerByteOrder::Little),
                "BYTORDA",
            ),
            (
                "TD above the limit",
                with(base_fields(), "TD", "4194305"),
                Vec::new(),
                "TD",
            ),
            (
                "calibration that is not monotone",
                // tof runs 21010..21018 around ML2 = 21011, so (tof - ML2)^2 repeats then rises.
                with(with(base_fields(), "TD", "5"), "ML2", "21011"),
                fid(&[1, 2, 3, 4, 5], BrukerByteOrder::Little),
                "strictly increasing",
            ),
        ];
        for (name, fields, fid_bytes, expected) in cases {
            let error = parse_bruker_flex(&acqu(&fields), &fid_bytes)
                .expect_err(name)
                .to_string();
            assert!(error.contains(expected), "{name}: {error}");
        }
    }
}
