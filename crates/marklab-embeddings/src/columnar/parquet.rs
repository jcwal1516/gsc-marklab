mod compact;
mod levels;
mod multiscale;
mod preflight;
mod profile;
mod publication;
mod read;
mod reader;
mod row_link;
mod schema;
mod writer;

pub use multiscale::*;
pub use preflight::{preflight_cell_embedding_table_parquet_bytes, CellEmbeddingParquetPreflight};
pub use publication::publish_cell_embedding_table_parquet;
pub use reader::{
    read_cell_embedding_table_parquet_bytes, read_cell_embedding_table_parquet_from_store,
    scan_cell_embedding_table_parquet_bytes, scan_cell_embedding_table_parquet_from_store,
    verify_cell_embedding_table_parquet_bytes, verify_cell_embedding_table_parquet_from_store,
};
pub use row_link::{
    preflight_cell_embedding_row_link_parquet_bytes, publish_cell_embedding_row_link_parquet,
    validate_cell_embedding_row_link_parquet_bytes,
    validate_cell_embedding_row_link_parquet_from_store,
    verify_cell_embedding_row_link_parquet_bytes,
    verify_cell_embedding_row_link_parquet_from_store, write_cell_embedding_row_link_parquet,
    CellEmbeddingRowLinkParquetPreflight,
};
pub use writer::write_cell_embedding_table_parquet;

pub(crate) use read::*;

pub(crate) use multiscale::cell_patch::reader::{
    verify_cell_patch_assignment_table_parquet_reader, verify_cell_patch_edge_table_parquet_reader,
};
pub(crate) use multiscale::matrix::reader::{
    verify_patch_embedding_table_parquet_reader, verify_region_embedding_table_parquet_reader,
    verify_slide_embedding_table_parquet_reader,
};
pub(crate) use multiscale::patch_region::reader::verify_patch_region_link_parquet_reader;
pub(crate) use multiscale::reader::{
    verify_patch_footprint_set_parquet_reader, verify_patch_overlap_graph_parquet_reader,
};
pub(crate) use reader::{
    read_cell_embedding_table_parquet_reader, verify_cell_embedding_table_parquet_reader,
};
pub(crate) use row_link::reader::verify_cell_embedding_row_link_parquet_reader;
