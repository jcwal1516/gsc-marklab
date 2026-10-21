pub(crate) mod alloc;
pub(crate) mod finite;
pub(crate) mod matrix;
pub(crate) mod seeds;
pub(crate) mod stats;

/// Locked `geo` release whose polygon overlap defines translation edge correction.
///
/// It is part of translation cache identities so a `geo` upgrade cannot reuse old results.
pub(crate) const GEO_IDENTITY: &str = "geo=0.33.1";

#[cfg(test)]
mod tests {
    #[test]
    fn geo_identity_matches_the_locked_geo_release() {
        let lock = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock"));
        let locked = lock
            .split("[[package]]")
            .find(|entry| entry.contains("\nname = \"geo\"\n"))
            .and_then(|entry| {
                entry
                    .lines()
                    .find_map(|line| line.strip_prefix("version = "))
            })
            .expect("geo is locked");

        assert_eq!(
            super::GEO_IDENTITY,
            format!("geo={}", locked.trim_matches('"'))
        );
    }
}
