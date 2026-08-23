use std::{io::Cursor, io::Write};

use marklab_project::{ArtifactReadSeek, ContentDigest};

use super::{
    manifest::parse_reader as parse_manifest_reader, CellVitCsvSummary, CellVitNpySummary,
    ReconciliationFailure, SourceBundleBudgets, SourceBundleError,
};

const RECONCILIATION_DOMAIN: &[u8] = b"marklab-source-bundle-reconciliation-v1";
const MISSING_PROMOTION_FIELDS: [MissingPromotionField; 4] = [
    MissingPromotionField::CanonicalIdentityMapping,
    MissingPromotionField::InputNormalizationAndRunConfiguration,
    MissingPromotionField::ReviewedSourceSnapshot,
    MissingPromotionField::LicenseRecord,
];

/// Closed ordered promotion prerequisites intentionally absent from source reconciliation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingPromotionField {
    /// Reviewed source-local identifier to canonical `CellId` mapping.
    CanonicalIdentityMapping,
    /// Complete input-normalization and inference-run configuration.
    InputNormalizationAndRunConfiguration,
    /// Reviewed source snapshot without generated-content ambiguity.
    ReviewedSourceSnapshot,
    /// Explicit reviewed license record.
    LicenseRecord,
}

impl MissingPromotionField {
    fn wire_name(self) -> &'static str {
        match self {
            Self::CanonicalIdentityMapping => "canonical_identity_mapping",
            Self::InputNormalizationAndRunConfiguration => {
                "input_normalization_and_run_configuration"
            }
            Self::ReviewedSourceSnapshot => "reviewed_source_snapshot",
            Self::LicenseRecord => "license_record",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct BundleEvidence {
    npy_digest: ContentDigest,
    csv_digest: ContentDigest,
    manifest_digest: ContentDigest,
    rows: u64,
    dimension: u32,
}

/// Bounded accumulator for aggregate-only, non-promotable source evidence.
#[derive(Debug)]
pub struct SourceBundleReconciler {
    budgets: SourceBundleBudgets,
    maximum_bundles: u32,
    bundles: Vec<BundleEvidence>,
}

impl SourceBundleReconciler {
    /// Create an empty accumulator with explicit per-bundle and bundle-count maxima.
    pub fn new(budgets: SourceBundleBudgets, maximum_bundles: u32) -> Self {
        Self {
            budgets,
            maximum_bundles,
            bundles: Vec::new(),
        }
    }

    /// Validate and add one complete borrowed NPY/CSV/manifest bundle.
    pub fn push_bytes(
        &mut self,
        npy_bytes: &[u8],
        csv_bytes: &[u8],
        manifest_bytes: &[u8],
    ) -> Result<(), SourceBundleError> {
        let mut npy = Cursor::new(npy_bytes);
        let mut csv = Cursor::new(csv_bytes);
        let mut manifest = Cursor::new(manifest_bytes);
        self.push_readers(&mut npy, &mut csv, &mut manifest)
    }

    /// Validate and add one bundle from bounded seekable readers.
    pub fn push_readers(
        &mut self,
        npy_reader: &mut dyn ArtifactReadSeek,
        csv_reader: &mut dyn ArtifactReadSeek,
        manifest_reader: &mut dyn ArtifactReadSeek,
    ) -> Result<(), SourceBundleError> {
        let next_count = self
            .bundles
            .len()
            .checked_add(1)
            .ok_or(SourceBundleError::SizeOverflow)?;
        if next_count
            > usize::try_from(self.maximum_bundles).map_err(|_| SourceBundleError::SizeOverflow)?
        {
            return Err(reconciliation_error(
                ReconciliationFailure::BundleLimitExceeded,
            ));
        }
        let retained = next_count
            .checked_mul(size_of::<BundleEvidence>())
            .ok_or(SourceBundleError::SizeOverflow)?;
        if retained > self.budgets.maximum_retained_bytes() {
            return Err(SourceBundleError::RetainedByteBudgetExceeded {
                required: retained,
                maximum: self.budgets.maximum_retained_bytes(),
            });
        }
        let accumulator_bytes = self
            .bundles
            .len()
            .checked_mul(size_of::<BundleEvidence>())
            .ok_or(SourceBundleError::SizeOverflow)?;
        let parser_budgets = remaining_budgets(self.budgets, accumulator_bytes)?;
        let csv = CellVitCsvSummary::from_reader(csv_reader, parser_budgets)
            .map_err(|error| compose_retained_error(error, accumulator_bytes, self.budgets))?;
        let npy = CellVitNpySummary::from_reader(npy_reader, parser_budgets)
            .map_err(|error| compose_retained_error(error, accumulator_bytes, self.budgets))?;
        if npy.row_count() != csv.row_count() {
            return Err(SourceBundleError::RowCountMismatch {
                npy: npy.row_count(),
                csv: csv.row_count(),
            });
        }
        let manifest = parse_manifest_reader(manifest_reader, npy.row_count(), parser_budgets)
            .map_err(|error| compose_retained_error(error, accumulator_bytes, self.budgets))?;
        let evidence = BundleEvidence {
            npy_digest: npy.content_digest(),
            csv_digest: csv.content_digest(),
            manifest_digest: manifest.content_digest,
            rows: npy.row_count(),
            dimension: npy.dimension(),
        };
        if self.bundles.contains(&evidence) {
            return Err(reconciliation_error(ReconciliationFailure::DuplicateBundle));
        }
        self.bundles
            .try_reserve_exact(1)
            .map_err(|_| SourceBundleError::AllocationFailed {
                requested: retained,
            })?;
        self.bundles.push(evidence);
        Ok(())
    }

    /// Finish deterministic aggregation and discard all per-bundle evidence tuples.
    pub fn finish(mut self) -> Result<SourceBundleReconciliation, SourceBundleError> {
        if self.bundles.is_empty() {
            return Err(reconciliation_error(ReconciliationFailure::Empty));
        }
        self.bundles.sort_unstable();
        if self.bundles.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(reconciliation_error(ReconciliationFailure::DuplicateBundle));
        }
        let dimension = self.bundles[0].dimension;
        if self
            .bundles
            .iter()
            .any(|bundle| bundle.dimension != dimension)
        {
            return Err(reconciliation_error(
                ReconciliationFailure::DimensionMismatch,
            ));
        }
        let row_count = self.bundles.iter().try_fold(0_u64, |total, bundle| {
            total
                .checked_add(bundle.rows)
                .ok_or(SourceBundleError::SizeOverflow)
        })?;
        let bundle_count =
            u64::try_from(self.bundles.len()).map_err(|_| SourceBundleError::SizeOverflow)?;
        let aggregate_content_digest = aggregate_digest(&self.bundles)?;
        let reconciliation_digest = reconciliation_digest(
            aggregate_content_digest,
            bundle_count,
            row_count,
            dimension,
            row_count,
            row_count,
        );
        Ok(SourceBundleReconciliation {
            bundle_count,
            row_count,
            dimension,
            present_count: row_count,
            qc_pass_count: row_count,
            aggregate_content_digest,
            reconciliation_digest,
            missing_promotion_fields: MISSING_PROMOTION_FIELDS,
        })
    }
}

fn remaining_budgets(
    budgets: SourceBundleBudgets,
    retained: usize,
) -> Result<SourceBundleBudgets, SourceBundleError> {
    let remaining = budgets
        .maximum_retained_bytes()
        .checked_sub(retained)
        .ok_or(SourceBundleError::RetainedByteBudgetExceeded {
            required: retained,
            maximum: budgets.maximum_retained_bytes(),
        })?;
    Ok(budgets.with_maximum_retained_bytes(remaining))
}

fn compose_retained_error(
    error: SourceBundleError,
    retained: usize,
    budgets: SourceBundleBudgets,
) -> SourceBundleError {
    match error {
        SourceBundleError::RetainedByteBudgetExceeded { required, .. } => {
            match retained.checked_add(required) {
                Some(required) => SourceBundleError::RetainedByteBudgetExceeded {
                    required,
                    maximum: budgets.maximum_retained_bytes(),
                },
                None => SourceBundleError::SizeOverflow,
            }
        }
        error => error,
    }
}

/// Aggregate-only report that cannot publish or construct project artifacts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceBundleReconciliation {
    bundle_count: u64,
    row_count: u64,
    dimension: u32,
    present_count: u64,
    qc_pass_count: u64,
    aggregate_content_digest: ContentDigest,
    reconciliation_digest: ContentDigest,
    missing_promotion_fields: [MissingPromotionField; 4],
}

impl SourceBundleReconciliation {
    /// Number of exact validated source bundles.
    pub fn bundle_count(self) -> u64 {
        self.bundle_count
    }

    /// Aggregate validated source rows.
    pub fn row_count(self) -> u64 {
        self.row_count
    }

    /// Common exact embedding dimension.
    pub fn dimension(self) -> u32 {
        self.dimension
    }

    /// Aggregate finite NPY rows.
    pub fn present_count(self) -> u64 {
        self.present_count
    }

    /// Aggregate rows satisfying the exact source QC predicate.
    pub fn qc_pass_count(self) -> u64 {
        self.qc_pass_count
    }

    /// Ledger-compatible unframed digest of sorted per-bundle identities and shapes.
    pub fn aggregate_content_digest(self) -> ContentDigest {
        self.aggregate_content_digest
    }

    /// Domain-framed reconciliation report identity.
    pub fn reconciliation_digest(self) -> ContentDigest {
        self.reconciliation_digest
    }

    /// Exact closed ordered prerequisites that continue to prohibit promotion.
    pub fn missing_promotion_fields(&self) -> &[MissingPromotionField] {
        &self.missing_promotion_fields
    }
}

fn aggregate_digest(bundles: &[BundleEvidence]) -> Result<ContentDigest, SourceBundleError> {
    let mut digest = ContentDigest::builder();
    for bundle in bundles {
        digest
            .write_all(bundle.npy_digest.as_bytes())
            .and_then(|()| digest.write_all(bundle.csv_digest.as_bytes()))
            .and_then(|()| digest.write_all(bundle.manifest_digest.as_bytes()))
            .and_then(|()| digest.write_all(&bundle.rows.to_be_bytes()))
            .and_then(|()| digest.write_all(&u64::from(bundle.dimension).to_be_bytes()))
            .map_err(|_| SourceBundleError::SizeOverflow)?;
    }
    Ok(digest.finish().0)
}

fn reconciliation_digest(
    aggregate_content_digest: ContentDigest,
    bundle_count: u64,
    row_count: u64,
    dimension: u32,
    present_count: u64,
    qc_pass_count: u64,
) -> ContentDigest {
    let bundle_count = bundle_count.to_be_bytes();
    let row_count = row_count.to_be_bytes();
    let dimension = dimension.to_be_bytes();
    let present_count = present_count.to_be_bytes();
    let qc_pass_count = qc_pass_count.to_be_bytes();
    ContentDigest::from_framed([
        RECONCILIATION_DOMAIN,
        aggregate_content_digest.as_bytes(),
        &bundle_count,
        &row_count,
        &dimension,
        &present_count,
        &qc_pass_count,
        MISSING_PROMOTION_FIELDS[0].wire_name().as_bytes(),
        MISSING_PROMOTION_FIELDS[1].wire_name().as_bytes(),
        MISSING_PROMOTION_FIELDS[2].wire_name().as_bytes(),
        MISSING_PROMOTION_FIELDS[3].wire_name().as_bytes(),
    ])
}

fn reconciliation_error(reason: ReconciliationFailure) -> SourceBundleError {
    SourceBundleError::Reconciliation { reason }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use marklab_project::ContentDigest;

    use super::reconciliation_digest;

    #[test]
    fn authorized_reconciliation_digest_is_pinned() {
        let aggregate = ContentDigest::from_str(
            "75335c9aca2ab167a823cfbcae6d6783482b1cffea71903ac03f61b8775113b6",
        )
        .expect("aggregate digest");
        assert_eq!(
            reconciliation_digest(aggregate, 32, 60_191, 1_280, 60_191, 60_191).to_string(),
            "e5aeb0a426a7f9f2b38d538c73dd867818a589e3ccbe1b1b955b4273b65996a8"
        );
    }
}
