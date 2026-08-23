#![no_main]

use std::{str::FromStr, sync::OnceLock};

use libfuzzer_sys::fuzz_target;
use marklab::{
    ArtifactId, CellId, CohortHierarchy, ContentDigest, HierarchyId, HierarchyNode, PatientId,
    ReplicationRole, SlideId,
};
use marklab_embeddings::{
    preflight_cell_embedding_row_link_arrow_bytes, preflight_cell_embedding_table_arrow_bytes,
    write_cell_embedding_row_link_arrow, write_cell_embedding_table_arrow, CellEmbeddingProvenance,
    CellEmbeddingRow, CellEmbeddingRowLink, CellEmbeddingRowLinkEntry, CellEmbeddingTable,
    CellEmbeddingTablePhysicalBindings, CellVitCsvSummary, CellVitNpyMatrix,
    EmbeddingColumnarBudgets, EmbeddingStatus, ExpectedCellSet, SourceBundleBudgets,
};

struct ArrowSeed {
    bytes: Vec<u8>,
    expected: ExpectedCellSet,
    bindings: CellEmbeddingTablePhysicalBindings,
}

struct RowLinkArrowSeed {
    bytes: Vec<u8>,
    expected: ExpectedCellSet,
    row_link: CellEmbeddingRowLink,
}

fn artifact_id(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

fn arrow_seed() -> &'static ArrowSeed {
    static SEED: OnceLock<ArrowSeed> = OnceLock::new();
    SEED.get_or_init(|| {
        let first_cell = marklab::CellId::new("fuzz-cell-a").expect("cell ID");
        let second_cell = marklab::CellId::new("fuzz-cell-b").expect("cell ID");
        let expected =
            ExpectedCellSet::new("all.v1", vec![first_cell.clone(), second_cell.clone()])
                .expect("expected cells");
        let expected_id = artifact_id(b"fuzz-expected");
        let provenance_id = artifact_id(b"fuzz-provenance");
        let row_link_id = artifact_id(b"fuzz-row-link");
        let row_link_digest = ContentDigest::from_bytes(b"fuzz-row-link-logical");
        let table = CellEmbeddingTable::from_rows(
            3,
            &expected,
            expected_id,
            provenance_id,
            row_link_digest,
            vec![
                CellEmbeddingRow::present(first_cell, vec![1.0, 0.0, -2.5]),
                CellEmbeddingRow::non_present(second_cell, EmbeddingStatus::MissingVector)
                    .expect("missing row"),
            ],
            1024,
        )
        .expect("seed table");
        let bindings = CellEmbeddingTablePhysicalBindings::new(
            expected_id,
            provenance_id,
            row_link_id,
            row_link_digest,
            table.qc_summary().logical_digest(),
        )
        .expect("bindings");
        let budgets = EmbeddingColumnarBudgets::new(
            2 * 1024 * 1024,
            2 * 1024 * 1024,
            2 * 1024 * 1024,
            2 * 1024 * 1024,
        );
        let mut bytes = Vec::new();
        write_cell_embedding_table_arrow(&mut bytes, &table, bindings, budgets)
            .expect("canonical seed");
        ArrowSeed {
            bytes,
            expected,
            bindings,
        }
    })
}

fn row_link_arrow_seed() -> &'static RowLinkArrowSeed {
    static SEED: OnceLock<RowLinkArrowSeed> = OnceLock::new();
    SEED.get_or_init(|| {
        let first_cell = CellId::new("fuzz-link-a").expect("cell ID");
        let second_cell = CellId::new("fuzz-link-b").expect("cell ID");
        let expected =
            ExpectedCellSet::new("all.v1", vec![first_cell.clone(), second_cell.clone()])
                .expect("expected cells");
        let patient = HierarchyId::from(PatientId::new("fuzz-patient").expect("patient ID"));
        let slide = HierarchyId::from(SlideId::new("fuzz-slide").expect("slide ID"));
        let hierarchy = CohortHierarchy::new(
            vec![
                HierarchyNode::new(patient.clone(), None, ReplicationRole::BiologicalUnit),
                HierarchyNode::new(
                    slide.clone(),
                    None,
                    ReplicationRole::TechnicalReplicate {
                        biological_source: patient,
                    },
                ),
                HierarchyNode::new(
                    HierarchyId::from(first_cell.clone()),
                    Some(slide.clone()),
                    ReplicationRole::Structural,
                ),
                HierarchyNode::new(
                    HierarchyId::from(second_cell.clone()),
                    Some(slide),
                    ReplicationRole::Structural,
                ),
            ],
            Vec::new(),
        )
        .expect("hierarchy");
        let row_link = CellEmbeddingRowLink::new(
            artifact_id(b"fuzz-link-source-cells"),
            artifact_id(b"fuzz-link-source-vectors"),
            artifact_id(b"fuzz-link-expected"),
            artifact_id(b"fuzz-link-identity"),
            artifact_id(b"fuzz-link-converter"),
            &expected,
            &hierarchy,
            vec![
                CellEmbeddingRowLinkEntry::present(first_cell, 1, 0),
                CellEmbeddingRowLinkEntry::missing_vector(second_cell, 0),
            ],
            1024,
        )
        .expect("row link");
        let budgets = EmbeddingColumnarBudgets::new(
            2 * 1024 * 1024,
            2 * 1024 * 1024,
            2 * 1024 * 1024,
            2 * 1024 * 1024,
        );
        let mut bytes = Vec::new();
        write_cell_embedding_row_link_arrow(&mut bytes, &row_link, budgets)
            .expect("canonical row-link seed");
        RowLinkArrowSeed {
            bytes,
            expected,
            row_link,
        }
    })
}

fn fuzz_arrow(input: &[u8]) {
    let seed = arrow_seed();
    let mut mutated;
    let candidate = if input.first().is_some_and(|selector| selector & 1 == 0) {
        &input[1..]
    } else {
        mutated = seed.bytes.clone();
        for mutation in input.get(1..).unwrap_or_default().chunks_exact(3) {
            let raw_offset = usize::from(u16::from_le_bytes([mutation[0], mutation[1]]));
            let length = mutated.len();
            if let Some(byte) = mutated.get_mut(raw_offset % length) {
                *byte ^= mutation[2];
            }
        }
        mutated.as_slice()
    };
    let budgets = EmbeddingColumnarBudgets::new(
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        2 * 1024 * 1024,
    );
    let _ = preflight_cell_embedding_table_arrow_bytes(
        candidate,
        &seed.expected,
        seed.bindings,
        budgets,
    );
}

fn fuzz_row_link_arrow(input: &[u8]) {
    let seed = row_link_arrow_seed();
    let mut mutated;
    let candidate = if input.first().is_some_and(|selector| selector & 1 == 0) {
        &input[1..]
    } else {
        mutated = seed.bytes.clone();
        for mutation in input.get(1..).unwrap_or_default().chunks_exact(3) {
            let raw_offset = usize::from(u16::from_le_bytes([mutation[0], mutation[1]]));
            let length = mutated.len();
            if let Some(byte) = mutated.get_mut(raw_offset % length) {
                *byte ^= mutation[2];
            }
        }
        mutated.as_slice()
    };
    let budgets = EmbeddingColumnarBudgets::new(
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        2 * 1024 * 1024,
    );
    let _ = preflight_cell_embedding_row_link_arrow_bytes(
        candidate,
        &seed.expected,
        &seed.row_link,
        budgets,
    );
}

fuzz_target!(|bytes: &[u8]| {
    let Some((selector, input)) = bytes.split_first() else {
        return;
    };
    let source_budgets = SourceBundleBudgets::new(
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        64 * 1024,
        2 * 1024 * 1024,
        2 * 1024 * 1024,
    );
    match selector % 5 {
        0 => {
            let _ = CellVitNpyMatrix::from_bytes(input, source_budgets);
        }
        1 => {
            let _ = CellVitCsvSummary::from_bytes(input, source_budgets);
        }
        2 => {
            let _ = CellEmbeddingProvenance::from_canonical_json(input);
        }
        3 => fuzz_arrow(input),
        _ => fuzz_row_link_arrow(input),
    }
});
