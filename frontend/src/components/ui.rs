//! Small shared render helpers.

use shared::{Parking, PlaceSummary, PlaceType, Requirement};
use yew::prelude::*;

use crate::glue;

pub fn badge(place_type: PlaceType) -> Html {
    html! {
        <div class="badge" style={format!("background:{}", place_type.color())}>
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

/// Opens turn-by-turn directions in Google Maps.
pub fn open_directions(place: &PlaceSummary) {
    let url = format!(
        "https://www.google.com/maps/dir/?api=1&destination={},{}",
        place.lat, place.lng
    );
    glue::open_url(&url);
}

#[derive(Properties, PartialEq)]
pub struct TypeChipsProps {
    pub active: Vec<PlaceType>,
    pub on_toggle: Callback<PlaceType>,
}

/// Horizontal scroller of place-type filter chips.
#[function_component(TypeChips)]
pub fn type_chips(props: &TypeChipsProps) -> Html {
    let chips = [
        (PlaceType::Shop, "Shop"),
        (PlaceType::Store, "Store"),
        (PlaceType::Mall, "Mall"),
        (PlaceType::Park, "Park"),
        (PlaceType::Public, "Public"),
        (PlaceType::Hall, "Hall"),
    ];
    html! {
        <div class="chips sb-scroll">
            { for chips.into_iter().map(|(t, label)| {
                let on = props.active.contains(&t);
                let onclick = {
                    let cb = props.on_toggle.clone();
                    Callback::from(move |_| cb.emit(t))
                };
                html! {
                    <button class={if on { "chip on" } else { "chip" }} {onclick}>
                        <span class="mi">{t.icon()}</span>{label}
                    </button>
                }
            }) }
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
            { badge(p.place_type) }
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
