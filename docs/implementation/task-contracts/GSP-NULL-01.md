# Task contract — GSP-NULL-01 restricted graph-spectrum global null

Status: complete

Date: 2026-08-25

Parent requirements: FND-06, INF-01B, GSP-01, FR-01, WS-31, WS-62,
GSP-SPECTRAL-01.

`marklab graph spectrum-null` requires one exact declared stratum per canonical node, 20–10,000
resolvable permutations, alpha, and seed. It permutes complete scalar-signal rows only within those
strata, projects every null through the fixed canonical eigensystem, and computes the same declared
band-energy curve as the observed signal. The canonical ERL owner is now `marklab-numerics`; both
embedding and graph callers reuse it. Output retains every null curve, simultaneous bounds, global
ERL p/depths, inclusive-plus-one upper-tail low-frequency p-value, and exact projection work. A
constant four-node path produces complete ties, low-band energy four, high-band energy zero, and
both p-values one.
