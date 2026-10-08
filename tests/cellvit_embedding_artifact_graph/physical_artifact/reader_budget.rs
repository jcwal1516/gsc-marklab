use super::*;

#[test]
fn arrow_materialization_charges_the_decoded_batch_alongside_the_final_table() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (table, bytes, draft) = write_embedding_table(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    let expected_record = record_with_schema(&fixture, "marklab.cell_embedding_expected_cells");
    let row_link_record = record_with_schema(&fixture, "marklab.cell_embedding_row_link");
    let bindings = CellEmbeddingTablePhysicalBindings::new(
        expected_record.id(),
        fixture.provenance_artifact_id,
        row_link_record.id(),
        fixture.row_link.logical_digest(),
        table.qc_summary().logical_digest(),
    )
    .expect("physical bindings");

    let mut retained_preflight_bytes = 0_usize;
    loop {
        let budgets = EmbeddingColumnarBudgets::new(
            8 * 1024 * 1024,
            retained_preflight_bytes,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
        );
        match preflight_cell_embedding_table_arrow_bytes(
            &bytes,
            &fixture.expected,
            bindings,
            budgets,
        ) {
            Ok(_) => break,
            Err(EmbeddingColumnarError::RetainedByteBudgetExceeded { required, maximum }) => {
                assert!(required > maximum);
                retained_preflight_bytes = required;
            }
            result => panic!("expected retained preflight edge, observed {result:?}"),
        }
    }

    let final_table_bytes = fixture
        .expected
        .cells()
        .len()
        .checked_mul(fixture.dimension as usize)
        .and_then(|components| components.checked_mul(size_of::<f32>()))
        .and_then(|value_bytes| {
            value_bytes.checked_add(
                fixture.expected.cells().len()
                    * (size_of::<CellId>() + size_of::<EmbeddingStatus>()),
            )
        })
        .and_then(|retained| {
            fixture
                .expected
                .cells()
                .iter()
                .try_fold(retained, |total, cell_id| {
                    total.checked_add(cell_id.as_str().len())
                })
        })
        .expect("final table retained bytes");
    let previously_undercounted_peak = retained_preflight_bytes + final_table_bytes;
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &bytes,
            &draft,
            &fixture.expected,
            &fixture.row_link,
            verified,
            EmbeddingColumnarBudgets::new(
                8 * 1024 * 1024,
                previously_undercounted_peak,
                8 * 1024 * 1024,
                8 * 1024 * 1024,
            ),
        ),
        Err(EmbeddingColumnarError::RetainedByteBudgetExceeded {
            required,
            maximum,
        }) if required > maximum && maximum == previously_undercounted_peak
    ));
}

#[test]
fn verified_graph_gates_full_arrow_table_materialization() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = fixture
        .provenance
        .validate_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected,
            &fixture.identity_map,
            &fixture.context,
            &fixture.row_link,
            &fixture.catalog,
            &fixture.store,
        )
        .expect("verified artifact graph");
    let expected_record = record_with_schema(&fixture, "marklab.cell_embedding_expected_cells");
    let row_link_record = record_with_schema(&fixture, "marklab.cell_embedding_row_link");
    let table = CellEmbeddingTable::from_rows(
        1_280,
        &fixture.expected,
        expected_record.id(),
        fixture.provenance_artifact_id,
        fixture.row_link.logical_digest(),
        vec![CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280])],
        1_000_000,
    )
    .expect("embedding table");
    let bindings = CellEmbeddingTablePhysicalBindings::new(
        expected_record.id(),
        fixture.provenance_artifact_id,
        row_link_record.id(),
        fixture.row_link.logical_digest(),
        table.qc_summary().logical_digest(),
    )
    .expect("physical bindings");
    let budgets = EmbeddingColumnarBudgets::new(
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
    );
    let mut bytes = Vec::new();
    write_cell_embedding_table_arrow(&mut bytes, &table, bindings, budgets)
        .expect("write Arrow table");
    let table_record = draft_record(
        "marklab.cell_embedding_table",
        "application/vnd.marklab.cell-embedding-table.v1+arrow",
        &bytes,
        vec![
            expected_record.id(),
            fixture.provenance_artifact_id,
            row_link_record.id(),
        ],
        Some(embedding_manifest(1, 1_280)),
    );

    let decoded = read_cell_embedding_table_arrow_bytes(
        &bytes,
        &table_record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        budgets,
    )
    .expect("read verified Arrow table");
    assert_eq!(decoded, table);

    let too_small = EmbeddingColumnarBudgets::new(
        bytes.len() as u64 - 1,
        budgets.maximum_retained_bytes(),
        budgets.maximum_row_group_bytes(),
        budgets.maximum_decoded_bytes(),
    );
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &bytes,
            row_link_record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            too_small,
        ),
        Err(EmbeddingColumnarError::FileByteBudgetExceeded { .. })
    ));
}

#[test]
fn verified_store_arrow_reader_matches_borrowed_bytes_and_checks_budget_first() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (expected_table, bytes, draft) = write_embedding_table(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    let record = fixture
        .store
        .publish(&draft, |writer| writer.write_all(&bytes))
        .expect("publish embedding table")
        .into_record();

    let decoded = read_cell_embedding_table_arrow_from_store(
        &fixture.store,
        &record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("read managed Arrow table");
    assert_eq!(decoded, expected_table);

    let too_small = EmbeddingColumnarBudgets::new(
        bytes.len() as u64 - 1,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
    );
    let unlocated = embedding_table_record(&fixture, &bytes);
    assert!(matches!(
        read_cell_embedding_table_arrow_from_store(
            &fixture.store,
            &unlocated,
            &fixture.expected,
            &fixture.row_link,
            verified,
            too_small,
        ),
        Err(VerifiedReaderError::Callback(
            EmbeddingColumnarError::FileByteBudgetExceeded { .. }
        ))
    ));
}

#[test]
fn verified_parquet_readers_match_the_domain_table_and_check_budget_first() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (expected_table, bytes, draft) = write_embedding_table_parquet_fixture(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );

    let borrowed = read_cell_embedding_table_parquet_bytes(
        &bytes,
        &draft,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("read borrowed Parquet table");
    assert_eq!(borrowed, expected_table);

    let mut different_same_length_content = bytes.clone();
    different_same_length_content[4] ^= 1;
    let forged_record = draft_record(
        "marklab.cell_embedding_table",
        "application/vnd.marklab.cell-embedding-table.v1+parquet",
        &different_same_length_content,
        draft.dependencies().to_vec(),
        Some(embedding_parquet_manifest(1, 1_280)),
    );
    assert!(matches!(
        read_cell_embedding_table_parquet_bytes(
            &bytes,
            &forged_record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::ArtifactBindingMismatch)
    ));

    let record = fixture
        .store
        .publish(&draft, |writer| writer.write_all(&bytes))
        .expect("publish embedding Parquet")
        .into_record();
    let managed = read_cell_embedding_table_parquet_from_store(
        &fixture.store,
        &record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("read managed Parquet table");
    assert_eq!(managed, expected_table);

    let too_small = EmbeddingColumnarBudgets::new(
        bytes.len() as u64 - 1,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
    );
    assert!(matches!(
        read_cell_embedding_table_parquet_bytes(
            &bytes,
            &draft,
            &fixture.expected,
            &fixture.row_link,
            verified,
            too_small,
        ),
        Err(EmbeddingColumnarError::FileByteBudgetExceeded { .. })
    ));
    assert!(matches!(
        read_cell_embedding_table_parquet_from_store(
            &fixture.store,
            &record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            too_small,
        ),
        Err(VerifiedReaderError::Callback(
            EmbeddingColumnarError::FileByteBudgetExceeded { .. }
        ))
    ));
}
