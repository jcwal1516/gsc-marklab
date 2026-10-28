#!/usr/bin/env Rscript
# Compare `marklab space pattern` with R SPACE's learn_pattern on SPACE's own tutorial table split
# into tiles. R SPACE (github.com/eschrom/SPACE, Apache-2.0) runs only as an external oracle, sourced
# from its pinned commit 94f0a0f9311e2ee2b406ebc45c84a4e93f2f38f8; neither its code nor its data is
# committed. kohonen (GPL) runs only here, as the reference for Marklab's own implementation. Run
# from the repository root (see compare_space.R for fetching the SPACE source):
#   R_LIBS=/path/to/library Rscript tests/fixtures/space/compare_space_pattern.R /path/to/marklab \
#     space-src space-src/inst/doc/HuLN_Object1_ObjTable_Types.csv /path/to/new/output
#
# Fixed before any comparison was run (2026-10-28):
# * Data: the six tiles of compare_space_trans.R (factors `column` and `row`), radius 10, 1,000
#   discs per tile from Marklab seed 20261028. R receives Marklab's census as one data frame per
#   tile. Ensemble: the three most frequent object codes. learn_pattern's defaults otherwise
#   (som_reps 50, smooth_window 100, conf_int 0.95, a chain, no subsampling).
# * Designs: (a) focal column "left" against reference column "right" (factor inferred), 2,000
#   focal and 2,000 reference discs; (b) all six tiles pooled without a reference, 6,000 discs.
# * Exact, after the map: R learn_pattern runs with its kohonen::som call (counted, exactly one)
#   replaced by a stub that returns Marklab's trained nodes, which are in path order, so R's chain
#   orientation keeps them. The stub must receive Marklab's focal disc count, rlen 50 and alpha
#   (0.05, 0.01). R's padding draws differ from Marklab's, so with h = 50: Y and Enr are compared
#   at path positions [h, n - h) and Ymin and Ymax at [2h, n - 2h) (0-based), where no padding
#   enters; X everywhere. All within 1e-12 scaled error max(1, |R|), for designs (a) and (b).
#   Marklab's initial_radius must equal R quantile(unit.distances(somgrid(1000, 1)), 2/3).
# * Map quality, design (b): kohonen::som inside R learn_pattern (set.seed(1) ... set.seed(5)) and
#   Marklab with seeds 1 ... 5 on the same discs (their seed cells given explicitly). Mean
#   quantization error (mean Euclidean distance from each disc to its nearest node) of Marklab
#   within 5% of R's; topographic error (share of discs whose two nearest nodes are not adjacent
#   on the chain) of Marklab at most R's + 0.05.
# * End to end, design (b): mean |Y_M(i) - Y_R(i)| over interior positions, averaged over the five
#   seeds, at most 1.5 x the mean |Y_R(i+1) - Y_R(i)| over the four consecutive R pairs, + 0.5
#   percentage points.
# * Wall-clock times of R learn_pattern and of the Marklab command, per seed, are reported, not
#   tested.
#
# learn_pattern draws its plots with print(); as in compare_space_views.R, print is a no-op that
# never evaluates its argument, so no plot is built.

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
for (file in c("utils.R", "build_distributions.R")) source(file.path(space_source, "R", file))
print <- function(x, ...) invisible(NULL)

# learn_pattern with its map call routed through `train_map`, which either returns given nodes or
# runs kohonen and records its codes. Only the pinned source text is parsed and evaluated.
pattern_text <- readLines(file.path(space_source, "R", "learn_map_patterns.R"))
som_call <- "SOM <- kohonen::som(as.matrix(foc),"
stopifnot(sum(lengths(regmatches(pattern_text, gregexpr(som_call, pattern_text,
                                                        fixed = TRUE)))) == 1)
pattern_text <- gsub(som_call, "SOM <- train_map(as.matrix(foc),", pattern_text, fixed = TRUE)
given_nodes <- NULL
recorded <- list()
train_map <- function(data, grid, rlen, alpha) {
  recorded <<- list(rows = nrow(data), rlen = rlen, alpha = alpha)
  if (!is.null(given_nodes)) return(list(codes = list(given_nodes)))
  map <- kohonen::som(data, grid = grid, rlen = rlen, alpha = alpha)
  recorded$codes <<- unname(map$codes[[1]])
  map
}
learn_pattern_routed <- local({
  eval(parse(text = pattern_text))
  learn_pattern
})

radius <- 10
size <- 1000L
seed <- 20261028L
half <- 50L
cells <- utils::read.csv(table_path)
stopifnot(identical(colnames(cells), c("X", "Y", "Z", "Object")), all(cells$Z == 0))
classes <- sort(unique(cells$Object))
column <- pmin(floor((cells$X - min(cells$X)) / (diff(range(cells$X)) / 3)), 2)
row <- pmin(floor((cells$Y - min(cells$Y)) / (diff(range(cells$Y)) / 2)), 1)
tile <- 1 + column + 3 * row
groups <- data.frame(column = rep(c("left", "middle", "right"), 2),
                     row = rep(c("bottom", "top"), each = 3))
frequent <- as.numeric(names(sort(table(cells$Object), decreasing = TRUE))[1:3])
variables <- paste0("cell_type.", frequent)
ensemble <- paste0("O1.", frequent)
palette <- list(O1 = rep("#000000", max(classes)))

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
request_with <- function(census) {
  list(format = "marklab.space_request", version = 1L,
       source = list(kind = "inline",
         channels = list(list(id = "cell_type", label = "HuLN object", kind = "categorical",
           unit = "categorical", measurement_status = "imported_prediction",
           provenance = "SPACE tutorial HuLN_Object1_ObjTable_Types.csv",
           levels = as.character(classes))),
         specimens = specimens),
       objects = list("cell_type"), census = census)
}
write_request <- function(request, name) {
  path <- file.path(output, name)
  jsonlite::write_json(request, path, auto_unbox = TRUE, digits = I(17))
  path
}
marklab <- function(arguments, name) {
  path <- file.path(output, name)
  started <- Sys.time()
  status <- system2(executable, c(arguments, "--out", path))
  if (status != 0) stop("marklab ", paste(arguments[1:2], collapse = " "), " failed")
  seconds <- as.numeric(difftime(Sys.time(), started, units = "secs"))
  list(result = jsonlite::fromJSON(path, simplifyVector = FALSE), seconds = seconds)
}
scaled <- function(observed, expected) abs(observed - expected) / pmax(1, abs(expected))
checks <- list()
passed <- logical()

request_path <- write_request(request_with(list(radii_um = list(radius),
                                                sample_size = list(size), seed = seed)),
                              "request.json")
census_json <- marklab(c("space", "census", "--request", request_path), "census.json")$result
stopifnot(identical(vapply(census_json$specimens, function(s) s$specimen_id, ""),
                    paste0("tile-", 1:6)))
# R censuses: the ensemble's variables (0 where a tile lacks a class), X, Y, Z, Radius.
censuses <- lapply(census_json$specimens, function(specimen) {
  discs <- specimen$radii[[1]]$discs
  present <- unlist(specimen$variables)
  values <- t(vapply(discs, function(disc) {
    v <- unlist(disc$values)[match(variables, present)]
    ifelse(is.na(v), 0, v)
  }, numeric(length(variables))))
  frame <- as.data.frame(values)
  colnames(frame) <- ensemble
  frame$X <- vapply(discs, function(disc) disc$x_um, 0)
  frame$Y <- vapply(discs, function(disc) disc$y_um, 0)
  frame$Z <- 0
  frame$Radius <- radius
  frame
})
seed_cells <- lapply(census_json$specimens, function(specimen) {
  list(vapply(specimen$radii[[1]]$discs, function(disc) disc$seed_cell_id, ""))
})
names(seed_cells) <- paste0("tile-", 1:6)
pattern_arguments <- c("--variables", paste(variables, collapse = ","), "--radius", radius)

nodes_of <- function(result) do.call(rbind, lapply(result$nodes, unlist))
curves_of <- function(result) {
  do.call(rbind, lapply(result$curves, function(curve) {
    data.frame(V = sub("^cell_type\\.", "O1.", curve$variable), X = unlist(result$positions),
               Y = unlist(curve$mean), Ymin = unlist(curve$lower), Ymax = unlist(curve$upper),
               Enr = unlist(result$enrichment))
  }))
}
interior_error <- function(m, r) {
  stopifnot(identical(m$V, r$V))
  n <- sum(m$V == m$V[1])
  place <- rep(seq_len(n) - 1L, length(unique(m$V)))
  inner <- place >= half & place < n - half
  inner2 <- place >= 2L * half & place < n - 2L * half
  c(X = max(scaled(m$X, r$X)), Y = max(scaled(m$Y[inner], r$Y[inner])),
    Enr = max(scaled(m$Enr[inner], r$Enr[inner])),
    Ymin = max(scaled(m$Ymin[inner2], r$Ymin[inner2])),
    Ymax = max(scaled(m$Ymax[inner2], r$Ymax[inner2])))
}
call_r <- function(nodes, focal = NULL, reference = NULL, group = NULL) {
  given_nodes <<- nodes
  started <- Sys.time()
  out <- learn_pattern_routed(censuses, ensemble, radius, palette, group = group, focal = focal,
                              reference = reference)
  list(out = out, seconds = as.numeric(difftime(Sys.time(), started, units = "secs")))
}

# Exact after the map.
design_a <- marklab(c("space", "pattern", "--request", request_path, pattern_arguments,
                      "--focal", "left", "--reference", "right"), "pattern-a.json")$result
stopifnot(identical(unlist(design_a$focal_specimens), c("tile-1", "tile-4")),
          identical(unlist(design_a$reference_specimens), c("tile-3", "tile-6")))
r_a <- call_r(nodes_of(design_a), "left", "right", groups)$out
stub_ok <- function(result) {
  recorded$rows == result$focal_disc_count && recorded$rlen == 50 &&
    identical(recorded$alpha, c(0.05, 0.01))
}
stub_a <- stub_ok(design_a)
exact_a <- interior_error(curves_of(design_a), r_a[, c("V", "X", "Y", "Ymin", "Ymax", "Enr")])

fixed_request <- write_request(request_with(list(radii_um = list(radius),
                                                 seed_cell_ids = seed_cells, seed = 1L)),
                               "request-seed-1.json")
marklab_runs <- list()
marklab_seconds <- numeric()
for (i in 1:5) {
  path <- if (i == 1) fixed_request else {
    write_request(request_with(list(radii_um = list(radius), seed_cell_ids = seed_cells,
                                    seed = i)), paste0("request-seed-", i, ".json"))
  }
  run <- marklab(c("space", "pattern", "--request", path, pattern_arguments),
                 paste0("pattern-b-seed-", i, ".json"))
  marklab_runs[[i]] <- run$result
  marklab_seconds[i] <- run$seconds
}
stopifnot(marklab_runs[[1]]$focal_disc_count == 6000)
r_b <- call_r(nodes_of(marklab_runs[[1]]))$out
stub_b <- stub_ok(marklab_runs[[1]])
exact_b <- interior_error(curves_of(marklab_runs[[1]]), r_b[, c("V", "X", "Y", "Ymin", "Ymax",
                                                                  "Enr")])
unit_distances <- kohonen::unit.distances(kohonen::somgrid(1000, 1, "rectangular", "gaussian",
                                                           FALSE))
expected_radius <- unname(stats::quantile(unit_distances, 2 / 3))
checks$exact_a_max_scaled_error <- as.list(exact_a)
checks$exact_b_max_scaled_error <- as.list(exact_b)
checks$initial_radius <- c(marklab = design_a$initial_radius, r = expected_radius)
passed["stub_arguments"] <- stub_a && stub_b
passed["exact"] <- all(c(exact_a, exact_b) <= 1e-12)
passed["initial_radius"] <- design_a$initial_radius == expected_radius &&
  marklab_runs[[1]]$initial_radius == expected_radius

# Map quality and end to end against kohonen inside learn_pattern.
focal <- as.matrix(do.call(rbind, lapply(censuses, function(cen) cen[, ensemble])))
quality <- function(nodes) {
  nearest <- FNN::get.knnx(nodes, focal, k = 2)
  c(quantization = mean(nearest$nn.dist[, 1]),
    topographic = mean(abs(nearest$nn.index[, 1] - nearest$nn.index[, 2]) != 1))
}
r_runs <- list()
r_seconds <- numeric()
r_quality <- NULL
for (i in 1:5) {
  set.seed(i)
  run <- call_r(NULL)
  r_runs[[i]] <- run$out
  r_seconds[i] <- run$seconds
  r_quality <- rbind(r_quality, quality(recorded$codes))
}
marklab_quality <- do.call(rbind, lapply(marklab_runs, function(run) quality(nodes_of(run))))
checks$quantization_error <- c(marklab = mean(marklab_quality[, "quantization"]),
                               r = mean(r_quality[, "quantization"]))
checks$topographic_error <- c(marklab = mean(marklab_quality[, "topographic"]),
                              r = mean(r_quality[, "topographic"]))
passed["quantization"] <- abs(checks$quantization_error[["marklab"]] /
                                checks$quantization_error[["r"]] - 1) <= 0.05
passed["topographic"] <- checks$topographic_error[["marklab"]] <=
  checks$topographic_error[["r"]] + 0.05

n <- 6000L
inner <- rep(seq_len(n) - 1L, length(ensemble))
inner <- inner >= half & inner < n - half
marklab_r <- mean(vapply(1:5, function(i) {
  m <- curves_of(marklab_runs[[i]])
  stopifnot(identical(m$V, r_runs[[i]]$V))
  mean(abs(m$Y[inner] - r_runs[[i]]$Y[inner]))
}, 0))
r_r <- mean(vapply(1:4, function(i) mean(abs(r_runs[[i + 1]]$Y[inner] - r_runs[[i]]$Y[inner])),
                   0))
checks$marklab_r_mean_abs_y_difference <- marklab_r
checks$r_r_mean_abs_y_difference <- r_r
passed["end_to_end"] <- marklab_r <= 1.5 * r_r + 0.5

checks$ensemble <- variables
checks$marklab_pattern_seconds <- marklab_seconds
checks$r_learn_pattern_seconds <- r_seconds
checks$passed <- as.list(passed)
jsonlite::write_json(checks, file.path(output, "summary.json"), auto_unbox = TRUE, digits = I(17),
                     pretty = TRUE)
base::print(checks)
if (!all(passed)) stop("SPACE pattern comparison criteria failed: ",
                       paste(names(passed)[!passed], collapse = ", "))
cat("All SPACE pattern comparison criteria passed.\n")
