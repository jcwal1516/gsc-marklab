# Compare `marklab spectrum preprocess` with readBrukerFlexData 1.9.3 and MALDIquant 1.22.3.
#
# Run from the repository root with the reference packages installed in an isolated library:
#
#   R_LIBS=target/maldi-reference-rlib Rscript tests/fixtures/maldi/compare_maldiquant.R \
#       target/debug/marklab /path/to/readBrukerFlexData/inst/Examples /path/to/new/output
#
# Fixed before any comparison was run (2026-10-28):
#
# * Data: the example acquisitions in the readBrukerFlexData 1.9.3 source package,
#   2010_05_19_Gibb_C8_A1/0_A1 and 0_A2 (linear mode, 22,431 time points each). The package is
#   GPL-licensed, so its files are read in place and not copied into this repository.
# * Reader: m/z and intensity against readBrukerFlexFile() with its defaults (negative counts
#   set to zero). The high-precision-calibration example hpc/fid/0_A20/1/1SRef must be rejected.
# * Pipeline, compared after every step by running each prefix of the step list:
#   transformIntensity("sqrt"); smoothIntensity("SavitzkyGolay", halfWindowSize = 10)
#   (polynomial order 3); removeBaseline("SNIP", iterations = 20, decreasing = TRUE and FALSE);
#   calibrateIntensity("TIC"); trim(range = c(2000, 20000)).
# * Binning of the final spectrum: 6,000 bins of 3 Da from 2,000 to 20,000, against an R
#   reference that assigns points with findInterval(rightmost.closed = TRUE) on the edges
#   2000 + 3 i and sums them in m/z order.
# * Tolerances: identical point counts; m/z within 1e-9 max(1, |reference|); every intensity
#   (or bin) within 1e-9 times the largest reference intensity of that spectrum and stage.
#   MALDIquant's Savitzky-Golay coefficients for these settings are within 1e-14 of the exact
#   rational values.
#
# Changed after the first runs: Marklab replaced readBrukerFlexData's root
# (-b + sqrt(b^2 - 4 ML3 c)) / (2 ML3), which loses digits to cancellation, with the equivalent
# -2c / (b + sqrt(b^2 - 4 ML3 c)). m/z therefore differs from the reference by the reference's
# rounding error. The tolerances are unchanged.

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 3L) {
  stop("usage: compare_maldiquant.R MARKLAB EXAMPLES_DIR NEW_OUTPUT_DIR")
}
marklab <- normalizePath(args[[1L]], mustWork = TRUE)
examples <- normalizePath(args[[2L]], mustWork = TRUE)
out_dir <- args[[3L]]
if (file.exists(out_dir)) {
  stop("output directory already exists: ", out_dir)
}
dir.create(out_dir, recursive = TRUE)

suppressPackageStartupMessages({
  library(MALDIquant)
  library(readBrukerFlexData)
  library(jsonlite)
})
stopifnot(
  packageVersion("MALDIquant") == "1.22.3",
  packageVersion("readBrukerFlexData") == "1.9.3"
)

tolerance <- 1e-9
acquisitions <- c(
  "2010_05_19_Gibb_C8_A1/0_A1/1/1SLin",
  "2010_05_19_Gibb_C8_A1/0_A2/1/1SLin"
)
stage_names <- c("sqrt", "savitzky_golay", "snip_baseline", "total_ion_current", "trim")
steps_for <- function(decreasing) {
  list(
    list(step = "sqrt"),
    list(step = "savitzky_golay", half_window = 10L, polynomial_order = 3L),
    list(step = "snip_baseline", iterations = 20L, decreasing = decreasing),
    list(step = "total_ion_current"),
    list(step = "trim", min_mz = 2000, max_mz = 20000)
  )
}
binning <- list(min_mz = 2000, max_mz = 20000, bins = 6000L)

reference_stages <- function(spectrum, decreasing) {
  stages <- vector("list", length(stage_names))
  x <- suppressWarnings(transformIntensity(spectrum, method = "sqrt"))
  stages[[1L]] <- x
  x <- suppressWarnings(smoothIntensity(x, method = "SavitzkyGolay", halfWindowSize = 10L))
  stages[[2L]] <- x
  x <- removeBaseline(x, method = "SNIP", iterations = 20L, decreasing = decreasing)
  stages[[3L]] <- x
  x <- calibrateIntensity(x, method = "TIC")
  stages[[4L]] <- x
  stages[[5L]] <- trim(x, range = c(2000, 20000))
  stages
}

reference_bins <- function(mz, intensity) {
  edges <- seq(0, binning$bins) * ((binning$max_mz - binning$min_mz) / binning$bins) +
    binning$min_mz
  edges[length(edges)] <- binning$max_mz
  inside <- mz >= binning$min_mz & mz <= binning$max_mz
  index <- findInterval(mz[inside], edges, rightmost.closed = TRUE)
  values <- intensity[inside]
  bins <- numeric(binning$bins)
  for (k in seq_along(index)) {
    bins[index[[k]]] <- bins[index[[k]]] + values[[k]]
  }
  bins
}

run_marklab <- function(acquisition_dir, steps, name, with_binning = FALSE) {
  config <- file.path(out_dir, paste0(name, ".config.json"))
  out <- file.path(out_dir, paste0(name, ".json"))
  body <- list(steps = steps)
  if (with_binning) {
    body$binning <- binning
  }
  writeLines(toJSON(body, auto_unbox = TRUE, digits = NA), config)
  output <- suppressWarnings(system2(
    marklab,
    c("spectrum", "preprocess", "--bruker", shQuote(acquisition_dir),
      "--config", shQuote(config), "--out", shQuote(out)),
    stdout = TRUE, stderr = TRUE
  ))
  list(status = attr(output, "status"), output = output, out = out)
}

read_result <- function(run) {
  if (!is.null(run$status)) {
    stop("marklab failed: ", paste(run$output, collapse = "\n"))
  }
  fromJSON(run$out, simplifyVector = TRUE)
}

compare <- function(label, ours_mz, ours_intensity, reference_mz, reference_intensity) {
  same_length <- length(ours_mz) == length(reference_mz) &&
    length(ours_intensity) == length(reference_intensity)
  mz_error <- if (same_length) {
    max(abs(ours_mz - reference_mz) / pmax(1, abs(reference_mz)))
  } else {
    Inf
  }
  scale <- max(abs(reference_intensity))
  intensity_error <- if (same_length) {
    max(abs(ours_intensity - reference_intensity)) / max(scale, .Machine$double.xmin)
  } else {
    Inf
  }
  data.frame(
    comparison = label,
    points = length(reference_mz),
    mz_error = mz_error,
    intensity_error = intensity_error,
    pass = mz_error <= tolerance && intensity_error <= tolerance
  )
}

rows <- NULL
for (acquisition in acquisitions) {
  dir <- file.path(examples, acquisition)
  id <- gsub("/", "_", acquisition, fixed = TRUE)
  raw <- readBrukerFlexFile(file.path(dir, "fid"))
  spectrum <- createMassSpectrum(mass = raw$spectrum$mass, intensity = raw$spectrum$intensity)

  ours <- read_result(run_marklab(dir, list(), paste0(id, "_read")))
  rows <- rbind(rows, compare(
    paste(acquisition, "read"), ours$spectrum$mz, ours$spectrum$intensity,
    mass(spectrum), intensity(spectrum)
  ))

  for (decreasing in c(TRUE, FALSE)) {
    reference <- reference_stages(spectrum, decreasing)
    steps <- steps_for(decreasing)
    snip <- if (decreasing) "decreasing" else "increasing"
    for (k in seq_along(steps)) {
      if (k < 3L && !decreasing) {
        next
      }
      last <- k == length(steps)
      name <- sprintf("%s_%s_%d", id, snip, k)
      ours <- read_result(run_marklab(dir, steps[seq_len(k)], name, with_binning = last))
      label <- sprintf("%s %s SNIP after %s", acquisition, snip, stage_names[[k]])
      rows <- rbind(rows, compare(
        label, ours$spectrum$mz, ours$spectrum$intensity,
        mass(reference[[k]]), intensity(reference[[k]])
      ))
      if (last) {
        bins <- reference_bins(mass(reference[[k]]), intensity(reference[[k]]))
        rows <- rbind(rows, compare(
          paste(label, "binned"), seq_along(ours$binned$intensity),
          ours$binned$intensity, seq_along(bins), bins
        ))
      }
    }
  }
}

hpc <- run_marklab(file.path(examples, "hpc/fid/0_A20/1/1SRef"), list(), "hpc_rejected")
hpc_rejected <- !is.null(hpc$status) &&
  any(grepl("high-precision calibration", hpc$output, fixed = TRUE))
rows <- rbind(rows, data.frame(
  comparison = "hpc/fid/0_A20 rejected", points = NA_integer_,
  mz_error = NA_real_, intensity_error = NA_real_, pass = hpc_rejected
))

write.csv(rows, file.path(out_dir, "comparison.csv"), row.names = FALSE)
cat(sprintf(
  "%s; MALDIquant %s; readBrukerFlexData %s; jsonlite %s\n", R.version.string,
  packageVersion("MALDIquant"), packageVersion("readBrukerFlexData"), packageVersion("jsonlite")
))
print(rows, row.names = FALSE, digits = 3)
if (!all(rows$pass)) {
  quit(status = 1L)
}
