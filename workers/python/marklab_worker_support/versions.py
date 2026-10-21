"""Exact interpreter and package versions required by the workers.

These pins mirror ``pyproject.toml`` and ``uv.lock``. Workers refuse to run under any other
version so that a request digest always identifies one numerical implementation.
"""

PYTHON = "3.12"

ARVIZ = "1.3.0"
ARVIZ_STATS = "1.3.1"
CCA_ZOO = "3.0.0"
GUDHI = "3.13.0"
H5PY = "3.16.0"
JAX = "0.11.1"
MOFAPY2 = "0.7.4"
NUMPY = "2.4.6"
NUMPYRO = "0.21.0"
POT = "0.9.7.post1"
PYMC = "6.3.0"
PYTENSOR = "3.2.4"
SBI = "0.26.1"
SCIPY = "1.18.1"
SCIKIT_IMAGE = "0.26.0"
SIMPLEITK = "2.5.5"
TORCH = "2.13.0"

PINS = {
    "arviz": ARVIZ,
    "arviz_stats": ARVIZ_STATS,
    "cca_zoo": CCA_ZOO,
    "gudhi": GUDHI,
    "h5py": H5PY,
    "jax": JAX,
    "mofapy2": MOFAPY2,
    "numpy": NUMPY,
    "numpyro": NUMPYRO,
    "pot": POT,
    "pymc": PYMC,
    "pytensor": PYTENSOR,
    "sbi": SBI,
    "scipy": SCIPY,
    "scikit_image": SCIKIT_IMAGE,
    "simpleitk": SIMPLEITK,
    "torch": TORCH,
}
