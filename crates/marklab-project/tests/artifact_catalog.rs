use std::{collections::BTreeMap, str::FromStr};

use marklab_project::{
    ArtifactCatalog, ArtifactCatalogError, ArtifactKey, ArtifactLocator, ArtifactRecord,
    ArtifactRecordError, ArtifactRef, ArtifactSchema, ContentDigest, StoreId, TableColumn,
    TableColumnType, TableFormat, TableManifest, TableManifestError, TableScalarType,
};
use serde_json::Value;

fn content(bytes: &[u8]) -> ArtifactRef {
    ArtifactRef::from_bytes("application/vnd.marklab.table", bytes).expect("content")
}

fn schema(version: u32) -> ArtifactSchema {
    ArtifactSchema::new("marklab.test.cells", version).expect("schema")
}

fn locator(store: &str, key: &str) -> ArtifactLocator {
    ArtifactLocator::new(
        StoreId::new(store).expect("store ID"),
        ArtifactKey::new(key).expect("artifact key"),
        None,
    )
    .expect("locator")
}

fn table(row_count: u64, value_type: TableColumnType) -> TableManifest {
    TableManifest::new(
        TableFormat::ArrowIpcFile,
        "arrow-ipc-1",
        row_count,
        vec![
            TableColumn::new(
                "cell_id",
                TableColumnType::Scalar(TableScalarType::Utf8),
                false,
            )
            .expect("ID column"),
            TableColumn::new("value", value_type, true).expect("value column"),
        ],
        vec!["cell_id".to_owned()],
    )
    .expect("table")
}

fn record(
    bytes: &[u8],
    schema_version: u32,
    table_manifest: Option<TableManifest>,
    dependencies: Vec<marklab_project::ArtifactId>,
    metadata: BTreeMap<String, String>,
    locations: Vec<ArtifactLocator>,
) -> ArtifactRecord {
    ArtifactRecord::new(
        content(bytes),
        schema(schema_version),
        table_manifest,
        dependencies,
        metadata,
        locations,
    )
    .expect("record")
}

#[test]
fn content_digest_text_and_schema_version_are_strict() {
    let digest = ContentDigest::from_bytes(b"abc");
    assert_eq!(
        ContentDigest::from_str(&digest.to_string()).expect("canonical digest"),
        digest
    );
    assert!(ContentDigest::from_str(&digest.to_string().to_uppercase()).is_err());
    assert!(ContentDigest::from_str("00").is_err());
    assert!(ArtifactSchema::new("marklab.test", 0).is_err());
    assert!(ArtifactSchema::new("marklab/test", 1).is_err());
}

#[test]
fn artifact_identity_covers_schema_table_dependency_and_metadata_but_not_location() {
    let base_table = table(1, TableColumnType::Scalar(TableScalarType::F64));
    let base = record(
        b"same bytes",
        1,
        Some(base_table.clone()),
        Vec::new(),
        BTreeMap::from([("model".to_owned(), "checkpoint-1".to_owned())]),
        vec![locator("local", "tables/cells.arrow")],
    );

    let relocated = record(
        b"same bytes",
        1,
        Some(base_table.clone()),
        Vec::new(),
        BTreeMap::from([("model".to_owned(), "checkpoint-1".to_owned())]),
        vec![locator("replica", "copies/cells.arrow")],
    );
    assert_eq!(base.id(), relocated.id());
    assert_eq!(base.semantic_digest(), relocated.semantic_digest());

    let schema_changed = record(
        b"same bytes",
        2,
        Some(base_table.clone()),
        Vec::new(),
        BTreeMap::from([("model".to_owned(), "checkpoint-1".to_owned())]),
        vec![locator("local", "tables/cells.arrow")],
    );
    assert_ne!(base.id(), schema_changed.id());

    let table_changed = record(
        b"same bytes",
        1,
        Some(table(2, TableColumnType::Scalar(TableScalarType::F64))),
        Vec::new(),
        BTreeMap::from([("model".to_owned(), "checkpoint-1".to_owned())]),
        vec![locator("local", "tables/cells.arrow")],
    );
    assert_ne!(base.id(), table_changed.id());

    let dependency = record(
        b"dependency",
        1,
        None,
        Vec::new(),
        BTreeMap::new(),
        vec![locator("local", "inputs/dependency")],
    );
    let dependency_changed = record(
        b"same bytes",
        1,
        Some(base_table.clone()),
        vec![dependency.id()],
        BTreeMap::from([("model".to_owned(), "checkpoint-1".to_owned())]),
        vec![locator("local", "tables/cells.arrow")],
    );
    assert_ne!(base.id(), dependency_changed.id());

    let metadata_changed = record(
        b"same bytes",
        1,
        Some(base_table),
        Vec::new(),
        BTreeMap::from([("model".to_owned(), "checkpoint-2".to_owned())]),
        vec![locator("local", "tables/cells.arrow")],
    );
    assert_ne!(base.id(), metadata_changed.id());
}

#[test]
fn table_manifest_rejects_unstable_keys_and_reports_exact_mismatch() {
    let columns = vec![
        TableColumn::new(
            "cell_id",
            TableColumnType::Scalar(TableScalarType::Utf8),
            false,
        )
        .expect("ID column"),
        TableColumn::new(
            "score",
            TableColumnType::Scalar(TableScalarType::F64),
            false,
        )
        .expect("score column"),
    ];
    assert!(matches!(
        TableManifest::new(
            TableFormat::ParquetFile,
            "parquet-2.6",
            1,
            columns.clone(),
            vec!["score".to_owned()],
        ),
        Err(TableManifestError::UnstablePrimaryKeyType { .. })
    ));

    let expected = TableManifest::new(
        TableFormat::ParquetFile,
        "parquet-2.6",
        1,
        columns,
        vec!["cell_id".to_owned()],
    )
    .expect("expected table");
    let observed = table(1, TableColumnType::Scalar(TableScalarType::F64));
    assert!(matches!(
        expected.require_exact(&observed),
        Err(TableManifestError::Mismatch { .. })
    ));

    assert!(matches!(
        TableColumn::new(
            "vector",
            TableColumnType::FixedSizeList {
                element: TableScalarType::F32,
                length: 0,
            },
            false,
        ),
        Err(TableManifestError::InvalidFixedListLength { .. })
    ));
}

#[test]
fn catalog_round_trip_is_canonical_and_replica_merge_preserves_identity() {
    let original = record(
        b"artifact",
        1,
        None,
        Vec::new(),
        BTreeMap::new(),
        vec![locator("local", "external/artifact")],
    );
    let replica = record(
        b"artifact",
        1,
        None,
        Vec::new(),
        BTreeMap::new(),
        vec![locator("archive", "replicas/artifact")],
    );
    assert_eq!(original.id(), replica.id());

    let mut catalog = ArtifactCatalog::new();
    catalog.register(original.clone()).expect("original");
    catalog.register(replica).expect("replica");
    assert_eq!(catalog.len(), 1);
    assert_eq!(
        catalog
            .get(original.id())
            .expect("merged")
            .locations()
            .len(),
        2
    );

    let encoded = catalog.to_canonical_json().expect("encode");
    assert!(encoded.ends_with(b"\n"));
    let decoded = ArtifactCatalog::from_canonical_json(&encoded).expect("decode");
    assert_eq!(decoded.to_canonical_json().expect("re-encode"), encoded);
    assert_eq!(
        decoded.digest().expect("digest"),
        catalog.digest().expect("digest")
    );
}

#[test]
fn catalog_registration_rejects_missing_dependencies_without_mutation() {
    let dependency = record(
        b"missing",
        1,
        None,
        Vec::new(),
        BTreeMap::new(),
        vec![locator("local", "external/missing")],
    );
    let dependent = record(
        b"dependent",
        1,
        None,
        vec![dependency.id()],
        BTreeMap::new(),
        vec![locator("local", "external/dependent")],
    );
    let mut catalog = ArtifactCatalog::new();
    assert!(matches!(
        catalog.register(dependent),
        Err(ArtifactCatalogError::MissingDependency { .. })
    ));
    assert!(catalog.is_empty());
}

#[test]
fn durable_v1_identity_and_catalog_encoding_match_golden_vectors() {
    let golden = ArtifactRecord::new(
        content(b"golden"),
        ArtifactSchema::new("marklab.test.golden", 1).expect("schema"),
        None,
        Vec::new(),
        BTreeMap::from([("source".to_owned(), "fixture-1".to_owned())]),
        vec![locator("local", "external/golden")],
    )
    .expect("golden record");
    assert_eq!(
        golden.id().to_string(),
        "bbc7639ccd4de261226b424506cd36466e66942349ab77d36daa579c22021e0f"
    );
    assert_eq!(
        golden.semantic_digest().to_string(),
        "77e63e78832cb1bc80457fb385426ff16bc87e2f018f7cbc57f4e16d5e923c2e"
    );
    let catalog = ArtifactCatalog::from_records([golden]).expect("golden catalog");
    assert_eq!(
        String::from_utf8(catalog.to_canonical_json().expect("canonical JSON"))
            .expect("UTF-8 JSON"),
        concat!(
            "{\"format\":\"marklab.artifact_catalog\",\"version\":1,\"artifacts\":[{",
            "\"id\":\"bbc7639ccd4de261226b424506cd36466e66942349ab77d36daa579c22021e0f\",",
            "\"semantic_digest\":\"77e63e78832cb1bc80457fb385426ff16bc87e2f018f7cbc57f4e16d5e923c2e\",",
            "\"content\":{\"kind\":\"application/vnd.marklab.table\",",
            "\"digest\":\"dd56de4137951d9c92681b03416ec15f886b4482a27e3a517d32f085244cbe5d\",",
            "\"byte_len\":6},\"schema\":{\"id\":\"marklab.test.golden\",\"version\":1},",
            "\"table\":null,\"dependencies\":[],",
            "\"semantic_metadata\":[{\"key\":\"source\",\"value\":\"fixture-1\"}],",
            "\"locations\":[{\"store_id\":\"local\",\"key\":\"external/golden\",",
            "\"object_version\":null}]}]}\n"
        )
    );
    assert_eq!(
        catalog.digest().expect("catalog digest").to_string(),
        "f4814ecf0c62f07322c13fa1e702c9edacda735df85dc78492bd4137aee31248"
    );
}

#[test]
fn locators_and_metadata_reject_reserved_or_privacy_unsafe_values() {
    for invalid in [
        "../escape",
        "/absolute",
        "C:/drive",
        "folder\\leaf",
        "folder//leaf",
        "./leaf",
        "folder/../leaf",
        "control/\u{0}",
    ] {
        assert!(
            ArtifactKey::new(invalid).is_err(),
            "key should be rejected: {invalid:?}"
        );
    }
    assert!(StoreId::new("../store").is_err());
    for reserved in [
        ".marklab-staging/escape",
        ".marklab-quarantine/evidence",
        ".marklab-store.lock",
        "objects/sha256/00/object",
    ] {
        assert!(ArtifactLocator::new(
            StoreId::new("local").expect("store"),
            ArtifactKey::new(reserved).expect("lexical key"),
            None,
        )
        .is_err());
    }
    assert!(ArtifactLocator::new(
        StoreId::new("local").expect("store"),
        ArtifactKey::new("external/value").expect("key"),
        Some("token?secret=true".to_owned()),
    )
    .is_err());

    assert!(ArtifactRecord::new(
        content(b"secret path"),
        schema(1),
        None,
        Vec::new(),
        BTreeMap::from([("source".to_owned(), "/patients/example".to_owned())]),
        vec![locator("local", "external/value")],
    )
    .is_err());
}

#[test]
fn replica_merge_is_canonical_and_same_store_conflict_is_atomic() {
    let first = record(
        b"replicated",
        1,
        None,
        Vec::new(),
        BTreeMap::new(),
        vec![
            locator("z_archive", "replicas/z"),
            locator("a_local", "replicas/a"),
        ],
    );
    assert_eq!(first.locations()[0].store_id().as_str(), "a_local");
    let same_store_conflict = record(
        b"replicated",
        1,
        None,
        Vec::new(),
        BTreeMap::new(),
        vec![locator("a_local", "replicas/different")],
    );
    let mut catalog = ArtifactCatalog::new();
    catalog.register(first.clone()).expect("first");
    let before = catalog.to_canonical_json().expect("before");
    assert!(matches!(
        catalog.register(same_store_conflict),
        Err(ArtifactCatalogError::InvalidRecord(
            ArtifactRecordError::ConflictingStoreLocator { .. }
        ))
    ));
    assert_eq!(catalog.to_canonical_json().expect("after"), before);
}

#[test]
fn strict_catalog_decode_rejects_unknown_versions_tampering_and_noncanonical_bytes() {
    let item = record(
        b"strict",
        1,
        None,
        Vec::new(),
        BTreeMap::from([("source".to_owned(), "instrument-1".to_owned())]),
        vec![locator("local", "external/strict")],
    );
    let catalog = ArtifactCatalog::from_records([item]).expect("catalog");
    let canonical = catalog.to_canonical_json().expect("canonical");
    let base: Value = serde_json::from_slice(&canonical).expect("JSON value");

    let mut duplicate = base.clone();
    let duplicated_record = duplicate["artifacts"][0].clone();
    duplicate["artifacts"]
        .as_array_mut()
        .expect("artifact array")
        .push(duplicated_record);
    assert!(matches!(
        ArtifactCatalog::from_json(&serde_json::to_vec(&duplicate).expect("duplicate JSON")),
        Err(ArtifactCatalogError::DuplicateArtifact { .. })
    ));

    let mut unknown = base.clone();
    unknown
        .as_object_mut()
        .expect("catalog object")
        .insert("unknown".to_owned(), Value::Bool(true));
    assert!(matches!(
        ArtifactCatalog::from_json(&serde_json::to_vec(&unknown).expect("unknown JSON")),
        Err(ArtifactCatalogError::Json { .. })
    ));

    for version in [0_u64, 2] {
        let mut unsupported = base.clone();
        unsupported["version"] = Value::from(version);
        assert!(matches!(
            ArtifactCatalog::from_json(
                &serde_json::to_vec(&unsupported).expect("unsupported JSON")
            ),
            Err(ArtifactCatalogError::UnsupportedCatalogVersion { observed })
                if observed == version as u32
        ));
    }

    let pretty = serde_json::to_vec_pretty(&base).expect("pretty JSON");
    assert!(ArtifactCatalog::from_json(&pretty).is_ok());
    assert!(matches!(
        ArtifactCatalog::from_canonical_json(&pretty),
        Err(ArtifactCatalogError::NonCanonicalCatalog)
    ));

    let mut semantic_tamper = base.clone();
    semantic_tamper["artifacts"][0]["semantic_digest"] = Value::from("0".repeat(64));
    assert!(matches!(
        ArtifactCatalog::from_json(
            &serde_json::to_vec(&semantic_tamper).expect("semantic tamper JSON")
        ),
        Err(ArtifactCatalogError::SemanticDigestMismatch { .. })
    ));

    let mut id_tamper = base;
    id_tamper["artifacts"][0]["id"] = Value::from("0".repeat(64));
    assert!(matches!(
        ArtifactCatalog::from_json(&serde_json::to_vec(&id_tamper).expect("ID tamper JSON")),
        Err(ArtifactCatalogError::ArtifactIdMismatch { .. })
    ));
}

#[test]
fn hostile_self_dependency_precedes_forged_identity_failure() {
    let item = record(
        b"self",
        1,
        None,
        Vec::new(),
        BTreeMap::new(),
        vec![locator("local", "external/self")],
    );
    let id = item.id();
    let catalog = ArtifactCatalog::from_records([item]).expect("catalog");
    let mut value: Value =
        serde_json::from_slice(&catalog.to_canonical_json().expect("canonical")).expect("JSON");
    value["artifacts"][0]["dependencies"] = Value::Array(vec![Value::from(id.to_string())]);
    assert!(matches!(
        ArtifactCatalog::from_json(&serde_json::to_vec(&value).expect("hostile JSON")),
        Err(ArtifactCatalogError::SelfDependency { artifact }) if artifact == id
    ));
}

#[test]
fn ten_thousand_record_chain_round_trips_and_tail_cycle_is_iterative() {
    const RECORDS: usize = 10_000;
    let mut records = Vec::with_capacity(RECORDS);
    let mut insertion_order = Vec::with_capacity(RECORDS);
    let mut dependency = None;
    for index in 0..RECORDS {
        let next = record(
            format!("record-{index}").as_bytes(),
            1,
            None,
            dependency.into_iter().collect(),
            BTreeMap::new(),
            vec![locator("local", &format!("external/record-{index}"))],
        );
        dependency = Some(next.id());
        insertion_order.push(next.id());
        records.push(next);
    }
    records.reverse();
    let catalog = ArtifactCatalog::from_records(records).expect("forward-reference batch");
    let canonical = catalog.to_canonical_json().expect("canonical chain");
    assert!(canonical.len() < 16 * 1024 * 1024);
    assert_eq!(
        ArtifactCatalog::from_canonical_json(&canonical)
            .expect("chain round trip")
            .len(),
        RECORDS
    );

    let root = insertion_order[0];
    let tail = insertion_order[RECORDS - 1];
    let mut hostile: Value = serde_json::from_slice(&canonical).expect("chain JSON");
    let artifacts = hostile["artifacts"].as_array_mut().expect("artifact array");
    let root_text = root.to_string();
    let root_wire = artifacts
        .iter_mut()
        .find(|artifact| artifact["id"].as_str() == Some(root_text.as_str()))
        .expect("root wire record");
    root_wire["dependencies"] = Value::Array(vec![Value::from(tail.to_string())]);

    let mut expected_cycle = insertion_order;
    expected_cycle.sort_unstable();
    assert!(matches!(
        ArtifactCatalog::from_json(&serde_json::to_vec(&hostile).expect("cycle JSON")),
        Err(ArtifactCatalogError::Cycle { artifacts }) if artifacts == expected_cycle
    ));
}

#[test]
fn oversized_catalog_is_rejected_before_json_parsing() {
    let oversized = vec![b' '; 16 * 1024 * 1024 + 1];
    assert!(matches!(
        ArtifactCatalog::from_json(&oversized),
        Err(ArtifactCatalogError::CatalogTooLarge { .. })
    ));
}

#[test]
fn oversized_catalog_is_rejected_during_canonical_encoding() {
    let metadata = (0..256)
        .map(|index| (format!("key{index:03}"), "x".repeat(4_096)))
        .collect::<BTreeMap<_, _>>();
    let records = (0..17)
        .map(|index| {
            record(
                format!("large-record-{index}").as_bytes(),
                1,
                None,
                Vec::new(),
                metadata.clone(),
                vec![locator("local", &format!("external/large-{index}"))],
            )
        })
        .collect::<Vec<_>>();
    let catalog = ArtifactCatalog::from_records(records).expect("large in-memory catalog");
    assert!(matches!(
        catalog.to_canonical_json(),
        Err(ArtifactCatalogError::CatalogTooLarge { .. })
    ));
}
