mod declaration;
mod error;
mod identity;
mod input;
mod provenance;

pub use declaration::{
    BinaryMarkDeclaration, BinaryMarkOrigin, DeclaredMarkUse, NucleusAreaUm2MarkDeclaration,
    ProbabilityMarkDeclaration, ProbabilityThresholdComparator, ScalarMarkId, ScalarMarkValueKind,
};
pub use error::DeclaredScalarInputError;
pub use identity::DeclaredScalarIdentity;
pub use input::DeclaredScalarPatternInput;
pub(crate) use provenance::validate_nucleus_area_um2_provenance;
