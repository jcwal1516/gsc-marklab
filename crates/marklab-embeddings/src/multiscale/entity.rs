use marklab_data::{HierarchyId, PatchId, RegionId, SlideId};

/// Closed typed entity families supported by multiscale embedding tables.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EmbeddingEntityKind {
    /// Image or feature patch.
    Patch,
    /// Declared spatial region.
    Region,
    /// Owning slide.
    Slide,
}

impl EmbeddingEntityKind {
    pub(crate) fn wire_name(self) -> &'static str {
        match self {
            Self::Patch => "patch",
            Self::Region => "region",
            Self::Slide => "slide",
        }
    }
}

pub(crate) trait EntitySpec: Clone + Eq + Ord {
    const KIND: EmbeddingEntityKind;
    const EXPECTED_FORMAT: &'static str;
    const EXPECTED_DOMAIN: &'static [u8];
    const TABLE_DOMAIN: &'static [u8];

    fn as_str(&self) -> &str;
    fn parse(value: String) -> Result<Self, ()>;
    fn hierarchy_id(&self) -> HierarchyId;
}

macro_rules! impl_entity_spec {
    ($id:ty, $kind:ident, $expected_format:literal, $expected_domain:literal, $table_domain:literal) => {
        impl EntitySpec for $id {
            const KIND: EmbeddingEntityKind = EmbeddingEntityKind::$kind;
            const EXPECTED_FORMAT: &'static str = $expected_format;
            const EXPECTED_DOMAIN: &'static [u8] = $expected_domain;
            const TABLE_DOMAIN: &'static [u8] = $table_domain;

            fn as_str(&self) -> &str {
                self.as_str()
            }

            fn parse(value: String) -> Result<Self, ()> {
                Self::new(value).map_err(|_| ())
            }

            fn hierarchy_id(&self) -> HierarchyId {
                HierarchyId::from(self.clone())
            }
        }
    };
}

impl_entity_spec!(
    PatchId,
    Patch,
    "marklab.expected_patch_set",
    b"marklab-expected-patch-set-logical-v1",
    b"marklab-patch-embedding-logical-v1"
);
impl_entity_spec!(
    RegionId,
    Region,
    "marklab.expected_region_set",
    b"marklab-expected-region-set-logical-v1",
    b"marklab-region-embedding-logical-v1"
);
impl_entity_spec!(
    SlideId,
    Slide,
    "marklab.expected_slide_set",
    b"marklab-expected-slide-set-logical-v1",
    b"marklab-slide-embedding-logical-v1"
);
