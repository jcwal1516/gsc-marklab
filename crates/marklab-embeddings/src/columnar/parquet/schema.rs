use crate::columnar::parquet::format::{
    ColumnOrder, ConvertedType, FieldRepetitionType, LogicalType, SchemaElement, Type,
};

/// Whether each leaf column has an order that Marklab's writer produces: the order its type
/// defines, or, for `FLOAT` and `DOUBLE` columns, the IEEE 754 total order that parquet 60 writes
/// where parquet 56 wrote the type-defined order.
pub(super) fn has_writer_column_orders(schema: &[SchemaElement], orders: &[ColumnOrder]) -> bool {
    let leaves = schema.iter().filter_map(|element| element.type_);
    leaves.clone().count() == orders.len()
        && leaves.zip(orders).all(|(leaf, order)| match order {
            ColumnOrder::TYPEORDER(_) => true,
            ColumnOrder::IEEE754TOTALORDER(_) => matches!(leaf, Type::FLOAT | Type::DOUBLE),
        })
}

pub(super) fn is_required_utf8(element: &SchemaElement, name: &str) -> bool {
    element.type_ == Some(Type::BYTE_ARRAY)
        && element.type_length.is_none()
        && element.repetition_type == Some(FieldRepetitionType::REQUIRED)
        && element.name == name
        && element.num_children.is_none()
        && element.converted_type == Some(ConvertedType::UTF8)
        && element.scale.is_none()
        && element.precision.is_none()
        && element.field_id.is_none()
        && matches!(element.logical_type, Some(LogicalType::STRING(_)))
}

pub(super) fn is_flat_group(element: &SchemaElement, name: &str, children: i32) -> bool {
    element.type_.is_none()
        && element.type_length.is_none()
        && element.repetition_type.is_none()
        && element.name == name
        && element.num_children == Some(children)
        && element.converted_type.is_none()
        && element.scale.is_none()
        && element.precision.is_none()
        && element.field_id.is_none()
        && element.logical_type.is_none()
}

pub(super) fn is_unsigned_u64(
    element: &SchemaElement,
    name: &str,
    repetition: FieldRepetitionType,
) -> bool {
    element.type_ == Some(Type::INT64)
        && element.type_length.is_none()
        && element.repetition_type == Some(repetition)
        && element.name == name
        && element.num_children.is_none()
        && element.converted_type == Some(ConvertedType::UINT_64)
        && element.scale.is_none()
        && element.precision.is_none()
        && element.field_id.is_none()
        && matches!(
            element.logical_type,
            Some(LogicalType::INTEGER(ref integer))
                if integer.bit_width == 64 && !integer.is_signed
        )
}
