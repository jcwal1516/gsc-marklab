import subprocess
import sys
import textwrap
import unittest
from pathlib import Path


WORKER_DIRECTORY = Path(__file__).resolve().parents[2] / "workers" / "python"


class NumpyroImportIsolationTest(unittest.TestCase):
    def test_numpyro_workers_import_without_pymc_or_pytensor(self):
        script = textwrap.dedent(
            f"""
            import importlib.abc
            import importlib.util
            import sys
            from pathlib import Path

            worker_directory = Path({str(WORKER_DIRECTORY)!r})
            sys.path.insert(0, str(worker_directory))

            class RejectPyMC(importlib.abc.MetaPathFinder):
                def find_spec(self, fullname, path=None, target=None):
                    if fullname in {{"pymc", "pytensor"}} or fullname.startswith(
                        ("pymc.", "pytensor.", "marklab_pymc_")
                    ):
                        raise ImportError(f"NumPyro worker imported {{fullname}}")
                    return None

            sys.meta_path.insert(0, RejectPyMC())
            workers = sorted(worker_directory.glob("marklab_numpyro*_worker.py"))
            for worker in workers:
                spec = importlib.util.spec_from_file_location(worker.stem, worker)
                module = importlib.util.module_from_spec(spec)
                spec.loader.exec_module(module)
            """
        )
        completed = subprocess.run(
            [sys.executable, "-I", "-c", script],
            check=False,
            capture_output=True,
            text=True,
            timeout=120,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)


if __name__ == "__main__":
    unittest.main()
