use std::mem::size_of;

use crate::data::Pattern;

#[cfg(test)]
thread_local! {
    static PLAN_BUILD_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_plan_build_call_count() {
    PLAN_BUILD_CALLS.set(0);
}

#[cfg(test)]
pub(crate) fn plan_build_call_count() -> usize {
    PLAN_BUILD_CALLS.get()
}

#[derive(Clone, Debug, PartialEq)]
pub struct RasterSpec {
    pub width: usize,
    pub height: usize,
    pub cell_size_um: f64,
}

/// Fixed mapping from pattern cells to a rectangular raster.
///
/// Geometry is resolved once. Alternate binary mark assignments only clear
/// and refill the raster through the retained linear bin indices.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RasterAssignmentPlan {
    spec: RasterSpec,
    cell_bins: Box<[usize]>,
}

impl RasterAssignmentPlan {
    pub(crate) fn new(pattern: &Pattern, cell_size_um: f64) -> Option<Self> {
        if pattern.is_empty() || cell_size_um <= 0.0 || !cell_size_um.is_finite() {
            return None;
        }
        #[cfg(test)]
        PLAN_BUILD_CALLS.set(PLAN_BUILD_CALLS.get() + 1);

        let min_x = pattern.x_um.iter().copied().reduce(f64::min)?;
        let max_x = pattern.x_um.iter().copied().reduce(f64::max)?;
        let min_y = pattern.y_um.iter().copied().reduce(f64::min)?;
        let max_y = pattern.y_um.iter().copied().reduce(f64::max)?;
        if [min_x, max_x, min_y, max_y]
            .into_iter()
            .any(|value| !value.is_finite())
        {
            return None;
        }

        let width = (((max_x - min_x) / cell_size_um).floor() as usize).checked_add(1)?;
        let height = (((max_y - min_y) / cell_size_um).floor() as usize).checked_add(1)?;
        width.checked_mul(height)?;
        let cell_bins = pattern
            .x_um
            .iter()
            .copied()
            .zip(pattern.y_um.iter().copied())
            .map(|(x, y)| {
                let column = ((x - min_x) / cell_size_um).floor() as usize;
                let row = ((y - min_y) / cell_size_um).floor() as usize;
                (column < width && row < height).then(|| row * width + column)
            })
            .collect::<Option<Vec<_>>>()?
            .into_boxed_slice();

        Some(Self {
            spec: RasterSpec {
                width,
                height,
                cell_size_um,
            },
            cell_bins,
        })
    }

    pub(crate) fn spec(&self) -> &RasterSpec {
        &self.spec
    }

    pub(crate) fn pixel_count(&self) -> usize {
        self.spec.width * self.spec.height
    }

    pub(crate) fn estimated_storage_bytes(&self) -> usize {
        self.cell_bins.len().saturating_mul(size_of::<usize>())
    }

    pub(crate) fn fill_centered_binary_marks(
        &self,
        marks: &[u8],
        raster: &mut Vec<f32>,
    ) -> Option<()> {
        if marks.len() != self.cell_bins.len() || marks.iter().any(|mark| *mark != 0 && *mark != 1)
        {
            return None;
        }

        raster.clear();
        raster.resize(self.pixel_count(), 0.0);
        let prevalence =
            marks.iter().filter(|mark| **mark == 1).count() as f64 / marks.len() as f64;
        for (bin, mark) in self.cell_bins.iter().copied().zip(marks.iter().copied()) {
            raster[bin] += (f64::from(mark) - prevalence) as f32;
        }
        Some(())
    }
}

pub fn centered_mark_raster(
    pattern: &Pattern,
    cell_size_um: f64,
) -> Option<(RasterSpec, Vec<f32>)> {
    centered_mark_raster_for_marks(pattern, &pattern.mark, cell_size_um)
}

pub fn centered_mark_raster_for_marks(
    pattern: &Pattern,
    marks: &[u8],
    cell_size_um: f64,
) -> Option<(RasterSpec, Vec<f32>)> {
    let plan = RasterAssignmentPlan::new(pattern, cell_size_um)?;
    let mut raster = Vec::with_capacity(plan.pixel_count());
    plan.fill_centered_binary_marks(marks, &mut raster)?;
    Some((plan.spec, raster))
}
