use super::support::*;

#[test]
fn high_cardinality_source_chain_uses_the_streaming_graph_path() {
    let fixture = fixture_with_options(FixtureOptions {
        entity_count: 4_096,
        ..FixtureOptions::default()
    });
    assert_eq!(fixture.source_entities.entries().len(), 4_096);
    assert_eq!(fixture.identity_map.entries().len(), 4_096);
    assert_eq!(fixture.source_row_link.entries().len(), 4_096);

    let receipt = fixture
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
        .expect("high-cardinality structural graph");
    assert_eq!(receipt.provenance_dependency_count(), 14);
    assert_eq!(receipt.output_dimension(), 1_024);
}
