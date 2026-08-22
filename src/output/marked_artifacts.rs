use std::path::Path;

use crate::{api::MarkedAnalysisRun, config::OutputSection, errors::Result};

#[cfg(feature = "cli")]
use super::manifest::RunManifestContext;
use super::{OutputManifest, OutputWriter, ResultDocument};

impl OutputWriter {
    pub fn write_marked_run(
        run: MarkedAnalysisRun,
        out: impl AsRef<Path>,
        options: &OutputSection,
    ) -> Result<OutputManifest> {
        Self::write(&ResultDocument::marked(run.result), out, options)
    }

    #[cfg(feature = "cli")]
    pub(crate) fn write_marked_run_with_manifest_context(
        run: MarkedAnalysisRun,
        out: impl AsRef<Path>,
        options: &OutputSection,
        manifest_context: RunManifestContext,
    ) -> Result<OutputManifest> {
        Self::write_with_manifest_context(
            &ResultDocument::marked(run.result),
            out,
            options,
            Some(manifest_context),
        )
    }
}
