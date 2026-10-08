use super::*;

#[test]
fn region_support_and_derived_graph_close_the_exact_dependency_chain() {
    let fixture = derived_fixture();
    assert_eq!(
        fixture.region_support.logical_digest(),
        fixture.region_support_logical_digest
    );
    let graph = validate_graph(&fixture);
    assert_eq!(
        graph.provenance_artifact_id(),
        fixture.provenance_artifact_id
    );
    assert_eq!(graph.dependency_count(), 8);
    assert_eq!(graph.output_dimension(), 3);
    let debug = format!("{graph:?}");
    assert!(!debug.contains(&fixture.provenance_artifact_id.to_string()));
    assert!(!debug.contains("private-derived-region"));
}
#[test]
fn derived_graph_accepts_every_patch_table_and_link_format_combination() {
    for (patch_format, link_format) in [
        (PhysicalFormat::Arrow, PhysicalFormat::Arrow),
        (PhysicalFormat::Arrow, PhysicalFormat::Parquet),
        (PhysicalFormat::Parquet, PhysicalFormat::Arrow),
        (PhysicalFormat::Parquet, PhysicalFormat::Parquet),
    ] {
        let fixture = derived_fixture_with_options(DerivedFixtureOptions {
            patch_format,
            link_format,
            ..DerivedFixtureOptions::default()
        });
        validate_graph(&fixture);
    }
}
#[test]
fn derived_graph_accepts_every_source_patch_status() {
    let fixture = derived_fixture_with_options(DerivedFixtureOptions {
        entity_count: 4,
        source_row_status_pattern: SourceRowStatusPattern::AllStatuses,
        ..DerivedFixtureOptions::default()
    });
    let graph = validate_graph(&fixture);
    assert_eq!(graph.output_dimension(), 3);
}

#[test]
fn derived_graph_accepts_empty_and_batch_boundary_source_tables() {
    for (entity_count, patch_format, link_format) in [
        (0, PhysicalFormat::Arrow, PhysicalFormat::Parquet),
        (8_193, PhysicalFormat::Parquet, PhysicalFormat::Arrow),
    ] {
        let fixture = derived_fixture_with_options(DerivedFixtureOptions {
            patch_format,
            link_format,
            entity_count,
            ..DerivedFixtureOptions::default()
        });
        validate_graph(&fixture);
    }
}
