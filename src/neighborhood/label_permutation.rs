use crate::{
    common::seeds::splitmix64, multimodal::cells::CellSection,
    permutation::labels::deterministic_shuffle,
};

pub(crate) fn shuffle_labels_within_sections<T: Clone>(
    labels: &[T],
    sections: &[CellSection],
    shuffled: &mut [T],
    seed: u64,
) {
    debug_assert_eq!(labels.len(), sections.len());
    debug_assert_eq!(labels.len(), shuffled.len());

    shuffled.clone_from_slice(labels);
    shuffle_section(labels, sections, shuffled, CellSection::He, seed);
    shuffle_section(
        labels,
        sections,
        shuffled,
        CellSection::Ihc,
        splitmix64(seed),
    );
}

fn shuffle_section<T: Clone>(
    labels: &[T],
    sections: &[CellSection],
    shuffled: &mut [T],
    section: CellSection,
    seed: u64,
) {
    let indices = sections
        .iter()
        .enumerate()
        .filter_map(|(index, current)| (*current == section).then_some(index))
        .collect::<Vec<_>>();
    let mut values = indices
        .iter()
        .map(|index| labels[*index].clone())
        .collect::<Vec<_>>();

    deterministic_shuffle(&mut values, seed);
    for (index, value) in indices.into_iter().zip(values) {
        shuffled[index] = value;
    }
}
