use marklab_data::{CellId, CoordinateFrameId, MeasurementStatus, SlideId};
use marklab_embeddings::{CellEmbeddingArtifact, CellEmbeddingTable};
use marklab_workflow::{ArtifactId, ArtifactRef, ContentDigest, MarklabProject};

use crate::data::Pattern;

use super::identity::cell_ids_identity;
use super::{
    declaration::{
        BinaryMarkDeclaration, BinaryMarkOrigin, HistologicCompartmentMarkDeclaration,
        NucleusAreaUm2MarkDeclaration, OrdinalMarkDeclaration, ProbabilityMarkDeclaration,
        ProbabilitySimplexMarkDeclaration, VectorArtifactRefMarkDeclaration,
    },
    provenance::{
        validate_histologic_compartment_provenance, validate_nucleus_area_um2_provenance,
        validate_provenance,
    },
    DeclaredScalarInputError, ScalarMarkId,
};

mod identity;

mod artifact;
mod assay;
mod column;
mod mark_table;
mod validation;

pub use assay::{AssayMarkDeclaration, AssayMarkValues};
pub use column::{MissingnessPolicy, ScalarMarkColumn, ScalarMarkModality, ScalarMarkUnit};
pub use mark_table::MarkTable;

use super::provenance;
use column::ScalarMarkColumnValues;
