pub(crate) fn is_lower_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn all_finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_identity_requires_exact_lowercase_ascii_hex() {
        assert!(is_lower_hex_sha256(&"0123456789abcdef".repeat(4)));
        assert!(!is_lower_hex_sha256(&"0123456789ABCDEF".repeat(4)));
        assert!(!is_lower_hex_sha256(&"0".repeat(63)));
        assert!(!is_lower_hex_sha256(&format!("{}g", "0".repeat(63))));
    }

    #[test]
    fn finite_slice_accepts_empty_and_rejects_each_nonfinite_class() {
        assert!(all_finite(&[]));
        assert!(all_finite(&[-0.0, f64::MIN, f64::MAX]));
        assert!(!all_finite(&[f64::NAN]));
        assert!(!all_finite(&[f64::INFINITY]));
        assert!(!all_finite(&[f64::NEG_INFINITY]));
    }
}
