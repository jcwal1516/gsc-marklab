use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[cfg(feature = "parquet")]
use crate::{io::parquet::write_filtered_pattern_export_parquet, Pattern, PatternMeta};
use crate::{permutation::labels::permute_fixed_count, MarklabError, Result};

pub(super) mod agent_competition;
pub(super) mod growth_front;
pub(super) mod level_set;
pub(super) mod mechanistic_tissue;
pub(super) mod reaction_diffusion;
pub(super) mod spatial_competition;
pub(super) mod summary_matching;
pub(super) mod vascular_transport;

/// Read a simulation input of at most 16 MiB. Special files are rejected before opening, the
/// opened handle is checked again, and the read is capped, so a file that grows after the
/// check cannot exceed the limit.
pub(super) fn read_input(path: &Path, label: &str) -> Result<Vec<u8>> {
    const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;
    let io_error = |source| MarklabError::io(path, source);
    let not_regular = || MarklabError::Validation(format!("{label} input must be a regular file"));
    let too_large = || MarklabError::Validation(format!("{label} input exceeds 16 MiB"));
    if !fs::metadata(path).map_err(io_error)?.is_file() {
        return Err(not_regular());
    }
    let file = fs::File::open(path).map_err(io_error)?;
    let metadata = file.metadata().map_err(io_error)?;
    if !metadata.is_file() {
        return Err(not_regular());
    }
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(too_large());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAXIMUM_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() as u64 > MAXIMUM_INPUT_BYTES {
        return Err(too_large());
    }
    Ok(bytes)
}

pub(super) fn run(n: usize, p: f64, seed: u64, out: PathBuf) -> Result<()> {
    if !(0.0..=1.0).contains(&p) || !p.is_finite() {
        bail!("--p must be a finite probability in [0, 1]");
    }

    let n_marked = (n as f64 * p).round() as usize;
    let labels = permute_fixed_count(n, n_marked, seed)?;

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }

    match out
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("csv") => {
            let mut text =
                String::from("x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n");
            for (index, label) in labels.iter().copied().enumerate() {
                let x = index as f64;
                text.push_str(&format!("{x},0.0,{label},simulated,post,MSH6,true,true\n"));
            }
            fs::write(out, text)?;
        }
        Some("parquet") => {
            #[cfg(feature = "parquet")]
            {
                let pattern = Pattern::from_arrays(
                    (0..n).map(|index| index as f64).collect(),
                    vec![0.0; n],
                    labels,
                    PatternMeta {
                        case_id: "simulated".into(),
                        timepoint: "post".into(),
                        protein: "MSH6".into(),
                        slide_id: None,
                        section_id: None,
                        stain_batch: None,
                        block_id: None,
                        region_id: None,
                    },
                )?;
                write_filtered_pattern_export_parquet(&pattern, out)?;
            }
            #[cfg(not(feature = "parquet"))]
            bail!("Parquet simulation output requires the parquet feature");
        }
        _ => bail!("simulation output extension must be .parquet or .csv"),
    }

    Ok(())
}
