mod preflight;
mod preflight_reader;
mod profile;
mod publication;
mod reader;
mod row_link;
mod writer;

pub use preflight::{preflight_cell_embedding_table_arrow_bytes, CellEmbeddingArrowPreflight};
pub use publication::{publish_cell_embedding_table_arrow, EmbeddingColumnarPublicationError};
pub use reader::{
    read_cell_embedding_table_arrow_bytes, read_cell_embedding_table_arrow_from_store,
};
pub use row_link::{
    preflight_cell_embedding_row_link_arrow_bytes, publish_cell_embedding_row_link_arrow,
    validate_cell_embedding_row_link_arrow_bytes,
    validate_cell_embedding_row_link_arrow_from_store, write_cell_embedding_row_link_arrow,
    CellEmbeddingRowLinkArrowPreflight,
};
pub use writer::write_cell_embedding_table_arrow;
pub(crate) use writer::{
    build_record_batch as build_embedding_record_batch,
    validate_table_bindings as validate_embedding_table_bindings,
};
