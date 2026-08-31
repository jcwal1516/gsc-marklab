use std::{cmp::Ordering, fmt, mem::size_of};

use marklab_data::PatchId;
use marklab_project::{ArtifactId, ContentDigest};

use super::{
    allocation::try_vec_capacity, context::PatchEmbeddingContext, digest::LogicalDigest,
    error::MultiscaleEmbeddingError, expected::ExpectedPatchSet, footprint::PatchFootprintSet,
};

const OVERLAP_DOMAIN: &[u8] = b"marklab-patch-overlap-graph-logical-v1";
const MAX_OVERLAP_EDGES: usize = 400_000_000;

/// One canonical positive-area overlap between two distinct sampled patches.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct PatchOverlapEdge {
    left_patch_id: PatchId,
    right_patch_id: PatchId,
}

impl fmt::Debug for PatchOverlapEdge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PatchOverlapEdge")
            .finish_non_exhaustive()
    }
}

impl PatchOverlapEdge {
    /// Canonically smaller patch identity.
    pub fn left_patch_id(&self) -> &PatchId {
        &self.left_patch_id
    }

    /// Canonically larger patch identity.
    pub fn right_patch_id(&self) -> &PatchId {
        &self.right_patch_id
    }
}

/// Deterministic graph of actual positive-area intersections among sampled patches.
#[derive(Clone, Eq, PartialEq)]
pub struct PatchOverlapGraph {
    patch_ids: Box<[PatchId]>,
    component_indices: Box<[usize]>,
    edges: Box<[PatchOverlapEdge]>,
    component_count: usize,
    expected_patches_artifact_id: ArtifactId,
    expected_patches_logical_digest: ContentDigest,
    patch_footprints_artifact_id: ArtifactId,
    patch_footprints_logical_digest: ContentDigest,
    logical_digest: ContentDigest,
}

impl fmt::Debug for PatchOverlapGraph {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PatchOverlapGraph")
            .field("patch_count", &self.patch_ids.len())
            .field("edge_count", &self.edges.len())
            .field("component_count", &self.component_count)
            .field("logical_digest", &self.logical_digest)
            .finish()
    }
}

impl PatchOverlapGraph {
    /// Derive the exact graph with two identical indexed passes and explicit allocation budgets.
    pub fn derive(
        expected: &ExpectedPatchSet,
        context: &PatchEmbeddingContext,
        footprints: &PatchFootprintSet,
        patch_footprints_artifact_id: ArtifactId,
        maximum_retained_bytes: usize,
        maximum_working_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        validate_bindings(expected, context, footprints)?;
        let patch_extent = context.patch_px().map(i64::from);
        let patch_count = expected.ids().len();
        let scratch_bytes = pass_scratch_bytes(patch_count)?;
        enforce_working_budget(scratch_bytes, maximum_working_bytes)?;

        let counted = overlap_pass(expected, footprints, patch_extent, None)?;
        let retained_bytes = retained_bytes(expected, &counted)?;
        if retained_bytes > maximum_retained_bytes {
            return Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded {
                required: retained_bytes,
                maximum: maximum_retained_bytes,
            });
        }
        let materialized_peak = materialized_peak_bytes(scratch_bytes, patch_count, &counted)?;
        enforce_working_budget(retained_bytes.max(materialized_peak), maximum_working_bytes)?;

        let materialized =
            overlap_pass(expected, footprints, patch_extent, Some(counted.edge_count))?;
        if materialized.edge_count != counted.edge_count
            || materialized.edge_text_bytes != counted.edge_text_bytes
            || materialized.component_count != counted.component_count
        {
            return Err(MultiscaleEmbeddingError::SizeOverflow);
        }
        let edges = materialized
            .edges
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        let component_indices = materialized
            .component_indices
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        let patch_ids = clone_patch_ids(expected.ids())?;
        let expected_patches_artifact_id = footprints.expected_patches_artifact_id();
        let expected_patches_logical_digest = footprints.expected_patches_logical_digest();
        let patch_footprints_logical_digest = footprints.logical_digest();
        let logical_digest = overlap_digest(
            expected_patches_artifact_id,
            expected_patches_logical_digest,
            patch_footprints_artifact_id,
            patch_footprints_logical_digest,
            &edges,
        )?;
        Ok(Self {
            patch_ids,
            component_indices,
            edges,
            component_count: materialized.component_count,
            expected_patches_artifact_id,
            expected_patches_logical_digest,
            patch_footprints_artifact_id,
            patch_footprints_logical_digest,
            logical_digest,
        })
    }

    /// Number of expected patches, including isolated patches.
    pub fn patch_count(&self) -> usize {
        self.patch_ids.len()
    }

    /// Number of canonical positive-area overlap edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Canonical overlap edges in lexicographic patch-ID order.
    pub fn edges(&self) -> &[PatchOverlapEdge] {
        &self.edges
    }

    /// Number of connected components, including singleton components.
    pub fn component_count(&self) -> usize {
        self.component_count
    }

    /// Minimum-patch-ID component identity, or `None` when the patch is not expected.
    pub fn component_id(&self, patch_id: &PatchId) -> Option<&PatchId> {
        self.patch_ids
            .binary_search(patch_id)
            .ok()
            .map(|index| &self.patch_ids[self.component_indices[index]])
    }

    /// Expected-patch artifact identity transitively bound through the footprint set.
    pub fn expected_patches_artifact_id(&self) -> ArtifactId {
        self.expected_patches_artifact_id
    }

    /// Expected-patch logical identity transitively bound through the footprint set.
    pub fn expected_patches_logical_digest(&self) -> ContentDigest {
        self.expected_patches_logical_digest
    }

    /// Exact footprint artifact identity used to derive this graph.
    pub fn patch_footprints_artifact_id(&self) -> ArtifactId {
        self.patch_footprints_artifact_id
    }

    /// Exact footprint logical identity used to derive this graph.
    pub fn patch_footprints_logical_digest(&self) -> ContentDigest {
        self.patch_footprints_logical_digest
    }

    /// Domain-separated logical identity over bindings and canonical edges.
    pub fn logical_digest(&self) -> ContentDigest {
        self.logical_digest
    }
}

fn validate_bindings(
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
) -> Result<(), MultiscaleEmbeddingError> {
    if expected.logical_digest() != footprints.expected_patches_logical_digest()
        || expected.owning_slide_id() != context.owning_slide_id()
        || context.logical_digest() != footprints.patch_context_logical_digest()
        || expected.ids().len() != footprints.footprints().len()
        || expected
            .ids()
            .iter()
            .zip(footprints.footprints())
            .any(|(expected_id, footprint)| expected_id != footprint.patch_id())
    {
        return Err(MultiscaleEmbeddingError::OverlapInputMismatch);
    }
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct BucketEntry {
    key: [i64; 2],
    patch_index: usize,
}

impl Ord for BucketEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key
            .cmp(&other.key)
            .then_with(|| self.patch_index.cmp(&other.patch_index))
    }
}

impl PartialOrd for BucketEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct PassOutput {
    edge_count: usize,
    edge_text_bytes: usize,
    component_count: usize,
    edges: Option<Box<[PatchOverlapEdge]>>,
    component_indices: Option<Box<[usize]>>,
}

fn overlap_pass(
    expected: &ExpectedPatchSet,
    footprints: &PatchFootprintSet,
    patch_extent: [i64; 2],
    materialize_edges: Option<usize>,
) -> Result<PassOutput, MultiscaleEmbeddingError> {
    let patch_count = footprints.footprints().len();
    let mut bucket_entries = try_vec_capacity::<BucketEntry>(patch_count)?;
    for (patch_index, footprint) in footprints.footprints().iter().enumerate() {
        let origin = footprint.origin_px();
        bucket_entries.push(BucketEntry {
            key: [
                origin[1].div_euclid(patch_extent[1]),
                origin[0].div_euclid(patch_extent[0]),
            ],
            patch_index,
        });
    }
    bucket_entries.sort_unstable();
    let mut parents = try_vec_capacity::<usize>(patch_count)?;
    parents.extend(0..patch_count);
    let mut edges = match materialize_edges {
        Some(capacity) => Some(try_vec_capacity::<PatchOverlapEdge>(capacity)?),
        None => None,
    };
    let mut edge_count = 0_usize;
    let mut edge_text_bytes = 0_usize;

    for current in 0..patch_count {
        let origin = footprints.footprints()[current].origin_px();
        let bucket = [
            origin[1].div_euclid(patch_extent[1]),
            origin[0].div_euclid(patch_extent[0]),
        ];
        for dy in -1_i64..=1 {
            for dx in -1_i64..=1 {
                let key = [
                    bucket[0]
                        .checked_add(dy)
                        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
                    bucket[1]
                        .checked_add(dx)
                        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
                ];
                let start = bucket_entries.partition_point(|entry| entry.key < key);
                let end = bucket_entries.partition_point(|entry| entry.key <= key);
                for entry in &bucket_entries[start..end] {
                    if entry.patch_index >= current {
                        break;
                    }
                    if !positive_area_intersection(
                        footprints.footprints()[entry.patch_index].origin_px(),
                        origin,
                        patch_extent,
                    )? {
                        continue;
                    }
                    edge_count = increment_edge_count(edge_count)?;
                    let left = &expected.ids()[entry.patch_index];
                    let right = &expected.ids()[current];
                    edge_text_bytes = edge_text_bytes
                        .checked_add(left.as_str().len())
                        .and_then(|total| total.checked_add(right.as_str().len()))
                        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
                    union_minimum(&mut parents, entry.patch_index, current);
                    if let Some(edges) = &mut edges {
                        edges.push(PatchOverlapEdge {
                            left_patch_id: left.clone(),
                            right_patch_id: right.clone(),
                        });
                    }
                }
            }
        }
    }
    if edges
        .as_ref()
        .is_some_and(|edges| edges.len() != materialize_edges.unwrap_or_default())
    {
        return Err(MultiscaleEmbeddingError::SizeOverflow);
    }
    if let Some(edges) = &mut edges {
        edges.sort_unstable();
    }
    let mut component_count = 0_usize;
    let mut component_indices = match materialize_edges {
        Some(_) => Some(try_vec_capacity::<usize>(patch_count)?),
        None => None,
    };
    for index in 0..patch_count {
        let root = find_root(&mut parents, index);
        if root == index {
            component_count = component_count
                .checked_add(1)
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        }
        if let Some(indices) = &mut component_indices {
            indices.push(root);
        }
    }
    Ok(PassOutput {
        edge_count,
        edge_text_bytes,
        component_count,
        edges: edges.map(Vec::into_boxed_slice),
        component_indices: component_indices.map(Vec::into_boxed_slice),
    })
}

fn positive_area_intersection(
    left: [i64; 2],
    right: [i64; 2],
    extent: [i64; 2],
) -> Result<bool, MultiscaleEmbeddingError> {
    for axis in 0..2 {
        let left_end = left[axis]
            .checked_add(extent[axis])
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        let right_end = right[axis]
            .checked_add(extent[axis])
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        if left[axis] >= right_end || right[axis] >= left_end {
            return Ok(false);
        }
    }
    Ok(true)
}

fn increment_edge_count(current: usize) -> Result<usize, MultiscaleEmbeddingError> {
    let observed = current
        .checked_add(1)
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    if observed > MAX_OVERLAP_EDGES {
        return Err(MultiscaleEmbeddingError::OverlapEdgeCountExceeded {
            observed,
            maximum: MAX_OVERLAP_EDGES,
        });
    }
    Ok(observed)
}

fn find_root(parents: &mut [usize], mut index: usize) -> usize {
    while parents[index] != index {
        let grandparent = parents[parents[index]];
        parents[index] = grandparent;
        index = grandparent;
    }
    index
}

fn union_minimum(parents: &mut [usize], left: usize, right: usize) {
    let left_root = find_root(parents, left);
    let right_root = find_root(parents, right);
    if left_root != right_root {
        let (minimum, maximum) = if left_root < right_root {
            (left_root, right_root)
        } else {
            (right_root, left_root)
        };
        parents[maximum] = minimum;
    }
}

fn clone_patch_ids(ids: &[PatchId]) -> Result<Box<[PatchId]>, MultiscaleEmbeddingError> {
    let mut cloned = try_vec_capacity::<PatchId>(ids.len())?;
    cloned.extend(ids.iter().cloned());
    Ok(cloned.into_boxed_slice())
}

fn pass_scratch_bytes(patch_count: usize) -> Result<usize, MultiscaleEmbeddingError> {
    patch_count
        .checked_mul(size_of::<BucketEntry>() + size_of::<usize>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)
}

fn retained_bytes(
    expected: &ExpectedPatchSet,
    counted: &PassOutput,
) -> Result<usize, MultiscaleEmbeddingError> {
    let patch_storage = expected
        .ids()
        .len()
        .checked_mul(size_of::<PatchId>() + size_of::<usize>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    let patch_text = expected.ids().iter().try_fold(0_usize, |total, id| {
        total
            .checked_add(id.as_str().len())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)
    })?;
    let edge_storage = counted
        .edge_count
        .checked_mul(size_of::<PatchOverlapEdge>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    size_of::<PatchOverlapGraph>()
        .checked_add(patch_storage)
        .and_then(|total| total.checked_add(patch_text))
        .and_then(|total| total.checked_add(edge_storage))
        .and_then(|total| total.checked_add(counted.edge_text_bytes))
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)
}

fn materialized_peak_bytes(
    scratch_bytes: usize,
    patch_count: usize,
    counted: &PassOutput,
) -> Result<usize, MultiscaleEmbeddingError> {
    let edge_storage = counted
        .edge_count
        .checked_mul(size_of::<PatchOverlapEdge>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    let component_storage = patch_count
        .checked_mul(size_of::<usize>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    scratch_bytes
        .checked_add(edge_storage)
        .and_then(|total| total.checked_add(counted.edge_text_bytes))
        .and_then(|total| total.checked_add(component_storage))
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)
}

fn enforce_working_budget(required: usize, maximum: usize) -> Result<(), MultiscaleEmbeddingError> {
    if required > maximum {
        return Err(MultiscaleEmbeddingError::WorkingByteBudgetExceeded { required, maximum });
    }
    Ok(())
}

fn overlap_digest(
    expected_artifact_id: ArtifactId,
    expected_logical_digest: ContentDigest,
    footprints_artifact_id: ArtifactId,
    footprints_logical_digest: ContentDigest,
    edges: &[PatchOverlapEdge],
) -> Result<ContentDigest, MultiscaleEmbeddingError> {
    let mut digest = LogicalDigest::new(OVERLAP_DOMAIN);
    digest.artifact_id(expected_artifact_id);
    digest.content_digest(expected_logical_digest);
    digest.artifact_id(footprints_artifact_id);
    digest.content_digest(footprints_logical_digest);
    digest.array_len(edges.len())?;
    for edge in edges {
        digest.text(edge.left_patch_id.as_str());
        digest.text(edge.right_patch_id.as_str());
    }
    Ok(digest.finish())
}

#[cfg(test)]
mod tests;
