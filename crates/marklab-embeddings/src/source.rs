#[cfg(feature = "csv")]
mod csv;
mod error;
#[cfg(feature = "csv")]
mod import;
#[cfg(feature = "csv")]
mod manifest;
mod npy;
#[cfg(feature = "csv")]
mod reconciliation;

#[cfg(feature = "csv")]
pub use csv::CellVitCsvSummary;
pub use error::{
    CellVitCsvField, CsvFailure, ImportFailure, ManifestFailure, NpyFailure, ReconciliationFailure,
    SourceBundleError, SourceFileKind, SourceIoFailure, SourceIoOperation,
};
#[cfg(feature = "csv")]
pub use import::{
    import_cellvit_he_bundle_bytes, import_cellvit_he_bundle_from_store,
    import_cellvit_he_bundle_readers, CellVitHeArtifactBindings, CellVitHeImportCandidate,
    CellVitHeImportRequest, ImportedCellVitHeBundle,
};
pub use npy::{CellVitNpyMatrix, CellVitNpySummary, NpyVersion};
#[cfg(feature = "csv")]
pub use reconciliation::{
    MissingPromotionField, SourceBundleReconciler, SourceBundleReconciliation,
};

fn enforce_retained(
    required: usize,
    budgets: SourceBundleBudgets,
) -> Result<(), SourceBundleError> {
    if required > budgets.maximum_retained_bytes() {
        return Err(SourceBundleError::RetainedByteBudgetExceeded {
            required,
            maximum: budgets.maximum_retained_bytes(),
        });
    }
    Ok(())
}

/// Explicit file, retained-memory, and decoded-value budgets for one source bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceBundleBudgets {
    maximum_npy_file_bytes: u64,
    maximum_csv_file_bytes: u64,
    maximum_manifest_file_bytes: u64,
    maximum_retained_bytes: usize,
    maximum_decoded_bytes: u64,
}

impl SourceBundleBudgets {
    /// Declare maxima for NPY, CSV, manifest, retained-memory, and decoded-value bytes.
    pub fn new(
        maximum_npy_file_bytes: u64,
        maximum_csv_file_bytes: u64,
        maximum_manifest_file_bytes: u64,
        maximum_retained_bytes: usize,
        maximum_decoded_bytes: u64,
    ) -> Self {
        Self {
            maximum_npy_file_bytes,
            maximum_csv_file_bytes,
            maximum_manifest_file_bytes,
            maximum_retained_bytes,
            maximum_decoded_bytes,
        }
    }

    /// Maximum accepted encoded NPY bytes.
    pub fn maximum_npy_file_bytes(self) -> u64 {
        self.maximum_npy_file_bytes
    }

    /// Maximum accepted encoded CSV bytes.
    pub fn maximum_csv_file_bytes(self) -> u64 {
        self.maximum_csv_file_bytes
    }

    /// Maximum accepted encoded reconciliation-manifest bytes.
    pub fn maximum_manifest_file_bytes(self) -> u64 {
        self.maximum_manifest_file_bytes
    }

    /// Maximum memory retained by parsed source metadata and imported values.
    pub fn maximum_retained_bytes(self) -> usize {
        self.maximum_retained_bytes
    }

    /// Maximum decoded vector bytes admitted from the NPY payload.
    pub fn maximum_decoded_bytes(self) -> u64 {
        self.maximum_decoded_bytes
    }

    #[cfg(feature = "csv")]
    pub(crate) fn with_maximum_retained_bytes(self, maximum_retained_bytes: usize) -> Self {
        Self {
            maximum_retained_bytes,
            ..self
        }
    }
}
