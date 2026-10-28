#!/usr/bin/env Rscript
# Compare `marklab space dist` and `marklab space rank` with R SPACE's plot_dist, plot_MI_rank and
# plot_MI_radius on SPACE's own tutorial table. R SPACE (github.com/eschrom/SPACE, Apache-2.0) runs
# only as an external oracle, sourced from its pinned commit
# 94f0a0f9311e2ee2b406ebc45c84a4e93f2f38f8; neither its code nor its data is committed. Run from the
# repository root (see compare_space.R for fetching the SPACE source):
#   R_LIBS=/path/to/library Rscript tests/fixtures/space/compare_space_views.R /path/to/marklab \
#     space-src space-src/inst/doc/HuLN_Object1_ObjTable_Types.csv /path/to/new/output
#
# Fixed before any comparison was run (2026-10-28):
# * Census: as compare_space.R, the tutorial table at radii 10 and 20 with 1,000 discs each from R
#   census_table (seed 20261028); Marklab receives R's centres as seed_cell_ids, and its census must
#   equal R's within 1e-12 scaled error max(1, |R|).
# * dist, radius 10: R plot_dist on its census with bin_num 4, 10 and 25 for (a) the most frequent
#   object and (b) the two most frequent objects, in census column order (one channel, so
#   impossible compositions are removed). Each variable must have more distinct values than bins,
#   checked before comparing, since R does not round otherwise. Marklab's cell values and
#   frequencies must equal R's data frame within 1e-12 scaled error, with NA exactly where Marklab
#   reports no frequency. Without bins, Marklab's disc values for (b) must equal R's census columns.
# * rank: Marklab cisMI (depth 2, 200 bootstraps) on the same request. R plot_MI_rank receives
#   Marklab's ensembles as its table (VA, VB, CisMI, Zscore, Pvalue, Padjust per radius) and runs at
#   radius 10 with its defaults; with depth = 2; with not = (a); and with all = (a), depth = 2,
#   p_thr = 1 and mi_thr = 0. The kept ensembles, their order, CisMI and Zscore must match exactly,
#   and the variable table's order must match with its aggregate Z within 1e-12 scaled error.
#   plot_MI_radius for (b) must return Marklab's CisMI at both radii exactly.
# * transMI rank: the table split into the 3 x 2 tiles of compare_space_trans.R, 300 discs per tile
#   at radius 10, Marklab transMI for factor `column` with depth 2 and 100 bootstraps. R
#   plot_MI_rank with group = "column", once with its defaults and once with p_thr = 1, must keep
#   the same ensembles in the same order with the same TransMI and Zscore, and the same variable
#   table.
#
# Revised after the first run stopped inside R before any rank comparison (2026-10-28): drawing
# plot_MI_rank's palette strip fails under ggplot2 4.0.3 ("object 'Y' not found"). The plots are
# display only, so `print` is replaced by a no-op that never builds them and only the returned data
# are compared. The criteria are unchanged.

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
for (file in c("utils.R", "build_distributions.R", "census_image_table.R", "measure_mutual_info.R",
               "plot_distributions.R", "plot_mutual_info.R")) {
  source(file.path(space_source, "R", file))
}
grDevices::pdf(NULL)
# The SPACE functions draw their plots with print(); its argument is never evaluated, so none is
# built.
print <- function(x, ...) invisible(NULL)

radii <- c(10, 20)
sizes <- c(1000L, 1000L)
seed <- 20261028L
cells <- utils::read.csv(table_path)
stopifnot(identical(colnames(cells), c("X", "Y", "Z", "Object")), all(cells$Z == 0))
classes <- sort(unique(cells$Object))
set.seed(seed)
seed_rows <- lapply(sizes, function(size) {
  sample(which(cells$Object %in% unique(cells$Object)), size)
})
set.seed(seed)
census <- census_table(cells, radii = radii, sample_size = sizes)[[1]]
census[is.na(census)] <- 0
to_r <- function(names) sub("^cell_type\\.", "O1.", names)
to_marklab <- function(names) sub("^O1\\.", "cell_type.", names)

window <- list(type = "MultiPolygon", coordinates = list(list(list(
  c(-1, -1), c(max(cells$X) + 1, -1), c(max(cells$X) + 1, max(cells$Y) + 1),
  c(-1, max(cells$Y) + 1), c(-1, -1)))))
channel <- list(id = "cell_type", label = "HuLN object", kind = "categorical",
                unit = "categorical", measurement_status = "imported_prediction",
                provenance = "SPACE tutorial HuLN_Object1_ObjTable_Types.csv",
                levels = as.character(classes))
request <- list(
  format = "marklab.space_request", version = 1L,
  source = list(kind = "inline", channels = list(channel),
    specimens = list(list(specimen_id = "HuLN", coordinate_frame_id = "HuLN-table",
      window = window, cell_ids = as.character(seq_len(nrow(cells))),
      coordinates_um = unname(as.matrix(cells[, c("X", "Y")])),
      observations = list(cell_type = match(cells$Object, classes) - 1)))),
  objects = list("cell_type"),
  census = list(radii_um = radii, seed = 1L,
                seed_cell_ids = list(HuLN = lapply(seed_rows, as.character))))
request_path <- file.path(output, "request.json")
jsonlite::write_json(request, request_path, auto_unbox = TRUE, digits = I(17))

marklab <- function(arguments, name) {
  path <- file.path(output, name)
  status <- system2(executable, c(arguments, "--out", path))
  if (status != 0) stop("marklab ", paste(arguments[1:2], collapse = " "), " failed")
  jsonlite::fromJSON(path, simplifyVector = FALSE)
}
scaled <- function(observed, expected) abs(observed - expected) / pmax(1, abs(expected))
checks <- list()
passed <- logical()

# Census parity.
marklab_census <- marklab(c("space", "census", "--request", request_path), "census.json")
specimen <- marklab_census$specimens[[1]]
census_error <- 0
for (i in seq_along(radii)) {
  discs <- specimen$radii[[i]]$discs
  values <- matrix(unlist(lapply(discs, `[[`, "values")), ncol = length(specimen$variables),
                   byrow = TRUE)
  expected <- as.matrix(census[census$Radius == radii[i], to_r(unlist(specimen$variables))])
  census_error <- max(census_error, scaled(values, expected))
}
checks$census_max_scaled_error <- census_error
passed["census"] <- census_error <= 1e-12

# dist.
counts <- sort(table(cells$Object), decreasing = TRUE)
frequent <- as.numeric(names(counts)[1:2])
census_order <- function(codes) {
  codes[order(match(paste0("O1.", codes), colnames(census)))]
}
pair <- census_order(frequent)
at_10 <- census[census$Radius == 10, ]
dist_error <- 0
dist_na_match <- TRUE
for (codes in list(frequent[1], pair)) {
  for (bins in c(4L, 10L, 25L)) {
    names_r <- paste0("O1.", codes)
    stopifnot(all(vapply(names_r, function(name) length(unique(at_10[[name]])) > bins, TRUE)))
    r <- plot_dist(census, names_r, 10, bin_num = bins)
    m <- marklab(c("space", "dist", "--request", request_path, "--radius", "10", "--bins", bins,
                   "--variables", paste(to_marklab(names_r), collapse = ",")),
                 sprintf("dist-%s-%d.json", paste(codes, collapse = "-"), bins))
    cells_m <- m$values$cells
    stopifnot(length(cells_m) == nrow(r))
    values <- matrix(unlist(lapply(cells_m, `[[`, "values")), ncol = length(codes), byrow = TRUE)
    frequency <- vapply(cells_m, function(cell) {
      if (is.null(cell$frequency)) NA_real_ else cell$frequency
    }, 0)
    dist_error <- max(dist_error, scaled(values, as.matrix(r[, names_r])))
    dist_na_match <- dist_na_match && identical(is.na(frequency), is.na(r$freq))
    kept <- !is.na(r$freq)
    dist_error <- max(dist_error, scaled(frequency[kept], r$freq[kept]))
  }
}
raw <- marklab(c("space", "dist", "--request", request_path, "--radius", "10", "--variables",
                 paste(to_marklab(paste0("O1.", pair)), collapse = ",")), "dist-raw.json")
raw_values <- matrix(unlist(raw$values$discs), ncol = 2, byrow = TRUE)
checks$dist_max_scaled_error <- dist_error
checks$dist_removed_cells_match <- dist_na_match
checks$dist_raw_matches_census <- identical(unname(raw_values),
                                            unname(as.matrix(at_10[, paste0("O1.", pair)])))
passed["dist"] <- dist_error <= 1e-12 && dist_na_match && checks$dist_raw_matches_census

# rank on cisMI.
cis <- marklab(c("space", "cis", "--request", request_path, "--depth", "2", "--bootstraps", "200"),
               "cis.json")
cis_path <- file.path(output, "cis.json")
r_table <- function(ensembles, statistic, columns) {
  frame <- data.frame(
    VA = vapply(ensembles, function(e) to_r(e$variables[[1]]), ""),
    VB = vapply(ensembles, function(e) {
      if (length(e$variables) > 1) to_r(e$variables[[2]]) else NA_character_
    }, ""),
    stringsAsFactors = FALSE)
  values <- lapply(ensembles, statistic)
  for (k in seq_along(columns)) frame[[columns[k]]] <- vapply(values, `[[`, 0, k)
  frame
}
cis_statistic <- function(e) {
  z <- if (is.null(e$z_score)) Inf * sign(e$cis_mi) else e$z_score
  c(e$cis_mi, z, e$p_value_normal, e$p_adjust_bh)
}
mi <- lapply(cis$specimens[[1]]$radii, function(radius) {
  r_table(radius$ensembles, cis_statistic, c("CisMI", "Zscore", "Pvalue", "Padjust"))
})
names(mi) <- as.character(radii)
palettes <- list(O1 = c("#FFFFFF", grDevices::rainbow(max(classes) + 1)))

same_ranking <- function(r, m, statistic_column, z_column) {
  ensembles <- m$view$ensembles
  variables <- m$view$variables
  if (is.null(r)) return(length(ensembles) == 0 && length(variables) == 0)
  ranked <- r[[1]]
  aggregate <- r[[2]]
  if (nrow(ranked) != length(ensembles) || nrow(aggregate) != length(variables)) return(FALSE)
  pairs_m <- vapply(ensembles, function(e) paste(to_r(unlist(e$variables)), collapse = "+"), "")
  pairs_r <- apply(ranked[, c("VA", "VB")], 1, function(row) {
    paste(row[!is.na(row)], collapse = "+")
  })
  z_m <- vapply(ensembles, function(e) if (is.null(e$z_score)) Inf else e$z_score, 0)
  score_m <- vapply(variables, function(v) if (v$unbounded_ensembles > 0) Inf else v$score, 0)
  identical(unname(pairs_m), unname(pairs_r)) &&
    identical(vapply(ensembles, `[[`, 0, "mi"), unname(ranked[[statistic_column]])) &&
    identical(abs(z_m), abs(unname(ranked[[z_column]]))) &&
    identical(to_r(vapply(variables, `[[`, "", "name")), unname(aggregate$V)) &&
    max(c(0, scaled(score_m, aggregate[[2]])), na.rm = FALSE) <= 1e-12
}
first <- paste0("O1.", frequent[1])
cis_cases <- list(
  defaults = list(r = list(), m = character()),
  depth_2 = list(r = list(depth = 2), m = c("--depth", "2")),
  not_first = list(r = list(not = first), m = c("--not", to_marklab(first))),
  all_first = list(r = list(depth = 2, all = first, p_thr = 1, mi_thr = 0),
                   m = c("--depth", "2", "--all", to_marklab(first), "--p-threshold", "1",
                         "--mi-threshold", "0")))
for (name in names(cis_cases)) {
  case <- cis_cases[[name]]
  r <- do.call(plot_MI_rank, c(list(mi = mi, radius = 10, col_pals = palettes), case$r))
  m <- marklab(c("space", "rank", "--result", cis_path, "--radius", "10", case$m),
               paste0("rank-cis-", name, ".json"))
  checks[[paste0("rank_cis_", name, "_kept")]] <- length(m$view$ensembles)
  passed[paste0("rank_cis_", name)] <- same_ranking(r, m, "CisMI", "Zscore")
}
r_radius <- plot_MI_radius(mi, ensemble = paste0("O1.", pair))
m_radius <- marklab(c("space", "rank", "--result", cis_path, "--ensemble",
                      paste(to_marklab(paste0("O1.", pair)), collapse = ",")), "rank-radius.json")
passed["rank_radius"] <- identical(vapply(m_radius$view$radii, `[[`, 0, "mi"),
                                   unname(r_radius[, 2]))

# rank on transMI.
column <- pmin(floor((cells$X - min(cells$X)) / (diff(range(cells$X)) / 3)), 2)
row <- pmin(floor((cells$Y - min(cells$Y)) / (diff(range(cells$Y)) / 2)), 1)
tile <- 1 + column + 3 * row
columns <- rep(c("left", "middle", "right"), 2)
specimens <- lapply(1:6, function(t) {
  rows <- which(tile == t)
  x <- cells$X[rows]
  y <- cells$Y[rows]
  tile_window <- list(type = "MultiPolygon", coordinates = list(list(list(
    c(min(x) - 1, min(y) - 1), c(max(x) + 1, min(y) - 1), c(max(x) + 1, max(y) + 1),
    c(min(x) - 1, max(y) + 1), c(min(x) - 1, min(y) - 1)))))
  list(specimen_id = paste0("tile-", t), coordinate_frame_id = "HuLN-table",
       groups = list(column = columns[t]), window = tile_window,
       cell_ids = as.character(rows), coordinates_um = unname(cbind(x, y)),
       observations = list(cell_type = match(cells$Object[rows], classes) - 1))
})
tiles <- list(format = "marklab.space_request", version = 1L,
              source = list(kind = "inline", channels = list(channel), specimens = specimens),
              objects = list("cell_type"),
              census = list(radii_um = list(10), sample_size = list(300L), seed = 20261028L))
tiles_path <- file.path(output, "tiles.json")
jsonlite::write_json(tiles, tiles_path, auto_unbox = TRUE, digits = I(17))
trans <- marklab(c("space", "trans", "--request", tiles_path, "--depth", "2", "--bootstraps", "100",
                   "--factor", "column"), "trans.json")
trans_path <- file.path(output, "trans.json")
trans_statistic <- function(e) {
  s <- e$factors[[1]]
  c(s$trans_mi, s$z_score, s$p_value_normal, s$p_adjust_bh)
}
mi_trans <- list(`10` = r_table(trans$radii[[1]]$ensembles, trans_statistic,
                                c("TransMI_column", "Zscore_column", "Pvalue_column",
                                  "Padjust_column")))
trans_cases <- list(defaults = list(r = list(), m = character()),
                    p_1 = list(r = list(p_thr = 1), m = c("--p-threshold", "1")))
for (name in names(trans_cases)) {
  case <- trans_cases[[name]]
  r <- do.call(plot_MI_rank, c(list(mi = mi_trans, radius = 10, col_pals = palettes,
                                    group = "column"), case$r))
  m <- marklab(c("space", "rank", "--result", trans_path, "--radius", "10", case$m),
               paste0("rank-trans-", name, ".json"))
  checks[[paste0("rank_trans_", name, "_kept")]] <- length(m$view$ensembles)
  passed[paste0("rank_trans_", name)] <- same_ranking(r, m, "TransMI_column", "Zscore_column")
}

checks$passed <- as.list(passed)
jsonlite::write_json(checks, file.path(output, "checks.json"), auto_unbox = TRUE, digits = I(17),
                     pretty = TRUE)
base::print(checks)
if (!all(passed)) stop("SPACE plot-data comparison failed: ",
                       paste(names(passed)[!passed], collapse = ", "))
cat("All SPACE plot-data comparison criteria passed.\n")
