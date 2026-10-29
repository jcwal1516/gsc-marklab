//! Private wire codecs shared by direct and durable commands. File admission and execution stay with callers.

pub(crate) mod embedding_envelope;
pub(crate) mod graph_signal;
pub(crate) mod hierarchical_max_t;

pub(crate) mod embedding_kernel;
pub(crate) mod fingerprints;
pub(crate) mod multiscale_kernel;
