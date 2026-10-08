use std::{collections::BTreeSet, mem::size_of};

use crate::errors::{MarklabError, Result};

use super::cells::{CellSection, FusedCell};

#[cfg(test)]
thread_local! {
    static PRIMARY_LABEL_ENCODING_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_primary_label_encoding_build_call_count() {
    PRIMARY_LABEL_ENCODING_BUILDS.set(0);
}

#[cfg(test)]
pub(crate) fn primary_label_encoding_build_call_count() -> usize {
    PRIMARY_LABEL_ENCODING_BUILDS.get()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PrimaryLabelId(u32);

impl PrimaryLabelId {
    pub(crate) const fn as_u32(self) -> u32 {
        self.0
    }

    pub(crate) const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

/// One deterministic compact encoding of every primary cell label in a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PrimaryLabelEncoding {
    ids: Box<[Option<PrimaryLabelId>]>,
    names: Box<[Box<str>]>,
}

impl PrimaryLabelEncoding {
    pub(crate) fn new(cells: &[FusedCell]) -> Result<Self> {
        #[cfg(test)]
        PRIMARY_LABEL_ENCODING_BUILDS.set(PRIMARY_LABEL_ENCODING_BUILDS.get() + 1);

        let names = cells
            .iter()
            .filter_map(primary_label)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(Box::<str>::from)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        if names.len() > u32::MAX as usize {
            return Err(MarklabError::Validation(
                "primary label count exceeds compact u32 identifier capacity".into(),
            ));
        }
        let ids = cells
            .iter()
            .map(|cell| {
                primary_label(cell)
                    .map(|label| {
                        names
                            .binary_search_by(|candidate| candidate.as_ref().cmp(label))
                            .expect("label catalog was built from the same cells")
                    })
                    .map(|index| {
                        u32::try_from(index)
                            .map(PrimaryLabelId)
                            .expect("label count was checked against u32 capacity")
                    })
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(Self { ids, names })
    }

    pub(crate) fn len(&self) -> usize {
        self.ids.len()
    }

    pub(crate) fn label_count(&self) -> usize {
        self.names.len()
    }

    pub(crate) fn total_name_bytes(&self) -> usize {
        self.names.iter().map(|name| name.len()).sum()
    }

    pub(crate) fn ids(&self) -> &[Option<PrimaryLabelId>] {
        &self.ids
    }

    pub(crate) fn id_at(&self, index: usize) -> Option<PrimaryLabelId> {
        self.ids.get(index).copied().flatten()
    }

    pub(crate) fn id_for(&self, label: &str) -> Option<PrimaryLabelId> {
        self.names
            .binary_search_by(|candidate| candidate.as_ref().cmp(label))
            .ok()
            .and_then(|index| u32::try_from(index).ok())
            .map(PrimaryLabelId)
    }

    pub(crate) fn name(&self, id: PrimaryLabelId) -> Option<&str> {
        self.names.get(id.0 as usize).map(AsRef::as_ref)
    }

    pub(crate) fn estimated_storage_bytes(&self) -> usize {
        self.ids
            .len()
            .saturating_mul(size_of::<Option<PrimaryLabelId>>())
            .saturating_add(self.names.len().saturating_mul(size_of::<Box<str>>()))
            .saturating_add(self.names.iter().map(|name| name.len()).sum::<usize>())
    }

    pub(crate) fn estimated_storage_upper_bound_for_cells(cells: &[FusedCell]) -> usize {
        cells
            .len()
            .saturating_mul(size_of::<Option<PrimaryLabelId>>())
            .saturating_add(
                cells
                    .iter()
                    .filter_map(primary_label)
                    .map(|label| size_of::<Box<str>>().saturating_add(label.len()))
                    .sum::<usize>(),
            )
    }
}

pub(crate) fn primary_label(cell: &FusedCell) -> Option<&str> {
    match cell.source_section {
        CellSection::Ihc => ihc_mmr_label(cell),
        CellSection::He => cell
            .cell_type
            .as_deref()
            .map(str::trim)
            .filter(|label| !label.is_empty()),
    }
}

fn ihc_mmr_label(cell: &FusedCell) -> Option<&'static str> {
    match cell.mmr_mark {
        Some(1) => Some("mmr_abnormal"),
        Some(0) => Some("mmr_retained"),
        _ => cell.mmr_probability.map(|probability| {
            if probability >= 0.5 {
                "mmr_abnormal"
            } else {
                "mmr_retained"
            }
        }),
    }
}
