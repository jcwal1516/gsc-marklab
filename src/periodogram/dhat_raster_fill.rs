#![cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]

use crate::{data::PatternMeta, periodogram::raster::centered_mark_raster_for_marks_into, Pattern};

#[test]
fn dhat_raster_fill_does_not_allocate_after_raster_allocation() {
    let pattern = small_pattern();
    let mut raster = Vec::with_capacity(64);

    let _profiler = dhat::Profiler::builder().testing().build();
    let before = dhat::HeapStats::get();

    let spec = centered_mark_raster_for_marks_into(&pattern, &pattern.mark, 1.0, &mut raster)
        .expect("raster");
    std::hint::black_box((&spec, &raster));

    let after = dhat::HeapStats::get();
    dhat::assert_eq!(after.total_blocks, before.total_blocks);
}

fn small_pattern() -> Pattern {
    Pattern::from_arrays(
        vec![0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0, 3.0],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        vec![1, 0, 0, 1, 0, 1, 0, 0],
        PatternMeta {
            case_id: "case_dhat".into(),
            timepoint: "post".into(),
            protein: "MSH6".into(),
            slide_id: None,
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern")
}
