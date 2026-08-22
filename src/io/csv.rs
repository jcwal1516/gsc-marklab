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
    let mut builder = PatternBuilder::new(mask, "CSV");
    let mask_filter_span = tracing::info_span!("marklab_stage", stage_name = "mask_filter");
    let mask_filter_enter = mask_filter_span.enter();
    for (index, row) in decoder::read_rows(path.as_ref())?.into_iter().enumerate() {
        builder.push(row, index + 2)?;
    }
    drop(mask_filter_enter);
    builder.finish()
}
