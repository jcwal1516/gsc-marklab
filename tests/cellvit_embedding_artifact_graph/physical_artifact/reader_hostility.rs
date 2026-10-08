use super::*;

fn first_embedding_buffer_offset(bytes: &[u8], buffer_index: usize) -> usize {
    let footer_length_offset = bytes.len() - 10;
    let footer_length = i32::from_le_bytes(
        bytes[footer_length_offset..footer_length_offset + 4]
            .try_into()
            .expect("footer length"),
    ) as usize;
    let footer_start = bytes.len() - 10 - footer_length;
    let footer = arrow::ipc::root_as_footer(&bytes[footer_start..footer_start + footer_length])
        .expect("footer");
    let block = footer.recordBatches().expect("batches").get(0);
    let message_start = block.offset() as usize;
    let body_start = message_start + block.metaDataLength() as usize;
    let message = arrow::ipc::root_as_message(
        &bytes[message_start + 8..message_start + block.metaDataLength() as usize],
    )
    .expect("message");
    let buffer = message
        .header_as_record_batch()
        .expect("batch")
        .buffers()
        .expect("buffers")
        .get(buffer_index);
    body_start + buffer.offset() as usize
}

fn first_footer_record_block_offset(bytes: &[u8]) -> usize {
    let footer_length_offset = bytes.len() - 10;
    let footer_length = i32::from_le_bytes(
        bytes[footer_length_offset..footer_length_offset + 4]
            .try_into()
            .expect("footer length"),
    ) as usize;
    let footer_start = bytes.len() - 10 - footer_length;
    let footer_bytes = &bytes[footer_start..footer_start + footer_length];
    let footer = arrow::ipc::root_as_footer(footer_bytes).expect("footer");
    let raw = footer.recordBatches().expect("batches").get(0).0;
    footer_start
        + footer_bytes
            .windows(raw.len())
            .position(|window| window == raw)
            .expect("record block")
}

fn replace_all_same_length(bytes: &mut [u8], from: &[u8], to: &[u8]) -> usize {
    assert_eq!(from.len(), to.len());
    let offsets = bytes
        .windows(from.len())
        .enumerate()
        .filter_map(|(index, window)| (window == from).then_some(index))
        .collect::<Vec<_>>();
    for offset in &offsets {
        bytes[*offset..*offset + to.len()].copy_from_slice(to);
    }
    offsets.len()
}

#[test]
fn borrowed_and_managed_parquet_paths_reject_the_same_hostile_page_header() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (_, mut bytes, _) = write_embedding_table_parquet_fixture(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    assert_eq!(bytes[4] & 0x0f, 5, "page type is i32");
    bytes[4] = (bytes[4] & 0xf0) | 8;
    let expected_record = record_with_schema(&fixture, "marklab.cell_embedding_expected_cells");
    let row_link_record = record_with_schema(&fixture, "marklab.cell_embedding_row_link");
    let draft = draft_record(
        "marklab.cell_embedding_table",
        "application/vnd.marklab.cell-embedding-table.v1+parquet",
        &bytes,
        vec![
            expected_record.id(),
            fixture.provenance_artifact_id,
            row_link_record.id(),
        ],
        Some(embedding_parquet_manifest(1, 1_280)),
    );
    let record = fixture
        .store
        .publish(&draft, |writer| writer.write_all(&bytes))
        .expect("publish hostile Parquet")
        .into_record();
    let borrowed = read_cell_embedding_table_parquet_bytes(
        &bytes,
        &draft,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect_err("borrowed hostile page");
    let managed = match read_cell_embedding_table_parquet_from_store(
        &fixture.store,
        &record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    ) {
        Err(VerifiedReaderError::Callback(error)) => error,
        result => panic!("expected managed callback failure, observed {result:?}"),
    };
    assert_eq!(managed, borrowed);
    let borrowed_scan = scan_cell_embedding_table_parquet_bytes(
        &bytes,
        &draft,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect_err("borrowed hostile scan");
    let managed_scan = match scan_cell_embedding_table_parquet_from_store(
        &fixture.store,
        &record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    ) {
        Err(VerifiedReaderError::Callback(error)) => error,
        result => panic!("expected managed scan callback failure, observed {result:?}"),
    };
    assert_eq!(borrowed_scan, borrowed);
    assert_eq!(managed_scan, borrowed);
    assert!(matches!(
        managed,
        EmbeddingColumnarError::Parquet {
            reason: marklab::ParquetFailure::InvalidPageHeader,
        }
    ));
}

#[test]
fn parquet_reader_rejects_nonfinite_status_and_cell_drift_after_raw_preflight() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (_, canonical, _) = write_embedding_table_parquet_fixture(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );

    let value_pattern = 0.25_f32.to_le_bytes();
    let value_offset = canonical
        .windows(value_pattern.len())
        .position(|window| window == value_pattern)
        .expect("first plain float");
    let mut nonfinite = canonical.clone();
    nonfinite[value_offset..value_offset + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(matches!(
        read_cell_embedding_table_parquet_bytes(
            &nonfinite,
            &embedding_parquet_record(&fixture, &nonfinite),
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Parquet {
            reason: marklab::ParquetFailure::InvalidComponent,
        })
    ));

    let mut invalid_status = canonical.clone();
    assert_eq!(
        replace_all_same_length(&mut invalid_status, b"present", b"invalid"),
        1
    );
    let error = read_cell_embedding_table_parquet_bytes(
        &invalid_status,
        &embedding_parquet_record(&fixture, &invalid_status),
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect_err("invalid status");
    assert!(matches!(
        error,
        EmbeddingColumnarError::Parquet {
            reason: marklab::ParquetFailure::InvalidStatus,
        }
    ));
    assert!(!error.to_string().contains("invalid"));

    let mut wrong_cell = canonical;
    assert_eq!(
        replace_all_same_length(&mut wrong_cell, b"cell-a", b"cell-z"),
        1
    );
    assert!(matches!(
        read_cell_embedding_table_parquet_bytes(
            &wrong_cell,
            &embedding_parquet_record(&fixture, &wrong_cell),
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Parquet {
            reason: marklab::ParquetFailure::InvalidCellOrder,
        })
    ));

    let missing = build_fixture_with_status(
        "marklab.model_checkpoint",
        LicenseAvailability::Managed,
        FixtureEmbeddingStatus::MissingVector,
    );
    let missing_verified = verified_graph(&missing);
    let (_, mut hidden_nonzero, _) = write_embedding_table_parquet_fixture(
        &missing,
        CellEmbeddingRow::non_present(cell("cell-a"), EmbeddingStatus::MissingVector)
            .expect("missing row"),
    );
    let zero_run = hidden_nonzero
        .windows(64)
        .position(|window| window.iter().all(|byte| *byte == 0))
        .expect("plain zero vector run");
    hidden_nonzero[zero_run..zero_run + 4].copy_from_slice(&1.0_f32.to_le_bytes());
    assert!(matches!(
        read_cell_embedding_table_parquet_bytes(
            &hidden_nonzero,
            &embedding_parquet_record(&missing, &hidden_nonzero),
            &missing.expected,
            &missing.row_link,
            missing_verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Parquet {
            reason: marklab::ParquetFailure::InvalidComponent,
        })
    ));
}

#[test]
fn verified_store_and_borrowed_arrow_paths_reject_the_same_hostile_block() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (table, mut bytes, _) = write_embedding_table(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    let block_offset = first_footer_record_block_offset(&bytes);
    bytes[block_offset..block_offset + 8].copy_from_slice(&(-1_i64).to_le_bytes());
    let draft = embedding_table_record(&fixture, &bytes);
    let record = fixture
        .store
        .publish(&draft, |writer| writer.write_all(&bytes))
        .expect("publish hostile fixture")
        .into_record();
    let expected_record = record_with_schema(&fixture, "marklab.cell_embedding_expected_cells");
    let row_link_record = record_with_schema(&fixture, "marklab.cell_embedding_row_link");
    let bindings = CellEmbeddingTablePhysicalBindings::new(
        expected_record.id(),
        fixture.provenance_artifact_id,
        row_link_record.id(),
        fixture.row_link.logical_digest(),
        table.qc_summary().logical_digest(),
    )
    .expect("bindings");
    let borrowed_error = preflight_cell_embedding_table_arrow_bytes(
        &bytes,
        &fixture.expected,
        bindings,
        embedding_budgets(),
    )
    .expect_err("borrowed hostile block");
    let managed_error = match read_cell_embedding_table_arrow_from_store(
        &fixture.store,
        &record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    ) {
        Err(VerifiedReaderError::Callback(error)) => error,
        result => panic!("expected managed callback failure, observed {result:?}"),
    };
    assert_eq!(managed_error, borrowed_error);
    let borrowed_scan = scan_cell_embedding_table_arrow_bytes(
        &bytes,
        &draft,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect_err("borrowed hostile scan");
    let managed_scan = match scan_cell_embedding_table_arrow_from_store(
        &fixture.store,
        &record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    ) {
        Err(VerifiedReaderError::Callback(error)) => error,
        result => panic!("expected managed scan callback failure, observed {result:?}"),
    };
    assert_eq!(borrowed_scan, borrowed_error);
    assert_eq!(managed_scan, borrowed_error);
    assert!(matches!(
        managed_error,
        EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidBlock,
        }
    ));
}

#[test]
fn arrow_reader_rejects_nonfinite_negative_zero_status_link_and_logical_drift() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (_, canonical, _) = write_embedding_table(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    let value_offset = first_embedding_buffer_offset(&canonical, 5);

    let mut nonfinite = canonical.clone();
    nonfinite[value_offset..value_offset + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    let nonfinite_record = embedding_table_record(&fixture, &nonfinite);
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &nonfinite,
            &nonfinite_record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidComponent,
        })
    ));

    let mut negative_zero = canonical.clone();
    negative_zero[value_offset..value_offset + 4].copy_from_slice(&(-0.0_f32).to_le_bytes());
    let negative_zero_record = embedding_table_record(&fixture, &negative_zero);
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &negative_zero,
            &negative_zero_record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidComponent,
        })
    ));

    let mut invalid_status = canonical.clone();
    let status_offset = first_embedding_buffer_offset(&invalid_status, 8);
    invalid_status[status_offset..status_offset + 7].copy_from_slice(b"invalid");
    let invalid_status_record = embedding_table_record(&fixture, &invalid_status);
    let error = read_cell_embedding_table_arrow_bytes(
        &invalid_status,
        &invalid_status_record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect_err("invalid status");
    assert!(matches!(
        error,
        EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidStatus,
        }
    ));
    assert!(!error.to_string().contains("invalid"));

    let mut logical_drift = canonical.clone();
    let (table, _, _) = write_embedding_table(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    let old_digest = table.qc_summary().logical_digest().to_string();
    let replacement = "a".repeat(64);
    assert_ne!(old_digest, replacement);
    assert_eq!(
        replace_all_same_length(
            &mut logical_drift,
            old_digest.as_bytes(),
            replacement.as_bytes(),
        ),
        2
    );
    let logical_record = embedding_table_record(&fixture, &logical_drift);
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &logical_drift,
            &logical_record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::LogicalDigestMismatch,
        })
    ));

    let (_, status_mismatch, status_mismatch_record) = write_embedding_table(
        &fixture,
        CellEmbeddingRow::non_present(cell("cell-a"), EmbeddingStatus::QcRejected)
            .expect("rejected row"),
    );
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &status_mismatch,
            &status_mismatch_record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidStatus,
        })
    ));
}

#[test]
fn arrow_reader_rejects_hidden_nonzero_and_malformed_string_offsets() {
    let missing = build_fixture_with_status(
        "marklab.model_checkpoint",
        LicenseAvailability::Managed,
        FixtureEmbeddingStatus::MissingVector,
    );
    let missing_verified = verified_graph(&missing);
    let (expected_table, mut hidden_nonzero, _) = write_embedding_table(
        &missing,
        CellEmbeddingRow::non_present(cell("cell-a"), EmbeddingStatus::MissingVector)
            .expect("missing row"),
    );
    let canonical_record = embedding_table_record(&missing, &hidden_nonzero);
    let decoded = read_cell_embedding_table_arrow_bytes(
        &hidden_nonzero,
        &canonical_record,
        &missing.expected,
        &missing.row_link,
        missing_verified,
        embedding_budgets(),
    )
    .expect("canonical missing row");
    assert_eq!(decoded, expected_table);
    let value_offset = first_embedding_buffer_offset(&hidden_nonzero, 5);
    hidden_nonzero[value_offset..value_offset + 4].copy_from_slice(&1.0_f32.to_le_bytes());
    let hidden_record = embedding_table_record(&missing, &hidden_nonzero);
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &hidden_nonzero,
            &hidden_record,
            &missing.expected,
            &missing.row_link,
            missing_verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidComponent,
        })
    ));

    let present = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let present_verified = verified_graph(&present);
    let (_, mut malformed_offsets, _) = write_embedding_table(
        &present,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    let offsets_start = first_embedding_buffer_offset(&malformed_offsets, 1);
    malformed_offsets[offsets_start + 4..offsets_start + 8]
        .copy_from_slice(&i32::MAX.to_le_bytes());
    let malformed_record = embedding_table_record(&present, &malformed_offsets);
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &malformed_offsets,
            &malformed_record,
            &present.expected,
            &present.row_link,
            present_verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::StockDecode,
        })
    ));
}

#[test]
fn arrow_reader_rejects_unexpected_cell_identity_after_raw_preflight() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (_, mut bytes, _) = write_embedding_table(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    assert_eq!(replace_all_same_length(&mut bytes, b"cell-a", b"cell-z"), 1);
    let record = embedding_table_record(&fixture, &bytes);
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &bytes,
            &record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::Arrow {
            reason: ArrowIpcFailure::InvalidCellOrder,
        })
    ));
}

#[test]
fn arrow_reader_rejects_record_binding_before_stock_decode() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = verified_graph(&fixture);
    let (_, bytes, _) = write_embedding_table(
        &fixture,
        CellEmbeddingRow::present(cell("cell-a"), vec![0.25; 1_280]),
    );
    let wrong_record = draft_record(
        "marklab.cell_embedding_table",
        "application/octet-stream",
        &bytes,
        Vec::new(),
        None,
    );
    assert!(matches!(
        read_cell_embedding_table_arrow_bytes(
            &bytes,
            &wrong_record,
            &fixture.expected,
            &fixture.row_link,
            verified,
            embedding_budgets(),
        ),
        Err(EmbeddingColumnarError::ArtifactBindingMismatch)
    ));
}
