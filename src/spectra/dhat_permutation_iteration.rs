#![cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]

use crate::{
    data::PatternMeta,
    spectra::structure_factor::{
        permutation_power_for_modes_into, resolvable_modes_for_pattern, total_phase_sums_for_modes,
    },
    Pattern,
};

#[test]
fn dhat_one_permutation_iteration_does_not_allocate_after_scratch_setup() {
    let pattern = small_pattern();
    let modes = resolvable_modes_for_pattern(&pattern, 4).expect("resolvable modes");
    let mut total_phase_sums = Vec::with_capacity(modes.len());
    total_phase_sums_for_modes(&pattern, &modes, &mut total_phase_sums).expect("phase sums");
    let mut selected_indices = Vec::with_capacity(pattern.len());
    let mut powers = Vec::with_capacity(modes.len());

    let _profiler = dhat::Profiler::builder().testing().build();
    let before = dhat::HeapStats::get();

    assert!(permutation_power_for_modes_into(
        &pattern,
        &modes,
        &total_phase_sums,
        123_456,
        &mut selected_indices,
        &mut powers
    )
    .is_some());
    std::hint::black_box(&powers);

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
