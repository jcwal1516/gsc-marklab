use super::*;
use marklab::{
    finalize_slide_embedding_table_from_patches, finalize_slide_embedding_table_from_regions,
    publish_region_embedding_table_arrow, publish_region_embedding_table_parquet,
    verify_region_embedding_table_arrow_from_store,
    verify_region_embedding_table_parquet_from_store, DerivedRegionEmbeddingTableCandidate,
    DerivedSlideEmbeddingTableCandidate, ExpectedSlideSet, RegionEmbeddingTable, SlideId,
    VerifiedDerivedSlideEmbeddingArtifactGraph, VerifiedRegionEmbeddingTableArtifact,
    VerifiedSlideEmbeddingSupportArtifact,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SlideSourceLevel {
    Patches,
    Regions,
}

#[derive(Clone, Copy)]
struct SlideFixtureOptions {
    source_level: SlideSourceLevel,
    lower_format: PhysicalFormat,
    entity_count: usize,
    region_count: usize,
    output_dimension: u32,
    source_vector_pattern: SourceVectorPattern,
    source_row_status_pattern: SourceRowStatusPattern,
}

impl Default for SlideFixtureOptions {
    fn default() -> Self {
        Self {
            source_level: SlideSourceLevel::Patches,
            lower_format: PhysicalFormat::Arrow,
            entity_count: 2,
            region_count: 2,
            output_dimension: 3,
            source_vector_pattern: SourceVectorPattern::Sequential,
            source_row_status_pattern: SourceRowStatusPattern::AllPresent,
        }
    }
}

enum SlideLowerTable {
    Patches,
    Regions {
        candidate: Box<DerivedRegionEmbeddingTableCandidate>,
        receipt: Box<VerifiedRegionEmbeddingTableArtifact>,
        record: Box<ArtifactRecord>,
    },
}

struct SlideFixture {
    lower: DerivedFixture,
    source_level: SlideSourceLevel,
    lower_table: SlideLowerTable,
    catalog: ArtifactCatalog,
    expected_slides: ExpectedSlideSet,
    expected_slide_record: ArtifactRecord,
    support_value: MultiscaleEmbeddingSupport,
    support_record: ArtifactRecord,
    support: VerifiedSlideEmbeddingSupportArtifact,
    derivation: MultiscaleEmbeddingDerivationContract,
    derivation_record: ArtifactRecord,
    provenance: MultiscaleEmbeddingProvenance,
    provenance_record: ArtifactRecord,
}

fn finalization_budgets() -> EmbeddingFinalizationBudgets {
    EmbeddingFinalizationBudgets::new(BUDGET, BUDGET, BUDGET as u64, BUDGET as u64)
}

fn slide_fixture(source_level: SlideSourceLevel) -> SlideFixture {
    slide_fixture_with_options(SlideFixtureOptions {
        source_level,
        ..SlideFixtureOptions::default()
    })
}

fn expected_slides_with_rule(slide_id: &SlideId, selection_rule: &str) -> ExpectedSlideSet {
    let patient = HierarchyId::from(PatientId::new("derived-slide-patient").expect("patient"));
    let hierarchy = CohortHierarchy::new(
        vec![
            HierarchyNode::new(patient.clone(), None, ReplicationRole::BiologicalUnit),
            HierarchyNode::new(
                HierarchyId::from(slide_id.clone()),
                None,
                ReplicationRole::TechnicalReplicate {
                    biological_source: patient,
                },
            ),
        ],
        Vec::new(),
    )
    .expect("slide hierarchy");
    ExpectedSlideSet::new(
        &hierarchy,
        slide_id.clone(),
        selection_rule,
        vec![slide_id.clone()],
        BUDGET,
    )
    .expect("expected slide")
}

fn slide_fixture_with_options(options: SlideFixtureOptions) -> SlideFixture {
    let lower = derived_fixture_with_options(DerivedFixtureOptions {
        patch_format: options.lower_format,
        link_format: options.lower_format,
        direct_parquet_physical: matches!(options.lower_format, PhysicalFormat::Parquet),
        entity_count: options.entity_count,
        region_count: options.region_count,
        output_dimension: options.output_dimension,
        source_vector_pattern: options.source_vector_pattern,
        source_row_status_pattern: options.source_row_status_pattern,
    });
    let slide_id = lower.expected_regions.owning_slide_id().clone();
    let expected_slides = expected_slides_with_rule(&slide_id, "derived_slide.v1");
    let expected_slide_bytes = expected_slides
        .to_canonical_json()
        .expect("expected-slide bytes");
    let expected_slide_record = publish_record(
        &lower._direct.store,
        "marklab.expected_slide_set",
        "application/vnd.marklab.expected-slide-set.v1+json",
        &expected_slide_bytes,
        Vec::new(),
        None,
    );

    let mut catalog = lower.catalog.clone();
    catalog
        .register(expected_slide_record.clone())
        .expect("register expected slide");
    let lower_table = match options.source_level {
        SlideSourceLevel::Patches => SlideLowerTable::Patches,
        SlideSourceLevel::Regions => {
            let candidate = finalize_region_embedding_table_from_patches(
                &lower.expected_regions,
                &lower.source_patch_table_value,
                &lower.link,
                validate_graph(&lower),
                finalization_budgets(),
            )
            .expect("source region candidate");
            let record = match options.lower_format {
                PhysicalFormat::Arrow => publish_region_embedding_table_arrow(
                    &lower._direct.store,
                    candidate.table(),
                    budgets(),
                )
                .expect("publish source region Arrow")
                .into_record(),
                PhysicalFormat::Parquet => publish_region_embedding_table_parquet(
                    &lower._direct.store,
                    candidate.table(),
                    budgets(),
                )
                .expect("publish source region Parquet")
                .into_record(),
            };
            let receipt = match options.lower_format {
                PhysicalFormat::Arrow => verify_region_embedding_table_arrow_from_store(
                    &lower._direct.store,
                    &record,
                    &candidate,
                    budgets(),
                ),
                PhysicalFormat::Parquet => verify_region_embedding_table_parquet_from_store(
                    &lower._direct.store,
                    &record,
                    &candidate,
                    budgets(),
                ),
            }
            .expect("source region receipt");
            catalog
                .register(record.clone())
                .expect("register source region table");
            SlideLowerTable::Regions {
                candidate: Box::new(candidate),
                receipt: Box::new(receipt),
                record: Box::new(record),
            }
        }
    };

    let support_value = match &lower_table {
        SlideLowerTable::Patches => MultiscaleEmbeddingSupport::slide_from_patches(
            slide_id.clone(),
            MultiscaleArtifactBinding::new(
                lower.patch_support.artifact_id(),
                lower.patch_support.logical_digest(),
            ),
            MultiscaleArtifactBinding::new(
                lower.source_patch_table_record.id(),
                lower.source_patch_table_value.logical_digest(),
            ),
            BUDGET,
        ),
        SlideLowerTable::Regions {
            candidate, record, ..
        } => MultiscaleEmbeddingSupport::slide_from_regions(
            slide_id.clone(),
            MultiscaleArtifactBinding::new(
                lower.region_support.artifact_id(),
                lower.region_support.logical_digest(),
            ),
            MultiscaleArtifactBinding::new(record.id(), candidate.logical_digest()),
            BUDGET,
        ),
    }
    .expect("slide support");
    let support_bytes = support_value
        .to_canonical_json()
        .expect("slide-support bytes");
    let support_record = publish_record(
        &lower._direct.store,
        "marklab.multiscale_embedding_support",
        "application/vnd.marklab.multiscale-embedding-support.v1+json",
        &support_bytes,
        support_value.direct_dependencies().collect(),
        None,
    );
    catalog
        .register(support_record.clone())
        .expect("register slide support");
    let support = match &lower_table {
        SlideLowerTable::Patches => support_value.verify_slide_from_patches_artifact(
            support_record.id(),
            &lower.source_patch_table_value,
            lower.patch_support,
            lower.source_patch_table,
            &catalog,
            &lower._direct.store,
        ),
        SlideLowerTable::Regions {
            candidate, receipt, ..
        } => support_value.verify_slide_from_regions_artifact(
            support_record.id(),
            candidate.table(),
            lower.region_support,
            **receipt,
            &catalog,
            &lower._direct.store,
        ),
    }
    .expect("slide-support receipt");

    let derivation =
        MultiscaleEmbeddingDerivationContract::arithmetic_mean("fixed_order_f64.v1", BUDGET)
            .expect("slide derivation");
    let derivation_bytes = derivation.to_canonical_json().expect("derivation bytes");
    let derivation_record = publish_record(
        &lower._direct.store,
        "marklab.multiscale_embedding_derivation",
        "application/vnd.marklab.multiscale-embedding-derivation.v1+json",
        &derivation_bytes,
        Vec::new(),
        None,
    );
    let run_config_record = publish_record(
        &lower._direct.store,
        "marklab.embedding_run_config",
        "application/json",
        b"derived-slide-run",
        Vec::new(),
        None,
    );
    let environment_record = publish_record(
        &lower._direct.store,
        "marklab.execution_environment",
        "application/json",
        b"derived-slide-environment",
        Vec::new(),
        None,
    );
    let converter_record = publish_record(
        &lower._direct.store,
        "marklab.converter_manifest",
        "application/json",
        b"derived-slide-converter",
        Vec::new(),
        None,
    );
    let execution = MultiscaleEmbeddingExecutionProvenance::new(
        run_config_record.id(),
        environment_record.id(),
        converter_record.id(),
        "marklab_slide_deriver",
        "1.0.0",
        BUDGET,
    )
    .expect("slide execution");
    let provenance = match &lower_table {
        SlideLowerTable::Patches => MultiscaleEmbeddingProvenance::derived_slide_from_patches(
            slide_id,
            &support_value,
            &derivation,
            options.output_dimension,
            execution,
            lower.source_patch_table_record.id(),
            expected_slide_record.id(),
            support_record.id(),
            derivation_record.id(),
            BUDGET,
        ),
        SlideLowerTable::Regions { record, .. } => {
            MultiscaleEmbeddingProvenance::derived_slide_from_regions(
                slide_id,
                &support_value,
                &derivation,
                options.output_dimension,
                execution,
                record.id(),
                expected_slide_record.id(),
                support_record.id(),
                derivation_record.id(),
                BUDGET,
            )
        }
    }
    .expect("slide provenance");
    let provenance_bytes = provenance.to_canonical_json().expect("provenance bytes");
    let provenance_record = publish_record(
        &lower._direct.store,
        "marklab.multiscale_embedding_provenance",
        "application/vnd.marklab.multiscale-embedding-provenance.v1+json",
        &provenance_bytes,
        provenance.direct_dependencies().collect(),
        None,
    );
    for record in [
        derivation_record.clone(),
        run_config_record.clone(),
        environment_record.clone(),
        converter_record.clone(),
        provenance_record.clone(),
    ] {
        catalog.register(record).expect("register slide role");
    }

    SlideFixture {
        lower,
        source_level: options.source_level,
        lower_table,
        catalog,
        expected_slides,
        expected_slide_record,
        support_value,
        support_record,
        support,
        derivation,
        derivation_record,
        provenance,
        provenance_record,
    }
}

fn source_region_candidate(fixture: &SlideFixture) -> &DerivedRegionEmbeddingTableCandidate {
    match &fixture.lower_table {
        SlideLowerTable::Regions { candidate, .. } => candidate,
        SlideLowerTable::Patches => panic!("region source required"),
    }
}

fn source_region_table(fixture: &SlideFixture) -> &RegionEmbeddingTable {
    source_region_candidate(fixture).table()
}

fn source_region_receipt(fixture: &SlideFixture) -> VerifiedRegionEmbeddingTableArtifact {
    match &fixture.lower_table {
        SlideLowerTable::Regions { receipt, .. } => **receipt,
        SlideLowerTable::Patches => panic!("region source required"),
    }
}

fn source_table_record(fixture: &SlideFixture) -> &ArtifactRecord {
    match &fixture.lower_table {
        SlideLowerTable::Patches => &fixture.lower.source_patch_table_record,
        SlideLowerTable::Regions { record, .. } => record,
    }
}

fn verify_slide_support_result(
    fixture: &SlideFixture,
    support: &MultiscaleEmbeddingSupport,
    support_artifact_id: ArtifactId,
    catalog: &ArtifactCatalog,
    store: &LocalArtifactStore,
) -> Result<VerifiedSlideEmbeddingSupportArtifact, MultiscaleEmbeddingArtifactGraphError> {
    match fixture.source_level {
        SlideSourceLevel::Patches => support.verify_slide_from_patches_artifact(
            support_artifact_id,
            &fixture.lower.source_patch_table_value,
            fixture.lower.patch_support,
            fixture.lower.source_patch_table,
            catalog,
            store,
        ),
        SlideSourceLevel::Regions => support.verify_slide_from_regions_artifact(
            support_artifact_id,
            source_region_table(fixture),
            fixture.lower.region_support,
            source_region_receipt(fixture),
            catalog,
            store,
        ),
    }
}

fn validate_slide_graph_result(
    fixture: &SlideFixture,
    provenance: &MultiscaleEmbeddingProvenance,
    provenance_artifact_id: ArtifactId,
    catalog: &ArtifactCatalog,
    store: &LocalArtifactStore,
) -> Result<VerifiedDerivedSlideEmbeddingArtifactGraph, MultiscaleEmbeddingArtifactGraphError> {
    match fixture.source_level {
        SlideSourceLevel::Patches => provenance.validate_derived_slide_from_patches_artifact_graph(
            provenance_artifact_id,
            &fixture.expected_slides,
            &fixture.derivation,
            fixture.support,
            catalog,
            store,
        ),
        SlideSourceLevel::Regions => provenance.validate_derived_slide_from_regions_artifact_graph(
            provenance_artifact_id,
            &fixture.expected_slides,
            &fixture.derivation,
            fixture.support,
            catalog,
            store,
        ),
    }
}

fn validate_slide_graph(fixture: &SlideFixture) -> VerifiedDerivedSlideEmbeddingArtifactGraph {
    validate_slide_graph_result(
        fixture,
        &fixture.provenance,
        fixture.provenance_record.id(),
        &fixture.catalog,
        &fixture.lower._direct.store,
    )
    .expect("derived-slide graph")
}

fn finalize_slide_with_budgets(
    fixture: &SlideFixture,
    graph: VerifiedDerivedSlideEmbeddingArtifactGraph,
    finalization_budgets: EmbeddingFinalizationBudgets,
) -> Result<DerivedSlideEmbeddingTableCandidate, MultiscaleEmbeddingError> {
    match fixture.source_level {
        SlideSourceLevel::Patches => finalize_slide_embedding_table_from_patches(
            &fixture.expected_slides,
            &fixture.lower.source_patch_table_value,
            graph,
            finalization_budgets,
        ),
        SlideSourceLevel::Regions => finalize_slide_embedding_table_from_regions(
            &fixture.expected_slides,
            source_region_table(fixture),
            graph,
            finalization_budgets,
        ),
    }
}

fn finalize_slide(fixture: &SlideFixture) -> DerivedSlideEmbeddingTableCandidate {
    finalize_slide_with_budgets(
        fixture,
        validate_slide_graph(fixture),
        finalization_budgets(),
    )
    .expect("derived-slide candidate")
}

#[path = "derived_slide/finalization.rs"]
mod finalization;
#[path = "derived_slide/graph.rs"]
mod graph;
#[path = "derived_slide/receipt.rs"]
mod receipt;
