# Compare marklab's Bruker reader with readBrukerFlexData 1.9.3 on a seeded random sample of the
# RKI MALDI-TOF database v4.2 (Zenodo 14562231). Usage:
#   R_LIBS=target/maldi-reference-rlib Rscript tests/fixtures/maldi/rki_readbrukerflex.R \
#       MARKLAB DB_ROOT N OUT_DIR
# Declared before running (2026-10-28): sample N acquisitions with set.seed(20261028); for
# acquisitions both readers read, m/z within 1e-9 relative and intensities identical; record every
# acquisition where only one reader reads or readBrukerFlexData warns.
#
# Changed after the first run: the summary first counted only acquisitions without a
# readBrukerFlexData warning. Most RKI acquisitions trigger an encoding warning on the acqu comment
# line, which does not affect the spectrum, so the summary now covers every acquisition both
# readers read and tabulates warnings separately. The comparison itself is unchanged.
args <- commandArgs(trailingOnly = TRUE)
marklab <- normalizePath(args[[1L]])
root <- normalizePath(args[[2L]])
n <- as.integer(args[[3L]])
out_dir <- args[[4L]]
stopifnot(!file.exists(out_dir))
dir.create(out_dir)
suppressPackageStartupMessages({
  library(readBrukerFlexData)
  library(jsonlite)
})
fids <- list.files(root, pattern = "^fid$", recursive = TRUE, full.names = TRUE)
set.seed(20261028)
sample_fids <- sample(fids, n)
config <- file.path(out_dir, "empty.json")
writeLines('{"steps": []}', config)
rows <- vector("list", length(sample_fids))
for (k in seq_along(sample_fids)) {
  fid <- sample_fids[[k]]
  dir <- dirname(fid)
  out <- tempfile(tmpdir = out_dir, fileext = ".json")
  msg <- suppressWarnings(system2(marklab, c("microbio", "preprocess", "--bruker", shQuote(dir),
    "--config", shQuote(config), "--out", shQuote(out)), stdout = TRUE, stderr = TRUE))
  ours <- if (is.null(attr(msg, "status"))) fromJSON(out) else NULL
  unlink(out)
  warned <- character()
  ref <- withCallingHandlers(
    tryCatch(readBrukerFlexFile(fid), error = function(e) NULL),
    warning = function(w) {
      warned <<- c(warned, conditionMessage(w))
      invokeRestart("muffleWarning")
    }
  )
  mz_error <- NA_real_
  intensity_identical <- NA
  if (!is.null(ours) && !is.null(ref)) {
    same <- length(ours$spectrum$mz) == length(ref$spectrum$mass)
    mz_error <- if (same) max(abs(ours$spectrum$mz - ref$spectrum$mass) / ref$spectrum$mass) else Inf
    intensity_identical <- same && identical(as.numeric(ours$spectrum$intensity),
                                             as.numeric(ref$spectrum$intensity))
  }
  rows[[k]] <- data.frame(
    acquisition = substring(dir, nchar(root) + 2L),
    marklab = if (is.null(ours)) sub(".*Bruker flex acquisition: ", "", tail(msg, 1L)) else "accepted",
    readbrukerflexdata = if (is.null(ref)) "error" else if (length(warned)) paste(unique(sub("\n.*", "", warned)), collapse = "; ") else "ok",
    mz_error = mz_error,
    intensity_identical = intensity_identical
  )
}
result <- do.call(rbind, rows)
write.csv(result, file.path(out_dir, "comparison.csv"), row.names = FALSE)
both <- result[result$marklab == "accepted" & result$readbrukerflexdata != "error", ]
cat(sprintf("sampled %d; both read %d; max mz error %.3g; intensities identical %d/%d\n",
  nrow(result), nrow(both), max(both$mz_error), sum(both$intensity_identical), nrow(both)))
warning_kind <- sub("'.*", "", result$readbrukerflexdata)
print(table(marklab = substr(result$marklab, 1, 70), readBrukerFlexData = warning_kind))
