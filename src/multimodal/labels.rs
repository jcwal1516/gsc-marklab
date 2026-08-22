use super::cells::{CellSection, FusedCell};

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
