use std::{fmt::Write as _, fs, path::Path};

use marklab_policy::Maturity;

use super::model::MultiplexStudyResult;
use crate::{output::OutputTransaction, ContentDigest, MarklabError, Result};

/// Atomically publish the scientific JSON, claim-bounded Markdown report and content manifest.
///
/// Uses the existing output transaction: a nonempty destination is never overwritten. Partial
/// study invocations have no result to publish. This does not promote the result's evidence.
pub fn publish_multiplex_study(result: &MultiplexStudyResult, output: &Path) -> Result<()> {
    result.validate_claim_state()?;
    let mut json = serde_json::to_vec_pretty(result)?;
    json.push(b'\n');
    let report = report(result);
    let manifest = serde_json::to_vec_pretty(&serde_json::json!({
        "format": "marklab.multiplex_study_manifest", "version": 1,
        "artifacts": [
            {"path":"result.json", "sha256":ContentDigest::from_bytes(&json).to_string(), "bytes":json.len()},
            {"path":"report.md", "sha256":ContentDigest::from_bytes(report.as_bytes()).to_string(), "bytes":report.len()}
        ]
    }))?;
    let transaction = OutputTransaction::new(output)?;
    for (name, bytes) in [
        ("result.json", json.as_slice()),
        ("report.md", report.as_bytes()),
        ("manifest.json", manifest.as_slice()),
    ] {
        let path = transaction.staging_path().join(name);
        fs::write(&path, bytes).map_err(|error| MarklabError::io(&path, error))?;
    }
    transaction.commit()
}

fn report(result: &MultiplexStudyResult) -> String {
    let mut text = String::from("# Multiplex spatial study\n\n");
    let maturity = match result.maturity.result_maturity {
        Maturity::Experimental => "experimental",
        _ => "unsupported_for_claim",
    };
    writeln!(
        text,
        "{} independent patients; {} slides. Maturity: **{maturity}**.\n",
        result.patients.len(),
        result.slides.len()
    )
    .expect("String write");
    text.push_str("The inference unit is the patient. Each patient's slides receive equal weight; patients receive equal weight in the prespecified group comparison.\n\n");
    writeln!(text, "Radius: {} µm. Weight policy: `{}`. The selected channel family contains {} Moran/Geary endpoints.\n", result.design.radius_um, result.design.weight_policy, result.endpoint_names.len()).expect("String write");
    text.push_str("**Missingness:** each channel uses its observed-cell subgraph. This conditions the spatial estimand on availability; it does not establish missing-at-random sampling. A required unavailable slide endpoint blocks complete-family patient inference.\n\n");
    if result.inference.status == "available" {
        text.push_str("## Patient comparison\n\nEffects are the declared group A minus group B. P-values use whole-patient, single-step Max-T adjustment over the complete endpoint family.\n\n| Endpoint | Mean difference | Adjusted p-value |\n|---|---:|---:|\n");
        for endpoint in &result.inference.endpoints {
            // Channel identifiers obey ScalarMarkId's bounded token grammar.
            writeln!(
                text,
                "| `{}` | {:.8} | {:.8} |",
                endpoint.endpoint, endpoint.effect_group_a_minus_group_b, endpoint.adjusted_p_value
            )
            .expect("String write");
        }
    } else {
        let unavailable = result
            .patients
            .iter()
            .filter(|patient| patient.values.is_none())
            .count();
        writeln!(text, "**Patient inference is unavailable.** {unavailable} patients contain a required unavailable slide endpoint. The JSON retains every patient, slide and diagnostic; no inferential claim is reported.\n").expect("String write");
        if unavailable == 0 {
            text.push_str("The canonical patient calculation reported a numerical or diagnostic failure. See its JSON reason and reported attempt counts.\n\n");
        }
    }
    text.push_str("\n## Evidence and claim limits\n\n");
    for limit in &result.evidence_limits {
        writeln!(text, "- {limit}").expect("String write");
    }
    text.push_str("\nThe result JSON preserves exact assay declarations, measurement status, source/panel/window identities, denominators, patient reduction and diagnostic decisions. The manifest binds the published files.\n");
    text
}
