#!/usr/bin/env Rscript
# Descriptive comparison of `marklab bayes advanced-cluster` with spatstat kppm (Thomas).
# Run from the repository root. spatstat is GPL and runs only as an external oracle here.
# R_LIBS=/path/to/kppm/library:/path/to/reference/library \
#   Rscript tests/fixtures/bayesian_thomas/compare_kppm.R \
#   /path/to/marklab /path/to/CRC_clusters_neighborhoods_markers.csv /path/to/new/output \
#   [draws_per_chain warmup_per_chain]
#
# Fixed before any fit was run (2026-10-10):
# * Patterns: spatstat.data `redwood` (62 points), and Schurch et al. (2020, CC BY 4.0)
#   "B cells" in TMA A spots, coordinates x 0.377442 um per pixel in the published
#   1920 x 1440 pixel frame. The three spots with the most B cells among spots with at most
#   600, one spot per patient, ties broken by region name. Counts alone choose them.
# * Marklab: separate one-pattern fits for the kppm comparison, then one joint fit of the three
#   B-cell spots. Priors, dilation and sampler settings are in `marklab_input` below.
# * kppm: Thomas clusters with `method = "mincon"` (K function), `"palm"` and `"clik2"`, all
#   other arguments at their package defaults.
# * Criterion: descriptive only. kppm estimators and the Bayesian posterior target different
#   estimands under different edge treatments, so no agreement tolerance applies. The output
#   reports each kppm estimate, its ratio to the posterior median and whether it lies in the
#   central 95% posterior interval.

args <- commandArgs(trailingOnly = TRUE)
if (!length(args) %in% c(3L, 5L)) {
  stop("expected Marklab executable, Schurch CSV, a new output directory, and optionally ",
       "draws and warmup per chain")
}
# Defaults are the first declared run; longer chains are a separate, reported run.
draws <- if (length(args) == 5L) as.integer(args[[4]]) else 2000L
warmup <- if (length(args) == 5L) as.integer(args[[5]]) else 1500L
if (anyNA(c(draws, warmup)) || draws < 50L || warmup < 50L) stop("invalid draws or warmup")
executable <- normalizePath(args[[1]], mustWork = TRUE)
schurch_csv <- normalizePath(args[[2]], mustWork = TRUE)
output <- args[[3]]
if (dir.exists(output) || file.exists(output)) stop("output path must not exist")
pin_path <- "tests/fixtures/bayesian_thomas/reference-packages.tsv"
pins <- utils::read.delim(pin_path, colClasses = "character", check.names = FALSE)
if (!identical(names(pins), c("package", "version")) || anyDuplicated(pins$package)) {
  stop("invalid reference package pin file: ", pin_path)
}
expected_r <- pins$version[pins$package == "R"]
if (length(expected_r) != 1L || as.character(getRversion()) != expected_r) {
  stop("reference comparison requires R ", expected_r, "; found ", getRversion())
}
for (row in which(pins$package != "R")) {
  package <- pins$package[row]
  if (!requireNamespace(package, quietly = TRUE)) stop("missing reference package: ", package)
  observed <- unname(utils::packageDescription(package, fields = "Version"))
  if (observed != pins$version[row]) {
    stop("reference package ", package, " must be ", pins$version[row], "; found ", observed)
  }
}
suppressPackageStartupMessages(library(spatstat.model))
dir.create(output, recursive = TRUE)
output <- normalizePath(output, mustWork = TRUE)
write_json <- function(value, path) {
  jsonlite::write_json(value, path, auto_unbox = TRUE, digits = NA, pretty = TRUE, null = "null",
                       json_verbatim = TRUE)
}
# A u64 beyond R's integer range, written as a JSON integer rather than in exponent form.
verbatim_integer <- function(text) structure(text, class = "json")

prior <- function(mean, sd) {
  list(population_mean_mean = mean, population_mean_sd = sd, between_pattern_sd_scale = 0.5)
}
pattern_json <- function(id, x, y, window) {
  list(
    pattern_id = id, patient_id = id,
    window = list(xmin_um = window[1], xmax_um = window[2], ymin_um = window[3], ymax_um = window[4]),
    points = unname(lapply(seq_along(x), function(i) c(x[i], y[i])))
  )
}
marklab_input <- function(patterns, scale) {
  list(
    patterns = patterns,
    dilation_um = scale$dilation,
    priors = list(
      log_kappa = prior(log(scale$kappa), 1.5),
      log_mu = prior(log(scale$mu), 1.5),
      log_sigma = prior(log(scale$sigma), 1.0)
    ),
    sampler = list(chains = 4L, warmup = warmup, draws = draws,
                   birth_death_proposals_per_sweep = 30L, seed = 20261010L),
    diagnostic_policy = list(maximum_r_hat = 1.01, minimum_bulk_ess = 400,
                             minimum_tail_ess = 400, maximum_edge_truncation_fraction = 1e-3),
    resources = list(maximum_parents_per_pattern = 1000L, maximum_points_per_pattern = 1000L,
                     maximum_kernel_bytes = 1073741824,
                     maximum_work_units = verbatim_integer("1000000000000000"),
                     maximum_output_bytes = 16777216),
    posterior_predictive_draws = 400L,
    finite_gibbs = NULL,
    timeout_seconds = 3600L
  )
}
run_marklab <- function(name, input) {
  input_path <- file.path(output, paste0(name, "-input.json"))
  result_path <- file.path(output, paste0(name, "-result.json"))
  write_json(input, input_path)
  status <- system2(executable, c("bayes", "advanced-cluster", "--input", input_path,
                                  "--out", result_path))
  if (status != 0L) stop("marklab advanced-cluster failed for ", name)
  jsonlite::read_json(result_path)
}
kppm_estimates <- function(pattern) {
  methods <- c("mincon", "palm", "clik2")
  rows <- lapply(methods, function(method) {
    fit <- kppm(pattern ~ 1, clusters = "Thomas", method = method)
    values <- parameters(fit)
    list(method = method, kappa = unname(values[["kappa"]]),
         sigma = unname(values[["scale"]]), mu = unname(values[["mu"]]))
  })
  stats::setNames(rows, methods)
}
compare <- function(posterior, estimate) {
  list(
    estimate = estimate,
    posterior_median = posterior$median,
    posterior_interval_95 = c(posterior$quantile_025, posterior$quantile_975),
    ratio_to_posterior_median = estimate / posterior$median,
    inside_interval_95 = posterior$quantile_025 <= estimate && estimate <= posterior$quantile_975
  )
}

# Patterns.
data("redwood", package = "spatstat.data")
redwood_window <- c(0, 1, -1, 0)
redwood_scale <- list(kappa = 25, mu = 3, sigma = 0.05, dilation = 0.3)
cells <- utils::read.csv(schurch_csv, check.names = FALSE)
b_cells <- cells[cells[["ClusterName"]] == "B cells" & cells[["TMA_AB"]] == "A", ]
counts <- stats::aggregate(list(count = b_cells$CellID),
                           list(region = b_cells$`File Name`, patient = b_cells$patients), length)
counts <- counts[counts$count <= 600, ]
counts <- counts[order(-counts$count, counts$region), ]
counts <- counts[!duplicated(counts$patient), ][1:3, ]
um_per_pixel <- 0.377442
cell_window <- c(-0.5, 1919.5, -0.5, 1439.5) * um_per_pixel
cell_scale <- list(kappa = 10 / prod(diff(cell_window)[c(1, 3)]), mu = 20, sigma = 25, dilation = 150)
patterns <- list(redwood = list(
  x = redwood$x, y = redwood$y, window = redwood_window, scale = redwood_scale
))
for (region in counts$region) {
  rows <- b_cells[b_cells$`File Name` == region, ]
  patterns[[region]] <- list(x = rows$`X:X` * um_per_pixel, y = rows$`Y:Y` * um_per_pixel,
                             window = cell_window, scale = cell_scale)
}

# One-pattern fits and kppm.
comparisons <- list()
for (name in names(patterns)) {
  item <- patterns[[name]]
  posterior <- run_marklab(name, marklab_input(
    list(pattern_json(name, item$x, item$y, item$window)), item$scale
  ))
  window <- owin(item$window[1:2], item$window[3:4])
  estimates <- kppm_estimates(ppp(item$x, item$y, window = window))
  row <- posterior$patterns[[1]]
  comparisons[[name]] <- list(
    pattern_id = name, point_count = length(item$x),
    marklab_fit_state = posterior$fit_state,
    marklab_nonconvergence_reasons = posterior$nonconvergence_reasons,
    marklab_maximum_r_hat = posterior$diagnostics$maximum_r_hat,
    marklab_minimum_ess_bulk = posterior$diagnostics$minimum_ess_bulk,
    kppm = lapply(estimates, function(estimate) list(
      kappa = compare(row$kappa_parent_per_um2, estimate$kappa),
      mu = compare(row$mu_offspring, estimate$mu),
      sigma = compare(row$sigma_um, estimate$sigma)
    ))
  )
}

# Joint fit of the three B-cell spots.
joint <- run_marklab("schurch-b-cells-joint", marklab_input(
  unname(lapply(counts$region, function(region) {
    item <- patterns[[region]]
    pattern_json(region, item$x, item$y, item$window)
  })),
  cell_scale
))
write_json(list(
  format = "marklab.bayesian_thomas_kppm_comparison", version = 1L,
  criterion = "descriptive_no_agreement_tolerance",
  selected_b_cell_spots = counts,
  comparisons = unname(comparisons),
  joint_fit = list(fit_state = joint$fit_state,
                   nonconvergence_reasons = joint$nonconvergence_reasons,
                   population = joint$population, diagnostics = joint$diagnostics),
  sampler = list(chains = 4L, draws_per_chain = draws, warmup_per_chain = warmup),
  r_version = as.character(getRversion())
), file.path(output, "comparison.json"))
cat("wrote", file.path(output, "comparison.json"), "\n")
