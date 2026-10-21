use marklab::PatternMeta;

pub fn pattern_meta(
    case_id: String,
    timepoint: String,
    protein: String,
    slide_id: Option<String>,
) -> PatternMeta {
    PatternMeta {
        case_id,
        timepoint,
        protein,
        slide_id,
        section_id: None,
        stain_batch: None,
        block_id: None,
        region_id: None,
    }
}
