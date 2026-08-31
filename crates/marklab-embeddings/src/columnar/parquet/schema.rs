use parquet::format::{ConvertedType, FieldRepetitionType, LogicalType, SchemaElement, Type};

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
