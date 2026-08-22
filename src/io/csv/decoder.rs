use std::path::Path;

use crate::{
    errors::{MarklabError, Result},
    io::row::{ArtifactFlags, DecodedCellRow, InternalControlState, NonviableFlags},
};

use super::schema::CellRow;

pub(super) fn visit_decoded_rows(
    path: &Path,
    mut visit: impl FnMut(DecodedCellRow, usize) -> Result<()>,
) -> Result<()> {
    let mut reader =
        ::csv::Reader::from_path(path).map_err(|error| MarklabError::io(path, error.into()))?;
    let headers = reader
        .headers()
        .map_err(|error| MarklabError::Schema(format!("{} headers: {error}", path.display())))?
        .clone();
    let has_column = |name| headers.iter().any(|header| header == name);
    let has_internal_control = has_column("internal_control_local");
    let has_artifact = has_column("artifact");
    let has_edge_artifact = has_column("edge_artifact");
    let has_fold_artifact = has_column("fold_artifact");
    let has_necrosis = has_column("necrosis");
    let has_nonviable_therapy_effect = has_column("nonviable_therapy_effect");
    for (index, row) in reader.deserialize::<CellRow>().enumerate() {
        let row_number = index + 2;
        let row = row.map_err(|error| {
            MarklabError::Schema(format!("{} row {row_number}: {error}", path.display()))
        })?;
        visit(
            DecodedCellRow {
                x_um: row.x_um,
                y_um: row.y_um,
                mark: row.mark,
                mark_probability: row.mark_probability,
                tumor_probability: row.tumor_probability,
                nucleus_area_um2: row.nucleus_area_um2,
                case_id: row.case_id,
                timepoint: row.timepoint,
                protein: row.protein,
                internal_control: has_internal_control
                    .then(|| InternalControlState::from_optional(row.internal_control_local)),
                slide_id: row.slide_id,
                section_id: row.section_id,
                stain_batch: row.stain_batch,
                block_id: row.block_id,
                region_id: row.region_id,
                slide_region: row.slide_region,
                histologic_compartment: row.histologic_compartment,
                valid_tumor: row.valid_tumor,
                valid_ihc: row.valid_ihc,
                artifact_flags: ArtifactFlags::new(
                    has_artifact.then_some(row.artifact.unwrap_or(false)),
                    has_edge_artifact.then_some(row.edge_artifact.unwrap_or(false)),
                    has_fold_artifact.then_some(row.fold_artifact.unwrap_or(false)),
                ),
                nonviable_flags: NonviableFlags::new(
                    has_necrosis.then_some(row.necrosis.unwrap_or(false)),
                    has_nonviable_therapy_effect
                        .then_some(row.nonviable_therapy_effect.unwrap_or(false)),
                ),
                qc_bin: row.qc_bin,
                component_id: row.component_id,
                local_dab_od: row.local_dab_od,
                local_hematoxylin_od: row.local_hematoxylin_od,
            },
            row_number,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::errors::MarklabError;

    use super::visit_decoded_rows;

    #[test]
    fn csv_rows_are_visited_before_later_rows_are_decoded() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let path = dir.path().join("cells.csv");
        fs::write(
            &path,
            "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n\
0,0,1,case,post,MSH6,true,true\n\
not-a-number,1,0,case,post,MSH6,true,true\n",
        )
        .expect("write fixture");

        let error = visit_decoded_rows(&path, |_row, _row_number| {
            Err(MarklabError::Compute("stop after first row".into()))
        })
        .expect_err("visitor should stop decoding");

        assert!(
            matches!(error, MarklabError::Compute(message) if message == "stop after first row")
        );
    }
}
