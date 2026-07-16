use super::*;

pub(super) fn run(
    manifest: PathBuf,
    config: PathBuf,
    out: PathBuf,
    threads: Option<usize>,
) -> Result<()> {
    let rows = read_manifest_rows(&manifest)?;
    #[cfg(feature = "parallel")]
    let batch_threads = batch_thread_count(&config, threads)?;

    #[cfg(feature = "parallel")]
    if rows.len() > 1 && batch_threads > 1 {
        use rayon::prelude::*;

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(batch_threads)
            .thread_name(|i| format!("marklab-batch-{i}"))
            .build()
            .map_err(|error| MarklabError::Compute(error.to_string()))?;

        return pool.install(|| {
            rows.into_par_iter().try_for_each(|row| {
                analyze::run(AnalyzeRequest {
                    cells: row.cells,
                    mask: row.mask,
                    config: config.clone(),
                    out: out.join(row.id),
                    threads: Some(1),
                    observability: ObservabilityOptions::default(),
                    heap_profile: None,
                })
            })
        });
    }

    for row in rows {
        analyze::run(AnalyzeRequest {
            cells: row.cells,
            mask: row.mask,
            config: config.clone(),
            out: out.join(row.id),
            threads,
            observability: ObservabilityOptions::default(),
            heap_profile: None,
        })?;
    }
    Ok(())
}

fn read_manifest_rows(manifest: &Path) -> Result<Vec<ManifestRow>> {
    let mut reader = ::csv::Reader::from_path(manifest)?;
    let mut rows = Vec::new();
    for row in reader.deserialize::<ManifestRow>() {
        rows.push(row?);
    }
    Ok(rows)
}

#[cfg(feature = "parallel")]
fn batch_thread_count(config: &Path, threads: Option<usize>) -> Result<usize> {
    let mut config = AnalysisConfig::from_toml_path(config)?;
    if let Some(threads) = threads {
        config.performance.threads = ThreadSetting::Count(threads);
    }
    if config.performance.strict_repro {
        return Ok(1);
    }

    let thread_count = match config.performance.threads {
        ThreadSetting::Auto => std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1),
        ThreadSetting::Count(count) => count.max(1),
    };
    Ok(thread_count)
}
