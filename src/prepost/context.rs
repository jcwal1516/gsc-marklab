use crate::output::StatusFlag;

pub(super) struct ComparisonContext {
    pub(super) anatomically_comparable: bool,
    pub(super) status_flags: Vec<StatusFlag>,
}

impl ComparisonContext {
    pub(super) fn from_metadata(
        pre_case_id: &str,
        pre_timepoint: &str,
        pre_protein: &str,
        post_case_id: &str,
        post_timepoint: &str,
        post_protein: &str,
    ) -> Self {
        let anatomically_comparable = pre_case_id == post_case_id
            && pre_protein == post_protein
            && pre_timepoint.eq_ignore_ascii_case("pre")
            && post_timepoint.eq_ignore_ascii_case("post");
        let status_flags = (!anatomically_comparable)
            .then_some(StatusFlag::PrePostNotAnatomicallyComparable)
            .into_iter()
            .collect();
        Self {
            anatomically_comparable,
            status_flags,
        }
    }

    pub(super) fn marked_interpretation(&self) -> String {
        if self.anatomically_comparable {
            "The post-treatment section shows descriptive change in coarse-scale organization of the configured mark field compared with the pretreatment section.".into()
        } else {
            "The pre/post sections are not anatomically comparable; numeric deltas are emitted as diagnostics only.".into()
        }
    }

    pub(super) fn multimodal_interpretation(&self) -> String {
        if self.anatomically_comparable {
            "The post-treatment section shows descriptive change in multimodal neighborhood organization compared with the pretreatment section.".into()
        } else {
            "The pre/post multimodal sections are not anatomically comparable; numeric deltas are emitted as diagnostics only.".into()
        }
    }
}
