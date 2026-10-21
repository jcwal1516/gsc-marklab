use geo::Polygon;

use crate::Result;

use super::{charge, input::BranchingDesign, invalid};

/// Pruned skeleton summary of one rasterized epithelial component.
pub(super) struct Skeleton {
    pub length_um: f64,
    pub branch_points: usize,
    pub end_points: usize,
}

/// Neighbor offsets P2..P9: N, NE, E, SE, S, SW, W, NW.
const OFFSETS: [(isize, isize); 8] = [
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
    (-1, 0),
    (-1, 1),
];

struct Grid {
    width: usize,
    pixels: Vec<u8>,
}

impl Grid {
    fn neighbor(&self, index: usize, offset: (isize, isize)) -> usize {
        // Foreground pixels never touch the one-pixel background border.
        (index as isize + offset.1 * self.width as isize + offset.0) as usize
    }

    fn ring(&self, index: usize) -> [u8; 8] {
        OFFSETS.map(|offset| self.pixels[self.neighbor(index, offset)])
    }
}

/// Foreground neighbor count and 0-to-1 transitions around the neighbor ring.
fn counts(ring: &[u8; 8]) -> (u8, u8) {
    let occupied = ring.iter().sum();
    let transitions = (0..8)
        .filter(|&k| ring[k] == 0 && ring[(k + 1) % 8] == 1)
        .count() as u8;
    (occupied, transitions)
}

/// Rasterize pixel centres, thin with Zhang-Suen, prune terminal branches shorter than
/// the declared length once, then count crossing-number branch points and ends.
pub(super) fn measure(
    polygon: &Polygon<f64>,
    design: &BranchingDesign,
    remaining_pixels: &mut usize,
    remaining_work: &mut usize,
    memory_budget_bytes: usize,
) -> Result<Skeleton> {
    let mut grid = rasterize(
        polygon,
        design.pixel_size_um,
        remaining_pixels,
        remaining_work,
        memory_budget_bytes,
    )?;
    thin(&mut grid, remaining_work)?;
    prune(
        &mut grid,
        design.minimum_branch_length_um / design.pixel_size_um,
        remaining_work,
    )?;
    Ok(summarize(&grid, design.pixel_size_um))
}

fn rasterize(
    polygon: &Polygon<f64>,
    pixel_um: f64,
    remaining_pixels: &mut usize,
    remaining_work: &mut usize,
    memory_budget_bytes: usize,
) -> Result<Grid> {
    let exterior = &polygon.exterior().0;
    let (mut minimum, mut maximum) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for point in exterior {
        minimum = [minimum[0].min(point.x), minimum[1].min(point.y)];
        maximum = [maximum[0].max(point.x), maximum[1].max(point.y)];
    }
    let span = |axis: usize| ((maximum[axis] - minimum[axis]) / pixel_um).ceil() as usize + 2;
    let (width, height) = (span(0), span(1));
    let pixels = width
        .checked_mul(height)
        .ok_or_else(|| invalid("branching raster size overflow"))?;
    charge(remaining_pixels, pixels, "branching raster pixel")?;
    if pixels.saturating_mul(3) > memory_budget_bytes {
        return Err(invalid("branching raster exceeds the memory budget"));
    }
    let origin = [minimum[0] - pixel_um, minimum[1] - pixel_um];
    let mut crossings = vec![Vec::new(); height];
    for ring in std::iter::once(polygon.exterior()).chain(polygon.interiors()) {
        for pair in ring.0.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if a.y == b.y {
                continue;
            }
            let row = |y: f64| (y - origin[1]) / pixel_um - 0.5;
            let first = row(a.y.min(b.y)).ceil().max(0.0) as usize;
            let last = (row(a.y.max(b.y)).floor() as usize).min(height - 1);
            for (j, row_crossings) in crossings.iter_mut().enumerate().take(last + 1).skip(first) {
                let center = origin[1] + (j as f64 + 0.5) * pixel_um;
                if (a.y > center) != (b.y > center) {
                    row_crossings.push(a.x + (center - a.y) * (b.x - a.x) / (b.y - a.y));
                }
            }
            charge(
                remaining_work,
                last.saturating_sub(first) + 1,
                "branching rasterization",
            )?;
        }
    }
    let mut grid = Grid {
        width,
        pixels: vec![0; pixels],
    };
    for (j, row) in crossings.iter_mut().enumerate() {
        row.sort_by(f64::total_cmp);
        for [enter, leave] in row.as_chunks::<2>().0 {
            let column = |x: f64| (x - origin[0]) / pixel_um - 0.5;
            let first = column(*enter).ceil().max(0.0) as usize;
            let last = (column(*leave).floor().max(-1.0) + 1.0) as usize;
            for i in first..last.min(width) {
                grid.pixels[j * width + i] = 1;
            }
        }
    }
    Ok(grid)
}

fn thin(grid: &mut Grid, remaining_work: &mut usize) -> Result<()> {
    let mut queued = vec![false; grid.pixels.len()];
    let mut candidates = (0..grid.pixels.len())
        .filter(|&index| grid.pixels[index] == 1 && grid.ring(index).contains(&0))
        .collect::<Vec<_>>();
    for &index in &candidates {
        queued[index] = true;
    }
    loop {
        let mut deleted_any = false;
        for step in 0..2 {
            charge(remaining_work, candidates.len().max(1), "skeleton thinning")?;
            let delete = candidates
                .iter()
                .copied()
                .filter(|&index| {
                    let ring = grid.ring(index);
                    let (occupied, transitions) = counts(&ring);
                    let [north, _, east, _, south, _, west, _] = ring;
                    let directional = if step == 0 {
                        north * east * south == 0 && east * south * west == 0
                    } else {
                        north * east * west == 0 && north * south * west == 0
                    };
                    (2..=6).contains(&occupied) && transitions == 1 && directional
                })
                .collect::<Vec<_>>();
            for &index in &delete {
                grid.pixels[index] = 0;
            }
            deleted_any |= !delete.is_empty();
            let mut next = Vec::with_capacity(candidates.len());
            for &index in &candidates {
                queued[index] = false;
            }
            let neighbors = delete
                .iter()
                .flat_map(|&index| OFFSETS.map(|offset| grid.neighbor(index, offset)));
            for index in candidates.iter().copied().chain(neighbors) {
                if grid.pixels[index] == 1 && !queued[index] {
                    queued[index] = true;
                    next.push(index);
                }
            }
            candidates = next;
        }
        if !deleted_any {
            return Ok(());
        }
    }
}

fn step_length(offset: (isize, isize)) -> f64 {
    if offset.0 != 0 && offset.1 != 0 {
        std::f64::consts::SQRT_2
    } else {
        1.0
    }
}

/// Remove each endpoint-to-branch-point path shorter than `minimum_pixels`, traced on the
/// unpruned skeleton; paths between two ends are kept.
fn prune(grid: &mut Grid, minimum_pixels: f64, remaining_work: &mut usize) -> Result<()> {
    if minimum_pixels <= 0.0 {
        return Ok(());
    }
    let foreground = (0..grid.pixels.len())
        .filter(|&index| grid.pixels[index] == 1)
        .collect::<Vec<_>>();
    charge(remaining_work, foreground.len().max(1), "skeleton pruning")?;
    let crossing = |grid: &Grid, index: usize| counts(&grid.ring(index)).1;
    let mut on_path = vec![false; grid.pixels.len()];
    let mut removals = Vec::new();
    for &end in foreground
        .iter()
        .filter(|&&index| crossing(grid, index) == 1)
    {
        let mut path = vec![end];
        on_path[end] = true;
        let (mut current, mut length, mut reached) = (end, 0.0, false);
        loop {
            let options = OFFSETS
                .iter()
                .map(|&offset| (offset, grid.neighbor(current, offset)))
                .filter(|&(_, index)| grid.pixels[index] == 1 && !on_path[index])
                .collect::<Vec<_>>();
            if let Some(&(offset, _)) = options
                .iter()
                .find(|&&(_, index)| crossing(grid, index) >= 3)
            {
                length += step_length(offset);
                reached = true;
                break;
            }
            let Some(&(offset, next)) = options
                .iter()
                .find(|(offset, _)| offset.0 == 0 || offset.1 == 0)
                .or_else(|| options.first())
            else {
                break;
            };
            length += step_length(offset);
            on_path[next] = true;
            path.push(next);
            current = next;
            if crossing(grid, next) == 1 || length >= minimum_pixels {
                break;
            }
        }
        for &index in &path {
            on_path[index] = false;
        }
        charge(remaining_work, path.len(), "skeleton branch tracing")?;
        if reached && length < minimum_pixels {
            removals.extend(path);
        }
    }
    for index in removals {
        grid.pixels[index] = 0;
    }
    Ok(())
}

fn summarize(grid: &Grid, pixel_um: f64) -> Skeleton {
    let foreground = (0..grid.pixels.len())
        .filter(|&index| grid.pixels[index] == 1)
        .collect::<Vec<_>>();
    let (mut orthogonal, mut diagonal, mut end_points) = (0_usize, 0_usize, 0_usize);
    let mut junction = vec![false; grid.pixels.len()];
    for &index in &foreground {
        let ring = grid.ring(index);
        let [north, north_east, east, _, _, _, west, north_west] = ring;
        orthogonal += usize::from(north) + usize::from(east);
        diagonal += usize::from(north_east == 1 && north == 0 && east == 0)
            + usize::from(north_west == 1 && north == 0 && west == 0);
        match counts(&ring).1 {
            1 => end_points += 1,
            3.. => junction[index] = true,
            _ => {}
        }
    }
    let mut branch_points = 0;
    let mut seen = vec![false; grid.pixels.len()];
    for &start in foreground.iter().filter(|&&index| junction[index]) {
        if seen[start] {
            continue;
        }
        branch_points += 1;
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(index) = stack.pop() {
            for offset in OFFSETS {
                let next = grid.neighbor(index, offset);
                if junction[next] && !seen[next] {
                    seen[next] = true;
                    stack.push(next);
                }
            }
        }
    }
    Skeleton {
        length_um: (orthogonal as f64 + std::f64::consts::SQRT_2 * diagonal as f64) * pixel_um,
        branch_points,
        end_points,
    }
}
