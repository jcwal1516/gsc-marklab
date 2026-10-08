#![no_main]

use libfuzzer_sys::fuzz_target;
use marklab::{
    PlaneSelection, RegionRequest, SlideLevelMetadata, SlideMetadata, SlideSampleType,
    SlideSceneMetadata, SlideSeriesMetadata,
};

fuzz_target!(|bytes: &[u8]| {
    if bytes.len() < 64 {
        return;
    }
    let number = |offset: usize| {
        u64::from_le_bytes(
            bytes[offset..offset + 8]
                .try_into()
                .expect("eight-byte slice"),
        )
    };
    let sample_type = match bytes[63] % 4 {
        0 => SlideSampleType::Uint8,
        1 => SlideSampleType::Uint16,
        2 => SlideSampleType::Float32,
        _ => SlideSampleType::Unknown,
    };
    let metadata = SlideMetadata {
        scenes: vec![SlideSceneMetadata {
            index: 0,
            id: "scene".into(),
            name: None,
            series: vec![SlideSeriesMetadata {
                index: 0,
                id: "series".into(),
                z_planes: u32::from(bytes[56] % 4),
                channels: u32::from(bytes[57] % 4),
                timepoints: u32::from(bytes[58] % 4),
                sample_type,
                levels: vec![SlideLevelMetadata {
                    index: 0,
                    width: number(0),
                    height: number(8),
                    downsample: 1.0,
                }],
            }],
        }],
        associated_images: Vec::new(),
    };
    let request = RegionRequest {
        scene: usize::from(bytes[59] % 3),
        series: usize::from(bytes[60] % 3),
        level: u32::from(bytes[61] % 3),
        plane: PlaneSelection {
            z: u32::from(bytes[52] % 5),
            c: u32::from(bytes[53] % 5),
            t: u32::from(bytes[54] % 5),
        },
        x: number(16),
        y: number(24),
        width: number(32) as u32,
        height: number(40) as u32,
    };
    let _ = request.validate_for(&metadata, number(48));
});
