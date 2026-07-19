//! Lightweight preview for an Overpass POI (raw OpenStreetMap data, not yet
//! an app place) — deliberately not the full `DetailView`, which assumes
//! reviews and an edit history that don't exist for it yet. Any of
//! Save/Rate/Edit *promotes* the POI into a normal app place first; the
//! caller (`App::on_poi_promoted`) then hands off to the normal
//! `DetailView` flow, where the pin/badge flips from blue/gray to brown.

use shared::{OverpassPoi, PlaceDetail, PlaceSource};
use yew::prelude::*;

use crate::api;
use crate::components::ui;

#[derive(Properties, PartialEq)]
pub struct PoiDetailViewProps {
    pub poi: OverpassPoi,
    pub device_id: String,
    /// Saving, rating, or editing requires an account; when false these
    /// actions emit `on_require_login` instead of promoting.
    pub logged_in: bool,
    pub on_require_login: Callback<()>,
    /// Like `on_require_login`, but for the Edit action specifically: the
    /// app routes it to this POI's `/poi/.../edit` URL, so the user comes
    /// back to editing this exact place once they've signed in.
    pub on_edit_signed_out: Callback<()>,
    pub on_close: Callback<()>,
    pub on_promoted: Callback<PlaceDetail>,
    pub on_toast: Callback<String>,
}

/// Promotes the POI into an app place, optionally saving it too (the
/// single-tap "Save" action), then hands the result to the caller.
async fn promote_and_notify(
    poi_id: String,
    device_id: String,
    then_save: bool,
    busy: UseStateHandle<bool>,
    on_promoted: Callback<PlaceDetail>,
    on_toast: Callback<String>,
) {
    match api::promote_overpass_poi(&poi_id, &device_id).await {
        Ok(detail) => {
            if then_save {
                let _ = api::save_place(&device_id, detail.summary.id).await;
                on_toast.emit("Saved — thanks, scout!".to_owned());
            } else {
                on_toast.emit("Added — thanks, scout!".to_owned());
            }
            on_promoted.emit(detail);
        }
        Err(msg) => {
            busy.set(false);
            on_toast.emit(msg);
        }
    }
}

#[function_component(PoiDetailView)]
pub fn poi_detail_view(props: &PoiDetailViewProps) -> Html {
    let busy = use_state(|| false);
    let poi = &props.poi;

    let close = {
        let cb = props.on_close.clone();
        Callback::from(move |_| cb.emit(()))
    };
    let directions = {
        let (lat, lng) = (poi.lat, poi.lng);
        Callback::from(move |_| ui::open_directions(lat, lng))
    };

    // Save/Rate/Edit all promote the POI first; only "Save" also saves the
    // resulting place in the same tap. Rate/Edit just land the user on the
    // normal `DetailView`, which already has the review composer and Edit
    // button, rather than auto-opening either sub-form here.
    let promote_action = |then_save: bool, signed_out: Callback<()>| {
        let poi_id = poi.id.clone();
        let device_id = props.device_id.clone();
        let logged_in = props.logged_in;
        let on_promoted = props.on_promoted.clone();
        let on_toast = props.on_toast.clone();
        let busy = busy.clone();
        Callback::from(move |_: MouseEvent| {
            if !logged_in {
                signed_out.emit(());
                return;
            }
            if *busy {
                return;
            }
            busy.set(true);
            wasm_bindgen_futures::spawn_local(promote_and_notify(
                poi_id.clone(),
                device_id.clone(),
                then_save,
                busy.clone(),
                on_promoted.clone(),
                on_toast.clone(),
            ));
        })
    };
    let save = promote_action(true, props.on_require_login.clone());
    let rate = promote_action(false, props.on_require_login.clone());
    let edit = promote_action(false, props.on_edit_signed_out.clone());

    html! {
        <div class="detail-overlay">
            <div class="detail-scroll sb-scroll">
                <div class="hero" style={format!("background:{}", poi.place_type.color(PlaceSource::Overpass))}>
                    <span class="mi">{poi.place_type.icon()}</span>
                    <div class="hero-btns">
                        <button class="hero-btn" onclick={close}>
                            <span class="mi">{"arrow_back"}</span>
                        </button>
                    </div>
                </div>
                <div class="detail-body">
                    <div class="detail-meta">
                        <span class="detail-type">{poi.place_type.label()}</span>
                        <span class="open-dot"></span>
                        <span class="detail-hours-label">{"Not yet in the app"}</span>
                    </div>
                    <div class="detail-name">{&poi.name}</div>

                    if !poi.address.is_empty() {
                        <div class="addr-card">
                            <span class="mi">{"location_on"}</span>
                            <div class="addr-line">{&poi.address}</div>
                        </div>
                    }

                    <div class="empty-sub mb-md">
                        {"From OpenStreetMap. Save, rate, or edit it to add it to the app."}
                    </div>

                    <div class="action-row">
                        <button class="directions-btn" onclick={directions}>
                            <span class="mi">{"directions"}</span>{"Directions"}
                        </button>
                        <button class="rate-btn" onclick={save} disabled={*busy}>
                            <span class="mi">{"bookmark_border"}</span>{"Save"}
                        </button>
                        <button class="rate-btn" onclick={rate} disabled={*busy}>
                            <span class="mi">{"rate_review"}</span>{"Rate"}
                        </button>
                        <button class="rate-btn" onclick={edit} disabled={*busy}>
                            <span class="mi">{"edit"}</span>{"Edit"}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    }
}
