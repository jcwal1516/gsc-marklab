use std::{fs, path::PathBuf};

#[cfg(feature = "parquet")]
use crate::{io::parquet::write_filtered_pattern_export_parquet, Pattern, PatternMeta};
use crate::{permutation::labels::permute_fixed_count, MarklabError, Result};

#[path = "simulate/agent_competition.rs"]
pub(super) mod agent_competition;
#[path = "simulate/growth_front.rs"]
pub(super) mod growth_front;
#[path = "simulate/level_set.rs"]
pub(super) mod level_set;
#[path = "simulate/mechanistic_tissue.rs"]
pub(super) mod mechanistic_tissue;
#[path = "simulate/reaction_diffusion.rs"]
pub(super) mod reaction_diffusion;
#[path = "simulate/spatial_competition.rs"]
pub(super) mod spatial_competition;
#[path = "simulate/summary_matching.rs"]
pub(super) mod summary_matching;
#[path = "simulate/vascular_transport.rs"]
pub(super) mod vascular_transport;

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
