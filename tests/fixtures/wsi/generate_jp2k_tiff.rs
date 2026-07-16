//! Reproducible generator for the single-tile JPEG2000 TIFF fixture.
//!
//! Run with:
//! `rustc generate_jp2k_tiff.rs -o /tmp/generate_jp2k_tiff &&
//!  /tmp/generate_jp2k_tiff raw/rgb_lossless.j2k tiff/rgb_lossless_jp2k.tiff`

use std::{convert::TryFrom, env, fs, io, path::Path};

fn main() -> io::Result<()> {
    let mut args = env::args_os().skip(1);
    let input = args.next().expect("input J2K codestream path");
    let output = args.next().expect("output TIFF path");
    assert!(args.next().is_none(), "expected exactly two paths");

    let codestream = fs::read(input)?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"II");
    bytes.extend_from_slice(&42_u16.to_le_bytes());
    let first_ifd_position = bytes.len();
    bytes.extend_from_slice(&0_u32.to_le_bytes());

    let tile_offset = u32::try_from(bytes.len()).expect("fixture offset fits u32");
    let tile_byte_count = u32::try_from(codestream.len()).expect("fixture size fits u32");
    bytes.extend_from_slice(&codestream);

    let ifd_offset = u32::try_from(bytes.len()).expect("fixture offset fits u32");
    bytes[first_ifd_position..first_ifd_position + 4]
        .copy_from_slice(&ifd_offset.to_le_bytes());

    let mut tags = vec![
        long_tag(256, 16),        // ImageWidth
        long_tag(257, 12),        // ImageLength
        short_tag(258, 8),        // BitsPerSample
        short_tag(259, 33_004),   // Compression: JPEG2000 RGB
        short_tag(262, 2),        // PhotometricInterpretation: RGB
        short_tag(277, 3),        // SamplesPerPixel
        long_tag(322, 16),        // TileWidth
        long_tag(323, 12),        // TileLength
        long_tag(324, tile_offset),
        long_tag(325, tile_byte_count),
    ];
    tags.sort_by_key(|entry| entry.0);
    bytes.extend_from_slice(&(tags.len() as u16).to_le_bytes());
    for (tag, field_type, count, value) in tags {
        bytes.extend_from_slice(&tag.to_le_bytes());
        bytes.extend_from_slice(&field_type.to_le_bytes());
        bytes.extend_from_slice(&count.to_le_bytes());
        bytes.extend_from_slice(&value);
    }
    bytes.extend_from_slice(&0_u32.to_le_bytes());

    if let Some(parent) = Path::new(&output).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, bytes)
}

type Tag = (u16, u16, u32, [u8; 4]);

fn long_tag(tag: u16, value: u32) -> Tag {
    (tag, 4, 1, value.to_le_bytes())
}

fn short_tag(tag: u16, value: u16) -> Tag {
    let mut encoded = [0_u8; 4];
    encoded[..2].copy_from_slice(&value.to_le_bytes());
    (tag, 3, 1, encoded)
}
