mod digest;
mod resources;
mod types;
mod validation;
mod wire;

use std::fmt;

use marklab_data::SlideId;
use marklab_project::{ArtifactId, ContentDigest};

use self::{
    resources::{
        assessment_retained_bytes, enforce_retained, enforce_working, input_bytes,
        link_retained_bytes, try_vec_capacity,
    },
    validation::{validate, CommonBindings},
};
use super::{
    context::PatchEmbeddingContext,
    error::MultiscaleEmbeddingError,
    expected::{ExpectedPatchSet, ExpectedRegionSet},
    footprint::PatchFootprintSet,
};

pub use types::{PatchRegionAssessmentBindings, PatchRegionDeclaration, PatchRegionRelation};

/// Complete expected-patch by expected-region producer assessment with sparse nonzero rows.
#[derive(Clone, Eq, PartialEq)]
pub struct PatchRegionAssessment {
    common: CommonBindings,
    nonzero_relations: Box<[PatchRegionDeclaration]>,
    nonzero_relations_digest: ContentDigest,
}

impl fmt::Debug for PatchRegionAssessment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PatchRegionAssessment")
            .field("assessed_pair_count", &self.common.assessed_pair_count)
            .field("nonzero_relation_count", &self.nonzero_relations.len())
            .field("nonzero_relations_digest", &self.nonzero_relations_digest)
            .finish()
    }
}

impl PatchRegionAssessment {
    /// Validate exact support bindings and one exhaustive sparse nonzero declaration set.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        expected_patches: &ExpectedPatchSet,
        expected_regions: &ExpectedRegionSet,
        context: &PatchEmbeddingContext,
        footprints: &PatchFootprintSet,
        bindings: &PatchRegionAssessmentBindings,
        declarations: Vec<PatchRegionDeclaration>,
        maximum_retained_bytes: usize,
        maximum_working_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        let common = validate(
            expected_patches,
            expected_regions,
            context,
            footprints,
            bindings,
            &declarations,
        )?;
        let input_bytes = input_bytes(&declarations)?;
        enforce_working(input_bytes, maximum_working_bytes)?;
        let retained =
            assessment_retained_bytes(common.owning_slide_id.as_str().len(), &declarations)?;
        enforce_retained(retained, maximum_retained_bytes)?;
        let peak = input_bytes
            .checked_add(retained)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        enforce_working(peak, maximum_working_bytes)?;
        let nonzero_relations = clone_rows(&declarations)?;
        if assessment_retained_bytes(common.owning_slide_id.as_str().len(), &nonzero_relations)?
            != retained
        {
            return Err(MultiscaleEmbeddingError::SizeOverflow);
        }
        let nonzero_relations_digest = digest::relations_digest(&nonzero_relations)?;
        Ok(Self {
            common,
            nonzero_relations,
            nonzero_relations_digest,
        })
    }

    /// Frozen exhaustive assessment policy.
    pub fn assessment_policy(&self) -> &'static str {
        "expected_cartesian_exhaustive"
    }

    /// Owning slide shared by every expected patch and region.
    pub fn owning_slide_id(&self) -> &SlideId {
        &self.common.owning_slide_id
    }

    /// Exact assessed Cartesian pair count, including declared-zero absent rows.
    pub fn assessed_pair_count(&self) -> u64 {
        self.common.assessed_pair_count
    }

    /// Number of stored nonzero declarations.
    pub fn nonzero_relation_count(&self) -> usize {
        self.nonzero_relations.len()
    }

    /// Canonical nonzero declarations; every absent expected pair is producer-declared zero.
    pub fn nonzero_relations(&self) -> &[PatchRegionDeclaration] {
        &self.nonzero_relations
    }

    /// Domain-separated digest over canonical nonzero declaration rows.
    pub fn nonzero_relations_digest(&self) -> ContentDigest {
        self.nonzero_relations_digest
    }

    /// Encode the exact canonical assessment descriptor with one final newline.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, MultiscaleEmbeddingError> {
        wire::to_canonical_json(self)
    }

    /// Validate canonical descriptor bytes against this already-complete exhaustive assessment.
    ///
    /// The descriptor intentionally stores only counts, bindings, and the nonzero-row digest, so
    /// decoding it cannot reconstruct the declaration rows. This instance method preserves that
    /// boundary and is the exact byte-comparison primitive used by later artifact-graph checks.
    pub fn validate_canonical_json(
        &self,
        bytes: &[u8],
        maximum_encoded_bytes: usize,
        maximum_decoded_bytes: usize,
    ) -> Result<(), MultiscaleEmbeddingError> {
        wire::validate_canonical_json(self, bytes, maximum_encoded_bytes, maximum_decoded_bytes)
    }

    #[cfg(feature = "parquet")]
    pub(in crate::multiscale) fn compare_canonical_json_reader<R: std::io::Read + ?Sized>(
        &self,
        reader: &mut R,
    ) -> Result<(), super::json::CanonicalJsonReaderError> {
        wire::compare_canonical_json_reader(self, reader)
    }

    /// Sorted distinct artifact dependency IDs for the C-03 record boundary.
    pub fn direct_dependencies(&self) -> impl ExactSizeIterator<Item = ArtifactId> {
        let mut dependencies = [
            self.common.expected_patches_artifact_id,
            self.common.expected_regions_artifact_id,
            self.common.patch_context_artifact_id,
            self.common.patch_footprints_artifact_id,
            self.common.converter_artifact_id,
        ];
        dependencies.sort_unstable();
        dependencies.into_iter()
    }
}

/// Immutable sparse patch-region link derived only from one complete assessment.
#[derive(Clone, Eq, PartialEq)]
pub struct PatchRegionLink {
    common: CommonBindings,
    assessment_artifact_id: ArtifactId,
    assessment_content_digest: ContentDigest,
    nonzero_relations: Box<[PatchRegionDeclaration]>,
    logical_digest: ContentDigest,
}

impl fmt::Debug for PatchRegionLink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PatchRegionLink")
            .field("assessed_pair_count", &self.common.assessed_pair_count)
            .field("nonzero_relation_count", &self.nonzero_relations.len())
            .field("logical_digest", &self.logical_digest)
            .finish()
    }
}

impl PatchRegionLink {
    /// Copy the exact nonzero rows from one exhaustive assessment; no separate link rows exist.
    pub fn from_exhaustive_assessment(
        assessment: &PatchRegionAssessment,
        assessment_artifact_id: ArtifactId,
        assessment_content_digest: ContentDigest,
        maximum_retained_bytes: usize,
        maximum_working_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        validate_assessment_artifact_id(&assessment.common, assessment_artifact_id)?;
        let retained = link_retained_bytes(
            assessment.common.owning_slide_id.as_str().len(),
            &assessment.nonzero_relations,
        )?;
        enforce_retained(retained, maximum_retained_bytes)?;
        let assessment_retained = assessment_retained_bytes(
            assessment.common.owning_slide_id.as_str().len(),
            &assessment.nonzero_relations,
        )?;
        let peak = assessment_retained
            .checked_add(retained)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        enforce_working(peak, maximum_working_bytes)?;
        let nonzero_relations = clone_rows(&assessment.nonzero_relations)?;
        if link_retained_bytes(
            assessment.common.owning_slide_id.as_str().len(),
            &nonzero_relations,
        )? != retained
        {
            return Err(MultiscaleEmbeddingError::SizeOverflow);
        }
        let logical_digest = digest::link_digest(
            &assessment.common,
            assessment_artifact_id,
            assessment_content_digest,
            &nonzero_relations,
        )?;
        Ok(Self {
            common: assessment.common.clone(),
            assessment_artifact_id,
            assessment_content_digest,
            nonzero_relations,
            logical_digest,
        })
    }

    /// Owning slide.
    pub fn owning_slide_id(&self) -> &SlideId {
        &self.common.owning_slide_id
    }

    /// Exact assessed Cartesian pair count.
    pub fn assessed_pair_count(&self) -> u64 {
        self.common.assessed_pair_count
    }

    /// Number of sparse nonzero relations.
    pub fn nonzero_relation_count(&self) -> usize {
        self.nonzero_relations.len()
    }

    /// Canonical nonzero relation rows.
    pub fn nonzero_relations(&self) -> &[PatchRegionDeclaration] {
        &self.nonzero_relations
    }

    /// Expected-patch artifact identity.
    pub fn expected_patches_artifact_id(&self) -> ArtifactId {
        self.common.expected_patches_artifact_id
    }

    /// Expected-patch logical identity.
    pub fn expected_patches_logical_digest(&self) -> ContentDigest {
        self.common.expected_patches_logical_digest
    }

    /// Expected-region artifact identity.
    pub fn expected_regions_artifact_id(&self) -> ArtifactId {
        self.common.expected_regions_artifact_id
    }

    /// Expected-region logical identity.
    pub fn expected_regions_logical_digest(&self) -> ContentDigest {
        self.common.expected_regions_logical_digest
    }

    /// Patch-context artifact identity.
    pub fn patch_context_artifact_id(&self) -> ArtifactId {
        self.common.patch_context_artifact_id
    }

    /// Patch-context logical identity.
    pub fn patch_context_logical_digest(&self) -> ContentDigest {
        self.common.patch_context_logical_digest
    }

    /// Patch-footprint artifact identity.
    pub fn patch_footprints_artifact_id(&self) -> ArtifactId {
        self.common.patch_footprints_artifact_id
    }

    /// Patch-footprint logical identity.
    pub fn patch_footprints_logical_digest(&self) -> ContentDigest {
        self.common.patch_footprints_logical_digest
    }

    /// Converter artifact identity.
    pub fn converter_artifact_id(&self) -> ArtifactId {
        self.common.converter_artifact_id
    }

    /// Converter exact content digest.
    pub fn converter_content_digest(&self) -> ContentDigest {
        self.common.converter_content_digest
    }

    /// Exhaustive-assessment artifact identity.
    pub fn assessment_artifact_id(&self) -> ArtifactId {
        self.assessment_artifact_id
    }

    /// Exhaustive-assessment exact content digest.
    pub fn assessment_content_digest(&self) -> ContentDigest {
        self.assessment_content_digest
    }

    /// Domain-separated logical identity.
    pub fn logical_digest(&self) -> ContentDigest {
        self.logical_digest
    }

    pub(in crate::multiscale) fn retained_bytes(&self) -> Result<usize, MultiscaleEmbeddingError> {
        resources::link_retained_bytes(
            self.common.owning_slide_id.as_str().len(),
            &self.nonzero_relations,
        )
    }
}

fn clone_rows(
    rows: &[PatchRegionDeclaration],
) -> Result<Box<[PatchRegionDeclaration]>, MultiscaleEmbeddingError> {
    let mut cloned = try_vec_capacity::<PatchRegionDeclaration>(rows.len())?;
    cloned.extend(rows.iter().cloned());
    Ok(cloned.into_boxed_slice())
}

fn validate_assessment_artifact_id(
    common: &CommonBindings,
    assessment_artifact_id: ArtifactId,
) -> Result<(), MultiscaleEmbeddingError> {
    if [
        common.expected_patches_artifact_id,
        common.expected_regions_artifact_id,
        common.patch_context_artifact_id,
        common.patch_footprints_artifact_id,
        common.converter_artifact_id,
    ]
    .contains(&assessment_artifact_id)
    {
        return Err(MultiscaleEmbeddingError::DuplicatePatchRegionArtifactDependency);
    }
    Ok(())
}
