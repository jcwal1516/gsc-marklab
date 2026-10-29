//! `marklab spectrum`: read and preprocess one-dimensional spectra, such as MALDI-TOF mass
//! spectra, and collect binned spectra into feature tables.

use std::{
    fs::{self, File},
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use marklab_microbio::{
    bin_spectrum, preprocess_spectrum, read_bruker_flex, read_text_spectrum, BinSpec,
    BinnedSpectrum, BrukerByteOrder, BrukerTofMode, MassSpectrum, MicrobioError, PreprocessStep,
};
use marklab_prediction::MAXIMUM_LOGISTIC_VALUES;
use serde::{Deserialize, Serialize};

use super::feature_table::{
    check_id, is_name, write_feature_table, write_new_json, FeatureTable, FeatureTableMeta,
};
use crate::{MarklabError, Result};

const MAXIMUM_PREPROCESS_RESULT_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreprocessConfig {
    steps: Vec<PreprocessStep>,
    binning: Option<BinSpec>,
}

/// Arguments of `marklab spectrum preprocess`.
#[derive(clap::Args, Debug)]
pub struct PreprocessArgs {
    /// Bruker flex spectrum directory holding `fid` and `acqu` (or `acqus`), such as
    /// `0_A1/1/1SLin`.
    #[arg(long, required_unless_present = "text", conflicts_with = "text")]
    bruker: Option<PathBuf>,
    /// Two-column text spectrum: strictly increasing m/z and non-negative intensity.
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
    let bytes = super::simulate::read_input(&config, "spectrum preprocess configuration")?;
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
    write_new_json(&out, &output)
}

/// Arguments of `marklab spectrum feature-table`.
#[derive(clap::Args, Debug)]
pub struct FeatureTableArgs {
    /// Directory of `marklab spectrum preprocess` results with `binning`. Every `*.json` file in
    /// it is one row, whose ID is the file name without `.json`.
    #[arg(long)]
    spectra: PathBuf,
    /// What the spectra are, such as `maldi_tof`. Recorded in the table; a model only accepts
    /// tables of its training modality.
    #[arg(long)]
    modality: String,
    /// What one spectrum (row) stands for, such as `isolate`. Recorded like the modality.
    #[arg(long)]
    unit: String,
    /// Feature table (Parquet) to write; it must not already exist.
    #[arg(long)]
    out: PathBuf,
}

#[derive(Deserialize)]
struct BinnedResult {
    steps: serde_json::Value,
    binned: Option<BinnedIntensities>,
}

#[derive(Deserialize)]
struct BinnedIntensities {
    min_mz: f64,
    max_mz: f64,
    bins: usize,
    intensity: Vec<f64>,
}

/// Collect binned spectra into a feature table with columns `bin_0`, `bin_1`, …. All spectra
/// must share the preprocessing steps and bins, which become the provenance.
pub(super) fn run_feature_table(args: FeatureTableArgs) -> Result<()> {
    if !is_name(&args.modality) || !is_name(&args.unit) {
        bail!("--modality and --unit must be non-empty and without surrounding spaces");
    }
    let directory = &args.spectra;
    let io_error = |source| MarklabError::io(directory, source);
    let mut paths = Vec::new();
    for entry in fs::read_dir(directory).map_err(io_error)? {
        let path = entry.map_err(io_error)?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            paths.push(path);
        }
    }
    paths.sort();
    let Some(first_path) = paths.first().cloned() else {
        bail!("{}: no `*.json` preprocess results", directory.display());
    };
    let mut provenance = None;
    let mut ids = Vec::with_capacity(paths.len());
    let mut values = Vec::new();
    let mut bins = 0;
    for path in &paths {
        let Some(id) = path.file_stem().and_then(|stem| stem.to_str()) else {
            bail!("{}: the file name is not valid UTF-8", path.display());
        };
        check_id(id, ids.len() + 1)?;
        let result = read_binned_result(path)?;
        let Some(binned) = result.binned else {
            bail!(
                "{}: no binned intensities; preprocess with `binning`",
                path.display()
            );
        };
        if binned.intensity.len() != binned.bins {
            bail!(
                "{}: {} intensities for {} bins",
                path.display(),
                binned.intensity.len(),
                binned.bins
            );
        }
        let this = serde_json::json!({
            "steps": result.steps,
            "binning": {"min_mz": binned.min_mz, "max_mz": binned.max_mz, "bins": binned.bins},
        });
        match &provenance {
            None => {
                if paths.len().saturating_mul(binned.bins) > MAXIMUM_LOGISTIC_VALUES {
                    bail!(
                        "{} spectra × {} bins exceed {MAXIMUM_LOGISTIC_VALUES} values",
                        paths.len(),
                        binned.bins
                    );
                }
                bins = binned.bins;
                values.reserve_exact(paths.len() * bins);
                provenance = Some(this);
            }
            Some(first) if *first != this => bail!(
                "{}: preprocessing steps or bins differ from {}",
                path.display(),
                first_path.display()
            ),
            Some(_) => {}
        }
        values.extend_from_slice(&binned.intensity);
        ids.push(id.to_owned());
    }
    let table = FeatureTable {
        meta: FeatureTableMeta {
            version: 1,
            modality: args.modality,
            unit: args.unit,
            provenance: provenance.expect("at least one spectrum was read"),
        },
        columns: (0..bins).map(|bin| format!("bin_{bin}")).collect(),
        ids,
        values,
    };
    write_feature_table(&args.out, &table)
}

fn read_binned_result(path: &Path) -> Result<BinnedResult> {
    let io_error = |source| MarklabError::io(path, source);
    let unusable = || {
        MarklabError::Validation(format!(
            "{}: a preprocess result must be a regular file of at most 512 MiB",
            path.display()
        ))
    };
    // Check before opening: opening a FIFO would block.
    if !fs::metadata(path).map_err(io_error)?.is_file() {
        return Err(unusable());
    }
    let file = File::open(path).map_err(io_error)?;
    if file.metadata().map_err(io_error)?.len() > MAXIMUM_PREPROCESS_RESULT_BYTES {
        return Err(unusable());
    }
    let reader = BufReader::new(file.take(MAXIMUM_PREPROCESS_RESULT_BYTES));
    serde_json::from_reader(reader)
        .map_err(|error| MarklabError::Validation(format!("{}: {error}", path.display())))
}

fn into_marklab(error: MicrobioError) -> MarklabError {
    match error {
        MicrobioError::Io { path, source } => MarklabError::io(path, source),
        other => MarklabError::Validation(other.to_string()),
    }
}
