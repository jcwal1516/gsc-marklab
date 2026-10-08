use super::*;

pub(crate) fn fixture() -> Fixture {
    fixture_with_options(FixtureOptions::default())
}

pub(crate) fn fixture_with_options(options: FixtureOptions) -> Fixture {
    let root = TempDir::new().expect("temporary store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("graph-local").expect("local store ID"),
    )
    .expect("local store");
    let (hierarchy, slide) = hierarchy(options.entity_count);
    let overlap_parquet_physical = options
        .overlap_parquet_physical
        .unwrap_or(options.parquet_physical);

    let checkpoint_candidate = draft_record_with_version(
        options.checkpoint_schema,
        options.checkpoint_schema_version,
        "application/octet-stream",
        b"checkpoint",
        Vec::new(),
        None,
        BTreeMap::new(),
    );
    let checkpoint = store
        .publish(&checkpoint_candidate, |writer| {
            writer.write_all(b"checkpoint")
        })
        .expect("publish checkpoint")
        .into_record();
    let source_snapshot = publish_record(
        &store,
        "marklab.source_snapshot",
        "application/octet-stream",
        b"source-snapshot",
        Vec::new(),
        None,
    );
    let license_candidate = draft_record(
        "marklab.license_record",
        "text/plain",
        b"license",
        Vec::new(),
        None,
        BTreeMap::new(),
    );
    let license = if options.license_catalog_only {
        license_candidate
    } else {
        store
            .publish(&license_candidate, |writer| writer.write_all(b"license"))
            .expect("publish license")
            .into_record()
    };
    let preprocessing = publish_record(
        &store,
        "marklab.embedding_preprocessing",
        "application/json",
        b"preprocessing",
        if options.dependency_drift == Some(DependencyShape::Leaf) {
            vec![checkpoint.id()]
        } else {
            Vec::new()
        },
        None,
    );
    let run_config = publish_record(
        &store,
        "marklab.embedding_run_config",
        "application/json",
        b"run-config",
        Vec::new(),
        None,
    );
    let environment = publish_record(
        &store,
        "marklab.execution_environment",
        "application/json",
        b"environment",
        Vec::new(),
        None,
    );
    let converter = publish_record(
        &store,
        "marklab.converter_manifest",
        "application/json",
        b"converter",
        Vec::new(),
        None,
    );

    let decimal = |value| PatchNormalizationDecimal::new(value).expect("normalization decimal");
    let input_normalization = PatchEmbeddingInputNormalization::he_srgb(
        [decimal("0.485"), decimal("0.456"), decimal("0.406")],
        [decimal("0.229"), decimal("0.224"), decimal("0.225")],
        BUDGET,
    )
    .expect("input normalization");
    let mut normalization_bytes = input_normalization
        .to_canonical_json()
        .expect("normalization JSON");
    drift_canonical_payload(
        &mut normalization_bytes,
        options.canonical_payload_drift,
        CanonicalPayloadRole::InputNormalization,
    );
    let normalization_record = publish_record(
        &store,
        "marklab.patch_embedding_input_normalization",
        "application/vnd.marklab.patch-embedding-input-normalization.v1+json",
        &normalization_bytes,
        Vec::new(),
        None,
    );

    let source_entities = PatchSourceEntitySet::new(
        "synthetic_patch_profile.v1",
        (0..options.entity_count)
            .map(|index| {
                PatchSourceEntityEntry::new(
                    source_at(index),
                    u64::try_from(index).expect("source row"),
                )
                .expect("source entity")
            })
            .collect(),
        BUDGET,
    )
    .expect("source entities");
    let source_entity_bytes = source_entities
        .to_canonical_json()
        .expect("source entity JSON");
    let mut published_source_entity_bytes = source_entity_bytes.clone();
    match options.source_entity_payload {
        SourceEntityPayload::Exact => {}
        SourceEntityPayload::SameLengthDrift => {
            let index = published_source_entity_bytes
                .iter()
                .position(|byte| *byte == b's')
                .expect("source JSON contains ASCII text");
            published_source_entity_bytes[index] = b't';
        }
        SourceEntityPayload::Truncated => {
            published_source_entity_bytes.pop();
        }
        SourceEntityPayload::Suffixed => published_source_entity_bytes.push(b' '),
    }
    drift_canonical_payload(
        &mut published_source_entity_bytes,
        options.canonical_payload_drift,
        CanonicalPayloadRole::SourceEntities,
    );
    let source_entity_table = options
        .source_entity_table_manifest
        .then(|| footprint_manifest(2, false, None));
    let source_entity_metadata = if options.source_entity_semantic_metadata {
        BTreeMap::from([(
            "privacy-sentinel-key".to_owned(),
            "private-source-a".to_owned(),
        )])
    } else {
        BTreeMap::new()
    };
    let source_entity_candidate = draft_record(
        "marklab.patch_source_entity_set",
        options.source_entity_kind,
        &published_source_entity_bytes,
        Vec::new(),
        source_entity_table,
        source_entity_metadata,
    );
    let source_entity_record = if options.source_entity_catalog_only {
        source_entity_candidate
    } else {
        store
            .publish(&source_entity_candidate, |writer| {
                writer.write_all(&published_source_entity_bytes)
            })
            .expect("publish source entities")
            .into_record()
    };
    let source_vectors = publish_record(
        &store,
        "marklab.patch_embedding_source_vectors",
        "application/vnd.marklab.patch-source-vectors.v1+binary",
        b"opaque-source-vectors",
        Vec::new(),
        None,
    );

    let expected_patches = ExpectedPatchSet::new(
        &hierarchy,
        slide.clone(),
        "all_patches.v1",
        (0..options.entity_count).map(patch_at).collect(),
        BUDGET,
    )
    .expect("expected patches");
    let mut expected_bytes = expected_patches
        .to_canonical_json()
        .expect("expected patch JSON");
    drift_canonical_payload(
        &mut expected_bytes,
        options.canonical_payload_drift,
        CanonicalPayloadRole::ExpectedPatches,
    );
    let expected_record = publish_record(
        &store,
        "marklab.expected_patch_set",
        "application/vnd.marklab.expected-patch-set.v1+json",
        &expected_bytes,
        Vec::new(),
        None,
    );

    let context = context(&hierarchy, slide.clone(), options.entity_count);
    let mut context_bytes = context.to_canonical_json().expect("context JSON");
    drift_canonical_payload(
        &mut context_bytes,
        options.canonical_payload_drift,
        CanonicalPayloadRole::PatchContext,
    );
    let context_record = publish_record(
        &store,
        "marklab.patch_embedding_context",
        "application/vnd.marklab.patch-embedding-context.v1+json",
        &context_bytes,
        Vec::new(),
        None,
    );

    let identity_map = PatchIdentityMap::new(
        &source_entities,
        source_entity_record.id(),
        &expected_patches,
        expected_record.id(),
        (0..options.entity_count)
            .map(|index| {
                PatchIdentityMapEntry::new(source_at(index), patch_at(index))
                    .expect("identity entry")
            })
            .collect(),
        BUDGET,
    )
    .expect("identity map");
    let mut identity_bytes = identity_map.to_canonical_json().expect("identity JSON");
    drift_canonical_payload(
        &mut identity_bytes,
        options.canonical_payload_drift,
        CanonicalPayloadRole::IdentityMap,
    );
    let mut identity_dependencies = identity_map.direct_dependencies().to_vec();
    if options.dependency_drift == Some(DependencyShape::IdentityMap) {
        identity_dependencies.push(checkpoint.id());
    }
    let identity_record = publish_record(
        &store,
        "marklab.patch_identity_map",
        "application/vnd.marklab.patch-identity-map.v1+json",
        &identity_bytes,
        identity_dependencies,
        None,
    );

    let mut source_vector_row = 0_u64;
    let source_row_entries = (0..options.entity_count)
        .map(|index| {
            let entity_row = u64::try_from(index).expect("source row");
            let status = match options.source_row_status_pattern {
                SourceRowStatusPattern::AllPresent => EmbeddingStatus::Present,
                #[cfg(feature = "parquet")]
                SourceRowStatusPattern::AllStatuses => match index % 4 {
                    0 => EmbeddingStatus::Present,
                    1 => EmbeddingStatus::MissingVector,
                    2 => EmbeddingStatus::ExtractionFailed,
                    _ => EmbeddingStatus::QcRejected,
                },
            };
            let vector_row = matches!(
                status,
                EmbeddingStatus::Present | EmbeddingStatus::QcRejected
            )
            .then(|| {
                let row = source_vector_row;
                source_vector_row += 1;
                row
            });
            PatchEmbeddingSourceRowLinkEntry::new(patch_at(index), status, entity_row, vector_row)
                .expect("source row-link entry")
        })
        .collect();
    let source_row_link = PatchEmbeddingSourceRowLink::new(
        &source_entities,
        source_entity_record.id(),
        source_vectors.id(),
        &expected_patches,
        expected_record.id(),
        &identity_map,
        identity_record.id(),
        converter.id(),
        source_row_entries,
        BUDGET,
    )
    .expect("source row link");
    let mut row_link_bytes = source_row_link
        .to_canonical_json()
        .expect("source row-link JSON");
    drift_canonical_payload(
        &mut row_link_bytes,
        options.canonical_payload_drift,
        CanonicalPayloadRole::SourceRowLink,
    );
    let mut row_link_dependencies = source_row_link.direct_dependencies().to_vec();
    if options.dependency_drift == Some(DependencyShape::SourceRowLink) {
        row_link_dependencies.push(checkpoint.id());
    }
    let row_link_record = publish_record(
        &store,
        "marklab.patch_embedding_source_row_link",
        "application/vnd.marklab.patch-embedding-source-row-link.v1+json",
        &row_link_bytes,
        row_link_dependencies,
        None,
    );

    let footprints = PatchFootprintSet::new(
        &hierarchy,
        &expected_patches,
        expected_record.id(),
        &context,
        context_record.id(),
        (0..options.entity_count)
            .map(|index| {
                let origin = i64::try_from(index).expect("footprint index") * 192;
                PatchFootprint::new(patch_at(index), [origin, 0])
            })
            .collect(),
        BUDGET,
    )
    .expect("patch footprints");
    let mut footprint_dependencies = vec![expected_record.id(), context_record.id()];
    if options.dependency_drift == Some(DependencyShape::PatchFootprints) {
        footprint_dependencies.push(checkpoint.id());
    }
    let footprint_mutation = physical_mutation(&options, PhysicalArtifactRole::PatchFootprints);
    let footprint_record = if options.canonical_physical {
        assert!(footprint_mutation.is_none());
        assert_eq!(footprint_dependencies.len(), 2);
        canonical_footprint_record(
            &store,
            &expected_patches,
            &context,
            &footprints,
            options.parquet_physical,
        )
    } else {
        publish_record(
            &store,
            "marklab.patch_footprint_table",
            if footprint_mutation == Some(PhysicalManifestMutation::ContentKind) {
                "application/octet-stream"
            } else if options.parquet_physical {
                "application/vnd.marklab.patch-footprint-table.v1+parquet"
            } else {
                "application/vnd.marklab.patch-footprint-table.v1+arrow"
            },
            b"structural-only-footprint-bytes",
            footprint_dependencies,
            if footprint_mutation == Some(PhysicalManifestMutation::MissingManifest) {
                None
            } else {
                Some(footprint_manifest(
                    u64::try_from(footprints.row_count()).expect("footprint count"),
                    options.parquet_physical,
                    footprint_mutation,
                ))
            },
        )
    };

    let overlap = PatchOverlapGraph::derive(
        &expected_patches,
        &context,
        &footprints,
        footprint_record.id(),
        BUDGET,
        BUDGET,
    )
    .expect("overlap graph");
    let mut overlap_dependencies = vec![
        expected_record.id(),
        context_record.id(),
        footprint_record.id(),
    ];
    if options.dependency_drift == Some(DependencyShape::PatchOverlapGraph) {
        overlap_dependencies.push(checkpoint.id());
    }
    let overlap_mutation = physical_mutation(&options, PhysicalArtifactRole::PatchOverlapGraph);
    let overlap_record = if options.canonical_physical {
        assert!(overlap_mutation.is_none());
        assert_eq!(overlap_dependencies.len(), 3);
        canonical_overlap_record(
            &store,
            &expected_patches,
            &context,
            &footprints,
            &overlap,
            overlap_parquet_physical,
        )
    } else {
        publish_record(
            &store,
            "marklab.patch_overlap_edge_table",
            if overlap_mutation == Some(PhysicalManifestMutation::ContentKind) {
                "application/octet-stream"
            } else if overlap_parquet_physical {
                "application/vnd.marklab.patch-overlap-edge-table.v1+parquet"
            } else {
                "application/vnd.marklab.patch-overlap-edge-table.v1+arrow"
            },
            b"structural-only-overlap-bytes",
            overlap_dependencies,
            if overlap_mutation == Some(PhysicalManifestMutation::MissingManifest) {
                None
            } else {
                Some(overlap_manifest(
                    u64::try_from(overlap.edge_count()).expect("overlap count"),
                    overlap_parquet_physical,
                    overlap_mutation,
                ))
            },
        )
    };

    let support = MultiscaleEmbeddingSupport::patch(
        slide.clone(),
        MultiscaleArtifactBinding::new(context_record.id(), context.logical_digest()),
        MultiscaleArtifactBinding::new(footprint_record.id(), footprints.logical_digest()),
        MultiscaleArtifactBinding::new(overlap_record.id(), overlap.logical_digest()),
        BUDGET,
    )
    .expect("patch support");
    let mut support_bytes = support.to_canonical_json().expect("support JSON");
    drift_canonical_payload(
        &mut support_bytes,
        options.canonical_payload_drift,
        CanonicalPayloadRole::PatchSupport,
    );
    let mut support_dependencies: Vec<_> = support.direct_dependencies().collect();
    if options.dependency_drift == Some(DependencyShape::PatchSupport) {
        support_dependencies.push(checkpoint.id());
    }
    let support_record = publish_record(
        &store,
        "marklab.multiscale_embedding_support",
        "application/vnd.marklab.multiscale-embedding-support.v1+json",
        &support_bytes,
        support_dependencies,
        None,
    );

    let model = MultiscaleDirectPatchModelProvenance::new(
        "patch_encoder",
        "1.0.0",
        "vit_h",
        checkpoint.id(),
        if options.checkpoint_digest_drift {
            ContentDigest::from_bytes(b"different-checkpoint")
        } else {
            checkpoint.content().digest()
        },
        source_snapshot.id(),
        license.id(),
        "Apache-2.0",
        "doi:10.1000/graph-fixture",
        "encoder.layer_32",
        32,
        BUDGET,
    )
    .expect("model provenance");
    let execution = MultiscaleEmbeddingExecutionProvenance::new(
        run_config.id(),
        environment.id(),
        converter.id(),
        "marklab_patch_converter",
        "1.0.0",
        BUDGET,
    )
    .expect("execution provenance");
    let inputs = MultiscaleDirectPatchInputArtifacts::new(
        normalization_record.id(),
        source_entity_record.id(),
        source_vectors.id(),
        expected_record.id(),
        identity_record.id(),
        row_link_record.id(),
        support_record.id(),
    );
    let provenance = MultiscaleEmbeddingProvenance::direct_patch(
        slide,
        &support,
        options.output_dimension,
        "mean_patch_tokens",
        model,
        execution,
        preprocessing.id(),
        inputs,
        BUDGET,
    )
    .expect("direct patch provenance");
    let mut provenance_bytes = provenance.to_canonical_json().expect("provenance JSON");
    drift_canonical_payload(
        &mut provenance_bytes,
        options.canonical_payload_drift,
        CanonicalPayloadRole::Provenance,
    );
    let mut provenance_dependencies: Vec<_> = provenance.direct_dependencies().collect();
    if options.dependency_drift == Some(DependencyShape::Provenance) {
        provenance_dependencies.push(context_record.id());
    }
    let provenance_record = publish_record(
        &store,
        "marklab.multiscale_embedding_provenance",
        "application/vnd.marklab.multiscale-embedding-provenance.v1+json",
        &provenance_bytes,
        provenance_dependencies,
        None,
    );
    let provenance_artifact_id = provenance_record.id();

    let mut records = vec![
        checkpoint,
        source_snapshot,
        license,
        normalization_record,
        preprocessing,
        run_config,
        environment,
        converter,
        source_entity_record,
        source_vectors,
        expected_record,
        identity_record,
        row_link_record,
        context_record,
        footprint_record.clone(),
        overlap_record.clone(),
        support_record,
        provenance_record,
    ];
    records.reverse();
    let catalog = ArtifactCatalog::from_records(records).expect("acyclic artifact catalog");
    Fixture {
        _root: root,
        store,
        catalog,
        provenance,
        provenance_artifact_id,
        expected_patches,
        source_entities,
        identity_map,
        source_row_link,
        input_normalization,
        context,
        footprints,
        #[cfg(feature = "parquet")]
        footprint_record,
        overlap,
        #[cfg(feature = "parquet")]
        overlap_record,
        support,
    }
}

#[cfg(feature = "parquet")]
fn physical_budgets() -> marklab::EmbeddingColumnarBudgets {
    marklab::EmbeddingColumnarBudgets::new(
        64 * 1024 * 1024,
        64 * 1024 * 1024,
        64 * 1024 * 1024,
        64 * 1024 * 1024,
    )
}

#[cfg(feature = "parquet")]
fn canonical_footprint_record(
    store: &LocalArtifactStore,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    parquet: bool,
) -> ArtifactRecord {
    if parquet {
        marklab::publish_patch_footprint_set_parquet(
            store,
            expected,
            context,
            footprints,
            physical_budgets(),
        )
        .expect("publish canonical footprint Parquet")
        .into_record()
    } else {
        marklab::publish_patch_footprint_set_arrow(
            store,
            expected,
            context,
            footprints,
            physical_budgets(),
        )
        .expect("publish canonical footprint Arrow")
        .into_record()
    }
}

#[cfg(not(feature = "parquet"))]
fn canonical_footprint_record(
    _store: &LocalArtifactStore,
    _expected: &ExpectedPatchSet,
    _context: &PatchEmbeddingContext,
    _footprints: &PatchFootprintSet,
    _parquet: bool,
) -> ArtifactRecord {
    panic!("canonical physical fixture requires the parquet feature")
}

#[cfg(feature = "parquet")]
fn canonical_overlap_record(
    store: &LocalArtifactStore,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
    parquet: bool,
) -> ArtifactRecord {
    if parquet {
        marklab::publish_patch_overlap_graph_parquet(
            store,
            expected,
            context,
            footprints,
            overlap,
            physical_budgets(),
        )
        .expect("publish canonical overlap Parquet")
        .into_record()
    } else {
        marklab::publish_patch_overlap_graph_arrow(
            store,
            expected,
            context,
            footprints,
            overlap,
            physical_budgets(),
        )
        .expect("publish canonical overlap Arrow")
        .into_record()
    }
}

#[cfg(not(feature = "parquet"))]
fn canonical_overlap_record(
    _store: &LocalArtifactStore,
    _expected: &ExpectedPatchSet,
    _context: &PatchEmbeddingContext,
    _footprints: &PatchFootprintSet,
    _overlap: &PatchOverlapGraph,
    _parquet: bool,
) -> ArtifactRecord {
    panic!("canonical physical fixture requires the parquet feature")
}
