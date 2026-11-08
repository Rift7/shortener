use rand::Rng;

const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Paths the shortener owns; a generated or custom slug may never shadow them.
const RESERVED: &[&str] = &[
    "api",
    "healthz",
    "health",
    "healthcheck",
    "static",
    "assets",
    "favicon.ico",
    "robots.txt",
    "sitemap.xml",
    "admin",
    "index.html",
];

pub fn random(len: usize) -> String {
    let mut rng = rand::rng();
    (0..len)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect()
}

pub fn is_reserved(slug: &str) -> bool {
    let lower = slug.to_ascii_lowercase();
    RESERVED.iter().any(|r| *r == lower)
}

/// Custom slugs must be URL-safe, non-empty, and reasonably short.
pub fn is_valid_custom(slug: &str) -> bool {
    if slug.is_empty() || slug.len() > 64 {
        return false;
    }
    if slug.starts_with('-') || slug.starts_with('_') {
        return false;
    }
    slug.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_is_alphanumeric_and_sized() {
        let s = random(7);
        assert_eq!(s.len(), 7);
        assert!(s.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn random_is_not_constant() {
        assert_ne!(random(12), random(12));
    }

    #[test]
    fn reserved_lookup_is_case_insensitive() {
        assert!(is_reserved("API"));
        assert!(is_reserved("favicon.ico"));
        assert!(!is_reserved("apix"));
    }

    #[test]
    fn custom_slug_validation() {
        assert!(is_valid_custom("promo2026"));
        assert!(is_valid_custom("a_b-c"));
        assert!(!is_valid_custom(""));
        assert!(!is_valid_custom("-leading"));
        assert!(!is_valid_custom("has space"));
        assert!(!is_valid_custom("sla/sh"));
        assert!(!is_valid_custom(&"x".repeat(65)));
    }
}
