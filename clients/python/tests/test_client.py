import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

import anndata
import numpy as np
import pandas as pd
from scipy.sparse import csr_matrix

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location("marklab_client", ROOT / "clients/python/marklab_client.py")
client = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(client)

CHANNELS = [
    {"id":"CD3","label":"CD3","kind":"continuous","unit":"fluorescence_au","measurement_status":"measured","provenance":"assay-1"},
    {"id":"CD8","label":"CD8","kind":"continuous","unit":"fluorescence_au","measurement_status":"measured","provenance":"assay-1"},
    {"id":"positive","label":"positive","kind":"binary","unit":"unitless","measurement_status":"measured","provenance":"threshold-1"},
    {"id":"cell_type","label":"class","kind":"categorical","unit":"categorical","measurement_status":"imported_prediction","provenance":"classifier-1","levels":["immune","tumor"]},
]
WINDOW = {"type":"MultiPolygon","coordinates":[[[[-1,-1],[6,-1],[6,1],[-1,1],[-1,-1]]]]}


def data(shift=0.0):
    # Intentionally reorder rows and variables; sparse materialization must select only the panel.
    rows = [2, 0, 5, 1, 4, 3]
    cd3 = np.array([1., 2.+shift, 4., 1.+shift, 8., 4.-shift])
    cd8 = np.array([3., 1., 4.+shift, 1., 5.+shift, 9.])
    matrix = np.column_stack([np.arange(6), cd8, cd3])[rows]
    obs = pd.DataFrame({"positive": pd.array([True, False, None, True, False, True], dtype="boolean"),
                        "cell_type":pd.Categorical(["immune", "tumor", None, "immune", "tumor", "immune"])}, index=[f"c{i}" for i in range(6)]).iloc[rows]
    adata = anndata.AnnData(csr_matrix(matrix), obs=obs, var=pd.DataFrame(index=["unused", "CD8", "CD3"]))
    adata.obsm["spatial_um"] = np.column_stack([np.arange(6), np.zeros(6)])[rows]
    return adata


def slide(adata, patient=0):
    return client.anndata_slide(adata, slide_id=f"s{patient}", patient_id=f"p{patient}",
        group="a" if patient < 3 else "b", coordinate_frame_id=f"s{patient}-um",
        coordinate_unit="micrometer", spatial_key="spatial_um", window=WINDOW, channels=CHANNELS)


class ClientTests(unittest.TestCase):
    def test_real_h5ad_round_trip_preserves_rows_sparse_values_annotations_and_units(self):
        adata = data()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "panel.h5ad"
            # The admitted AnnData >=0.11 profile supports Pandas 3 nullable string indices.
            with anndata.settings.override(allow_write_nullable_strings=True):
                adata.write_h5ad(path)
            imported = slide(anndata.read_h5ad(path))
        self.assertEqual(imported["cell_ids"], [f"2:s0:c{i}" for i in range(6)])
        self.assertEqual(imported["observations"]["CD3"], [1.,2.,4.,1.,8.,4.])
        self.assertEqual(imported["observations"]["positive"], [1,0,None,1,0,1])
        self.assertEqual(imported["observations"]["cell_type"], [0,1,None,0,1,0])
        self.assertEqual(imported["coordinates_um"], [[i,0.] for i in range(6)])
        self.assertEqual(imported["window"], WINDOW)
        self.assertEqual(list(adata.var_names), ["unused", "CD8", "CD3"])

    def test_bad_units_duplicate_rows_and_oversized_materialization_are_rejected(self):
        adata = data()
        kwargs = dict(slide_id="s",patient_id="p",group="a",coordinate_frame_id="s-um",spatial_key="spatial_um",window=WINDOW,channels=CHANNELS)
        with self.assertRaisesRegex(ValueError, "micrometer"):
            client.anndata_slide(adata,coordinate_unit="pixel",**kwargs)
        with self.assertRaisesRegex(ValueError, "materialization"):
            client.anndata_slide(adata,coordinate_unit="micrometer",maximum_matrix_values=1,**kwargs)
        adata.obs_names = ["same"]*6
        with self.assertRaisesRegex(ValueError, "unique"):
            client.anndata_slide(adata,coordinate_unit="micrometer",**kwargs)

    def test_python_calls_the_native_study_and_keeps_durable_replay(self):
        recipe = {"format":"marklab.multiplex_study_recipe","version":1,"study_id":"client-test",
            "channels":CHANNELS,"slides":[slide(data(.23*i),i) for i in range(6)],
            "design":{"selected_channels":["CD3","CD8"],"radius_um":1.1,"weight_policy":"binary_symmetric","missingness":"per_channel_complete_case","patient_reduction":"equal_slide_mean","exchangeability":"independent_patients","group_a":"a","group_b":"b","permutations":99,"seed":41,"alpha":.05}}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = ROOT / "target/debug/marklab"
            self.assertIsNone(client.run_study(recipe,project=root/"project",out=root/"partial",binary=binary,through_slides=2))
            first = client.run_study(recipe,project=root/"project",out=root/"first",binary=binary)
            second = client.run_study(recipe,project=root/"project",out=root/"second",binary=binary)
            self.assertEqual(first, second)
            self.assertEqual(first["inference"]["status"], "available")
            self.assertEqual(first["maturity"]["result_maturity"], "experimental")
            self.assertEqual(len((root/"project/executions.jsonl").read_text().splitlines()), 7)
            self.assertTrue((root/"first/report.md").is_file())
            with self.assertRaisesRegex(client.MarklabError, "overwrite"):
                client.run_study(recipe,project=root/"project",out=root/"first",binary=binary)


if __name__ == "__main__":
    unittest.main()
