#!/usr/bin/env Rscript
# Compare `marklab numerics morisita-dispersion` with vegan::dispindmorisita on public cells.
# Run from the repository root. vegan (GPL) runs only as an external oracle.
# R_LIBS=/path/to/library Rscript tests/fixtures/morisita/compare_vegan.R \
#   /path/to/marklab /path/to/CRC_clusters_neighborhoods_markers.csv /path/to/new/output
#
# Fixed before any comparison was run (2026-10-10):
# * Data: Schurch et al. (2020, CC BY 4.0), the first ten TMA A spots by file name; in each,
#   B cells, CD8+ T cells and CD68+CD163+ macrophages with at least two cells. Coordinates are
#   pixels x 0.377442 um in the published 1920 x 1440 pixel frame.
# * Quadrats: 100 um, origin at the frame's lower-left corner; only quadrats wholly inside the
#   frame count. R assigns cells by floor((x - origin) / width) independently of Marklab.
# * Tolerances: identical quadrat counts; Morisita index and chi-square statistic within 1e-12
#   scaled error max(1, |reference|); uniform, clumped and standardized indices within 1e-8
#   scaled; the chi-square upper-tail p-value within 1e-8 absolute (R qchisq/pchisq versus the
#   Rust statrs chi-square).

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 3L) stop("expected Marklab executable, Schurch CSV and a new output directory")
executable <- normalizePath(args[[1]], mustWork = TRUE)
csv <- normalizePath(args[[2]], mustWork = TRUE)
output <- args[[3]]
if (dir.exists(output) || file.exists(output)) stop("output path must not exist")
pins <- utils::read.delim("tests/fixtures/morisita/reference-packages.tsv",
                          colClasses = "character", check.names = FALSE)
if (as.character(getRversion()) != pins$version[pins$package == "R"]) stop("R version differs")
for (row in which(pins$package != "R")) {
  observed <- unname(utils::packageDescription(pins$package[row], fields = "Version"))
  if (!identical(observed, pins$version[row])) {
    stop("reference package ", pins$package[row], " must be ", pins$version[row])
  }
}
dir.create(output, recursive = TRUE)
tolerances <- list(index_scaled = 1e-12, derived_scaled = 1e-8, p_absolute = 1e-8)
scaled <- function(observed, reference) abs(observed - reference) / max(1, abs(reference))

um_per_pixel <- 0.377442
frame <- c(-0.5, 1919.5, -0.5, 1439.5) * um_per_pixel
width <- 100
columns <- floor((frame[2] - frame[1]) / width)
rows <- floor((frame[4] - frame[3]) / width)
cells <- utils::read.csv(csv, check.names = FALSE)
cells <- cells[cells[["TMA_AB"]] == "A", ]
spots <- sort(unique(cells[["File Name"]]))[1:10]
types <- c("B cells", "CD8+ T cells", "CD68+CD163+ macrophages")
window <- list(type = "MultiPolygon", coordinates = list(list(list(
  c(frame[1], frame[3]), c(frame[2], frame[3]), c(frame[2], frame[4]), c(frame[1], frame[4]),
  c(frame[1], frame[3])))))

results <- list()
for (spot in spots) for (type in types) {
  rows_here <- cells[cells[["File Name"]] == spot & cells[["ClusterName"]] == type, ]
  if (nrow(rows_here) < 2L) next
  x <- rows_here[["X:X"]] * um_per_pixel
  y <- rows_here[["Y:Y"]] * um_per_pixel
  column <- floor((x - frame[1]) / width)
  row <- floor((y - frame[3]) / width)
  inside <- column < columns & row < rows
  counts <- matrix(0L, nrow = rows, ncol = columns)
  for (k in which(inside)) counts[row[k] + 1L, column[k] + 1L] <- counts[row[k] + 1L, column[k] + 1L] + 1L
  reference <- vegan::dispindmorisita(matrix(as.vector(t(counts)), ncol = 1))
  case <- paste(spot, type)
  stem <- gsub("[^A-Za-z0-9]+", "-", case)
  request <- list(
    format = "marklab.morisita_dispersion_request", version = 1L,
    coordinate_frame_id = paste0(spot, "-um"), window = window,
    cells = lapply(seq_along(x), function(i) list(id = paste0("c", rows_here$CellID[i]),
                                                  x_um = x[i], y_um = y[i])),
    quadrat = list(origin_um = c(frame[1], frame[3]), width_um = width),
    limits = list(maximum_cells = 100000L, maximum_candidate_quadrats = 10000L,
                  maximum_clip_vertices = 10000L))
  input <- file.path(output, paste0(stem, "-input.json"))
  result_path <- file.path(output, paste0(stem, "-result.json"))
  jsonlite::write_json(request, input, auto_unbox = TRUE, digits = NA)
  status <- system2(executable, c("numerics", "morisita-dispersion", "--input", shQuote(input),
                                  "--out", shQuote(result_path)))
  if (status != 0L) stop(case, ": Marklab failed")
  marklab <- jsonlite::read_json(result_path)
  marklab_counts <- matrix(-1L, nrow = rows, ncol = columns)
  for (q in marklab$quadrat_counts) marklab_counts[q$row + 1L, q$column + 1L] <- q$count
  value <- function(field) if (is.null(marklab[[field]])) NA_real_ else marklab[[field]]
  errors <- c(
    morisita = scaled(value("morisita_index"), reference$imor),
    chi_square = scaled(value("chi_square"), reference$imor * (sum(counts) - 1) + length(counts) - sum(counts)),
    uniform = scaled(value("uniform_index"), reference$muni),
    clumped = scaled(value("clumped_index"), reference$mclu),
    standardized = scaled(value("standardized_index"), reference$imst),
    p_value = abs(value("chi_square_upper_p_value") - reference$pchisq))
  passed <- identical(marklab_counts, counts) && marklab$included_quadrats == length(counts) &&
    all(is.finite(errors)) &&
    all(errors[c("morisita", "chi_square")] <= tolerances$index_scaled) &&
    all(errors[c("uniform", "clumped", "standardized")] <= tolerances$derived_scaled) &&
    errors[["p_value"]] <= tolerances$p_absolute
  results[[length(results) + 1L]] <- c(list(case = case, cells = length(x),
    counted = sum(counts), quadrats = length(counts), imor = reference$imor,
    imst = reference$imst, pchisq = reference$pchisq, passed = passed), as.list(errors))
  cat(case, "cells", length(x), "I", format(reference$imor, digits = 5), "passed", passed, "\n")
}
summary <- list(format = "marklab.morisita_vegan_comparison", version = 1L,
                vegan = as.character(packageVersion("vegan")), tolerances = tolerances,
                cases = results, passed = all(vapply(results, function(r) r$passed, FALSE)))
jsonlite::write_json(summary, file.path(output, "summary.json"), auto_unbox = TRUE, digits = NA,
                     pretty = TRUE)
if (!summary$passed) stop("Morisita comparison failed; see summary.json")
