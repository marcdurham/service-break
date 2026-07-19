//! Bindings to the small JS glue layer in index.html (Leaflet map + geolocation).

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// Creates the Leaflet map with OpenStreetMap tiles inside `#el_id`.
    /// `on_select` is called with a place id when a pin is tapped.
    /// `on_bounds_changed(min_lat, min_lng, max_lat, max_lng)` fires once
    /// immediately with the initial viewport, then again on every
    /// (debounced) pan/zoom -- drives the Overpass POI layer.
    #[wasm_bindgen(js_name = sbInitMap)]
    pub fn sb_init_map(
        el_id: &str,
        lat: f64,
        lng: f64,
        zoom: f64,
        on_select: &JsValue,
        on_bounds_changed: &JsValue,
    );

    #[wasm_bindgen(js_name = sbDestroyMap)]
    pub fn sb_destroy_map();

    /// Replaces all pins. Takes a JSON array of
    /// `{id, lat, lng, color, icon, name, selected}`.
    #[wasm_bindgen(js_name = sbSetPins)]
    pub fn sb_set_pins(pins_json: &str);

    #[wasm_bindgen(js_name = sbSetUser)]
    pub fn sb_set_user(lat: f64, lng: f64);

    #[wasm_bindgen(js_name = sbFlyTo)]
    pub fn sb_fly_to(lat: f64, lng: f64, zoom: f64);

    /// Creates the location-picker map inside `#el_id`. Tapping the map drops
    /// (or moves) a pin and calls `on_pick(lat, lng)`. When `has_marker` is
    /// true the pin starts at the given center.
    #[wasm_bindgen(js_name = sbInitPickMap)]
    pub fn sb_init_pick_map(
        el_id: &str,
        lat: f64,
        lng: f64,
        zoom: f64,
        has_marker: bool,
        on_pick: &JsValue,
    );

    #[wasm_bindgen(js_name = sbDestroyPickMap)]
    pub fn sb_destroy_pick_map();

    /// Shows (or moves) a marker for the user's own location on the
    /// location-picker map, distinct from the dropped pin.
    #[wasm_bindgen(js_name = sbSetPickUser)]
    pub fn sb_set_pick_user(lat: f64, lng: f64);

    /// Browser geolocation: `ok(lat, lng)` or `err(message)`.
    #[wasm_bindgen(js_name = sbLocate)]
    pub fn sb_locate(ok: &JsValue, err: &JsValue);

    /// Writes text to the clipboard, best-effort.
    #[wasm_bindgen(js_name = sbCopyText)]
    pub fn sb_copy_text(text: &str);

    /// Saves `text` to the user's device as a JSON file named `filename`
    /// (used for admin backup downloads).
    #[wasm_bindgen(js_name = sbDownload)]
    pub fn sb_download(filename: &str, text: &str);

    /// Opens the native share sheet with `text` and a link back to the app
    /// carrying `code`; returns `true` if it did, `false` if it fell back to
    /// copying the clipboard.
    #[wasm_bindgen(js_name = sbShareInvite)]
    pub fn sb_share_invite(text: &str, code: &str) -> bool;
}

/// Opens a URL in a new tab (directions handoff to Google Maps / OSM).
pub fn open_url(url: &str) {
    if let Some(win) = web_sys::window() {
        let _ = win.open_with_url_and_target(url, "_blank");
    }
}
