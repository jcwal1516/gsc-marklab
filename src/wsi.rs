use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::errors::{MarklabError, Result};

pub const DEFAULT_MAX_REGION_PIXELS: u64 = 16_777_216;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlideOpenOptions {
    pub max_region_pixels: u64,
}

impl Default for SlideOpenOptions {
    fn default() -> Self {
        Self {
            max_region_pixels: DEFAULT_MAX_REGION_PIXELS,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct PlaneSelection {
    pub z: u32,
    pub c: u32,
    pub t: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RegionRequest {
    pub scene: usize,
    pub series: usize,
    pub level: u32,
    pub plane: PlaneSelection,
    pub x: u64,
    pub y: u64,
    pub width: u32,
    pub height: u32,
}

impl RegionRequest {
    /// Validate this request against known slide metadata without decoding.
    pub fn validate_for(&self, metadata: &SlideMetadata, max_region_pixels: u64) -> Result<()> {
        validate_region_request(metadata, self, max_region_pixels)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RgbaRegion {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SlideSampleType {
    Uint8,
    Uint16,
    Float32,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SlideMetadata {
    pub scenes: Vec<SlideSceneMetadata>,
    pub associated_images: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SlideSceneMetadata {
    pub index: usize,
    pub id: String,
    pub name: Option<String>,
    pub series: Vec<SlideSeriesMetadata>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SlideSeriesMetadata {
    pub index: usize,
    pub id: String,
    pub z_planes: u32,
    pub channels: u32,
    pub timepoints: u32,
    pub sample_type: SlideSampleType,
    pub levels: Vec<SlideLevelMetadata>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SlideLevelMetadata {
    pub index: u32,
    pub width: u64,
    pub height: u64,
    pub downsample: f64,
}

#[derive(Debug)]
pub struct SlideReader {
    slide: wsi_rs::Slide,
    metadata: SlideMetadata,
    max_region_pixels: u64,
}

impl SlideReader {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_options(path, SlideOpenOptions::default())
    }

    pub fn open_with_options(path: impl AsRef<Path>, options: SlideOpenOptions) -> Result<Self> {
        if options.max_region_pixels == 0 {
            return Err(MarklabError::Validation(
                "slide max_region_pixels must be greater than zero".into(),
            ));
        }
        let upstream_options = wsi_rs::SlideOpenOptions::deterministic()
            .with_max_region_pixels(options.max_region_pixels);
        let slide = wsi_rs::Slide::open_with_options(path.as_ref(), upstream_options)
            .map_err(|error| MarklabError::Slide(error.to_string()))?;
        let metadata = metadata_from_dataset(slide.dataset());
        Ok(Self {
            slide,
            metadata,
            max_region_pixels: options.max_region_pixels,
        })
    }

    pub fn metadata(&self) -> &SlideMetadata {
        &self.metadata
    }

    pub fn read_region_rgba(&self, request: &RegionRequest) -> Result<RgbaRegion> {
        request.validate_for(&self.metadata, self.max_region_pixels)?;
        let upstream = wsi_rs::RegionRequest::new(
            wsi_rs::SceneId::new(request.scene),
            wsi_rs::SeriesId::new(request.series),
            wsi_rs::LevelIdx::new(request.level),
            (
                i64::try_from(request.x).map_err(|_| {
                    MarklabError::Validation("region x coordinate exceeds i64 range".into())
                })?,
                i64::try_from(request.y).map_err(|_| {
                    MarklabError::Validation("region y coordinate exceeds i64 range".into())
                })?,
            ),
            (request.width, request.height),
        )
        .with_plane(wsi_rs::PlaneSelection::new(
            request.plane.z,
            request.plane.c,
            request.plane.t,
        ));
        let image = self
            .slide
            .read_region_rgba(&upstream)
            .map_err(|error| MarklabError::Slide(error.to_string()))?;
        let (width, height) = image.dimensions();
        if (width, height) != (request.width, request.height) {
            return Err(MarklabError::Compute(format!(
                "slide decoder returned {width}x{height}, expected {}x{}",
                request.width, request.height
            )));
        }
        let pixels = image.into_raw();
        let expected = usize::try_from(u64::from(width) * u64::from(height) * 4)
            .map_err(|_| MarklabError::Compute("RGBA buffer size overflow".into()))?;
        if pixels.len() != expected {
            return Err(MarklabError::Compute(format!(
                "slide decoder returned {} RGBA bytes, expected {expected}",
                pixels.len()
            )));
        }
        Ok(RgbaRegion {
            width,
            height,
            pixels,
        })
    }
}

fn metadata_from_dataset(dataset: &wsi_rs::Dataset) -> SlideMetadata {
    let scenes = dataset
        .scenes
        .iter()
        .enumerate()
        .map(|(scene_index, scene)| SlideSceneMetadata {
            index: scene_index,
            id: scene.id.clone(),
            name: scene.name.clone(),
            series: scene
                .series
                .iter()
                .enumerate()
                .map(|(series_index, series)| SlideSeriesMetadata {
                    index: series_index,
                    id: series.id.clone(),
                    z_planes: series.axes.z,
                    channels: series.axes.c,
                    timepoints: series.axes.t,
                    sample_type: sample_type(series.sample_type),
                    levels: series
                        .levels
                        .iter()
                        .enumerate()
                        .map(|(level_index, level)| SlideLevelMetadata {
                            index: level_index as u32,
                            width: level.dimensions.0,
                            height: level.dimensions.1,
                            downsample: level.downsample,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect();
    let mut associated_images = dataset
        .associated_images
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    associated_images.sort();
    SlideMetadata {
        scenes,
        associated_images,
    }
}

fn sample_type(sample_type: wsi_rs::SampleType) -> SlideSampleType {
    match sample_type {
        wsi_rs::SampleType::Uint8 => SlideSampleType::Uint8,
        wsi_rs::SampleType::Uint16 => SlideSampleType::Uint16,
        wsi_rs::SampleType::Float32 => SlideSampleType::Float32,
        _ => SlideSampleType::Unknown,
    }
}

fn validate_region_request(
    metadata: &SlideMetadata,
    request: &RegionRequest,
    max_region_pixels: u64,
) -> Result<()> {
    if request.width == 0 || request.height == 0 {
        return Err(MarklabError::Validation(
            "region width and height must be greater than zero".into(),
        ));
    }
    let pixels = u64::from(request.width)
        .checked_mul(u64::from(request.height))
        .ok_or_else(|| MarklabError::Validation("region pixel count overflow".into()))?;
    if pixels > max_region_pixels {
        return Err(MarklabError::Validation(format!(
            "region contains {pixels} pixels, exceeding limit {max_region_pixels}"
        )));
    }
    let scene = metadata.scenes.get(request.scene).ok_or_else(|| {
        MarklabError::Validation(format!("scene index {} is out of range", request.scene))
    })?;
    let series = scene.series.get(request.series).ok_or_else(|| {
        MarklabError::Validation(format!("series index {} is out of range", request.series))
    })?;
    let level = series.levels.get(request.level as usize).ok_or_else(|| {
        MarklabError::Validation(format!("level index {} is out of range", request.level))
    })?;
    for (name, index, extent) in [
        ("z", request.plane.z, series.z_planes),
        ("c", request.plane.c, series.channels),
        ("t", request.plane.t, series.timepoints),
    ] {
        if index >= extent {
            return Err(MarklabError::Validation(format!(
                "{name} index {index} is out of range for extent {extent}"
            )));
        }
    }
    if series.sample_type != SlideSampleType::Uint8 {
        return Err(MarklabError::UnsupportedSlideSampleType(format!(
            "{:?}; only uint8 is supported for RGBA extraction",
            series.sample_type
        )));
    }
    let end_x = request
        .x
        .checked_add(u64::from(request.width))
        .ok_or_else(|| MarklabError::Validation("region x extent overflow".into()))?;
    let end_y = request
        .y
        .checked_add(u64::from(request.height))
        .ok_or_else(|| MarklabError::Validation("region y extent overflow".into()))?;
    if end_x > level.width || end_y > level.height {
        return Err(MarklabError::Validation(format!(
            "region [{}, {}) x [{}, {}) extends beyond level dimensions {}x{}",
            request.x, end_x, request.y, end_y, level.width, level.height
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(sample_type: SlideSampleType) -> SlideMetadata {
        SlideMetadata {
            scenes: vec![SlideSceneMetadata {
                index: 0,
                id: "scene".into(),
                name: None,
                series: vec![SlideSeriesMetadata {
                    index: 0,
                    id: "series".into(),
                    z_planes: 1,
                    channels: 1,
                    timepoints: 1,
                    sample_type,
                    levels: vec![SlideLevelMetadata {
                        index: 0,
                        width: 100,
                        height: 80,
                        downsample: 1.0,
                    }],
                }],
            }],
            associated_images: Vec::new(),
        }
    }

    fn request() -> RegionRequest {
        RegionRequest {
            scene: 0,
            series: 0,
            level: 0,
            plane: PlaneSelection::default(),
            x: 10,
            y: 10,
            width: 20,
            height: 20,
        }
    }

    fn assert_invalid_contains(metadata: &SlideMetadata, request: &RegionRequest, expected: &str) {
        let error = request
            .validate_for(metadata, 16_777_216)
            .expect_err("request should be rejected before decoding");
        assert!(
            error.to_string().contains(expected),
            "expected {expected:?} in {error}"
        );
    }

    #[test]
    fn validates_all_indices_bounds_dimensions_and_limits_before_decode() {
        let metadata = metadata(SlideSampleType::Uint8);
        assert!(request().validate_for(&metadata, 400).is_ok());

        let mut invalid = request();
        invalid.scene = 1;
        assert_invalid_contains(&metadata, &invalid, "scene index 1");
        let mut invalid = request();
        invalid.series = 1;
        assert_invalid_contains(&metadata, &invalid, "series index 1");
        let mut invalid = request();
        invalid.level = 1;
        assert_invalid_contains(&metadata, &invalid, "level index 1");
        let mut invalid = request();
        invalid.plane.z = 1;
        assert_invalid_contains(&metadata, &invalid, "z index 1");
        let mut invalid = request();
        invalid.plane.c = 1;
        assert_invalid_contains(&metadata, &invalid, "c index 1");
        let mut invalid = request();
        invalid.plane.t = 1;
        assert_invalid_contains(&metadata, &invalid, "t index 1");
        let mut invalid = request();
        invalid.width = 0;
        assert_invalid_contains(&metadata, &invalid, "greater than zero");
        let mut invalid = request();
        invalid.height = 0;
        assert_invalid_contains(&metadata, &invalid, "greater than zero");
        let mut invalid = request();
        invalid.x = 90;
        assert_invalid_contains(&metadata, &invalid, "extends beyond level dimensions");
        let mut invalid = request();
        invalid.y = 70;
        assert_invalid_contains(&metadata, &invalid, "extends beyond level dimensions");
        let mut invalid = request();
        invalid.x = u64::MAX;
        assert_invalid_contains(&metadata, &invalid, "x extent overflow");
        let mut invalid = request();
        invalid.y = u64::MAX;
        assert_invalid_contains(&metadata, &invalid, "y extent overflow");

        let limit_error = validate_region_request(&metadata, &request(), 399)
            .expect_err("pixel cap should be enforced before decoding");
        assert!(limit_error.to_string().contains("exceeding limit 399"));
    }

    #[test]
    fn rejects_non_u8_sample_types_before_decode() {
        for sample_type in [SlideSampleType::Uint16, SlideSampleType::Float32] {
            let error = validate_region_request(&metadata(sample_type), &request(), 400)
                .expect_err("unsupported sample type");
            assert!(matches!(error, MarklabError::UnsupportedSlideSampleType(_)));
        }
    }
}
