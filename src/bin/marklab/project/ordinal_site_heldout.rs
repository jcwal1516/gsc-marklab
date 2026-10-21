use std::borrow::Cow;

use marklab_bayes::OrdinalSiteHeldoutComparison;
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;

use super::cli_schema::OrdinalSiteHeldoutComparisonProjectArgs;
use super::{
    bayes,
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_RESULT_BYTES,
};

const INPUT_KIND: &str = "application/vnd.marklab.source.ordinal-site-heldout+csv;version=1";

pub(super) fn run(args: OrdinalSiteHeldoutComparisonProjectArgs) -> Result<(), CliError> {
    let OrdinalSiteHeldoutComparisonProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    const LABEL: &str = "ordinal heldout source";
    let source = Source::identify(model.input.clone(), INPUT_KIND)?;
    let prepared = bayes::prepare_ordinal_site_heldout(model.clone())?;
    source.ensure_unchanged(LABEL)?;
    let spec_bytes = serde_json::to_vec(&prepared.spec)?;
    let contract = NodeContract {
        id: "ordinal-site-heldout-comparison",
        kind: "ordinal_site_heldout_comparison",
        version: 1,
        configuration_digest: ContentDigest::from_framed([
            b"marklab-ordinal-site-heldout-v1".as_slice(),
            spec_bytes.as_slice(),
        ]),
        execution_policy: Cow::Borrowed(
            b"serial;leave-one-entire-site-out;training-fold-only-fit;bounded-coordinate-search;fixed-model.smoothing",
        ),
        implementation_identity: "marklab-project-ordinal-site-heldout-comparison-node-v1",
        output_kind: "application/vnd.marklab.ordinal-site-heldout-comparison+json;version=1",
        codec: Codec::Json,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| bayes::execute_ordinal_site_heldout(prepared).map_err(NodeError::execution),
        |prepared, output: &OrdinalSiteHeldoutComparison| {
            if output.validates(&prepared.spec) {
                Ok(())
            } else {
                Err(NodeError::decode(CliError::Input(
                    "ordinal heldout output differs from cache-bound request".into(),
                )))
            }
        },
    )?;
    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.ordinal_site_heldout_comparison", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "ordinal-site-heldout-comparison",
        &output_path,
        &run.output,
        run.cache_status,
    )
}
