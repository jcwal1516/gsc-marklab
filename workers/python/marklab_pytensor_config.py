"""PyTensor settings shared by the PyMC workers. Import before compiling.

Reproducible numerics: Numba ``fastmath`` lets LLVM reorder floating-point
operations differently for freshly compiled and cached kernels, so one seed could
give different draws depending on the compile-cache state. Workers compile with
strict IEEE arithmetic instead.

Toolchain compatibility: PyTensor adds ``-ld64`` on every macOS 15+ host to select
Apple's classic linker. Toolchains without that linker read the flag as ``-l d64``,
so every C compilation fails. The flag is dropped only when a probe link with it
fails; other platforms and older toolchains keep PyTensor's flags unchanged.

Workers run with ``python -P``, so a worker appends (never prepends) its own
directory to ``sys.path`` before importing this module; installed and standard
modules keep precedence over files in the worker directory.
"""

import functools

from pytensor import config
from pytensor.link.c import cmodule

config.numba__fastmath = False

_upstream_compile_args = cmodule.GCC_compiler.compile_args


@functools.cache
def _toolchain_links_with_ld64() -> bool:
    return bool(cmodule.GCC_compiler.try_flags(["-ld64"], comp_args=False))


def _compile_args(march_flags=True):
    flags = _upstream_compile_args(march_flags)
    if "-ld64" in flags and not _toolchain_links_with_ld64():
        return [flag for flag in flags if flag != "-ld64"]
    return flags


cmodule.GCC_compiler.compile_args = staticmethod(_compile_args)
