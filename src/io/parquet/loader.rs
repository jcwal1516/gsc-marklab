use std::{fs::File, path::Path};

use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use crate::{
    errors::{MarklabError, Result},
    geom::mask::TumorMask,
    io::{pattern_builder::PatternBuilder, PatternLoadResult},
};

pub fn load_pattern_parquet_with_diagnostics(
    path: impl AsRef<Path>,
    mask: &TumorMask,
) -> Result<PatternLoadResult> {
    load_pattern_parquet_with_builder(path.as_ref(), mask, false)
}

#[allow(dead_code, reason = "used by the feature-gated classical CLI adapter")]
pub(crate) fn load_classical_pattern_parquet_with_diagnostics(
    path: impl AsRef<Path>,
    mask: &TumorMask,
) -> Result<PatternLoadResult> {
    load_pattern_parquet_with_builder(path.as_ref(), mask, true)
}

fn load_pattern_parquet_with_builder(
    path: &Path,
    mask: &TumorMask,
    allow_sparse: bool,
) -> Result<PatternLoadResult> {
    let file = File::open(path).map_err(|source| MarklabError::io(path, source))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|error| MarklabError::Schema(error.to_string()))?;
    let reader = builder
        .build()
        .map_err(|error| MarklabError::Schema(error.to_string()))?;

    let mut pattern_builder = if allow_sparse {
        PatternBuilder::new_classical(mask, "Parquet")
    } else {
        PatternBuilder::new(mask, "Parquet")
    };
    let decode_and_filter_span =
        tracing::info_span!("marklab_stage", stage_name = "decode_and_filter");
    let decode_and_filter_enter = decode_and_filter_span.enter();
    let mut source_row = 0;
    for batch in reader {
        let batch = batch.map_err(|error| MarklabError::Schema(error.to_string()))?;
        let columns = super::schema::BatchColumns::try_new(&batch)?;
        for row_index in 0..batch.num_rows() {
            source_row += 1;
            pattern_builder.push(super::row::decode_cell_row(&columns, row_index), source_row)?;
        }
    }
    drop(decode_and_filter_enter);
    pattern_builder.finish()
}
