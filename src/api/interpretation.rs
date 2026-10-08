use crate::output::{AnalysisStatus, Interpretation, InterpretationClass, StatusFlag};

pub(super) fn interpretation_for(
    status_flags: &[StatusFlag],
    status: AnalysisStatus,
    low_k_excess: Option<f64>,
) -> Interpretation {
    if status != AnalysisStatus::Ok {
        if status_flags.contains(&StatusFlag::InternalControlFailureOverlap)
            || status_flags.contains(&StatusFlag::StainGradientSuspect)
        {
            return Interpretation {
                class: InterpretationClass::SuppressedQcArtifact,
                text: "The configured mark field overlaps recorded input-QC artifact structure; interpretation is suppressed.".into(),
            };
        }
        return Interpretation {
            class: InterpretationClass::Suppressed,
            text: "Numeric spatial diagnostics are emitted, but interpretation is suppressed by validation status.".into(),
        };
    }

    let Some(low_k_excess) = low_k_excess else {
        return Interpretation {
            class: InterpretationClass::InsufficientData,
            text: "Spectrum inference is unavailable at interpretable scales; no spatial-pattern classification is made.".into(),
        };
    };

    if low_k_excess >= 1.25 {
        Interpretation {
            class: InterpretationClass::CoarseExcess,
            text: "The configured mark field shows coarse-scale spectral excess relative to fixed-position random labeling.".into(),
        }
    } else if low_k_excess <= 0.80 {
        Interpretation {
            class: InterpretationClass::LowFrequencySuppression,
            text: "The configured mark field shows low-frequency spectral suppression relative to fixed-position random labeling.".into(),
        }
    } else {
        Interpretation {
            class: InterpretationClass::RandomLike,
            text: "The configured mark field is random-like relative to fixed-position random labeling at the analyzed scales.".into(),
        }
    }
}
