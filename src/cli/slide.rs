use std::{fs, path::PathBuf};

use crate::{MarklabError, RegionRequest, Result, SlideOpenOptions, SlideReader};

const MAX_CLI_REGION_PIXELS: u64 = 16_777_216;

#[derive(clap::Args, Debug)]
pub struct ExtractRequest {
    /// Whole-slide image to inspect or extract from.
    pub slide: PathBuf,
    /// Image scene index to inspect or extract.
    #[arg(long, default_value_t = 0)]
    pub scene: usize,
    /// Image series index to extract.
    #[arg(long, default_value_t = 0)]
    pub series: usize,
    /// Image pyramid level to extract.
    #[arg(long, default_value_t = 0)]
    pub level: u32,
    /// Image depth index to extract.
    #[arg(long, default_value_t = 0)]
    pub z: u32,
    /// Image channel index to extract.
    #[arg(long, default_value_t = 0)]
    pub c: u32,
    /// Image time index to extract.
    #[arg(long, default_value_t = 0)]
    pub t: u32,
    /// Horizontal origin of the extracted image region.
    #[arg(long)]
    pub x: u64,
    /// Vertical origin of the extracted image region.
    #[arg(long)]
    pub y: u64,
    /// Width of the extracted region in pixels.
    #[arg(long)]
    pub width: u32,
    /// Height of the extracted region in pixels.
    #[arg(long)]
    pub height: u32,
    /// Destination for the generated output.
    #[arg(long)]
    pub output: PathBuf,
    /// Allow replacement of an existing extraction output.
    #[arg(long)]
    pub force: bool,
}

pub fn inspect(slide_path: PathBuf, output: Option<PathBuf>) -> Result<()> {
    let slide = SlideReader::open(&slide_path)?;
    let json = serde_json::to_string_pretty(slide.metadata())?;
    if let Some(output) = output {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output, json)?;
    } else {
        println!("{json}");
    }
    Ok(())
}

pub fn extract(request: ExtractRequest) -> Result<()> {
    if request.output.exists() && !request.force {
        return Err(MarklabError::Validation(format!(
            "refusing to overwrite existing output {}; pass --force to replace it",
            request.output.display()
        )));
    }
    let slide = SlideReader::open_with_options(
        &request.slide,
        SlideOpenOptions {
            max_region_pixels: MAX_CLI_REGION_PIXELS,
        },
    )?;
    let region_request = RegionRequest {
        scene: request.scene,
        series: request.series,
        level: request.level,
        plane: crate::PlaneSelection {
            z: request.z,
            c: request.c,
            t: request.t,
        },
        x: request.x,
        y: request.y,
        width: request.width,
        height: request.height,
    };
    let region = slide.read_region_rgba(&region_request)?;
    if let Some(parent) = request.output.parent() {
        fs::create_dir_all(parent)?;
    }
    image::save_buffer_with_format(
        &request.output,
        &region.pixels,
        region.width,
        region.height,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )?;
    Ok(())
}
