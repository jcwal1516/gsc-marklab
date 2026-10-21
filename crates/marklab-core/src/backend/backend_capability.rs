//! The [`BackendCapability`] vocabulary.

crate::vocabulary_enum! {
    /// The inference capability a backend provides for a model.
    pub enum BackendCapability {
        AnnealedSmc => "annealed_smc",
        BoundedNonlinearLeastSquares => "bounded_nonlinear_least_squares",
        BoundedWeightedPoissonOptimization => "bounded_weighted_poisson_optimization",
        InlaStyleNestedLaplaceWithNutsComparison => "inla_style_nested_laplace_with_nuts_comparison",
        LaplaceApproximation => "laplace_approximation",
        Nuts => "nuts",
        NutsExactDenseAnisotropicGp3d => "nuts_exact_dense_anisotropic_gp3d",
        NutsExactDenseGp => "nuts_exact_dense_gp",
        NutsExactDenseMultiOutputGp => "nuts_exact_dense_multi_output_gp",
        PriorSensitivity => "prior_sensitivity",
        SimulationBasedCalibration => "simulation_based_calibration",
    }
}
