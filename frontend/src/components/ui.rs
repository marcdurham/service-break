//! Small shared render helpers.

use shared::{Amenity, Parking, PlaceSource, PlaceSummary, PlaceType, Requirement};
use yew::prelude::*;

use crate::glue;

pub fn badge(place_type: PlaceType, source: PlaceSource) -> Html {
    html! {
        <div class="badge" style={format!("background:{}", place_type.color(source))}>
            <span class="mi">{place_type.icon()}</span>
        </div>
    }
}

pub fn stars(rating: f64) -> Html {
    let lit = rating.round() as i32;
    html! {
        <div class="stars">
            { for (0..5).map(|i| {
                let class = if i < lit { "mi lit" } else { "mi" };
                html! { <span class={class}>{"star"}</span> }
            }) }
        </div>
    }
}

/// Row of five faces for optionally scoring an aspect (1-5). Tapping the
/// currently-selected score again clears it back to "not rated".
pub fn aspect_score_picker(value: Option<i16>, on_change: &Callback<Option<i16>>) -> Html {
    html! {
        <div class="clean-row">
            { for (1..=5i16).map(|n| {
                let on = value.is_some_and(|v| n <= v);
                let onclick = {
                    let on_change = on_change.clone();
                    Callback::from(move |_| {
                        on_change.emit(if value == Some(n) { None } else { Some(n) })
                    })
                };
                html! {
                    <button class={if on { "clean-pick on" } else { "clean-pick" }} {onclick}>
                        <span class="mi">
                            {if on { "sentiment_very_satisfied" } else { "sentiment_neutral" }}
                        </span>
                    </button>
                }
            }) }
        </div>
    }
}

pub fn clean_label(clean_avg: Option<f64>) -> String {
    match clean_avg {
        Some(c) => format!("{c:.1}"),
        None => "New".to_owned(),
    }
}

pub fn parking_tag(parking: Parking) -> Html {
    let class = match parking {
        Parking::Easy => "tag tag-park-easy",
        Parking::Street => "tag tag-park-street",
        Parking::None => "tag tag-park-none",
    };
    html! {
        <span class={class}><span class="mi">{"local_parking"}</span>{parking.label()}</span>
    }
}

pub fn access_label(purchase_required: Requirement, code_required: Requirement) -> String {
    let mut parts = Vec::new();
    match purchase_required {
        Requirement::Yes => parts.push("Purchase required"),
        Requirement::Unknown => parts.push("Purchase unknown"),
        Requirement::No => {}
    }
    match code_required {
        Requirement::Yes => parts.push("Code required"),
        Requirement::Unknown => parts.push("Code unknown"),
        Requirement::No => {}
    }
    if parts.is_empty() {
        "Free · no purchase".to_owned()
    } else {
        parts.join(" · ")
    }
}

pub fn access_class(purchase_required: Requirement, code_required: Requirement) -> &'static str {
    if purchase_required == Requirement::Yes || code_required == Requirement::Yes {
        "tag tag-code"
    } else if purchase_required == Requirement::Unknown || code_required == Requirement::Unknown {
        "tag tag-unknown"
    } else {
        "tag tag-free"
    }
}

/// Floating toggle for the Overpass "unvisited places" layer — gray with a
/// slash through the marker-plus icon when off, solid blue when on. Shared
/// between the Map and List screens so both stay visually identical.
pub fn discover_button(on: bool, above_card: bool, on_toggle: &Callback<()>) -> Html {
    let onclick = {
        let cb = on_toggle.clone();
        Callback::from(move |_| cb.emit(()))
    };
    let class = match (on, above_card) {
        (true, true) => "discover on above-card",
        (true, false) => "discover on",
        (false, true) => "discover above-card",
        (false, false) => "discover",
    };
    html! {
        <button {class} {onclick}>
            <span class="mi">{"add_location"}</span>
        </button>
    }
}

/// Opens turn-by-turn directions in Google Maps to `(lat, lng)` — takes
/// coordinates rather than a `PlaceSummary` so it works for Overpass POIs
/// (which aren't app places) too.
pub fn open_directions(lat: f64, lng: f64) {
    let url = format!("https://www.google.com/maps/dir/?api=1&destination={lat},{lng}");
    glue::open_url(&url);
}

#[derive(Properties, PartialEq)]
pub struct FilterChipsProps {
    pub active_amenities: Vec<Amenity>,
    pub on_toggle_amenity: Callback<Amenity>,
}

/// Top-level filter chips: the things a place offers (Restroom, Coffee,
/// Food, Seating — all on by default). Place-type filtering lives in the
/// Filter modal (`FiltersSheet`) instead.
#[function_component(FilterChips)]
pub fn filter_chips(props: &FilterChipsProps) -> Html {
    // The task copy asks for singular "Restroom" on the chip, so the
    // labels here differ slightly from `Amenity::label`.
    let amenity_chips = [
        (Amenity::Restrooms, "Restroom"),
        (Amenity::Coffee, "Coffee"),
        (Amenity::Food, "Food"),
        (Amenity::Seating, "Seating"),
    ];
    html! {
        <div class="chipbar">
            <div class="chips sb-scroll">
                { for amenity_chips.into_iter().map(|(a, label)| {
                    let on = props.active_amenities.contains(&a);
                    let onclick = {
                        let cb = props.on_toggle_amenity.clone();
                        Callback::from(move |_| cb.emit(a))
                    };
                    html! {
                        <button class={if on { "chip on" } else { "chip" }} {onclick}>
                            <span class="mi">{a.icon()}</span>{label}
                        </button>
                    }
                }) }
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct ConfirmModalProps {
    pub title: AttrValue,
    pub body: AttrValue,
    #[prop_or("Delete".into())]
    pub confirm_label: AttrValue,
    #[prop_or("Cancel".into())]
    pub cancel_label: AttrValue,
    /// Shown on the confirm button (and disables it) while the action runs.
    #[prop_or_default]
    pub busy: bool,
    #[prop_or("Working…".into())]
    pub busy_label: AttrValue,
    pub on_confirm: Callback<()>,
    pub on_cancel: Callback<()>,
}

/// Shared destructive-action confirmation dialog. Used for every delete
/// button in the app so the prompt looks and behaves the same everywhere.
#[function_component(ConfirmModal)]
pub fn confirm_modal(props: &ConfirmModalProps) -> Html {
    let cancel = {
        let cb = props.on_cancel.clone();
        Callback::from(move |_| cb.emit(()))
    };
    let confirm = {
        let cb = props.on_confirm.clone();
        Callback::from(move |_| cb.emit(()))
    };
    html! {
        <div class="confirm-scrim" onclick={cancel.clone()}>
            <div class="confirm-dialog" onclick={|e: MouseEvent| e.stop_propagation()}>
                <div class="confirm-title">{props.title.clone()}</div>
                <div class="confirm-body">{props.body.clone()}</div>
                <div class="confirm-actions">
                    <button class="confirm-cancel" onclick={cancel}>
                        {props.cancel_label.clone()}
                    </button>
                    <button class="confirm-ok" disabled={props.busy} onclick={confirm}>
                        {if props.busy { props.busy_label.clone() } else { props.confirm_label.clone() }}
                    </button>
                </div>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct PlaceCardProps {
    pub place: PlaceSummary,
    pub on_open: Callback<uuid::Uuid>,
}

/// List-style card for a place, as in the List tab of the mockup.
#[function_component(PlaceCard)]
pub fn place_card(props: &PlaceCardProps) -> Html {
    let p = &props.place;
    let onclick = {
        let cb = props.on_open.clone();
        let id = p.id;
        Callback::from(move |_| cb.emit(id))
    };
    html! {
        <button class="card" {onclick}>
            { badge(p.place_type, PlaceSource::App) }
            <div class="card-main">
                <div class="card-top">
                    <div style="min-width:0">
                        <div class="card-name">{&p.name}</div>
                        <div class="card-type">{p.place_type.label()}</div>
                    </div>
                    if let Some(d) = p.distance_mi {
                        <div class="card-dist">
                            <div class="dist-num">{shared::fmt_distance_mi(d)}</div>
                            <div class="dist-unit">{"MI"}</div>
                        </div>
                    }
                </div>
                <div class="tag-row">
                    <span class="tag tag-clean">
                        <span class="mi">{"mop"}</span>{clean_label(p.clean_avg)}{" clean"}
                    </span>
                    <span class="tag tag-door">
                        <span class="mi">{"directions_walk"}</span>{shared::door_short(p.door_ft)}
                    </span>
                    { parking_tag(p.parking) }
                </div>
            </div>
        </button>
    }
}
