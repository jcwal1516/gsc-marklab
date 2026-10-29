//! Rasters as SPACE inputs: label masks and marker images sampled at each cell's centroid, or a
//! grid of points taken as the cells of a specimen. Pixel (column i, row j) covers
//! `[origin + i·size, origin + (i+1)·size)` in micrometres.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{invalid, resource, SpaceError};
use crate::multiplex_study::model::{Channel, ChannelKind};

/// What a raster's samples mean.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(super) enum RasterKind {
    /// Integer labels of a categorical channel; 0 is background unless named.
    Labels,
    /// Marker intensities of a continuous channel.
    Intensity,
}

/// The channel a raster provides.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RasterChannel {
    pub channel: String,
    pub kind: RasterKind,
    /// For labels, the class of each label value, such as `{"1": "tumour"}`.
    #[serde(default)]
    pub levels: BTreeMap<String, String>,
}

impl RasterChannel {
    /// The label classes of a label raster, `None` for intensities.
    pub(super) fn classes(&self) -> Result<Option<LabelClasses>, SpaceError> {
        match self.kind {
            RasterKind::Labels => LabelClasses::new(self).map(Some),
            RasterKind::Intensity => {
                if !self.levels.is_empty() {
                    return Err(invalid(format!(
                        "intensity raster {} names classes",
                        self.channel
                    )));
                }
                Ok(None)
            }
        }
    }

    /// The channel declaration this raster stands for.
    pub(super) fn declaration(&self, classes: Option<&LabelClasses>) -> Channel {
        Channel {
            id: self.channel.clone(),
            label: self.channel.clone(),
            kind: match self.kind {
                RasterKind::Labels => ChannelKind::Categorical,
                RasterKind::Intensity => ChannelKind::Continuous,
            },
            unit: match self.kind {
                RasterKind::Labels => "categorical".into(),
                RasterKind::Intensity => "intensity".into(),
            },
            measurement_status: "raster".into(),
            provenance: "raster image".into(),
            levels: classes
                .map(|classes| classes.names.clone())
                .unwrap_or_default(),
        }
    }
}

/// A raster channel sampled at the cells of another source (request `rasters`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RasterDeclaration {
    pub channel: String,
    pub kind: RasterKind,
    #[serde(default)]
    pub levels: BTreeMap<String, String>,
    /// One image per specimen, by specimen ID.
    pub images: BTreeMap<String, SampledImage>,
}

impl RasterDeclaration {
    pub(super) fn layer(&self) -> RasterChannel {
        RasterChannel {
            channel: self.channel.clone(),
            kind: self.kind,
            levels: self.levels.clone(),
        }
    }
}

/// An image sampled at cells, with its placement in the specimen's frame.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SampledImage {
    pub path: PathBuf,
    #[serde(default)]
    pub series: usize,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub plane: ImagePlane,
    pub pixel_size_um: [f64; 2],
    #[serde(default)]
    pub origin_um: [f64; 2],
}

impl SampledImage {
    pub(super) fn reference(&self) -> ImageReference {
        ImageReference {
            path: self.path.clone(),
            series: self.series,
            level: self.level,
            plane: self.plane,
        }
    }

    pub(super) fn geometry(&self) -> PixelGeometry {
        PixelGeometry {
            pixel_size_um: self.pixel_size_um,
            origin_um: self.origin_um,
        }
    }
}

/// A specimen whose cells are the points of a pixel grid (source `raster`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RasterSpecimen {
    pub specimen_id: String,
    #[serde(default)]
    pub groups: BTreeMap<String, String>,
    pub coordinate_frame_id: String,
    pub pixel_size_um: [f64; 2],
    #[serde(default)]
    pub origin_um: [f64; 2],
    /// Every `stride_px`-th pixel in each direction becomes a point.
    #[serde(default = "one")]
    pub stride_px: usize,
    /// One image per layer, by channel; every image has the same pixel grid.
    pub images: BTreeMap<String, ImageReference>,
}

fn one() -> usize {
    1
}

/// One image file; series, level and plane select within multi-image files.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    not(feature = "wsi"),
    expect(
        dead_code,
        reason = "parsed so the request format is the same; read only with wsi"
    )
)]
pub(super) struct ImageReference {
    pub path: PathBuf,
    #[serde(default)]
    pub series: usize,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub plane: ImagePlane,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct ImagePlane {
    #[serde(default)]
    pub z: u32,
    #[serde(default)]
    pub c: u32,
    #[serde(default)]
    pub t: u32,
}

/// Where a raster's pixels lie in the specimen's micrometre frame.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct PixelGeometry {
    pub pixel_size_um: [f64; 2],
    #[serde(default)]
    pub origin_um: [f64; 2],
}

impl PixelGeometry {
    pub(super) fn validate(&self) -> Result<(), SpaceError> {
        let positive = self
            .pixel_size_um
            .iter()
            .all(|size| size.is_finite() && *size > 0.0);
        if !positive || !self.origin_um.iter().all(|origin| origin.is_finite()) {
            return Err(invalid(
                "pixel_size_um must be positive and origin_um finite",
            ));
        }
        Ok(())
    }

    /// The pixel containing a point, if it lies within `width` × `height` pixels.
    fn pixel(&self, x_um: f64, y_um: f64, width: usize, height: usize) -> Option<(usize, usize)> {
        let column = ((x_um - self.origin_um[0]) / self.pixel_size_um[0]).floor();
        let row = ((y_um - self.origin_um[1]) / self.pixel_size_um[1]).floor();
        let inside = column >= 0.0 && row >= 0.0 && column < width as f64 && row < height as f64;
        inside.then_some((column as usize, row as usize))
    }

    fn centre(&self, column: usize, row: usize) -> [f64; 2] {
        [
            self.origin_um[0] + (column as f64 + 0.5) * self.pixel_size_um[0],
            self.origin_um[1] + (row as f64 + 0.5) * self.pixel_size_um[1],
        ]
    }
}

/// One decoded image plane, row-major.
pub(super) struct Raster {
    pub width: usize,
    pub height: usize,
    pub samples: Vec<u16>,
}

impl Raster {
    fn sample(&self, column: usize, row: usize) -> u16 {
        self.samples[row * self.width + column]
    }
}

/// Decode one greyscale image plane holding at most `maximum_pixels`. PNG files are read now;
/// TIFF and slide formats arrive with the wsi-rs update.
#[cfg(feature = "wsi")]
pub(super) fn read_raster(
    path: &Path,
    image: &ImageReference,
    maximum_pixels: usize,
) -> Result<Raster, SpaceError> {
    let png = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"));
    if !png {
        return Err(invalid(format!(
            "{} is not a PNG; TIFF and slide rasters need the wsi-rs update",
            path.display()
        )));
    }
    if image.series != 0 || image.level != 0 || image.plane != ImagePlane::default() {
        return Err(invalid(format!(
            "{} is a single-plane PNG; series, level and plane must be 0",
            path.display()
        )));
    }
    let failure = |error: image::ImageError| invalid(format!("{}: {error}", path.display()));
    let reader = image::ImageReader::open(path)
        .map_err(|error| invalid(format!("{}: {error}", path.display())))?
        .with_guessed_format()
        .map_err(|error| invalid(format!("{}: {error}", path.display())))?;
    let (width, height) = reader.into_dimensions().map_err(failure)?;
    let pixels = (width as usize).saturating_mul(height as usize);
    if pixels == 0 || pixels > maximum_pixels {
        return Err(resource(format!(
            "{} has {pixels} pixels; between 1 and {maximum_pixels} are admitted",
            path.display()
        )));
    }
    let decoded = image::ImageReader::open(path)
        .map_err(|error| invalid(format!("{}: {error}", path.display())))?
        .with_guessed_format()
        .map_err(|error| invalid(format!("{}: {error}", path.display())))?
        .decode()
        .map_err(failure)?;
    let samples = match decoded {
        image::DynamicImage::ImageLuma8(buffer) => {
            buffer.into_raw().into_iter().map(u16::from).collect()
        }
        image::DynamicImage::ImageLuma16(buffer) => buffer.into_raw(),
        _ => {
            return Err(invalid(format!(
                "{} must be an 8- or 16-bit greyscale image",
                path.display()
            )))
        }
    };
    Ok(Raster {
        width: width as usize,
        height: height as usize,
        samples,
    })
}

#[cfg(not(feature = "wsi"))]
pub(super) fn read_raster(
    path: &Path,
    _image: &ImageReference,
    _maximum_pixels: usize,
) -> Result<Raster, SpaceError> {
    Err(invalid(format!(
        "reading {} needs marklab built with the wsi feature",
        path.display()
    )))
}

/// The classes of a label raster: their names in label order, and each label value's class.
pub(super) struct LabelClasses {
    pub names: Vec<String>,
    codes: BTreeMap<u16, u32>,
}

impl LabelClasses {
    pub(super) fn new(channel: &RasterChannel) -> Result<Self, SpaceError> {
        let mut by_value = BTreeMap::new();
        for (value, name) in &channel.levels {
            let label = value.parse::<u16>().map_err(|_| {
                invalid(format!(
                    "raster channel {} names label {value}; labels are integers from 0 to 65535",
                    channel.channel
                ))
            })?;
            by_value.insert(label, name.clone());
        }
        if by_value.is_empty() {
            return Err(invalid(format!(
                "label raster {} names no classes",
                channel.channel
            )));
        }
        let codes = by_value
            .keys()
            .enumerate()
            .map(|(code, &value)| (value, code as u32))
            .collect();
        Ok(Self {
            names: by_value.into_values().collect(),
            codes,
        })
    }

    /// The class code of a label, `None` for unnamed background 0.
    fn code(&self, channel: &str, label: u16) -> Result<Option<u32>, SpaceError> {
        match self.codes.get(&label) {
            Some(&code) => Ok(Some(code)),
            None if label == 0 => Ok(None),
            None => Err(invalid(format!(
                "raster channel {channel} has label {label}, which names no class"
            ))),
        }
    }
}

/// Each cell's observation of a raster channel: the class code or intensity of the pixel
/// holding its centroid. Labels outside the raster are unclassified; an intensity outside it
/// is an error, as SPACE scalars are observed on every cell.
pub(super) fn sample_at_cells(
    channel: &RasterChannel,
    classes: Option<&LabelClasses>,
    raster: &Raster,
    geometry: &PixelGeometry,
    cells: &[[f64; 2]],
    cell_ids: &[String],
) -> Result<Vec<Option<f64>>, SpaceError> {
    let mut inside = 0_usize;
    let values = cells
        .iter()
        .zip(cell_ids)
        .map(|(&[x, y], id)| {
            let Some((column, row)) = geometry.pixel(x, y, raster.width, raster.height) else {
                return match channel.kind {
                    RasterKind::Labels => Ok(None),
                    RasterKind::Intensity => Err(invalid(format!(
                        "cell {id} lies outside the {} raster",
                        channel.channel
                    ))),
                };
            };
            inside += 1;
            let sample = raster.sample(column, row);
            Ok(match classes {
                Some(classes) => classes.code(&channel.channel, sample)?.map(f64::from),
                None => Some(f64::from(sample)),
            })
        })
        .collect::<Result<Vec<_>, SpaceError>>()?;
    if inside == 0 {
        return Err(invalid(format!(
            "no cell lies within the {} raster; check pixel_size_um and origin_um",
            channel.channel
        )));
    }
    Ok(values)
}

/// Grid points of a raster specimen: every `stride`-th pixel's centre.
pub(super) struct Grid {
    pub cell_ids: Vec<String>,
    pub coordinates_um: Vec<[f64; 2]>,
    /// Each point's (column, row) in grid steps, for patch adjacency.
    pub steps: Vec<[u32; 2]>,
    /// Pixel (column, row) of each point.
    pixels: Vec<(usize, usize)>,
}

impl Grid {
    pub(super) fn new(
        width: usize,
        height: usize,
        geometry: &PixelGeometry,
        stride: usize,
        maximum_points: usize,
    ) -> Result<Self, SpaceError> {
        if stride == 0 {
            return Err(invalid("stride_px must be at least 1"));
        }
        let columns = width.div_ceil(stride);
        let rows = height.div_ceil(stride);
        let points = columns.saturating_mul(rows);
        if points > maximum_points {
            return Err(resource(format!(
                "a stride of {stride} px gives {points} grid points, more than the {maximum_points} admitted; use a larger stride_px"
            )));
        }
        let mut grid = Self {
            cell_ids: Vec::with_capacity(points),
            coordinates_um: Vec::with_capacity(points),
            steps: Vec::with_capacity(points),
            pixels: Vec::with_capacity(points),
        };
        for (step_row, row) in (0..height).step_by(stride).enumerate() {
            for (step_column, column) in (0..width).step_by(stride).enumerate() {
                grid.cell_ids.push(format!("{column}:{row}"));
                grid.coordinates_um.push(geometry.centre(column, row));
                grid.steps.push([step_column as u32, step_row as u32]);
                grid.pixels.push((column, row));
            }
        }
        Ok(grid)
    }

    /// The grid's observations of one layer.
    pub(super) fn observe(
        &self,
        channel: &RasterChannel,
        classes: Option<&LabelClasses>,
        raster: &Raster,
    ) -> Result<Vec<Option<f64>>, SpaceError> {
        self.pixels
            .iter()
            .map(|&(column, row)| {
                let sample = raster.sample(column, row);
                Ok(match classes {
                    Some(classes) => classes.code(&channel.channel, sample)?.map(f64::from),
                    None => Some(f64::from(sample)),
                })
            })
            .collect()
    }

    /// The raster rectangle as a GeoJSON window.
    pub(super) fn window(width: usize, height: usize, geometry: &PixelGeometry) -> String {
        let [x0, y0] = geometry.origin_um;
        let x1 = x0 + width as f64 * geometry.pixel_size_um[0];
        let y1 = y0 + height as f64 * geometry.pixel_size_um[1];
        serde_json::json!({"type": "MultiPolygon",
            "coordinates": [[[[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]]]})
        .to_string()
    }
}

/// R `census_image`'s binarization threshold (close to ImageJ's IsoData) for integer intensities:
/// a value is bright when at least the threshold. R bins 8-bit images over 0 to 255; here the
/// levels run to the largest value present, which gives R's threshold on 8-bit data.
pub(super) fn isodata_threshold(values: impl IntoIterator<Item = u16>) -> u32 {
    let mut histogram = vec![0_u64; usize::from(u16::MAX) + 1];
    for value in values {
        histogram[usize::from(value)] += 1;
    }
    let present = histogram
        .iter()
        .enumerate()
        .filter(|(_, count)| **count > 0)
        .map(|(value, _)| value as u32)
        .collect::<Vec<_>>();
    match present.as_slice() {
        [] => return 0,
        [only] => return *only,
        [lower, _] => return lower + 1,
        _ => {}
    }
    let top = *present.last().expect("at least three values") as usize;
    // Prefix counts and value-weighted sums of the nonzero levels; exact integers, as R's sums
    // of small integer products are.
    let mut counts = vec![0_u64; top + 2];
    let mut weighted = vec![0_u64; top + 2];
    for level in 1..=top {
        counts[level + 1] = counts[level] + histogram[level];
        weighted[level + 1] = weighted[level] + histogram[level] * level as u64;
    }
    // Mean of the levels in [low, high], NaN when none is present.
    let mean = |low: usize, high: usize| {
        if low > high {
            return f64::NAN;
        }
        let count = counts[high + 1] - counts[low];
        (weighted[high + 1] - weighted[low]) as f64 / count as f64
    };
    // R starts one above the smallest nonzero level.
    let smallest = present
        .iter()
        .find(|level| **level > 0)
        .expect("three levels");
    let mut threshold = *smallest as usize + 1;
    loop {
        let (low, high) = (mean(1, threshold - 1), mean(threshold + 1, top));
        if high.is_nan() {
            // R steps back once no level lies above the threshold.
            return threshold as u32 - 1;
        }
        if ((low + high) / 2.0).round_ties_even() == threshold as f64 {
            return threshold as u32;
        }
        threshold += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isodata_matches_r_census_image() {
        // R: the threshold loop of census_image on these 8-bit images, with and without zeros.
        assert_eq!(isodata_threshold([7, 7, 7]), 7);
        assert_eq!(isodata_threshold([0, 200, 0, 200]), 1);
        assert_eq!(isodata_threshold([0, 0, 10, 10, 12, 200, 210, 220]), 110);
        assert_eq!(isodata_threshold([0, 1, 2, 3, 250, 251, 252]), 126);
        assert_eq!(isodata_threshold([5, 6, 7, 50, 60]), 30);
        assert_eq!(
            isodata_threshold([40, 41, 42, 43, 90, 91, 92, 93, 94, 95]),
            67
        );
        assert_eq!(isodata_threshold([3, 9, 9, 9, 15]), 7);
    }

    #[test]
    fn pixels_cover_half_open_squares_from_the_origin() {
        let geometry = PixelGeometry {
            pixel_size_um: [2.0, 0.5],
            origin_um: [10.0, 0.0],
        };
        assert_eq!(geometry.pixel(10.0, 0.0, 4, 4), Some((0, 0)));
        assert_eq!(geometry.pixel(11.999, 0.49, 4, 4), Some((0, 0)));
        assert_eq!(geometry.pixel(12.0, 0.5, 4, 4), Some((1, 1)));
        assert_eq!(geometry.pixel(9.999, 0.0, 4, 4), None);
        assert_eq!(geometry.pixel(18.0, 0.0, 4, 4), None);
        assert_eq!(geometry.centre(1, 2), [13.0, 1.25]);
    }
}
