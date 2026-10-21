//! Point-pattern structure descriptors added at the Research tier, each with a reference
//! comparison: quadrat dispersion, ordinary kriging with held-out checks, bond-orientational
//! order, box-counting dimension with lacunarity of a declared mask, mutual information
//! between two cell marks with a spatial null, the Mapper graph of cell features, and the
//! Landy–Szalay two-point correlation against a declared random catalog, and the structure
//! factor with hyperuniformity indices of a fully observed rectangle.

pub(crate) mod bond_order;
pub(crate) mod hyperuniformity;
pub(crate) mod kriging;
pub(crate) mod lacunarity;
pub(crate) mod landy_szalay;
pub(crate) mod mapper;
pub(crate) mod morisita;
pub(crate) mod mutual_information;
pub(crate) mod surrogates;
