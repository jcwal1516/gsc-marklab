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
    let decode_and_filter_span =
        tracing::info_span!("marklab_stage", stage_name = "decode_and_filter");
    let decode_and_filter_enter = decode_and_filter_span.enter();
    decoder::visit_decoded_rows(path.as_ref(), |row, row_number| {
        builder.push(row, row_number)
    })?;
    drop(decode_and_filter_enter);
    builder.finish()
}
