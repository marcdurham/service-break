//! Bindings to the small JS glue layer in index.html (Leaflet map + geolocation).

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// Creates the Leaflet map with OpenStreetMap tiles inside `#el_id`.
    /// `on_select` is called with a place id when a pin is tapped.
    #[wasm_bindgen(js_name = sbInitMap)]
    pub fn sb_init_map(el_id: &str, lat: f64, lng: f64, zoom: f64, on_select: &JsValue);

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

    /// Browser geolocation: `ok(lat, lng)` or `err(message)`.
    #[wasm_bindgen(js_name = sbLocate)]
    pub fn sb_locate(ok: &JsValue, err: &JsValue);

    /// Writes text to the clipboard, best-effort.
    #[wasm_bindgen(js_name = sbCopyText)]
    pub fn sb_copy_text(text: &str);

    /// Opens the native share sheet with `text` and the app's URL; returns
    /// `true` if it did, `false` if it fell back to copying the clipboard.
    #[wasm_bindgen(js_name = sbShareInvite)]
    pub fn sb_share_invite(text: &str) -> bool;
}

/// Opens a URL in a new tab (directions handoff to Google Maps / OSM).
pub fn open_url(url: &str) {
    if let Some(win) = web_sys::window() {
        let _ = win.open_with_url_and_target(url, "_blank");
    }
}
