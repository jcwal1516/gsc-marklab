use std::{fs, path::Path};

use crate::{
    config::AnalysisConfig,
    data::Pattern,
    errors::{MmrspaceError, Result},
    periodogram::raster::centered_mark_raster,
};

pub fn write_analysis_intermediates(
    out: impl AsRef<Path>,
    pattern: &Pattern,
    config: &AnalysisConfig,
) -> Result<()> {
    let dir = out.as_ref().join("intermediates");
    fs::create_dir_all(&dir).map_err(|source| MmrspaceError::io(&dir, source))?;

    write_filtered_cells(&dir, pattern)?;
    write_kgrid(&dir, pattern, config)?;
    write_residual_raster(&dir, pattern)?;

    Ok(())
}

#[cfg(feature = "parquet")]
fn write_filtered_cells(dir: &Path, pattern: &Pattern) -> Result<()> {
    crate::io::parquet::write_pattern_parquet(pattern, dir.join("filtered_cells.parquet"))
}

#[cfg(not(feature = "parquet"))]
fn write_filtered_cells(_dir: &Path, _pattern: &Pattern) -> Result<()> {
    Err(MmrspaceError::Schema(
        "intermediate filtered_cells.parquet requires the parquet feature".into(),
    ))
}

#[cfg(feature = "parquet")]
fn write_kgrid(dir: &Path, pattern: &Pattern, config: &AnalysisConfig) -> Result<()> {
    use std::{fs::File, sync::Arc};

    use crate::spectra::kgrid::{resolvable_k_modes, KBand};

    use arrow::{
        array::{Float64Array, RecordBatch, UInt32Array},
        datatypes::{DataType, Field, Schema},
    };
    use parquet::arrow::arrow_writer::ArrowWriter;

    let modes = KBand::from_window(pattern.window.l_eff_um, pattern.window.d_nn_mean_um)
        .map(|band| resolvable_k_modes(band, config.spectrum.k_shells))
        .unwrap_or_default();
    let schema = Arc::new(Schema::new(vec![
        Field::new("kx", DataType::Float64, false),
        Field::new("ky", DataType::Float64, false),
        Field::new("k", DataType::Float64, false),
        Field::new("shell_index", DataType::UInt32, false),
    ]));
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            Arc::new(Float64Array::from(
                modes.iter().map(|mode| mode.kx).collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                modes.iter().map(|mode| mode.ky).collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                modes.iter().map(|mode| mode.k).collect::<Vec<_>>(),
            )),
            Arc::new(UInt32Array::from(
                modes
                    .iter()
                    .map(|mode| mode.shell_index as u32)
                    .collect::<Vec<_>>(),
            )),
        ],
    )
    .map_err(|err| MmrspaceError::Schema(err.to_string()))?;

    let path = dir.join("kgrid.parquet");
    let file = File::create(&path).map_err(|source| MmrspaceError::io(&path, source))?;
    let mut writer = ArrowWriter::try_new(file, schema, None)
        .map_err(|err| MmrspaceError::Schema(err.to_string()))?;
    writer
        .write(&batch)
        .map_err(|err| MmrspaceError::Schema(err.to_string()))?;
    writer
        .close()
        .map_err(|err| MmrspaceError::Schema(err.to_string()))?;
    Ok(())
}

#[cfg(not(feature = "parquet"))]
fn write_kgrid(_dir: &Path, _pattern: &Pattern, _config: &AnalysisConfig) -> Result<()> {
    Err(MmrspaceError::Schema(
        "intermediate kgrid.parquet requires the parquet feature".into(),
    ))
}

fn write_residual_raster(dir: &Path, pattern: &Pattern) -> Result<()> {
    let (spec, raster) = centered_mark_raster(pattern, pattern.window.d_nn_mean_um.max(1.0))
        .ok_or_else(|| MmrspaceError::Compute("could not build residual raster".into()))?;
    let bytes = npy_f32_2d(&raster, spec.height, spec.width);
    let path = dir.join("residual_raster.npy");
    fs::write(&path, bytes).map_err(|source| MmrspaceError::io(&path, source))?;
    Ok(())
}

fn npy_f32_2d(values: &[f32], rows: usize, cols: usize) -> Vec<u8> {
    let mut header =
        format!("{{'descr': '<f4', 'fortran_order': False, 'shape': ({rows}, {cols}), }}")
            .into_bytes();
    let padding = (16 - ((10 + header.len() + 1) % 16)) % 16;
    header.extend(std::iter::repeat_n(b' ', padding));
    header.push(b'\n');

    let mut bytes = Vec::with_capacity(10 + header.len() + values.len() * 4);
    bytes.extend_from_slice(b"\x93NUMPY");
    bytes.push(1);
    bytes.push(0);
    bytes.extend_from_slice(&(header.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&header);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}
