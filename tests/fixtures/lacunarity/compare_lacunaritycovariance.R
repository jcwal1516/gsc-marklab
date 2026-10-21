#!/usr/bin/env Rscript
# Compare `marklab numerics lacunarity-fractal-dimension` with lacunaritycovariance::gblemp and
# an independent R box count on masks rasterized from public cells. Run from the repository
# root. lacunaritycovariance (GPL) runs only as an external oracle.
# R_LIBS=/path/to/library Rscript tests/fixtures/lacunarity/compare_lacunaritycovariance.R \
#   /path/to/marklab /path/to/CRC_clusters_neighborhoods_markers.csv /path/to/new/output
#
# Fixed before any comparison was run (2026-10-10):
# * Data: Schurch et al. (2020, CC BY 4.0), the first ten TMA A spots by file name; tumor cells,
#   CD8+ T cells, B cells and CD68+CD163+ macrophages. Coordinates are in the published
#   1920 x 1440 pixel frame of 0.377442 um pixels, frame edges at -0.5 and 1919.5 / 1439.5.
# * Mask: 8 x 8 published pixels per mask pixel (3.019536 um), 240 columns by 180 rows; a mask
#   pixel is foreground when it holds at least one cell of the type. Row 0 is the smallest y.
# * Box sizes: 1, 3, 5, 9, 15, 27 and 45 mask pixels; gblemp uses only odd pixel widths, so
#   these are exactly the widths both sides evaluate. Box counting tiles from row 0, column 0.
# * Reference: gblemp gives mean(M^2) / mean(M)^2 over boxes wholly inside the image, with M
#   an area; its mean area divided by the mask pixel area is compared with Marklab's mean mass
#   in pixels. Box counts come from integer division of each foreground pixel's indices,
#   independently of Marklab's summed-area table; the dimension from lm(log N ~ log s).
# * Revision after run 01 (no tolerance changed): R^2 is undefined when the occupied-box count
#   is the same at every scale, where lm reports rounding noise with an "essentially perfect
#   fit" warning; such cases now require Marklab to report R^2 as unavailable.
# * Tolerances: identical occupied-box counts; lacunarity and mean mass within 1e-12 scaled
#   error max(1, |reference|); dimension, intercept and R^2 within 1e-10 scaled. A scale with
#   no foreground must be unavailable on both sides.

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 3L) stop("expected Marklab executable, Schurch CSV and a new output directory")
executable <- normalizePath(args[[1]], mustWork = TRUE)
csv <- normalizePath(args[[2]], mustWork = TRUE)
output <- args[[3]]
if (dir.exists(output) || file.exists(output)) stop("output path must not exist")
pins <- utils::read.delim("tests/fixtures/lacunarity/reference-packages.tsv",
                          colClasses = "character", check.names = FALSE)
if (as.character(getRversion()) != pins$version[pins$package == "R"]) stop("R version differs")
for (row in which(pins$package != "R")) {
  observed <- unname(utils::packageDescription(pins$package[row], fields = "Version"))
  if (!identical(observed, pins$version[row])) {
    stop("reference package ", pins$package[row], " must be ", pins$version[row])
  }
}
suppressPackageStartupMessages(library(lacunaritycovariance))
dir.create(output, recursive = TRUE)
tolerances <- list(lacunarity_scaled = 1e-12, regression_scaled = 1e-10)
scaled <- function(observed, reference) abs(observed - reference) / max(1, abs(reference))

pixel_um <- 8 * 0.377442
columns <- 240L
rows <- 180L
sizes <- c(1L, 3L, 5L, 9L, 15L, 27L, 45L)
cells <- utils::read.csv(csv, check.names = FALSE)
cells <- cells[cells[["TMA_AB"]] == "A", ]
spots <- sort(unique(cells[["File Name"]]))[1:10]
types <- c("tumor cells", "CD8+ T cells", "B cells", "CD68+CD163+ macrophages")

results <- list()
for (spot in spots) for (type in types) {
  here <- cells[cells[["File Name"]] == spot & cells[["ClusterName"]] == type, ]
  column <- floor((here[["X:X"]] + 0.5) / 8)
  row <- floor((here[["Y:Y"]] + 0.5) / 8)
  mask <- matrix(0L, nrow = rows, ncol = columns)
  inside <- column >= 0 & column < columns & row >= 0 & row < rows
  mask[cbind(row[inside] + 1L, column[inside] + 1L)] <- 1L
  case <- paste(spot, type)
  stem <- gsub("[^A-Za-z0-9]+", "-", case)
  request <- list(
    format = "marklab.lacunarity_fractal_dimension_request", version = 1L,
    mask_id = case, pixel_size_um = pixel_um,
    foreground_definition = paste("at least one", type, "per 8 x 8 published pixels"),
    rows = apply(mask, 1, paste, collapse = ""), box_sizes_px = sizes,
    limits = list(maximum_pixels = 100000L, maximum_box_sizes = 16L))
  input <- file.path(output, paste0(stem, "-input.json"))
  result_path <- file.path(output, paste0(stem, "-result.json"))
  jsonlite::write_json(request, input, auto_unbox = TRUE, digits = NA)
  status <- system2(executable, c("numerics", "lacunarity-fractal-dimension", "--input",
                                  shQuote(input), "--out", shQuote(result_path)))
  if (status != 0L) stop(case, ": Marklab failed")
  marklab <- jsonlite::read_json(result_path)

  image <- spatstat.geom::im(mask, xcol = (seq_len(columns) - 0.5) * pixel_um,
                             yrow = (seq_len(rows) - 0.5) * pixel_um)
  gbl <- as.data.frame(gblemp(sizes * pixel_um, image))
  foreground <- which(mask == 1L, arr.ind = TRUE) - 1L
  occupied <- vapply(sizes, function(s) {
    nrow(unique(foreground %/% s))
  }, 0L)
  number <- function(value) if (is.null(value)) NA_real_ else value
  checks <- lapply(seq_along(sizes), function(k) {
    scale <- marklab$scales[[k]]
    reference_gbl <- if (is.finite(gbl$GBL[k])) gbl$GBL[k] else NA_real_
    reference_mean <- gbl$xbar[k] / pixel_um^2
    available_agree <- is.na(reference_gbl) == is.null(scale$lacunarity)
    list(size = sizes[k], width_agrees = isTRUE(all.equal(gbl$s[k], sizes[k] * pixel_um)),
         occupied_marklab = scale$occupied_boxes, occupied_reference = occupied[k],
         lacunarity_reference = reference_gbl,
         lacunarity_error = if (is.na(reference_gbl)) 0 else
           scaled(number(scale$lacunarity), reference_gbl),
         mean_mass_error = scaled(number(scale$mean_mass), reference_mean),
         available_agree = available_agree)
  })
  usable <- occupied > 0
  regression <- if (sum(usable) >= 2L) {
    fit <- stats::lm(log(occupied[usable]) ~ log(sizes[usable]))
    # R^2 = 1 - RSS / TSS is undefined for constant counts; lm then reports rounding noise.
    constant <- length(unique(occupied[usable])) == 1L
    c(dimension = -unname(coef(fit)[2]), intercept = unname(coef(fit)[1]),
      r_squared = if (constant) NA_real_ else summary(fit)$r.squared)
  } else c(dimension = NA_real_, intercept = NA_real_, r_squared = NA_real_)
  regression_errors <- c(
    dimension = scaled(number(marklab$box_counting_dimension), regression[["dimension"]]),
    intercept = scaled(number(marklab$regression_intercept), regression[["intercept"]]),
    r_squared = scaled(number(marklab$regression_r_squared), regression[["r_squared"]]))
  regression_passed <- if (is.na(regression[["dimension"]])) {
    is.null(marklab$box_counting_dimension)
  } else if (is.na(regression[["r_squared"]])) {
    is.null(marklab$regression_r_squared) &&
      all(is.finite(regression_errors[1:2])) &&
      all(regression_errors[1:2] <= tolerances$regression_scaled)
  } else all(is.finite(regression_errors)) &&
    all(regression_errors <= tolerances$regression_scaled)
  scale_passed <- all(vapply(checks, function(c) {
    c$width_agrees && c$available_agree && c$occupied_marklab == c$occupied_reference &&
      is.finite(c$lacunarity_error) && c$lacunarity_error <= tolerances$lacunarity_scaled &&
      is.finite(c$mean_mass_error) && c$mean_mass_error <= tolerances$lacunarity_scaled
  }, FALSE))
  passed <- scale_passed && regression_passed && marklab$foreground_pixels == sum(mask)
  results[[length(results) + 1L]] <- list(
    case = case, cells = nrow(here), foreground_pixels = sum(mask),
    dimension = regression[["dimension"]], r_squared = regression[["r_squared"]],
    regression_errors = as.list(regression_errors), scales = checks, passed = passed)
  cat(case, "pixels", sum(mask), "D", format(regression[["dimension"]], digits = 4),
      "GBL(1)", format(checks[[1]]$lacunarity_reference, digits = 4), "passed", passed, "\n")
}
summary <- list(format = "marklab.lacunarity_reference_comparison", version = 1L,
                lacunaritycovariance = as.character(packageVersion("lacunaritycovariance")),
                tolerances = tolerances, cases = results,
                max_lacunarity_error = max(unlist(lapply(results, function(r)
                  vapply(r$scales, function(c) c$lacunarity_error, 0)))),
                max_mean_mass_error = max(unlist(lapply(results, function(r)
                  vapply(r$scales, function(c) c$mean_mass_error, 0)))),
                max_regression_error = max(unlist(lapply(results, function(r)
                  unlist(r$regression_errors))), na.rm = TRUE),
                passed = all(vapply(results, function(r) r$passed, FALSE)))
jsonlite::write_json(summary, file.path(output, "summary.json"), auto_unbox = TRUE, digits = NA,
                     pretty = TRUE)
cat("max errors: lacunarity", summary$max_lacunarity_error, "mean mass",
    summary$max_mean_mass_error, "regression", summary$max_regression_error, "\n")
if (!summary$passed) stop("lacunarity comparison failed; see summary.json")
