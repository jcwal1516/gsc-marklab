# Task contract — EVO-PHYLO-ASSOC-01 restricted phylogenetic–spatial association

Status: complete

Date: 2026-08-25

Parent requirements: Part XI §101.2, CLN-01D, WS-81.

`marklab longitudinal phylogenetic-spatial-association` consumes an imported finite undirected tree
with unique positive-length edges and a provenance label, plus at least three clone records. Each
clone has a unique ID, unique tree node, finite physical 3-D centroid, and explicit patient/specimen
block. Only within-block clone pairs enter the prespecified Pearson correlation between tree path
distance and Euclidean spatial distance. The tree must be connected and acyclic; every clone node
must exist; both distance vectors must have positive variation.

The null independently permutes clone-to-tree-node assignments within each exact patient/specimen
block using a named ChaCha20 seed namespace. Singleton blocks remain fixed, but at least one block
must contain two or more clones. The result retains every observed pair, every null statistic,
inclusive plus-one two-sided p-value, exact tree/pair/permutation work bounds, and explicit
cross-sectional/noncausal language. A four-clone path with collinear unit-spaced centroids has
observed correlation one and must replay byte-identically. This does not infer migration direction,
ancestral locations, clone dynamics, or causality.
