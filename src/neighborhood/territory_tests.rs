use crate::{
    multimodal::cell_table::{CellSection, FusedCell},
    neighborhood::territories::{detect_mmr_abnormal_territories, TerritoryDomainConfig},
};

fn ihc_cell(id: &str, x_um: f64, y_um: f64, mmr_probability: f64) -> FusedCell {
    FusedCell {
        source_section: CellSection::Ihc,
        source_cell_id: id.into(),
        x_um_registered: x_um,
        y_um_registered: y_um,
        mmr_mark: None,
        mmr_probability: Some(mmr_probability),
        cell_type: None,
        cell_type_probability: None,
        same_section: true,
        registration_error_um: Some(2.0),
        timepoint: "post".into(),
        case_id: "case1".into(),
        protein: "MSH6".into(),
    }
}

fn he_cell(id: &str, x_um: f64, y_um: f64, cell_type: &str) -> FusedCell {
    FusedCell {
        source_section: CellSection::He,
        source_cell_id: id.into(),
        x_um_registered: x_um,
        y_um_registered: y_um,
        mmr_mark: None,
        mmr_probability: None,
        cell_type: Some(cell_type.into()),
        cell_type_probability: Some(0.95),
        same_section: false,
        registration_error_um: Some(2.0),
        timepoint: "post".into(),
        case_id: "case1".into(),
        protein: "MSH6".into(),
    }
}

#[test]
fn dbscan_territory_detection_clusters_density_reachable_ihc_abnormal_cells_and_drops_noise() {
    let cells = vec![
        ihc_cell("a", 0.0, 0.0, 0.9),
        ihc_cell("b", 8.0, 0.0, 0.9),
        ihc_cell("c", 16.0, 0.0, 0.9),
        ihc_cell("noise", 200.0, 0.0, 0.9),
        ihc_cell("retained", 4.0, 30.0, 0.1),
        he_cell("he-abnormal-label", 6.0, 0.0, "mmr_abnormal"),
    ];

    let territories = detect_mmr_abnormal_territories(
        &cells,
        TerritoryDomainConfig {
            eps_um: 10.0,
            min_cells: 2,
            min_radius_um: 1.0,
        },
    )
    .expect("territories");

    assert_eq!(territories.len(), 1);
    assert_eq!(territories[0].supporting_cells, 3);
    assert_eq!(territories[0].component_id, Some(0));
    assert!((territories[0].center_x_um - 8.0).abs() < 1.0e-9);
    assert_eq!(territories[0].center_y_um, 0.0);
    assert!(territories[0].radius_um >= 9.0);
}

#[test]
fn dbscan_territory_detection_rejects_invalid_config() {
    let cells = vec![ihc_cell("a", 0.0, 0.0, 0.9)];

    assert!(detect_mmr_abnormal_territories(
        &cells,
        TerritoryDomainConfig {
            eps_um: 0.0,
            min_cells: 1,
            min_radius_um: 1.0,
        },
    )
    .is_err());
    assert!(detect_mmr_abnormal_territories(
        &cells,
        TerritoryDomainConfig {
            eps_um: 10.0,
            min_cells: 0,
            min_radius_um: 1.0,
        },
    )
    .is_err());
}
