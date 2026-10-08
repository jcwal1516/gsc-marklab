import importlib.util
from pathlib import Path
import unittest
from unittest import mock


WORKER = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_sbi_neural_estimators_worker.py"
)
SPEC = importlib.util.spec_from_file_location("neural_sbi_worker", WORKER)
worker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(worker)


class NeuralSbiVersionTests(unittest.TestCase):
    def test_pinned_torch_distribution_accepts_platform_build_label(self):
        with (
            mock.patch.object(worker.torch, "__version__", "2.13.0+cu130"),
            mock.patch.object(worker, "version", return_value="2.13.0"),
        ):
            worker.validate_backend_versions()

    def test_torch_release_drift_is_rejected(self):
        with (
            mock.patch.object(worker.torch, "__version__", "2.12.0+cu130"),
            mock.patch.object(worker, "version", return_value="2.12.0"),
        ):
            with self.assertRaisesRegex(worker.ContractError, "backend version drift"):
                worker.validate_backend_versions()


if __name__ == "__main__":
    unittest.main()
