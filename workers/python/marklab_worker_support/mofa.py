"""MOFA+ (mofapy2) training shared by the matrix-factor and multiview workers."""

from __future__ import annotations

import contextlib
import io
from collections.abc import Sequence
from typing import Any

import numpy as np
from mofapy2.run.entry_point import entry_point


def train_mofa(
    views: Sequence[np.ndarray],
    *,
    view_names: Sequence[str],
    group_name: str,
    sample_names: Sequence[str],
    feature_names: Sequence[Sequence[str]],
    factors: int,
    iterations: int,
    convergence_mode: str,
    seed: int,
) -> tuple[Any, str]:
    """Fit a single-group MOFA+ model by variational inference.

    Each view is a standardized (samples x features) matrix with NaN for unobserved
    entries and a Gaussian likelihood with feature-wise noise precision. Factors and
    weights have ARD (automatic relevance determination) priors without spike-and-slab
    sparsity; data are neither re-centred nor re-scaled by mofapy2. Returns the trained
    entry point and the mofapy2 log captured from stdout.
    """
    captured = io.StringIO()
    with contextlib.redirect_stdout(captured):
        model = entry_point()
        model.set_data_options(
            scale_views=False,
            scale_groups=False,
            center_groups=False,
            use_float32=False,
        )
        model.set_data_matrix(
            [[view] for view in views],
            likelihoods=["gaussian"] * len(views),
            views_names=list(view_names),
            groups_names=[group_name],
            samples_names=[list(sample_names)],
            features_names=[list(names) for names in feature_names],
        )
        model.set_model_options(
            factors=factors,
            spikeslab_factors=False,
            spikeslab_weights=False,
            ard_factors=True,
            ard_weights=True,
        )
        model.set_train_options(
            iter=iterations,
            startELBO=1,
            freqELBO=1,
            convergence_mode=convergence_mode,
            startDrop=1,
            freqDrop=1,
            dropR2=None,
            nostop=False,
            quiet=True,
            seed=seed,
            gpu_mode=False,
        )
        model.build()
        model.run()
    return model, captured.getvalue()
