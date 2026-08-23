#![no_main]

use std::{str::FromStr, sync::OnceLock};

use libfuzzer_sys::fuzz_target;
use marklab::{ArtifactId, ContentDigest};
use marklab_embeddings::{
    preflight_cell_embedding_table_arrow_bytes, write_cell_embedding_table_arrow,
    CellEmbeddingProvenance, CellEmbeddingRow, CellEmbeddingTable,
    CellEmbeddingTablePhysicalBindings, CellVitCsvSummary, CellVitNpyMatrix,
    EmbeddingColumnarBudgets, EmbeddingStatus, ExpectedCellSet, SourceBundleBudgets,
};

struct ArrowSeed {
    bytes: Vec<u8>,
    expected: ExpectedCellSet,
    bindings: CellEmbeddingTablePhysicalBindings,
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
    match selector % 4 {
        0 => {
            let _ = CellVitNpyMatrix::from_bytes(input, source_budgets);
        }
        1 => {
            let _ = CellVitCsvSummary::from_bytes(input, source_budgets);
        }
        2 => {
            let _ = CellEmbeddingProvenance::from_canonical_json(input);
        }
        _ => fuzz_arrow(input),
    }
});
