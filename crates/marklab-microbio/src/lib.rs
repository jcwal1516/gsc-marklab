#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! MALDI-TOF mass spectrum reading and preprocessing for Marklab microbiology workflows.
//!
//! Bruker flex-series acquisitions are read from their `fid` and `acqu` files and converted from
//! time of flight to m/z; spectra already in m/z are read from two-column text. Spectra are
//! preprocessed by an explicitly ordered list of steps. Each step follows the definition used by
//! the R package MALDIquant so results can be compared with it.

mod bruker;
mod error;
mod preprocess;
mod read;
mod spectrum;
mod text;

pub use bruker::{
    parse_bruker_flex, read_bruker_flex, BrukerAcquisition, BrukerByteOrder, BrukerTofMode,
    MAXIMUM_BRUKER_TIME_POINTS,
};
pub use error::MicrobioError;
pub use preprocess::{bin_spectrum, preprocess_spectrum, BinSpec, BinnedSpectrum, PreprocessStep};
pub use spectrum::MassSpectrum;
pub use text::{parse_text_spectrum, read_text_spectrum};
