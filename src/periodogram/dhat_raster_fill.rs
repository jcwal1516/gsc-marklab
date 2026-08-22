#![cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]

use crate::{data::PatternMeta, periodogram::raster::RasterAssignmentPlan, Pattern};

#[test]
fn dhat_raster_fill_does_not_allocate_after_raster_allocation() {
    let pattern = small_pattern();
    let plan = RasterAssignmentPlan::new(&pattern, 1.0).expect("raster plan");
    let mut raster = Vec::with_capacity(64);

    let _profiler = dhat::Profiler::builder().testing().build();
    let before = dhat::HeapStats::get();

    plan.fill_centered_binary_marks(&pattern.mark, &mut raster)
        .expect("raster");
    std::hint::black_box((&plan, &raster));

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
