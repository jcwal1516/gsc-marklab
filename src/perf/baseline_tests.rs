use std::{
    fs::File,
    hint::black_box,
    io::{BufWriter, Write},
    time::Instant,
};

use serde_json::{json, Value};

use crate::{
    config::{AnalysisConfig, ComponentMode, ThreadSetting},
    data::{Pattern, PatternMeta},
    geom::{mask::TumorMask, spatial_index::mean_nearest_neighbor_distance},
    io::{
        csv::load_pattern_csv_with_diagnostics,
        parquet::{load_pattern_parquet_with_diagnostics, write_filtered_pattern_export_parquet},
    },
    multimodal::{
        cells::{CellSection, FusedCell, HeCell, IhcCell},
        MultimodalEngine, MultimodalInput,
    },
    multiscale_residual::territories::{detect_residual_territories, ResidualTerritoryPlan},
    neighborhood::{
        graph::{build_spatial_graph, GraphConfig},
        profiles::territory_profiles,
        territories::{detect_mmr_abnormal_territories, TerritoryDomainConfig},
    },
    output::NeighborhoodTerritory,
    registration::landmarks::LandmarkPair,
    spectra::{
        mark_pair_covariance::{mark_pair_covariance, MarkPairCovariancePlan},
        structure_factor::{
            observed_power_for_modes, permutation_whitened_spectrum,
            permutation_whitened_value_spectrum, resolvable_modes_for_pattern,
            SpectrumPermutationOptions,
        },
    },
    AnalysisEngine,
};

const SPATIAL_SIZES: [usize; 3] = [256, 512, 1_024];
const PHASE6_INDEX_SIZES: [usize; 5] = [1_024, 2_048, 4_096, 8_192, 16_384];
const SPECTRAL_SIZES: [usize; 3] = [64, 128, 256];
const COMPLETE_MULTIMODAL_SIZES: [usize; 3] = [24, 48, 96];

fn sample_count() -> usize {
    std::env::var("MARKLAB_BASELINE_SAMPLES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(5)
        .max(3)
}

fn measure_case(
    workload: &str,
    input_size: usize,
    metadata: Value,
    mut operation: impl FnMut() -> u64,
) {
    let expected_checksum = black_box(operation());
    let mut samples_ns = Vec::with_capacity(sample_count());
    for _ in 0..sample_count() {
        let started = Instant::now();
        let checksum = black_box(operation());
        let elapsed_ns = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        assert_eq!(
            checksum, expected_checksum,
            "{workload} must perform deterministic equivalent work"
        );
        samples_ns.push(elapsed_ns);
    }
    samples_ns.sort_unstable();
    let median_ns = samples_ns[samples_ns.len() / 2];
    println!(
        "MARKLAB_BASELINE {}",
        json!({
            "workload": workload,
            "input_size": input_size,
            "samples_ns": samples_ns,
            "median_ns": median_ns,
            "min_ns": samples_ns[0],
            "max_ns": samples_ns[samples_ns.len() - 1],
            "checksum": expected_checksum,
            "thread_count": 1,
            "compiler_profile": if cfg!(debug_assertions) { "debug" } else { "release" },
            "metadata": metadata,
        })
    );
}

fn side_for(n: usize) -> usize {
    (n as f64).sqrt().ceil() as usize
}

fn coordinates(n: usize) -> (Vec<f64>, Vec<f64>) {
    let side = side_for(n);
    (
        (0..n).map(|index| (index % side) as f64).collect(),
        (0..n).map(|index| (index / side) as f64).collect(),
    )
}

fn metadata() -> PatternMeta {
    PatternMeta {
        case_id: "baseline".into(),
        timepoint: "post".into(),
        protein: "MSH6".into(),
        slide_id: None,
        section_id: None,
        stain_batch: None,
        block_id: None,
        region_id: None,
    }
}

fn pattern(n: usize) -> Pattern {
    let (x, y) = coordinates(n);
    let side = side_for(n);
    let mut pattern = Pattern::from_arrays(
        x,
        y,
        (0..n).map(|index| u8::from(index % 7 == 0)).collect(),
        metadata(),
    )
    .expect("benchmark pattern");
    pattern.window.area_um2 = (side * side) as f64;
    pattern.window.l_eff_um = side as f64;
    pattern.window.d_nn_mean_um = 1.0;
    pattern
}

fn fused_cells(n: usize) -> Vec<FusedCell> {
    let (x, y) = coordinates(n);
    x.into_iter()
        .zip(y)
        .enumerate()
        .map(|(index, (x_um, y_um))| {
            let ihc = index % 2 == 0;
            FusedCell {
                source_section: if ihc {
                    CellSection::Ihc
                } else {
                    CellSection::He
                },
                source_cell_id: format!("cell-{index}"),
                x_um_registered: x_um,
                y_um_registered: y_um,
                mmr_mark: ihc.then_some(1),
                mmr_probability: ihc.then_some(0.9),
                cell_type: (!ihc).then(|| {
                    if index % 5 == 0 {
                        "lymphocyte".to_string()
                    } else {
                        "tumor".to_string()
                    }
                }),
                cell_type_probability: (!ihc).then_some(0.9),
                same_section: false,
                registration_error_um: Some(0.25),
            }
        })
        .collect()
}

fn checksum_f64(values: &[f64]) -> u64 {
    values.iter().fold(values.len() as u64, |checksum, value| {
        checksum.rotate_left(7) ^ value.to_bits()
    })
}

fn permutation_options(n_permutations: usize) -> SpectrumPermutationOptions {
    SpectrumPermutationOptions {
        n_shells: 8,
        low_k_modes: 2,
        n_permutations,
        seed: 123,
        family_wise_alpha: 0.10,
        max_scale_um: f64::INFINITY,
        k_shell_min: 1,
    }
}

fn marked_config(n: usize) -> AnalysisConfig {
    let mut config = AnalysisConfig::default();
    config.analysis.analyze_components = ComponentMode::Pooled;
    config.validation.n_min = n;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.validation.area_min_um2 = 1.0;
    config.validation.k_shell_min = 1;
    config.spectrum.k_shells = 8;
    config.spectrum.low_k_shells = 2;
    config.spectrum.anisotropy_low_k_shells = 2;
    config.permutation.b = 19;
    config.permutation.stratified = false;
    config.inference.family_wise_alpha = 0.10;
    config.performance.threads = ThreadSetting::Count(1);
    config.performance.memory_budget_mib = 16_384;
    config
}

fn multimodal_input(n_per_modality: usize) -> MultimodalInput {
    let (x, y) = coordinates(n_per_modality);
    let he_cells = x
        .iter()
        .copied()
        .zip(y.iter().copied())
        .enumerate()
        .map(|(index, (x_um, y_um))| HeCell {
            cell_id: format!("he-{index}"),
            x_um,
            y_um,
            cell_type: Some(if index % 5 == 0 {
                "lymphocyte".into()
            } else {
                "tumor".into()
            }),
            cell_type_probability: Some(0.9),
        })
        .collect();
    let ihc_cells = x
        .into_iter()
        .zip(y)
        .enumerate()
        .map(|(index, (x_um, y_um))| IhcCell {
            cell_id: format!("ihc-{index}"),
            x_um,
            y_um,
            mmr_mark: Some(u8::from(index % 4 == 0)),
            mmr_probability: Some(if index % 4 == 0 { 0.9 } else { 0.1 }),
        })
        .collect();

    MultimodalInput {
        he_cells,
        ihc_cells,
        landmarks: vec![
            LandmarkPair::new(0.0, 0.0, 0.0, 0.0),
            LandmarkPair::new(10.0, 0.0, 10.0, 0.0),
            LandmarkPair::new(0.0, 10.0, 0.0, 10.0),
            LandmarkPair::new(10.0, 10.0, 10.0, 10.0),
            LandmarkPair::new(5.0, 2.0, 5.0, 2.0),
            LandmarkPair::new(2.0, 5.0, 2.0, 5.0),
        ],
        case_id: "baseline".into(),
        timepoint: "post".into(),
        protein: "MSH6".into(),
    }
}

fn multimodal_config() -> AnalysisConfig {
    let mut config = marked_config(1);
    config.validation.n_min = 2;
    config.registration.min_landmarks = 6;
    config.neighborhood.radius_um = 2.0;
    config.neighborhood.k_nearest = 4;
    config.neighborhood.territory_eps_um = 2.0;
    config.neighborhood.territory_min_cells = 1;
    config.neighborhood.territory_min_radius_um = 0.5;
    config
}

fn fixed_density_metadata(n: usize, extra: Value) -> Value {
    json!({
        "point_density": n as f64 / (side_for(n) * side_for(n)) as f64,
        "extra": extra,
    })
}

#[test]
#[ignore = "manual Phase 0 performance baseline"]
fn baseline_perf_nearest_neighbor() {
    for n in SPATIAL_SIZES {
        let (x, y) = coordinates(n);
        measure_case(
            "nearest_neighbor",
            n,
            fixed_density_metadata(n, json!({})),
            || {
                mean_nearest_neighbor_distance(black_box(&x), black_box(&y))
                    .expect("nearest-neighbor distance")
                    .to_bits()
            },
        );
    }
}

#[test]
#[ignore = "manual Phase 6 spatial-index scaling benchmark"]
fn phase6_perf_spatial_index_nearest_neighbor_scaling() {
    for n in PHASE6_INDEX_SIZES {
        let (x, y) = coordinates(n);
        measure_case(
            "phase6_nearest_neighbor",
            n,
            fixed_density_metadata(n, json!({"backend": "rstar_0.13.0"})),
            || {
                mean_nearest_neighbor_distance(black_box(&x), black_box(&y))
                    .expect("nearest-neighbor distance")
                    .to_bits()
            },
        );
    }
}

#[test]
#[ignore = "manual Phase 0 performance baseline"]
fn baseline_perf_radius_and_knn_graph() {
    for n in SPATIAL_SIZES {
        let cells = fused_cells(n);
        measure_case(
            "radius_graph",
            n,
            fixed_density_metadata(n, json!({"radius_um": 1.5})),
            || {
                let graph = build_spatial_graph(
                    black_box(&cells),
                    GraphConfig {
                        radius_um: Some(1.5),
                        k_nearest: None,
                    },
                )
                .expect("radius graph");
                assert!(graph.edges.iter().all(|edge| edge.source < edge.target));
                graph.edges.len() as u64
            },
        );
        measure_case(
            "knn_graph",
            n,
            fixed_density_metadata(n, json!({"k": 8})),
            || {
                let graph = build_spatial_graph(
                    black_box(&cells),
                    GraphConfig {
                        radius_um: None,
                        k_nearest: Some(8),
                    },
                )
                .expect("kNN graph");
                assert!(graph.edges.iter().all(|edge| edge.source < edge.target));
                graph.edges.len() as u64
            },
        );
    }
}

#[test]
#[ignore = "manual Phase 0 performance baseline"]
fn baseline_perf_mark_pair_covariance() {
    for n in SPATIAL_SIZES {
        let pattern = pattern(n);
        measure_case(
            "mark_pair_covariance",
            n,
            fixed_density_metadata(n, json!({"bin_width_um": 1.0, "max_r_um": 5.0})),
            || {
                let bins = mark_pair_covariance(black_box(&pattern), 1.0, 5.0)
                    .expect("mark-pair covariance");
                let pair_count = bins.iter().map(|bin| bin.count).sum::<usize>();
                (pair_count as u64)
                    ^ checksum_f64(
                        &bins
                            .iter()
                            .map(|bin| bin.value.unwrap_or(0.0))
                            .collect::<Vec<_>>(),
                    )
            },
        );
    }
}

#[test]
#[ignore = "manual Phase 6 pair-plan build and evaluation benchmark"]
fn phase6_perf_mark_pair_covariance_plan() {
    for n in SPATIAL_SIZES {
        let pattern = pattern(n);
        let plan = MarkPairCovariancePlan::new(&pattern, 1.0, 5.0).expect("pair plan");
        measure_case(
            "pair_plan_observed_evaluation",
            n,
            fixed_density_metadata(
                n,
                json!({"pair_count": plan.evaluate(&pattern.mark).expect("observed").iter().map(|bin| bin.count).sum::<usize>()}),
            ),
            || pair_bin_checksum(&plan.evaluate(black_box(&pattern.mark)).expect("observed")),
        );

        let permutation_marks = (0..19)
            .map(|permutation| {
                (0..n)
                    .map(|index| u8::from((index + 7 * permutation) % 5 == 0))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        measure_case(
            "pair_plan_19_label_evaluations",
            n,
            fixed_density_metadata(n, json!({"evaluations": permutation_marks.len()})),
            || {
                permutation_marks.iter().fold(0_u64, |checksum, marks| {
                    checksum
                        ^ pair_bin_checksum(&plan.evaluate(black_box(marks)).expect("permutation"))
                })
            },
        );
    }
}

fn pair_bin_checksum(bins: &[crate::spectra::mark_pair_covariance::MarkPairCovarianceBin]) -> u64 {
    bins.iter().fold(0_u64, |checksum, bin| {
        checksum.rotate_left(7) ^ bin.value.unwrap_or(0.0).to_bits() ^ bin.count as u64
    })
}

#[test]
#[ignore = "manual Phase 0 performance baseline"]
fn baseline_perf_territories_and_profiles() {
    for n in SPATIAL_SIZES {
        let pattern = pattern(n);
        measure_case(
            "marked_territories",
            n,
            fixed_density_metadata(n, json!({"scales": 3, "min_z": 0.0})),
            || detect_residual_territories(black_box(&pattern), 0.0).len() as u64,
        );

        let cells = fused_cells(n);
        measure_case(
            "multimodal_territories",
            n,
            fixed_density_metadata(n, json!({"eps_um": 1.5, "abnormal_fraction": 0.5})),
            || {
                detect_mmr_abnormal_territories(
                    black_box(&cells),
                    TerritoryDomainConfig {
                        eps_um: 1.5,
                        min_cells: 2,
                        min_radius_um: 0.5,
                    },
                )
                .expect("multimodal territories")
                .len() as u64
            },
        );

        let side = side_for(n);
        let territories = (0..16)
            .map(|index| NeighborhoodTerritory {
                center_x_um: (index % 4) as f64 * (side as f64 / 4.0),
                center_y_um: (index / 4) as f64 * (side as f64 / 4.0),
                radius_um: 3.0,
                supporting_abnormal_cells: 1,
                cluster_id: index,
            })
            .collect::<Vec<_>>();
        measure_case(
            "territory_profiles",
            n,
            fixed_density_metadata(n, json!({"territory_count": territories.len()})),
            || {
                territory_profiles(black_box(&territories), black_box(&cells), 1.0)
                    .expect("territory profiles")
                    .iter()
                    .map(|profile| {
                        profile
                            .cell_type_fractions
                            .iter()
                            .map(|fraction| fraction.count)
                            .sum::<usize>()
                    })
                    .sum::<usize>() as u64
            },
        );
    }
}

#[test]
#[ignore = "manual Phase 6 residual-territory plan benchmark"]
fn phase6_perf_residual_territory_plan() {
    for n in SPATIAL_SIZES {
        let pattern = pattern(n);
        let plan = ResidualTerritoryPlan::new(&pattern).expect("residual territory plan");
        measure_case(
            "residual_territory_plan_observed_evaluation",
            n,
            fixed_density_metadata(n, json!({"scales": 3, "min_z": 0.0})),
            || {
                residual_territory_checksum(
                    &plan
                        .detect_for_marks(black_box(&pattern), &pattern.mark, 0.0)
                        .expect("observed residual territories"),
                )
            },
        );

        let permutation_marks = (0..19)
            .map(|permutation| {
                (0..n)
                    .map(|index| u8::from((index + 7 * permutation) % 5 == 0))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        measure_case(
            "residual_territory_plan_19_label_evaluations",
            n,
            fixed_density_metadata(n, json!({"evaluations": permutation_marks.len()})),
            || {
                permutation_marks.iter().fold(0_u64, |checksum, marks| {
                    checksum
                        ^ residual_territory_checksum(
                            &plan
                                .detect_for_marks(black_box(&pattern), marks, 0.0)
                                .expect("permuted residual territories"),
                        )
                })
            },
        );
    }
}

#[test]
#[ignore = "manual Phase 6 indexed radius-consumer scaling benchmark"]
fn phase6_perf_indexed_radius_consumers_scaling() {
    for &n in &PHASE6_INDEX_SIZES[..4] {
        let cells = fused_cells(n);
        measure_case(
            "phase6_radius_graph_scaling",
            n,
            fixed_density_metadata(n, json!({"radius_um": 1.5})),
            || {
                build_spatial_graph(
                    black_box(&cells),
                    GraphConfig {
                        radius_um: Some(1.5),
                        k_nearest: None,
                    },
                )
                .expect("radius graph")
                .edges
                .len() as u64
            },
        );
        measure_case(
            "phase6_multimodal_territory_scaling",
            n,
            fixed_density_metadata(n, json!({"eps_um": 1.5, "abnormal_fraction": 0.5})),
            || {
                detect_mmr_abnormal_territories(
                    black_box(&cells),
                    TerritoryDomainConfig {
                        eps_um: 1.5,
                        min_cells: 2,
                        min_radius_um: 0.5,
                    },
                )
                .expect("multimodal territories")
                .len() as u64
            },
        );

        let side = side_for(n);
        let territory_count = n / 8;
        let territories = (0..territory_count)
            .map(|territory_index| {
                let cell_index = territory_index * 8;
                NeighborhoodTerritory {
                    center_x_um: (cell_index % side) as f64,
                    center_y_um: (cell_index / side) as f64,
                    radius_um: 3.0,
                    supporting_abnormal_cells: 1,
                    cluster_id: territory_index as u32,
                }
            })
            .collect::<Vec<_>>();
        measure_case(
            "phase6_territory_profile_scaling",
            n,
            fixed_density_metadata(n, json!({"territory_count": territory_count})),
            || {
                territory_profiles(black_box(&territories), black_box(&cells), 1.0)
                    .expect("territory profiles")
                    .iter()
                    .map(|profile| {
                        profile
                            .cell_type_fractions
                            .iter()
                            .map(|fraction| fraction.count)
                            .sum::<usize>()
                    })
                    .sum::<usize>() as u64
            },
        );
    }
}

fn residual_territory_checksum(
    territories: &[crate::multiscale_residual::territories::ResidualTerritoryCandidate],
) -> u64 {
    territories.iter().fold(0_u64, |checksum, territory| {
        checksum.rotate_left(7)
            ^ territory.center_x_um.to_bits()
            ^ territory.center_y_um.to_bits()
            ^ territory.residual_score.to_bits()
            ^ territory.supporting_marked_cells as u64
    })
}

#[test]
#[ignore = "manual Phase 0 performance baseline"]
fn baseline_perf_structure_factor_observed() {
    for n in SPECTRAL_SIZES {
        let pattern = pattern(n);
        let modes = resolvable_modes_for_pattern(&pattern, 8).expect("resolvable modes");
        measure_case(
            "structure_factor_observed",
            n,
            fixed_density_metadata(n, json!({"mode_count": modes.len(), "shell_count": 8})),
            || {
                checksum_f64(&observed_power_for_modes(
                    black_box(&pattern),
                    black_box(&modes),
                ))
            },
        );
    }
}

#[test]
#[ignore = "manual Phase 0 performance baseline"]
fn baseline_perf_structure_factor_permutations() {
    for n in SPECTRAL_SIZES {
        let pattern = pattern(n);
        measure_case(
            "structure_factor_permutations",
            n,
            fixed_density_metadata(n, json!({"permutation_count": 19, "shell_count": 8})),
            || {
                let spectrum =
                    permutation_whitened_spectrum(black_box(&pattern), permutation_options(19))
                        .expect("permutation spectrum")
                        .expect("evaluable spectrum");
                checksum_f64(&spectrum.observed_power) ^ spectrum.n_permutations as u64
            },
        );
    }
}

#[test]
#[ignore = "manual Phase 0 performance baseline"]
fn baseline_perf_probabilistic_spectrum() {
    for n in SPECTRAL_SIZES {
        let pattern = pattern(n);
        let values = (0..n)
            .map(|index| if index % 7 == 0 { 0.9 } else { 0.1 })
            .collect::<Vec<_>>();
        measure_case(
            "probabilistic_mark_spectrum",
            n,
            fixed_density_metadata(n, json!({"permutation_count": 19, "shell_count": 8})),
            || {
                let spectrum = permutation_whitened_value_spectrum(
                    black_box(&pattern),
                    black_box(&values),
                    permutation_options(19),
                )
                .expect("probabilistic spectrum")
                .expect("evaluable spectrum");
                checksum_f64(&spectrum.observed_power) ^ spectrum.n_permutations as u64
            },
        );
    }
}

fn write_csv_fixture(path: &std::path::Path, n: usize) {
    let file = File::create(path).expect("create CSV fixture");
    let mut writer = BufWriter::new(file);
    writeln!(
        writer,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc"
    )
    .expect("CSV header");
    let side = side_for(n);
    for index in 0..n {
        writeln!(
            writer,
            "{},{},{},baseline,post,MSH6,true,true",
            index % side,
            index / side,
            u8::from(index % 7 == 0)
        )
        .expect("CSV row");
    }
    writer.flush().expect("flush CSV fixture");
}

fn bounding_mask(n: usize) -> TumorMask {
    let side = side_for(n);
    TumorMask::from_geojson_str(&format!(
        r#"{{"type":"MultiPolygon","coordinates":[[[[-1,-1],[{side},-1],[{side},{side}],[-1,{side}],[-1,-1]]]]}}"#
    ))
    .expect("bounding mask")
}

#[test]
#[ignore = "manual Phase 0 performance baseline"]
fn baseline_perf_csv_and_parquet_load() {
    let directory = tempfile::tempdir().expect("input benchmark directory");
    for n in SPATIAL_SIZES {
        let mask = bounding_mask(n);
        let csv_path = directory.path().join(format!("cells-{n}.csv"));
        write_csv_fixture(&csv_path, n);
        measure_case(
            "csv_load",
            n,
            fixed_density_metadata(n, json!({"format": "csv"})),
            || {
                let loaded = load_pattern_csv_with_diagnostics(black_box(&csv_path), &mask)
                    .expect("CSV load")
                    .pattern;
                ((loaded.len() as u64) << 32) ^ loaded.n_marked() as u64
            },
        );

        let parquet_path = directory.path().join(format!("cells-{n}.parquet"));
        write_filtered_pattern_export_parquet(&pattern(n), &parquet_path).expect("Parquet fixture");
        measure_case(
            "parquet_load",
            n,
            fixed_density_metadata(n, json!({"format": "parquet"})),
            || {
                let loaded = load_pattern_parquet_with_diagnostics(black_box(&parquet_path), &mask)
                    .expect("Parquet load")
                    .pattern;
                ((loaded.len() as u64) << 32) ^ loaded.n_marked() as u64
            },
        );
    }
}

#[test]
#[ignore = "manual Phase 0 performance baseline"]
fn baseline_perf_complete_marked_analysis() {
    for n in SPECTRAL_SIZES {
        let pattern = pattern(n);
        let engine = AnalysisEngine::new(marked_config(n)).expect("marked engine");
        measure_case(
            "complete_marked_analysis",
            n,
            fixed_density_metadata(n, json!({"permutation_count": 19, "thread_count": 1})),
            || {
                let result = engine
                    .analyze_pattern(black_box(&pattern))
                    .expect("marked analysis");
                ((result.n_cells as u64) << 32)
                    ^ result.n_marked as u64
                    ^ result.spectrum_curve.len() as u64
            },
        );
    }
}

#[test]
#[ignore = "manual Phase 0 performance baseline"]
fn baseline_perf_complete_multimodal_analysis() {
    for n in COMPLETE_MULTIMODAL_SIZES {
        let input = multimodal_input(n);
        let engine = MultimodalEngine::new(multimodal_config()).expect("multimodal engine");
        measure_case(
            "complete_multimodal_analysis",
            n * 2,
            fixed_density_metadata(
                n * 2,
                json!({
                    "he_cells": n,
                    "ihc_cells": n,
                    "permutation_count": 19,
                    "thread_count": 1,
                }),
            ),
            || {
                let result = engine
                    .analyze(black_box(&input))
                    .expect("multimodal analysis");
                ((result.fused_cells.len() as u64) << 32)
                    ^ result.neighborhood_enrichment.value().map_or(0, Vec::len) as u64
                    ^ result.cross_interaction_curves.value().map_or(0, Vec::len) as u64
            },
        );
    }
}
