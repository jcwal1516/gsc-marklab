//! `marklab microbio`: MALDI-TOF mass spectra of bacterial isolates.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

use marklab_microbio::{
    bin_spectrum, preprocess_spectrum, read_bruker_flex, read_text_spectrum, BinSpec,
    BinnedSpectrum, BrukerByteOrder, BrukerTofMode, MassSpectrum, MicrobioError, PreprocessStep,
};
use serde::{Deserialize, Serialize};

use crate::{MarklabError, Result};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreprocessConfig {
    steps: Vec<PreprocessStep>,
    binning: Option<BinSpec>,
}

/// Arguments of `marklab microbio preprocess`.
#[derive(clap::Args, Debug)]
pub struct PreprocessArgs {
    /// Bruker flex spectrum directory holding `fid` and `acqu` (or `acqus`), such as
    /// `0_A1/1/1SLin`.
    #[arg(long, required_unless_present = "text", conflicts_with = "text")]
    bruker: Option<PathBuf>,
    /// Two-column text spectrum of m/z and intensity, such as a DRIAMS `raw` file.
    #[arg(long)]
    text: Option<PathBuf>,
    /// JSON configuration with the ordered preprocessing `steps` and optional `binning`.
    #[arg(long)]
    config: PathBuf,
    /// File to write the result to; it must not already exist.
    #[arg(long)]
    out: PathBuf,
}

#[derive(Serialize)]
#[serde(tag = "format", rename_all = "snake_case")]
enum Source {
    BrukerFlex {
        time_points: usize,
        byte_order: BrukerByteOrder,
        laser_shots: u64,
        tof_mode: Option<BrukerTofMode>,
        negative_intensities_clipped: usize,
    },
    Text {
        points: usize,
    },
}

#[derive(Serialize)]
struct PreprocessOutput {
    source: Source,
    steps: Vec<PreprocessStep>,
    spectrum: MassSpectrum,
    binned: Option<BinnedSpectrum>,
}

pub(super) fn run_preprocess(args: PreprocessArgs) -> Result<()> {
    let PreprocessArgs {
        bruker,
        text,
        config,
        out,
    } = args;
    let bytes = super::simulate::read_input(&config, "microbio preprocess configuration")?;
    let config: PreprocessConfig = serde_json::from_slice(&bytes)?;
    let (source, input) = match (bruker, text) {
        (Some(bruker), None) => {
            let acquisition = read_bruker_flex(&bruker).map_err(into_marklab)?;
            let source = Source::BrukerFlex {
                time_points: acquisition.time_points,
                byte_order: acquisition.byte_order,
                laser_shots: acquisition.laser_shots,
                tof_mode: acquisition.tof_mode,
                negative_intensities_clipped: acquisition.negative_intensities_clipped,
            };
            (source, acquisition.spectrum)
        }
        (None, Some(text)) => {
            let spectrum = read_text_spectrum(&text).map_err(into_marklab)?;
            let source = Source::Text {
                points: spectrum.mz().len(),
            };
            (source, spectrum)
        }
        _ => unreachable!("clap requires exactly one of --bruker and --text"),
    };
    let spectrum = preprocess_spectrum(&input, &config.steps).map_err(into_marklab)?;
    let binned = config
        .binning
        .as_ref()
        .map(|spec| bin_spectrum(&spectrum, spec))
        .transpose()
        .map_err(into_marklab)?;
    let output = PreprocessOutput {
        source,
        steps: config.steps,
        spectrum,
        binned,
    };
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|source| MarklabError::io(parent, source))?;
    }
    let bytes = serde_json::to_vec_pretty(&output)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&out)
        .map_err(|source| MarklabError::io(&out, source))?;
    file.write_all(&bytes)
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.sync_all())
        .map_err(|source| MarklabError::io(&out, source))
}

fn into_marklab(error: MicrobioError) -> MarklabError {
    match error {
        MicrobioError::Io { path, source } => MarklabError::io(path, source),
        other => MarklabError::Validation(other.to_string()),
    }
}
