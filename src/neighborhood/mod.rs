pub mod cross_curves;
pub mod enrichment;
pub mod graph;
pub(crate) mod label_permutation;
pub mod profiles;
pub mod territories;

#[cfg(all(test, feature = "cli"))]
mod cross_curve_tests;
#[cfg(test)]
mod profile_tests;
#[cfg(test)]
mod territory_tests;
#[cfg(test)]
mod tests;
