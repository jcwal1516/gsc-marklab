mod declaration;
mod error;
mod identity;
mod input;
mod provenance;
mod table;

pub use declaration::{
    BinaryMarkDeclaration, BinaryMarkOrigin, DeclaredMarkUse, HistologicCompartmentMarkDeclaration,
    NucleusAreaUm2MarkDeclaration, ProbabilityMarkDeclaration, ProbabilitySimplexMarkDeclaration,
    ProbabilityThresholdComparator, ScalarMarkId, ScalarMarkValueKind,
    VectorArtifactRefMarkDeclaration,
};
pub use error::DeclaredScalarInputError;
pub use identity::DeclaredScalarIdentity;
pub use input::DeclaredScalarPatternInput;
pub(crate) use provenance::validate_nucleus_area_um2_provenance;
pub use table::{
    MarkTable, MissingnessPolicy, ScalarMarkColumn, ScalarMarkModality, ScalarMarkUnit,
};
