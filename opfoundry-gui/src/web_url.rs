#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn derive_ws_url(protocol: &str, host: &str) -> Option<String> {
    let normalized = protocol.trim_end_matches(':');
    let scheme = match normalized {
        "https" | "wss" => "wss",
        "http" | "ws" => "ws",
        _ => return None,
    };

    let host = host.trim();
    if host.is_empty() {
        return None;
    }

    Some(format!("{scheme}://{host}"))
}

#[cfg(test)]
mod tests {
    use super::derive_ws_url;

    #[test]
    fn maps_http_locations_to_ws() {
        assert_eq!(
            derive_ws_url("http:", "example.com"),
            Some("ws://example.com".to_string())
        );
    }

    #[test]
    fn maps_https_locations_to_wss() {
        assert_eq!(
            derive_ws_url("https:", "example.com:443"),
            Some("wss://example.com:443".to_string())
        );
    }

    #[test]
    fn returns_none_for_unknown_protocol() {
        assert_eq!(derive_ws_url("file:", ""), None);
    }

    #[test]
    fn trims_trailing_colon_variants() {
        assert_eq!(
            derive_ws_url("https", "example.com"),
            Some("wss://example.com".to_string())
        );
    }
}
