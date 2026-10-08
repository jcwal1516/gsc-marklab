use crate::config::NeighborhoodNullModel;

pub(super) fn marked_acceptance_criterion(generator: &str) -> &'static str {
    match generator {
        "random_labeling" => {
            "smoke only: type-I rate must remain below the replicate-count-dependent guard"
        }
        "single_gaussian_cluster" | "single_matern_cluster" => {
            "smoke only: mean production residual-territory count >= 1"
        }
        "many_small_foci" => {
            "smoke only: mean production residual-territory count >= 4 with finite low-k excess"
        }
        "anisotropic_stripe" => "smoke only: mean production anisotropy index > 1.05",
        "low_k_suppressed_dispersed" => "smoke only: mean production low-k excess <= 1.25",
        "cell_density_gradient_random_labels" => {
            "smoke only: mean production residual-territory count <= 1"
        }
        "stain_gradient_artifact" => {
            "smoke only: every replicate is suppressed and carries the production stain-gradient flag"
        }
        "internal_control_dropout_artifact" => {
            "smoke only: production status includes internal-control failure overlap"
        }
        "fragmented_tumor_islands" => {
            "smoke only: production status includes mask-fragmentation suspect"
        }
        "rare_phenotype" => {
            "smoke only: production status includes too-few-marked underpowering"
        }
        "prepost_metadata_mismatch" => {
            "smoke only: every production pre/post comparison reports anatomical incomparability"
        }
        _ => "unknown smoke acceptance criterion",
    }
}

pub(super) fn note_for(generator: &str) -> &'static str {
    match generator {
        "random_labeling" => {
            "fixed-position random labeling should keep spectra near the permutation baseline"
        }
        "single_gaussian_cluster" => {
            "clustered labels should produce residual territories at interpretable scales"
        }
        "single_matern_cluster" => {
            "cluster-process-like labels should produce residual territories at interpretable scales"
        }
        "many_small_foci" => "many small foci should increase local-difference or residual scale energy",
        "anisotropic_stripe" => "stripe labels should elevate the anisotropy index",
        "low_k_suppressed_dispersed" => "regularly dispersed labels should suppress low-k power",
        "cell_density_gradient_random_labels" => {
            "random labels on a spatial field should not produce territory inflation"
        }
        "stain_gradient_artifact" => "stain gradients should suppress biologic interpretation",
        "internal_control_dropout_artifact" => {
            "internal-control dropout is represented as a severe IHC-validity artifact"
        }
        "fragmented_tumor_islands" => {
            "fragmented component layouts should trigger a mask/window flag"
        }
        "rare_phenotype" => "rare phenotypes should be labeled low-power/unstable",
        "prepost_metadata_mismatch" => {
            "mismatched pre/post identifiers must be reported as not anatomically comparable"
        }
        _ => "synthetic generator smoke check generator",
    }
}

pub(super) fn small_sample_type_i_limit(replicates: usize) -> f64 {
    if replicates < 20 {
        0.25
    } else if replicates < 100 {
        0.20
    } else {
        0.15
    }
}

pub(super) const fn multimodal_null_model_name(null_model: NeighborhoodNullModel) -> &'static str {
    match null_model {
        NeighborhoodNullModel::SourceSection => "source_section",
        NeighborhoodNullModel::SourceSectionDensity => "source_section_density",
        NeighborhoodNullModel::SourceSectionCellClass => "source_section_cell_class",
        NeighborhoodNullModel::SourceSectionRegistrationQc => "source_section_registration_qc",
    }
}

pub(super) fn multimodal_acceptance_criterion(generator: &str) -> &'static str {
    match generator {
        "random_labels_no_association" => {
            "smoke only: production enrichment does not detect an association in at least 90% of replicates"
        }
        "two_unrelated_mmr_territories" => {
            "smoke only: production keeps unrelated territories separate in at least 80% of replicates"
        }
        "two_related_mmr_territories" => {
            "smoke only: production merges the related territory in at least 80% of replicates"
        }
        "immune_associated_mmr_territory" => {
            "smoke only: production q-value <= 0.05 in at least 80% of replicates"
        }
        "immune_independent_mmr_territory" => {
            "smoke only: production enrichment does not detect independent immune cells in at least 80% of replicates"
        }
        "registration_jitter_no_association" => {
            "smoke only: noisy registration without association remains negative in at least 80% of replicates"
        }
        "cross_interaction_enrichment" => {
            "smoke only: production cross-interaction global p-value detects enrichment in at least 80% of replicates"
        }
        "registration_jitter" => {
            "smoke only: production flags the association below registration resolution in at least 80% of replicates"
        }
        "prepost_within_margin_spatial_pattern" => {
            "smoke only: production keeps matched pre/post curves within the descriptive margin in at least 80% of replicates"
        }
        "prepost_changed_spatial_pattern" => {
            "smoke only: production puts changed pre/post curves outside the descriptive margin in at least 80% of replicates"
        }
        "registration_residual_above_threshold" => {
            "smoke only: production engine rejects registration RMSE above the configured threshold"
        }
        "too_few_landmarks" => {
            "smoke only: production engine rejects fewer than the configured landmark minimum"
        }
        "degenerate_landmarks" => {
            "smoke only: production registration rejects degenerate landmark geometry"
        }
        "empty_he_cells" => {
            "smoke only: production result truthfully reports an empty H&E section"
        }
        "empty_ihc_cells" => {
            "smoke only: production result truthfully reports an empty IHC section"
        }
        "no_abnormal_cells" => {
            "smoke only: production territory result is an available empty set when no abnormal cells exist"
        }
        "sparse_graph" => "smoke only: production graph is empty under a sub-spacing radius",
        "zero_expected_edge_count" => {
            "smoke only: production enrichment types zero expectation instead of emitting a non-finite ratio"
        }
        "multiple_cell_classes" => {
            "smoke only: production result contains every configured cell-class enrichment pair"
        }
        "multiple_null_models" => {
            "smoke only: production run contains every configured null-model sensitivity"
        }
        "rigid_rotation" => {
            "smoke only: production rigid fit recovers the known rotation and translation"
        }
        "affine_deformation" => {
            "smoke only: production affine fit recovers the known deformation"
        }
        _ => "unknown smoke acceptance criterion",
    }
}

pub(super) fn multimodal_min_criterion_rate(generator: &str) -> f64 {
    match generator {
        "random_labels_no_association" => 0.90,
        _ => 0.80,
    }
}

pub(super) fn multimodal_note_for(generator: &str) -> &'static str {
    match generator {
        "random_labels_no_association" => {
            "random H&E labels without designed association are a negative enrichment control"
        }
        "two_unrelated_mmr_territories" => {
            "spatially separated MMR territories should not be called related"
        }
        "two_related_mmr_territories" => {
            "nearby MMR territories with bridge support should be detected as related"
        }
        "immune_associated_mmr_territory" => {
            "MMR territory with local lymphocyte enrichment should be detected"
        }
        "immune_independent_mmr_territory" => {
            "immune cells spatially independent of the MMR territory should remain negative"
        }
        "registration_jitter_no_association" => {
            "registration noise must not manufacture an absent immune/MMR association"
        }
        "cross_interaction_enrichment" => {
            "a known local immune/MMR association should alter the production cross-interaction curve"
        }
        "registration_jitter" => {
            "serial-section associations below registration resolution should be flagged"
        }
        "prepost_within_margin_spatial_pattern" => {
            "matched pre/post curves should remain within the configured descriptive margin"
        }
        "prepost_changed_spatial_pattern" => {
            "pre/post curves beyond the difference threshold should be detected as changed"
        }
        "registration_residual_above_threshold" => {
            "registration residuals above the configured maximum must fail production analysis"
        }
        "too_few_landmarks" => "too few landmarks must fail production input validation",
        "degenerate_landmarks" => "degenerate landmarks must fail production registration",
        "empty_he_cells" => "an empty H&E section must remain explicit in production results",
        "empty_ihc_cells" => "an empty IHC section must remain explicit in production results",
        "no_abnormal_cells" => {
            "absence of abnormal IHC cells should produce no neighborhood territories"
        }
        "sparse_graph" => "a radius below all cell spacings should produce no graph edges",
        "zero_expected_edge_count" => {
            "zero expected edges must produce a typed unavailable enrichment ratio"
        }
        "multiple_cell_classes" => {
            "every configured cell-class pair should appear in production enrichment"
        }
        "multiple_null_models" => {
            "every configured null model should appear in production sensitivity results"
        }
        "rigid_rotation" => "the rigid production path should recover a known 90-degree rotation",
        "affine_deformation" => {
            "the affine production path should recover a known shear and anisotropic scale"
        }
        _ => "multimodal synthetic generator smoke check generator",
    }
}
