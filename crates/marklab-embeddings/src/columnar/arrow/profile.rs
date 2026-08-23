use std::{collections::HashMap, sync::Arc};

use arrow::datatypes::{DataType, Field, Schema};
use arrow_ipc::{Endianness, Precision, Type};
use flatbuffers::VerifierOptions;

use super::super::{
    ArrowIpcFailure, CellEmbeddingTablePhysicalBindings, EmbeddingColumnarBudgets,
    EmbeddingColumnarError,
};

pub(super) const ARROW_MAGIC: &[u8; 6] = b"ARROW1";
pub(super) const CONTINUATION_MARKER: &[u8; 4] = &[0xff; 4];
pub(super) const ALIGNMENT: usize = 64;
pub(super) const HEADER_BYTES: usize = ALIGNMENT;
pub(super) const EOS_BYTES: usize = 8;
pub(super) const TRAILER_BYTES: usize = 10;
pub(super) const MAXIMUM_FOOTER_BYTES: usize = 1024 * 1024;
pub(super) const MAXIMUM_MESSAGE_BYTES: usize = 64 * 1024;
const MAXIMUM_APPLICATION_METADATA_BYTES: usize = 64 * 1024;
const MAXIMUM_ROWS: usize = 100_000_000;
pub(super) const MAXIMUM_DIMENSION: u32 = 65_536;
pub(super) const RECORD_BATCH_ROWS: usize = 8_192;

pub(super) const SCHEMA_ID: &str = "marklab.cell_embedding_table";
const SCHEMA_VERSION: &str = "1";
pub(super) const ENCODING_VERSION: &str = "marklab.arrow-ipc.embedding-table.v1";

pub(super) const METADATA_KEYS: [&str; 7] = [
    "marklab.encoding_version",
    "marklab.expected_cells_artifact_id",
    "marklab.logical_digest",
    "marklab.provenance_artifact_id",
    "marklab.row_link_artifact_id",
    "marklab.schema_id",
    "marklab.schema_version",
];

pub(super) fn validity_bytes(bit_count: usize) -> Result<usize, EmbeddingColumnarError> {
    bit_count
        .checked_add(7)
        .map(|value| value / 8)
        .ok_or(EmbeddingColumnarError::SizeOverflow)
}

pub(super) fn validate_all_valid_bitmap(
    bytes: &[u8],
    _bit_count: usize,
) -> Result<(), EmbeddingColumnarError> {
    if bytes.iter().any(|byte| *byte != 0xff) {
        return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
    }
    Ok(())
}

pub(super) fn validate_flatbuffer_schema(
    schema: arrow_ipc::Schema<'_>,
    bindings: CellEmbeddingTablePhysicalBindings,
) -> Result<u32, EmbeddingColumnarError> {
    if schema.endianness() != Endianness::Little
        || schema
            .features()
            .is_some_and(|features| !features.is_empty())
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    validate_application_metadata(schema.custom_metadata(), bindings)?;
    let fields = schema
        .fields()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidSchema))?;
    if fields.len() != 3 {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    validate_utf8_field(fields.get(0), "cell_id")?;
    let embedding = fields.get(1);
    if embedding.name() != Some("embedding")
        || embedding.nullable()
        || embedding.dictionary().is_some()
        || embedding
            .custom_metadata()
            .is_some_and(|metadata| !metadata.is_empty())
        || embedding.type_type() != Type::FixedSizeList
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    let list = embedding
        .type_as_fixed_size_list()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidSchema))?;
    let dimension = list.listSize();
    if dimension <= 0 || dimension > MAXIMUM_DIMENSION as i32 {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    let children = embedding
        .children()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidSchema))?;
    if children.len() != 1 {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    let child = children.get(0);
    if child.name() != Some("item")
        || child.nullable()
        || child.dictionary().is_some()
        || child
            .custom_metadata()
            .is_some_and(|metadata| !metadata.is_empty())
        || child
            .children()
            .is_some_and(|children| !children.is_empty())
        || child.type_type() != Type::FloatingPoint
        || child
            .type_as_floating_point()
            .is_none_or(|floating| floating.precision() != Precision::SINGLE)
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    validate_utf8_field(fields.get(2), "embedding_status")?;
    u32::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)
}

pub(super) fn validate_utf8_field(
    field: arrow_ipc::Field<'_>,
    name: &str,
) -> Result<(), EmbeddingColumnarError> {
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
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    Ok(())
}

pub(super) fn validate_application_metadata(
    metadata: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<arrow_ipc::KeyValue<'_>>>,
    >,
    bindings: CellEmbeddingTablePhysicalBindings,
) -> Result<(), EmbeddingColumnarError> {
    let metadata =
        metadata.ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata))?;
    if metadata.len() != METADATA_KEYS.len() {
        return Err(arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata));
    }
    let expected_values = metadata_values(bindings);
    let mut total_bytes = 0_usize;
    for ((entry, expected_key), expected_value) in metadata
        .iter()
        .zip(METADATA_KEYS)
        .zip(expected_values.iter())
    {
        let key = entry
            .key()
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata))?;
        let value = entry
            .value()
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata))?;
        total_bytes = total_bytes
            .checked_add(key.len())
            .and_then(|total| total.checked_add(value.len()))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        if key != expected_key || value != expected_value {
            return Err(arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata));
        }
    }
    if total_bytes > MAXIMUM_APPLICATION_METADATA_BYTES {
        return Err(arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata));
    }
    Ok(())
}

pub(super) fn embedding_schema(
    dimension: u32,
    bindings: CellEmbeddingTablePhysicalBindings,
) -> Result<Schema, EmbeddingColumnarError> {
    let dimension = i32::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    let fields = vec![
        Field::new("cell_id", DataType::Utf8, false),
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
    let values = metadata_values(bindings);
    let metadata = METADATA_KEYS
        .iter()
        .zip(values)
        .map(|(key, value)| ((*key).to_owned(), value))
        .collect::<HashMap<_, _>>();
    Ok(Schema::new_with_metadata(fields, metadata))
}

pub(super) fn metadata_values(bindings: CellEmbeddingTablePhysicalBindings) -> [String; 7] {
    [
        ENCODING_VERSION.to_owned(),
        bindings.expected_cells_artifact_id().to_string(),
        bindings.table_logical_digest().to_string(),
        bindings.provenance_artifact_id().to_string(),
        bindings.row_link_artifact_id().to_string(),
        SCHEMA_ID.to_owned(),
        SCHEMA_VERSION.to_owned(),
    ]
}

pub(super) fn validate_shape(
    row_count: usize,
    dimension: u32,
) -> Result<(), EmbeddingColumnarError> {
    if row_count > MAXIMUM_ROWS {
        return Err(arrow_failure(ArrowIpcFailure::InvalidRowCount));
    }
    if dimension == 0 || dimension > MAXIMUM_DIMENSION {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    Ok(())
}

pub(super) fn component_bytes(
    row_count: usize,
    dimension: u32,
) -> Result<u64, EmbeddingColumnarError> {
    let rows = u64::try_from(row_count).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    rows.checked_mul(u64::from(dimension))
        .and_then(|value| value.checked_mul(size_of::<f32>() as u64))
        .ok_or(EmbeddingColumnarError::SizeOverflow)
}

pub(super) fn enforce_decoded_budget(
    required: u64,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), EmbeddingColumnarError> {
    if required > budgets.maximum_decoded_bytes() {
        return Err(EmbeddingColumnarError::DecodedByteBudgetExceeded {
            required,
            maximum: budgets.maximum_decoded_bytes(),
        });
    }
    Ok(())
}

pub(super) fn enforce_retained_budget(
    required: usize,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), EmbeddingColumnarError> {
    if required > budgets.maximum_retained_bytes() {
        return Err(EmbeddingColumnarError::RetainedByteBudgetExceeded {
            required,
            maximum: budgets.maximum_retained_bytes(),
        });
    }
    Ok(())
}

pub(super) fn enforce_row_group_budget(
    required: usize,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), EmbeddingColumnarError> {
    if required > budgets.maximum_row_group_bytes() {
        return Err(EmbeddingColumnarError::RowGroupByteBudgetExceeded {
            required,
            maximum: budgets.maximum_row_group_bytes(),
        });
    }
    Ok(())
}

pub(super) fn nonnegative_usize(
    value: i64,
    failure: ArrowIpcFailure,
) -> Result<usize, EmbeddingColumnarError> {
    if value < 0 {
        return Err(arrow_failure(failure));
    }
    usize::try_from(value).map_err(|_| arrow_failure(failure))
}

pub(super) fn nonnegative_i32_usize(
    value: i32,
    failure: ArrowIpcFailure,
) -> Result<usize, EmbeddingColumnarError> {
    if value < 0 {
        return Err(arrow_failure(failure));
    }
    usize::try_from(value).map_err(|_| arrow_failure(failure))
}

pub(super) fn align(value: usize) -> Result<usize, EmbeddingColumnarError> {
    value
        .checked_add(ALIGNMENT - 1)
        .map(|value| value & !(ALIGNMENT - 1))
        .ok_or(EmbeddingColumnarError::SizeOverflow)
}

pub(super) fn footer_verifier_options() -> VerifierOptions {
    VerifierOptions {
        max_depth: 8,
        max_tables: 32,
        max_apparent_size: MAXIMUM_FOOTER_BYTES,
        ignore_missing_null_terminator: false,
    }
}

pub(super) fn message_verifier_options() -> VerifierOptions {
    VerifierOptions {
        max_depth: 4,
        max_tables: 4,
        max_apparent_size: MAXIMUM_MESSAGE_BYTES,
        ignore_missing_null_terminator: false,
    }
}

pub(super) fn schema_message_verifier_options() -> VerifierOptions {
    VerifierOptions {
        max_depth: 8,
        max_tables: 32,
        max_apparent_size: MAXIMUM_MESSAGE_BYTES,
        ignore_missing_null_terminator: false,
    }
}

pub(super) fn arrow_failure(reason: ArrowIpcFailure) -> EmbeddingColumnarError {
    EmbeddingColumnarError::Arrow { reason }
}
