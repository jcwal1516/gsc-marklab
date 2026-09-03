import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
import zipfile


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_crc_spatial_phenotype_outcome.py"
)


def load_module():
    spec = importlib.util.spec_from_file_location("crc_spatial_phenotype_outcome", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def workbook_bytes():
    target = io.BytesIO()
    with zipfile.ZipFile(target, "w") as archive:
        archive.writestr(
            "xl/workbook.xml",
            """<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
                 xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
                 <sheets><sheet name="Clinical" sheetId="1" r:id="rId1"/></sheets></workbook>""",
        )
        archive.writestr(
            "xl/_rels/workbook.xml.rels",
            """<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
                 <Relationship Id="rId1" Target="worksheets/sheet1.xml" Type="worksheet"/>
                 </Relationships>""",
        )
        archive.writestr(
            "xl/sharedStrings.xml",
            """<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                 <si><t>patient</t></si><si><t>PFI</t></si><si><t>PFI.time</t></si>
                 <si><t>TCGA-AA-0001</t></si></sst>""",
        )
        archive.writestr(
            "xl/worksheets/sheet1.xml",
            """<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                 <sheetData>
                 <row r="1"><c r="B1" t="s"><v>0</v></c><c r="C1" t="s"><v>1</v></c><c r="D1" t="s"><v>2</v></c></row>
                 <row r="2"><c r="B2" t="s"><v>3</v></c><c r="C2"><v>1</v></c><c r="D2"><v>365</v></c></row>
                 </sheetData></worksheet>""",
        )
    return target.getvalue()


def duplicate_header_workbook_bytes():
    raw = workbook_bytes()
    source = zipfile.ZipFile(io.BytesIO(raw))
    target = io.BytesIO()
    with zipfile.ZipFile(target, "w") as archive:
        for name in source.namelist():
            value = source.read(name)
            if name == "xl/sharedStrings.xml":
                value = value.replace(b"<si><t>PFI.time</t></si>", b"<si><t>PFI</t></si>")
            archive.writestr(name, value)
    return target.getvalue()


class CrcSpatialPhenotypeOutcomeTest(unittest.TestCase):
    def test_xlsx_reader_preserves_sparse_column_positions(self):
        module = load_module()
        rows = module.read_xlsx_sheet_bytes(workbook_bytes(), "Clinical")
        self.assertEqual(
            rows,
            [{"patient": "TCGA-AA-0001", "PFI": "1", "PFI.time": "365"}],
        )

    def test_crc_location_is_bounded_to_prespecified_groups(self):
        module = load_module()
        self.assertEqual(module.classify_crc_location("Ascending colon", "TCGA-COAD"), "right")
        self.assertEqual(module.classify_crc_location("Sigmoid colon", "TCGA-COAD"), "left")
        self.assertEqual(module.classify_crc_location("Rectum, NOS", "TCGA-READ"), "rectum")
        self.assertEqual(module.classify_crc_location("Not Reported", "TCGA-COAD"), "unavailable")

    def test_tcga_locations_reject_duplicate_or_missing_case_identity(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "locations.json"
            path.write_text(
                json.dumps(
                    {
                        "data": {
                            "hits": [
                                {"case_id": "case-1", "diagnoses": []},
                                {"id": "case-1", "diagnoses": []},
                            ]
                        }
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "duplicate case identity"):
                module._tcga_locations(path)

            path.write_text(
                json.dumps({"data": {"hits": [{"diagnoses": []}]}}),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "missing case identity"):
                module._tcga_locations(path)

    def test_unique_rows_reject_duplicate_patient_labels(self):
        module = load_module()
        with self.assertRaisesRegex(ValueError, "TCGA labels has an empty or duplicate patient_id"):
            module._unique(
                [
                    {"patient_id": "P1", "class_name": "MSI"},
                    {"patient_id": "P1", "class_name": "MSS"},
                ],
                "patient_id",
                "TCGA labels",
            )

    def test_xlsx_reader_disambiguates_duplicate_source_headers(self):
        module = load_module()
        rows = module.read_xlsx_sheet_bytes(duplicate_header_workbook_bytes(), "Clinical")
        self.assertEqual(rows[0]["PFI"], "1")
        self.assertEqual(rows[0]["PFI__2"], "365")

    def test_stanford_roi_retains_unavailable_cross_compartment_states(self):
        module = load_module()
        points = [(float(index), 0.0, 80.0, "tumor") for index in range(20)]
        result = module._roi_ecology(points)
        self.assertIsNotNone(result["tumor_tumor_proximity"])
        self.assertIsNone(result["tumor_immune_proximity"])
        self.assertIsNone(result["tumor_stromal_proximity"])

    def test_cox_lane_excludes_nonpositive_followup_explicitly(self):
        module = load_module()
        rows = [
            {"os_time": time, "os_event": event, "phenotype": phenotype}
            for time, event, phenotype in zip(
                [0.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0],
                [1, 0, 1, 0, 1, 0, 1, 0, 1, 0],
                [2.0, 0.0, -0.5, 1.5, 1.0, -2.0, -1.0, -1.5, 0.5, 0.0],
            )
        ]
        result = module._safe_cox(rows, "os_time", "os_event", "phenotype", [])
        self.assertEqual(result["excluded_nonpositive_time_count"], 1)
        self.assertEqual(result["patient_count"], 9)
        self.assertEqual(result["event_count"], 4)

    def test_attribute_matrix_preserves_patient_level_clinical_values(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "clinical.tsi"
            path.write_text(
                "attrib_name\tP1\tP2\nAge\t729\t\nGender\tMale\tFemale\nStage\tStage III\tStage II\n",
                encoding="utf-8",
            )
            rows = module.read_attribute_matrix(path)
        self.assertEqual(rows["P1"]["Age"], "729")
        self.assertEqual(rows["P2"]["Age"], "")
        self.assertEqual(rows["P2"]["Gender"], "Female")


if __name__ == "__main__":
    unittest.main()
