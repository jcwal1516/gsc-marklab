use super::*;

pub(crate) fn publish_record(
    store: &LocalArtifactStore,
    schema: &str,
    kind: &str,
    bytes: &[u8],
    dependencies: Vec<ArtifactId>,
    table: Option<TableManifest>,
) -> ArtifactRecord {
    publish_record_with_metadata(
        store,
        schema,
        kind,
        bytes,
        dependencies,
        table,
        BTreeMap::new(),
    )
}

pub(crate) fn draft_record(
    schema: &str,
    kind: &str,
    bytes: &[u8],
    dependencies: Vec<ArtifactId>,
    table: Option<TableManifest>,
    semantic_metadata: BTreeMap<String, String>,
) -> ArtifactRecord {
    draft_record_with_version(
        schema,
        1,
        kind,
        bytes,
        dependencies,
        table,
        semantic_metadata,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draft_record_with_version(
    schema: &str,
    schema_version: u32,
    kind: &str,
    bytes: &[u8],
    dependencies: Vec<ArtifactId>,
    table: Option<TableManifest>,
    semantic_metadata: BTreeMap<String, String>,
) -> ArtifactRecord {
    ArtifactRecord::new(
        ArtifactRef::from_bytes(kind, bytes).expect("artifact content"),
        ArtifactSchema::new(schema, schema_version).expect("artifact schema"),
        table,
        dependencies,
        semantic_metadata,
        vec![ArtifactLocator::new(
            StoreId::new("source").expect("source store"),
            ArtifactKey::new(format!("fixtures/{schema}")).expect("source key"),
            None,
        )
        .expect("source locator")],
    )
    .expect("candidate record")
}

#[allow(clippy::too_many_arguments)]
fn publish_record_with_metadata(
    store: &LocalArtifactStore,
    schema: &str,
    kind: &str,
    bytes: &[u8],
    dependencies: Vec<ArtifactId>,
    table: Option<TableManifest>,
    semantic_metadata: BTreeMap<String, String>,
) -> ArtifactRecord {
    let candidate = draft_record(schema, kind, bytes, dependencies, table, semantic_metadata);
    store
        .publish(&candidate, |writer| writer.write_all(bytes))
        .expect("publish fixture record")
        .into_record()
}
