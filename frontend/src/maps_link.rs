//! Recognizing Google Maps place links in user-provided text, whether
//! pasted whole into the search box or bundled with other text by
//! Android's share sheet.

/// Whether `q` looks like a link to a Google Maps place (a `maps.app.goo.gl`
/// short link, an older `goo.gl/maps` short link, or a full
/// `google.com/maps` URL) rather than a place-search query.
pub fn looks_like_google_maps_link(q: &str) -> bool {
    let q = q.trim();
    (q.starts_with("http://") || q.starts_with("https://"))
        && (q.contains("maps.app.goo.gl") || q.contains("goo.gl/maps") || q.contains("google.com/maps"))
}

/// First whitespace-delimited token in `s` that looks like a Google Maps
/// link. Android's share sheet often bundles a place name and the URL
/// together (e.g. `"Some Place\nhttps://maps.app.goo.gl/xyz"`).
pub fn extract_google_maps_link(s: &str) -> Option<String> {
    s.split_whitespace().find(|tok| looks_like_google_maps_link(tok)).map(str::to_owned)
}
