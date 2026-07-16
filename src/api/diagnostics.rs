use super::*;

pub(super) fn run(
    config: &AnalysisConfig,
    pattern: &Pattern,
    timings: &mut Vec<TimingStage>,
    threads: usize,
) -> Result<crate::output::AnalysisSection<DiagnosticsResult>> {
    if !config.diagnostics.beta_binomial {
        return Ok(crate::output::AnalysisSection::Disabled);
    }

    let beta_binomial = timed_stage(timings, "diagnostic_beta_binomial", threads, || {
        beta_binomial(pattern)
    })?;
    Ok(crate::output::AnalysisSection::available(
        DiagnosticsResult {
            beta_binomial: Some(beta_binomial),
            graph_smoothing: None,
        },
    ))
}
