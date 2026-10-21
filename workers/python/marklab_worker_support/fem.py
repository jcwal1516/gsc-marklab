"""Piecewise-linear finite elements for the alpha = 2 SPDE and one-factor SPDE fits.

Shared by the advanced Bayesian worker and the multimodal validation worker. ``assemble_fem``
builds the P1 mass and stiffness matrices, ``stationary_precision`` the Lindgren, Rue and
Lindstrom (2011) GMRF precision of a Matern field with smoothness 1 in two dimensions,
``build_mesh`` a regular rectangular Delaunay mesh, and ``projection`` the barycentric matrix
from mesh vertices to locations. ``fit_spatial_factor`` is the MAP fit of the one-factor model
in ``factor_objective``; ``fit_factor_map`` fits the same objective for any design matrix and
precision.
"""

import numpy as np
from scipy.optimize import minimize
from scipy.spatial import Delaunay

from .protocol import ContractError

LINEAR_ELEMENT_MASS = np.asarray([[2, 1, 1], [1, 2, 1], [1, 1, 2]], float)
OPTIMIZER_OPTIONS = {"ftol": 1e-12, "gtol": 1e-7}
GRADIENT_TOLERANCE = 2e-4


def triangle_double_area(coordinates):
    """Twice the signed area of a triangle (positive for counter-clockwise vertices)."""
    first = coordinates[1] - coordinates[0]
    second = coordinates[2] - coordinates[0]
    return float(first[0] * second[1] - first[1] * second[0])


def basis_gradients(coordinates, double_area):
    """x- and y-components of the gradients of a triangle's three linear basis functions."""
    (x0, y0), (x1, y1), (x2, y2) = coordinates
    gradient_x = np.asarray([y1 - y2, y2 - y0, y0 - y1]) / double_area
    gradient_y = np.asarray([x2 - x1, x0 - x2, x1 - x0]) / double_area
    return gradient_x, gradient_y


def add_element(matrix, triangle, local):
    """Add a 3x3 element matrix into the global matrix at the triangle's vertex indices."""
    matrix[np.ix_(triangle, triangle)] += local


def assemble_fem(vertices, triangles, minimum_area, label):
    """Assemble the P1 mass matrix ``C``, isotropic stiffness matrix ``G``, and triangle areas.

    Areas are returned as a list of Python floats; ``sum`` over it is compensated (Python 3.12).
    """
    mass = np.zeros((len(vertices), len(vertices)))
    stiffness = np.zeros_like(mass)
    areas = []
    for triangle in triangles:
        coordinates = vertices[triangle]
        double_area = triangle_double_area(coordinates)
        area = abs(double_area) / 2.0
        if area <= minimum_area:
            raise ContractError(f"{label} produced a degenerate triangle")
        areas.append(area)
        gradient_x, gradient_y = basis_gradients(coordinates, double_area)
        add_element(mass, triangle, area / 12.0 * LINEAR_ELEMENT_MASS)
        local_stiffness = area * (
            np.outer(gradient_x, gradient_x) + np.outer(gradient_y, gradient_y)
        )
        add_element(stiffness, triangle, local_stiffness)
    return mass, stiffness, areas


def stationary_precision(mass, stiffness, lumped, kappa, tau):
    """Stationary alpha = 2 SPDE (Matern, nu = 1) GMRF precision with a lumped mass matrix.

    ``Q = tau^2 (kappa^4 C + 2 kappa^2 G + G C~^-1 G)`` where ``C~`` is the row-sum lumped mass.
    """
    lumped_inverse = np.diag(1.0 / lumped)
    precision = tau**2 * (
        kappa**4 * mass + 2.0 * kappa**2 * stiffness + stiffness @ lumped_inverse @ stiffness
    )
    return (precision + precision.T) / 2.0


def sparse_triplets(matrix, tolerance=1e-14):
    rows, columns = np.nonzero(np.abs(matrix) > tolerance)
    return {
        "rows": rows.tolist(),
        "columns": columns.tolist(),
        "values": matrix[rows, columns].tolist(),
        "shape": list(matrix.shape),
    }


def initial_factor(design, centered, precision, noise_sd):
    """Ridge-smoothed field for the first feature and the matching least-squares loadings."""
    system = design.T @ design + noise_sd**2 * precision + np.eye(len(precision)) * 1e-8
    field = np.linalg.solve(system, design.T @ centered[:, 0])
    loadings = np.linalg.lstsq((design @ field)[:, None], centered, rcond=None)[0].ravel()
    return field, loadings


def factor_objective(field, loadings, design, centered, precision, noise_sd):
    """Negative log posterior (up to a constant) of the one-factor SPDE model and its gradient.

    Model: ``centered_ij = (A w)_i * l_j + e_ij`` with ``e_ij ~ Normal(0, noise_sd)``, mesh
    weights ``w ~ Normal(0, Q^-1)`` (SPDE GMRF), and loadings ``l_j ~ Normal(0, 1)``; ``A`` is
    the barycentric projection from mesh vertices to observation locations.
    """
    projected = design @ field
    residual = centered - projected[:, None] * loadings[None, :]
    value = (
        0.5 * np.sum(residual**2) / noise_sd**2
        + 0.5 * field @ precision @ field
        + 0.5 * loadings @ loadings
    )
    gradient_field = precision @ field - design.T @ (residual @ loadings) / noise_sd**2
    gradient_loadings = loadings - projected @ residual / noise_sd**2
    return float(value), np.concatenate([gradient_field, gradient_loadings])


def sign_aligned_factor(field, loadings, projected):
    """Resolve the factor sign so that the first loading is non-negative."""
    if loadings[0] < 0:
        return -field, -loadings, -projected
    return field, loadings, projected


def build_mesh(bounds, resolution, kappa, tau):
    """Regular Delaunay mesh of a rectangle with the stationary SPDE precision."""
    x0, x1, y0, y1 = map(float, bounds)
    xs = np.linspace(x0, x1, resolution)
    ys = np.linspace(y0, y1, resolution)
    vertices = np.asarray([[x, y] for y in ys for x in xs], dtype=float)
    triangulation = Delaunay(vertices)
    triangles = np.asarray(
        sorted(tuple(sorted(map(int, simplex))) for simplex in triangulation.simplices),
        dtype=int,
    )
    mass, stiffness, areas = assemble_fem(vertices, triangles, 0.0, "SPDE mesh")
    precision = stationary_precision(mass, stiffness, mass.sum(axis=1), kappa, tau)
    return {
        "vertices": vertices,
        "triangles": triangles,
        "triangulation": triangulation,
        "mass": mass,
        "stiffness": stiffness,
        "precision": precision,
        "triangle_areas": np.asarray(areas),
    }


def projection(mesh, locations):
    """Barycentric projection matrix from mesh vertices to ``locations``."""
    locations = np.asarray(locations, float)
    triangulation = mesh["triangulation"]
    simplices = triangulation.find_simplex(locations)
    if np.any(simplices < 0):
        raise ContractError("SPDE projection location lies outside the rectangular mesh")
    matrix = np.zeros((len(locations), len(mesh["vertices"])))
    for row, (point, simplex_index) in enumerate(zip(locations, simplices, strict=True)):
        transform = triangulation.transform[simplex_index]
        first = transform[:2] @ (point - transform[2])
        barycentric = np.asarray([first[0], first[1], 1.0 - first.sum()])
        if barycentric.min() < -1e-10:
            raise ContractError("invalid SPDE barycentric projection")
        matrix[row, triangulation.simplices[simplex_index]] = barycentric
    return matrix


def fit_factor_map(design, centered, precision, noise_sd, maximum_iterations, label):
    """L-BFGS-B MAP fit of ``factor_objective`` for any design matrix and prior precision.

    Starts from ``initial_factor`` and accepts a stop whose gradient is within
    ``GRADIENT_TOLERANCE``. Returns the sign-aligned field, loadings and projected factor and the
    SciPy result.
    """
    initial_field, initial_loadings = initial_factor(design, centered, precision, noise_sd)
    vertex_count = len(initial_field)

    def objective(parameter):
        field = parameter[:vertex_count]
        loadings = parameter[vertex_count:]
        return factor_objective(field, loadings, design, centered, precision, noise_sd)

    fit = minimize(
        objective,
        np.concatenate([initial_field, initial_loadings]),
        jac=True,
        method="L-BFGS-B",
        options={"maxiter": maximum_iterations, **OPTIMIZER_OPTIONS},
    )
    if not fit.success and np.linalg.norm(fit.jac, ord=np.inf) > GRADIENT_TOLERANCE:
        raise ContractError(f"{label} optimization failed: {fit.message}")
    field = fit.x[:vertex_count]
    loadings = fit.x[vertex_count:]
    field, loadings, projected = sign_aligned_factor(field, loadings, design @ field)
    return field, loadings, projected, fit


def fit_spatial_factor(mesh, observations, noise_sd, maximum_iterations):
    """L-BFGS-B MAP fit of the one-factor SPDE model (see ``factor_objective``)."""
    coordinates = np.asarray([row["coordinates"] for row in observations], float)
    values = np.asarray([row["values"] for row in observations], float)
    if values.ndim != 2 or values.shape[1] < 2 or not np.isfinite(values).all():
        raise ContractError("SPDE factor model requires a finite matrix with at least two features")
    design = projection(mesh, coordinates)
    means = values.mean(axis=0)
    centered = values - means
    field, loadings, projected, fit = fit_factor_map(
        design, centered, mesh["precision"], noise_sd, maximum_iterations, "SPDE factor"
    )
    reconstruction = means + projected[:, None] * loadings[None, :]
    return {
        "means": means,
        "field": field,
        "projected": projected,
        "loadings": loadings,
        "rmse": float(np.sqrt(np.mean((reconstruction - values) ** 2))),
        "x_correlation": float(np.corrcoef(projected, coordinates[:, 0])[0, 1]),
        "objective": float(fit.fun),
        "iterations": int(fit.nit),
        "projection": design,
    }
