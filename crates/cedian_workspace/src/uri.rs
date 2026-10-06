//! `cedian://` virtual files (plan §11: host URI schemes).
//!
//! `cedian://buffer/<path>` reads/writes go through the `BufferStore` — this
//! is how OMP tools (LSP/diagnostics later) read Zed state without a backend
//! seam. Reserved OMP schemes (`local://`, `skill://`, `artifact://`,
//! `security://`, `mcp://`, …) are rejected at parse: cedian never shadows one.
//! OMP `edit` does NOT target host URIs (upstream) — writes come from the
//! `write` tool via host tools, or explicit `cedian://` writes here.

/// Reserved OMP built-in schemes — never registered, never shadowed.
pub const RESERVED_SCHEMES: &[&str] = &["local", "skill", "artifact", "security", "mcp"];

/// A parsed `cedian://` URL: kind + path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CedianUri {
    /// `buffer`, `diagnostics`, … (extensible per host service).
    pub kind: String,
    /// Path within the kind namespace (leading `/` stripped).
    pub path: String,
}

/// URI failures (host answers `isError` on these).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UriError {
    /// Not a `cedian://` URL.
    WrongScheme { got: String },
    /// Missing kind or path.
    BadShape { url: String },
    /// Reserved OMP scheme — refused, never shadowed.
    Reserved { scheme: String },
}

impl std::fmt::Display for UriError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongScheme { got } => write!(f, "not a cedian:// URL: {got}"),
            Self::BadShape { url } => write!(f, "bad cedian:// shape: {url}"),
            Self::Reserved { scheme } => write!(f, "scheme reserved by OMP: {scheme}://"),
        }
    }
}

impl std::error::Error for UriError {}

/// Parse a URL into a [`CedianUri`]. Accepts only `cedian://kind/path`.
pub fn parse_cedian_uri(url: &str) -> Result<CedianUri, UriError> {
    let (scheme, rest) = url.split_once("://").ok_or_else(|| UriError::BadShape {
        url: url.to_string(),
    })?;
    let scheme = scheme.to_lowercase();
    if RESERVED_SCHEMES.contains(&scheme.as_str()) {
        return Err(UriError::Reserved { scheme });
    }
    if scheme != "cedian" {
        return Err(UriError::WrongScheme { got: scheme });
    }
    let (kind, path) = rest.split_once('/').ok_or_else(|| UriError::BadShape {
        url: url.to_string(),
    })?;
    if kind.is_empty() || path.is_empty() {
        return Err(UriError::BadShape {
            url: url.to_string(),
        });
    }
    Ok(CedianUri {
        kind: kind.to_string(),
        path: path.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_url_parses() {
        let u = parse_cedian_uri("cedian://buffer/src/main.rs").unwrap();
        assert_eq!(u.kind, "buffer");
        assert_eq!(u.path, "src/main.rs");
    }

    #[test]
    fn reserved_never_shadowed() {
        for scheme in ["local", "skill", "artifact", "security", "mcp"] {
            let err = parse_cedian_uri(&format!("{scheme}://x/y")).unwrap_err();
            assert_eq!(
                err,
                UriError::Reserved {
                    scheme: scheme.to_string()
                }
            );
        }
    }

    #[test]
    fn wrong_scheme_and_shape_rejected() {
        assert!(matches!(
            parse_cedian_uri("db://x/y"),
            Err(UriError::WrongScheme { .. })
        ));
        assert!(matches!(
            parse_cedian_uri("cedian://nokind"),
            Err(UriError::BadShape { .. })
        ));
    }
}
