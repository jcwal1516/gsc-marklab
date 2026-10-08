use std::path::Path;

use crate::{
    errors::Result,
    geom::mask::TumorMask,
    io::{pattern_builder::PatternBuilder, PatternLoadResult},
};

mod decoder;
mod schema;

pub fn load_pattern_csv_with_diagnostics(
    path: impl AsRef<Path>,
    mask: &TumorMask,
) -> Result<PatternLoadResult> {
    load_pattern_csv_with_builder(path.as_ref(), PatternBuilder::new(mask, "CSV"))
}

#[allow(dead_code, reason = "used by the feature-gated classical CLI adapter")]
pub(crate) fn load_classical_pattern_csv_with_diagnostics(
    path: impl AsRef<Path>,
    mask: &TumorMask,
) -> Result<PatternLoadResult> {
    load_pattern_csv_with_builder(path.as_ref(), PatternBuilder::new_classical(mask, "CSV"))
}

fn load_pattern_csv_with_builder(
    path: &Path,
    mut builder: PatternBuilder<'_>,
) -> Result<PatternLoadResult> {
    let decode_and_filter_span =
        tracing::info_span!("marklab_stage", stage_name = "decode_and_filter");
    let decode_and_filter_enter = decode_and_filter_span.enter();
    decoder::visit_decoded_rows(path, |row, row_number| builder.push(row, row_number))?;
    drop(decode_and_filter_enter);
    builder.finish()
}
