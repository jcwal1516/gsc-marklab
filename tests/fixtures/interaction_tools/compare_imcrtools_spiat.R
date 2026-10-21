#!/usr/bin/env Rscript
# Compare `marklab numerics categorical-enrichment` with imcRtools::testInteractions (classic)
# and SPIAT::mixing_score_summary on public cells. Run from the repository root. imcRtools and
# SPIAT (GPL / Artistic) run only as external oracles.
# R_LIBS=/path/to/libraries Rscript tests/fixtures/interaction_tools/compare_imcrtools_spiat.R \
#   /path/to/marklab /path/to/CRC_clusters_neighborhoods_markers.csv /path/to/new/output
#
# Fixed before any comparison was run (2026-10-10):
# * Data: Schurch et al. (2020, CC BY 4.0), TMA A spots reg001_A to reg003_A, all cells except
#   the "dirt" cluster, labelled by ClusterName; positions are pixels x 0.377442 um in the
#   published frame (edges at -0.5 and 1919.5 / 1439.5 pixels).
# * Graph: cells within 25 um (inclusive) are neighbors. Marklab runs 9,999 permutations with
#   seed 20261010; imcRtools buildSpatialGraph(type = "expansion", threshold = 25) and
#   testInteractions(method = "classic", iter = 9999), one image per call, SerialParam with
#   RNGseed 20261010.
# * imcRtools criteria: its directed edge count is twice Marklab's undirected count; its
#   classic ct for every ordered class pair (A, B) equals Marklab's directed count / n_A
#   (directed count X_AB for A != B, 2 X_AA for A = B) within 1e-12 scaled error
#   max(1, |reference|); and its p_gt agrees with Marklab's Monte Carlo enrichment p-value
#   within 0.03 for every ordered pair. Both are one-sided label-permutation p-values of the
#   same statistic from independent streams of 9,999 draws; 0.03 is 4.2 standard deviations of
#   their difference at p = 0.5.
# * SPIAT criteria: for every ordered pair of distinct types among tumor cells, CD8+ T cells,
#   CD68+CD163+ macrophages and B cells at radius 25, Reference_target_interaction and
#   Reference_reference_interaction equal Marklab's X_RT and X_RR; Mixing_score equals
#   X_RT / X_RR and Normalised_mixing_score equals 0.5 (X_RT / X_RR) (n_R - 1) / n_T within
#   1e-12 scaled error. Where X_RR = 0 the ratio is undefined; SPIAT reports 0, and such cases
#   are counted as a convention difference, not compared.

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 3L) stop("expected Marklab executable, Schurch CSV and a new output directory")
executable <- normalizePath(args[[1]], mustWork = TRUE)
csv <- normalizePath(args[[2]], mustWork = TRUE)
output <- args[[3]]
if (dir.exists(output) || file.exists(output)) stop("output path must not exist")
pins <- utils::read.delim("tests/fixtures/interaction_tools/reference-packages.tsv",
                          colClasses = "character", check.names = FALSE)
if (as.character(getRversion()) != pins$version[pins$package == "R"]) stop("R version differs")
for (row in which(pins$package != "R")) {
  observed <- unname(utils::packageDescription(pins$package[row], fields = "Version"))
  if (!identical(observed, pins$version[row])) {
    stop("reference package ", pins$package[row], " must be ", pins$version[row])
  }
}
suppressPackageStartupMessages({
  library(SpatialExperiment); library(imcRtools); library(SPIAT); library(BiocParallel)
})
dir.create(output, recursive = TRUE)
scaled <- function(observed, reference) abs(observed - reference) / pmax(1, abs(reference))
um_per_pixel <- 0.377442
frame <- c(-0.5, 1919.5, -0.5, 1439.5) * um_per_pixel
radius <- 25
permutations <- 9999L
seed <- 20261010L
spiat_types <- c("tumor cells", "CD8+ T cells", "CD68+CD163+ macrophages", "B cells")
window <- list(type = "MultiPolygon", coordinates = list(list(list(
  c(frame[1], frame[3]), c(frame[2], frame[3]), c(frame[2], frame[4]), c(frame[1], frame[4]),
  c(frame[1], frame[3])))))
cells <- utils::read.csv(csv, check.names = FALSE)
cells <- cells[cells[["TMA_AB"]] == "A" & cells[["ClusterName"]] != "dirt", ]

imc_rows <- list(); spiat_rows <- list(); graph_rows <- list()
for (spot in c("reg001_A", "reg002_A", "reg003_A")) {
  here <- cells[cells[["File Name"]] == spot, ]
  x <- here[["X:X"]] * um_per_pixel
  y <- here[["Y:Y"]] * um_per_pixel
  label <- here[["ClusterName"]]
  classes <- sort(unique(label))
  ids <- paste0(spot, ":", here$CellID)
  request <- list(
    format = "marklab.categorical_enrichment", version = 1L,
    coordinate_frame_id = paste0(spot, "-um"), coordinate_unit = "micrometer",
    window = window,
    channel = list(id = "cell_type", label = "Schurch CODEX cell type", kind = "categorical",
                   unit = "categorical", measurement_status = "classified",
                   provenance = "Schurch et al. 2020 ClusterName; 10.17632/mpjzbtfgfr.1",
                   classes = as.list(classes)),
    cells = lapply(seq_along(x), function(i) list(id = ids[i], x_um = x[i], y_um = y[i],
                                                  class_id = label[i])),
    radius_um = radius, permutations = permutations, seed = seed,
    limits = list(maximum_input_bytes = 16L * 1024L * 1024L, maximum_points = 20000L,
                  maximum_classes = 64L, maximum_edges = 500000L,
                  maximum_permutations = 10000L, maximum_permutation_work = 2000000000,
                  maximum_retained_bytes = 512L * 1024L * 1024L))
  input <- file.path(output, paste0(spot, "-input.json"))
  result_path <- file.path(output, paste0(spot, "-result.json"))
  jsonlite::write_json(request, input, auto_unbox = TRUE, digits = NA)
  status <- system2(executable, c("numerics", "categorical-enrichment", "--input",
                                  shQuote(input), "--out", shQuote(result_path)))
  if (status != 0L) stop(spot, ": Marklab failed")
  marklab <- jsonlite::read_json(result_path)
  counts <- setNames(unlist(marklab$class_counts), unlist(marklab$classes))
  undirected <- matrix(0, length(classes), length(classes), dimnames = list(classes, classes))
  p_value <- undirected
  for (pair in marklab$pairs) {
    a <- pair$monte_carlo$label_a; b <- pair$monte_carlo$label_b
    undirected[a, b] <- undirected[b, a] <- pair$monte_carlo$observed_edges
    p_value[a, b] <- p_value[b, a] <- pair$monte_carlo$p_value
  }

  spe <- SpatialExperiment(assays = list(counts = matrix(0, nrow = 1, ncol = length(x),
                                                          dimnames = list("none", ids))),
                           colData = S4Vectors::DataFrame(sample_id = spot,
                                                          celltype = label, Cell.Type = label),
                           spatialCoords = cbind(Cell.X.Position = x, Cell.Y.Position = y))
  spe <- buildSpatialGraph(spe, img_id = "sample_id", type = "expansion", threshold = radius,
                           coords = c("Cell.X.Position", "Cell.Y.Position"))
  directed_edges <- length(colPair(spe, "expansion_interaction_graph"))
  graph_rows[[spot]] <- list(spot = spot, cells = length(x),
                             marklab_undirected = marklab$undirected_edge_count,
                             imcrtools_directed = directed_edges,
                             passed = directed_edges == 2 * marklab$undirected_edge_count)
  tested <- as.data.frame(testInteractions(
    spe, group_by = "sample_id", label = "celltype", colPairName = "expansion_interaction_graph",
    method = "classic", iter = permutations, BPPARAM = SerialParam(RNGseed = seed)))
  for (r in seq_len(nrow(tested))) {
    a <- tested$from_label[r]; b <- tested$to_label[r]
    directed <- if (a == b) 2 * undirected[a, a] else undirected[a, b]
    imc_rows[[length(imc_rows) + 1L]] <- list(
      spot = spot, from = a, to = b, ct = tested$ct[r],
      ct_error = scaled(directed / counts[[a]], tested$ct[r]),
      p_gt = tested$p_gt[r], marklab_p = p_value[a, b],
      p_difference = abs(tested$p_gt[r] - p_value[a, b]))
  }

  for (reference in intersect(spiat_types, classes)) for (target in intersect(spiat_types, classes)) {
    if (reference == target) next
    s <- mixing_score_summary(spe, reference_celltype = reference, target_celltype = target,
                              radius = radius, feature_colname = "Cell.Type")
    rt <- undirected[reference, target]; rr <- undirected[reference, reference]
    defined <- rr > 0
    mixing <- if (defined) rt / rr else NA_real_
    normalised <- if (defined) 0.5 * mixing * (counts[[reference]] - 1) / counts[[target]] else NA_real_
    passed <- s$Reference_target_interaction == rt && s$Reference_reference_interaction == rr &&
      (!defined || (scaled(mixing, s$Mixing_score) <= 1e-12 &&
                    scaled(normalised, s$Normalised_mixing_score) <= 1e-12))
    spiat_rows[[length(spiat_rows) + 1L]] <- list(
      spot = spot, reference = reference, target = target, rt = rt, rr = rr,
      spiat_rt = s$Reference_target_interaction, spiat_rr = s$Reference_reference_interaction,
      mixing = mixing, spiat_mixing = s$Mixing_score, normalised = normalised,
      spiat_normalised = s$Normalised_mixing_score, ratio_defined = defined, passed = passed)
  }
  cat(spot, "cells", length(x), "edges", marklab$undirected_edge_count, "\n")
}
imc_ct_error <- max(vapply(imc_rows, function(r) r$ct_error, 0))
imc_p_difference <- max(vapply(imc_rows, function(r) r$p_difference, 0))
imc_p_exceed <- sum(vapply(imc_rows, function(r) r$p_difference > 0.03, FALSE))
summary <- list(
  format = "marklab.interaction_tools_comparison", version = 1L,
  versions = setNames(as.list(pins$version), pins$package),
  graphs = unname(graph_rows),
  imcrtools = list(ordered_pairs = length(imc_rows), max_ct_error = imc_ct_error,
                   max_p_difference = imc_p_difference, p_differences_above_0_03 = imc_p_exceed,
                   passed = all(vapply(graph_rows, function(r) r$passed, FALSE)) &&
                     imc_ct_error <= 1e-12 && imc_p_exceed == 0L, rows = imc_rows),
  spiat = list(pairs = length(spiat_rows),
               undefined_ratio_reported_as_zero = sum(!vapply(spiat_rows, function(r) r$ratio_defined, FALSE)),
               passed = all(vapply(spiat_rows, function(r) r$passed, FALSE)), rows = spiat_rows))
jsonlite::write_json(summary, file.path(output, "summary.json"), auto_unbox = TRUE, digits = NA,
                     pretty = TRUE)
cat("imcRtools: pairs", length(imc_rows), "max ct error", imc_ct_error, "max p difference",
    imc_p_difference, "above 0.03", imc_p_exceed, "passed", summary$imcrtools$passed, "\n")
cat("SPIAT: pairs", length(spiat_rows), "undefined", summary$spiat$undefined_ratio_reported_as_zero,
    "passed", summary$spiat$passed, "\n")
if (!summary$imcrtools$passed || !summary$spiat$passed) stop("comparison failed; see summary.json")
