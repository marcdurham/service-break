use serde::Serialize;
use shared::{PlaceSummary, PlaceType};
use uuid::Uuid;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use yew::prelude::*;

use crate::app::FALLBACK_CENTER;
use crate::components::ui::{self, TypeChips};
use crate::glue;

#[derive(Serialize)]
struct Pin {
    id: String,
    lat: f64,
    lng: f64,
    color: &'static str,
    icon: &'static str,
    name: String,
    selected: bool,
}

#[derive(Properties, PartialEq)]
pub struct MapViewProps {
    pub places: Vec<PlaceSummary>,
    pub selected: Option<Uuid>,
    pub origin: Option<(f64, f64)>,
    pub filters_active: bool,
    pub active_types: Vec<PlaceType>,
    pub on_select: Callback<Uuid>,
    pub on_open: Callback<Uuid>,
    pub on_open_filters: Callback<()>,
    pub on_toggle_type: Callback<PlaceType>,
    pub on_recenter: Callback<()>,
}

#[function_component(MapView)]
pub fn map_view(props: &MapViewProps) -> Html {
    // The pin-tap callback must survive re-renders; the JS side holds one
    // function for the map's lifetime, reading the latest Yew callback
    // through this ref.
    let select_ref = use_mut_ref(|| props.on_select.clone());
    *select_ref.borrow_mut() = props.on_select.clone();
    let closure_slot = use_mut_ref(|| None::<Closure<dyn Fn(String)>>);

    {
        let closure_slot = closure_slot.clone();
        let center = props.origin.unwrap_or(FALLBACK_CENTER);
        use_effect_with((), move |()| {
            let closure = Closure::<dyn Fn(String)>::new(move |id: String| {
                if let Ok(id) = Uuid::parse_str(&id) {
                    select_ref.borrow().emit(id);
                }
            });
            glue::sb_init_map("sb-map", center.0, center.1, 14.0, closure.as_ref().unchecked_ref());
            *closure_slot.borrow_mut() = Some(closure);
            || glue::sb_destroy_map()
        });
    }

    use_effect_with(
        (props.places.clone(), props.selected),
        |(places, selected)| {
            let pins: Vec<Pin> = places
                .iter()
                .map(|p| Pin {
                    id: p.id.to_string(),
                    lat: p.lat,
                    lng: p.lng,
                    color: p.place_type.color(),
                    icon: p.place_type.icon(),
                    name: p.name.clone(),
                    selected: *selected == Some(p.id),
                })
                .collect();
            if let Ok(json) = serde_json::to_string(&pins) {
                glue::sb_set_pins(&json);
            }
        },
    );

    use_effect_with(props.origin, |origin| {
        if let Some((lat, lng)) = origin {
            glue::sb_set_user(*lat, *lng);
            glue::sb_fly_to(*lat, *lng, 14.0);
        }
    });

    let featured = props
        .selected
        .and_then(|id| props.places.iter().find(|p| p.id == id))
        .or_else(|| props.places.first());

    let open_filters = {
        let cb = props.on_open_filters.clone();
        Callback::from(move |_| cb.emit(()))
    };
    let recenter = {
        let cb = props.on_recenter.clone();
        Callback::from(move |_| cb.emit(()))
    };

    html! {
        <div class="map-screen">
            <div id="sb-map"></div>

            <div class="map-top">
                <div class="map-top-row">
                    <div class="searchbar">
                        <span class="mi">{"search"}</span>
                        {"Search coffee, parks, restrooms…"}
                    </div>
                    <button class="icon-btn" onclick={open_filters}>
                        <span class="mi">{"tune"}</span>
                        if props.filters_active {
                            <span class="filter-dot"></span>
                        }
                    </button>
                </div>
                <TypeChips active={props.active_types.clone()} on_toggle={props.on_toggle_type.clone()} />
            </div>

            <button class="recenter" onclick={recenter}>
                <span class="mi">{"my_location"}</span>
            </button>

            if let Some(p) = featured {
                { featured_card(p, props) }
            }
        </div>
    }
}

fn featured_card(p: &PlaceSummary, props: &MapViewProps) -> Html {
    let near_tag = if props.selected == Some(p.id) { "Selected" } else { "Nearest to you" };
    let open_card = {
        let cb = props.on_open.clone();
        let id = p.id;
        Callback::from(move |_| cb.emit(id))
    };
    let directions = {
        let place = p.clone();
        Callback::from(move |_| ui::open_directions(&place))
    };
    html! {
        <div class="sel-wrap" key={p.id.to_string()}>
            <button class="sel-card" onclick={open_card}>
                { ui::badge(p.place_type) }
                <div class="sel-main">
                    <div class="sel-tags">
                        <span class="near-tag">{near_tag}</span>
                        <span class="open-dot"></span>
                        <span class="open-label">{p.place_type.label()}</span>
                    </div>
                    <div class="sel-name">{&p.name}</div>
                    <div class="sel-stats">
                        <span class="stat">
                            <span class="mi">{"mop"}</span>{ui::clean_label(p.clean_avg)}
                        </span>
                        <span class="stat">
                            <span class="mi walk">{"directions_walk"}</span>{shared::door_short(p.door_ft)}
                        </span>
                    </div>
                </div>
                if let Some(d) = p.distance_mi {
                    <div class="dist-col">
                        <div class="dist-num">{shared::fmt_distance_mi(d)}</div>
                        <div class="dist-unit">{"MI"}</div>
                    </div>
                }
            </button>
            <button class="directions-btn" onclick={directions}>
                <span class="mi">{"directions"}</span>{format!("Directions to {}", p.name)}
            </button>
        </div>
    }
}
