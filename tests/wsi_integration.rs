#![cfg(feature = "wsi")]

use assert_cmd::Command;
use predicates::prelude::*;

fn raw_fixture(extension: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/wsi/raw")
        .join(match extension {
            "ppm" => "rgb_nomct.ppm".to_owned(),
            "htj2k" => "rgb_lossless_htj2k.j2k".to_owned(),
            _ => format!("rgb_lossless.{extension}"),
        })
}

fn ppm_rgb_oracle() -> Vec<u8> {
    let bytes = std::fs::read(raw_fixture("ppm")).expect("PPM oracle");
    let marker = b"255\n";
    let offset = bytes
        .windows(marker.len())
        .position(|window| window == marker)
        .expect("PPM max value")
        + marker.len();
    bytes[offset..].to_vec()
}

fn rgba_oracle() -> Vec<u8> {
    ppm_rgb_oracle()
        .chunks_exact(3)
        .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
        .collect()
}

#[test]
fn slide_commands_are_present_with_wsi_feature() {
    Command::cargo_bin("mmrspace")
        .expect("binary")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("inspect-slide"))
        .stdout(predicate::str::contains("extract-region"));
}

#[test]
fn extract_region_rejects_negative_coordinates_during_cli_parsing() {
    Command::cargo_bin("mmrspace")
        .expect("binary")
        .args([
            "extract-region",
            "missing.svs",
            "--x",
            "-1",
            "--y",
            "0",
            "--width",
            "1",
            "--height",
            "1",
            "--output",
            "region.png",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unexpected argument '-1'"));
}

#[test]
fn extract_region_refuses_overwrite_before_opening_slide() {
    let dir = tempfile::tempdir().expect("tempdir");
    let output = dir.path().join("region.png");
    std::fs::write(&output, b"existing").expect("existing output");

    Command::cargo_bin("mmrspace")
        .expect("binary")
        .args([
            "extract-region",
            "missing.svs",
            "--x",
            "0",
            "--y",
            "0",
            "--width",
            "1",
            "--height",
            "1",
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("refusing to overwrite"));
}

#[test]
fn inspect_slide_reports_corrupt_or_unsupported_input() {
    let dir = tempfile::tempdir().expect("tempdir");
    let slide = dir.path().join("truncated.j2k");
    std::fs::write(&slide, [0xff, 0x4f, 0xff]).expect("truncated fixture");

    Command::cargo_bin("mmrspace")
        .expect("binary")
        .args(["inspect-slide", slide.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Slide("));
}

#[test]
fn raw_j2k_j2c_and_htj2k_metadata_full_reads_and_crops_match_oracle_exactly() {
    let expected = rgba_oracle();
    for extension in ["j2k", "j2c", "htj2k"] {
        let slide = mmrspace::SlideReader::open(raw_fixture(extension)).expect("open raw fixture");
        let level = &slide.metadata().scenes[0].series[0].levels[0];
        assert_eq!((level.width, level.height), (16, 12));

        let full = slide
            .read_region_rgba(&mmrspace::RegionRequest {
                scene: 0,
                series: 0,
                level: 0,
                plane: mmrspace::PlaneSelection::default(),
                x: 0,
                y: 0,
                width: 16,
                height: 12,
            })
            .expect("full read");
        assert_eq!(full.pixels, expected, "{extension} full read");

        let crop = slide
            .read_region_rgba(&mmrspace::RegionRequest {
                scene: 0,
                series: 0,
                level: 0,
                plane: mmrspace::PlaneSelection::default(),
                x: 3,
                y: 2,
                width: 5,
                height: 4,
            })
            .expect("cropped read");
        let expected_crop = (2_usize..6)
            .flat_map(|y| {
                let expected = &expected;
                (3_usize..8).flat_map(move |x| expected[(y * 16 + x) * 4..][..4].to_vec())
            })
            .collect::<Vec<_>>();
        assert_eq!(crop.pixels, expected_crop, "{extension} cropped read");
    }
}

#[test]
fn tiled_tiff_jpeg2000_full_read_and_crop_match_oracle_exactly() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/wsi/tiff/rgb_lossless_jp2k.tiff");
    let expected = rgba_oracle();
    let slide = mmrspace::SlideReader::open(fixture).expect("open tiled TIFF fixture");
    let level = &slide.metadata().scenes[0].series[0].levels[0];
    assert_eq!((level.width, level.height), (16, 12));

    let full = slide
        .read_region_rgba(&mmrspace::RegionRequest {
            scene: 0,
            series: 0,
            level: 0,
            plane: mmrspace::PlaneSelection::default(),
            x: 0,
            y: 0,
            width: 16,
            height: 12,
        })
        .expect("full TIFF read");
    assert_eq!(full.pixels, expected);

    let crop = slide
        .read_region_rgba(&mmrspace::RegionRequest {
            scene: 0,
            series: 0,
            level: 0,
            plane: mmrspace::PlaneSelection::default(),
            x: 3,
            y: 2,
            width: 5,
            height: 4,
        })
        .expect("cropped TIFF read");
    let expected_crop = (2_usize..6)
        .flat_map(|y| {
            let expected = &expected;
            (3_usize..8).flat_map(move |x| expected[(y * 16 + x) * 4..][..4].to_vec())
        })
        .collect::<Vec<_>>();
    assert_eq!(crop.pixels, expected_crop);
}

#[test]
fn synthetic_dicom_vl_wsi_jpeg2000_full_read_and_crop_match_oracle_exactly() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/wsi/dicom/synthetic_vl_wsi_jp2k.dcm");
    let expected = rgba_oracle();
    let slide = mmrspace::SlideReader::open(fixture).expect("open DICOM VL WSI fixture");
    let level = &slide.metadata().scenes[0].series[0].levels[0];
    assert_eq!((level.width, level.height), (16, 12));

    let full = slide
        .read_region_rgba(&mmrspace::RegionRequest {
            scene: 0,
            series: 0,
            level: 0,
            plane: mmrspace::PlaneSelection::default(),
            x: 0,
            y: 0,
            width: 16,
            height: 12,
        })
        .expect("full DICOM read");
    assert_eq!(full.pixels, expected);

    let crop = slide
        .read_region_rgba(&mmrspace::RegionRequest {
            scene: 0,
            series: 0,
            level: 0,
            plane: mmrspace::PlaneSelection::default(),
            x: 3,
            y: 2,
            width: 5,
            height: 4,
        })
        .expect("cropped DICOM read");
    let expected_crop = (2_usize..6)
        .flat_map(|y| {
            let expected = &expected;
            (3_usize..8).flat_map(move |x| expected[(y * 16 + x) * 4..][..4].to_vec())
        })
        .collect::<Vec<_>>();
    assert_eq!(crop.pixels, expected_crop);
}

#[test]
fn slide_cli_writes_pretty_metadata_json_and_exact_size_png() {
    let dir = tempfile::tempdir().expect("tempdir");
    let metadata = dir.path().join("metadata.json");
    let png = dir.path().join("region.png");

    Command::cargo_bin("mmrspace")
        .expect("binary")
        .args([
            "inspect-slide",
            raw_fixture("j2k").to_str().unwrap(),
            "--output",
            metadata.to_str().unwrap(),
        ])
        .assert()
        .success();
    let metadata_text = std::fs::read_to_string(metadata).expect("metadata JSON");
    assert!(metadata_text.contains("\n  \"scenes\""));
    let value: serde_json::Value = serde_json::from_str(&metadata_text).expect("valid JSON");
    assert_eq!(value["scenes"][0]["series"][0]["levels"][0]["width"], 16);
    assert!(
        value.get("properties").is_none(),
        "inspect-slide must not export unreviewed vendor properties"
    );

    Command::cargo_bin("mmrspace")
        .expect("binary")
        .args([
            "extract-region",
            raw_fixture("j2k").to_str().unwrap(),
            "--x",
            "3",
            "--y",
            "2",
            "--width",
            "5",
            "--height",
            "4",
            "--output",
            png.to_str().unwrap(),
        ])
        .assert()
        .success();
    let decoded = image::open(png).expect("PNG").to_rgba8();
    assert_eq!(decoded.dimensions(), (5, 4));
}

#[test]
fn inspect_slide_defaults_to_pretty_json_on_stdout() {
    Command::cargo_bin("mmrspace")
        .expect("binary")
        .args(["inspect-slide", raw_fixture("j2k").to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("\n  \"scenes\""));
}

#[test]
fn extract_region_enforces_the_cli_pixel_cap_before_decoding() {
    let dir = tempfile::tempdir().expect("tempdir");
    Command::cargo_bin("mmrspace")
        .expect("binary")
        .args([
            "extract-region",
            raw_fixture("j2k").to_str().unwrap(),
            "--x",
            "0",
            "--y",
            "0",
            "--width",
            "4096",
            "--height",
            "4097",
            "--output",
            dir.path().join("too-large.png").to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("exceeding limit 16777216"));
}

#[test]
#[ignore = "requires the checksummed public Aperio SVS and an independent OpenSlide oracle"]
fn public_aperio_jp2k_region_matches_openslide_oracle() {
    let slide_path = std::env::var_os("MMRSPACE_PUBLIC_APERIO_SVS")
        .expect("MMRSPACE_PUBLIC_APERIO_SVS must name the checksummed public fixture");
    let oracle_path = std::env::var_os("MMRSPACE_PUBLIC_APERIO_ORACLE_PNG")
        .expect("MMRSPACE_PUBLIC_APERIO_ORACLE_PNG must name the OpenSlide oracle PNG");

    let slide = mmrspace::SlideReader::open(slide_path).expect("open public Aperio fixture");
    let metadata = slide.metadata();
    let levels = &metadata.scenes[0].series[0].levels;
    assert_eq!((levels[0].width, levels[0].height), (15_374, 17_497));
    assert_eq!(levels.len(), 3);

    let actual = slide
        .read_region_rgba(&mmrspace::RegionRequest {
            scene: 0,
            series: 0,
            level: 0,
            plane: mmrspace::PlaneSelection::default(),
            x: 7_000,
            y: 8_000,
            width: 256,
            height: 256,
        })
        .expect("decode public Aperio region");
    let oracle = image::open(oracle_path)
        .expect("OpenSlide oracle PNG")
        .to_rgba8();
    assert_eq!(oracle.dimensions(), (actual.width, actual.height));

    let mut maximum_error = [0_u8; 4];
    let mut sum_error = [0_u64; 4];
    for (index, (decoded, reference)) in actual.pixels.iter().zip(oracle.as_raw()).enumerate() {
        let channel = index % 4;
        let error = decoded.abs_diff(*reference);
        maximum_error[channel] = maximum_error[channel].max(error);
        sum_error[channel] += u64::from(error);
    }
    let mean_error = sum_error.into_iter().sum::<u64>() as f64 / actual.pixels.len() as f64;
    assert!(
        maximum_error.into_iter().all(|error| error <= 24),
        "decoded pixels exceed the reviewed maximum channel error: max={maximum_error:?}, \
         mean={mean_error:.4}"
    );
    assert!(
        mean_error <= 2.5,
        "decoded pixels exceed the reviewed mean RGBA absolute error: {mean_error:.4} > 2.5"
    );
}
