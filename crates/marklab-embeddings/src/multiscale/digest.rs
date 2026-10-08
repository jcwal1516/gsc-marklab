use marklab_project::{ArtifactId, ContentDigest};

use crate::digest::FramedDigest;

pub(super) struct LogicalDigest(FramedDigest);

impl LogicalDigest {
    pub(super) fn new(domain: &[u8]) -> Self {
        let mut digest = FramedDigest::new();
        digest.field(domain);
        Self(digest)
    }

    pub(super) fn bytes(&mut self, value: &[u8]) {
        self.0.field(value);
    }

    pub(super) fn text(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    pub(super) fn u8(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    pub(super) fn u32(&mut self, value: u32) {
        self.bytes(&value.to_be_bytes());
    }

    pub(super) fn u64(&mut self, value: u64) {
        self.bytes(&value.to_be_bytes());
    }

    pub(super) fn i64(&mut self, value: i64) {
        self.bytes(&value.to_be_bytes());
    }

    pub(super) fn f32(&mut self, value: f32) {
        self.bytes(&value.to_bits().to_be_bytes());
    }

    pub(super) fn f64_bits(&mut self, bits: u64) {
        self.bytes(&bits.to_be_bytes());
    }

    pub(super) fn artifact_id(&mut self, value: ArtifactId) {
        self.bytes(value.digest().as_bytes());
    }

    pub(super) fn content_digest(&mut self, value: ContentDigest) {
        self.bytes(value.as_bytes());
    }

    pub(super) fn array_len(
        &mut self,
        length: usize,
    ) -> Result<(), super::error::MultiscaleEmbeddingError> {
        let length = u64::try_from(length)
            .map_err(|_| super::error::MultiscaleEmbeddingError::SizeOverflow)?;
        self.u64(length);
        Ok(())
    }

    pub(super) fn finish(self) -> ContentDigest {
        self.0.finish()
    }
}
