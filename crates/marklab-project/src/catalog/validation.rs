use super::{codec::DecodedRecord, *};

pub(super) fn validate_record_dependencies(
    artifacts: &BTreeMap<ArtifactId, ArtifactRecord>,
    record: &ArtifactRecord,
) -> Result<(), ArtifactCatalogError> {
    for dependency in record.dependencies() {
        if *dependency == record.id() {
            return Err(ArtifactCatalogError::SelfDependency {
                artifact: record.id(),
            });
        }
        if !artifacts.contains_key(dependency) {
            return Err(ArtifactCatalogError::MissingDependency {
                artifact: record.id(),
                dependency: *dependency,
            });
        }
    }
    Ok(())
}

pub(super) fn validate_graph(
    artifacts: &BTreeMap<ArtifactId, ArtifactRecord>,
) -> Result<(), ArtifactCatalogError> {
    for record in artifacts.values() {
        validate_record_dependencies(artifacts, record)?;
    }
    validate_acyclic(
        artifacts
            .iter()
            .map(|(id, record)| (*id, record.dependencies())),
    )
}

pub(super) fn validate_decoded_graph(
    records: &BTreeMap<ArtifactId, DecodedRecord>,
) -> Result<(), ArtifactCatalogError> {
    for (id, decoded) in records {
        for dependency in decoded.record.dependencies() {
            if dependency == id {
                return Err(ArtifactCatalogError::SelfDependency { artifact: *id });
            }
            if !records.contains_key(dependency) {
                return Err(ArtifactCatalogError::MissingDependency {
                    artifact: *id,
                    dependency: *dependency,
                });
            }
        }
    }
    validate_acyclic(
        records
            .iter()
            .map(|(id, record)| (*id, record.record.dependencies())),
    )
}

fn validate_acyclic<'a>(
    records: impl IntoIterator<Item = (ArtifactId, &'a [ArtifactId])>,
) -> Result<(), ArtifactCatalogError> {
    let records = records.into_iter().collect::<BTreeMap<_, _>>();
    let mut remaining = records
        .iter()
        .map(|(id, dependencies)| (*id, dependencies.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<ArtifactId, Vec<ArtifactId>>::new();
    for (id, dependencies) in &records {
        for dependency in *dependencies {
            dependents.entry(*dependency).or_default().push(*id);
        }
    }
    let mut ready = remaining
        .iter()
        .filter_map(|(id, count)| (*count == 0).then_some(*id))
        .collect::<BTreeSet<_>>();
    let mut visited = 0;
    while let Some(id) = ready.pop_first() {
        visited += 1;
        if let Some(children) = dependents.get(&id) {
            for child in children {
                let count = remaining
                    .get_mut(child)
                    .expect("validated dependent must exist");
                *count -= 1;
                if *count == 0 {
                    ready.insert(*child);
                }
            }
        }
    }
    if visited == records.len() {
        Ok(())
    } else {
        Err(ArtifactCatalogError::Cycle {
            artifacts: remaining
                .into_iter()
                .filter_map(|(id, count)| (count > 0).then_some(id))
                .collect(),
        })
    }
}
