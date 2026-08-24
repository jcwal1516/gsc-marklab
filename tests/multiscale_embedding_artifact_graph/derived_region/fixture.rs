use super::super::support::*;
use marklab::{
    finalize_region_embedding_table_from_patches, publish_patch_embedding_table_arrow,
    publish_patch_embedding_table_parquet, publish_patch_region_link_arrow,
    publish_patch_region_link_parquet, verify_patch_embedding_table_arrow_from_store,
    verify_patch_embedding_table_parquet_from_store, verify_patch_footprint_set_arrow_from_store,
    verify_patch_footprint_set_parquet_from_store, verify_patch_overlap_graph_arrow_from_store,
    verify_patch_overlap_graph_parquet_from_store, verify_patch_region_link_arrow_from_store,
    verify_patch_region_link_parquet_from_store, ArtifactAvailabilityFailure, ArtifactCatalog,
    ArtifactRecord, EmbeddingColumnarBudgets, EmbeddingFinalizationBudgets, EmbeddingStatus,
    ExpectedRegionSet, HierarchyId, HierarchyNode, MultiscaleArtifactBinding,
    MultiscaleEmbeddingArtifactGraphError, MultiscaleEmbeddingArtifactRole,
    MultiscaleEmbeddingDerivationContract, MultiscaleEmbeddingError,
    MultiscaleEmbeddingExecutionProvenance, MultiscaleEmbeddingProvenance,
    MultiscaleEmbeddingSupport, PatchEmbeddingRow, PatchEmbeddingTable, PatchRegionAssessment,
    PatchRegionAssessmentBindings, PatchRegionDeclaration, PatchRegionLink, PatientId, RegionId,
    ReplicationRole, VerifiedDerivedRegionEmbeddingArtifactGraph,
    VerifiedPatchEmbeddingSupportArtifact, VerifiedPatchEmbeddingTableArtifact,
    VerifiedPatchRegionLinkArtifact, VerifiedRegionEmbeddingSupportArtifact,
};

fn budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(BUDGET as u64, BUDGET, BUDGET, BUDGET as u64)
}

fn direct_graph(fixture: &Fixture) -> marklab::VerifiedDirectPatchEmbeddingArtifactGraph {
    fixture
        .provenance
        .validate_direct_patch_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected_patches,
            &fixture.source_entities,
            &fixture.identity_map,
            &fixture.source_row_link,
            &fixture.input_normalization,
            &fixture.context,
            &fixture.footprints,
            &fixture.overlap,
            &fixture.support,
            &fixture.catalog,
            &fixture.store,
        )
        .expect("direct graph")
}

fn patch_support(
    fixture: &Fixture,
    graph: marklab::VerifiedDirectPatchEmbeddingArtifactGraph,
    parquet_physical: bool,
) -> VerifiedPatchEmbeddingSupportArtifact {
    let footprints = if parquet_physical {
        verify_patch_footprint_set_parquet_from_store(
            &fixture.store,
            &fixture.footprint_record,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            graph,
            budgets(),
        )
    } else {
        verify_patch_footprint_set_arrow_from_store(
            &fixture.store,
            &fixture.footprint_record,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            graph,
            budgets(),
        )
    }
    .expect("footprint receipt");
    let overlap = if parquet_physical {
        verify_patch_overlap_graph_parquet_from_store(
            &fixture.store,
            &fixture.overlap_record,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            &fixture.overlap,
            footprints,
            graph,
            budgets(),
        )
    } else {
        verify_patch_overlap_graph_arrow_from_store(
            &fixture.store,
            &fixture.overlap_record,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            &fixture.overlap,
            footprints,
            graph,
            budgets(),
        )
    }
    .expect("overlap receipt");
    VerifiedPatchEmbeddingSupportArtifact::from_verified_components(graph, footprints, overlap)
        .expect("patch support receipt")
}

fn patch_table(
    fixture: &Fixture,
    support: VerifiedPatchEmbeddingSupportArtifact,
    dimension: u32,
    vector_pattern: SourceVectorPattern,
) -> PatchEmbeddingTable {
    let rows = fixture
        .expected_patches
        .ids()
        .iter()
        .enumerate()
        .map(|(index, patch_id)| {
            let status = fixture.source_row_link.entries()[index].status();
            if status == EmbeddingStatus::Present {
                PatchEmbeddingRow::present(
                    patch_id.clone(),
                    (0..dimension)
                        .map(|column| match vector_pattern {
                            SourceVectorPattern::Sequential => index as f32 + column as f32 + 1.0,
                            SourceVectorPattern::CancellationSensitive => match index {
                                0 => f32::MAX,
                                1 => 1.0,
                                2 => -f32::MAX,
                                _ => 0.0,
                            },
                        })
                        .collect(),
                )
            } else {
                PatchEmbeddingRow::non_present(patch_id.clone(), status).expect("patch status")
            }
        })
        .collect();
    PatchEmbeddingTable::from_rows(
        dimension,
        &fixture.expected_patches,
        fixture.source_row_link.expected_patches_artifact_id(),
        support.artifact_id(),
        support.logical_digest(),
        fixture.provenance_artifact_id,
        fixture.provenance.logical_digest(),
        rows,
        BUDGET,
    )
    .expect("patch table")
}

#[derive(Clone, Copy)]
enum PhysicalFormat {
    Arrow,
    Parquet,
}

#[derive(Clone, Copy)]
enum SourceVectorPattern {
    Sequential,
    CancellationSensitive,
}

#[derive(Clone, Copy)]
struct DerivedFixtureOptions {
    patch_format: PhysicalFormat,
    link_format: PhysicalFormat,
    direct_parquet_physical: bool,
    entity_count: usize,
    region_count: usize,
    output_dimension: u32,
    source_vector_pattern: SourceVectorPattern,
    source_row_status_pattern: SourceRowStatusPattern,
}

impl Default for DerivedFixtureOptions {
    fn default() -> Self {
        Self {
            patch_format: PhysicalFormat::Arrow,
            link_format: PhysicalFormat::Arrow,
            direct_parquet_physical: false,
            entity_count: 2,
            region_count: 2,
            output_dimension: 3,
            source_vector_pattern: SourceVectorPattern::Sequential,
            source_row_status_pattern: SourceRowStatusPattern::AllPresent,
        }
    }
}

struct DerivedFixture {
    _direct: Fixture,
    catalog: ArtifactCatalog,
    expected_regions: ExpectedRegionSet,
    derivation: MultiscaleEmbeddingDerivationContract,
    provenance: MultiscaleEmbeddingProvenance,
    provenance_artifact_id: ArtifactId,
    provenance_record: ArtifactRecord,
    expected_region_record: ArtifactRecord,
    derivation_record: ArtifactRecord,
    run_config_record: ArtifactRecord,
    environment_record: ArtifactRecord,
    converter_record: ArtifactRecord,
    patch_support: VerifiedPatchEmbeddingSupportArtifact,
    source_patch_table: VerifiedPatchEmbeddingTableArtifact,
    source_patch_table_value: PatchEmbeddingTable,
    source_patch_table_record: ArtifactRecord,
    link: PatchRegionLink,
    link_receipt: VerifiedPatchRegionLinkArtifact,
    region_support_value: MultiscaleEmbeddingSupport,
    region_support: VerifiedRegionEmbeddingSupportArtifact,
    region_support_record: ArtifactRecord,
    region_support_logical_digest: ContentDigest,
}

fn derived_fixture() -> DerivedFixture {
    derived_fixture_with_options(DerivedFixtureOptions::default())
}

fn derived_fixture_with_options(options: DerivedFixtureOptions) -> DerivedFixture {
    let direct = fixture_with_options(FixtureOptions {
        canonical_physical: true,
        parquet_physical: options.direct_parquet_physical,
        output_dimension: options.output_dimension,
        entity_count: options.entity_count,
        source_row_status_pattern: options.source_row_status_pattern,
        ..FixtureOptions::default()
    });
    let graph = direct_graph(&direct);
    let patch_support = patch_support(&direct, graph, options.direct_parquet_physical);
    let patch_table = patch_table(
        &direct,
        patch_support,
        options.output_dimension,
        options.source_vector_pattern,
    );
    let patch_table_record = match options.patch_format {
        PhysicalFormat::Arrow => {
            publish_patch_embedding_table_arrow(&direct.store, &patch_table, budgets())
                .expect("publish Arrow patch table")
                .into_record()
        }
        PhysicalFormat::Parquet => {
            publish_patch_embedding_table_parquet(&direct.store, &patch_table, budgets())
                .expect("publish Parquet patch table")
                .into_record()
        }
    };
    let source_patch_table = match options.patch_format {
        PhysicalFormat::Arrow => verify_patch_embedding_table_arrow_from_store(
            &direct.store,
            &patch_table_record,
            &patch_table,
            &direct.source_row_link,
            patch_support,
            graph,
            budgets(),
        ),
        PhysicalFormat::Parquet => verify_patch_embedding_table_parquet_from_store(
            &direct.store,
            &patch_table_record,
            &patch_table,
            &direct.source_row_link,
            patch_support,
            graph,
            budgets(),
        ),
    }
    .expect("patch table receipt");

    let slide_id = direct.expected_patches.owning_slide_id().clone();
    let slide = HierarchyId::from(slide_id.clone());
    let patient = HierarchyId::from(PatientId::new("derived-region-patient").expect("patient"));
    let regions: Vec<_> = (0..options.region_count)
        .map(|index| {
            RegionId::new(format!("derived-region-{index:06}")).expect("derived region identity")
        })
        .collect();
    let mut nodes = vec![
        HierarchyNode::new(patient.clone(), None, ReplicationRole::BiologicalUnit),
        HierarchyNode::new(
            slide.clone(),
            None,
            ReplicationRole::TechnicalReplicate {
                biological_source: patient,
            },
        ),
    ];
    nodes.extend(direct.expected_patches.ids().iter().cloned().map(|patch| {
        HierarchyNode::new(
            HierarchyId::from(patch),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    nodes.extend(regions.iter().cloned().map(|region| {
        HierarchyNode::new(
            HierarchyId::from(region),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("derived hierarchy");
    let expected_regions = ExpectedRegionSet::new(
        &hierarchy,
        slide_id.clone(),
        "derived_regions.v1",
        regions.clone(),
        BUDGET,
    )
    .expect("expected regions");
    let expected_region_bytes = expected_regions
        .to_canonical_json()
        .expect("expected-region bytes");
    let expected_region_record = publish_record(
        &direct.store,
        "marklab.expected_region_set",
        "application/vnd.marklab.expected-region-set.v1+json",
        &expected_region_bytes,
        Vec::new(),
        None,
    );
    let converter_record = direct
        .catalog
        .get(direct.source_row_link.converter_artifact_id())
        .expect("converter record")
        .clone();
    let assessment_bindings = PatchRegionAssessmentBindings::new(
        expected_region_record.id(),
        direct.footprint_record.id(),
        converter_record.id(),
        converter_record.content().digest(),
    );
    let patches = direct.expected_patches.ids();
    let mut declarations = Vec::new();
    if let (Some(patch), Some(region)) = (patches.first(), regions.first()) {
        declarations.push(PatchRegionDeclaration::fully_contained(
            patch.clone(),
            region.clone(),
        ));
    }
    if let (Some(patch), Some(first_region)) = (patches.get(1), regions.first()) {
        declarations.push(
            PatchRegionDeclaration::partial_overlap(patch.clone(), first_region.clone(), 1, 2)
                .expect("partial relation"),
        );
        if let Some(second_region) = regions.get(1) {
            declarations.push(PatchRegionDeclaration::fully_contained(
                patch.clone(),
                second_region.clone(),
            ));
        }
    }
    if !regions.is_empty() {
        for (index, patch) in patches.iter().enumerate().skip(2) {
            declarations.push(PatchRegionDeclaration::fully_contained(
                patch.clone(),
                regions[index % regions.len()].clone(),
            ));
        }
    }
    let assessment = PatchRegionAssessment::new(
        &direct.expected_patches,
        &expected_regions,
        &direct.context,
        &direct.footprints,
        &assessment_bindings,
        declarations,
        BUDGET,
        BUDGET,
    )
    .expect("assessment");
    let assessment_bytes = assessment.to_canonical_json().expect("assessment bytes");
    let assessment_record = publish_record(
        &direct.store,
        "marklab.patch_region_assessment",
        "application/vnd.marklab.patch-region-assessment.v1+json",
        &assessment_bytes,
        assessment.direct_dependencies().collect(),
        None,
    );
    let link = marklab::PatchRegionLink::from_exhaustive_assessment(
        &assessment,
        assessment_record.id(),
        assessment_record.content().digest(),
        BUDGET,
        BUDGET,
    )
    .expect("patch-region link");
    let link_record = match options.link_format {
        PhysicalFormat::Arrow => publish_patch_region_link_arrow(&direct.store, &link, budgets())
            .expect("publish Arrow patch-region link")
            .into_record(),
        PhysicalFormat::Parquet => {
            publish_patch_region_link_parquet(&direct.store, &link, budgets())
                .expect("publish Parquet patch-region link")
                .into_record()
        }
    };

    let mut catalog = direct.catalog.clone();
    catalog
        .register(patch_table_record.clone())
        .expect("register patch table");
    catalog
        .register(expected_region_record.clone())
        .expect("register expected regions");
    catalog
        .register(assessment_record.clone())
        .expect("register assessment");
    catalog
        .register(link_record.clone())
        .expect("register link");
    let link_graph = assessment
        .validate_patch_region_input_artifact_graph(
            assessment_record.id(),
            &direct.expected_patches,
            &expected_regions,
            &direct.context,
            &direct.footprints,
            &link,
            &catalog,
            &direct.store,
            budgets(),
        )
        .expect("patch-region graph");
    let link_receipt = match options.link_format {
        PhysicalFormat::Arrow => verify_patch_region_link_arrow_from_store(
            &direct.store,
            &link_record,
            &link,
            link_graph,
            budgets(),
        ),
        PhysicalFormat::Parquet => verify_patch_region_link_parquet_from_store(
            &direct.store,
            &link_record,
            &link,
            link_graph,
            budgets(),
        ),
    }
    .expect("patch-region receipt");

    let region_support_value = MultiscaleEmbeddingSupport::region_from_patches(
        slide_id.clone(),
        MultiscaleArtifactBinding::new(patch_support.artifact_id(), patch_support.logical_digest()),
        MultiscaleArtifactBinding::new(link_record.id(), link.logical_digest()),
        BUDGET,
    )
    .expect("region support");
    let region_support_bytes = region_support_value
        .to_canonical_json()
        .expect("region-support bytes");
    let region_support_record = publish_record(
        &direct.store,
        "marklab.multiscale_embedding_support",
        "application/vnd.marklab.multiscale-embedding-support.v1+json",
        &region_support_bytes,
        region_support_value.direct_dependencies().collect(),
        None,
    );
    catalog
        .register(region_support_record.clone())
        .expect("register region support");
    let region_support = region_support_value
        .verify_region_from_patches_artifact(
            region_support_record.id(),
            patch_support,
            &link,
            link_receipt,
            &catalog,
            &direct.store,
        )
        .expect("region-support receipt");
    let region_support_logical_digest = region_support_value.logical_digest();

    let derivation =
        MultiscaleEmbeddingDerivationContract::weighted_mean("fixed_order_f64.v1", BUDGET)
            .expect("derivation");
    let derivation_bytes = derivation.to_canonical_json().expect("derivation bytes");
    let derivation_record = publish_record(
        &direct.store,
        "marklab.multiscale_embedding_derivation",
        "application/vnd.marklab.multiscale-embedding-derivation.v1+json",
        &derivation_bytes,
        Vec::new(),
        None,
    );
    let run_config = publish_record(
        &direct.store,
        "marklab.embedding_run_config",
        "application/json",
        b"derived-region-run",
        Vec::new(),
        None,
    );
    let environment = publish_record(
        &direct.store,
        "marklab.execution_environment",
        "application/json",
        b"derived-region-environment",
        Vec::new(),
        None,
    );
    let converter = publish_record(
        &direct.store,
        "marklab.converter_manifest",
        "application/json",
        b"derived-region-converter",
        Vec::new(),
        None,
    );
    let execution = MultiscaleEmbeddingExecutionProvenance::new(
        run_config.id(),
        environment.id(),
        converter.id(),
        "marklab_region_deriver",
        "1.0.0",
        BUDGET,
    )
    .expect("derived execution");
    let provenance = MultiscaleEmbeddingProvenance::derived_region(
        slide_id,
        &region_support_value,
        &derivation,
        options.output_dimension,
        execution,
        patch_table_record.id(),
        link_record.id(),
        expected_region_record.id(),
        region_support_record.id(),
        derivation_record.id(),
        BUDGET,
    )
    .expect("derived provenance");
    let provenance_bytes = provenance.to_canonical_json().expect("provenance bytes");
    let provenance_record = publish_record(
        &direct.store,
        "marklab.multiscale_embedding_provenance",
        "application/vnd.marklab.multiscale-embedding-provenance.v1+json",
        &provenance_bytes,
        provenance.direct_dependencies().collect(),
        None,
    );
    for record in [
        derivation_record.clone(),
        run_config.clone(),
        environment.clone(),
        converter.clone(),
        provenance_record.clone(),
    ] {
        catalog.register(record).expect("register derived role");
    }
    DerivedFixture {
        _direct: direct,
        catalog,
        expected_regions,
        derivation,
        provenance,
        provenance_artifact_id: provenance_record.id(),
        provenance_record,
        expected_region_record,
        derivation_record,
        run_config_record: run_config,
        environment_record: environment,
        converter_record: converter,
        patch_support,
        source_patch_table,
        source_patch_table_value: patch_table,
        source_patch_table_record: patch_table_record,
        link,
        link_receipt,
        region_support_value,
        region_support,
        region_support_record,
        region_support_logical_digest,
    }
}

fn validate_graph(fixture: &DerivedFixture) -> VerifiedDerivedRegionEmbeddingArtifactGraph {
    validate_graph_result(
        fixture,
        &fixture.provenance,
        fixture.provenance_artifact_id,
        &fixture.derivation,
        &fixture.catalog,
        &fixture._direct.store,
    )
    .expect("derived-region graph")
}

fn validate_graph_result(
    fixture: &DerivedFixture,
    provenance: &MultiscaleEmbeddingProvenance,
    provenance_artifact_id: ArtifactId,
    derivation: &MultiscaleEmbeddingDerivationContract,
    catalog: &ArtifactCatalog,
    store: &LocalArtifactStore,
) -> Result<VerifiedDerivedRegionEmbeddingArtifactGraph, MultiscaleEmbeddingArtifactGraphError> {
    provenance.validate_derived_region_artifact_graph(
        provenance_artifact_id,
        &fixture.expected_regions,
        derivation,
        fixture.source_patch_table,
        fixture.region_support,
        catalog,
        store,
    )
}

fn validate_region_support_result(
    fixture: &DerivedFixture,
    support: &MultiscaleEmbeddingSupport,
    support_artifact_id: ArtifactId,
    catalog: &ArtifactCatalog,
    store: &LocalArtifactStore,
) -> Result<VerifiedRegionEmbeddingSupportArtifact, MultiscaleEmbeddingArtifactGraphError> {
    support.verify_region_from_patches_artifact(
        support_artifact_id,
        fixture.patch_support,
        &fixture.link,
        fixture.link_receipt,
        catalog,
        store,
    )
}

fn catalog_with(catalog: &ArtifactCatalog, record: ArtifactRecord) -> ArtifactCatalog {
    ArtifactCatalog::from_records(
        catalog
            .iter()
            .map(|(_id, existing)| existing.clone())
            .chain(std::iter::once(record)),
    )
    .expect("extended catalog")
}

fn corrupt_managed(root: &TempDir, artifact_id: ArtifactId) -> String {
    let artifact_id = artifact_id.to_string();
    let managed_path = root
        .path()
        .join("objects/sha256")
        .join(&artifact_id[..2])
        .join(&artifact_id);
    std::fs::write(&managed_path, b"private-derived-region-corruption")
        .expect("corrupt temporary managed fixture");
    artifact_id
}

fn rebuild_provenance(
    fixture: &DerivedFixture,
    run_config_artifact_id: ArtifactId,
    environment_artifact_id: ArtifactId,
    converter_artifact_id: ArtifactId,
    derivation_artifact_id: ArtifactId,
) -> (MultiscaleEmbeddingProvenance, ArtifactRecord) {
    let execution = MultiscaleEmbeddingExecutionProvenance::new(
        run_config_artifact_id,
        environment_artifact_id,
        converter_artifact_id,
        "marklab_region_deriver",
        "1.0.0",
        BUDGET,
    )
    .expect("rebuilt execution");
    let provenance = MultiscaleEmbeddingProvenance::derived_region(
        fixture.expected_regions.owning_slide_id().clone(),
        &fixture.region_support_value,
        &fixture.derivation,
        fixture.source_patch_table_value.dimension(),
        execution,
        fixture.source_patch_table_record.id(),
        fixture.link_receipt.artifact_id(),
        fixture.expected_region_record.id(),
        fixture.region_support_record.id(),
        derivation_artifact_id,
        BUDGET,
    )
    .expect("rebuilt provenance");
    let bytes = provenance.to_canonical_json().expect("provenance bytes");
    let record = publish_record(
        &fixture._direct.store,
        "marklab.multiscale_embedding_provenance",
        "application/vnd.marklab.multiscale-embedding-provenance.v1+json",
        &bytes,
        provenance.direct_dependencies().collect(),
        None,
    );
    (provenance, record)
}

#[path = "fixture/derived_graph.rs"]
mod derived_graph;
#[path = "fixture/finalization.rs"]
mod finalization;
#[path = "fixture/happy.rs"]
mod happy;
#[path = "fixture/region_receipt.rs"]
mod region_receipt;
#[path = "fixture/support_receipt.rs"]
mod support_receipt;
