"""PyTensor settings shared by the PyMC workers. Import before ``pymc`` compiles anything.

Reproducible numerics: Numba ``fastmath`` lets LLVM reorder floating-point
operations differently for freshly compiled and cached kernels, so one seed could
give different draws depending on the compile-cache state. Workers compile with
strict IEEE arithmetic instead.

Compiler: PyTensor silently falls back to much slower Python implementations when it
finds no C++ compiler on ``PATH``. Workers run with a fixed system ``PATH``, so a
compiler installed elsewhere (Nix, conda, Homebrew-only GCC) is invisible to them.
Importing this module fails instead, naming ``MARKLAB_BACKEND_PATH``, which replaces
the worker ``PATH`` with the directories it lists.

Toolchain compatibility: PyTensor adds ``-ld64`` on every macOS 15+ host to select
Apple's classic linker. Toolchains without that linker read the flag as ``-l d64``,
so every C compilation fails. The flag is dropped only when a probe link with it
fails; other platforms and older toolchains keep PyTensor's flags unchanged.
"""

import functools

from pytensor import config
from pytensor.link.c import cmodule

if not config.cxx:
    raise RuntimeError(
        "PyTensor found no C++ compiler on the worker PATH; install one in /usr/bin or set "
        "MARKLAB_BACKEND_PATH to absolute directories that contain it"
    )

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
