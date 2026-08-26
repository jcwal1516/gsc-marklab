use super::{embedding_spatial, publish_json, BayesCliError};
use marklab_bayes::{
    build_region_retrieval_index, retrieve_analogous_regions, sha256_hex, QueryRegion,
    RegionRetrievalError, RegionRetrievalResult, RetrievalLeakagePolicy, TrainingRegion,
};
use serde::Serialize;
use std::path::PathBuf;
pub(super) fn run(
    training: PathBuf,
    query: PathBuf,
    k: u32,
    policy: String,
    max: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let tb = embedding_spatial::read(&training)?;
    let qb = embedding_spatial::read(&query)?;
    let (rows, names) = training_rows(&tb)?;
    let q = query_row(&qb)?;
    let index = build_region_retrieval_index(rows, names, max).map_err(map)?;
    let result = retrieve_analogous_regions(
        index,
        q,
        k,
        RetrievalLeakagePolicy::parse(&policy).map_err(map)?,
    )
    .map_err(map)?;
    publish_json(
        &out,
        &Output {
            training_sha256: sha256_hex(&tb),
            query_sha256: sha256_hex(&qb),
            result,
        },
    )
}
fn training_rows(b: &[u8]) -> Result<(Vec<TrainingRegion>, Vec<String>), BayesCliError> {
    let mut r = csv::ReaderBuilder::new().from_reader(b);
    let h = r.headers()?.clone();
    if h.len() < 8
        || h.iter().take(6).collect::<Vec<_>>()
            != [
                "region_id",
                "patient_id",
                "site_id",
                "split",
                "domain",
                "provenance_sha256",
            ]
    {
        return Err(BayesCliError::Input(
            "training retrieval headers differ".into(),
        ));
    }
    let names = h.iter().skip(6).map(str::to_owned).collect();
    let mut rows = Vec::new();
    for x in r.records() {
        let x = x?;
        rows.push(TrainingRegion {
            region_id: x[0].into(),
            patient_id: x[1].into(),
            site_id: x[2].into(),
            split: x[3].into(),
            domain: x[4].into(),
            provenance_sha256: x[5].into(),
            embedding: (6..x.len())
                .map(|i| {
                    x[i].parse()
                        .map_err(|_| BayesCliError::Input("training embedding is invalid".into()))
                })
                .collect::<Result<_, _>>()?,
        });
    }
    Ok((rows, names))
}
fn query_row(b: &[u8]) -> Result<QueryRegion, BayesCliError> {
    let mut r = csv::ReaderBuilder::new().from_reader(b);
    let h = r.headers()?.clone();
    if h.len() < 7
        || h.iter().take(5).collect::<Vec<_>>()
            != [
                "region_id",
                "patient_id",
                "site_id",
                "domain",
                "provenance_sha256",
            ]
    {
        return Err(BayesCliError::Input(
            "query retrieval headers differ".into(),
        ));
    }
    let rows = r.records().collect::<Result<Vec<_>, _>>()?;
    if rows.len() != 1 {
        return Err(BayesCliError::Input("query must contain one row".into()));
    }
    let x = &rows[0];
    Ok(QueryRegion {
        region_id: x[0].into(),
        patient_id: x[1].into(),
        site_id: x[2].into(),
        domain: x[3].into(),
        provenance_sha256: x[4].into(),
        embedding: (5..x.len())
            .map(|i| {
                x[i].parse()
                    .map_err(|_| BayesCliError::Input("query embedding is invalid".into()))
            })
            .collect::<Result<_, _>>()?,
    })
}
fn map(e: RegionRetrievalError) -> BayesCliError {
    BayesCliError::Input(e.to_string())
}
#[derive(Serialize)]
struct Output {
    training_sha256: String,
    query_sha256: String,
    #[serde(flatten)]
    result: RegionRetrievalResult,
}
