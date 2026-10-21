#!/usr/bin/env Rscript
# Compare the production pathology L-r workflow with spatstat on published cells.
# Run from the repository root. R packages are validation tools, not Marklab backends.
# R_LIBS=/path/to/local/library Rscript tests/fixtures/inhomogeneous_spatial/compare_spatstat.R \
#   /path/to/marklab /path/to/new/output-directory

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 2L) stop("expected Marklab executable and a new output directory")
executable <- normalizePath(args[[1]], mustWork = TRUE)
output <- args[[2]]
if (dir.exists(output) || file.exists(output)) stop("output path must not exist")
pin_path <- "tests/fixtures/inhomogeneous_spatial/reference-packages.tsv"
pins <- utils::read.delim(pin_path, colClasses = "character", check.names = FALSE)
if (!identical(names(pins), c("package", "version")) || anyDuplicated(pins$package)) {
  stop("invalid reference package pin file: ", pin_path)
}
expected_r <- pins$version[pins$package == "R"]
if (length(expected_r) != 1L || as.character(getRversion()) != expected_r) {
  stop("reference comparison requires R ", expected_r, "; found ", getRversion())
}
package_pins <- pins[pins$package != "R", , drop = FALSE]
for (row in seq_len(nrow(package_pins))) {
  package <- package_pins$package[row]
  expected <- package_pins$version[row]
  if (!requireNamespace(package, quietly = TRUE)) stop("missing reference package: ", package)
  observed <- unname(utils::packageDescription(package, fields = "Version"))
  if (observed != expected) {
    stop("reference package ", package, " must be ", expected, "; found ", observed)
  }
}
dir.create(output, recursive = TRUE)
output <- normalizePath(output, mustWork = TRUE)
write_json <- function(value, path) {
  jsonlite::write_json(value, path, auto_unbox = TRUE, digits = NA, pretty = TRUE, null = "null")
}

# Exact-float workflow documents represent f64 values by their IEEE-754 bit
# integer. Decode those tags without routing the uint64 through an R double.
decimal_divide_256 <- function(value) {
  digits <- utf8ToInt(value) - utf8ToInt("0")
  carry <- 0L
  quotient <- integer(length(digits))
  for (index in seq_along(digits)) {
    current <- carry * 10L + digits[[index]]
    quotient[[index]] <- current %/% 256L
    carry <- current %% 256L
  }
  nonzero <- which(quotient != 0L)
  first <- if (length(nonzero) == 0L) NA_integer_ else nonzero[[1]]
  list(quotient = if (is.na(first)) "0" else paste0(quotient[first:length(quotient)], collapse = ""),
       remainder = carry)
}
bits_to_double <- function(bits) {
  remaining <- as.character(bits)
  bytes <- integer(8L)
  for (index in seq_along(bytes)) {
    divided <- decimal_divide_256(remaining)
    remaining <- divided$quotient
    bytes[[index]] <- divided$remainder
  }
  if (remaining != "0") stop("exact f64 bit integer exceeds uint64")
  readBin(as.raw(bytes), what = "double", n = 1L, size = 8L, endian = "little")
}
decode_exact_float_tags <- function(value) {
  if (is.list(value) && identical(names(value), "__marklab_f64_bits")) {
    return(bits_to_double(value[[1]]))
  }
  if (is.list(value)) {
    decoded <- lapply(value, decode_exact_float_tags)
    names(decoded) <- names(value)
    return(decoded)
  }
  value
}
read_exact_json <- function(path) {
  decode_exact_float_tags(jsonlite::fromJSON(
    path, simplifyVector = FALSE, bigint_as_char = TRUE
  ))
}
scaled_error <- function(observed, reference) {
  if (length(observed) == 0L || length(observed) != length(reference) ||
      any(!is.finite(observed)) || any(!is.finite(reference))) return(Inf)
  max(abs(observed - reference) / pmax(1, abs(reference)))
}
optional_scaled_error <- function(observed, reference) {
  if (length(observed) == 0L || length(observed) != length(reference) ||
      !identical(is.finite(observed), is.finite(reference))) return(Inf)
  finite <- is.finite(observed)
  if (!any(finite)) return(Inf)
  scaled_error(observed[finite], reference[finite])
}
optional_curve_value <- function(point, name) {
  value <- point[[name]]
  if (is.null(value)) NA_real_ else value
}
fv_column <- function(value, name) {
  if (name %in% names(value)) value[[name]] else rep(NA_real_, length(value$r))
}

# Set tolerances before evaluating any dataset. Exact pair-counting agreement is
# separate from the declared cell-centre quadrature error in the intensity fit.
tolerances <- list(matched_intensity_k_scaled = 1e-10, analytic_intensity_relative = 1e-3,
                   analytic_intensity_k_scaled = 2e-3, l_transform_absolute_um = 1e-10,
                   get_absolute = 1e-12, convention_algebra_scaled = 1e-10,
                   homogeneous_k_scaled = 1e-10,
                   homogeneous_l_absolute_um = 1e-10,
                   cross_k_scaled = 1e-10, cross_l_absolute_um = 1e-10,
                   cross_reference_count_absolute = 1e-9,
                   homogeneous_g_scaled = 1e-10, cross_g_scaled = 1e-10,
                   inhomogeneous_cross_g_scaled = 1e-10,
                   nearest_space_scaled = 1e-10,
                   null_curve_get_p_absolute = 1e-12,
                   null_curve_get_bound_scaled = 1e-12)
settings <- list(grid = c(64L, 64L), bandwidth_short_side_fraction = 0.15,
                 radii_short_side_fractions = (1:16) / 80, simulations = 39L,
                 alpha = 0.05, seed = 20261010L,
                 cross_k_reference_subdivisions = 1024L,
                 pair_bandwidth_short_side_fraction = 1 / 40,
                 pair_radii_short_side_fractions = (3:18) / 80,
                 cross_g_reference_subdivisions = 1024L,
                 nearest_space_reference_subdivisions = 128L,
                 nearest_space_j_denominator_epsilon = 1e-12)

# The checked-in tie case is also consumed by the existing native ERL test.
# Re-evaluate it with the actual R package, rather than reimplementing GET.
oracle <- jsonlite::fromJSON("tests/fixtures/get_erl_1_0_7_oracle.json")
get_result <- GET::global_envelope_test(
  GET::create_curve_set(list(r = seq_along(oracle$observed), obs = oracle$observed,
                            sim_m = t(oracle$permutations))),
  type = "erl", alpha = oracle$alpha, alternative = "two.sided"
)
get_errors <- c(lower = max(abs(get_result$lo - oracle$lower)),
                upper = max(abs(get_result$hi - oracle$upper)),
                p_value = abs(attr(get_result, "p") - oracle$p_global),
                depths = max(abs(attr(get_result, "M") - oracle$erl_depths)),
                critical_depth = abs(attr(get_result, "M_alpha") - oracle$critical_depth))

# Both recordings have documented physical scales. rescale() uses amacrine's
# supplied 662-micron unit; betacells is already in microns. Neither recording
# supplies independent patient groups, and we do not manufacture replicate patients.
data_env <- new.env()
utils::data(list = c("amacrine", "betacells"), package = "spatstat.data", envir = data_env)
patterns <- list(amacrine = spatstat.geom::rescale(data_env$amacrine),
                 betacells = data_env$betacells)
patterns$betacells <- spatstat.geom::setmarks(
  patterns$betacells, spatstat.geom::marks(patterns$betacells)$type
)

rectangle <- function(window) {
  x <- window$xrange
  y <- window$yrange
  ring <- rbind(c(x[1], y[1]), c(x[2], y[1]), c(x[2], y[2]),
                c(x[1], y[2]), c(x[1], y[1]))
  list(type = "MultiPolygon", coordinates = list(list(unname(ring))))
}

recipe <- function(pattern, name, phenotype) {
  window <- spatstat.geom::Window(pattern)
  short_side <- min(diff(window$xrange), diff(window$yrange))
  labels <- as.character(spatstat.geom::marks(pattern))
  cells <- lapply(seq_len(pattern$n), function(i) {
    list(id = sprintf("cell-%05d", i), x_um = pattern$x[i], y_um = pattern$y[i],
         phenotype = labels[i], stratum = "all")
  })
  list(
    format = "marklab.pathology_spatial_study_recipe", version = 1L,
    study = list(study_id = paste0("published-reference-", name),
                 phenotypes = list(names = sort(unique(labels)), measurement_status = "measured",
                                   provenance = paste("spatstat.data", name, "published cell types")),
                 slides = list(list(slide_id = name, patient_id = name, group = "reference",
                                    coordinate_frame_id = paste0(name, "-microns"),
                                    window = rectangle(window), cells = cells))),
    design = list(group_a = "reference", group_b = "unused", permutations = settings$simulations,
                  seed = settings$seed, alpha = settings$alpha, statistic = "l_minus_r",
                  phenotype = phenotype, source = NULL, target = NULL,
                  radii_um = short_side * settings$radii_short_side_fractions,
                  pair_bandwidth_um = short_side / 40,
                  intensity = list(model = "gaussian", bandwidth_um = short_side *
                                     settings$bandwidth_short_side_fraction,
                                   grid = settings$grid, minimum_intensity_per_um2 = 1e-12,
                                   cross_fit_folds = NULL, bandwidth_candidates_um = list(),
                                   null_mode = "plugin"), compartments = list()),
    limits = list(maximum_cells = 1000L, maximum_work = 1000000000,
                  maximum_geometry_vertices = 1000L, maximum_tiles = 10000L,
                  memory_budget_bytes = 268435456L)
  )
}

# Re-run a slide's within-slide ERL test with GET on Marklab's exported simulated curves,
# restricted to the radii eligible in the observed and every simulated curve.
null_curve_get_check <- function(result_dir, slide, radii) {
  null_curves <- jsonlite::fromJSON(file.path(result_dir, "null-curves.json"),
                                    simplifyVector = FALSE)
  exported <- null_curves$slides[[1]]
  optional_number <- function(value) if (is.null(value)) NA_real_ else value
  eligible <- vapply(exported$jointly_eligible, isTRUE, FALSE)
  observed_curve <- vapply(exported$observed, optional_number, 0.0)
  simulated_curves <- vapply(exported$simulated, function(curve) {
    vapply(curve, optional_number, 0.0)
  }, numeric(length(radii)))
  get_null <- GET::global_envelope_test(
    GET::create_curve_set(list(r = radii[eligible], obs = observed_curve[eligible],
                               sim_m = simulated_curves[eligible, , drop = FALSE])),
    type = "erl", alpha = null_curves$alpha, alternative = "two.sided"
  )
  native_lower <- vapply(slide$support, function(p) optional_curve_value(p, "lower_l"), 0.0)
  native_upper <- vapply(slide$support, function(p) optional_curve_value(p, "upper_l"), 0.0)
  p_error <- if (identical(exported$native_p_global, slide$p_global)) {
    abs(attr(get_null, "p") - exported$native_p_global)
  } else Inf
  bound_error <- max(
    scaled_error((native_lower - radii)[eligible], get_null$lo),
    scaled_error((native_upper - radii)[eligible], get_null$hi)
  )
  list(marklab_p_global = exported$native_p_global, get_p_global = attr(get_null, "p"),
       p_absolute_error = p_error, bound_scaled_error = bound_error,
       passed = is.finite(p_error) && is.finite(bound_error) &&
         length(exported$simulated) == settings$simulations && all(eligible) &&
         p_error <= tolerances$null_curve_get_p_absolute &&
         bound_error <= tolerances$null_curve_get_bound_scaled)
}

rows <- list()
for (name in names(patterns)) {
  original <- patterns[[name]]
  for (selection in c("all", "off", "on")) {
    id <- paste(name, selection, sep = "-")
    case_dir <- file.path(output, id)
    dir.create(case_dir)
    input <- recipe(original, name, if (selection == "all") NULL else selection)
    input_path <- file.path(case_dir, "recipe.json")
    result_dir <- file.path(case_dir, "marklab")
    write_json(input, input_path)
    status <- system2(executable,
                      c("pathology", "spatial-study", "--recipe", shQuote(input_path),
                        "--out", shQuote(result_dir)),
                      stdout = file.path(case_dir, "command.log"),
                      stderr = file.path(case_dir, "command.log"))
    if (status != 0L) stop(id, ": Marklab failed; see command.log")
    result <- jsonlite::fromJSON(file.path(result_dir, "result.json"), simplifyVector = FALSE)
    slide <- result$slides[[1]]
    if (!is.null(slide$unavailable)) stop(id, ": ", slide$unavailable)
    if (!is.null(result$inference$p_value)) stop("single specimen unexpectedly produced a group test")
    x <- if (selection == "all") original else {
      original[spatstat.geom::marks(original) == selection]
    }
    x <- spatstat.geom::unmark(x)
    points <- slide$intensity$point_values
    stopifnot(length(points) == x$n,
              identical(vapply(points, function(p) as.integer(p$row), 0L), seq_len(x$n) - 1L))
    lambda <- vapply(points, function(p) p$intensity_per_um2, 0.0)
    radii <- input$design$radii_um
    k_native <- vapply(slide$support, function(p) p$k, 0.0)
    l_minus_r <- unlist(slide$values)

    # Kinhom's default renormalisation is deliberately disabled. Supply precisely
    # the same event intensities to isolate its independent border pair estimator.
    matched <- spatstat.explore::Kinhom(x, lambda = lambda, r = c(0, radii),
                                       correction = "border", renormalise = FALSE)$border[-1]

    # A separate Gaussian KDE reference uses analytic rectangle mass (pnorm),
    # avoiding Marklab's grid quadrature and spatstat's pixel interpolation.
    bandwidth <- input$design$intensity$bandwidth_um
    distance <- spatstat.geom::pairdist(x)
    kernel <- exp(-0.5 * (distance / bandwidth)^2) / (2 * pi * bandwidth^2)
    diag(kernel) <- 0
    window <- spatstat.geom::Window(x)
    mass <- (pnorm((window$xrange[2] - x$x) / bandwidth) -
               pnorm((window$xrange[1] - x$x) / bandwidth)) *
      (pnorm((window$yrange[2] - x$y) / bandwidth) -
         pnorm((window$yrange[1] - x$y) / bandwidth))
    # Marklab estimates the n-event binomial-process intensity from n-1 training
    # events. Ordinary Poisson leave-one-out intensity omits this finite-count
    # multiplier. Report both intensity conventions with and without Kinhom's
    # separate renormalisation instead of aligning only the reference that passes.
    analytic_lambda_raw <- rowSums(kernel) / mass
    analytic_lambda_scaled <- (x$n / (x$n - 1)) * analytic_lambda_raw
    k_raw_unrenormalised <- spatstat.explore::Kinhom(
      x, lambda = analytic_lambda_raw, r = c(0, radii), correction = "border",
      renormalise = FALSE
    )$border[-1]
    k_scaled_unrenormalised <- spatstat.explore::Kinhom(
      x, lambda = analytic_lambda_scaled, r = c(0, radii), correction = "border",
      renormalise = FALSE
    )$border[-1]
    k_raw_renormalised <- spatstat.explore::Kinhom(
      x, lambda = analytic_lambda_raw, r = c(0, radii), correction = "border",
      renormalise = TRUE, normpower = 1
    )$border[-1]
    k_scaled_renormalised <- spatstat.explore::Kinhom(
      x, lambda = analytic_lambda_scaled, r = c(0, radii), correction = "border",
      renormalise = TRUE, normpower = 1
    )$border[-1]
    # This is the package's actual default intensity and renormalisation pipeline:
    # lambda is omitted, leave-one-out is TRUE and renormalise is TRUE.
    k_spatstat_default <- spatstat.explore::Kinhom(
      x, sigma = bandwidth, r = c(0, radii), correction = "border"
    )$border[-1]
    independent <- k_scaled_unrenormalised
    matched_error <- max(abs(k_native - matched) / pmax(1, abs(matched)))
    intensity_error <- max(abs(lambda / analytic_lambda_scaled - 1))
    independent_error <- max(abs(k_native - independent) / pmax(1, abs(independent)))
    expected_scale <- (x$n - 1) / x$n
    unrenormalised_scale_error <- max(
      abs(k_scaled_unrenormalised - expected_scale * k_raw_unrenormalised) /
        pmax(1, abs(k_scaled_unrenormalised))
    )
    renormalised_scale_error <- max(
      abs(k_scaled_renormalised - k_raw_renormalised) /
        pmax(1, abs(k_scaled_renormalised))
    )
    default_k_scaled_difference <- max(
      abs(k_native - k_spatstat_default) / pmax(1, abs(k_spatstat_default))
    )
    # Validate the L transform separately. Kinhom can leave roundoff of either
    # sign at genuinely zero-pair radii; retain those raw values in curves.csv.
    l_error <- max(abs(l_minus_r - (sqrt(k_native / pi) - radii)))

    null_check <- null_curve_get_check(result_dir, slide, radii)
    passed <- all(is.finite(c(matched_error, intensity_error, independent_error, l_error,
                              unrenormalised_scale_error, renormalised_scale_error,
                              default_k_scaled_difference))) &&
      null_check$passed &&
      matched_error <= tolerances$matched_intensity_k_scaled &&
      intensity_error <= tolerances$analytic_intensity_relative &&
      independent_error <= tolerances$analytic_intensity_k_scaled &&
      l_error <= tolerances$l_transform_absolute_um &&
      unrenormalised_scale_error <= tolerances$convention_algebra_scaled &&
      renormalised_scale_error <= tolerances$convention_algebra_scaled
    utils::write.csv(data.frame(radius_um = radii, k_marklab = k_native,
                               k_spatstat_matched_intensity = matched,
                               k_analytic_raw_unrenormalised = k_raw_unrenormalised,
                               k_analytic_scaled_unrenormalised = k_scaled_unrenormalised,
                               k_analytic_raw_renormalised = k_raw_renormalised,
                               k_analytic_scaled_renormalised = k_scaled_renormalised,
                               k_spatstat_default = k_spatstat_default,
                               l_minus_r_marklab = l_minus_r),
                     file.path(case_dir, "curves.csv"), row.names = FALSE)
    rows[[length(rows) + 1L]] <- list(
      case = id, cells = x$n, radii = length(radii), matched_intensity_k_scaled_error = matched_error,
      analytic_intensity_relative_error = intensity_error,
      analytic_intensity_k_scaled_error = independent_error, l_transform_absolute_error_um = l_error,
      raw_to_scaled_unrenormalised_k_error = unrenormalised_scale_error,
      raw_to_scaled_renormalised_k_error = renormalised_scale_error,
      marklab_to_spatstat_default_k_scaled_difference = default_k_scaled_difference,
      null_curve_marklab_p_global = null_check$marklab_p_global,
      null_curve_get_p_global = null_check$get_p_global,
      null_curve_get_p_absolute_error = null_check$p_absolute_error,
      null_curve_get_bound_scaled_error = null_check$bound_scaled_error,
      passed = passed
    )
    cat(id, "cells", x$n, "K error", format(matched_error, digits = 4),
        "intensity error", format(intensity_error, digits = 4),
        "GET null-curve p error", format(null_check$p_absolute_error, digits = 4),
        "passed", passed, "\n")
  }
}

# The published retinal curves are each the most extreme of their simulations, so their GET
# p-values only test the extreme rank. A pattern drawn from an inhomogeneous Poisson process,
# fixed before running, checks a non-extreme within-slide p-value with the same tolerances.
set.seed(settings$seed)
synthetic <- spatstat.random::rpoispp(
  function(x, y) 2e-4 * exp(1.5 * x / 1000), win = spatstat.geom::owin(c(0, 1000), c(0, 1000))
)
synthetic <- spatstat.geom::setmarks(synthetic, factor(rep("cell", synthetic$n)))
synthetic_dir <- file.path(output, "synthetic-inhomogeneous-poisson")
dir.create(synthetic_dir)
synthetic_recipe <- recipe(synthetic, "synthetic-inhomogeneous-poisson", NULL)
synthetic_recipe$study$phenotypes$names <- list("cell")  # keep a JSON array of one name
synthetic_recipe$study$phenotypes$provenance <-
  "spatstat.random rpoispp, intensity 2e-4 exp(1.5 x / 1000) per square micron"
write_json(synthetic_recipe, file.path(synthetic_dir, "recipe.json"))
status <- system2(executable,
                  c("pathology", "spatial-study", "--recipe",
                    shQuote(file.path(synthetic_dir, "recipe.json")), "--out",
                    shQuote(file.path(synthetic_dir, "marklab"))),
                  stdout = file.path(synthetic_dir, "command.log"),
                  stderr = file.path(synthetic_dir, "command.log"))
if (status != 0L) stop("synthetic null-curve case: Marklab failed; see command.log")
synthetic_result <- jsonlite::fromJSON(file.path(synthetic_dir, "marklab", "result.json"),
                                       simplifyVector = FALSE)
synthetic_check <- null_curve_get_check(file.path(synthetic_dir, "marklab"),
                                        synthetic_result$slides[[1]],
                                        synthetic_recipe$design$radii_um)
synthetic_check$cells <- synthetic$n
cat("synthetic inhomogeneous Poisson cells", synthetic$n, "Marklab p",
    synthetic_check$marklab_p_global, "GET p", synthetic_check$get_p_global,
    "passed", synthetic_check$passed, "\n")

# Run the three public homogeneous K/L workflows against Kest on an exact fine
# grid containing every requested radius. Also retain sparse-grid and package-
# default curves so the reference convention remains visible rather than being
# silently tuned to the equality target.
homogeneous_rows <- list()
for (name in names(patterns)) {
  x <- patterns[[name]]
  window <- spatstat.geom::Window(x)
  short_side <- min(diff(window$xrange), diff(window$yrange))
  radii <- short_side * settings$radii_short_side_fractions
  case_dir <- file.path(output, paste0(name, "-homogeneous-k-l"))
  dir.create(case_dir)
  cells_path <- file.path(case_dir, "cells.csv")
  window_path <- file.path(case_dir, "window.geojson")
  labels <- as.character(spatstat.geom::marks(x))
  utils::write.csv(
    data.frame(cell_id = sprintf("%s:%09d", name, seq_len(x$n) - 1L),
               x_um = x$x, y_um = x$y, mark = 0L, case_id = name,
               timepoint = "published", protein = "spatstat.data",
               valid_tumor = "true", valid_ihc = "true", slide_id = name,
               histologic_compartment = labels),
    cells_path, row.names = FALSE, quote = FALSE
  )
  write_json(rectangle(window), window_path)
  maximum_pairs <- x$n * (x$n - 1L) / 2 * (settings$simulations + 1L)
  maximum_draws <- x$n * settings$simulations * 2L
  common <- c("--cells", shQuote(cells_path), "--mask", shQuote(window_path),
              "--radii-um", paste(radii, collapse = ","),
              "--simulations", settings$simulations, "--seed", settings$seed,
              "--alpha", settings$alpha, "--memory-budget-mib", 256L,
              "--max-pair-visits", maximum_pairs,
              "--max-csr-draws", maximum_draws)
  translation_path <- file.path(case_dir, "translation.json")
  status <- system2(
    executable,
    c("project", "translation-spatial", "--project",
      shQuote(file.path(case_dir, "translation-project")), "--out",
      shQuote(translation_path), common,
      "--max-overlap-evaluations", maximum_pairs,
      "--max-overlap-candidate-work", maximum_pairs * 128,
      "--max-overlap-output-vertices", 10000L),
    stdout = file.path(case_dir, "translation-command.log"),
    stderr = file.path(case_dir, "translation-command.log")
  )
  if (status != 0L) stop(name, ": Marklab translation K/L failed")
  isotropic_path <- file.path(case_dir, "isotropic.json")
  status <- system2(
    executable,
    c("project", "isotropic-spatial", "--project",
      shQuote(file.path(case_dir, "isotropic-project")), "--out",
      shQuote(isotropic_path), common,
      "--max-visible-arc-evaluations", maximum_pairs * 2,
      "--max-arc-segment-tests", maximum_pairs * 256,
      "--max-arc-membership-queries", maximum_pairs * 256),
    stdout = file.path(case_dir, "isotropic-command.log"),
    stderr = file.path(case_dir, "isotropic-command.log")
  )
  if (status != 0L) stop(name, ": Marklab isotropic K/L failed")
  border_dir <- file.path(case_dir, "border")
  status <- system2(
    executable,
    c("project", "classical", "--project",
      shQuote(file.path(case_dir, "border-project")), "--cells", shQuote(cells_path),
      "--mask", shQuote(window_path), "--out", shQuote(border_dir),
      "--r-max-um", max(radii), "--r-steps", length(radii),
      "--simulations", settings$simulations, "--seed", settings$seed,
      "--alpha", settings$alpha, "--memory-budget-mib", 256L,
      "--max-pair-visits", maximum_pairs, "--max-csr-draws", maximum_draws),
    stdout = file.path(case_dir, "border-command.log"),
    stderr = file.path(case_dir, "border-command.log")
  )
  if (status != 0L) stop(name, ": Marklab border K/L failed")
  native_documents <- list(
    border = jsonlite::fromJSON(file.path(border_dir, "result.json"), simplifyVector = FALSE),
    translation = jsonlite::fromJSON(translation_path, simplifyVector = FALSE),
    isotropic = jsonlite::fromJSON(isotropic_path, simplifyVector = FALSE)
  )
  native <- lapply(native_documents, function(document) {
    list(k = vapply(document$analysis$curve, function(point) point$k, 0.0),
         l = vapply(document$analysis$curve, function(point) point$l, 0.0))
  })
  subdivisions <- settings$cross_k_reference_subdivisions
  reference_r <- seq(0, max(radii), length.out = length(radii) * subdivisions + 1L)
  reference_index <- seq_along(radii) * subdivisions + 1L
  reference <- spatstat.explore::Kest(
    x, r = reference_r, correction = c("border", "translation", "isotropic")
  )
  if (max(abs(reference$r[reference_index] - radii)) > .Machine$double.eps *
        max(1, max(radii)) * 4) {
    stop(name, ": requested radii are not represented by the Kest reference grid")
  }
  reference_k <- list(border = reference$border[reference_index],
                      translation = reference$trans[reference_index],
                      isotropic = reference$iso[reference_index])
  sparse <- spatstat.explore::Kest(
    x, r = c(0, radii), correction = c("border", "translation", "isotropic")
  )
  sparse_k <- list(border = sparse$border[-1], translation = sparse$trans[-1],
                   isotropic = sparse$iso[-1])
  default_reference <- spatstat.explore::Kest(x)
  utils::write.csv(
    data.frame(radius_um = default_reference$r,
               k_spatstat_border = default_reference$border,
               k_spatstat_translation = default_reference$trans,
               k_spatstat_isotropic = default_reference$iso),
    file.path(case_dir, "spatstat-default-grid.csv"), row.names = FALSE
  )
  curves <- data.frame(radius_um = radii)
  for (correction in names(native)) {
    k_marklab <- native[[correction]]$k
    l_marklab <- native[[correction]]$l
    k_reference <- reference_k[[correction]]
    l_reference <- sqrt(pmax(k_reference, 0) / pi)
    k_error <- max(abs(k_marklab - k_reference) / pmax(1, abs(k_reference)))
    l_error <- max(abs(l_marklab - l_reference))
    sparse_k_error <- max(
      abs(k_marklab - sparse_k[[correction]]) / pmax(1, abs(sparse_k[[correction]]))
    )
    sparse_l_error <- max(
      abs(l_marklab - sqrt(pmax(sparse_k[[correction]], 0) / pi))
    )
    passed <- all(is.finite(c(k_error, l_error, sparse_k_error, sparse_l_error))) &&
      k_error <= tolerances$homogeneous_k_scaled &&
      l_error <= tolerances$homogeneous_l_absolute_um
    curves[[paste0("k_marklab_", correction)]] <- k_marklab
    curves[[paste0("l_marklab_", correction)]] <- l_marklab
    curves[[paste0("k_spatstat_", correction)]] <- k_reference
    homogeneous_rows[[length(homogeneous_rows) + 1L]] <- list(
      case = paste(name, correction, sep = "-"), correction = correction,
      cells = x$n, radii = length(radii), reference_grid_points = length(reference_r),
      reference_grid_spacing_um = reference_r[[2]],
      homogeneous_k_scaled_error = k_error,
      homogeneous_l_absolute_error_um = l_error,
      sparse_grid_k_scaled_error = sparse_k_error,
      sparse_grid_l_absolute_error_um = sparse_l_error,
      spatstat_default_grid_points = length(default_reference$r),
      spatstat_default_grid_spacing_um = default_reference$r[[2]],
      spatstat_default_grid_maximum_um = max(default_reference$r),
      spatstat_default_preferred = attr(default_reference, "valu"),
      passed = passed
    )
    cat(name, correction, "K error", format(k_error, digits = 4),
        "L error", format(l_error, digits = 4), "passed", passed, "\n")
  }
  utils::write.csv(curves, file.path(case_dir, "curves.csv"), row.names = FALSE)
}

# The categorical-pair command already uses the standard reduced-sample border
# estimator. Run both directed off/on curves through that public CLI and compare
# them with Kcross. Kcross's default invocation also reports translation and
# isotropic curves; retain their differences descriptively because they are
# different edge estimators, not alternate equality targets.
cross_rows <- list()
for (name in names(patterns)) {
  x <- patterns[[name]]
  window <- spatstat.geom::Window(x)
  short_side <- min(diff(window$xrange), diff(window$yrange))
  radii <- short_side * settings$radii_short_side_fractions
  case_dir <- file.path(output, paste0(name, "-cross-k-l"))
  dir.create(case_dir)
  cells_path <- file.path(case_dir, "cells.csv")
  window_path <- file.path(case_dir, "window.geojson")
  labels <- as.character(spatstat.geom::marks(x))
  utils::write.csv(
    data.frame(cell_id = sprintf("%s:%09d", name, seq_len(x$n) - 1L),
               x_um = x$x, y_um = x$y, mark = 0L, case_id = name,
               timepoint = "published", protein = "spatstat.data",
               valid_tumor = "true", valid_ihc = "true", slide_id = name,
               histologic_compartment = labels),
    cells_path, row.names = FALSE, quote = FALSE
  )
  write_json(rectangle(window), window_path)
  for (levels in list(c("off", "on"), c("on", "off"))) {
    source <- levels[[1]]
    target <- levels[[2]]
    id <- paste(name, source, target, sep = "-")
    direction_dir <- file.path(case_dir, paste(source, target, sep = "-to-"))
    dir.create(direction_dir)
    project_dir <- file.path(direction_dir, "project")
    result_path <- file.path(direction_dir, "result.json")
    command_log <- file.path(direction_dir, "command.log")
    maximum_pairs <- x$n * (x$n - 1L)
    maximum_null_pairs <- maximum_pairs * settings$simulations
    status <- system2(
      executable,
      c("project", "categorical-pair", "--project", shQuote(project_dir),
        "--cells", shQuote(cells_path), "--mask", shQuote(window_path),
        "--out", shQuote(result_path), "--source-level", source,
        "--target-level", target, "--radii-um", paste(radii, collapse = ","),
        "--permutations", settings$simulations, "--seed", settings$seed,
        "--alpha", settings$alpha, "--memory-budget-mib", 256L,
        "--max-pair-visits", maximum_pairs,
        "--max-null-pair-evaluations", maximum_null_pairs),
      stdout = command_log, stderr = command_log
    )
    if (status != 0L) stop(id, ": Marklab cross-K/L failed; see ", command_log)
    result <- jsonlite::fromJSON(result_path, simplifyVector = FALSE)
    if (result$format != "marklab.categorical-pair/2") {
      stop(id, ": unexpected categorical-pair result format")
    }
    k_marklab <- vapply(result$curve, function(point) point$cross_k, 0.0)
    l_marklab <- vapply(result$curve, function(point) point$cross_l, 0.0)
    # Kcross forms cumulative counts from a histogram on the supplied r grid.
    # A grid containing only the requested radii assigns events and border-risk
    # changes to intervals that are too wide for pointwise agreement. Evaluate
    # its independent implementation on a declared fine grid and select the
    # exact requested grid positions.
    subdivisions <- settings$cross_k_reference_subdivisions
    reference_r <- seq(0, max(radii), length.out = length(radii) * subdivisions + 1L)
    reference_index <- seq_along(radii) * subdivisions + 1L
    reference <- spatstat.explore::Kcross(
      x, source, target, r = reference_r, ratio = TRUE
    )
    if (max(abs(reference$r[reference_index] - radii)) > .Machine$double.eps *
          max(1, max(radii)) * 4) {
      stop(id, ": requested radii are not represented by the Kcross reference grid")
    }
    k_border <- reference$border[reference_index]
    k_translation <- reference$trans[reference_index]
    k_isotropic <- reference$iso[reference_index]
    source_pattern <- x[labels == source]
    target_pattern <- x[labels == target]
    source_boundary <- spatstat.geom::bdist.points(source_pattern)
    cross_distance <- spatstat.geom::crossdist(source_pattern, target_pattern)
    direct_eligible <- vapply(
      radii, function(radius) sum(source_boundary >= radius), 0L
    )
    direct_pairs <- vapply(radii, function(radius) {
      eligible <- matrix(source_boundary >= radius, nrow = source_pattern$n,
                         ncol = target_pattern$n)
      sum(cross_distance <= radius & eligible)
    }, 0L)
    reference_numerator <- attr(reference, "numerator")$border[reference_index]
    reference_denominator <- attr(reference, "denominator")$border[reference_index]
    count_error <- max(
      abs(reference_numerator / spatstat.geom::area.owin(window) - direct_pairs),
      abs(reference_denominator / target_pattern$n - direct_eligible)
    )
    if (!is.finite(count_error) ||
        count_error > tolerances$cross_reference_count_absolute) {
      stop(id, ": fine Kcross grid does not recover direct border counts")
    }
    sparse_reference <- spatstat.explore::Kcross(
      x, source, target, r = c(0, radii)
    )
    sparse_k <- sparse_reference$border[-1]
    sparse_k_error <- max(abs(k_marklab - sparse_k) / pmax(1, abs(sparse_k)))
    sparse_l_error <- max(abs(l_marklab - sqrt(pmax(sparse_k, 0) / pi)))
    default_reference <- spatstat.explore::Kcross(x, source, target)
    utils::write.csv(
      data.frame(radius_um = default_reference$r,
                 cross_k_spatstat_border = default_reference$border,
                 cross_k_spatstat_translation = default_reference$trans,
                 cross_k_spatstat_isotropic = default_reference$iso),
      file.path(direction_dir, "spatstat-default-grid.csv"), row.names = FALSE
    )
    k_error <- max(abs(k_marklab - k_border) / pmax(1, abs(k_border)))
    l_error <- max(abs(l_marklab - sqrt(pmax(k_border, 0) / pi)))
    translation_difference <- max(
      abs(k_marklab - k_translation) / pmax(1, abs(k_translation))
    )
    isotropic_difference <- max(
      abs(k_marklab - k_isotropic) / pmax(1, abs(k_isotropic))
    )
    passed <- all(is.finite(c(k_error, l_error, translation_difference,
                              isotropic_difference))) &&
      k_error <= tolerances$cross_k_scaled &&
      l_error <= tolerances$cross_l_absolute_um
    utils::write.csv(
      data.frame(radius_um = radii, cross_k_marklab = k_marklab,
                 cross_l_marklab = l_marklab, cross_k_spatstat_border = k_border,
                 cross_k_spatstat_translation = k_translation,
                 cross_k_spatstat_isotropic = k_isotropic),
      file.path(direction_dir, "curves.csv"), row.names = FALSE
    )
    cross_rows[[length(cross_rows) + 1L]] <- list(
      case = id, source = source, target = target, cells = x$n,
      source_cells = sum(labels == source), target_cells = sum(labels == target),
      radii = length(radii), reference_grid_points = length(reference_r),
      reference_grid_spacing_um = reference_r[[2]],
      reference_border_count_absolute_error = count_error,
      sparse_grid_border_cross_k_scaled_error = sparse_k_error,
      sparse_grid_border_cross_l_absolute_error_um = sparse_l_error,
      spatstat_default_grid_points = length(default_reference$r),
      spatstat_default_grid_spacing_um = default_reference$r[[2]],
      spatstat_default_grid_maximum_um = max(default_reference$r),
      spatstat_default_preferred = attr(default_reference, "valu"),
      border_cross_k_scaled_error = k_error, border_cross_l_absolute_error_um = l_error,
      translation_cross_k_scaled_difference = translation_difference,
      isotropic_cross_k_scaled_difference = isotropic_difference,
      passed = passed
    )
    cat(id, "cross-K error", format(k_error, digits = 4),
        "cross-L error", format(l_error, digits = 4), "passed", passed, "\n")
  }
}

# Homogeneous translation and isotropic g use the same explicit Epanechnikov
# half-width, r divisor, and no zero-distance correction as pcf.ppp. Its
# fast=FALSE path evaluates the analytic kernel sum and is the equality oracle.
homogeneous_g_rows <- list()
cross_g_rows <- list()
for (name in names(patterns)) {
  x <- patterns[[name]]
  unmarked <- spatstat.geom::unmark(x)
  window <- spatstat.geom::Window(x)
  area <- spatstat.geom::area.owin(window)
  short_side <- min(diff(window$xrange), diff(window$yrange))
  bandwidth <- short_side * settings$pair_bandwidth_short_side_fraction
  radii <- short_side * settings$pair_radii_short_side_fractions
  reference_r <- seq(0, max(radii), by = short_side / 80)
  reference_index <- 3:18 + 1L
  case_dir <- file.path(output, paste0(name, "-g"))
  dir.create(case_dir)
  source_dir <- file.path(output, paste0(name, "-homogeneous-k-l"))
  cells_path <- file.path(source_dir, "cells.csv")
  window_path <- file.path(source_dir, "window.geojson")
  maximum_pairs <- x$n * (x$n - 1L) / 2 * (settings$simulations + 1L)
  maximum_draws <- x$n * settings$simulations * 2L
  common <- c("--cells", shQuote(cells_path), "--mask", shQuote(window_path),
              "--radii-um", paste(radii, collapse = ","),
              "--bandwidth-um", bandwidth,
              "--simulations", settings$simulations, "--seed", settings$seed,
              "--alpha", settings$alpha, "--memory-budget-mib", 256L,
              "--max-pair-visits", maximum_pairs,
              "--max-csr-draws", maximum_draws)
  translation_path <- file.path(case_dir, "translation.json")
  status <- system2(
    executable,
    c("project", "translation-pair-correlation", "--project",
      shQuote(file.path(case_dir, "translation-project")), "--out",
      shQuote(translation_path), common,
      "--max-overlap-evaluations", maximum_pairs,
      "--max-overlap-candidate-work", maximum_pairs * 128,
      "--max-overlap-output-vertices", 10000L),
    stdout = file.path(case_dir, "translation-command.log"),
    stderr = file.path(case_dir, "translation-command.log")
  )
  if (status != 0L) stop(name, ": Marklab translation g failed")
  isotropic_path <- file.path(case_dir, "isotropic.json")
  status <- system2(
    executable,
    c("project", "isotropic-pair-correlation", "--project",
      shQuote(file.path(case_dir, "isotropic-project")), "--out",
      shQuote(isotropic_path), common,
      "--max-visible-arc-evaluations", maximum_pairs * 2,
      "--max-arc-segment-tests", maximum_pairs * 256,
      "--max-arc-membership-queries", maximum_pairs * 256),
    stdout = file.path(case_dir, "isotropic-command.log"),
    stderr = file.path(case_dir, "isotropic-command.log")
  )
  if (status != 0L) stop(name, ": Marklab isotropic g failed")
  translation <- jsonlite::fromJSON(translation_path, simplifyVector = FALSE)
  isotropic <- jsonlite::fromJSON(isotropic_path, simplifyVector = FALSE)
  native_translation <- vapply(translation$analysis$curve, function(point) point$g, 0.0)
  native_isotropic <- vapply(isotropic$analysis$curve, function(point) point$g, 0.0)
  reference <- spatstat.explore::pcf.ppp(
    unmarked, r = reference_r, h = bandwidth, kernel = "epanechnikov",
    correction = c("translate", "Ripley"), divisor = "r", zerocor = "none",
    fast = FALSE
  )
  reference_translation <- reference$trans[reference_index]
  reference_isotropic <- reference$iso[reference_index]
  translation_error <- scaled_error(native_translation, reference_translation)
  isotropic_error <- scaled_error(native_isotropic, reference_isotropic)
  passed <- all(is.finite(c(translation_error, isotropic_error))) &&
    translation_error <= tolerances$homogeneous_g_scaled &&
    isotropic_error <= tolerances$homogeneous_g_scaled
  default_reference <- spatstat.explore::pcf.ppp(unmarked)
  utils::write.csv(
    data.frame(radius_um = default_reference$r,
               g_spatstat_translation = default_reference$trans,
               g_spatstat_isotropic = default_reference$iso),
    file.path(case_dir, "spatstat-default-grid.csv"), row.names = FALSE
  )
  utils::write.csv(
    data.frame(radius_um = radii, g_marklab_translation = native_translation,
               g_spatstat_translation = reference_translation,
               g_marklab_isotropic = native_isotropic,
               g_spatstat_isotropic = reference_isotropic),
    file.path(case_dir, "curves.csv"), row.names = FALSE
  )
  homogeneous_g_rows[[length(homogeneous_g_rows) + 1L]] <- list(
    case = name, cells = x$n, radii = length(radii), bandwidth_um = bandwidth,
    translation_g_scaled_error = translation_error,
    isotropic_g_scaled_error = isotropic_error,
    spatstat_default_grid_points = length(default_reference$r),
    spatstat_default_bandwidth_um = unname(attr(default_reference, "bw.distance")),
    passed = passed
  )
  cat(name, "homogeneous translation-g error", format(translation_error, digits = 4),
      "isotropic-g error", format(isotropic_error, digits = 4),
      "passed", passed, "\n")

  labels <- as.character(spatstat.geom::marks(x))
  for (levels in list(c("off", "on"), c("on", "off"))) {
    source <- levels[[1]]
    target <- levels[[2]]
    id <- paste(name, source, target, sep = "-")
    direction_dir <- file.path(case_dir, paste(source, target, sep = "-to-"))
    dir.create(direction_dir)
    maximum_directed_pairs <- x$n * (x$n - 1L)
    maximum_null_pairs <- maximum_directed_pairs * settings$simulations
    base <- c("--cells", shQuote(cells_path), "--mask", shQuote(window_path),
              "--source-level", source, "--target-level", target,
              "--radii-um", paste(radii, collapse = ","),
              "--bandwidth-um", bandwidth,
              "--permutations", settings$simulations, "--seed", settings$seed,
              "--alpha", settings$alpha, "--memory-budget-mib", 256L,
              "--max-pair-visits", maximum_directed_pairs,
              "--max-null-pair-evaluations", maximum_null_pairs)
    translation_cross_path <- file.path(direction_dir, "translation.json")
    status <- system2(
      executable,
      c("project", "translation-categorical-cross-pair-correlation", "--project",
        shQuote(file.path(direction_dir, "translation-project")), "--out",
        shQuote(translation_cross_path), base,
        "--max-overlap-evaluations", maximum_directed_pairs,
        "--max-overlap-candidate-work", maximum_directed_pairs * 128,
        "--max-overlap-output-vertices", 10000L),
      stdout = file.path(direction_dir, "translation-command.log"),
      stderr = file.path(direction_dir, "translation-command.log")
    )
    if (status != 0L) stop(id, ": Marklab translation cross-g failed")
    isotropic_cross_path <- file.path(direction_dir, "isotropic.json")
    status <- system2(
      executable,
      c("project", "isotropic-categorical-cross-pair-correlation", "--project",
        shQuote(file.path(direction_dir, "isotropic-project")), "--out",
        shQuote(isotropic_cross_path), base,
        "--max-visible-arc-evaluations", maximum_directed_pairs * 2,
        "--max-arc-segment-tests", maximum_directed_pairs * 256,
        "--max-arc-membership-queries", maximum_directed_pairs * 256),
      stdout = file.path(direction_dir, "isotropic-command.log"),
      stderr = file.path(direction_dir, "isotropic-command.log")
    )
    if (status != 0L) stop(id, ": Marklab isotropic cross-g failed")
    translation_cross <- jsonlite::fromJSON(translation_cross_path, simplifyVector = FALSE)
    isotropic_cross <- jsonlite::fromJSON(isotropic_cross_path, simplifyVector = FALSE)
    native_translation_cross <- vapply(
      translation_cross$curve, function(point) point$cross_g, 0.0
    )
    native_isotropic_cross <- vapply(
      isotropic_cross$curve, function(point) point$cross_g, 0.0
    )
    source_pattern <- x[labels == source]
    target_pattern <- x[labels == target]
    close <- spatstat.geom::crosspairs(
      source_pattern, target_pattern, max(radii) + bandwidth, what = "all"
    )
    kernels <- t(vapply(radii, function(radius) {
      scaled <- (radius - close$d) / bandwidth
      ifelse(abs(scaled) < 1, 0.75 * (1 - scaled^2) / bandwidth, 0)
    }, numeric(length(close$d))))
    translation_weights <- spatstat.explore::edge.Trans(
      dx = close$dx, dy = close$dy, W = window, paired = TRUE
    )
    repeated_sources <- spatstat.geom::ppp(
      close$xi, close$yi, window = window, check = FALSE
    )
    isotropic_weights <- spatstat.explore::edge.Ripley(
      repeated_sources, matrix(close$d, ncol = 1),
      bdistX = spatstat.geom::bdist.points(source_pattern)[close$i]
    )[, 1]
    denominator <- 2 * pi * radii * source_pattern$n * target_pattern$n / area
    direct_translation <- as.vector(kernels %*% translation_weights) / denominator
    direct_isotropic <- as.vector(kernels %*% isotropic_weights) / denominator
    direct_translation_error <- scaled_error(native_translation_cross, direct_translation)
    direct_isotropic_error <- scaled_error(native_isotropic_cross, direct_isotropic)

    # pcfcross delegates to pcfmulti, whose public implementation always takes
    # sewpcf's fast discretised path. Retain sparse, 1,024-subdivision and
    # untouched default output descriptively; exact pair sums are the oracle.
    sparse_package <- spatstat.explore::pcfcross(
      x, source, target, r = reference_r, h = bandwidth,
      kernel = "epanechnikov", correction = c("translate", "Ripley"),
      divisor = "r", zerocor = "none"
    )
    subdivisions <- settings$cross_g_reference_subdivisions
    fine_r <- seq(0, max(radii), length.out = 18L * subdivisions + 1L)
    fine_index <- (3:18) * subdivisions + 1L
    fine_package <- spatstat.explore::pcfcross(
      x, source, target, r = fine_r, h = bandwidth,
      kernel = "epanechnikov", correction = c("translate", "Ripley"),
      divisor = "r", zerocor = "none"
    )
    default_package <- spatstat.explore::pcfcross(x, source, target)
    utils::write.csv(
      data.frame(radius_um = default_package$r,
                 g_spatstat_translation = fv_column(default_package, "trans"),
                 g_spatstat_isotropic = fv_column(default_package, "iso")),
      file.path(direction_dir, "spatstat-default-grid.csv"), row.names = FALSE
    )
    utils::write.csv(
      data.frame(radius_um = radii,
                 g_marklab_translation = native_translation_cross,
                 g_direct_translation = direct_translation,
                 g_spatstat_sparse_translation = sparse_package$trans[reference_index],
                 g_spatstat_fine_translation = fine_package$trans[fine_index],
                 g_marklab_isotropic = native_isotropic_cross,
                 g_direct_isotropic = direct_isotropic,
                 g_spatstat_sparse_isotropic = sparse_package$iso[reference_index],
                 g_spatstat_fine_isotropic = fine_package$iso[fine_index]),
      file.path(direction_dir, "curves.csv"), row.names = FALSE
    )
    analytic_reference_passed <-
      all(is.finite(c(direct_translation_error, direct_isotropic_error))) &&
      direct_translation_error <= tolerances$cross_g_scaled &&
      direct_isotropic_error <= tolerances$cross_g_scaled
    package_errors <- c(
      sparse_translation = scaled_error(
        native_translation_cross, sparse_package$trans[reference_index]
      ),
      sparse_isotropic = scaled_error(
        native_isotropic_cross, sparse_package$iso[reference_index]
      ),
      fine_translation = scaled_error(
        native_translation_cross, fine_package$trans[fine_index]
      ),
      fine_isotropic = scaled_error(
        native_isotropic_cross, fine_package$iso[fine_index]
      )
    )
    package_exact_agreement_passed <- all(is.finite(package_errors)) &&
      all(package_errors <= tolerances$cross_g_scaled)
    cross_g_rows[[length(cross_g_rows) + 1L]] <- list(
      case = id, source = source, target = target,
      source_cells = source_pattern$n, target_cells = target_pattern$n,
      radii = length(radii), bandwidth_um = bandwidth,
      direct_translation_g_scaled_error = direct_translation_error,
      direct_isotropic_g_scaled_error = direct_isotropic_error,
      sparse_pcfcross_translation_g_scaled_difference = package_errors[["sparse_translation"]],
      sparse_pcfcross_isotropic_g_scaled_difference = package_errors[["sparse_isotropic"]],
      fine_pcfcross_translation_g_scaled_difference = package_errors[["fine_translation"]],
      fine_pcfcross_isotropic_g_scaled_difference = package_errors[["fine_isotropic"]],
      sparse_pcfcross_missing_radii = sum(
        !is.finite(sparse_package$trans[reference_index]) |
          !is.finite(sparse_package$iso[reference_index])
      ),
      fine_pcfcross_missing_radii = sum(
        !is.finite(fine_package$trans[fine_index]) |
          !is.finite(fine_package$iso[fine_index])
      ),
      spatstat_default_grid_points = length(default_package$r),
      package_comparison_path = file.path(direction_dir, "curves.csv"),
      analytic_reference_passed = analytic_reference_passed,
      package_exact_agreement_passed = package_exact_agreement_passed,
      passed = analytic_reference_passed
    )
    cat(id, "direct translation cross-g error",
        format(direct_translation_error, digits = 4),
        "direct isotropic cross-g error", format(direct_isotropic_error, digits = 4),
        "analytic reference passed", analytic_reference_passed,
        "package exact agreement passed", package_exact_agreement_passed, "\n")
  }
}

# Robust inhomogeneous cross-g supplies the exact native type-specific point
# intensities to pcfcross.inhom. Its public path has the same discretised smoother
# as pcfcross, so exact pair sums with spatstat edge weights remain the equality
# oracle and package curves remain descriptive.
inhomogeneous_cross_g_rows <- list()
for (name in names(patterns)) {
  x <- patterns[[name]]
  window <- spatstat.geom::Window(x)
  area <- spatstat.geom::area.owin(window)
  short_side <- min(diff(window$xrange), diff(window$yrange))
  pair_bandwidth <- short_side * settings$pair_bandwidth_short_side_fraction
  intensity_bandwidth <- short_side * settings$bandwidth_short_side_fraction
  radii <- short_side * settings$pair_radii_short_side_fractions
  reference_r <- seq(0, max(radii), by = short_side / 80)
  reference_index <- 3:18 + 1L
  source_dir <- file.path(output, paste0(name, "-homogeneous-k-l"))
  cells_path <- file.path(source_dir, "cells.csv")
  window_path <- file.path(source_dir, "window.geojson")
  case_dir <- file.path(output, paste0(name, "-inhomogeneous-cross-g"))
  dir.create(case_dir)
  labels <- as.character(spatstat.geom::marks(x))
  for (levels in list(c("off", "on"), c("on", "off"))) {
    source <- levels[[1]]
    target <- levels[[2]]
    id <- paste(name, source, target, sep = "-")
    direction_dir <- file.path(case_dir, paste(source, target, sep = "-to-"))
    dir.create(direction_dir)
    results <- list()
    for (correction in c("translation", "isotropic")) {
      result_path <- file.path(direction_dir, paste0(correction, ".json"))
      args <- c(
        "project", "inhomogeneous-categorical-cross-pair-correlation",
        "--project", shQuote(file.path(direction_dir, paste0(correction, "-project"))),
        "--cells", shQuote(cells_path), "--mask", shQuote(window_path),
        "--out", shQuote(result_path), "--source-level", source,
        "--target-level", target, "--radii-um", paste(radii, collapse = ","),
        "--intensity-bandwidth-um", intensity_bandwidth,
        "--edge-correction", correction, "--pair-bandwidth-um", pair_bandwidth,
        "--grid-x", settings$grid[[1]], "--grid-y", settings$grid[[2]],
        "--simulations", settings$simulations, "--seed", settings$seed,
        "--alpha", settings$alpha, "--minimum-intensity-per-um2", "1e-12",
        "--memory-budget-mib", 512L, "--max-probes", prod(settings$grid),
        "--max-intensity-evaluations", "1000000000",
        "--max-pair-visits", "100000000", "--max-null-draws", "100000000"
      )
      args <- if (correction == "translation") {
        c(args, "--max-overlap-evaluations", "100000000",
          "--max-overlap-candidate-work", "10000000000",
          "--max-overlap-output-vertices", "10000")
      } else {
        c(args, "--max-visible-arc-evaluations", "200000000",
          "--max-arc-segment-tests", "10000000000",
          "--max-arc-membership-queries", "10000000000")
      }
      status <- system2(
        executable, args, stdout = file.path(direction_dir, paste0(correction, "-command.log")),
        stderr = file.path(direction_dir, paste0(correction, "-command.log"))
      )
      if (status != 0L) stop(id, ": Marklab ", correction, " inhomogeneous cross-g failed")
      results[[correction]] <- read_exact_json(result_path)
    }
    source_pattern <- x[labels == source]
    target_pattern <- x[labels == target]
    source_intensity <- vapply(
      results$translation$source_intensity$point_values,
      function(point) point$intensity_per_um2, 0.0
    )
    target_intensity <- vapply(
      results$translation$target_intensity$point_values,
      function(point) point$intensity_per_um2, 0.0
    )
    if (!identical(
      vapply(results$translation$source_intensity$point_values,
             function(point) as.integer(point$row), 0L),
      seq_len(source_pattern$n) - 1L
    ) || !identical(
      vapply(results$translation$target_intensity$point_values,
             function(point) as.integer(point$row), 0L),
      seq_len(target_pattern$n) - 1L
    )) stop(id, ": native type-specific intensity rows are not canonical")
    isotropic_source_intensity <- vapply(
      results$isotropic$source_intensity$point_values,
      function(point) point$intensity_per_um2, 0.0
    )
    isotropic_target_intensity <- vapply(
      results$isotropic$target_intensity$point_values,
      function(point) point$intensity_per_um2, 0.0
    )
    if (!identical(source_intensity, isotropic_source_intensity) ||
        !identical(target_intensity, isotropic_target_intensity)) {
      stop(id, ": edge corrections received different native intensity values")
    }
    close <- spatstat.geom::crosspairs(
      source_pattern, target_pattern, max(radii) + pair_bandwidth, what = "all"
    )
    kernels <- t(vapply(radii, function(radius) {
      scaled <- (radius - close$d) / pair_bandwidth
      ifelse(abs(scaled) < 1, 0.75 * (1 - scaled^2) / pair_bandwidth, 0)
    }, numeric(length(close$d))))
    inverse_intensity <- 1 / (
      source_intensity[close$i] * target_intensity[close$j]
    )
    translation_weights <- spatstat.explore::edge.Trans(
      dx = close$dx, dy = close$dy, W = window, paired = TRUE
    )
    repeated_sources <- spatstat.geom::ppp(
      close$xi, close$yi, window = window, check = FALSE
    )
    isotropic_weights <- spatstat.explore::edge.Ripley(
      repeated_sources, matrix(close$d, ncol = 1),
      bdistX = spatstat.geom::bdist.points(source_pattern)[close$i]
    )[, 1]
    direct_translation <- as.vector(
      kernels %*% (translation_weights * inverse_intensity)
    ) / (2 * pi * radii * area)
    direct_isotropic <- as.vector(
      kernels %*% (isotropic_weights * inverse_intensity)
    ) / (2 * pi * radii * area)
    native_translation <- vapply(
      results$translation$curve, function(point) point$cross_g, 0.0
    )
    native_isotropic <- vapply(
      results$isotropic$curve, function(point) point$cross_g, 0.0
    )
    direct_translation_error <- scaled_error(native_translation, direct_translation)
    direct_isotropic_error <- scaled_error(native_isotropic, direct_isotropic)
    sparse_package <- spatstat.explore::pcfcross.inhom(
      x, source, target, lambdaI = source_intensity, lambdaJ = target_intensity,
      r = reference_r, h = pair_bandwidth, kernel = "epanechnikov",
      correction = c("translate", "Ripley"), divisor = "r", zerocor = "none"
    )
    subdivisions <- settings$cross_g_reference_subdivisions
    fine_r <- seq(0, max(radii), length.out = 18L * subdivisions + 1L)
    fine_index <- (3:18) * subdivisions + 1L
    fine_package <- spatstat.explore::pcfcross.inhom(
      x, source, target, lambdaI = source_intensity, lambdaJ = target_intensity,
      r = fine_r, h = pair_bandwidth, kernel = "epanechnikov",
      correction = c("translate", "Ripley"), divisor = "r", zerocor = "none"
    )
    default_package <- spatstat.explore::pcfcross.inhom(
      x, source, target, lambdaI = source_intensity, lambdaJ = target_intensity
    )
    utils::write.csv(
      data.frame(radius_um = default_package$r,
                 g_spatstat_translation = fv_column(default_package, "trans"),
                 g_spatstat_isotropic = fv_column(default_package, "iso")),
      file.path(direction_dir, "spatstat-default-grid.csv"), row.names = FALSE
    )
    utils::write.csv(
      data.frame(radius_um = radii,
                 g_marklab_translation = native_translation,
                 g_direct_translation = direct_translation,
                 g_spatstat_sparse_translation = sparse_package$trans[reference_index],
                 g_spatstat_fine_translation = fine_package$trans[fine_index],
                 g_marklab_isotropic = native_isotropic,
                 g_direct_isotropic = direct_isotropic,
                 g_spatstat_sparse_isotropic = sparse_package$iso[reference_index],
                 g_spatstat_fine_isotropic = fine_package$iso[fine_index]),
      file.path(direction_dir, "curves.csv"), row.names = FALSE
    )
    analytic_reference_passed <-
      all(is.finite(c(direct_translation_error, direct_isotropic_error))) &&
      direct_translation_error <= tolerances$inhomogeneous_cross_g_scaled &&
      direct_isotropic_error <= tolerances$inhomogeneous_cross_g_scaled
    package_errors <- c(
      sparse_translation = scaled_error(
        native_translation, sparse_package$trans[reference_index]
      ),
      sparse_isotropic = scaled_error(
        native_isotropic, sparse_package$iso[reference_index]
      ),
      fine_translation = scaled_error(
        native_translation, fine_package$trans[fine_index]
      ),
      fine_isotropic = scaled_error(
        native_isotropic, fine_package$iso[fine_index]
      )
    )
    package_exact_agreement_passed <- all(is.finite(package_errors)) &&
      all(package_errors <= tolerances$inhomogeneous_cross_g_scaled)
    inhomogeneous_cross_g_rows[[length(inhomogeneous_cross_g_rows) + 1L]] <- list(
      case = id, source = source, target = target,
      source_cells = source_pattern$n, target_cells = target_pattern$n,
      radii = length(radii), pair_bandwidth_um = pair_bandwidth,
      intensity_bandwidth_um = intensity_bandwidth,
      direct_translation_g_scaled_error = direct_translation_error,
      direct_isotropic_g_scaled_error = direct_isotropic_error,
      sparse_pcfcross_inhom_translation_g_scaled_difference = package_errors[["sparse_translation"]],
      sparse_pcfcross_inhom_isotropic_g_scaled_difference = package_errors[["sparse_isotropic"]],
      fine_pcfcross_inhom_translation_g_scaled_difference = package_errors[["fine_translation"]],
      fine_pcfcross_inhom_isotropic_g_scaled_difference = package_errors[["fine_isotropic"]],
      sparse_pcfcross_inhom_missing_radii = sum(
        !is.finite(sparse_package$trans[reference_index]) |
          !is.finite(sparse_package$iso[reference_index])
      ),
      fine_pcfcross_inhom_missing_radii = sum(
        !is.finite(fine_package$trans[fine_index]) |
          !is.finite(fine_package$iso[fine_index])
      ),
      spatstat_default_grid_points = length(default_package$r),
      package_comparison_path = file.path(direction_dir, "curves.csv"),
      analytic_reference_passed = analytic_reference_passed,
      package_exact_agreement_passed = package_exact_agreement_passed,
      passed = analytic_reference_passed
    )
    cat(id, "direct inhomogeneous translation cross-g error",
        format(direct_translation_error, digits = 4),
        "direct isotropic cross-g error", format(direct_isotropic_error, digits = 4),
        "analytic reference passed", analytic_reference_passed,
        "package exact agreement passed", package_exact_agreement_passed, "\n")
  }
}

# F uses the declared 64 x 64 cell-centred probe grid. A spatstat mask with the
# same dimensions has exactly those pixel centres, so Fest's reduced-sample
# estimate is an equality oracle on these rectangular public windows. Gest is
# evaluated against the original polygon so event eligibility uses the exact
# boundary. J is the documented survival ratio of those two matched components.
nearest_space_rows <- list()
for (name in names(patterns)) {
  x <- patterns[[name]]
  unmarked <- spatstat.geom::unmark(x)
  window <- spatstat.geom::Window(unmarked)
  short_side <- min(diff(window$xrange), diff(window$yrange))
  radii <- short_side * settings$radii_short_side_fractions
  case_dir <- file.path(output, paste0(name, "-nearest-space"))
  dir.create(case_dir)
  source_dir <- file.path(output, paste0(name, "-homogeneous-k-l"))
  result_dir <- file.path(case_dir, "marklab")
  maximum_queries <- (x$n + prod(settings$grid)) * (settings$simulations + 1L)
  maximum_draws <- x$n * settings$simulations * 2L
  status <- system2(
    executable,
    c("project", "nearest-space", "--project", shQuote(file.path(case_dir, "project")),
      "--cells", shQuote(file.path(source_dir, "cells.csv")),
      "--mask", shQuote(file.path(source_dir, "window.geojson")),
      "--out", shQuote(result_dir), "--r-max-um", max(radii),
      "--r-steps", length(radii), "--probe-grid-x", settings$grid[[1]],
      "--probe-grid-y", settings$grid[[2]],
      "--simulations", settings$simulations, "--seed", settings$seed,
      "--alpha", settings$alpha,
      "--j-denominator-epsilon", settings$nearest_space_j_denominator_epsilon,
      "--memory-budget-mib", 256L, "--max-nearest-queries", maximum_queries,
      "--max-csr-draws", maximum_draws),
    stdout = file.path(case_dir, "command.log"),
    stderr = file.path(case_dir, "command.log")
  )
  if (status != 0L) stop(name, ": Marklab nearest-space failed")
  native <- jsonlite::fromJSON(
    file.path(result_dir, "result.json"), simplifyVector = FALSE
  )$analysis
  if (!identical(as.integer(unlist(native$probes$requested_grid)), settings$grid)) {
    stop(name, ": native nearest-space probe grid is not the declared grid")
  }
  native_f <- vapply(native$curve, optional_curve_value, 0.0, name = "f")
  native_g <- vapply(native$curve, optional_curve_value, 0.0, name = "g")
  native_j <- vapply(native$curve, optional_curve_value, 0.0, name = "j")

  subdivisions <- settings$nearest_space_reference_subdivisions
  reference_r <- seq(
    0, max(radii), length.out = length(radii) * subdivisions + 1L
  )
  reference_index <- seq_along(radii) * subdivisions + 1L
  if (max(abs(reference_r[reference_index] - radii)) >
      .Machine$double.eps * max(1, max(radii)) * 4) {
    stop(name, ": requested radii are not represented by the F/G reference grid")
  }
  masked <- unmarked
  reference_mask <- spatstat.geom::as.mask(
    window, dimyx = rev(settings$grid)
  )
  spatstat.geom::Window(masked) <- reference_mask
  if (abs(reference_mask$xstep - native$probes$spacing_um[[1]]) >
        .Machine$double.eps * max(1, reference_mask$xstep) * 4 ||
      abs(reference_mask$ystep - native$probes$spacing_um[[2]]) >
        .Machine$double.eps * max(1, reference_mask$ystep) * 4) {
    stop(name, ": Fest mask spacing does not match the native probe grid")
  }
  f_reference <- spatstat.explore::Fest(
    masked, r = reference_r, correction = "rs"
  )$rs[reference_index]
  g_reference <- spatstat.explore::Gest(
    unmarked, r = reference_r, correction = "rs"
  )$rs[reference_index]
  j_reference <- ifelse(
    1 - f_reference > settings$nearest_space_j_denominator_epsilon,
    (1 - g_reference) / (1 - f_reference), NA_real_
  )
  f_error <- scaled_error(native_f, f_reference)
  g_error <- scaled_error(native_g, g_reference)
  j_error <- optional_scaled_error(native_j, j_reference)
  passed <- all(is.finite(c(f_error, g_error, j_error))) &&
    f_error <= tolerances$nearest_space_scaled &&
    g_error <= tolerances$nearest_space_scaled &&
    j_error <= tolerances$nearest_space_scaled
  utils::write.csv(
    data.frame(radius_um = radii, f_marklab = native_f, f_Fest_rs = f_reference,
               g_marklab = native_g, g_Gest_rs = g_reference,
               j_marklab = native_j, j_survival_ratio = j_reference),
    file.path(case_dir, "curves.csv"), row.names = FALSE
  )
  nearest_space_rows[[length(nearest_space_rows) + 1L]] <- list(
    case = name, cells = x$n, radii = length(radii),
    requested_probe_grid = as.list(settings$grid),
    retained_probes = native$probes$retained_probe_count,
    reference_grid_points = length(reference_r),
    f_Fest_reduced_sample_scaled_error = f_error,
    g_Gest_reduced_sample_scaled_error = g_error,
    j_matched_survival_ratio_scaled_error = j_error,
    j_finite_radius_count = sum(is.finite(j_reference)),
    passed = passed
  )
  cat(name, "nearest-space F error", format(f_error, digits = 4),
      "G error", format(g_error, digits = 4),
      "J error", format(j_error, digits = 4), "passed", passed, "\n")
}
packages <- package_pins$package
analytic_reference_passed <-
  all(vapply(cross_g_rows, function(row) row$analytic_reference_passed, FALSE)) &&
  all(vapply(inhomogeneous_cross_g_rows,
             function(row) row$analytic_reference_passed, FALSE))
package_exact_agreement_passed <-
  all(vapply(cross_g_rows,
             function(row) row$package_exact_agreement_passed, FALSE)) &&
  all(vapply(inhomogeneous_cross_g_rows,
             function(row) row$package_exact_agreement_passed, FALSE))
package_exact_agreement_failed_outputs <- c(
  vapply(cross_g_rows, function(row) row$package_comparison_path, ""),
  vapply(inhomogeneous_cross_g_rows, function(row) row$package_comparison_path, "")
)[!c(
  vapply(cross_g_rows, function(row) row$package_exact_agreement_passed, FALSE),
  vapply(inhomogeneous_cross_g_rows,
         function(row) row$package_exact_agreement_passed, FALSE)
)]
summary <- list(
  scope = "Numerical validation on two published animal retinal recordings; not patient-group or clinical validation",
  references = list(amacrine = "Diggle (1986), J Neurosci Methods 18:115-125; 662 microns per stored unit",
                    betacells = "Waessle, Boycott and Illing (1981), Proc R Soc Lond B 212:177-195; stored microns"),
  package_lock = pin_path, r_version = R.version.string,
  packages = setNames(lapply(packages, function(p) {
    unname(utils::packageDescription(p, fields = "Version"))
  }), packages),
  settings = settings, tolerances = tolerances, cases = rows,
  homogeneous_k_l_cases = homogeneous_rows, cross_k_l_cases = cross_rows,
  homogeneous_g_cases = homogeneous_g_rows, cross_g_cases = cross_g_rows,
  inhomogeneous_cross_g_cases = inhomogeneous_cross_g_rows,
  nearest_space_cases = nearest_space_rows,
  analytic_reference_passed = analytic_reference_passed,
  package_exact_agreement_passed = package_exact_agreement_passed,
  package_exact_agreement_failed_outputs = as.list(package_exact_agreement_failed_outputs),
  get_checked_fixture = list(errors = as.list(get_errors), passed = all(get_errors <= tolerances$get_absolute)),
  synthetic_null_curve_case = synthetic_check,
  limitations = c("The spatstat default-pipeline difference is descriptive and has no agreement threshold",
                  "GET agreement on exported within-slide curves covers the plug-in fixed-intensity L-r null of these six cases, whose observed curves are all most extreme (p = 1/40), and one synthetic inhomogeneous Poisson case with a non-extreme p-value; refit, g, cross-g and compartment nulls export curves but are not compared here",
                  "The tied-curve GET oracle is checked separately from the real-data runs",
                  "No native spatstat estimator matches Marklab's radius-plus-bandwidth standard-border g or cross-g estimator",
                  "pcfcross and pcfcross.inhom use a fast discretised smoother; their sparse and fine-grid curves fail the fixed exact-agreement criterion and are reported separately, while defaults remain descriptive and exact pair sums are the analytic cross-g oracle",
                  "Cross-K/L agreement covers homogeneous off/on retinal types with standard border correction, not an inhomogeneous cross-K/L estimator",
                  "Nearest-space F/J agreement uses the declared 64 x 64 fixed cell-centred probe grid, not spatstat's default raster or a continuous empty-space integral",
                  "No patient-group size/power, clinical validity, diagnostic performance, or fitted-null calibration claim"),
  passed = all(vapply(rows, function(row) row$passed, FALSE)) &&
    all(vapply(homogeneous_rows, function(row) row$passed, FALSE)) &&
    all(vapply(cross_rows, function(row) row$passed, FALSE)) &&
    all(vapply(homogeneous_g_rows, function(row) row$passed, FALSE)) &&
    analytic_reference_passed &&
    all(vapply(nearest_space_rows, function(row) row$passed, FALSE)) &&
    all(get_errors <= tolerances$get_absolute) &&
    synthetic_check$passed
)
write_json(summary, file.path(output, "summary.json"))
if (!summary$passed) stop("reference comparison failed; see summary.json")
