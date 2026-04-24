#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

/// Reads a browser local-storage value.
#[cfg(target_arch = "wasm32")]
pub(crate) fn browser_local_storage_item(key: &str) -> Option<String> {
    web_sys::window()?
        .local_storage()
        .ok()
        .flatten()
        .and_then(|storage| storage.get_item(key).ok().flatten())
}

/// Reads one query parameter from the current browser URL.
#[cfg(target_arch = "wasm32")]
pub(crate) fn browser_query_param(key: &str) -> Option<String> {
    web_sys::window()?
        .location()
        .search()
        .ok()
        .and_then(|search| query_param(&search, key))
}

/// Reads one un-decoded query parameter from a URL search string.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn query_param(search: &str, key: &str) -> Option<String> {
    let search = search.strip_prefix('?').unwrap_or(search);
    search.split('&').find_map(|pair| {
        let (pair_key, pair_value) = pair.split_once('=').unwrap_or((pair, ""));
        (pair_key == key).then(|| pair_value.to_owned())
    })
}

/// Parses truthy demo flags accepted by URL parameters.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn parse_bool_flag(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Parses a finite f64 used by inspection URL parameters.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn parse_finite_f64(value: &str) -> Option<f64> {
    let parsed = value.trim().parse::<f64>().ok()?;
    parsed.is_finite().then_some(parsed)
}

/// Parses a finite positive f32 used by inspection URL parameters.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn parse_positive_f32(value: &str) -> Option<f32> {
    let parsed = value.trim().parse::<f32>().ok()?;
    (parsed.is_finite() && parsed > 0.0).then_some(parsed)
}

/// Accepts only public Mapbox tokens for browser-distributed demos.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn normalized_mapbox_public_token(token: String) -> Option<String> {
    let token = token.trim();
    token.starts_with("pk.").then(|| token.to_owned())
}
