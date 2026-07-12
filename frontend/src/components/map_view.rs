use serde::Serialize;
use shared::{Amenity, PlaceSummary, PlaceType, PlacesQuery};
use uuid::Uuid;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::app::FALLBACK_CENTER;
use crate::components::ui::{self, FilterChips};
use crate::{api, glue};

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
    /// A place to center on instead of the user ("Show on map").
    pub focus: Option<(f64, f64)>,
    pub filters_active: bool,
    pub active_types: Vec<PlaceType>,
    pub active_amenities: Vec<Amenity>,
    pub on_select: Callback<Uuid>,
    pub on_open: Callback<Uuid>,
    pub on_open_filters: Callback<()>,
    pub on_toggle_type: Callback<PlaceType>,
    pub on_toggle_amenity: Callback<Amenity>,
    pub on_recenter: Callback<()>,
}

#[function_component(MapView)]
pub fn map_view(props: &MapViewProps) -> Html {
    let query = use_state(String::new);
    // None while idle or a fetch is pending; Some(list) once results arrived.
    let results = use_state(|| None::<Vec<PlaceSummary>>);
    // Bumped on every keystroke so stale debounced fetches drop themselves.
    let search_gen = use_mut_ref(|| 0u32);

    {
        let results = results.clone();
        let search_gen = search_gen.clone();
        let origin = props.origin;
        use_effect_with((*query).clone(), move |query| {
            *search_gen.borrow_mut() += 1;
            let generation = *search_gen.borrow();
            let query = query.trim().to_owned();
            if query.is_empty() {
                results.set(None);
                return;
            }
            wasm_bindgen_futures::spawn_local(async move {
                gloo_timers::future::TimeoutFuture::new(250).await;
                if *search_gen.borrow() != generation {
                    return;
                }
                let q = PlacesQuery {
                    q: Some(query),
                    lat: origin.map(|(lat, _)| lat),
                    lng: origin.map(|(_, lng)| lng),
                    ..PlacesQuery::default()
                };
                if let Ok(list) = api::fetch_places(&q).await {
                    if *search_gen.borrow() == generation {
                        results.set(Some(list));
                    }
                }
            });
        });
    }

    let on_search_input = {
        let query = query.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                query.set(el.value());
            }
        })
    };
    let clear_search = {
        let query = query.clone();
        let results = results.clone();
        Callback::from(move |_| {
            query.set(String::new());
            results.set(None);
        })
    };
    let pick_result = {
        let query = query.clone();
        let results = results.clone();
        let on_open = props.on_open.clone();
        Callback::from(move |p: PlaceSummary| {
            glue::sb_fly_to(p.lat, p.lng, 16.0);
            query.set(String::new());
            results.set(None);
            on_open.emit(p.id);
        })
    };
    let searching = !query.trim().is_empty();
    // The pin-tap callback must survive re-renders; the JS side holds one
    // function for the map's lifetime, reading the latest Yew callback
    // through this ref.
    let select_ref = use_mut_ref(|| props.on_select.clone());
    *select_ref.borrow_mut() = props.on_select.clone();
    let closure_slot = use_mut_ref(|| None::<Closure<dyn Fn(String)>>);

    {
        let closure_slot = closure_slot.clone();
        let (center, zoom) = match props.focus {
            Some(f) => (f, 16.0),
            None => (props.origin.unwrap_or(FALLBACK_CENTER), 14.0),
        };
        use_effect_with((), move |()| {
            let closure = Closure::<dyn Fn(String)>::new(move |id: String| {
                if let Ok(id) = Uuid::parse_str(&id) {
                    select_ref.borrow().emit(id);
                }
            });
            glue::sb_init_map("sb-map", center.0, center.1, zoom, closure.as_ref().unchecked_ref());
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

    // Follow the user's position unless a place is being focused; the map
    // was already initialized at the right center, so neither branch needs
    // to fly on first render.
    let first_render = use_mut_ref(|| true);
    use_effect_with((props.origin, props.focus), move |(origin, focus)| {
        let first = std::mem::replace(&mut *first_render.borrow_mut(), false);
        if let Some((lat, lng)) = origin {
            glue::sb_set_user(*lat, *lng);
            if focus.is_none() && !first {
                glue::sb_fly_to(*lat, *lng, 14.0);
            }
        }
        if let Some((lat, lng)) = focus {
            if !first {
                glue::sb_fly_to(*lat, *lng, 16.0);
            }
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
                        <input
                            class="search-input"
                            placeholder="Search shops, malls, parks…"
                            value={(*query).clone()}
                            oninput={on_search_input}
                        />
                        if searching {
                            <button class="search-clear" onclick={clear_search}>
                                <span class="mi">{"close"}</span>
                            </button>
                        }
                    </div>
                    <button class="icon-btn" onclick={open_filters}>
                        <span class="mi">{"tune"}</span>
                        if props.filters_active {
                            <span class="filter-dot"></span>
                        }
                    </button>
                </div>
                if searching {
                    if let Some(list) = (*results).clone() {
                        { search_results(&list, &pick_result) }
                    }
                } else {
                    <FilterChips
                        active_types={props.active_types.clone()}
                        active_amenities={props.active_amenities.clone()}
                        on_toggle_type={props.on_toggle_type.clone()}
                        on_toggle_amenity={props.on_toggle_amenity.clone()}
                    />
                }
            </div>

            <button
                class={if featured.is_some() { "recenter above-card" } else { "recenter" }}
                onclick={recenter}
            >
                <span class="mi">{"my_location"}</span>
            </button>

            if let Some(p) = featured {
                { featured_card(p, props) }
            }
        </div>
    }
}

fn search_results(list: &[PlaceSummary], pick: &Callback<PlaceSummary>) -> Html {
    if list.is_empty() {
        return html! {
            <div class="search-results">
                <div class="sresult-empty">{"No places found"}</div>
            </div>
        };
    }
    html! {
        <div class="search-results sb-scroll">
            { for list.iter().map(|p| {
                let onclick = {
                    let pick = pick.clone();
                    let place = p.clone();
                    Callback::from(move |_| pick.emit(place.clone()))
                };
                html! {
                    <button class="sresult" key={p.id.to_string()} {onclick}>
                        <span
                            class="sresult-icon mi"
                            style={format!("background:{}", p.place_type.color())}
                        >
                            {p.place_type.icon()}
                        </span>
                        <span class="sresult-main">
                            <span class="sresult-name">{&p.name}</span>
                            <span class="sresult-sub">
                                {p.place_type.label()}
                                if !p.address.is_empty() {
                                    {format!(" · {}", p.address)}
                                }
                            </span>
                        </span>
                        if let Some(d) = p.distance_mi {
                            <span class="sresult-dist">{format!("{} mi", shared::fmt_distance_mi(d))}</span>
                        }
                    </button>
                }
            }) }
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
