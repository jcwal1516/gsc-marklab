#![allow(dead_code)]

#[path = "support/real_cellvit_categorical.rs"]
mod real_cellvit_categorical;

use std::path::Path;

use marklab::{
    top1_pair_mixing_bounds, DeclaredScalarPatternInput, ScalarMarkId, Top1PairMixingBoundsConfig,
    Top1PairMixingBoundsError, Top1PairMixingBoundsLimits,
};
use real_cellvit_categorical::real_fixture;

const REAL_CELLS: &str = "MARKLAB_REAL_CELLVIT_CATEGORICAL_CSV";
const REAL_WINDOW: &str = "MARKLAB_REAL_CELLVIT_CATEGORICAL_WINDOW";

#[test]
#[ignore = "requires the admitted real CellViT coordinate CSV and exact window"]
fn admitted_cellvit_pixel_support_is_not_accepted_as_top1_probability() {
    let cells = std::env::var_os(REAL_CELLS).expect(REAL_CELLS);
    let window = std::env::var_os(REAL_WINDOW).expect(REAL_WINDOW);
    let fixture = real_fixture(Path::new(&cells), Path::new(&window));
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &fixture.table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("real declared input");
    let config = Top1PairMixingBoundsConfig::new(
        50.0,
        5,
        Top1PairMixingBoundsLimits::new(2_000, 8, 4_000_000, 64_000_000, 128 << 20)
            .expect("limits"),
    )
    .expect("config");
    let categorical = ScalarMarkId::new("histologic_compartment").expect("categorical mark ID");
    let confidence =
        ScalarMarkId::new("cellvit_winning_class_confidence").expect("confidence mark ID");

    assert!(matches!(
        top1_pair_mixing_bounds(
            &input,
            &fixture.window,
            &categorical,
            &confidence,
            &config,
        ),
        Err(Top1PairMixingBoundsError::InfeasibleTop1Probability {
            row: 462,
            probability,
            minimum,
        }) if probability.to_bits() == f64::from(0.00395256915_f32).to_bits()
            && minimum.to_bits() == 0.2_f64.to_bits()
    ));
}
