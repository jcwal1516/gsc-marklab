//! Architecture resource ceilings scaled to the host's physical memory.
//!
//! The CLI includes this file too (`#[path]`), so its recipe read cap and the library's
//! admission check follow one rule. Physical memory, not currently free memory, sets the
//! ceiling, so the same recipe is admitted the same way on a given machine.

/// Bytes the preparation memory estimate charges per recipe byte.
pub(crate) const ESTIMATE_BYTES_PER_INPUT_BYTE: u64 = 16;
/// The fixed memory-budget ceiling before host scaling; never lowered by it.
const MINIMUM_BUDGET_CEILING: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AdmittedResources {
    /// Largest `limits.memory_budget_bytes` a recipe may declare.
    pub memory_budget_ceiling: u64,
    /// Largest recipe accepted, the most the budget ceiling can cover at the estimate's rate.
    pub recipe_bytes: u64,
}

pub(crate) fn admitted_resources() -> AdmittedResources {
    admitted_for(physical_memory_bytes())
}

/// Three quarters of physical memory, at least the former fixed 8 GiB; 8 GiB when unknown.
pub(crate) fn admitted_for(physical_bytes: Option<u64>) -> AdmittedResources {
    let ceiling = physical_bytes
        .map_or(MINIMUM_BUDGET_CEILING, |bytes| bytes / 4 * 3)
        .max(MINIMUM_BUDGET_CEILING);
    AdmittedResources {
        memory_budget_ceiling: ceiling,
        recipe_bytes: ceiling / ESTIMATE_BYTES_PER_INPUT_BYTE,
    }
}

#[cfg(target_os = "macos")]
fn physical_memory_bytes() -> Option<u64> {
    let output = std::process::Command::new("/usr/sbin/sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    std::str::from_utf8(&output.stdout)
        .ok()?
        .trim()
        .parse()
        .ok()
}

#[cfg(target_os = "linux")]
fn physical_memory_bytes() -> Option<u64> {
    parse_meminfo(&std::fs::read_to_string("/proc/meminfo").ok()?)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn physical_memory_bytes() -> Option<u64> {
    None
}

/// `MemTotal` from Linux `/proc/meminfo`, which reports kibibytes as `kB`.
#[cfg(any(target_os = "linux", test))]
fn parse_meminfo(text: &str) -> Option<u64> {
    let line = text.lines().find(|line| line.starts_with("MemTotal:"))?;
    let mut fields = line.split_whitespace().skip(1);
    let kibibytes: u64 = fields.next()?.parse().ok()?;
    (fields.next()? == "kB")
        .then(|| kibibytes.checked_mul(1024))
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn ceilings_scale_with_physical_memory_but_never_drop_below_the_fixed_ceiling() {
        for (physical, ceiling) in [
            (Some(64 * GIB), 48 * GIB),
            (Some(24 * GIB), 18 * GIB),
            (Some(4 * GIB), 8 * GIB),
            (None, 8 * GIB),
        ] {
            let admitted = admitted_for(physical);
            assert_eq!(admitted.memory_budget_ceiling, ceiling, "{physical:?}");
            assert_eq!(admitted.recipe_bytes, ceiling / 16, "{physical:?}");
        }
    }

    #[test]
    fn meminfo_total_is_read_in_bytes_and_malformed_text_is_unknown() {
        let text = "MemFree:  1000 kB\nMemTotal:       16384000 kB\nBuffers: 1 kB\n";
        assert_eq!(parse_meminfo(text), Some(16_384_000 * 1024));
        for malformed in [
            "",
            "MemTotal: lots kB",
            "MemTotal: 16384000",
            "MemTotal: 1 MB",
        ] {
            assert_eq!(parse_meminfo(malformed), None, "{malformed:?}");
        }
    }
}
