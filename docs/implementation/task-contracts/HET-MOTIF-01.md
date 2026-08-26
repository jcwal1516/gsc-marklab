# Task contract — HET-MOTIF-01 typed triangle motifs

Status: complete

Date: 2026-08-25

Owns `CountTypedMotifs`, `BuildMotifAdjacency`, and `MotifNullTest` through IC-0154. The consumed
`marklab graph motif-triangle` path exhaustively enumerates bounded triangles, builds pairwise motif
adjacency, and permutes complete labels within declared strata. The one-triangle complete-tie oracle
passes. General motif catalogs, directed/weighted motifs, and real exchangeability designs remain
open.
