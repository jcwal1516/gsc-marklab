//! Closed result vocabularies serialized with their exact existing spellings.

/// Define a closed vocabulary whose variants serialize as the given wire spellings.
///
/// Every variant names its spelling explicitly, so renaming a Rust variant can never change
/// JSON, and spellings that `rename_all` cannot produce (such as `one_3d_point_pattern`) need
/// no special handling. The generated enum is `Copy`, comparable, hashable, and serde
/// round-trips only the listed spellings. Callers must depend on `serde`.
///
/// ```
/// marklab_core::vocabulary_enum! {
///     /// Whether a check ran.
///     pub enum CheckState {
///         /// The check ran to completion.
///         Complete => "complete",
///         NotRun => "not_run",
///     }
/// }
/// assert_eq!(CheckState::NotRun.as_str(), "not_run");
/// assert_eq!(serde_json::to_string(&CheckState::Complete).unwrap(), "\"complete\"");
/// ```
#[macro_export]
macro_rules! vocabulary_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($(#[$variant_meta:meta])* $variant:ident => $wire:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(
            Clone, Copy, Debug, Eq, Hash, PartialEq, ::serde::Deserialize, ::serde::Serialize,
        )]
        $vis enum $name {
            $(
                $(#[$variant_meta])*
                #[doc = concat!("Serialized as `", $wire, "`.")]
                #[serde(rename = $wire)]
                $variant,
            )+
        }

        impl $name {
            /// Exact spelling in existing JSON and tabular result formats.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire,)+
                }
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

mod null_hypothesis;
mod statistical_unit;
mod validation_status;

pub use null_hypothesis::NullHypothesis;
pub use statistical_unit::StatisticalUnit;
pub use validation_status::{ValidationStatus, ValidationSuiteStatus};
