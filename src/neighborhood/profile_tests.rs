use crate::{
    multimodal::cell_table::{CellSection, FusedCell},
    neighborhood::profiles::{compare_territory_profiles, territory_profiles},
    LabelFraction, MmrspaceError, TerritoryFeature, TerritoryProfile,
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
        timepoint: "post".into(),
        case_id: "case1".into(),
        protein: "MSH6".into(),
    }
}

fn territory(center_x_um: f64, radius_um: f64) -> TerritoryFeature {
    TerritoryFeature {
        center_x_um,
        center_y_um: 0.0,
        radius_um,
        scale_um: 3.5,
        z_or_power: 3.0,
        supporting_cells: 3,
        component_id: None,
        qc_overlap_fraction: 0.0,
    }
}

fn profile(territory_id: usize, fractions: Vec<LabelFraction>) -> TerritoryProfile {
    TerritoryProfile {
        territory_id,
        cell_type_fractions: fractions,
        enrichment: Vec::new(),
        cross_curves: Vec::new(),
        below_registration_resolution: false,
    }
}

#[test]
fn territory_profile_counts_local_cell_type_fractions() {
    let territories = vec![TerritoryFeature {
        center_x_um: 0.0,
        center_y_um: 0.0,
        radius_um: 10.0,
        scale_um: 7.0,
        z_or_power: 4.0,
        supporting_cells: 5,
        component_id: None,
        qc_overlap_fraction: 0.0,
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
fn territory_comparison_reports_difference_and_equivalence() {
    let territories = vec![
        TerritoryFeature {
            center_x_um: 0.0,
            center_y_um: 0.0,
            radius_um: 5.0,
            scale_um: 3.5,
            z_or_power: 3.0,
            supporting_cells: 3,
            component_id: None,
            qc_overlap_fraction: 0.0,
        },
        TerritoryFeature {
            center_x_um: 100.0,
            center_y_um: 0.0,
            radius_um: 5.0,
            scale_um: 3.5,
            z_or_power: 3.0,
            supporting_cells: 3,
            component_id: None,
            qc_overlap_fraction: 0.0,
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
    assert!((tests[0].statistic - 1.0).abs() < f64::EPSILON);
    assert!(tests[0].equivalence_margin.is_some());
    assert_eq!(tests[0].equivalent, Some(false));
}

#[test]
fn territory_comparison_with_no_known_labels_is_non_confirmatory() {
    let profiles = vec![profile(0, Vec::new()), profile(1, Vec::new())];

    let tests = compare_territory_profiles(&profiles, Some(0.25)).expect("comparison");

    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].statistic, 0.0);
    assert_eq!(tests[0].equivalence_margin, Some(0.25));
    assert_eq!(tests[0].equivalent, None);
    assert!(tests[0].interpretation.contains("insufficient"));
    assert!(tests[0].interpretation.contains("non-confirmatory"));
}

#[test]
fn territory_comparison_with_only_zero_count_rows_is_non_confirmatory() {
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
    assert_eq!(tests[0].statistic, 0.0);
    assert_eq!(tests[0].equivalence_margin, Some(0.25));
    assert_eq!(tests[0].equivalent, None);
    assert!(tests[0].interpretation.contains("insufficient"));
    assert!(tests[0].interpretation.contains("non-confirmatory"));
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

    let invalid_center = vec![TerritoryFeature {
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

#[test]
fn territory_comparison_rejects_invalid_equivalence_margins() {
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

    assert!(matches!(err, MmrspaceError::Schema(_)));
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

    assert!(matches!(err, MmrspaceError::Schema(_)));
}

#[test]
fn territory_profile_rejects_inclusion_radius_overflow() {
    let territories = vec![territory(0.0, f64::MAX)];
    let cells = vec![fused("cell", 1.0, 0.0, "lymphocyte")];

    assert!(territory_profiles(&territories, &cells, f64::MAX).is_err());
}
