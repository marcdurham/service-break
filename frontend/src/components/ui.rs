//! Small shared render helpers.

use shared::{Parking, PlaceSummary, PlaceType};
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

pub fn access_label(purchase_required: bool, code_required: bool) -> &'static str {
    match (purchase_required, code_required) {
        (true, true) => "Purchase & code",
        (true, false) => "Purchase required",
        (false, true) => "Code / key required",
        (false, false) => "Free · no purchase",
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
        (PlaceType::Coffee, "Coffee"),
        (PlaceType::Grocery, "Grocery"),
        (PlaceType::Bookstore, "Books"),
        (PlaceType::Park, "Parks"),
        (PlaceType::Gas, "Gas"),
        (PlaceType::Restroom, "Restrooms"),
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
