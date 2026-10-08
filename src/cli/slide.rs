use std::{fs, path::PathBuf};

use crate::{MarklabError, RegionRequest, Result, SlideOpenOptions, SlideReader};

const MAX_CLI_REGION_PIXELS: u64 = 16_777_216;

pub struct ExtractRequest {
    pub slide: PathBuf,
    pub region: RegionRequest,
    pub output: PathBuf,
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
    let region = slide.read_region_rgba(&request.region)?;
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
