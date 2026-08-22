/// Domain namespaces for deterministic permutation streams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub(crate) enum SeedEndpoint {
    SpectrumBinary = 0x7370_6563_5f62_696e,
    SpectrumContinuous = 0x7370_6563_5f63_6f6e,
    SpectrumStratified = 0x7370_6563_5f73_7472,
    SpectrumComponent = 0x7370_6563_5f63_6d70,
    Anisotropy = 0x616e_6973_6f74_726f,
    MarkPairCovariance = 0x6d61_726b_5f63_6f76,
    ScaleEnergy = 0x7363_616c_655f_656e,
    ResidualTerritory = 0x7265_7369_645f_7465,
    CrossInteraction = 0x6372_6f73_735f_696e,
    NeighborhoodEnrichment = 0x6e65_6967_685f_656e,
    NeighborhoodStratifiedEnrichment = 0x6e65_6967_685f_7374,
    #[cfg(any(feature = "cli", test))]
    PooledBinDifference = 0x6375_7276_655f_6469,
}

/// Derive a stable seed from a base seed, endpoint namespace, and run index.
///
/// The mapping is independent of execution order and thread count. Endpoint
/// tags are part of the reproducibility contract and must not be reused.
pub(crate) fn derive_seed(base_seed: u64, endpoint: SeedEndpoint, index: usize) -> u64 {
    let endpoint_seed = splitmix64(base_seed ^ endpoint as u64);
    splitmix64(endpoint_seed ^ index as u64)
}

/// SplitMix64 mixer used by deterministic shuffles and seed derivation.
pub(crate) fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = value;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_derivation_is_stable_and_domain_separated() {
        assert_eq!(
            derive_seed(123, SeedEndpoint::SpectrumBinary, 7),
            0x70ce_2c19_611b_fa86
        );
        assert_ne!(
            derive_seed(123, SeedEndpoint::SpectrumBinary, 7),
            derive_seed(123, SeedEndpoint::SpectrumContinuous, 7)
        );
        assert_ne!(
            derive_seed(123, SeedEndpoint::SpectrumBinary, 7),
            derive_seed(123, SeedEndpoint::SpectrumBinary, 8)
        );
    }

    #[test]
    fn every_declared_endpoint_has_a_distinct_stream() {
        let endpoints = [
            SeedEndpoint::SpectrumBinary,
            SeedEndpoint::SpectrumContinuous,
            SeedEndpoint::SpectrumStratified,
            SeedEndpoint::SpectrumComponent,
            SeedEndpoint::Anisotropy,
            SeedEndpoint::MarkPairCovariance,
            SeedEndpoint::ScaleEnergy,
            SeedEndpoint::ResidualTerritory,
            SeedEndpoint::CrossInteraction,
            SeedEndpoint::NeighborhoodEnrichment,
            SeedEndpoint::NeighborhoodStratifiedEnrichment,
            SeedEndpoint::PooledBinDifference,
        ];
        let mut seeds = endpoints
            .into_iter()
            .map(|endpoint| derive_seed(42, endpoint, 0))
            .collect::<Vec<_>>();
        seeds.sort_unstable();
        seeds.dedup();

        assert_eq!(seeds.len(), endpoints.len());
    }
}
