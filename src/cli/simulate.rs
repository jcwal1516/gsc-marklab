use super::*;

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
                write_pattern_parquet(&pattern, out)?;
            }
            #[cfg(not(feature = "parquet"))]
            bail!("Parquet simulation output requires the parquet feature");
        }
        _ => bail!("simulation output extension must be .parquet or .csv"),
    }

    Ok(())
}
