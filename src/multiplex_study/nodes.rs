use super::{
    admission::{PreparedSlide, PreparedStudy},
    analyze_slide, invalid,
    model::{MultiplexStudyResult, SlideResult},
    reduction,
};
use crate::{exact_float_json, Result};
use marklab_workflow::{
    ArtifactRef, CacheKeyMaterial, ContentDigest, NodeError, NodeId, NodeSpec, WorkflowNode,
};

pub(super) struct SlideNode<'a> {
    spec: NodeSpec,
    study: &'a PreparedStudy,
    slide: &'a PreparedSlide,
    inputs: Vec<ArtifactRef>,
    configuration: ContentDigest,
    pub remaining_edge_work: usize,
}

impl<'a> SlideNode<'a> {
    pub fn new(study: &'a PreparedStudy, slide: &'a PreparedSlide) -> Result<Self> {
        let id = format!(
            "multiplex-slide-{}",
            ContentDigest::from_bytes(slide.source.slide_id.as_bytes())
        );
        let configuration = ContentDigest::from_bytes(&exact_float_json::encode(
            &serde_json::json!({
                "channels": study.recipe.channels, "selected_channels": study.recipe.design.selected_channels,
                "radius_um": study.recipe.design.radius_um, "weight_policy": study.recipe.design.weight_policy,
                "missingness": study.recipe.design.missingness, "limits": study.recipe.limits,
            }),
        )?);
        Ok(Self {
            spec: spec(&id, "multiplex_slide", Vec::new())?,
            study,
            slide,
            inputs: vec![slide.source_artifact.clone(), slide.panel_artifact.clone()],
            configuration,
            remaining_edge_work: study.recipe.limits.maximum_edge_evaluations,
        })
    }

    fn validate(&self, output: &SlideResult) -> Result<()> {
        let slide = self.slide;
        if output.slide_id != slide.source.slide_id
            || output.patient_id != slide.source.patient_id
            || output.group != slide.source.group
            || output.coordinate_frame_id != slide.frame.as_str()
            || output.source_sha256 != slide.source_artifact.digest().to_string()
            || output.panel_identity != slide.panel_artifact.digest().to_string()
            || output.window_identity != slide.window.descriptor().logical_digest.to_string()
            || output.cell_count != slide.table.cell_ids().len()
            || output.channels.len() != self.study.selection.len()
        {
            return Err(invalid("cached slide identity or shape mismatch"));
        }
        for (channel, selected) in output.channels.iter().zip(&self.study.selection) {
            if channel.channel != selected.as_str()
                || channel.observed_rows.checked_add(channel.missing_rows)
                    != Some(output.cell_count)
                || channel.directed_edges > self.study.recipe.limits.maximum_directed_edges
            {
                return Err(invalid("cached slide channel binding or counts mismatch"));
            }
            match channel.status.as_str() {
                "available"
                    if channel.reason.is_none()
                        && channel.moran_i.is_some_and(f64::is_finite)
                        && channel
                            .geary_c
                            .is_some_and(|value| value.is_finite() && value >= 0.0)
                        && channel.observed_rows >= 3
                        && channel.directed_edges > 0
                        && channel.weights_digest.is_some() => {}
                "unavailable"
                    if channel.moran_i.is_none()
                        && channel.geary_c.is_none()
                        && channel.reason.as_deref().is_some_and(|reason| {
                            matches!(
                                reason,
                                "insufficient_observed_points"
                                    | "isolated_observed_point"
                                    | "zero_variance"
                                    | "numerical_failure"
                            )
                        }) => {}
                _ => return Err(invalid("cached slide status and numeric payload disagree")),
            }
            if let Some(digest) = &channel.weights_digest {
                digest
                    .parse::<ContentDigest>()
                    .map_err(|error| invalid(error.to_string()))?;
            }
        }
        Ok(())
    }
}

impl WorkflowNode for SlideNode<'_> {
    type Output = SlideResult;
    fn spec(&self) -> &NodeSpec {
        &self.spec
    }
    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.inputs
    }
    fn verify_input_content(&self) -> std::result::Result<(), NodeError> {
        let source = ArtifactRef::from_bytes(
            self.slide.source_artifact.kind(),
            &serde_json::to_vec(&self.slide.source).map_err(NodeError::input)?,
        )
        .map_err(NodeError::input)?;
        if source != self.slide.source_artifact {
            return Err(NodeError::input(invalid(
                "slide source changed after admission",
            )));
        }
        Ok(())
    }
    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.configuration,
            execution_policy:
                b"serial;per-channel-complete-cases;shared-radius-graph;bounded-study-work",
            implementation_identity: "marklab-multiplex-slide-v1",
        }
    }
    fn execute(&self) -> std::result::Result<Self::Output, NodeError> {
        analyze_slide(self.study, self.slide, self.remaining_edge_work)
            .map_err(NodeError::execution)
    }
    fn encode_output(&self, output: &Self::Output) -> std::result::Result<Box<[u8]>, NodeError> {
        self.validate(output).map_err(NodeError::encoding)?;
        exact_float_json::encode(output).map_err(NodeError::encoding)
    }
    fn decode_output(&self, bytes: &[u8]) -> std::result::Result<Self::Output, NodeError> {
        let output = exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        self.validate(&output).map_err(NodeError::decode)?;
        Ok(output)
    }
    fn output_kind(&self) -> &'static str {
        "application/vnd.marklab.multiplex-slide-result+json;version=1"
    }
}

/// A concrete bounded fan-in when a study exceeds the durable owner's 64-input limit.
pub(super) struct SlideBatchNode<'a> {
    spec: NodeSpec,
    rows: &'a [SlideResult],
    inputs: Vec<ArtifactRef>,
}
impl<'a> SlideBatchNode<'a> {
    pub fn new(
        index: usize,
        rows: &'a [SlideResult],
        inputs: Vec<ArtifactRef>,
        dependencies: Vec<NodeId>,
    ) -> Result<Self> {
        Ok(Self {
            spec: spec(
                &format!("multiplex-collection-{index}"),
                "multiplex_slide_collection",
                dependencies,
            )?,
            rows,
            inputs,
        })
    }
    fn validate(&self, output: &[SlideResult]) -> Result<()> {
        if exact_float_json::encode(&output)? != exact_float_json::encode(&self.rows)? {
            return Err(invalid(
                "cached collection does not contain its exact admitted dependency outputs",
            ));
        }
        Ok(())
    }
}
impl WorkflowNode for SlideBatchNode<'_> {
    type Output = Vec<SlideResult>;
    fn spec(&self) -> &NodeSpec {
        &self.spec
    }
    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.inputs
    }
    fn verify_input_content(&self) -> std::result::Result<(), NodeError> {
        Ok(())
    }
    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_bytes(b"ordered-slide-collection-v1"),
            execution_policy: b"serial;at-most-64-verified-dependencies",
            implementation_identity: "marklab-multiplex-collection-v1",
        }
    }
    fn execute(&self) -> std::result::Result<Self::Output, NodeError> {
        Ok(self.rows.to_vec())
    }
    fn encode_output(&self, output: &Self::Output) -> std::result::Result<Box<[u8]>, NodeError> {
        self.validate(output).map_err(NodeError::encoding)?;
        exact_float_json::encode(output).map_err(NodeError::encoding)
    }
    fn decode_output(&self, bytes: &[u8]) -> std::result::Result<Self::Output, NodeError> {
        let output: Vec<SlideResult> =
            exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        self.validate(&output).map_err(NodeError::decode)?;
        Ok(output)
    }
    fn output_kind(&self) -> &'static str {
        "application/vnd.marklab.multiplex-slide-collection+json;version=1"
    }
}

pub(super) struct CohortNode<'a> {
    spec: NodeSpec,
    study: &'a PreparedStudy,
    slides: &'a [SlideResult],
    inputs: Vec<ArtifactRef>,
}
impl<'a> CohortNode<'a> {
    pub fn new(
        study: &'a PreparedStudy,
        slides: &'a [SlideResult],
        inputs: Vec<ArtifactRef>,
        dependencies: Vec<NodeId>,
    ) -> Result<Self> {
        Ok(Self {
            spec: spec(
                "multiplex-patient-inference",
                "multiplex_study",
                dependencies,
            )?,
            study,
            slides,
            inputs,
        })
    }
    fn validate(&self, output: &MultiplexStudyResult) -> Result<()> {
        if output.format != "marklab.multiplex_study_result"
            || output.version != 1
            || output.study_id != self.study.recipe.study_id
            || output.recipe_sha256 != self.study.recipe_digest.to_string()
            || exact_float_json::encode(&output.slides)? != exact_float_json::encode(&self.slides)?
            || exact_float_json::encode(&output.design)?
                != exact_float_json::encode(&self.study.recipe.design)?
            || exact_float_json::encode(&output.channels)?
                != exact_float_json::encode(&self.study.recipe.channels)?
            || exact_float_json::encode(&output.limits)?
                != exact_float_json::encode(&self.study.recipe.limits)?
        {
            return Err(invalid(
                "cached study input or dependency identity mismatch",
            ));
        }
        let (endpoints, patients) = reduction::patient_reduction(self.study, self.slides)?;
        if output.endpoint_names != endpoints
            || exact_float_json::encode(&output.patients)? != exact_float_json::encode(&patients)?
        {
            return Err(invalid("cached study patient reduction mismatch"));
        }
        output.validate_claim_state()?;
        Ok(())
    }
}
impl WorkflowNode for CohortNode<'_> {
    type Output = MultiplexStudyResult;
    fn spec(&self) -> &NodeSpec {
        &self.spec
    }
    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.inputs
    }
    fn verify_input_content(&self) -> std::result::Result<(), NodeError> {
        Ok(())
    }
    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial { configuration_digest: self.study.recipe_digest, execution_policy: b"serial;equal-slide-patient-mean;complete-family-max-t;no-silent-exclusions;experimental", implementation_identity: "marklab-multiplex-study-v1" }
    }
    fn execute(&self) -> std::result::Result<Self::Output, NodeError> {
        reduction::reduce(self.study, self.slides.to_vec()).map_err(NodeError::execution)
    }
    fn encode_output(&self, output: &Self::Output) -> std::result::Result<Box<[u8]>, NodeError> {
        self.validate(output).map_err(NodeError::encoding)?;
        exact_float_json::encode(output).map_err(NodeError::encoding)
    }
    fn decode_output(&self, bytes: &[u8]) -> std::result::Result<Self::Output, NodeError> {
        let output = exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        self.validate(&output).map_err(NodeError::decode)?;
        Ok(output)
    }
    fn output_kind(&self) -> &'static str {
        "application/vnd.marklab.multiplex-study-result+json;version=1"
    }
}

fn spec(id: &str, kind: &str, dependencies: Vec<NodeId>) -> Result<NodeSpec> {
    NodeSpec::new(
        NodeId::new(id).map_err(|error| invalid(error.to_string()))?,
        kind,
        1,
        dependencies,
    )
    .map_err(|error| invalid(error.to_string()))
}
