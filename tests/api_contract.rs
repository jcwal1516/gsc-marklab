use std::fs;

use mmrspace::{
    AnalysisConfig, AnalysisEngine, HeCell, IhcCell, LandmarkPair, MultimodalEngine,
    MultimodalInput, Pattern, PatternMeta, ResultDocument, StatusFlag,
};

#[test]
fn public_api_exposes_engine_config_pattern_and_flags() {
    let mut config = AnalysisConfig::default();
    config.permutation.stratified = false;
    let engine = AnalysisEngine::new(config.clone()).expect("engine");
    let pattern = Pattern::from_arrays(
        vec![0.0, 10.0, 20.0, 30.0],
        vec![0.0, 0.0, 0.0, 0.0],
        vec![1, 0, 1, 0],
        PatternMeta {
            case_id: "case_001".into(),
            timepoint: "post".into(),
            protein: "MSH6".into(),
            slide_id: None,
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern");

    let result = engine.analyze_pattern(&pattern).expect("analysis");
    assert_eq!(result.case_id, "case_001");
    assert!(!result
        .status_flags
        .contains(&StatusFlag::SuppressedBiologicInterpretation));
}

#[test]
fn public_multimodal_engine_returns_a_distinct_multimodal_result() {
    let mut config = AnalysisConfig::default();
    config.registration.enabled = true;
    config.registration.min_landmarks = 3;
    config.neighborhood.enabled = true;
    config.neighborhood.label_pairs.clear();
    config.permutation.b = 9;
    config.inference.family_wise_alpha = 0.25;
    let input = MultimodalInput {
        he_cells: vec![HeCell {
            cell_id: "he-1".into(),
            x_um: 1.0,
            y_um: 1.0,
            cell_type: Some("tumor".into()),
            cell_type_probability: Some(1.0),
        }],
        ihc_cells: vec![IhcCell {
            cell_id: "ihc-1".into(),
            x_um: 1.0,
            y_um: 1.0,
            mmr_mark: Some(1),
            mmr_probability: Some(1.0),
        }],
        landmarks: vec![
            LandmarkPair::new(0.0, 0.0, 0.0, 0.0),
            LandmarkPair::new(10.0, 0.0, 10.0, 0.0),
            LandmarkPair::new(0.0, 10.0, 0.0, 10.0),
        ],
        case_id: "case-mm".into(),
        timepoint: "post".into(),
        protein: "MSH6".into(),
    };

    let result = MultimodalEngine::new(config)
        .expect("engine")
        .analyze(&input)
        .expect("analysis");

    assert_eq!(result.case_id, "case-mm");
    assert_eq!(result.fused_cells.len(), 2);
    assert!(result.neighborhood_territories.value().is_some());
    let json = serde_json::to_value(ResultDocument::multimodal(result)).expect("serialize");
    assert!(json["analysis"]["result"].get("spectrum").is_none());
    assert!(json["analysis"]["result"].get("wavelet").is_none());
}

#[test]
fn public_pattern_api_loads_from_cell_and_mask_paths() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("cells.csv");
    let mask = dir.path().join("mask.geojson");
    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,slide_id,section_id,stain_batch,block_id,region_id\n\
0.0,0.0,1,case_001,post,MSH6,true,true,slide_a,section_a,batch_a,block_a,region_a\n\
1.0,0.0,0,case_001,post,MSH6,true,true,slide_a,section_a,batch_a,block_a,region_a\n\
2.0,0.0,1,case_001,post,MSH6,true,true,slide_a,section_a,batch_a,block_a,region_a\n\
3.0,0.0,0,case_001,post,MSH6,true,true,slide_a,section_a,batch_a,block_a,region_a\n",
    )
    .expect("write cells");
    fs::write(
        &mask,
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("write mask");

    let pattern = Pattern::from_paths(&cells, &mask).expect("load pattern from paths");

    assert_eq!(pattern.len(), 4);
    assert_eq!(pattern.n_marked(), 2);
    assert_eq!(pattern.meta.case_id, "case_001");
    assert_eq!(pattern.meta.slide_id.as_deref(), Some("slide_a"));
    assert_eq!(pattern.meta.section_id.as_deref(), Some("section_a"));
    assert_eq!(pattern.meta.stain_batch.as_deref(), Some("batch_a"));
    assert_eq!(pattern.meta.block_id.as_deref(), Some("block_a"));
    assert_eq!(pattern.meta.region_id.as_deref(), Some("region_a"));
}
