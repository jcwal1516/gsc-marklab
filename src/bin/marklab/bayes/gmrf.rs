use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

use marklab_bayes::{
    gmrf_log_density, GmrfConstraint, GmrfDensityError, GmrfDensityResult, GmrfSpec,
};
use serde::{Deserialize, Serialize};

use super::{publish_json, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegionRow {
    region_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrecisionRow {
    source_region: String,
    target_region: String,
    value: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FieldRow {
    region_id: String,
    value: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstraintRow {
    constraint_id: String,
    region_id: String,
    coefficient: f64,
}

pub(super) fn run(
    region_path: PathBuf,
    precision_path: PathBuf,
    field_path: PathBuf,
    constraint_path: PathBuf,
    constraint_tolerance: f64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let region_ids = read_regions(&region_path)?;
    let (precision, precision_entries) = read_precision(&precision_path, &region_ids)?;
    let field = read_field(&field_path, &region_ids)?;
    let constraints = read_constraints(&constraint_path, &region_ids)?;
    let result = gmrf_log_density(GmrfSpec {
        dimension: region_ids.len(),
        precision,
        field,
        constraints,
        constraint_tolerance,
    })
    .map_err(map_error)?;
    publish_json(
        &output_path,
        &GmrfOutput::new(
            region_path,
            precision_path,
            field_path,
            constraint_path,
            precision_entries,
            constraint_tolerance,
            result,
        ),
    )
}

fn read_regions(path: &std::path::Path) -> Result<Vec<String>, BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader.headers()?.iter().eq(["region_id"]) {
        return Err(BayesCliError::Input(
            "GMRF region headers must be exactly: region_id".into(),
        ));
    }
    let mut region_ids = reader
        .deserialize::<RegionRow>()
        .map(|row| row.map(|row| row.region_id).map_err(BayesCliError::from))
        .collect::<Result<Vec<_>, _>>()?;
    region_ids.sort();
    if region_ids.len() < 2
        || region_ids.len() > 256
        || region_ids.iter().enumerate().any(|(index, region_id)| {
            region_id.is_empty()
                || region_id.trim() != region_id
                || (index > 0 && region_ids[index - 1] == *region_id)
        })
    {
        return Err(BayesCliError::Input(
            "GMRF requires 2-256 exact unique region IDs".into(),
        ));
    }
    Ok(region_ids)
}

fn region_index(region_ids: &[String]) -> BTreeMap<&str, usize> {
    region_ids
        .iter()
        .enumerate()
        .map(|(index, region_id)| (region_id.as_str(), index))
        .collect()
}

fn read_precision(
    path: &std::path::Path,
    region_ids: &[String],
) -> Result<(Vec<f64>, usize), BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["source_region", "target_region", "value"])
    {
        return Err(BayesCliError::Input(
            "GMRF precision headers must be exactly: source_region,target_region,value".into(),
        ));
    }
    let indices = region_index(region_ids);
    let dimension = region_ids.len();
    let mut precision = vec![0.0; dimension * dimension];
    let mut stored = BTreeSet::new();
    let mut diagonal = vec![false; dimension];
    for row in reader.deserialize::<PrecisionRow>() {
        let row = row?;
        let source = *indices.get(row.source_region.as_str()).ok_or_else(|| {
            BayesCliError::Input(format!(
                "unknown precision source region {}",
                row.source_region
            ))
        })?;
        let target = *indices.get(row.target_region.as_str()).ok_or_else(|| {
            BayesCliError::Input(format!(
                "unknown precision target region {}",
                row.target_region
            ))
        })?;
        if !row.value.is_finite() || !stored.insert((source, target)) {
            return Err(BayesCliError::Input(
                "GMRF precision entries must be unique and finite".into(),
            ));
        }
        if stored.len() > 65_536 {
            return Err(BayesCliError::Input(
                "GMRF precision accepts at most 65536 stored entries".into(),
            ));
        }
        precision[source * dimension + target] = row.value;
        if source == target {
            diagonal[source] = true;
        }
    }
    if diagonal.iter().any(|present| !present) {
        return Err(BayesCliError::Input(
            "GMRF precision requires every diagonal entry explicitly".into(),
        ));
    }
    Ok((precision, stored.len()))
}

fn read_field(path: &std::path::Path, region_ids: &[String]) -> Result<Vec<f64>, BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader.headers()?.iter().eq(["region_id", "value"]) {
        return Err(BayesCliError::Input(
            "GMRF field headers must be exactly: region_id,value".into(),
        ));
    }
    let mut values = BTreeMap::new();
    for row in reader.deserialize::<FieldRow>() {
        let row = row?;
        if !row.value.is_finite() || values.insert(row.region_id, row.value).is_some() {
            return Err(BayesCliError::Input(
                "GMRF field region IDs must be unique with finite values".into(),
            ));
        }
    }
    if values.len() != region_ids.len() {
        return Err(BayesCliError::Input(
            "GMRF field must contain exactly one row per region".into(),
        ));
    }
    region_ids
        .iter()
        .map(|region_id| {
            values.remove(region_id).ok_or_else(|| {
                BayesCliError::Input(format!("missing GMRF field region {region_id}"))
            })
        })
        .collect()
}

fn read_constraints(
    path: &std::path::Path,
    region_ids: &[String],
) -> Result<Vec<GmrfConstraint>, BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["constraint_id", "region_id", "coefficient"])
    {
        return Err(BayesCliError::Input(
            "GMRF constraint headers must be exactly: constraint_id,region_id,coefficient".into(),
        ));
    }
    let indices = region_index(region_ids);
    let mut rows = BTreeMap::<String, Vec<f64>>::new();
    let mut stored = BTreeSet::new();
    for row in reader.deserialize::<ConstraintRow>() {
        let row = row?;
        let index = *indices.get(row.region_id.as_str()).ok_or_else(|| {
            BayesCliError::Input(format!("unknown constraint region {}", row.region_id))
        })?;
        if row.constraint_id.is_empty()
            || row.constraint_id.trim() != row.constraint_id
            || !row.coefficient.is_finite()
            || !stored.insert((row.constraint_id.clone(), index))
        {
            return Err(BayesCliError::Input(
                "GMRF constraint entries require exact names, unique regions, and finite coefficients"
                    .into(),
            ));
        }
        let coefficients = rows
            .entry(row.constraint_id)
            .or_insert_with(|| vec![0.0; region_ids.len()]);
        coefficients[index] = row.coefficient;
        if rows.len() >= region_ids.len() || rows.len() > 255 {
            return Err(BayesCliError::Input(
                "GMRF constraint count must be below dimension and at most 255".into(),
            ));
        }
    }
    Ok(rows
        .into_iter()
        .map(|(name, coefficients)| GmrfConstraint { name, coefficients })
        .collect())
}

fn validate_file(path: &std::path::Path) -> Result<(), BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "GMRF input must be a regular file within the 16 MiB limit".into(),
        ));
    }
    Ok(())
}

fn map_error(error: GmrfDensityError) -> BayesCliError {
    match error {
        GmrfDensityError::InvalidInput(message) => BayesCliError::Input(message),
        GmrfDensityError::Numerical(message) => BayesCliError::Backend(message),
    }
}

#[derive(Debug, Serialize)]
struct GmrfOutput {
    format: &'static str,
    version: u32,
    regions_path: PathBuf,
    precision_path: PathBuf,
    field_path: PathBuf,
    constraints_path: PathBuf,
    dimension: usize,
    precision_entries: usize,
    constrained_dimension: usize,
    rank_deficiency: usize,
    constraint_tolerance: f64,
    constraints: Vec<String>,
    log_determinant: f64,
    quadratic: f64,
    log_density: f64,
    claim_status: &'static str,
}

impl GmrfOutput {
    #[allow(clippy::too_many_arguments)]
    fn new(
        regions_path: PathBuf,
        precision_path: PathBuf,
        field_path: PathBuf,
        constraints_path: PathBuf,
        precision_entries: usize,
        constraint_tolerance: f64,
        result: GmrfDensityResult,
    ) -> Self {
        Self {
            format: "marklab.gmrf_density",
            version: 1,
            regions_path,
            precision_path,
            field_path,
            constraints_path,
            dimension: result.constrained_dimension + result.rank_deficiency,
            precision_entries,
            constrained_dimension: result.constrained_dimension,
            rank_deficiency: result.rank_deficiency,
            constraint_tolerance,
            constraints: result.constraint_names,
            log_determinant: result.log_determinant,
            quadratic: result.quadratic,
            log_density: result.log_density,
            claim_status: "experimental_field_density_diagnostic",
        }
    }
}
