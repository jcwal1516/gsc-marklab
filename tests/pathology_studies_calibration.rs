//! Prespecified scientific experiments, deliberately excluded from ordinary test runs.
use serde_json::{json, Value};
use std::{collections::BTreeMap, io::Write};

struct Rng(u64);
impl Rng {
    fn unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }
    fn poisson(&mut self, mean: f64) -> usize {
        let stop = (-mean).exp();
        let mut product = 1.0;
        let mut count = 0;
        loop {
            product *= self.unit();
            if product <= stop {
                return count;
            }
            count += 1;
        }
    }
}
fn window(width: f64) -> Value {
    json!({"type":"MultiPolygon","coordinates":[[[[0,0],[width,0],[width,1000],[0,1000],[0,0]]]]})
}
fn cell(i: usize, x: f64, y: f64, positive: bool, stratum: &str) -> Value {
    json!({"id":format!("c{i:04}"),"x_um":x,"y_um":y,"phenotype":if positive {"immune"}else{"tumor"},"stratum":stratum})
}
fn slide(i: usize, width: f64, cells: Vec<Value>) -> Value {
    json!({"slide_id":format!("s{i:03}"),"patient_id":format!("p{i:03}"),"group":if i<8 {"A"}else{"B"},"coordinate_frame_id":format!("f{i}"),"window":window(width),"cells":cells})
}
fn recipe(format: &str, slides: Vec<Value>, design: Value) -> Value {
    json!({"format":format,"version":1,
    "study":{"study_id":"prespecified-calibration","phenotypes":{"names":["immune","tumor"],"measurement_status":"measured","provenance":"synthetic known generator"},"slides":slides},"design":design,
    "limits":{"maximum_cells":10000,"maximum_work":100000000,"maximum_geometry_vertices":10000,"maximum_tiles":10000,"memory_budget_bytes":134217728}})
}

fn composition(scenario: usize, seed: u64) -> Value {
    let mut rng = Rng(seed);
    let mut slides = Vec::new();
    for i in 0..16 {
        let width = if scenario == 1 {
            500.0 + 1000.0 * rng.unit()
        } else {
            1000.0
        };
        let count = rng.poisson(48.0 * width / 1000.0);
        let mut cells = Vec::new();
        for j in 0..count {
            let probability = if scenario == 2 {
                0.035
            } else if scenario == 3 && i >= 8 {
                0.65
            } else {
                0.3
            };
            let positive = rng.unit() < probability;
            let (x, y) = if scenario == 4 && i >= 8 && positive {
                (220.0 + 60.0 * rng.unit(), 220.0 + 60.0 * rng.unit())
            } else {
                (width * rng.unit(), 1000.0 * rng.unit())
            };
            cells.push(cell(j, x, y, positive, "all"));
        }
        slides.push(slide(i, width, cells));
    }
    recipe(
        "marklab.pathology_composition_recipe",
        slides,
        json!({"group_a":"A","group_b":"B","permutations":199,"bootstrap_replicates":199,"seed":seed^0x434f4d50,"alpha":0.05,
        "balances":[{"name":"immune-tumor","kind":"amalgamated_log_ratio","numerator":["immune"],"denominator":["tumor"]}],
        "maup":{"phenotype":"immune","origin_um":[0,0],"primary":"w250-00","grids":[{"id":"w250-00","width_um":250,"offset_um":[0,0]},{"id":"w250-half","width_um":250,"offset_um":[125,125]},{"id":"w500-00","width_um":500,"offset_um":[0,0]}]}}),
    )
}

// Families: 0 homogeneous, 1 smooth gradient, 2 masked gradient, 3 sharp compartments,
// 4 sparse phenotype, 5 clustered, 6 inhibited, 7 cross attraction.
fn spatial(family: usize, statistic: &str, mode: &str, seed: u64) -> Value {
    let mut rng = Rng(seed);
    let count = if statistic == "cross_g_minus_one" && family != 4 {
        80
    } else if family == 6 {
        36
    } else {
        40
    };
    let mut cells = Vec::new();
    for i in 0..count {
        let (mut x, mut y) = loop {
            let x = if family == 1 || family == 2 {
                1000.0 * rng.unit().sqrt()
            } else {
                1000.0 * rng.unit()
            };
            let y = 1000.0 * rng.unit();
            if family == 2
                && ((400.0..=600.0).contains(&x)
                    || ((100.0..=200.0).contains(&x) && (400.0..=600.0).contains(&y)))
            {
                continue;
            }
            break (x, y);
        };
        if family == 3 {
            x = if i < 30 {
                500.0 * rng.unit()
            } else {
                500.0 + 500.0 * rng.unit()
            };
        }
        if family == 5 {
            let center = if i % 2 == 0 { 300.0 } else { 700.0 };
            x = center + 50.0 * (rng.unit() - 0.5);
            y = center + 50.0 * (rng.unit() - 0.5);
        }
        if family == 6 {
            x = 80.0 + (i % 6) as f64 * 160.0 + 10.0 * rng.unit();
            y = 80.0 + (i / 6) as f64 * 160.0 + 10.0 * rng.unit();
        }
        if family == 7 {
            if i < 40 {
                x = 100.0 + 800.0 * rng.unit();
                y = 100.0 + 800.0 * rng.unit();
            } else {
                let parent: &Value = &cells[i - 40];
                let angle = std::f64::consts::TAU * rng.unit();
                x = parent["x_um"].as_f64().unwrap() + 45.0 * angle.cos();
                y = parent["y_um"].as_f64().unwrap() + 45.0 * angle.sin();
            }
        }
        let positive = if family == 4 {
            i < 4
        } else if family == 7 {
            i < 40
        } else {
            i % 2 == 0
        };
        cells.push(cell(i, x, y, positive, "all"));
    }
    let mut s = slide(0, 1000.0, cells);
    let mut partitions = vec![];
    if family == 2 {
        s["window"] = json!({"type":"MultiPolygon","coordinates":[[[[0,0],[400,0],[400,1000],[0,1000],[0,0]],[[100,400],[100,600],[200,600],[200,400],[100,400]]],[[[600,0],[1000,0],[1000,1000],[600,1000],[600,0]]]]});
    }
    if family == 3 {
        s["window"] = json!({"type":"MultiPolygon","coordinates":[[[[0,0],[500,0],[1000,0],[1000,1000],[500,1000],[0,1000],[0,0]]]]});
        partitions.push(json!({"slide_id":"s000","negative_id":"left","positive_id":"right","provenance":"known synthetic split",
            "negative":{"type":"MultiPolygon","coordinates":[[[[0,0],[500,0],[500,1000],[0,1000],[0,0]]]]},
            "positive":{"type":"MultiPolygon","coordinates":[[[[500,0],[1000,0],[1000,1000],[500,1000],[500,0]]]]}}));
    }
    let cross = statistic == "cross_g_minus_one";
    let radii = if family == 7 || statistic == "l_minus_r" {
        vec![50, 100]
    } else {
        vec![100, 200]
    };
    recipe(
        "marklab.pathology_spatial_study_recipe",
        vec![s],
        json!({"group_a":"A","group_b":"B","permutations":199,"seed":seed^0x53504154,"alpha":0.05,"statistic":statistic,"phenotype":null,
        "source":if cross {Some("immune")}else{None},"target":if cross {Some("tumor")}else{None},"radii_um":radii,"pair_bandwidth_um":if family==7 {30}else{40},
        "intensity":{"model":if family==3 {"binary_compartments"}else{"gaussian"},"bandwidth_um":250,"grid":[8,8],"minimum_intensity_per_um2":1e-12,"cross_fit_folds":null,"bandwidth_candidates_um":[],"null_mode":mode},"compartments":partitions}),
    )
}

fn scan(family: usize, seed: u64) -> Value {
    let mut rng = Rng(seed);
    let mut cells = Vec::new();
    for i in 0..48 {
        let (x, y) = if family == 2 && i < 12 {
            (250.0 + 40.0 * rng.unit(), 250.0 + 40.0 * rng.unit())
        } else {
            (1000.0 * rng.unit(), 1000.0 * rng.unit())
        };
        let (p, stratum) = if family == 1 {
            if x < 500.0 {
                (0.8, "left")
            } else {
                (0.2, "right")
            }
        } else if family == 2 {
            if i < 12 {
                (0.95, "all")
            } else {
                (0.1, "all")
            }
        } else {
            (0.35, "all")
        };
        cells.push(cell(i, x, y, rng.unit() < p, stratum));
    }
    recipe(
        "marklab.pathology_scan_recipe",
        vec![slide(0, 1000.0, cells)],
        json!({"positive_phenotype":"immune","eligible_phenotypes":["immune","tumor"],"radii_um":[80,150,250],"permutations":199,"seed":seed^0x5343414e,"alpha":0.05}),
    )
}

fn interval(successes: usize, n: usize) -> Option<[f64; 2]> {
    if n == 0 {
        return None;
    }
    let n = n as f64;
    let p = successes as f64 / n;
    let z = 1.959963984540054;
    let d = 1.0 + z * z / n;
    let center = (p + z * z / (2.0 * n)) / d;
    let half = z * ((p * (1.0 - p) + z * z / (4.0 * n)) / n).sqrt() / d;
    Some([(center - half).max(0.0), (center + half).min(1.0)])
}

#[test]
#[ignore = "explicit 1000-null/250-alternative scientific experiment; use release mode"]
fn pathology_studies_prespecified_calibration() {
    let path = std::env::var("MARKLAB_STAT_CALIBRATION_OUTPUT")
        .expect("set a new calibration output path");
    let mut output = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .unwrap();
    let mut scenarios = Vec::new();
    for (family, name) in [
        "composition_exchangeable",
        "composition_unequal_area",
        "composition_rare_zero",
        "composition_shift",
        "composition_heterogeneity",
    ]
    .into_iter()
    .enumerate()
    {
        scenarios.push((name.to_string(), "composition", family, "", "", family < 3));
    }
    for (family, statistic, mode, name) in [
        (0, "l_minus_r", "plugin", "homogeneous_l_plugin"),
        (0, "l_minus_r", "refit", "homogeneous_l_refit"),
        (1, "l_minus_r", "plugin", "gradient_l_plugin"),
        (1, "l_minus_r", "refit", "gradient_l_refit"),
        (2, "l_minus_r", "refit", "masked_gradient_l_refit"),
        (
            3,
            "l_minus_r",
            "conditional_compartments",
            "sharp_compartment_l",
        ),
        (0, "g_minus_one", "refit", "homogeneous_g_refit"),
        (
            0,
            "cross_g_minus_one",
            "plugin",
            "homogeneous_cross_g_plugin",
        ),
        (0, "cross_g_minus_one", "refit", "homogeneous_cross_g_refit"),
        (4, "cross_g_minus_one", "refit", "sparse_cross_g_refit"),
        (5, "l_minus_r", "plugin", "cluster_l_plugin"),
        (5, "l_minus_r", "refit", "cluster_l_refit"),
        (5, "g_minus_one", "refit", "cluster_g_refit"),
        (6, "l_minus_r", "refit", "inhibition_l_refit"),
        (7, "cross_g_minus_one", "refit", "attraction_cross_g_refit"),
    ] {
        scenarios.push((name.into(), "spatial", family, statistic, mode, family < 5));
    }
    for (family, name) in [
        "scan_random_labels",
        "scan_stratified_prevalence",
        "scan_enrichment",
    ]
    .into_iter()
    .enumerate()
    {
        scenarios.push((name.into(), "scan", family, "", "", family < 2));
    }
    for (name, workflow, family, statistic, mode, is_null) in scenarios {
        let attempts = if is_null { 1000 } else { 250 };
        let mut rejections = 0;
        let mut completed = 0;
        let mut unavailable = 0;
        let mut errors = 0;
        let mut partial = 0;
        let mut first_failure = None;
        let base = match workflow {
            "composition" => 0x434f4d50,
            "spatial" => 0x53504154,
            _ => 0x5343414e,
        };
        let mut reasons = BTreeMap::<String, usize>::new();
        for replicate in 0..attempts {
            let seed = 20261002u64
                .wrapping_add(base)
                .wrapping_add(family as u64 * 100_000)
                .wrapping_add(replicate as u64);
            let input = match workflow {
                "composition" => composition(family, seed),
                "spatial" => spatial(family, statistic, mode, seed),
                _ => scan(family, seed),
            };
            let bytes = serde_json::to_vec(&input).unwrap();
            let value = match workflow {
                "composition" => marklab::analyze_pathology_composition(&bytes)
                    .map(|v| serde_json::to_value(v).unwrap()),
                "spatial" => marklab::analyze_pathology_spatial_study(&bytes)
                    .map(|v| serde_json::to_value(v).unwrap()),
                _ => marklab::analyze_pathology_scan(&bytes)
                    .map(|v| serde_json::to_value(v).unwrap()),
            };
            match value {
                Err(e) => {
                    errors += 1;
                    if first_failure.is_none() {
                        first_failure = Some(e.to_string());
                    }
                }
                Ok(v) => {
                    let p = if workflow == "composition" {
                        let endpoints = v["inference"]["endpoints"].as_array().unwrap();
                        partial +=
                            usize::from(endpoints.iter().any(|e| e["adjusted_p_value"].is_null()));
                        endpoints
                            .iter()
                            .filter_map(|e| e["adjusted_p_value"].as_f64())
                            .reduce(f64::min)
                    } else {
                        v["slides"][0]["p_global"].as_f64()
                    };
                    if let Some(p) = p {
                        completed += 1;
                        rejections += usize::from(p <= 0.05);
                    } else {
                        unavailable += 1;
                        let reason = if workflow == "composition" {
                            v["inference"]["unavailable"].as_str()
                        } else {
                            v["slides"][0]["unavailable"].as_str()
                        }
                        .unwrap_or("no available inference");
                        *reasons.entry(reason.to_string()).or_default() += 1;
                    }
                }
            }
        }
        let rate = if completed == 0 {
            None
        } else {
            Some(rejections as f64 / completed as f64)
        };
        let row = json!({"scenario":name,"workflow":workflow,"statistic":statistic,"null_mode":mode,"is_null":is_null,"outer_attempted":attempts,"inner_replicates":199,
            "completed_inference":completed,"unavailable":unavailable,"errors":errors,"partial_endpoint_family":partial,"rejections":rejections,
            "rate_among_available":rate,"wilson_95_among_available":interval(rejections,completed),"mcse_among_available":rate.map(|p|(p*(1.0-p)/completed as f64).sqrt()),
            "rejections_per_attempt":rejections as f64/attempts as f64,"availability_rate":completed as f64/attempts as f64,
            "unavailable_reasons":reasons,"first_error":first_failure,
            "scope":if workflow=="composition" {"patient_MaxT_available_family"}else{"single_slide_complete_search_or_curve"}});
        writeln!(output, "{row}").unwrap();
        output.flush().unwrap();
        println!("{row}");
        assert_eq!(completed + unavailable + errors, attempts);
        assert_eq!(
            errors, 0,
            "unexpected execution error in {name}; preserve output and investigate"
        );
    }
}
