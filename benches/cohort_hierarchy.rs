use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use marklab_data::{
    BlockId, CellId, CohortHierarchy, HierarchyId, HierarchyKind, HierarchyNode, PatientId,
    ReplicationRole, SlideId, SpecimenId,
};

fn patient(value: String) -> HierarchyId {
    PatientId::new(value).expect("patient ID").into()
}

fn specimen(value: String) -> HierarchyId {
    SpecimenId::new(value).expect("specimen ID").into()
}

fn block(value: String) -> HierarchyId {
    BlockId::new(value).expect("block ID").into()
}

fn slide(value: String) -> HierarchyId {
    SlideId::new(value).expect("slide ID").into()
}

fn cell(value: String) -> HierarchyId {
    CellId::new(value).expect("cell ID").into()
}

fn hierarchy_fixture(n_specimens: usize, cells_per_specimen: usize) -> Vec<HierarchyNode> {
    let n_cells = n_specimens * cells_per_specimen;
    let mut nodes = Vec::with_capacity(n_cells + 4 * n_specimens);
    for specimen_index in 0..n_specimens {
        let patient_id = patient(format!("patient-{specimen_index}"));
        let specimen_id = specimen(format!("specimen-{specimen_index}"));
        let block_id = block(format!("block-{specimen_index}"));
        let slide_id = slide(format!("slide-{specimen_index}"));
        nodes.push(HierarchyNode::new(
            patient_id.clone(),
            None,
            ReplicationRole::BiologicalUnit,
        ));
        nodes.push(HierarchyNode::new(
            specimen_id.clone(),
            Some(patient_id),
            ReplicationRole::Structural,
        ));
        nodes.push(HierarchyNode::new(
            block_id.clone(),
            Some(specimen_id),
            ReplicationRole::Structural,
        ));
        nodes.push(HierarchyNode::new(
            slide_id.clone(),
            Some(block_id),
            ReplicationRole::Structural,
        ));
        for cell_index in 0..cells_per_specimen {
            nodes.push(HierarchyNode::new(
                cell(format!("cell-{specimen_index}-{cell_index}")),
                Some(slide_id.clone()),
                ReplicationRole::Structural,
            ));
        }
    }
    nodes
}

fn bench_cohort_hierarchy_1m_cells_10k_specimens(c: &mut Criterion) {
    let full = std::env::var("MARKLAB_BENCH_PROFILE").as_deref() == Ok("full");
    let n_specimens = if full { 10_000 } else { 100 };
    let cells_per_specimen = 100;
    let n_cells = n_specimens * cells_per_specimen;
    let fixture = hierarchy_fixture(n_specimens, cells_per_specimen);

    let mut group = c.benchmark_group("cohort_hierarchy");
    group.sample_size(10);
    group.throughput(Throughput::Elements(n_cells as u64));
    group.bench_function(
        format!("validate_{n_cells}_cells_{n_specimens}_specimens"),
        |b| {
            b.iter_batched(
                || fixture.clone(),
                |nodes| {
                    let hierarchy = CohortHierarchy::new(black_box(nodes), Vec::new())
                        .expect("valid hierarchy fixture");
                    let summary = hierarchy.design_summary();
                    assert_eq!(summary.object_count(HierarchyKind::Patient), n_specimens);
                    assert_eq!(summary.object_count(HierarchyKind::Specimen), n_specimens);
                    assert_eq!(summary.object_count(HierarchyKind::Block), n_specimens);
                    assert_eq!(summary.object_count(HierarchyKind::Slide), n_specimens);
                    assert_eq!(summary.object_count(HierarchyKind::Cell), n_cells);
                    assert_eq!(summary.biological_unit_count(), n_specimens);
                    assert_eq!(summary.pair_set_count(), 0);
                    assert_eq!(summary.repeated_set_count(), 0);
                    black_box(hierarchy)
                },
                BatchSize::LargeInput,
            );
        },
    );
    group.finish();
}

criterion_group!(benches, bench_cohort_hierarchy_1m_cells_10k_specimens);
criterion_main!(benches);
