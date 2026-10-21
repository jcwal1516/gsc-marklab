use std::mem::size_of;

use marklab_project::ArtifactId;

use crate::{
    multiscale::{
        physical::SpatialPhysicalEncoding, MatrixSummaryAccumulator, MultiscaleEmbeddingQcSummary,
    },
    EmbeddingStatus,
};

use super::ColumnarError;

pub(crate) use crate::multiscale::MultiscaleMatrixTable;

pub(crate) const MATRIX_METADATA_KEYS: [&str; 9] = [
    "marklab.encoding_version",
    "marklab.entity_kind",
    "marklab.expected_entities_artifact_id",
    "marklab.logical_digest",
    "marklab.owning_slide_id",
    "marklab.provenance_artifact_id",
    "marklab.schema_id",
    "marklab.schema_version",
    "marklab.support_artifact_id",
];

pub(crate) fn matrix_dependencies(
    table: &dyn MultiscaleMatrixTable,
) -> Result<[ArtifactId; 3], ColumnarError> {
    let mut dependencies = [
        table.expected_entities_artifact_id(),
        table.support_artifact_id(),
        table.provenance_artifact_id(),
    ];
    dependencies.sort_unstable();
    if dependencies.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    Ok(dependencies)
}

pub(crate) fn matrix_metadata(
    encoding: SpatialPhysicalEncoding,
    table: &dyn MultiscaleMatrixTable,
) -> Result<[(String, String); 9], ColumnarError> {
    validate_matrix_domain(table)?;
    let profile = table.profile();
    let values = [
        profile.encoding_version(encoding).to_owned(),
        table.entity_kind().wire_name().to_owned(),
        table.expected_entities_artifact_id().to_string(),
        table.logical_digest().to_string(),
        table.owning_slide_id().as_str().to_owned(),
        table.provenance_artifact_id().to_string(),
        profile.schema_id().to_owned(),
        "1".to_owned(),
        table.support_artifact_id().to_string(),
    ];
    Ok(std::array::from_fn(|index| {
        (
            MATRIX_METADATA_KEYS[index].to_owned(),
            values[index].clone(),
        )
    }))
}

pub(crate) fn validate_matrix_domain(
    table: &dyn MultiscaleMatrixTable,
) -> Result<(), ColumnarError> {
    if table.profile().entity_kind() != table.entity_kind()
        || table.dimension() == 0
        || table.dimension() > 65_536
        || table.row_count() > 100_000_000
    {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    matrix_dependencies(table)?;
    let mut accumulator = MatrixPhysicalAccumulator::new(table)?;
    for index in 0..table.row_count() {
        let row = table
            .row(index)
            .map_err(|_| ColumnarError::ArtifactBindingMismatch)?;
        accumulator.push(row.id(), row.status(), row.vector())?;
    }
    if accumulator.finish()? != table.qc_summary() {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

pub(crate) fn matrix_decoded_bytes(
    table: &dyn MultiscaleMatrixTable,
    start: usize,
    end: usize,
) -> Result<usize, ColumnarError> {
    if start > end || end > table.row_count() {
        return Err(ColumnarError::SizeOverflow);
    }
    let rows = end - start;
    if rows == 0 {
        return Ok(0);
    }
    let dimension = usize::try_from(table.dimension()).map_err(|_| ColumnarError::SizeOverflow)?;
    let components = rows
        .checked_mul(dimension)
        .ok_or(ColumnarError::SizeOverflow)?;
    let (identifier_bytes, status_bytes) = (start..end).try_fold(
        (0_usize, 0_usize),
        |(identifier_bytes, status_bytes), index| {
            let row = table
                .row(index)
                .map_err(|_| ColumnarError::ArtifactBindingMismatch)?;
            Ok::<_, ColumnarError>((
                identifier_bytes
                    .checked_add(row.id().len())
                    .ok_or(ColumnarError::SizeOverflow)?,
                status_bytes
                    .checked_add(row.status().wire_name().len())
                    .ok_or(ColumnarError::SizeOverflow)?,
            ))
        },
    )?;
    let row_validity = bitmap_bytes(rows)?;
    let component_validity = bitmap_bytes(components)?;
    let offsets = rows
        .checked_add(1)
        .and_then(|value| value.checked_mul(2 * size_of::<i32>()))
        .ok_or(ColumnarError::SizeOverflow)?;
    let values = components
        .checked_mul(size_of::<f32>())
        .ok_or(ColumnarError::SizeOverflow)?;
    row_validity
        .checked_mul(3)
        .and_then(|value| value.checked_add(component_validity))
        .and_then(|value| value.checked_add(offsets))
        .and_then(|value| value.checked_add(identifier_bytes))
        .and_then(|value| value.checked_add(status_bytes))
        .and_then(|value| value.checked_add(values))
        .ok_or(ColumnarError::SizeOverflow)
}

fn bitmap_bytes(values: usize) -> Result<usize, ColumnarError> {
    values
        .checked_add(7)
        .map(|value| value / 8)
        .ok_or(ColumnarError::SizeOverflow)
}

pub(crate) struct MatrixPhysicalAccumulator(MatrixSummaryAccumulator);

impl MatrixPhysicalAccumulator {
    pub(crate) fn new(table: &dyn MultiscaleMatrixTable) -> Result<Self, ColumnarError> {
        MatrixSummaryAccumulator::new(
            table.entity_kind(),
            table.profile().logical_domain(),
            table.owning_slide_id(),
            table.expected_entities_artifact_id(),
            table.expected_entities_logical_digest(),
            table.support_artifact_id(),
            table.support_logical_digest(),
            table.provenance_artifact_id(),
            table.provenance_logical_digest(),
            table.dimension(),
            table.row_count(),
        )
        .map(Self)
        .map_err(|_| ColumnarError::ArtifactBindingMismatch)
    }

    pub(crate) fn push(
        &mut self,
        id: &str,
        status: EmbeddingStatus,
        vector: Option<&[f32]>,
    ) -> Result<(), ColumnarError> {
        self.0
            .push(id, status, vector)
            .map_err(|_| ColumnarError::ArtifactBindingMismatch)
    }

    pub(crate) fn finish(self) -> Result<MultiscaleEmbeddingQcSummary, ColumnarError> {
        self.0
            .finish()
            .map_err(|_| ColumnarError::ArtifactBindingMismatch)
    }
}

pub(crate) fn matrix_schema_fields(
    profile: super::MatrixPhysicalProfile,
    dimension: u32,
    encoding: SpatialPhysicalEncoding,
) -> Result<Vec<arrow::datatypes::Field>, ColumnarError> {
    use arrow::datatypes::{DataType, Field};
    let dimension = i32::try_from(dimension).map_err(|_| ColumnarError::SizeOverflow)?;
    let child_name = match encoding {
        SpatialPhysicalEncoding::Arrow => "item",
        SpatialPhysicalEncoding::Parquet => "element",
    };
    Ok(vec![
        Field::new(profile.id_column(), DataType::Utf8, false),
        Field::new(
            "embedding",
            DataType::FixedSizeList(
                std::sync::Arc::new(Field::new(child_name, DataType::Float32, false)),
                dimension,
            ),
            false,
        ),
        Field::new("embedding_status", DataType::Utf8, false),
    ])
}
