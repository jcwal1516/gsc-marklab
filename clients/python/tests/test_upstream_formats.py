import csv
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

import anndata
import numpy as np
import pandas as pd
from numcodecs import Blosc, VLenUTF8
from scipy.sparse import csr_matrix


CLIENT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(CLIENT_ROOT))
import marklab_client  # noqa: E402
import upstream_formats  # noqa: E402

VALIDATOR_SPEC = importlib.util.spec_from_file_location(
    "validate_steinbock",
    CLIENT_ROOT.parents[1] / "examples/public-cohorts/validate_steinbock.py",
)
validator = importlib.util.module_from_spec(VALIDATOR_SPEC)
VALIDATOR_SPEC.loader.exec_module(validator)
STHELAR_VALIDATOR_SPEC = importlib.util.spec_from_file_location(
    "validate_sthelar_cellvit",
    CLIENT_ROOT.parents[1]
    / "examples/public-cohorts/validate_sthelar_cellvit.py",
)
sthelar_validator = importlib.util.module_from_spec(STHELAR_VALIDATOR_SPEC)
STHELAR_VALIDATOR_SPEC.loader.exec_module(sthelar_validator)


CHANNELS = [
    {
        "id": "CD3",
        "label": "CD3",
        "kind": "continuous",
        "unit": "fluorescence_au",
        "measurement_status": "measured",
        "provenance": "steinbock intensities",
    },
    {
        "id": "CD8",
        "label": "CD8",
        "kind": "continuous",
        "unit": "fluorescence_au",
        "measurement_status": "measured",
        "provenance": "steinbock intensities",
    },
]
QUPATH_CLASSIFICATION = {
    "id": "nucleus_class",
    "label": "QuPath nucleus classification",
    "kind": "categorical",
    "levels": ["tumor", "lymphocyte"],
    "measurement_status": "classified",
    "provenance": "QuPath GeoJSON classification.name",
}
WINDOW = {
    "type": "MultiPolygon",
    "coordinates": [[[[-1, -1], [11, -1], [11, 21], [-1, 21], [-1, -1]]]],
}


def write_csv(path, rows):
    with path.open("w", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)


def write_geojson(path, features):
    path.write_text(json.dumps({"type": "FeatureCollection", "features": features}))


def qupath_feature(identity, ring, classification="tumor", **overrides):
    feature = {
        "type": "Feature",
        "id": identity,
        "geometry": {"type": "Polygon", "coordinates": [ring]},
        "properties": {
            "objectType": "annotation",
            "classification": {"name": classification, "color": [255, 0, 0]},
        },
    }
    feature.update(overrides)
    return feature


class UpstreamFormatTests(unittest.TestCase):
    def test_sthelar_blosc_decode_rejects_oversized_numeric_and_string_headers(self):
        with self.assertRaisesRegex(ValueError, "truncated Blosc header"):
            sthelar_validator.decode_numeric(
                b"truncated", "<f4", (1,), "test numeric"
            )

        numeric = np.arange(8, dtype="<f4").reshape(2, 4)
        numeric_encoded = Blosc(cname="lz4", clevel=5).encode(numeric)
        decoded = sthelar_validator.decode_numeric(
            numeric_encoded, "<f4", numeric.shape, "test numeric"
        )
        np.testing.assert_array_equal(decoded, numeric)
        forged_numeric = bytearray(numeric_encoded)
        forged_numeric[4:8] = (numeric.nbytes + 4).to_bytes(4, "little")
        with self.assertRaisesRegex(ValueError, "decoded-size bound"):
            sthelar_validator.decode_numeric(
                forged_numeric, "<f4", numeric.shape, "test numeric"
            )

        strings = np.array(["cell-a", "cell-b"], dtype=object)
        string_payload = VLenUTF8().encode(strings)
        string_encoded = Blosc(cname="lz4", clevel=5).encode(string_payload)
        decoded_strings = sthelar_validator.decode_strings(
            string_encoded, len(strings), "test strings"
        )
        np.testing.assert_array_equal(decoded_strings, strings)
        maximum_string_bytes = 4 + len(strings) * (
            4 + sthelar_validator.MAXIMUM_SOURCE_CELL_ID_BYTES
        )
        forged_strings = bytearray(string_encoded)
        forged_strings[4:8] = (maximum_string_bytes + 1).to_bytes(4, "little")
        with self.assertRaisesRegex(ValueError, "decoded-size bound"):
            sthelar_validator.decode_strings(
                forged_strings, len(strings), "test strings"
            )
        hostile_length_payload = (
            (1).to_bytes(4, "little") + (2**32 - 1).to_bytes(4, "little")
        )
        hostile_length_encoded = Blosc(cname="lz4", clevel=5).encode(
            hostile_length_payload
        )
        with self.assertRaisesRegex(ValueError, "oversized source cell ID"):
            sthelar_validator.decode_strings(
                hostile_length_encoded, 1, "test strings"
            )
        oversized_id = np.array(
            ["x" * (sthelar_validator.MAXIMUM_SOURCE_CELL_ID_BYTES + 1), "cell-b"],
            dtype=object,
        )
        oversized_id_encoded = Blosc(cname="lz4", clevel=5).encode(
            VLenUTF8().encode(oversized_id)
        )
        with self.assertRaisesRegex(ValueError, "oversized source cell ID"):
            sthelar_validator.decode_strings(
                oversized_id_encoded, len(oversized_id), "test strings"
            )

    def test_qupath_nuclei_preserves_ids_classes_and_scaled_area_centroids(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "nuclei.geojson"
            write_geojson(
                source,
                [
                    qupath_feature(
                        "b",
                        [[4, 0], [8, 0], [8, 2], [4, 2], [4, 0]],
                        "lymphocyte",
                    ),
                    qupath_feature(
                        "a", [[0, 0], [4, 0], [0, 2], [0, 0]], "tumor"
                    ),
                ],
            )
            slide = upstream_formats.qupath_nuclei_slide(
                source,
                slide_id="roi",
                patient_id="patient",
                group="a",
                coordinate_frame_id="roi-um",
                window=WINDOW,
                coordinate_scale_um=(0.5, 2.0),
                classification_channel=QUPATH_CLASSIFICATION,
            )
        self.assertEqual(slide["cell_ids"], ["3:roi:a", "3:roi:b"])
        self.assertAlmostEqual(slide["coordinates_um"][0][0], 2 / 3)
        self.assertAlmostEqual(slide["coordinates_um"][0][1], 4 / 3)
        self.assertEqual(slide["coordinates_um"][1], [3.0, 2.0])
        self.assertEqual(slide["observations"], {"nucleus_class": [0, 1]})

    def test_qupath_nuclei_rejects_data_outside_the_bounded_simple_polygon_profile(self):
        ring = [[0, 0], [2, 0], [2, 2], [0, 0]]
        base = qupath_feature("a", ring)
        cases = [
            ([base, qupath_feature("a", ring)], {}, "duplicate"),
            ([qupath_feature("a", ring, properties={"objectType": "cell"})], {}, "annotation"),
            (
                [qupath_feature("a", ring, geometry={"type": "Point", "coordinates": [0, 0]})],
                {},
                "simple Polygon",
            ),
            (
                [
                    qupath_feature(
                        "a",
                        ring,
                        geometry={"type": "MultiPolygon", "coordinates": [[ring]]},
                    )
                ],
                {},
                "simple Polygon",
            ),
            (
                [
                    qupath_feature(
                        "a",
                        ring,
                        geometry={"type": "Polygon", "coordinates": [ring, ring]},
                    )
                ],
                {},
                "holes",
            ),
            ([qupath_feature("a", [[0, 0], [2, 0], [2, 2], [0, 1]])], {}, "closed"),
            ([qupath_feature("a", [[0, 0], [2, 2], [0, 2], [2, 0], [0, 0]])], {}, "valid"),
            ([qupath_feature("a", [[0, 0], [float("nan"), 0], [0, 2], [0, 0]])], {}, "finite"),
            (
                [qupath_feature("a", ring, properties={"objectType": "annotation"})],
                {},
                "classification",
            ),
            ([base, qupath_feature("b", ring)], {"maximum_nuclei": 1}, "nuclei limit"),
            ([base], {"maximum_vertices": 3}, "vertex limit"),
            ([base], {"maximum_file_bytes": 1}, "byte bound"),
        ]
        for features, extra, message in cases:
            with self.subTest(message=message), tempfile.TemporaryDirectory() as directory:
                source = Path(directory) / "nuclei.geojson"
                write_geojson(source, features)
                with self.assertRaisesRegex(ValueError, message):
                    upstream_formats.qupath_nuclei_slide(
                        source,
                        slide_id="roi",
                        patient_id="patient",
                        group="a",
                        coordinate_frame_id="roi-um",
                        window=WINDOW,
                        coordinate_scale_um=(1.0, 1.0),
                        classification_channel=QUPATH_CLASSIFICATION,
                        **extra,
                    )

        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "nuclei.geojson"
            encoded = json.dumps({"type": "FeatureCollection", "features": [base]})
            source.write_text(encoded.replace(
                '"name": "tumor"', '"name": "lymphocyte", "name": "tumor"'
            ))
            with self.assertRaisesRegex(ValueError, "duplicate JSON field"):
                upstream_formats.qupath_nuclei_slide(
                    source, slide_id="roi", patient_id="patient", group="a",
                    coordinate_frame_id="roi-um", window=WINDOW,
                    coordinate_scale_um=(1.0, 1.0),
                    classification_channel=QUPATH_CLASSIFICATION,
                )

    def test_steinbock_csv_joins_objects_and_preserves_ids_axes_scale_and_values(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            intensities = root / "intensities.csv"
            regionprops = root / "regionprops.csv"
            write_csv(
                intensities,
                [
                    {"Object": "2", "CD3": "", "CD8": "0"},
                    {"Object": "1", "CD3": "3.5", "CD8": "4"},
                ],
            )
            # Steinbock/skimage centroids are row (Y), column (X).
            write_csv(
                regionprops,
                [
                    {"Object": "1", "centroid-0": "2", "centroid-1": "3"},
                    {"Object": "2", "centroid-0": "7", "centroid-1": "11"},
                ],
            )
            slide = upstream_formats.steinbock_csv_slide(
                intensities,
                regionprops,
                slide_id="roi",
                patient_id="patient",
                group="a",
                coordinate_frame_id="roi-um",
                window=WINDOW,
                channels=CHANNELS,
                coordinate_scale_um=(0.5, 2.0),
                channel_columns={"CD3": "CD3", "CD8": "CD8"},
            )
        self.assertEqual(slide["cell_ids"], ["3:roi:1", "3:roi:2"])
        self.assertEqual(slide["coordinates_um"], [[1.5, 4.0], [5.5, 14.0]])
        self.assertEqual(slide["observations"], {"CD3": [3.5, None], "CD8": [4.0, 0.0]})
        adata = anndata.AnnData(
            csr_matrix(np.asarray([[3.5, 4.0], [np.nan, 0.0]])),
            obs=pd.DataFrame(index=["1", "2"]),
            var=pd.DataFrame(index=["CD3", "CD8"]),
        )
        adata.obsm["spatial_um"] = np.asarray(slide["coordinates_um"])
        roundtrip = marklab_client.anndata_slide(
            adata,
            slide_id="roi",
            patient_id="patient",
            group="a",
            coordinate_frame_id="roi-um",
            coordinate_unit="micrometer",
            spatial_key="spatial_um",
            window=WINDOW,
            channels=CHANNELS,
        )
        self.assertEqual(roundtrip, slide)

    def test_steinbock_csv_rejects_mismatched_duplicate_or_oversized_inputs(self):
        cases = [
            (
                [{"Object": "1", "CD3": "1", "CD8": "2"}],
                [{"Object": "2", "centroid-0": "1", "centroid-1": "2"}],
                {},
                "identities differ",
            ),
            (
                [
                    {"Object": "1", "CD3": "1", "CD8": "2"},
                    {"Object": "1", "CD3": "3", "CD8": "4"},
                ],
                [{"Object": "1", "centroid-0": "1", "centroid-1": "2"}],
                {},
                "duplicate",
            ),
            (
                [{"Object": "1", "CD3": "1", "CD8": "2"}],
                [{"Object": "1", "centroid-0": "1", "centroid-1": "2"}],
                {"maximum_file_bytes": 1},
                "exceeds",
            ),
            (
                [{"Object": "1", "CD3": "1", "CD8": "2"}],
                [{"Object": "1", "x": "2", "y": "1"}],
                {},
                r"centroid-0 \(Y\).+centroid-1 \(X\)",
            ),
        ]
        for intensity_rows, regionprop_rows, extra, message in cases:
            with self.subTest(message=message), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                intensities = root / "intensities.csv"
                regionprops = root / "regionprops.csv"
                write_csv(intensities, intensity_rows)
                write_csv(regionprops, regionprop_rows)
                with self.assertRaisesRegex(ValueError, message):
                    upstream_formats.steinbock_csv_slide(
                        intensities,
                        regionprops,
                        slide_id="roi",
                        patient_id="patient",
                        group="a",
                        coordinate_frame_id="roi-um",
                        window=WINDOW,
                        channels=CHANNELS,
                        coordinate_scale_um=(1.0, 1.0),
                        channel_columns={"CD3": "CD3", "CD8": "CD8"},
                        **extra,
                    )

    def test_steinbock_csv_rejects_ambiguous_headers_and_reserved_channel_columns(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            intensities = root / "intensities.csv"
            regionprops = root / "regionprops.csv"
            intensities.write_text("Object,CD3,CD3,CD8\n1,1,2,3\n")
            write_csv(
                regionprops,
                [{"Object": "1", "centroid-0": "1", "centroid-1": "2"}],
            )
            with self.assertRaisesRegex(ValueError, "duplicate column"):
                upstream_formats.steinbock_csv_slide(
                    intensities,
                    regionprops,
                    slide_id="roi",
                    patient_id="patient",
                    group="a",
                    coordinate_frame_id="roi-um",
                    window=WINDOW,
                    channels=CHANNELS,
                    coordinate_scale_um=(1.0, 1.0),
                    channel_columns={"CD3": "CD3", "CD8": "CD8"},
                )

            intensities.write_text("Object,centroid-1,CD8\n1,99,3\n")
            with self.assertRaisesRegex(ValueError, "reserved"):
                upstream_formats.steinbock_csv_slide(
                    intensities,
                    regionprops,
                    slide_id="roi",
                    patient_id="patient",
                    group="a",
                    coordinate_frame_id="roi-um",
                    window=WINDOW,
                    channels=CHANNELS,
                    coordinate_scale_um=(1.0, 1.0),
                    channel_columns={"CD3": "centroid-1", "CD8": "CD8"},
                )

    def test_mcmicro_csv_preserves_ids_axes_scale_and_values(self):
        with tempfile.TemporaryDirectory() as directory:
            quantification = Path(directory) / "cell.csv"
            # MCQuant: X_centroid is the column (X) and Y_centroid the row (Y), in pixels.
            write_csv(
                quantification,
                [
                    {"CellID": "2", "CD3": "", "CD8": "0", "X_centroid": "11",
                     "Y_centroid": "7", "Area": "40", "Eccentricity": "0.5"},
                    {"CellID": "1", "CD3": "3.5", "CD8": "4", "X_centroid": "3",
                     "Y_centroid": "2", "Area": "35", "Eccentricity": "0.2"},
                ],
            )
            slide = upstream_formats.mcmicro_csv_slide(
                quantification,
                slide_id="roi",
                patient_id="patient",
                group="a",
                coordinate_frame_id="roi-um",
                window=WINDOW,
                channels=CHANNELS,
                coordinate_scale_um=(0.5, 2.0),
                channel_columns={"CD3": "CD3", "CD8": "CD8"},
            )
        self.assertEqual(slide["cell_ids"], ["3:roi:1", "3:roi:2"])
        self.assertEqual(slide["coordinates_um"], [[1.5, 4.0], [5.5, 14.0]])
        self.assertEqual(slide["observations"], {"CD3": [3.5, None], "CD8": [4.0, 0.0]})

    def test_mcmicro_csv_rejects_duplicates_missing_centroids_and_morphology_channels(self):
        valid = {"CellID": "1", "CD3": "1", "CD8": "2", "X_centroid": "1", "Y_centroid": "2"}
        cases = [
            ([valid, valid], {}, "duplicate CellID"),
            ([{"CellID": "1", "CD3": "1", "CD8": "2", "X_centroid": "1"}], {}, "Y_centroid"),
            ([valid], {"maximum_file_bytes": 1}, "exceeds"),
            ([{**valid, "Area": "3"}], {"channel_columns": {"CD3": "Area", "CD8": "CD8"}},
             "reserved"),
        ]
        for rows, extra, message in cases:
            with self.subTest(message=message), tempfile.TemporaryDirectory() as directory:
                quantification = Path(directory) / "cell.csv"
                write_csv(quantification, rows)
                arguments = {
                    "slide_id": "roi", "patient_id": "patient", "group": "a",
                    "coordinate_frame_id": "roi-um", "window": WINDOW, "channels": CHANNELS,
                    "coordinate_scale_um": (1.0, 1.0),
                    "channel_columns": {"CD3": "CD3", "CD8": "CD8"},
                }
                with self.assertRaisesRegex(ValueError, message):
                    upstream_formats.mcmicro_csv_slide(
                        quantification, **{**arguments, **extra}
                    )

    def test_steinbock_validator_can_reuse_preserved_source_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "source"
            source.mkdir()
            (source / "LICENSE").write_text("Attribution 4.0 International\n")
            (source / validator.ARCHIVE_NAME).write_bytes(b"archive")
            for name in validator.SOURCE_FILES:
                (source / name).write_text("header\n")
            (source / "source.json").write_text(
                json.dumps(
                    {
                        "archive_url": validator.ARCHIVE_URL,
                        "license": "CC BY 4.0",
                        "license_url": validator.LICENSE_URL,
                        "archive_bytes": 7,
                        "roi": validator.ROI,
                        "source_members": validator.SOURCE_FILES,
                    }
                )
            )
            reused, archive_bytes = validator.reuse_sources(source)
        self.assertEqual(reused, source.resolve())
        self.assertEqual(archive_bytes, 7)


if __name__ == "__main__":
    unittest.main()
