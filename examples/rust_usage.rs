use marklab::{AnalysisConfig, AnalysisEngine, OutputWriter, Pattern, PatternMeta, ResultDocument};

fn main() -> marklab::Result<()> {
    let config = AnalysisConfig::from_toml_path("examples/config.toml")?;
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
    )?;
    let output = config.output.clone();
    let engine = AnalysisEngine::new(config)?;
    let result = engine.analyze_pattern(&pattern)?;
    OutputWriter::write(
        &ResultDocument::marked(result),
        "out/case_001_post",
        &output,
    )?;
    Ok(())
}
