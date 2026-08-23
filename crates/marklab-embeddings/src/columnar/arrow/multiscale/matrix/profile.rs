use std::{collections::HashMap, sync::Arc};

use arrow::datatypes::{DataType, Field, Schema};
use arrow_ipc::{Endianness, Precision, Type};

use crate::{
    columnar::{
        multiscale::{
            matrix_metadata, MatrixPhysicalProfile, MultiscaleMatrixTable, MATRIX_METADATA_KEYS,
        },
        MultiscaleColumnarError, SpatialArrowFailure,
    },
    EmbeddingEntityKind,
};

use super::super::profile::METADATA_LIMIT;

pub(super) const COLUMN_COUNT: usize = 3;
pub(super) const NODE_COUNT: usize = 4;
pub(super) const BUFFER_COUNT: usize = 9;

pub(super) fn arrow_failure(reason: SpatialArrowFailure) -> MultiscaleColumnarError {
    MultiscaleColumnarError::Arrow { reason }
}

pub(super) fn schema(table: &dyn MultiscaleMatrixTable) -> Result<Schema, MultiscaleColumnarError> {
    let dimension =
        i32::try_from(table.dimension()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    let fields = vec![
        Field::new(id_column(table.profile()), DataType::Utf8, false),
        Field::new(
            "embedding",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, false)),
                dimension,
            ),
            false,
        ),
        Field::new("embedding_status", DataType::Utf8, false),
    ];
    let metadata = matrix_metadata(
        crate::multiscale::physical::SpatialPhysicalEncoding::Arrow,
        table,
    )?
    .into_iter()
    .collect::<HashMap<_, _>>();
    Ok(Schema::new_with_metadata(fields, metadata))
}

pub(super) fn validate_flatbuffer_schema(
    observed: arrow_ipc::Schema<'_>,
    table: &dyn MultiscaleMatrixTable,
) -> Result<(), MultiscaleColumnarError> {
    if observed.endianness() != Endianness::Little
        || observed
            .features()
            .is_some_and(|features| !features.is_empty())
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidSchema));
    }
    validate_metadata(observed.custom_metadata(), table)?;
    let fields = observed
        .fields()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidSchema))?;
    if fields.len() != COLUMN_COUNT {
        return Err(arrow_failure(SpatialArrowFailure::InvalidSchema));
    }
    validate_utf8(fields.get(0), id_column(table.profile()))?;
    let expected_dimension =
        i32::try_from(table.dimension()).map_err(|_| MultiscaleColumnarError::SizeOverflow)?;
    let embedding = fields.get(1);
    if embedding.name() != Some("embedding")
        || embedding.nullable()
        || embedding.dictionary().is_some()
        || embedding
            .custom_metadata()
            .is_some_and(|metadata| !metadata.is_empty())
        || embedding.type_type() != Type::FixedSizeList
        || embedding
            .type_as_fixed_size_list()
            .is_none_or(|list| list.listSize() != expected_dimension)
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidSchema));
    }
    let children = embedding
        .children()
        .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidSchema))?;
    if children.len() != 1 {
        return Err(arrow_failure(SpatialArrowFailure::InvalidSchema));
    }
    let item = children.get(0);
    if item.name() != Some("item")
        || item.nullable()
        || item.dictionary().is_some()
        || item
            .custom_metadata()
            .is_some_and(|metadata| !metadata.is_empty())
        || item.children().is_some_and(|children| !children.is_empty())
        || item.type_type() != Type::FloatingPoint
        || item
            .type_as_floating_point()
            .is_none_or(|floating| floating.precision() != Precision::SINGLE)
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidSchema));
    }
    validate_utf8(fields.get(2), "embedding_status")
}

fn validate_utf8(field: arrow_ipc::Field<'_>, name: &str) -> Result<(), MultiscaleColumnarError> {
    if field.name() != Some(name)
        || field.nullable()
        || field.dictionary().is_some()
        || field
            .custom_metadata()
            .is_some_and(|metadata| !metadata.is_empty())
        || field
            .children()
            .is_some_and(|children| !children.is_empty())
        || field.type_type() != Type::Utf8
        || field.type_as_utf_8().is_none()
    {
        return Err(arrow_failure(SpatialArrowFailure::InvalidSchema));
    }
    Ok(())
}

fn validate_metadata(
    metadata: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<arrow_ipc::KeyValue<'_>>>,
    >,
    table: &dyn MultiscaleMatrixTable,
) -> Result<(), MultiscaleColumnarError> {
    let metadata =
        metadata.ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidApplicationMetadata))?;
    let expected = matrix_metadata(
        crate::multiscale::physical::SpatialPhysicalEncoding::Arrow,
        table,
    )?;
    if metadata.len() != MATRIX_METADATA_KEYS.len() || expected.len() != metadata.len() {
        return Err(arrow_failure(
            SpatialArrowFailure::InvalidApplicationMetadata,
        ));
    }
    let mut total = 0_usize;
    for (entry, (expected_key, expected_value)) in metadata.iter().zip(expected) {
        let key = entry
            .key()
            .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidApplicationMetadata))?;
        let value = entry
            .value()
            .ok_or_else(|| arrow_failure(SpatialArrowFailure::InvalidApplicationMetadata))?;
        total = total
            .checked_add(key.len())
            .and_then(|sum| sum.checked_add(value.len()))
            .ok_or(MultiscaleColumnarError::SizeOverflow)?;
        if key != expected_key || value != expected_value || value.is_empty() {
            return Err(arrow_failure(
                SpatialArrowFailure::InvalidApplicationMetadata,
            ));
        }
    }
    if total > METADATA_LIMIT {
        return Err(arrow_failure(
            SpatialArrowFailure::InvalidApplicationMetadata,
        ));
    }
    Ok(())
}

pub(super) fn id_column(profile: MatrixPhysicalProfile) -> &'static str {
    profile.id_column()
}

pub(super) fn entity_kind(profile: MatrixPhysicalProfile) -> EmbeddingEntityKind {
    profile.entity_kind()
}
