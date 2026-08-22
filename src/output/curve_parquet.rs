use std::path::Path;

use crate::errors::{MarklabError, Result};

use super::result_types::MarkedPatternResult;

impl MarkedPatternResult {
    pub(super) fn write_spectra_parquet(&self, out: &Path) -> Result<()> {
        use std::{fs::File, sync::Arc};

        use arrow::{
            array::{Float64Array, RecordBatch},
            datatypes::{DataType, Field, Schema},
        };
        use parquet::arrow::arrow_writer::ArrowWriter;

        if self.spectrum_curve.is_empty() {
            return Ok(());
        }
        let points = &self.spectrum_curve;
        let schema = Arc::new(Schema::new(vec![
            Field::new("k", DataType::Float64, false),
            Field::new("observed_power", DataType::Float64, false),
            Field::new("median_permutation_power", DataType::Float64, false),
            Field::new("whitened_power", DataType::Float64, false),
            Field::new("lower_global_envelope", DataType::Float64, true),
            Field::new("upper_global_envelope", DataType::Float64, true),
        ]));
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![
                Arc::new(Float64Array::from(
                    points.iter().map(|point| point.k).collect::<Vec<_>>(),
                )),
                Arc::new(Float64Array::from(
                    points
                        .iter()
                        .map(|point| point.observed_power)
                        .collect::<Vec<_>>(),
                )),
                Arc::new(Float64Array::from(
                    points
                        .iter()
                        .map(|point| point.median_permutation_power)
                        .collect::<Vec<_>>(),
                )),
                Arc::new(Float64Array::from(
                    points
                        .iter()
                        .map(|point| point.whitened_power)
                        .collect::<Vec<_>>(),
                )),
                Arc::new(Float64Array::from(
                    points
                        .iter()
                        .map(|point| point.lower_global_envelope)
                        .collect::<Vec<_>>(),
                )),
                Arc::new(Float64Array::from(
                    points
                        .iter()
                        .map(|point| point.upper_global_envelope)
                        .collect::<Vec<_>>(),
                )),
            ],
        )
        .map_err(|err| MarklabError::Compute(err.to_string()))?;
        let path = out.join("spectra.parquet");
        let file = File::create(&path).map_err(|source| MarklabError::io(&path, source))?;
        let mut writer = ArrowWriter::try_new(file, schema, None)
            .map_err(|err| MarklabError::Compute(err.to_string()))?;
        writer
            .write(&batch)
            .map_err(|err| MarklabError::Compute(err.to_string()))?;
        writer
            .close()
            .map_err(|err| MarklabError::Compute(err.to_string()))?;
        Ok(())
    }

    pub(super) fn write_mark_pair_covariance_parquet(&self, out: &Path) -> Result<()> {
        use std::{fs::File, sync::Arc};

        use arrow::{
            array::{Float64Array, RecordBatch, UInt64Array},
            datatypes::{DataType, Field, Schema},
        };
        use parquet::arrow::arrow_writer::ArrowWriter;

        if self.mark_pair_covariance_curve.is_empty() {
            return Ok(());
        }
        let points = &self.mark_pair_covariance_curve;
        let schema = Arc::new(Schema::new(vec![
            Field::new("r_min_um", DataType::Float64, false),
            Field::new("r_max_um", DataType::Float64, false),
            Field::new("covariance", DataType::Float64, true),
            Field::new("lower_global_envelope", DataType::Float64, true),
            Field::new("upper_global_envelope", DataType::Float64, true),
            Field::new("pair_count", DataType::UInt64, false),
        ]));
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![
                Arc::new(Float64Array::from(
                    points
                        .iter()
                        .map(|point| point.r_min_um)
                        .collect::<Vec<_>>(),
                )),
                Arc::new(Float64Array::from(
                    points
                        .iter()
                        .map(|point| point.r_max_um)
                        .collect::<Vec<_>>(),
                )),
                Arc::new(Float64Array::from(
                    points
                        .iter()
                        .map(|point| point.covariance)
                        .collect::<Vec<_>>(),
                )),
                Arc::new(Float64Array::from(
                    points
                        .iter()
                        .map(|point| point.lower_global_envelope)
                        .collect::<Vec<_>>(),
                )),
                Arc::new(Float64Array::from(
                    points
                        .iter()
                        .map(|point| point.upper_global_envelope)
                        .collect::<Vec<_>>(),
                )),
                Arc::new(UInt64Array::from(
                    points
                        .iter()
                        .map(|point| point.pair_count as u64)
                        .collect::<Vec<_>>(),
                )),
            ],
        )
        .map_err(|err| MarklabError::Compute(err.to_string()))?;
        let path = out.join("mark_pair_covariance.parquet");
        let file = File::create(&path).map_err(|source| MarklabError::io(&path, source))?;
        let mut writer = ArrowWriter::try_new(file, schema, None)
            .map_err(|err| MarklabError::Compute(err.to_string()))?;
        writer
            .write(&batch)
            .map_err(|err| MarklabError::Compute(err.to_string()))?;
        writer
            .close()
            .map_err(|err| MarklabError::Compute(err.to_string()))?;
        Ok(())
    }

    pub(super) fn write_scale_energy_parquet(&self, out: &Path) -> Result<()> {
        use std::{fs::File, sync::Arc};

        use arrow::{
            array::{Float64Array, RecordBatch, StringArray},
            datatypes::{DataType, Field, Schema},
        };
        use parquet::arrow::arrow_writer::ArrowWriter;

        if self.scale_energy_curve.is_empty() {
            return Ok(());
        }
        let points = &self.scale_energy_curve;
        let schema = Arc::new(Schema::new(vec![
            Field::new("band", DataType::Utf8, false),
            Field::new("scale_um", DataType::Float64, false),
            Field::new("energy_fraction", DataType::Float64, false),
            Field::new("lower_global_envelope", DataType::Float64, true),
            Field::new("upper_global_envelope", DataType::Float64, true),
        ]));
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![
                Arc::new(StringArray::from(
                    points
                        .iter()
                        .map(|point| point.band.as_str())
                        .collect::<Vec<_>>(),
                )),
                Arc::new(Float64Array::from(
                    points
                        .iter()
                        .map(|point| point.scale_um)
                        .collect::<Vec<_>>(),
                )),
                Arc::new(Float64Array::from(
                    points
                        .iter()
                        .map(|point| point.energy_fraction)
                        .collect::<Vec<_>>(),
                )),
                Arc::new(Float64Array::from(
                    points
                        .iter()
                        .map(|point| point.lower_global_envelope)
                        .collect::<Vec<_>>(),
                )),
                Arc::new(Float64Array::from(
                    points
                        .iter()
                        .map(|point| point.upper_global_envelope)
                        .collect::<Vec<_>>(),
                )),
            ],
        )
        .map_err(|err| MarklabError::Compute(err.to_string()))?;
        let path = out.join("scale_energy.parquet");
        let file = File::create(&path).map_err(|source| MarklabError::io(&path, source))?;
        let mut writer = ArrowWriter::try_new(file, schema, None)
            .map_err(|err| MarklabError::Compute(err.to_string()))?;
        writer
            .write(&batch)
            .map_err(|err| MarklabError::Compute(err.to_string()))?;
        writer
            .close()
            .map_err(|err| MarklabError::Compute(err.to_string()))?;
        Ok(())
    }
}
