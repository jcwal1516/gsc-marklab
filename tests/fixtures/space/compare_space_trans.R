#!/usr/bin/env Rscript
# Compare `marklab space trans` with R SPACE's transMI on SPACE's own tutorial table split into tiles.
# R SPACE (github.com/eschrom/SPACE, Apache-2.0) runs only as an external oracle, sourced from its
# pinned commit 94f0a0f9311e2ee2b406ebc45c84a4e93f2f38f8; neither its code nor its data is committed.
# Census parity with R census_table is tested by compare_space.R; here R receives Marklab's census.
# Run from the repository root (see compare_space.R for fetching the SPACE source):
#   R_LIBS=/path/to/library Rscript tests/fixtures/space/compare_space_trans.R /path/to/marklab \
#     space-src space-src/inst/doc/HuLN_Object1_ObjTable_Types.csv /path/to/new/output
#
# Fixed before any comparison was run (2026-10-28):
# * Data: HuLN_Object1_ObjTable_Types.csv (9,757 cells, 20 object codes) split into a 3 x 2 grid of
#   equal-width tiles over its bounding box, giving six specimens tile-1 ... tile-6 (1,098 to 2,047
#   cells). Factors: `column` (3 levels) and `row` (2 levels). Object codes are the classes of one
#   categorical channel; classes absent from a tile are zero there (R standardize_censuses).
# * Census: radius 10, 1,000 discs per tile, Marklab seed 20261028. transMI: depth 3, 1,000
#   bootstraps on each side, so b = floor((1001 / 10)^(1/3)) = 4.
# * R measure_transMI has two defects that change results here, so R runs a copy with both fixed:
#   it reads the grid maximum from census j (the depth index) instead of census k, and (not
#   triggered here, since all/alo are unused) collapses cols_to_drop to a scalar.
# * Exact: for every ensemble and factor, Marklab's trans_mi must equal the modularity computed from
#   R's own round_column, build_dist and smooth_dist (each census on its own minimum and maximum),
#   both-direction KL and igraph::modularity, and must equal the fixed measure_transMI's TransMI,
#   within 1e-12 absolute. Marklab's BH adjustment must equal p.adjust(p, "BH") per factor and its
#   normal P must equal pnorm(Z, lower.tail = FALSE), within 1e-12 scaled error.
# * Distributional (the null shuffles the 15 pairwise divergences): the fixed measure_transMI runs
#   twice. Spearman's correlation of Z, Marklab versus R run 1, must be at least that of R run 2
#   versus R run 1 minus 0.01, and the mean |Z_M - Z_R1| at most 1.5 times the mean |Z_R2 - Z_R1|.
# * Wall-clock times of R measure_transMI (detectCores() - 1 workers) and of the Marklab command
#   (all cores) are reported, not tested.

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 4L) {
  stop("expected Marklab executable, SPACE source directory, tutorial object table and new output")
}
executable <- normalizePath(args[[1]], mustWork = TRUE)
space_source <- normalizePath(args[[2]], mustWork = TRUE)
table_path <- normalizePath(args[[3]], mustWork = TRUE)
output <- args[[4]]
if (dir.exists(output) || file.exists(output)) stop("output path must not exist")
pins <- utils::read.delim("tests/fixtures/space/reference-packages.tsv",
                          colClasses = "character", check.names = FALSE)
if (as.character(getRversion()) != pins$version[pins$package == "R"]) stop("R version differs")
for (row in which(pins$package != "R")) {
  observed <- unname(utils::packageDescription(pins$package[row], fields = "Version"))
  if (!identical(observed, pins$version[row])) {
    stop("reference package ", pins$package[row], " must be ", pins$version[row])
  }
}
dir.create(output)
for (file in c("utils.R", "build_distributions.R", "measure_mutual_info.R")) {
  source(file.path(space_source, "R", file))
}
# The fixed copy is the pinned SPACE source with exactly these expressions edited (counts are
# checked); only that trusted text is parsed and evaluated, in its own environment. The census
# loop's sub_max[[j]] is correct there and is left alone.
fixed_text <- readLines(file.path(space_source, "R", "measure_mutual_info.R"))
count <- function(pattern) sum(lengths(regmatches(fixed_text, gregexpr(pattern, fixed_text,
                                                                        fixed = TRUE))))
grid_defect <- "rbind(sub_min[[k]], sub_max[[j]])"
drop_defect <- "cols_to_drop <- cols_to_drop[!(var_cols %in% all) & !(var_cols %in% alo)] <- T"
stopifnot(count(grid_defect) == 2, count(drop_defect) == 1)
fixed_text <- gsub(grid_defect, "rbind(sub_min[[k]], sub_max[[k]])", fixed_text, fixed = TRUE)
fixed_text <- gsub(drop_defect, "cols_to_drop[!(var_cols %in% all) & !(var_cols %in% alo)] <- T",
                   fixed_text, fixed = TRUE)
measure_transMI_fixed <- local({
  eval(parse(text = fixed_text))
  measure_transMI
})

radius <- 10
size <- 1000L
depth <- 3L
bootstraps <- 1000L
bins <- floor(((size + 1) / 10)^(1 / depth))
cells <- utils::read.csv(table_path)
stopifnot(identical(colnames(cells), c("X", "Y", "Z", "Object")), all(cells$Z == 0))
classes <- sort(unique(cells$Object))
column <- pmin(floor((cells$X - min(cells$X)) / (diff(range(cells$X)) / 3)), 2)
row <- pmin(floor((cells$Y - min(cells$Y)) / (diff(range(cells$Y)) / 2)), 1)
tile <- 1 + column + 3 * row
groups <- data.frame(column = rep(c("left", "middle", "right"), 2),
                     row = rep(c("bottom", "top"), each = 3))

specimens <- lapply(1:6, function(t) {
  rows <- which(tile == t)
  x <- cells$X[rows]
  y <- cells$Y[rows]
  window <- list(type = "MultiPolygon", coordinates = list(list(list(
    c(min(x) - 1, min(y) - 1), c(max(x) + 1, min(y) - 1), c(max(x) + 1, max(y) + 1),
    c(min(x) - 1, max(y) + 1), c(min(x) - 1, min(y) - 1)))))
  list(specimen_id = paste0("tile-", t), coordinate_frame_id = "HuLN-table",
       groups = list(column = groups$column[t], row = groups$row[t]), window = window,
       cell_ids = as.character(rows), coordinates_um = unname(cbind(x, y)),
       observations = list(cell_type = match(cells$Object[rows], classes) - 1))
})
request <- list(
  format = "marklab.space_request", version = 1L,
  source = list(kind = "inline",
    channels = list(list(id = "cell_type", label = "HuLN object", kind = "categorical",
      unit = "categorical", measurement_status = "imported_prediction",
      provenance = "SPACE tutorial HuLN_Object1_ObjTable_Types.csv",
      levels = as.character(classes))),
    specimens = specimens),
  objects = list("cell_type"),
  census = list(radii_um = list(radius), sample_size = list(size), seed = 20261028L))
request_path <- file.path(output, "request.json")
jsonlite::write_json(request, request_path, auto_unbox = TRUE, digits = I(17))

run <- function(arguments) {
  started <- Sys.time()
  status <- system2(executable, arguments)
  if (status != 0) stop("marklab ", paste(arguments[1:2], collapse = " "), " failed")
  as.numeric(difftime(Sys.time(), started, units = "secs"))
}
scaled <- function(observed, expected) abs(observed - expected) / pmax(1, abs(expected))
checks <- list()
passed <- logical()

run(c("space", "census", "--request", request_path, "--out", file.path(output, "census.json")))
marklab_seconds <- run(c("space", "trans", "--request", request_path, "--out",
                         file.path(output, "trans.json"), "--depth", depth, "--bootstraps",
                         bootstraps))
census_json <- jsonlite::fromJSON(file.path(output, "census.json"), simplifyVector = FALSE)
trans <- jsonlite::fromJSON(file.path(output, "trans.json"), simplifyVector = FALSE)
passed["bins"] <- trans$bins == bins
analysed <- unlist(trans$analysed_variables)
to_r <- function(names) sub("^cell_type\\.", "O1.", names)
var_names <- to_r(analysed)
stopifnot(identical(vapply(trans$specimens, function(s) s$specimen_id, ""), paste0("tile-", 1:6)))

# R censuses from Marklab's: analysed variables (0 where a tile lacks a class), X, Y, Z, Radius.
censuses <- lapply(census_json$specimens, function(specimen) {
  discs <- specimen$radii[[1]]$discs
  present <- unlist(specimen$variables)
  values <- t(vapply(discs, function(disc) {
    v <- unlist(disc$values)[match(analysed, present)]
    ifelse(is.na(v), 0, v)
  }, numeric(length(analysed))))
  frame <- as.data.frame(values)
  colnames(frame) <- var_names
  frame$X <- vapply(discs, function(disc) disc$x_um, 0)
  frame$Y <- vapply(discs, function(disc) disc$y_um, 0)
  frame$Z <- 0
  frame$Radius <- radius
  frame
})

# Exact: R's own rounding, distributions, KL and igraph modularity on each census's own range.
sub_cen <- lapply(censuses, function(cen) cen[, var_names, drop = FALSE])
sub_min <- lapply(sub_cen, function(x) as.numeric(apply(x, 2, min)))
sub_max <- lapply(sub_cen, function(x) as.numeric(apply(x, 2, max)))
rounded <- lapply(seq_along(sub_cen), function(k) {
  mapply(round_column, sub_cen[[k]][, , drop = FALSE], sub_min[[k]], sub_max[[k]], bins, TRUE)
})
pairs <- t(utils::combn(length(censuses), 2))
modularity_of <- function(divergence, membership) {
  mat <- matrix(0, nrow = length(membership), ncol = length(membership))
  mat[lower.tri(mat)] <- divergence
  mat <- t(mat)
  mat <- mat + t(mat)
  mat <- matrix(1, nrow = nrow(mat), ncol = ncol(mat)) - (mat / max(mat))
  graph <- igraph::graph_from_adjacency_matrix(mat, weighted = TRUE)
  igraph::modularity(graph, membership, weights = igraph::E(graph)$weight)
}
key <- function(names) paste(sort(names), collapse = "+")
ensembles <- trans$radii[[1]]$ensembles
expected_count <- sum(vapply(1:depth, function(j) choose(length(analysed), j), 0))
passed["ensemble_count"] <- length(ensembles) == expected_count
factor_names <- vapply(trans$factors, function(f) f$factor, "")
stopifnot(identical(factor_names, c("column", "row")))
memberships <- lapply(factor_names, function(f) as.numeric(as.factor(groups[[f]])))
rows <- lapply(ensembles, function(e) {
  vars <- to_r(unlist(e$variables))
  distributions <- sapply(seq_along(rounded), function(k) {
    mins_and_maxs <- as.data.frame(rbind(sub_min[[k]], sub_max[[k]]))[, match(vars, var_names),
                                                                       drop = FALSE]
    smooth_dist(build_dist(rounded[[k]], vars, "all"), bins, mins_and_maxs, full_dist = FALSE)
  })
  if (is.null(dim(distributions))) distributions <- matrix(distributions, nrow = 1)
  divergence <- apply(pairs, 1, function(p) {
    d1 <- distributions[, p[1]]
    d2 <- distributions[, p[2]]
    (sum(d1 * log2(d1 / d2), na.rm = TRUE) + sum(d2 * log2(d2 / d1), na.rm = TRUE)) / 2
  })
  stats <- e$factors
  data.frame(key = key(unlist(e$variables)), factor = factor_names,
             q_m = vapply(stats, function(s) s$trans_mi, 0),
             z_m = vapply(stats, function(s) s$z_score, 0),
             p_m = vapply(stats, function(s) s$p_value_normal, 0),
             padj_m = vapply(stats, function(s) s$p_adjust_bh, 0),
             q_r_internal = vapply(memberships, function(m) modularity_of(divergence, m), 0))
})
m <- do.call(rbind, rows)
internal_error <- max(abs(m$q_m - m$q_r_internal))
checks$trans_mi_vs_r_internals_max_error <- internal_error

r_runs <- list()
r_seconds <- numeric()
for (replicate in 1:2) {
  started <- Sys.time()
  r_runs[[replicate]] <- measure_transMI_fixed(censuses, groups, depth = depth, radii = radius,
                                               bootstraps = bootstraps)
  r_seconds[replicate] <- as.numeric(difftime(Sys.time(), started, units = "secs"))
  saveRDS(r_runs[[replicate]], file.path(output, paste0("r_measure_transMI_", replicate, ".rds")))
}
r_long <- function(r) {
  r <- r[[1]]
  v_cols <- grep("^V[A-Z]$", colnames(r))
  keys <- apply(r[, v_cols, drop = FALSE], 1, function(row) {
    key(sub("^O1\\.", "cell_type.", row[!is.na(row)]))
  })
  do.call(rbind, lapply(factor_names, function(f) {
    data.frame(key = keys, factor = f, q = r[[paste0("TransMI_", f)]],
               z = r[[paste0("Zscore_", f)]])
  }))
}
r1 <- r_long(r_runs[[1]])
r2 <- r_long(r_runs[[2]])
joined <- merge(merge(m, setNames(r1, c("key", "factor", "q_r1", "z_r1")), by = c("key", "factor")),
                setNames(r2, c("key", "factor", "q_r2", "z_r2")), by = c("key", "factor"))
stopifnot(nrow(joined) == nrow(m), nrow(joined) == nrow(r1))
function_error <- max(abs(joined$q_m - joined$q_r1), abs(joined$q_m - joined$q_r2))
checks$trans_mi_vs_fixed_measure_transMI_max_error <- function_error

bh_error <- 0
for (f in factor_names) {
  rows_f <- m$factor == f
  bh_error <- max(bh_error, scaled(m$padj_m[rows_f], stats::p.adjust(m$p_m[rows_f], "BH")))
}
normal_error <- max(abs(m$p_m - stats::pnorm(m$z_m, lower.tail = FALSE)) / pmax(m$p_m, 1e-300))
checks$bh_max_scaled_error <- bh_error
checks$normal_p_max_scaled_error <- normal_error
passed["exact"] <- internal_error <= 1e-12 && function_error <= 1e-12 && bh_error <= 1e-12 &&
  normal_error <= 1e-12

spearman <- function(a, b) stats::cor(a, b, method = "spearman")
marklab_spearman <- spearman(joined$z_m, joined$z_r1)
r_spearman <- spearman(joined$z_r2, joined$z_r1)
marklab_mad <- mean(abs(joined$z_m - joined$z_r1))
r_mad <- mean(abs(joined$z_r2 - joined$z_r1))
checks$marklab_r1_spearman_z <- marklab_spearman
checks$r2_r1_spearman_z <- r_spearman
checks$marklab_r1_mean_abs_z_difference <- marklab_mad
checks$r2_r1_mean_abs_z_difference <- r_mad
passed["distribution"] <- marklab_spearman >= r_spearman - 0.01 && marklab_mad <= 1.5 * r_mad

checks$ensembles <- length(ensembles)
checks$analysed_variables <- analysed
checks$bins <- bins
checks$tile_cells <- as.list(table(tile))
checks$marklab_trans_seconds <- marklab_seconds
checks$r_measure_transMI_seconds <- r_seconds
checks$r_workers <- parallel::detectCores() - 1
checks$passed <- as.list(passed)
utils::write.csv(joined, file.path(output, "comparison.csv"), row.names = FALSE)
jsonlite::write_json(checks, file.path(output, "summary.json"), auto_unbox = TRUE, digits = I(17),
                     pretty = TRUE)
print(checks)
if (!all(passed)) stop("SPACE transMI comparison criteria failed: ",
                       paste(names(passed)[!passed], collapse = ", "))
cat("All SPACE transMI comparison criteria passed.\n")
