use std::{fmt::Write as _, fs, path::Path};

use assert_cmd::Command;

#[allow(dead_code)]
pub(crate) fn command(location: &Path, embedding: &Path, output: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args(arguments(location, embedding, output));
    command
}

pub(crate) fn arguments(location: &Path, embedding: &Path, output: &Path) -> Vec<String> {
    let mut arguments = vec![
        "bayes".to_owned(),
        "fit-joint-replicated-location-embedding".to_owned(),
        "--location-input".to_owned(),
        location.to_str().unwrap().to_owned(),
        "--embedding-input".to_owned(),
        embedding.to_str().unwrap().to_owned(),
    ];
    arguments.extend(
        [
            "--reference-group",
            "reference",
            "--comparison-group",
            "comparison",
            "--embedding-projection-identity",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--factors",
            "1",
            "--location-prior-sd",
            "1",
            "--group-prior-sd",
            "0.5",
            "--patient-factor-prior-scale",
            "0.5",
            "--location-factor-loading-prior-sd",
            "1",
            "--embedding-loading-prior-sd",
            "1",
            "--embedding-noise-prior-scale",
            "1",
            "--field-length-scale-prior-scale-um",
            "1",
            "--jitter",
            "0.000001",
            "--chains",
            "2",
            "--tune",
            "100",
            "--draws",
            "100",
            "--target-accept",
            "0.9",
            "--seed",
            "20260831",
            "--maximum-patients",
            "8",
            "--maximum-patterns",
            "16",
            "--maximum-location-rows",
            "128",
            "--maximum-embedding-points",
            "128",
            "--maximum-embedding-dimension",
            "8",
            "--maximum-factor-count",
            "4",
            "--maximum-nearest-node-visits",
            "1024",
            "--maximum-kernel-cube-work",
            "2048",
            "--maximum-draw-observation-work",
            "200000",
            "--maximum-working-bytes",
            "536870912",
            "--maximum-tree-depth",
            "10",
            "--timeout-seconds",
            "600",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    arguments.push("--out".into());
    arguments.push(output.to_str().unwrap().to_owned());
    arguments
}

pub(crate) fn write_inputs(location: &Path, embedding: &Path) {
    let mut locations = String::from(
        "pattern_id,patient_id,group,cohort,node_id,type_id,x_um,y_um,weight_um2,window_area_um2,covariate,count,window_sha256,event_sha256\n",
    );
    let mut embeddings = String::from(
        "pattern_id,patient_id,group,point_id,x_um,y_um,embedding_0,embedding_1,embedding_2\n",
    );
    for group_index in 0..2 {
        let group = if group_index == 0 {
            "reference"
        } else {
            "comparison"
        };
        for patient_index in 0..4 {
            let patient = format!("{group}-p{patient_index}");
            for pattern_index in 0..2 {
                let pattern = format!("{patient}-s{pattern_index}");
                for node_index in 0..4 {
                    let ix = node_index % 2;
                    let iy = node_index / 2;
                    let signal = if ix == iy { 1.5 } else { -1.5 };
                    for (type_index, type_id) in ["A", "B"].into_iter().enumerate() {
                        writeln!(
                            locations,
                            "{pattern},{patient},{group},synthetic,q-{node_index},{type_id},{},{},1,4,0,{},{:064x},{:064x}",
                            ix as f64 + 0.5,
                            iy as f64 + 0.5,
                            if signal > 0.0 { 12 + type_index } else { 2 + type_index },
                            group_index * 100 + patient_index * 10 + pattern_index + 1,
                            group_index * 1000 + patient_index * 100 + pattern_index + 1,
                        ).unwrap();
                    }
                    for replicate in 0..2 {
                        let point = format!("{pattern}-c{node_index}-{replicate}");
                        let offset = patient_index as f64 * 0.02 + replicate as f64 * 0.01;
                        writeln!(
                            embeddings,
                            "{pattern},{patient},{group},{point},{},{},{},{},{}",
                            ix as f64 + 0.45 + replicate as f64 * 0.05,
                            iy as f64 + 0.5,
                            signal + offset,
                            0.5 * signal - offset,
                            -signal + offset,
                        )
                        .unwrap();
                    }
                }
            }
        }
    }
    fs::write(location, locations).unwrap();
    fs::write(embedding, embeddings).unwrap();
}
