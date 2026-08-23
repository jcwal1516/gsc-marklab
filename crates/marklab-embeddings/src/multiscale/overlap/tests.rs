use super::{increment_edge_count, MultiscaleEmbeddingError, MAX_OVERLAP_EDGES};

#[test]
fn overlap_edge_counter_rejects_the_first_value_above_the_hard_limit() {
    assert_eq!(
        increment_edge_count(MAX_OVERLAP_EDGES - 1),
        Ok(MAX_OVERLAP_EDGES)
    );
    assert_eq!(
        increment_edge_count(MAX_OVERLAP_EDGES),
        Err(MultiscaleEmbeddingError::OverlapEdgeCountExceeded {
            observed: MAX_OVERLAP_EDGES + 1,
            maximum: MAX_OVERLAP_EDGES,
        })
    );
}
