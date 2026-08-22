use crate::{
    common::stats::{mean_all_finite, median_average_even},
    output::{
        AnalysisSection, MarkedPatternResult, MultimodalResult, ResidualTerritory,
        TerritoryFeature, TerritoryPrePostSummary,
    },
};

use super::numeric_delta;

pub(super) fn marked_summary(
    pre: &MarkedPatternResult,
    post: &MarkedPatternResult,
) -> AnalysisSection<TerritoryPrePostSummary> {
    summary_from_slices(
        pre.residual_territories.value().map(Vec::as_slice),
        post.residual_territories.value().map(Vec::as_slice),
        "multiscale residual territories are unavailable in one or both results",
    )
}

pub(super) fn multimodal_summary(
    pre: &MultimodalResult,
    post: &MultimodalResult,
) -> AnalysisSection<TerritoryPrePostSummary> {
    summary_from_slices(
        pre.neighborhood_territories.value().map(Vec::as_slice),
        post.neighborhood_territories.value().map(Vec::as_slice),
        "neighborhood territories are unavailable in one or both results",
    )
}

fn summary_from_slices<T: TerritorySummaryView>(
    pre_territories: Option<&[T]>,
    post_territories: Option<&[T]>,
    unavailable_reason: &str,
) -> AnalysisSection<TerritoryPrePostSummary> {
    let (Some(pre_territories), Some(post_territories)) = (pre_territories, post_territories)
    else {
        return AnalysisSection::InsufficientData {
            reason: unavailable_reason.into(),
        };
    };
    let pre_count = pre_territories.len();
    let post_count = post_territories.len();
    AnalysisSection::available(TerritoryPrePostSummary {
        pre_count,
        post_count,
        delta_count: post_count as isize - pre_count as isize,
        delta_mean_radius_um: numeric_delta(
            mean_territory_radius(pre_territories),
            mean_territory_radius(post_territories),
            "mean territory radius is undefined because one result has no territories",
        ),
        delta_median_radius_um: numeric_delta(
            median_territory_radius(pre_territories),
            median_territory_radius(post_territories),
            "median territory radius is undefined because one result has no territories",
        ),
        delta_mean_supporting_cells: numeric_delta(
            mean_supporting_cells(pre_territories),
            mean_supporting_cells(post_territories),
            "mean supporting-cell count is undefined because one result has no territories",
        ),
        delta_median_supporting_cells: numeric_delta(
            median_supporting_cells(pre_territories),
            median_supporting_cells(post_territories),
            "median supporting-cell count is undefined because one result has no territories",
        ),
        new_domain_count: unmatched_domain_count(post_territories, pre_territories),
        lost_domain_count: unmatched_domain_count(pre_territories, post_territories),
    })
}

fn mean_territory_radius(territories: &[impl TerritorySummaryView]) -> Option<f64> {
    mean_all_finite(territories.iter().map(TerritorySummaryView::radius_um))
}

fn median_territory_radius(territories: &[impl TerritorySummaryView]) -> Option<f64> {
    let mut values = territories
        .iter()
        .map(TerritorySummaryView::radius_um)
        .collect::<Vec<_>>();
    median_average_even(&mut values)
}

fn mean_supporting_cells(territories: &[impl TerritorySummaryView]) -> Option<f64> {
    mean_all_finite(
        territories
            .iter()
            .map(|territory| territory.supporting_cells() as f64),
    )
}

fn median_supporting_cells(territories: &[impl TerritorySummaryView]) -> Option<f64> {
    let mut values = territories
        .iter()
        .map(|territory| territory.supporting_cells() as f64)
        .collect::<Vec<_>>();
    median_average_even(&mut values)
}

fn unmatched_domain_count<T: TerritorySummaryView>(query: &[T], reference: &[T]) -> usize {
    query
        .iter()
        .filter(|territory| {
            !reference
                .iter()
                .any(|candidate| domains_match(*territory, candidate))
        })
        .count()
}

fn domains_match(left: &impl TerritorySummaryView, right: &impl TerritorySummaryView) -> bool {
    let dx = left.center_x_um() - right.center_x_um();
    let dy = left.center_y_um() - right.center_y_um();
    let tolerance = left.radius_um().max(right.radius_um());
    dx.hypot(dy) <= tolerance
}

trait TerritorySummaryView {
    fn center_x_um(&self) -> f64;
    fn center_y_um(&self) -> f64;
    fn radius_um(&self) -> f64;
    fn supporting_cells(&self) -> usize;
}

impl TerritorySummaryView for ResidualTerritory {
    fn center_x_um(&self) -> f64 {
        self.center_x_um
    }
    fn center_y_um(&self) -> f64 {
        self.center_y_um
    }
    fn radius_um(&self) -> f64 {
        self.radius_um
    }
    fn supporting_cells(&self) -> usize {
        self.supporting_marked_cells
    }
}

impl TerritorySummaryView for TerritoryFeature {
    fn center_x_um(&self) -> f64 {
        self.center_x_um
    }
    fn center_y_um(&self) -> f64 {
        self.center_y_um
    }
    fn radius_um(&self) -> f64 {
        self.radius_um
    }
    fn supporting_cells(&self) -> usize {
        self.supporting_cells
    }
}
