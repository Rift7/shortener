use url::Url;

use crate::error::AppError;

const MAX_URL_LEN: usize = 2048;

/// Normalize and validate a user-supplied target URL.
///
/// - trims whitespace
/// - assumes `https://` when the input has no scheme (`example.com/x`)
/// - accepts only http/https with a host
pub fn normalize(raw: &str) -> Result<String, AppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("url must not be empty".into()));
    }
    if trimmed.len() > MAX_URL_LEN {
        return Err(AppError::BadRequest(format!(
            "url must be at most {MAX_URL_LEN} characters"
        )));
    }

    let candidate = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };

    let parsed =
        Url::parse(&candidate).map_err(|e| AppError::BadRequest(format!("invalid url: {e}")))?;

    match parsed.scheme() {
        "http" | "https" => {}
        other => {
            return Err(AppError::BadRequest(format!(
                "unsupported url scheme '{other}'; only http and https are allowed"
            )))
        }
    }

    match parsed.host_str() {
        Some(h) if !h.is_empty() => {}
        _ => return Err(AppError::BadRequest("url must include a host".into())),
    }

    // Reject credentials-in-URL: agents pass tokens by mistake and it leaks them.
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::BadRequest(
            "url must not embed credentials".into(),
        ));
    }

    Ok(parsed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_https_scheme() {
        assert_eq!(normalize("example.com").unwrap(), "https://example.com/");
    }

    #[test]
    fn keeps_http() {
        assert_eq!(
            normalize("http://example.com/a?b=1").unwrap(),
            "http://example.com/a?b=1"
        );
    }

    #[test]
    fn rejects_other_schemes() {
        assert!(normalize("ftp://example.com").is_err());
        assert!(normalize("javascript:alert(1)").is_err());
        assert!(normalize("file:///etc/passwd").is_err());
    }

    #[test]
    fn rejects_empty() {
        assert!(normalize("   ").is_err());
    }

    #[test]
    fn rejects_embedded_credentials() {
        assert!(normalize("https://user:pass@example.com").is_err());
    }

    #[test]
    fn rejects_oversize() {
        let long = format!("https://example.com/{}", "a".repeat(2100));
        assert!(normalize(&long).is_err());
    }
}
