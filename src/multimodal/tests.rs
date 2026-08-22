use crate::{
    multimodal::{
        cell_table::{
            load_he_cell_table_csv, load_ihc_cell_table_csv, CellSection, FusedCell, HeCell,
            IhcCell,
        },
        fusion::{fuse_registered_cells, FusionMeta},
    },
    registration::transform::Transform2D,
    AnalysisConfig, RegistrationTransform,
};

#[test]
fn multimodal_config_defaults_are_conservative() {
    let config = AnalysisConfig::default();
    assert!(config.registration.enabled);
    assert_eq!(config.registration.transform, RegistrationTransform::Affine);
    assert_eq!(config.registration.min_landmarks, 6);
    assert_eq!(config.registration.max_rmse_um, 25.0);
    assert_eq!(config.registration.claim_distance_multiplier, 2.0);

    assert!(config.neighborhood.enabled);
    assert_eq!(config.neighborhood.radius_um, 50.0);
    assert_eq!(config.neighborhood.k_nearest, 8);
    assert!(config.comparison.margins.territory_profile.is_none());
    assert!(!config.diagnostics.beta_posterior_groups);
    assert!(!config.diagnostics.graph_smoothing);
}

#[test]
fn he_csv_requires_cell_type_and_accepts_probability() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("he.csv");
    std::fs::write(
        &path,
        "cell_id,x_um,y_um,cell_type,cell_type_probability\nh1,1.0,2.0,lymphocyte,0.91\n",
    )
    .expect("write");

    let cells = load_he_cell_table_csv(&path).expect("he cells");
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].cell_id, "h1");
    assert_eq!(cells[0].cell_type.as_deref(), Some("lymphocyte"));
    assert_eq!(cells[0].cell_type_probability, Some(0.91));
}

#[test]
fn he_csv_rejects_blank_cell_id() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("he_blank_id.csv");
    std::fs::write(
        &path,
        "cell_id,x_um,y_um,cell_type,cell_type_probability\n,1.0,2.0,lymphocyte,0.91\n",
    )
    .expect("write");

    let err = load_he_cell_table_csv(&path).expect_err("blank cell_id should fail");
    assert!(err.to_string().contains("cell_id is required"));
}

#[test]
fn he_csv_rejects_whitespace_only_cell_type() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("he_blank_type.csv");
    std::fs::write(
        &path,
        "cell_id,x_um,y_um,cell_type,cell_type_probability\nh1,1.0,2.0,   ,0.91\n",
    )
    .expect("write");

    let err = load_he_cell_table_csv(&path).expect_err("blank cell_type should fail");
    assert!(err.to_string().contains("H&E cell_type is required"));
}

#[test]
fn he_csv_rejects_invalid_probability() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("he_bad_probability.csv");
    std::fs::write(
        &path,
        "cell_id,x_um,y_um,cell_type,cell_type_probability\nh1,1.0,2.0,lymphocyte,1.2\n",
    )
    .expect("write");

    let err = load_he_cell_table_csv(&path).expect_err("invalid probability should fail");
    assert!(err
        .to_string()
        .contains("cell_type_probability must be in [0, 1]"));
}

#[test]
fn he_csv_rejects_missing_cell_type_probability() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("he.csv");
    std::fs::write(
        &path,
        "cell_id,x_um,y_um,cell_type,cell_type_probability\nh1,1.0,2.0,lymphocyte,\n",
    )
    .expect("write");

    let err = load_he_cell_table_csv(&path).expect_err("missing probability should fail");

    assert!(err
        .to_string()
        .contains("cell_type_probability is required"));
}

#[test]
fn ihc_csv_accepts_binary_mark_or_probability() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("ihc.csv");
    std::fs::write(
        &path,
        "cell_id,x_um,y_um,mmr_mark,mmr_probability\nm1,5.0,8.0,1,0.97\n",
    )
    .expect("write");

    let cells = load_ihc_cell_table_csv(&path).expect("ihc cells");
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].cell_id, "m1");
    assert_eq!(cells[0].mmr_mark, Some(1));
    assert_eq!(cells[0].mmr_probability, Some(0.97));
}

#[test]
fn ihc_csv_rejects_invalid_mmr_mark() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("ihc_bad_mark.csv");
    std::fs::write(
        &path,
        "cell_id,x_um,y_um,mmr_mark,mmr_probability\nm1,5.0,8.0,2,0.97\n",
    )
    .expect("write");

    let err = load_ihc_cell_table_csv(&path).expect_err("invalid mmr_mark should fail");
    assert!(err.to_string().contains("mmr_mark must be 0 or 1"));
}

#[test]
fn ihc_csv_rejects_blank_cell_id() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("ihc_blank_id.csv");
    std::fs::write(
        &path,
        "cell_id,x_um,y_um,mmr_mark,mmr_probability\n,5.0,8.0,1,0.97\n",
    )
    .expect("write");

    let err = load_ihc_cell_table_csv(&path).expect_err("blank cell_id should fail");
    assert!(err.to_string().contains("cell_id is required"));
}

#[test]
fn ihc_csv_rejects_row_without_mark_or_probability() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("ihc_missing_mark_probability.csv");
    std::fs::write(
        &path,
        "cell_id,x_um,y_um,mmr_mark,mmr_probability\nm1,5.0,8.0,,\n",
    )
    .expect("write");

    let err = load_ihc_cell_table_csv(&path).expect_err("missing IHC markers should fail");
    assert!(err
        .to_string()
        .contains("IHC row requires mmr_mark or mmr_probability"));
}

#[test]
fn custom_validation_error_includes_row_path_and_cell_id_context() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("he_context.csv");
    std::fs::write(
        &path,
        "cell_id,x_um,y_um,cell_type,cell_type_probability\nh-context,1.0,2.0,lymphocyte,1.2\n",
    )
    .expect("write");

    let err = load_he_cell_table_csv(&path).expect_err("invalid probability should fail");
    let message = err.to_string();
    assert!(message.contains("row 2"));
    assert!(message.contains(path.to_string_lossy().as_ref()));
    assert!(message.contains("cell_id h-context"));
}

#[test]
fn fused_cell_preserves_serial_section_identity() {
    let fused = FusedCell {
        source_section: CellSection::He,
        source_cell_id: "h1".into(),
        x_um_registered: 1.0,
        y_um_registered: 2.0,
        mmr_mark: None,
        mmr_probability: None,
        cell_type: Some("lymphocyte".into()),
        cell_type_probability: Some(0.91),
        same_section: false,
        registration_error_um: Some(12.0),
        timepoint: "pre".into(),
        case_id: "case1".into(),
        protein: "MSH6".into(),
    };

    assert_eq!(fused.source_section, CellSection::He);
    assert!(!fused.same_section);
}

#[test]
fn fusion_maps_he_cells_into_ihc_coordinate_space() {
    let he = vec![HeCell {
        cell_id: "h1".into(),
        x_um: 2.0,
        y_um: 3.0,
        cell_type: Some("lymphocyte".into()),
        cell_type_probability: Some(0.9),
    }];
    let ihc = vec![IhcCell {
        cell_id: "m1".into(),
        x_um: 10.0,
        y_um: 20.0,
        mmr_mark: Some(1),
        mmr_probability: Some(0.99),
    }];
    let transform = Transform2D {
        transform_type: "test".into(),
        m00: 1.0,
        m01: 0.0,
        m02: 100.0,
        m10: 0.0,
        m11: 1.0,
        m12: 200.0,
    };
    let meta = FusionMeta {
        case_id: "case1".into(),
        timepoint: "pre".into(),
        protein: "MSH6".into(),
        registration_error_um: Some(8.0),
    };

    let fused = fuse_registered_cells(&he, &ihc, &transform, &meta).expect("fused");
    assert_eq!(fused.len(), 2);
    assert_eq!(fused[0].source_section, CellSection::He);
    assert_eq!(fused[0].x_um_registered, 102.0);
    assert_eq!(fused[0].registration_error_um, Some(8.0));
    assert!(!fused[0].same_section);
    assert_eq!(fused[0].mmr_mark, None);
    assert_eq!(fused[0].mmr_probability, None);
    assert_eq!(fused[0].cell_type.as_deref(), Some("lymphocyte"));
    assert_eq!(fused[1].source_section, CellSection::Ihc);
    assert!(fused[1].same_section);
    assert_eq!(fused[1].cell_type, None);
    assert_eq!(fused[1].cell_type_probability, None);
    assert_eq!(fused[1].mmr_mark, Some(1));
    assert_eq!(fused[1].mmr_probability, Some(0.99));
}

#[test]
fn fusion_rejects_blank_metadata_fields() {
    for (field_name, meta) in [
        (
            "case_id",
            FusionMeta {
                case_id: "   ".into(),
                timepoint: "pre".into(),
                protein: "MSH6".into(),
                registration_error_um: Some(8.0),
            },
        ),
        (
            "timepoint",
            FusionMeta {
                case_id: "case1".into(),
                timepoint: "\t".into(),
                protein: "MSH6".into(),
                registration_error_um: Some(8.0),
            },
        ),
        (
            "protein",
            FusionMeta {
                case_id: "case1".into(),
                timepoint: "pre".into(),
                protein: " ".into(),
                registration_error_um: Some(8.0),
            },
        ),
    ] {
        let err = fuse_registered_cells(&[], &[], &Transform2D::identity("test"), &meta)
            .expect_err("blank metadata field should fail");
        let message = err.to_string();
        assert!(message.contains("input schema error"));
        assert!(message.contains(field_name));
        assert!(message.contains("must not be blank"));
    }
}

#[test]
fn fusion_rejects_non_finite_transform_coefficient() {
    let mut transform = Transform2D::identity("test");
    transform.m02 = f64::INFINITY;
    let meta = valid_fusion_meta();

    let err = fuse_registered_cells(&[], &[], &transform, &meta)
        .expect_err("non-finite transform should fail");

    assert!(err
        .to_string()
        .contains("registration transform coefficients must be finite"));
}

#[test]
fn fusion_rejects_invalid_registration_error() {
    for registration_error_um in [Some(-0.1), Some(f64::INFINITY), Some(f64::NAN)] {
        let meta = FusionMeta {
            registration_error_um,
            ..valid_fusion_meta()
        };

        let err = fuse_registered_cells(&[], &[], &Transform2D::identity("test"), &meta)
            .expect_err("invalid registration_error_um should fail");

        assert!(err
            .to_string()
            .contains("registration_error_um must be finite and non-negative"));
    }
}

fn valid_fusion_meta() -> FusionMeta {
    FusionMeta {
        case_id: "case1".into(),
        timepoint: "pre".into(),
        protein: "MSH6".into(),
        registration_error_um: Some(8.0),
    }
}
