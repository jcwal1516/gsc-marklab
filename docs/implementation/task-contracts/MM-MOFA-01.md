# Task contract — MM-MOFA-01 multiview factors and missing modalities

Status: complete

Date: 2026-08-25

Owns `FitMultiviewFactorModel` and the structural/MAR branch of `InferMissingModalities` through
pinned mofapy2 0.7.4 (LGPL-3.0). `marklab multimodal mofa` fits observed training entries only,
retains ELBO/ARD/loadings/variance evidence, aligns factor order/sign, conditions held-out factors on
available views, and evaluates masked targets. MNAR sensitivity, non-Gaussian validation, hierarchy,
and real multimodal cohorts remain open.
