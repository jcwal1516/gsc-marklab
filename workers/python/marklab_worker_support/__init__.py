"""Code shared by the Marklab Python workers.

Marklab launches each worker as ``python -P -s -B <worker>.py``, so the worker directory is not
on ``sys.path``. A worker appends (never prepends) its own directory to ``sys.path`` and then
imports submodules of this package; installed and standard modules keep precedence.

Importing the package has no side effects. PyMC workers import
``marklab_worker_support.pytensor_config`` before ``pymc``.
"""
