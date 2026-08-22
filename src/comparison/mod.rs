pub(crate) mod curves;
#[cfg(any(feature = "cli", test))]
pub(crate) mod margin_assessment;
#[cfg(any(feature = "cli", test))]
pub(crate) mod pooled_bin_difference;
pub(crate) mod result;
