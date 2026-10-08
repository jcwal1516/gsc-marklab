use crate::{
    common::seeds::splitmix64,
    config::{NeighborhoodNullModel, RegistrationTransform},
    errors::{MarklabError, Result},
    multimodal::{HeCell, IhcCell, MultimodalInput},
    permutation::labels::permute_fixed_count,
    registration::landmarks::LandmarkPair,
    AnalysisConfig,
};

#[derive(Clone, Debug)]
pub(in crate::synthetic_smoke) struct MultimodalScenario {
    pub(in crate::synthetic_smoke) config: AnalysisConfig,
    pub(in crate::synthetic_smoke) pre: MultimodalInput,
    pub(in crate::synthetic_smoke) post: Option<MultimodalInput>,
}

pub(in crate::synthetic_smoke) fn multimodal_replicate_scenario(
    generator: &str,
    seed: u64,
    generator_index: u64,
    replicate: usize,
) -> Result<MultimodalScenario> {
    let scenario_seed = seed
        ^ (generator_index.wrapping_mul(0x9e37_79b9_7f4a_7c15))
        ^ ((replicate as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9));
    let mut rng = DeterministicRng::new(scenario_seed);
    let mut config = multimodal_smoke_config(seed);
    config.permutation.seed = splitmix64(scenario_seed ^ 0x6e75_6c6c_5f73_6565);
    let (pre, post) = match generator {
        "random_labels_no_association" => {
            (random_label_input("pre", splitmix64(scenario_seed))?, None)
        }
        "two_unrelated_mmr_territories" => {
            config.neighborhood.label_pairs = vec![["mmr_abnormal".into(), "mmr_abnormal".into()]];
            (territory_relation_input(false, "pre", &mut rng), None)
        }
        "two_related_mmr_territories" => {
            config.neighborhood.label_pairs = vec![["mmr_abnormal".into(), "mmr_abnormal".into()]];
            (territory_relation_input(true, "pre", &mut rng), None)
        }
        "immune_associated_mmr_territory" => {
            (immune_association_input(true, false, "pre", &mut rng), None)
        }
        "immune_independent_mmr_territory" => (
            immune_association_input(false, false, "pre", &mut rng),
            None,
        ),
        "registration_jitter_no_association" => {
            (immune_association_input(false, true, "pre", &mut rng), None)
        }
        "cross_interaction_enrichment" => {
            (immune_association_input(true, false, "pre", &mut rng), None)
        }
        "registration_jitter" => (immune_association_input(true, true, "pre", &mut rng), None),
        "prepost_within_margin_spatial_pattern" => {
            let pre = immune_association_input(true, false, "pre", &mut rng);
            let mut post = pre.clone();
            post.timepoint = "post".into();
            (pre, Some(post))
        }
        "prepost_changed_spatial_pattern" => {
            let pre = immune_association_input(true, false, "pre", &mut rng);
            let mut post = pre.clone();
            post.timepoint = "post".into();
            for cell in &mut post.he_cells {
                cell.cell_type = match cell.cell_type.as_deref() {
                    Some("lymphocyte") => Some("tumor".into()),
                    Some("tumor") => Some("lymphocyte".into()),
                    _ => cell.cell_type.clone(),
                };
            }
            (pre, Some(post))
        }
        "registration_residual_above_threshold" => {
            config.registration.max_rmse_um = 1.0;
            (immune_association_input(false, true, "pre", &mut rng), None)
        }
        "too_few_landmarks" => {
            let mut input = immune_association_input(false, false, "pre", &mut rng);
            input.landmarks.truncate(2);
            (input, None)
        }
        "degenerate_landmarks" => {
            let mut input = immune_association_input(false, false, "pre", &mut rng);
            input.landmarks = degenerate_landmarks();
            (input, None)
        }
        "empty_he_cells" => {
            let mut input = immune_association_input(false, false, "pre", &mut rng);
            input.he_cells.clear();
            (input, None)
        }
        "empty_ihc_cells" => {
            let mut input = immune_association_input(false, false, "pre", &mut rng);
            input.ihc_cells.clear();
            (input, None)
        }
        "no_abnormal_cells" => {
            let mut input = immune_association_input(false, false, "pre", &mut rng);
            for cell in &mut input.ihc_cells {
                cell.mmr_mark = Some(0);
                cell.mmr_probability = Some(0.05);
            }
            (input, None)
        }
        "sparse_graph" => {
            config.neighborhood.radius_um = 0.25;
            (
                immune_association_input(false, false, "pre", &mut rng),
                None,
            )
        }
        "zero_expected_edge_count" => {
            let mut input = immune_association_input(false, false, "pre", &mut rng);
            for cell in &mut input.he_cells {
                cell.cell_type = Some("tumor".into());
            }
            (input, None)
        }
        "multiple_cell_classes" => {
            config.neighborhood.label_pairs = vec![
                ["lymphocyte".into(), "mmr_abnormal".into()],
                ["tumor".into(), "mmr_abnormal".into()],
                ["stroma".into(), "mmr_abnormal".into()],
            ];
            let mut input = immune_association_input(true, false, "pre", &mut rng);
            input
                .he_cells
                .extend(cell_cluster("stroma", (45.0, 65.0), "stroma"));
            (input, None)
        }
        "multiple_null_models" => {
            config.neighborhood.null_models = vec![
                NeighborhoodNullModel::SourceSection,
                NeighborhoodNullModel::SourceSectionDensity,
                NeighborhoodNullModel::SourceSectionCellClass,
                NeighborhoodNullModel::SourceSectionRegistrationQc,
            ];
            (immune_association_input(true, false, "pre", &mut rng), None)
        }
        "rigid_rotation" => {
            let mut input = immune_association_input(false, false, "pre", &mut rng);
            input.landmarks = rigid_rotation_landmarks();
            (input, None)
        }
        "affine_deformation" => {
            config.registration.transform = RegistrationTransform::Affine;
            let mut input = immune_association_input(false, false, "pre", &mut rng);
            input.landmarks = affine_landmarks();
            (input, None)
        }
        _ => {
            return Err(MarklabError::Validation(format!(
                "unknown multimodal synthetic generator {generator}"
            )));
        }
    };
    Ok(MultimodalScenario { config, pre, post })
}

fn random_label_input(timepoint: &str, seed: u64) -> Result<MultimodalInput> {
    let labels = permute_fixed_count(25, 12, seed)?;
    let mut he_cells = Vec::new();
    for row in 0..5 {
        for column in 0..5 {
            let index = row * 5 + column;
            he_cells.push(HeCell {
                cell_id: format!("random-{row}-{column}"),
                x_um: 10.0 + column as f64 * 15.0,
                y_um: 10.0 + row as f64 * 15.0,
                cell_type: Some(if labels[index] == 1 {
                    "lymphocyte".into()
                } else {
                    "tumor".into()
                }),
                cell_type_probability: Some(0.95),
            });
        }
    }
    let mut ihc_cells = territory_cluster("random", (20.0, 20.0))
        .into_iter()
        .chain(retained_controls())
        .collect::<Vec<_>>();
    let ihc_marks =
        permute_fixed_count(ihc_cells.len(), 6, splitmix64(seed ^ 0x6968_635f_6d61_726b))?;
    for (cell, mark) in ihc_cells.iter_mut().zip(ihc_marks) {
        cell.mmr_mark = Some(mark);
        cell.mmr_probability = Some(if mark == 1 { 0.95 } else { 0.05 });
    }

    Ok(MultimodalInput {
        he_cells,
        ihc_cells,
        landmarks: identity_landmarks(),
        case_id: "smoke-random-labels".into(),
        timepoint: timepoint.into(),
        protein: "MSH6".into(),
    })
}

pub(in crate::synthetic_smoke) fn multimodal_smoke_config(seed: u64) -> AnalysisConfig {
    let mut config = AnalysisConfig::default();
    config.registration.transform = RegistrationTransform::Rigid;
    config.registration.min_landmarks = 3;
    config.registration.max_rmse_um = 20.0;
    config.registration.claim_distance_multiplier = 2.0;
    config.neighborhood.radius_um = 12.0;
    config.neighborhood.k_nearest = 0;
    config.neighborhood.label_pairs = vec![["lymphocyte".into(), "mmr_abnormal".into()]];
    config.neighborhood.territory_eps_um = 10.0;
    config.neighborhood.territory_min_cells = 3;
    config.neighborhood.territory_min_radius_um = 1.0;
    config.neighborhood.null_models = vec![NeighborhoodNullModel::SourceSection];
    config.permutation.b = 99;
    config.permutation.seed = seed;
    config.permutation.stratified = false;
    config.spectrum.fit_low_k_alpha = false;
    config.comparison.margins.cross_interaction = Some(0.15);
    config
}

fn territory_relation_input(
    related: bool,
    timepoint: &str,
    rng: &mut DeterministicRng,
) -> MultimodalInput {
    let first_center = (20.0 + rng.centered(0.25), 20.0 + rng.centered(0.25));
    let second_center = if related {
        (28.0 + rng.centered(0.25), 20.0 + rng.centered(0.25))
    } else {
        (80.0 + rng.centered(0.25), 80.0 + rng.centered(0.25))
    };
    let ihc_cells = territory_cluster("a", first_center)
        .into_iter()
        .chain(territory_cluster("b", second_center))
        .chain(retained_controls())
        .collect();
    MultimodalInput {
        he_cells: background_he_cells(),
        ihc_cells,
        landmarks: identity_landmarks(),
        case_id: "smoke-territory-relation".into(),
        timepoint: timepoint.into(),
        protein: "MSH6".into(),
    }
}

fn immune_association_input(
    associated: bool,
    registration_jitter: bool,
    timepoint: &str,
    rng: &mut DeterministicRng,
) -> MultimodalInput {
    let abnormal_center = (20.0 + rng.centered(0.25), 20.0 + rng.centered(0.25));
    let lymphocyte_center = if associated {
        (20.5 + rng.centered(0.25), 20.5 + rng.centered(0.25))
    } else {
        (65.0 + rng.centered(0.25), 65.0 + rng.centered(0.25))
    };
    let ihc_cells = territory_cluster("immune", abnormal_center)
        .into_iter()
        .chain(retained_controls())
        .collect();
    let mut he_cells = cell_cluster("lymph", lymphocyte_center, "lymphocyte");
    he_cells.extend(cell_cluster("tumor", (65.0, 20.0), "tumor"));
    MultimodalInput {
        he_cells,
        ihc_cells,
        landmarks: if registration_jitter {
            jittered_landmarks()
        } else {
            identity_landmarks()
        },
        case_id: "smoke-immune-association".into(),
        timepoint: timepoint.into(),
        protein: "MSH6".into(),
    }
}

fn territory_cluster(prefix: &str, center: (f64, f64)) -> Vec<IhcCell> {
    cluster_offsets()
        .into_iter()
        .enumerate()
        .map(|(index, (dx, dy))| IhcCell {
            cell_id: format!("{prefix}-abnormal-{index}"),
            x_um: center.0 + dx,
            y_um: center.1 + dy,
            mmr_mark: Some(1),
            mmr_probability: Some(0.95),
        })
        .collect()
}

fn retained_controls() -> Vec<IhcCell> {
    [
        (50.0, 10.0),
        (60.0, 10.0),
        (70.0, 10.0),
        (50.0, 40.0),
        (60.0, 40.0),
        (70.0, 40.0),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (x_um, y_um))| IhcCell {
        cell_id: format!("retained-{index}"),
        x_um,
        y_um,
        mmr_mark: Some(0),
        mmr_probability: Some(0.95),
    })
    .collect()
}

fn cell_cluster(prefix: &str, center: (f64, f64), label: &str) -> Vec<HeCell> {
    cluster_offsets()
        .into_iter()
        .enumerate()
        .map(|(index, (dx, dy))| HeCell {
            cell_id: format!("{prefix}-{index}"),
            x_um: center.0 + dx,
            y_um: center.1 + dy,
            cell_type: Some(label.into()),
            cell_type_probability: Some(0.95),
        })
        .collect()
}

fn background_he_cells() -> Vec<HeCell> {
    cell_cluster("background-a", (45.0, 20.0), "tumor")
        .into_iter()
        .chain(cell_cluster("background-b", (65.0, 60.0), "stroma"))
        .collect()
}

fn cluster_offsets() -> [(f64, f64); 6] {
    [
        (-1.0, 0.0),
        (0.0, 0.0),
        (1.0, 0.0),
        (-1.0, 1.0),
        (0.0, 1.0),
        (1.0, 1.0),
    ]
}

fn identity_landmarks() -> Vec<LandmarkPair> {
    vec![
        LandmarkPair::new(0.0, 0.0, 0.0, 0.0),
        LandmarkPair::new(100.0, 0.0, 100.0, 0.0),
        LandmarkPair::new(0.0, 100.0, 0.0, 100.0),
        LandmarkPair::new(100.0, 100.0, 100.0, 100.0),
    ]
}

fn jittered_landmarks() -> Vec<LandmarkPair> {
    vec![
        LandmarkPair::new(0.0, 0.0, -4.0, 0.0),
        LandmarkPair::new(100.0, 0.0, 104.0, 0.0),
        LandmarkPair::new(0.0, 100.0, 0.0, 104.0),
        LandmarkPair::new(100.0, 100.0, 100.0, 96.0),
    ]
}

fn degenerate_landmarks() -> Vec<LandmarkPair> {
    vec![
        LandmarkPair::new(0.0, 0.0, 0.0, 0.0),
        LandmarkPair::new(0.0, 0.0, 1.0, 0.0),
        LandmarkPair::new(0.0, 0.0, 0.0, 1.0),
    ]
}

fn rigid_rotation_landmarks() -> Vec<LandmarkPair> {
    [(0.0, 0.0), (100.0, 0.0), (0.0, 100.0), (100.0, 100.0)]
        .into_iter()
        .map(|(x, y)| LandmarkPair::new(x, y, -y + 10.0, x - 5.0))
        .collect()
}

fn affine_landmarks() -> Vec<LandmarkPair> {
    [(0.0, 0.0), (100.0, 0.0), (0.0, 100.0), (100.0, 100.0)]
        .into_iter()
        .map(|(x, y)| LandmarkPair::new(x, y, 1.1 * x + 0.2 * y + 3.0, -0.1 * x + 0.9 * y - 4.0))
        .collect()
}

#[derive(Clone, Copy, Debug)]
struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0xa076_1d64_78bd_642f,
        }
    }

    fn unit(&mut self) -> f64 {
        let value = self.next_u64() >> 11;
        value as f64 / ((1_u64 << 53) as f64)
    }

    fn centered(&mut self, width: f64) -> f64 {
        (2.0 * self.unit() - 1.0) * width
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state ^ (self.state >> 33)
    }
}
