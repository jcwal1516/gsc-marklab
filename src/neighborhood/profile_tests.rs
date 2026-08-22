use std::collections::BTreeMap;

use crate::{
    multimodal::cells::{CellSection, FusedCell},
    neighborhood::profiles::{compare_territory_profiles, territory_profiles},
    CurveComparisonAvailability, LabelFraction, MarklabError, NeighborhoodTerritory,
    TerritoryProfile,
};

fn fused(id: &str, x: f64, y: f64, label: &str) -> FusedCell {
    fused_with_label(id, x, y, Some(label))
}

fn fused_with_label(id: &str, x: f64, y: f64, label: Option<&str>) -> FusedCell {
    FusedCell {
        source_section: CellSection::He,
        source_cell_id: id.into(),
        x_um_registered: x,
        y_um_registered: y,
        mmr_mark: None,
        mmr_probability: None,
        cell_type: label.map(str::to_owned),
        cell_type_probability: Some(1.0),
        same_section: false,
        registration_error_um: Some(3.0),
    }
}

fn territory(center_x_um: f64, radius_um: f64) -> NeighborhoodTerritory {
    NeighborhoodTerritory {
        center_x_um,
        center_y_um: 0.0,
        radius_um,
        supporting_abnormal_cells: 3,
        cluster_id: 0,
    }
}

fn profile(territory_id: usize, fractions: Vec<LabelFraction>) -> TerritoryProfile {
    TerritoryProfile {
        territory_id,
        cell_type_fractions: fractions,
        below_registration_resolution: false,
    }
}

#[test]
fn territory_profile_counts_local_cell_type_fractions() {
    let territories = vec![NeighborhoodTerritory {
        center_x_um: 0.0,
        center_y_um: 0.0,
        radius_um: 10.0,
        supporting_abnormal_cells: 5,
        cluster_id: 0,
    }];
    let cells = vec![
        fused("l1", 1.0, 0.0, "lymphocyte"),
        fused("l2", 2.0, 0.0, "lymphocyte"),
        fused("s1", 30.0, 0.0, "stroma"),
    ];

    let profiles = territory_profiles(&territories, &cells, 0.0).expect("profiles");
    assert_eq!(profiles.len(), 1);
    let lymphocyte = profiles[0]
        .cell_type_fractions
        .iter()
        .find(|row| row.label == "lymphocyte")
        .expect("lymphocyte");
    assert_eq!(lymphocyte.count, 2);
    assert_eq!(lymphocyte.fraction, 1.0);
}

#[test]
fn territory_neighbors_match_bruteforce() {
    let cells = (0..200)
        .map(|index| {
            let label = if index % 3 == 0 {
                "lymphocyte"
            } else {
                "stroma"
            };
            let mut cell = fused(
                &format!("cell_{index}"),
                ((index * 17) % 53) as f64 - 20.0,
                ((index * 31) % 47) as f64 - 18.0,
                label,
            );
            cell.registration_error_um = Some((index % 7) as f64 * 0.2);
            cell
        })
        .collect::<Vec<_>>();
    let territories = vec![
        territory(-10.0, 4.0),
        territory(0.0, 8.0),
        territory(15.0, 12.0),
    ];
    let buffer_um = 1.25;

    let actual = territory_profiles(&territories, &cells, buffer_um).expect("profiles");
    let expected = territories
        .iter()
        .enumerate()
        .map(|(territory_id, territory)| {
            brute_force_territory_profile(territory_id, territory, &cells, buffer_um)
        })
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
}

#[test]
fn territory_comparison_reports_distance_and_margin_assessment() {
    let territories = vec![
        NeighborhoodTerritory {
            center_x_um: 0.0,
            center_y_um: 0.0,
            radius_um: 5.0,
            supporting_abnormal_cells: 3,
            cluster_id: 0,
        },
        NeighborhoodTerritory {
            center_x_um: 100.0,
            center_y_um: 0.0,
            radius_um: 5.0,
            supporting_abnormal_cells: 3,
            cluster_id: 1,
        },
    ];
    let cells = vec![
        fused("l1", 1.0, 0.0, "lymphocyte"),
        fused("l2", 101.0, 0.0, "stroma"),
    ];
    let profiles = territory_profiles(&territories, &cells, 0.0).expect("profiles");
    let tests = compare_territory_profiles(&profiles, Some(0.25)).expect("comparison");
    assert!(!tests.is_empty());
    assert!(tests[0].comparison_name.contains("territory_0_vs_1"));
    assert!((tests[0].statistic.expect("statistic") - 1.0).abs() < f64::EPSILON);
    assert!(tests[0].margin.is_some());
    assert_eq!(tests[0].within_margin, Some(false));
}

#[test]
fn territory_comparison_with_no_known_labels_reports_margin_unavailable() {
    let profiles = vec![profile(0, Vec::new()), profile(1, Vec::new())];

    let tests = compare_territory_profiles(&profiles, Some(0.25)).expect("comparison");

    assert_eq!(tests.len(), 1);
    assert_eq!(
        tests[0].availability,
        CurveComparisonAvailability::InsufficientData
    );
    assert_eq!(tests[0].statistic, None);
    assert!(tests[0].unavailable_reason.is_some());
    assert_eq!(tests[0].margin, Some(0.25));
    assert_eq!(tests[0].within_margin, None);
    assert!(tests[0].interpretation.contains("insufficient"));
    assert!(tests[0].interpretation.contains("unavailable"));
}

#[test]
fn territory_comparison_with_only_zero_count_rows_reports_margin_unavailable() {
    let profiles = vec![
        profile(
            0,
            vec![LabelFraction {
                label: "lymphocyte".into(),
                fraction: 0.0,
                count: 0,
            }],
        ),
        profile(
            1,
            vec![LabelFraction {
                label: "lymphocyte".into(),
                fraction: 0.0,
                count: 0,
            }],
        ),
    ];

    let tests = compare_territory_profiles(&profiles, Some(0.25)).expect("comparison");

    assert_eq!(tests.len(), 1);
    assert_eq!(
        tests[0].availability,
        CurveComparisonAvailability::InsufficientData
    );
    assert_eq!(tests[0].statistic, None);
    assert!(tests[0].unavailable_reason.is_some());
    assert_eq!(tests[0].margin, Some(0.25));
    assert_eq!(tests[0].within_margin, None);
    assert!(tests[0].interpretation.contains("insufficient"));
    assert!(tests[0].interpretation.contains("unavailable"));
}

#[test]
fn territory_profile_excludes_blank_and_missing_cell_types_from_denominator() {
    let territories = vec![territory(0.0, 10.0)];
    let cells = vec![
        fused("known", 1.0, 0.0, "lymphocyte"),
        fused_with_label("blank", 2.0, 0.0, Some("   ")),
        fused_with_label("missing", 3.0, 0.0, None),
    ];

    let profiles = territory_profiles(&territories, &cells, 0.0).expect("profiles");

    assert_eq!(profiles[0].cell_type_fractions.len(), 1);
    assert_eq!(profiles[0].cell_type_fractions[0].label, "lymphocyte");
    assert_eq!(profiles[0].cell_type_fractions[0].count, 1);
    assert_eq!(profiles[0].cell_type_fractions[0].fraction, 1.0);
}

#[test]
fn territory_profiles_reject_invalid_buffer_radius_and_coordinates() {
    let territories = vec![territory(0.0, 10.0)];
    let cells = vec![fused("cell", 1.0, 0.0, "lymphocyte")];
    assert!(territory_profiles(&territories, &cells, -1.0).is_err());

    let invalid_radius = vec![territory(0.0, -1.0)];
    assert!(territory_profiles(&invalid_radius, &cells, 0.0).is_err());

    let invalid_center = vec![NeighborhoodTerritory {
        center_x_um: f64::NAN,
        ..territory(0.0, 10.0)
    }];
    assert!(territory_profiles(&invalid_center, &cells, 0.0).is_err());

    let invalid_cells = vec![fused("bad", f64::NAN, 0.0, "lymphocyte")];
    assert!(territory_profiles(&territories, &invalid_cells, 0.0).is_err());
}

#[test]
fn territory_profile_without_registration_error_is_not_below_resolution() {
    let territories = vec![territory(0.0, 1.0)];
    let mut cell = fused("cell", 0.0, 0.0, "lymphocyte");
    cell.registration_error_um = None;

    let profiles = territory_profiles(&territories, &[cell], 0.0).expect("profiles");

    assert!(!profiles[0].below_registration_resolution);
}

#[test]
fn territory_profile_registration_resolution_uses_strict_boundary() {
    let cells = vec![fused("cell", 0.0, 0.0, "lymphocyte")];

    let below = territory_profiles(&[territory(0.0, 5.9)], &cells, 0.0).expect("below");
    let boundary = territory_profiles(&[territory(0.0, 6.0)], &cells, 0.0).expect("boundary");

    assert!(below[0].below_registration_resolution);
    assert!(!boundary[0].below_registration_resolution);
}

fn brute_force_territory_profile(
    territory_id: usize,
    territory: &NeighborhoodTerritory,
    cells: &[FusedCell],
    buffer_um: f64,
) -> TerritoryProfile {
    let radius = territory.radius_um + buffer_um;
    let mut counts = BTreeMap::<&str, usize>::new();
    let mut known_cell_count = 0usize;
    let mut max_registration_error_um = 0.0_f64;
    for cell in cells {
        let distance = (cell.x_um_registered - territory.center_x_um)
            .hypot(cell.y_um_registered - territory.center_y_um);
        if distance > radius {
            continue;
        }
        max_registration_error_um =
            max_registration_error_um.max(cell.registration_error_um.unwrap_or(0.0));
        if let Some(label) = cell.cell_type.as_deref() {
            *counts.entry(label).or_insert(0) += 1;
            known_cell_count += 1;
        }
    }
    TerritoryProfile {
        territory_id,
        cell_type_fractions: counts
            .into_iter()
            .map(|(label, count)| LabelFraction {
                label: label.into(),
                fraction: count as f64 / known_cell_count as f64,
                count,
            })
            .collect(),
        below_registration_resolution: territory.radius_um < 2.0 * max_registration_error_um,
    }
}

#[test]
fn territory_comparison_rejects_invalid_margins() {
    let profiles = vec![profile(
        0,
        vec![LabelFraction {
            label: "lymphocyte".into(),
            fraction: 1.0,
            count: 1,
        }],
    )];

    assert!(compare_territory_profiles(&profiles, Some(-0.1)).is_err());
    assert!(compare_territory_profiles(&profiles, Some(f64::NAN)).is_err());
}

#[test]
fn territory_comparison_rejects_duplicate_profile_labels() {
    let profiles = vec![
        profile(
            0,
            vec![
                LabelFraction {
                    label: "lymphocyte".into(),
                    fraction: 0.5,
                    count: 1,
                },
                LabelFraction {
                    label: "lymphocyte".into(),
                    fraction: 0.5,
                    count: 1,
                },
            ],
        ),
        profile(
            1,
            vec![LabelFraction {
                label: "stroma".into(),
                fraction: 1.0,
                count: 1,
            }],
        ),
    ];

    let err = compare_territory_profiles(&profiles, Some(0.25)).expect_err("duplicate labels");

    assert!(matches!(err, MarklabError::Schema(_)));
}

#[test]
fn territory_comparison_rejects_invalid_public_profile_fractions() {
    let profiles = vec![
        profile(
            0,
            vec![LabelFraction {
                label: "lymphocyte".into(),
                fraction: 1.25,
                count: 1,
            }],
        ),
        profile(
            1,
            vec![LabelFraction {
                label: "lymphocyte".into(),
                fraction: 1.0,
                count: 1,
            }],
        ),
    ];

    let err = compare_territory_profiles(&profiles, Some(0.25)).expect_err("invalid fraction");

    assert!(matches!(err, MarklabError::Schema(_)));
}

#[test]
fn territory_profile_rejects_inclusion_radius_overflow() {
    let territories = vec![territory(0.0, f64::MAX)];
    let cells = vec![fused("cell", 1.0, 0.0, "lymphocyte")];

    assert!(territory_profiles(&territories, &cells, f64::MAX).is_err());
}
