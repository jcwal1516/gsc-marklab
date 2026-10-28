//! CSV tables written beside SPACE JSON results.

use marklab::space::{
    SpaceCensusResult, SpaceCisResult, SpaceDistResult, SpaceDistValues, SpaceDiversityResult,
    SpaceMapResult, SpaceNullStatus, SpacePatternResult, SpaceRankResult, SpaceRankView,
    SpaceSuggestResult, SpaceTransResult,
};
use marklab::MarklabError;

/// One row per disc; variables absent from a specimen are zero (R `standardize_censuses`).
/// Three-dimensional requests add `z_um`, empty for planar specimens.
pub(super) fn census_table(result: &SpaceCensusResult) -> marklab::Result<Vec<u8>> {
    let names = result
        .variables
        .iter()
        .map(|variable| variable.name.as_str())
        .filter(|name| {
            result
                .specimens
                .iter()
                .any(|specimen| specimen.variables.iter().any(|known| known == name))
        })
        .collect::<Vec<_>>();
    let depth = result.specimens.iter().any(|specimen| {
        specimen
            .radii
            .iter()
            .any(|radius| radius.discs.iter().any(|disc| disc.z_um.is_some()))
    });
    let mut writer = csv::Writer::from_writer(Vec::new());
    let mut header = vec!["specimen_id", "radius_um", "seed_cell_id", "x_um", "y_um"];
    if depth {
        header.push("z_um");
    }
    header.push("member_count");
    header.extend(&names);
    writer.write_record(&header)?;
    for specimen in &result.specimens {
        let columns = names
            .iter()
            .map(|name| specimen.variables.iter().position(|known| known == name))
            .collect::<Vec<_>>();
        for radius in &specimen.radii {
            for disc in &radius.discs {
                let mut record = vec![
                    specimen.specimen_id.clone(),
                    number(radius.radius_um),
                    disc.seed_cell_id.clone(),
                    number(disc.x_um),
                    number(disc.y_um),
                ];
                if depth {
                    record.push(disc.z_um.map(number).unwrap_or_default());
                }
                record.push(disc.member_count.to_string());
                record.extend(
                    columns.iter().map(|column| {
                        column.map_or_else(|| "0".into(), |c| number(disc.values[c]))
                    }),
                );
                writer.write_record(&record)?;
            }
        }
    }
    finish(writer)
}

pub(super) fn cis_table(result: &SpaceCisResult) -> marklab::Result<Vec<u8>> {
    let depth = result.parameters.depth;
    let mut writer = csv::Writer::from_writer(Vec::new());
    let mut header = vec!["specimen_id".to_owned(), "radius_um".into(), "depth".into()];
    header.extend((1..=depth).map(|position| format!("v{position}")));
    header.extend(
        [
            "observed_mi",
            "null_mean_mi",
            "null_sd_mi",
            "cis_mi",
            "z_score",
            "status",
            "p_value_normal",
            "p_adjust_bh",
            "p_value_monte_carlo",
        ]
        .map(String::from),
    );
    writer.write_record(&header)?;
    for specimen in &result.specimens {
        for radius in &specimen.radii {
            for ensemble in &radius.ensembles {
                let mut record = vec![
                    specimen.specimen_id.clone(),
                    number(radius.radius_um),
                    ensemble.variables.len().to_string(),
                ];
                record.extend((0..depth).map(|position| {
                    ensemble
                        .variables
                        .get(position)
                        .cloned()
                        .unwrap_or_default()
                }));
                record.extend([
                    number(ensemble.observed_mi),
                    number(ensemble.null_mean_mi),
                    number(ensemble.null_sd_mi),
                    number(ensemble.cis_mi),
                    ensemble.z_score.map(number).unwrap_or_default(),
                    status(ensemble.status),
                    number(ensemble.p_value_normal),
                    number(ensemble.p_adjust_bh),
                    number(ensemble.p_value_monte_carlo),
                ]);
                writer.write_record(&record)?;
            }
        }
    }
    finish(writer)
}

pub(super) fn trans_table(result: &SpaceTransResult) -> marklab::Result<Vec<u8>> {
    let depth = result.parameters.depth;
    let mut writer = csv::Writer::from_writer(Vec::new());
    let mut header = vec!["radius_um".to_owned(), "depth".into()];
    header.extend((1..=depth).map(|position| format!("v{position}")));
    header.extend(
        [
            "factor",
            "trans_mi",
            "null_mean",
            "null_sd",
            "z_score",
            "status",
            "p_value_normal",
            "p_adjust_bh",
            "p_value_monte_carlo",
        ]
        .map(String::from),
    );
    writer.write_record(&header)?;
    for radius in &result.radii {
        for ensemble in &radius.ensembles {
            for statistic in &ensemble.factors {
                let mut record = vec![
                    number(radius.radius_um),
                    ensemble.variables.len().to_string(),
                ];
                record.extend((0..depth).map(|position| {
                    ensemble
                        .variables
                        .get(position)
                        .cloned()
                        .unwrap_or_default()
                }));
                record.extend([
                    statistic.factor.clone(),
                    number(statistic.trans_mi),
                    number(statistic.null_mean),
                    number(statistic.null_sd),
                    number(statistic.z_score),
                    status(statistic.status),
                    number(statistic.p_value_normal),
                    number(statistic.p_adjust_bh),
                    number(statistic.p_value_monte_carlo),
                ]);
                writer.write_record(&record)?;
            }
        }
    }
    finish(writer)
}

fn status(status: SpaceNullStatus) -> String {
    match status {
        SpaceNullStatus::Available => "available".into(),
        SpaceNullStatus::DegenerateNull => "degenerate_null".into(),
    }
}

/// Shortest round-trip decimal, as in the JSON result.
fn number(value: f64) -> String {
    format!("{value:?}")
}

/// One row per disc, or per grid cell with its smoothed frequency (empty where removed).
pub(super) fn dist_table(result: &SpaceDistResult) -> marklab::Result<Vec<u8>> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    let mut header = result
        .variables
        .iter()
        .map(|variable| variable.name.clone())
        .collect::<Vec<_>>();
    match &result.values {
        SpaceDistValues::Discs { discs } => {
            writer.write_record(&header)?;
            for values in discs {
                writer.write_record(values.iter().copied().map(number))?;
            }
        }
        SpaceDistValues::Distribution { cells, .. } => {
            header.push("frequency".into());
            writer.write_record(&header)?;
            for cell in cells {
                let mut record = cell.values.iter().copied().map(number).collect::<Vec<_>>();
                record.push(cell.frequency.map(number).unwrap_or_default());
                writer.write_record(&record)?;
            }
        }
    }
    finish(writer)
}

/// The ranked ensembles of one radius, or one ensemble at every radius.
pub(super) fn rank_table(result: &SpaceRankResult) -> marklab::Result<Vec<u8>> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    let statistics = [
        "mi",
        "z_score",
        "p_value_normal",
        "p_adjust_bh",
        "p_value_monte_carlo",
    ];
    match &result.view {
        SpaceRankView::Ensembles { ensembles, .. } => {
            let depth = ensembles
                .iter()
                .map(|ensemble| ensemble.variables.len())
                .max()
                .unwrap_or(1);
            let mut header = vec!["rank".to_owned(), "depth".into()];
            header.extend((1..=depth).map(|position| format!("v{position}")));
            header.extend(statistics.map(String::from));
            writer.write_record(&header)?;
            for (rank, ensemble) in ensembles.iter().enumerate() {
                let mut record = vec![(rank + 1).to_string(), ensemble.variables.len().to_string()];
                record.extend((0..depth).map(|position| {
                    ensemble
                        .variables
                        .get(position)
                        .cloned()
                        .unwrap_or_default()
                }));
                record.extend([
                    number(ensemble.mi),
                    ensemble.z_score.map(number).unwrap_or_default(),
                    number(ensemble.p_value_normal),
                    number(ensemble.p_adjust_bh),
                    number(ensemble.p_value_monte_carlo),
                ]);
                writer.write_record(&record)?;
            }
        }
        SpaceRankView::Radii { radii, .. } => {
            let mut header = vec!["radius_um"];
            header.extend(statistics);
            writer.write_record(&header)?;
            for radius in radii {
                writer.write_record([
                    number(radius.radius_um),
                    number(radius.mi),
                    radius.z_score.map(number).unwrap_or_default(),
                    number(radius.p_value_normal),
                    number(radius.p_adjust_bh),
                    number(radius.p_value_monte_carlo),
                ])?;
            }
        }
    }
    finish(writer)
}

/// The ranked variables of one radius.
pub(super) fn rank_variable_table(result: &SpaceRankResult) -> marklab::Result<Vec<u8>> {
    let SpaceRankView::Ensembles { variables, .. } = &result.view else {
        return Err(MarklabError::Config(
            "a variable table needs a ranking of one radius".into(),
        ));
    };
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer.write_record([
        "rank",
        "variable",
        "score",
        "unbounded_ensembles",
        "ensembles",
    ])?;
    for (rank, variable) in variables.iter().enumerate() {
        writer.write_record([
            (rank + 1).to_string(),
            variable.name.clone(),
            number(variable.score),
            variable.unbounded_ensembles.to_string(),
            variable.ensembles.to_string(),
        ])?;
    }
    finish(writer)
}

fn finish(writer: csv::Writer<Vec<u8>>) -> marklab::Result<Vec<u8>> {
    writer
        .into_inner()
        .map_err(|error| MarklabError::Compute(format!("CSV buffer: {error}")))
}

/// One row per variable and path position, as R `learn_pattern` returns them.
pub(super) fn pattern_table(result: &SpacePatternResult) -> marklab::Result<Vec<u8>> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer.write_record([
        "position",
        "variable",
        "mean",
        "lower",
        "upper",
        "enrichment",
        "mean_normalized",
        "lower_normalized",
        "upper_normalized",
    ])?;
    for curve in &result.curves {
        for (place, position) in result.positions.iter().enumerate() {
            writer.write_record([
                number(*position),
                curve.variable.clone(),
                number(curve.mean[place]),
                number(curve.lower[place]),
                number(curve.upper[place]),
                number(result.enrichment[place]),
                number(curve.mean_normalized[place]),
                number(curve.lower_normalized[place]),
                number(curve.upper_normalized[place]),
            ])?;
        }
    }
    finish(writer)
}

/// One row per cell with its region, empty where no disc outvoted the background.
pub(super) fn map_table(result: &SpaceMapResult) -> marklab::Result<Vec<u8>> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer.write_record(["specimen_id", "cell_id", "region"])?;
    for specimen in &result.specimens {
        for (cell, region) in specimen.cell_ids.iter().zip(&specimen.regions) {
            writer.write_record([
                specimen.specimen_id.clone(),
                cell.clone(),
                region.map(|region| region.to_string()).unwrap_or_default(),
            ])?;
        }
    }
    finish(writer)
}

/// One row per specimen, with an empty parent, then one per parent; constituent shares in
/// percent. A specimen row's divergence is its beta diversity.
pub(super) fn diversity_table(result: &SpaceDiversityResult) -> marklab::Result<Vec<u8>> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    let mut header = vec![
        "specimen_id",
        "parent",
        "cell_count",
        "alpha_bits",
        "divergence_bits",
    ];
    header.extend(result.constituents.iter().map(String::as_str));
    writer.write_record(&header)?;
    let optional = |value: Option<f64>| value.map(|value| value.to_string()).unwrap_or_default();
    for specimen in &result.specimens {
        let mut record = vec![
            specimen.specimen_id.clone(),
            String::new(),
            specimen.cell_count.to_string(),
            specimen.alpha_bits.to_string(),
            optional(specimen.beta_bits),
        ];
        record.extend(specimen.percentages.iter().map(f64::to_string));
        writer.write_record(&record)?;
        for parent in &specimen.parents {
            let mut record = vec![
                specimen.specimen_id.clone(),
                parent.parent.clone(),
                parent.cell_count.to_string(),
                parent.alpha_bits.to_string(),
                optional(parent.divergence_bits),
            ];
            record.extend(parent.percentages.iter().map(f64::to_string));
            writer.write_record(&record)?;
        }
    }
    finish(writer)
}

pub(super) fn suggest_table(result: &SpaceSuggestResult) -> marklab::Result<Vec<u8>> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer.write_record([
        "specimen_id",
        "radius_um",
        "counted_cells",
        "eligible_centres",
        "mean_disc_cells",
        "sample_size",
    ])?;
    for specimen in &result.specimens {
        for radius in &specimen.radii {
            writer.write_record([
                specimen.specimen_id.clone(),
                radius.radius_um.to_string(),
                specimen.counted_cells.to_string(),
                specimen.eligible_centres.to_string(),
                radius.mean_disc_cells.to_string(),
                radius.sample_size.to_string(),
            ])?;
        }
    }
    finish(writer)
}
