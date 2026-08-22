use crate::data::Pattern;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct GeometrySummary {
    pub(super) area_um2: f64,
    pub(super) effective_length_um: f64,
    pub(super) mean_nearest_neighbor_um: f64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct MarkedAnalysisContext<'pattern> {
    pattern: &'pattern Pattern,
    n_cells: usize,
    n_marked: usize,
    n_unmarked: usize,
    prevalence: f64,
    geometry: GeometrySummary,
}

impl<'pattern> MarkedAnalysisContext<'pattern> {
    pub(super) fn new(pattern: &'pattern Pattern) -> Self {
        let n_cells = pattern.len();
        let n_marked = pattern.mark.iter().filter(|mark| **mark == 1).count();
        Self {
            pattern,
            n_cells,
            n_marked,
            n_unmarked: n_cells.saturating_sub(n_marked),
            prevalence: if n_cells == 0 {
                0.0
            } else {
                n_marked as f64 / n_cells as f64
            },
            geometry: GeometrySummary {
                area_um2: pattern.window.area_um2,
                effective_length_um: pattern.window.l_eff_um,
                mean_nearest_neighbor_um: pattern.window.d_nn_mean_um,
            },
        }
    }

    pub(super) fn pattern(self) -> &'pattern Pattern {
        self.pattern
    }

    pub(super) fn n_cells(self) -> usize {
        self.n_cells
    }

    pub(super) fn n_marked(self) -> usize {
        self.n_marked
    }

    pub(super) fn n_unmarked(self) -> usize {
        self.n_unmarked
    }

    pub(super) fn prevalence(self) -> f64 {
        self.prevalence
    }

    pub(super) fn geometry(self) -> GeometrySummary {
        self.geometry
    }
}
