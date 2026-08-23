use std::io::Write;

use marklab_project::{ContentDigest, ContentDigestWriter};

pub(crate) struct FramedDigest {
    writer: ContentDigestWriter,
}

impl FramedDigest {
    pub(crate) fn new() -> Self {
        Self {
            writer: ContentDigest::builder(),
        }
    }

    pub(crate) fn field(&mut self, value: &[u8]) {
        let length = (value.len() as u128).to_be_bytes();
        self.writer
            .write_all(&length)
            .expect("in-memory digest writer cannot fail");
        self.writer
            .write_all(value)
            .expect("in-memory digest writer cannot fail");
    }

    pub(crate) fn finish(self) -> ContentDigest {
        self.writer.finish().0
    }
}

pub(crate) fn canonical_positive_zero(value: f32) -> f32 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}
