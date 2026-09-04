//! SHA-256 fingerprinting of canonical text.

use sha2::{Digest, Sha256};

/// Compute the SHA-256 fingerprint of `canonical` text and return it as
/// 64 lowercase hex characters.
///
/// The choice of hash is not load-bearing: any deterministic function of
/// the canonical text would do. SHA-256 is convenient because it is
/// collision-resistant for our small input space and produces a stable
/// 64-char hex string that fits naturally in reports and storage.
pub fn fingerprint(canonical: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let bytes = hasher.finalize();
    let mut out = String::with_capacity(64);
    for b in bytes {
        // Manual hex formatting avoids an extra dependency on `hex`.
        let hi = HEX[(b >> 4) as usize];
        let lo = HEX[(b & 0x0f) as usize];
        out.push(hi);
        out.push(lo);
    }
    out
}

const HEX: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_64_lowercase_hex_chars() {
        let h = fingerprint("anything");
        assert_eq!(h.len(), 64);
        assert!(h
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn fingerprint_is_deterministic() {
        let a = fingerprint("error: connection refused at /var/log/app.log:42");
        let b = fingerprint("error: connection refused at /var/log/app.log:42");
        assert_eq!(a, b);
    }

    #[test]
    fn distinct_inputs_produce_distinct_fingerprints() {
        let a = fingerprint("error A");
        let b = fingerprint("error B");
        assert_ne!(a, b);
    }

    #[test]
    fn empty_input_still_yields_a_hash() {
        let h = fingerprint("");
        assert_eq!(h.len(), 64);
    }
}
