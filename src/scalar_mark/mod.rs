mod declaration;
mod error;
mod identity;
mod input;
mod provenance;
mod table;

pub(crate) use declaration::COMPARTMENT_MARK_ID;
pub use declaration::{
    BinaryMarkDeclaration, BinaryMarkOrigin, DeclaredMarkUse, HistologicCompartmentMarkDeclaration,
    NucleusAreaUm2MarkDeclaration, OrdinalMarkDeclaration, ProbabilityMarkDeclaration,
    ProbabilitySimplexMarkDeclaration, ProbabilityThresholdComparator, ScalarMarkId,
    ScalarMarkValueKind, VectorArtifactRefMarkDeclaration,
};
pub use error::DeclaredScalarInputError;
pub use identity::DeclaredScalarIdentity;
pub(crate) use input::validate_pattern_probabilities;
pub use input::DeclaredScalarPatternInput;
pub(crate) use provenance::validate_nucleus_area_um2_provenance;
pub use table::{
    AssayMarkDeclaration, AssayMarkValues, MarkTable, MissingnessPolicy, ScalarMarkColumn,
    ScalarMarkModality, ScalarMarkUnit,
};
