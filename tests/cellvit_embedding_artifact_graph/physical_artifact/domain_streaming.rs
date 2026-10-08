use super::*;

#[test]
fn compact_embedding_artifact_binds_records_shape_dtype_digest_and_qc() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (table, bytes, embedding_record) = write_embedding_table(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    let row_link_record = record_with_schema(&fixture, "marklab.cell_embedding_row_link");
    let embedding_receipt = verify_cell_embedding_table_arrow_bytes(
        &bytes,
        &embedding_record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("verified embedding receipt");
    let row_link_receipt = verify_cell_embedding_row_link_arrow_from_store(
        &fixture.store,
        row_link_record,
        &fixture.expected,
        &fixture.row_link,
        embedding_budgets(),
    )
    .expect("verified row-link receipt");
    let artifact = CellEmbeddingArtifact::new(embedding_receipt, row_link_receipt, verified)
        .expect("embedding artifact binding");

    assert_eq!(artifact.embedding_artifact_id(), embedding_record.id());
    assert_eq!(artifact.row_link_artifact_id(), row_link_record.id());
    assert_eq!(
        artifact.provenance_artifact_id(),
        fixture.provenance_artifact_id
    );
    assert_eq!(artifact.row_count(), 1);
    assert_eq!(artifact.dimension(), 1_280);
    assert_eq!(artifact.dtype(), EmbeddingDtype::F32);
    assert_eq!(
        artifact.logical_digest(),
        table.qc_summary().logical_digest()
    );
    assert_eq!(artifact.qc_summary(), table.qc_summary());

    let (parquet_table, parquet_bytes, parquet_record) = write_embedding_table_parquet_fixture(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    let parquet_receipt = verify_cell_embedding_table_parquet_bytes(
        &parquet_bytes,
        &parquet_record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("verified Parquet embedding receipt");
    let parquet_artifact = CellEmbeddingArtifact::new(parquet_receipt, row_link_receipt, verified)
        .expect("Parquet embedding artifact binding");
    assert_eq!(parquet_artifact.qc_summary(), parquet_table.qc_summary());
    assert_eq!(parquet_artifact.logical_digest(), artifact.logical_digest());
    assert_ne!(
        parquet_artifact.embedding_artifact_id(),
        artifact.embedding_artifact_id()
    );

    let wrong_row_link = record_with_schema(&fixture, "marklab.cell_embedding_expected_cells");
    assert!(matches!(
        verify_cell_embedding_row_link_arrow_from_store(
            &fixture.store,
            wrong_row_link,
            &fixture.expected,
            &fixture.row_link,
            embedding_budgets(),
        ),
        Err(VerifiedReaderError::Callback(
            EmbeddingColumnarError::ArtifactBindingMismatch
        ))
    ));

    let unrelated = build_fixture_with_status(
        "marklab.model_checkpoint",
        LicenseAvailability::Managed,
        FixtureEmbeddingStatus::MissingVector,
    );
    let unrelated_row_link = record_with_schema(&unrelated, "marklab.cell_embedding_row_link");
    let unrelated_receipt = verify_cell_embedding_row_link_arrow_from_store(
        &unrelated.store,
        unrelated_row_link,
        &unrelated.expected,
        &unrelated.row_link,
        embedding_budgets(),
    )
    .expect("unrelated verified row-link receipt");
    assert_eq!(
        CellEmbeddingArtifact::new(embedding_receipt, unrelated_receipt, verified),
        Err(marklab::EmbeddingError::ArtifactBindingMismatch)
    );
}

#[test]
fn verified_graph_rejects_embedding_dimension_drift_from_provenance() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let expected_record = record_with_schema(&fixture, "marklab.cell_embedding_expected_cells");
    let row_link_record = record_with_schema(&fixture, "marklab.cell_embedding_row_link");
    let table = CellEmbeddingTable::from_rows(
        1,
        &fixture.expected,
        expected_record.id(),
        fixture.provenance_artifact_id,
        fixture.row_link.logical_digest(),
        vec![CellEmbeddingRow::present(cell("cell-a"), vec![0.25])],
        1_024,
    )
    .expect("one-dimensional table");
    let bindings = CellEmbeddingTablePhysicalBindings::new(
        expected_record.id(),
        fixture.provenance_artifact_id,
        row_link_record.id(),
        fixture.row_link.logical_digest(),
        table.qc_summary().logical_digest(),
    )
    .expect("one-dimensional bindings");
    let mut bytes = Vec::new();
    write_cell_embedding_table_arrow(&mut bytes, &table, bindings, embedding_budgets())
        .expect("one-dimensional Arrow table");
    let record = draft_record(
        "marklab.cell_embedding_table",
        "application/vnd.marklab.cell-embedding-table.v1+arrow",
        &bytes,
        vec![
            expected_record.id(),
            fixture.provenance_artifact_id,
            row_link_record.id(),
        ],
        Some(embedding_manifest(1, 1)),
    );

    assert_eq!(
        verify_cell_embedding_table_arrow_bytes(
            &bytes,
            &record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::ArtifactBindingMismatch)
    );
}

#[test]
fn streaming_arrow_and_parquet_scans_match_materialized_qc_without_retaining_the_table() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let row = || CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]);
    let (arrow_table, arrow_bytes, arrow_draft) = write_embedding_table(&fixture, row());
    let arrow_record = fixture
        .store
        .publish(&arrow_draft, |writer| writer.write_all(&arrow_bytes))
        .expect("publish Arrow table")
        .into_record();
    let (parquet_table, parquet_bytes, parquet_draft) =
        write_embedding_table_parquet_fixture(&fixture, row());
    let parquet_record = fixture
        .store
        .publish(&parquet_draft, |writer| writer.write_all(&parquet_bytes))
        .expect("publish Parquet table")
        .into_record();

    let arrow_borrowed = scan_cell_embedding_table_arrow_bytes(
        &arrow_bytes,
        &arrow_draft,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("borrowed Arrow scan");
    let arrow_managed = scan_cell_embedding_table_arrow_from_store(
        &fixture.store,
        &arrow_record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("managed Arrow scan");
    let parquet_borrowed = scan_cell_embedding_table_parquet_bytes(
        &parquet_bytes,
        &parquet_draft,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("borrowed Parquet scan");
    let parquet_managed = scan_cell_embedding_table_parquet_from_store(
        &fixture.store,
        &parquet_record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("managed Parquet scan");

    assert_eq!(arrow_borrowed, arrow_table.qc_summary());
    assert_eq!(arrow_managed, arrow_borrowed);
    assert_eq!(parquet_borrowed, parquet_table.qc_summary());
    assert_eq!(parquet_managed, parquet_borrowed);
    assert_eq!(parquet_borrowed, arrow_borrowed);
}

#[test]
fn managed_embedding_scans_report_store_integrity_before_columnar_callbacks() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let graph = verified_graph(&fixture);
    let row = || CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]);
    let (_, arrow_bytes, arrow_draft) = write_embedding_table(&fixture, row());
    let arrow_record = fixture
        .store
        .publish(&arrow_draft, |writer| writer.write_all(&arrow_bytes))
        .expect("publish Arrow table")
        .into_record();
    let (_, parquet_bytes, parquet_draft) = write_embedding_table_parquet_fixture(&fixture, row());
    let parquet_record = fixture
        .store
        .publish(&parquet_draft, |writer| writer.write_all(&parquet_bytes))
        .expect("publish Parquet table")
        .into_record();

    let corrupt = |record: &ArtifactRecord, bytes: &[u8]| {
        let mut corrupted = bytes.to_vec();
        corrupted[0] ^= 1;
        fs::write(managed_path(&fixture._root, record), corrupted).expect("corrupt managed object");
    };
    corrupt(&arrow_record, &arrow_bytes);
    assert!(matches!(
        scan_cell_embedding_table_arrow_from_store(
            &fixture.store,
            &arrow_record,
            &fixture.expected,
            &fixture.row_link,
            graph,
            embedding_budgets(),
        ),
        Err(VerifiedReaderError::Store(
            ArtifactStoreError::ContentIntegrity { .. }
        ))
    ));

    corrupt(&parquet_record, &parquet_bytes);
    assert!(matches!(
        scan_cell_embedding_table_parquet_from_store(
            &fixture.store,
            &parquet_record,
            &fixture.expected,
            &fixture.row_link,
            graph,
            embedding_budgets(),
        ),
        Err(VerifiedReaderError::Store(
            ArtifactStoreError::ContentIntegrity { .. }
        ))
    ));
}

#[test]
fn every_embedding_status_has_identical_domain_arrow_and_parquet_qc() {
    let rows = vec![
        (cell("cell-a"), FixtureEmbeddingStatus::Present),
        (cell("cell-b"), FixtureEmbeddingStatus::MissingVector),
        (cell("cell-c"), FixtureEmbeddingStatus::ExtractionFailed),
        (cell("cell-d"), FixtureEmbeddingStatus::QcRejected),
    ];
    let fixture = build_fixture_with_rows(
        "marklab.model_checkpoint",
        LicenseAvailability::Managed,
        1_280,
        rows.clone(),
    );
    let verified = verified_graph(&fixture);
    let domain_rows = rows
        .into_iter()
        .map(|(cell_id, status)| status.table_row(cell_id, fixture.dimension))
        .collect::<Vec<_>>();
    let (arrow_table, arrow_bytes, arrow_record) =
        write_embedding_rows_arrow(&fixture, domain_rows.clone(), embedding_budgets());
    let (parquet_table, parquet_bytes, parquet_record) =
        write_embedding_rows_parquet(&fixture, domain_rows, embedding_budgets());

    let arrow_qc = scan_cell_embedding_table_arrow_bytes(
        &arrow_bytes,
        &arrow_record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("mixed Arrow scan");
    let parquet_qc = scan_cell_embedding_table_parquet_bytes(
        &parquet_bytes,
        &parquet_record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("mixed Parquet scan");

    assert_eq!(arrow_qc, arrow_table.qc_summary());
    assert_eq!(parquet_qc, parquet_table.qc_summary());
    assert_eq!(parquet_qc, arrow_qc);
    assert_eq!(arrow_qc.row_count(), 4);
    assert_eq!(arrow_qc.present_count(), 1);
    assert_eq!(arrow_qc.missing_vector_count(), 1);
    assert_eq!(arrow_qc.extraction_failed_count(), 1);
    assert_eq!(arrow_qc.qc_rejected_count(), 1);
    assert_eq!(arrow_qc.all_zero_present_count(), 0);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(12))]

    #[test]
    fn property_generated_domain_arrow_and_parquet_rows_are_semantically_identical(
        raw_statuses in any::<[u8; 4]>(),
        seeds in any::<[u16; 4]>(),
    ) {
        let cells = [cell("cell-a"), cell("cell-b"), cell("cell-c"), cell("cell-d")];
        let statuses = raw_statuses.map(|code| match code % 4 {
            0 => FixtureEmbeddingStatus::Present,
            1 => FixtureEmbeddingStatus::MissingVector,
            2 => FixtureEmbeddingStatus::ExtractionFailed,
            _ => FixtureEmbeddingStatus::QcRejected,
        });
        let fixture_rows = cells
            .iter()
            .cloned()
            .zip(statuses)
            .collect::<Vec<_>>();
        let fixture = build_fixture_with_rows(
            "marklab.model_checkpoint",
            LicenseAvailability::Managed,
            1_280,
            fixture_rows.clone(),
        );
        let graph = verified_graph(&fixture);
        let domain_rows = fixture_rows
            .into_iter()
            .zip(seeds)
            .map(|((cell_id, status), seed)| match status {
                FixtureEmbeddingStatus::Present => CellEmbeddingRow::present(
                    cell_id,
                    (0..1_280)
                        .map(|column| {
                            if column == 0 && (seed as usize).is_multiple_of(2) {
                                -0.0
                            } else {
                                let column = u16::try_from(column).expect("property column");
                                f32::from(seed.wrapping_add(column)) / 16.0 + 0.25
                            }
                        })
                        .collect(),
                ),
                status => CellEmbeddingRow::non_present(cell_id, status.domain_status())
                    .expect("non-present property row"),
            })
            .collect::<Vec<_>>();
        let (domain, arrow_bytes, arrow_record) = write_embedding_rows_arrow(
            &fixture,
            domain_rows.clone(),
            embedding_budgets(),
        );
        let (_, parquet_bytes, parquet_record) = write_embedding_rows_parquet(
            &fixture,
            domain_rows,
            embedding_budgets(),
        );
        let arrow_qc = scan_cell_embedding_table_arrow_bytes(
            &arrow_bytes,
            &arrow_record,
            &fixture.expected,
            &fixture.row_link,
            graph,
            embedding_budgets(),
        )
        .expect("property Arrow scan");
        let parquet_qc = scan_cell_embedding_table_parquet_bytes(
            &parquet_bytes,
            &parquet_record,
            &fixture.expected,
            &fixture.row_link,
            graph,
            embedding_budgets(),
        )
        .expect("property Parquet scan");
        let arrow_table = read_cell_embedding_table_arrow_bytes(
            &arrow_bytes,
            &arrow_record,
            &fixture.expected,
            &fixture.row_link,
            graph,
            embedding_budgets(),
        )
        .expect("property Arrow read");
        let parquet_table = read_cell_embedding_table_parquet_bytes(
            &parquet_bytes,
            &parquet_record,
            &fixture.expected,
            &fixture.row_link,
            graph,
            embedding_budgets(),
        )
        .expect("property Parquet read");

        prop_assert_eq!(arrow_qc, domain.qc_summary());
        prop_assert_eq!(parquet_qc, arrow_qc);
        prop_assert_eq!(&arrow_table, &domain);
        prop_assert_eq!(&parquet_table, &arrow_table);
    }
}

#[cfg(feature = "csv")]
#[test]
fn all_present_npy_csv_domain_arrow_and_parquet_paths_share_exact_semantics() {
    let mut source_rows = vec![
        (0..1_280)
            .map(|column| column as f32 + 0.25)
            .collect::<Vec<_>>(),
        (0..1_280)
            .map(|column| -(column as f32) - 0.5)
            .collect::<Vec<_>>(),
    ];
    source_rows[0][3] = -0.0;
    source_rows[1][7] = -0.0;
    let expected_values = source_rows
        .iter()
        .flatten()
        .map(|value| if *value == 0.0 { 0.0 } else { *value })
        .collect::<Vec<_>>();
    let csv_bytes = all_present_source_csv(2);
    let npy_bytes = all_present_source_npy(&source_rows);
    let fixture = build_fixture_with_rows_and_sources(
        "marklab.model_checkpoint",
        LicenseAvailability::Managed,
        1_280,
        vec![
            (cell("cell-a"), FixtureEmbeddingStatus::Present),
            (cell("cell-b"), FixtureEmbeddingStatus::Present),
        ],
        &csv_bytes,
        &npy_bytes,
    );
    let graph = verified_graph(&fixture);
    let source_bindings = CellVitHeArtifactBindings::from_records(
        record_with_schema(&fixture, "marklab.cell_embedding_source_cells").clone(),
        record_with_schema(&fixture, "marklab.cell_embedding_source_npy").clone(),
        record_with_schema(&fixture, "marklab.cell_embedding_expected_cells").clone(),
        record_with_schema(&fixture, "marklab.cell_embedding_identity_map").clone(),
        record_with_schema(&fixture, "marklab.converter_manifest").clone(),
    )
    .expect("source artifact bindings");
    let hierarchy = hierarchy(&fixture.expected);
    let source_budgets = SourceBundleBudgets::new(
        u64::try_from(npy_bytes.len()).expect("NPY length"),
        u64::try_from(csv_bytes.len()).expect("CSV length"),
        64 * 1024,
        8 * 1024 * 1024,
        2 * 1_280 * 4,
    );
    let request = CellVitHeImportRequest::new(
        &fixture.expected,
        &fixture.identity_map,
        &hierarchy,
        &source_bindings,
        source_budgets,
    );
    let candidate = import_cellvit_he_bundle_bytes(&npy_bytes, &csv_bytes, request)
        .expect("bounded NPY/CSV import");
    assert_eq!(candidate.canonical_values(), expected_values);
    assert_eq!(candidate.row_link(), &fixture.row_link);
    let imported = candidate
        .finalize(&fixture.expected, &graph)
        .expect("verified source finalization");
    let table = imported.table();
    assert_eq!(
        table.scan_qc(1).expect("single-row blocks"),
        table.qc_summary()
    );

    let expected_record = record_with_schema(&fixture, "marklab.cell_embedding_expected_cells");
    let row_link_record = record_with_schema(&fixture, "marklab.cell_embedding_row_link");
    let physical_bindings = CellEmbeddingTablePhysicalBindings::new(
        expected_record.id(),
        fixture.provenance_artifact_id,
        row_link_record.id(),
        fixture.row_link.logical_digest(),
        table.qc_summary().logical_digest(),
    )
    .expect("physical table bindings");
    let mut arrow_bytes = Vec::new();
    write_cell_embedding_table_arrow(
        &mut arrow_bytes,
        table,
        physical_bindings,
        embedding_budgets(),
    )
    .expect("write imported Arrow table");
    let arrow_record = embedding_table_record(&fixture, &arrow_bytes);
    let mut parquet_bytes = Vec::new();
    write_cell_embedding_table_parquet(
        &mut parquet_bytes,
        table,
        physical_bindings,
        embedding_budgets(),
    )
    .expect("write imported Parquet table");
    let parquet_record = embedding_parquet_record(&fixture, &parquet_bytes);

    let arrow_qc = scan_cell_embedding_table_arrow_bytes(
        &arrow_bytes,
        &arrow_record,
        &fixture.expected,
        &fixture.row_link,
        graph,
        embedding_budgets(),
    )
    .expect("scan imported Arrow table");
    let parquet_qc = scan_cell_embedding_table_parquet_bytes(
        &parquet_bytes,
        &parquet_record,
        &fixture.expected,
        &fixture.row_link,
        graph,
        embedding_budgets(),
    )
    .expect("scan imported Parquet table");
    let arrow_table = read_cell_embedding_table_arrow_bytes(
        &arrow_bytes,
        &arrow_record,
        &fixture.expected,
        &fixture.row_link,
        graph,
        embedding_budgets(),
    )
    .expect("read imported Arrow table");
    let parquet_table = read_cell_embedding_table_parquet_bytes(
        &parquet_bytes,
        &parquet_record,
        &fixture.expected,
        &fixture.row_link,
        graph,
        embedding_budgets(),
    )
    .expect("read imported Parquet table");

    assert_eq!(arrow_qc, table.qc_summary());
    assert_eq!(parquet_qc, arrow_qc);
    for observed in [table, &arrow_table, &parquet_table] {
        let mut values = Vec::with_capacity(expected_values.len());
        for row in 0..observed.row_count() {
            values.extend_from_slice(
                observed
                    .row(row)
                    .expect("canonical row")
                    .vector()
                    .expect("all-present vector"),
            );
        }
        assert_eq!(values, expected_values);
    }
}

#[test]
fn multi_group_scans_succeed_at_a_retained_limit_that_rejects_materialization() {
    let rows = (0..8_193)
        .map(|index| {
            let status = match index % 4 {
                0 => FixtureEmbeddingStatus::Present,
                1 => FixtureEmbeddingStatus::MissingVector,
                2 => FixtureEmbeddingStatus::ExtractionFailed,
                _ => FixtureEmbeddingStatus::QcRejected,
            };
            (cell(&format!("cell-{index:08}")), status)
        })
        .collect::<Vec<_>>();
    let fixture = build_fixture_with_rows(
        "marklab.model_checkpoint",
        LicenseAvailability::Managed,
        1_280,
        rows.clone(),
    );
    let verified = verified_graph(&fixture);
    let domain_rows = rows
        .into_iter()
        .map(|(cell_id, status)| status.table_row(cell_id, fixture.dimension))
        .collect::<Vec<_>>();
    let generous = EmbeddingColumnarBudgets::new(
        512 * 1024 * 1024,
        512 * 1024 * 1024,
        512 * 1024 * 1024,
        512 * 1024 * 1024,
    );
    let (_, arrow_bytes, arrow_record) =
        write_embedding_rows_arrow(&fixture, domain_rows.clone(), generous);
    let (_, parquet_bytes, parquet_record) =
        write_embedding_rows_parquet(&fixture, domain_rows, generous);

    let arrow_retained = exact_retained_limit(|maximum_retained_bytes| {
        scan_cell_embedding_table_arrow_bytes(
            &arrow_bytes,
            &arrow_record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            EmbeddingColumnarBudgets::new(
                generous.maximum_file_bytes(),
                maximum_retained_bytes,
                generous.maximum_row_group_bytes(),
                generous.maximum_decoded_bytes(),
            ),
        )
    });
    let parquet_retained = exact_retained_limit(|maximum_retained_bytes| {
        scan_cell_embedding_table_parquet_bytes(
            &parquet_bytes,
            &parquet_record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            EmbeddingColumnarBudgets::new(
                generous.maximum_file_bytes(),
                maximum_retained_bytes,
                generous.maximum_row_group_bytes(),
                generous.maximum_decoded_bytes(),
            ),
        )
    });

    for (retained, result) in [
        (
            arrow_retained,
            read_cell_embedding_table_arrow_bytes(
                &arrow_bytes,
                &arrow_record,
                &fixture.expected,
                &fixture.row_link,
                verified,
                EmbeddingColumnarBudgets::new(
                    generous.maximum_file_bytes(),
                    arrow_retained,
                    generous.maximum_row_group_bytes(),
                    generous.maximum_decoded_bytes(),
                ),
            ),
        ),
        (
            parquet_retained,
            read_cell_embedding_table_parquet_bytes(
                &parquet_bytes,
                &parquet_record,
                &fixture.expected,
                &fixture.row_link,
                verified,
                EmbeddingColumnarBudgets::new(
                    generous.maximum_file_bytes(),
                    parquet_retained,
                    generous.maximum_row_group_bytes(),
                    generous.maximum_decoded_bytes(),
                ),
            ),
        ),
    ] {
        assert!(matches!(
            result,
            Err(EmbeddingColumnarError::RetainedByteBudgetExceeded { required, maximum })
                if maximum == retained && required > maximum
        ));
    }
}
