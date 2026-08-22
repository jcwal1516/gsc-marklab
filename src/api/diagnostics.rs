use crate::{
    config::AnalysisConfig,
    data::Pattern,
    errors::Result,
    output::{AnalysisSection, DiagnosticsResult, TimingStage},
};

use super::{beta_posterior_group_summary, timed_stage};

pub(super) fn run(
    config: &AnalysisConfig,
    pattern: &Pattern,
    timings: &mut Vec<TimingStage>,
    threads: usize,
) -> Result<AnalysisSection<DiagnosticsResult>> {
    if !config.diagnostics.beta_posterior_groups {
        return Ok(AnalysisSection::Disabled);
    }

    let summary = timed_stage(timings, "diagnostic_beta_posterior_groups", threads, || {
        beta_posterior_group_summary(pattern)
    })?;
    Ok(AnalysisSection::available(DiagnosticsResult {
        beta_posterior_groups: Some(summary),
        graph_smoothing: None,
    }))
}
