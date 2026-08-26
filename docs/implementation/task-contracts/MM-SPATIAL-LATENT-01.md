# MM-SPATIAL-LATENT-01 — Exact GP spatial latent factor

Own `FitSpatialLatentFactorModel` through IC-0172. The missing-command red preceded implementation.
Pinned PyMC 6.3.0 two-chain NUTS fits one Matérn-3/2 factor; a 12-region shared-field oracle recovers
three masks below RMSE 0.35 with zero divergences. Claims remain small synthetic exact-GP only.
