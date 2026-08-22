use std::path::Path;

use crate::{api::MarkedAnalysisRun, config::OutputSection, errors::Result};

use super::{OutputManifest, OutputWriter, ResultDocument};

impl OutputWriter {
    pub fn write_marked_run(
        run: MarkedAnalysisRun,
        out: impl AsRef<Path>,
        options: &OutputSection,
    ) -> Result<OutputManifest> {
        Self::write(&ResultDocument::marked(run.result), out, options)
    }
}
