use serde::Serialize;
use shared::{
    Amenity, BBox, MapsLinkResult, OverpassPoi, PlaceSource, PlaceSummary, PlacesQuery,
};
use uuid::Uuid;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::app::FALLBACK_CENTER;
use crate::components::ui::{self, FilterChips};
use crate::maps_link::looks_like_google_maps_link;
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
    pub active_amenities: Vec<Amenity>,
    /// Overpass POIs currently in view; only rendered/searched when
    /// `show_unvisited` is on.
    pub overpass_places: Vec<OverpassPoi>,
    pub show_unvisited: bool,
    /// Index into [`shared::MARKER_DENSITY_LEVELS`] capping how many
    /// Overpass POI pins render at once.
    pub marker_density: u8,
    pub on_select: Callback<Uuid>,
    pub on_open: Callback<Uuid>,
    /// Fired when a tapped pin or search result is an unpromoted Overpass
    /// POI, so the app can open the lightweight preview.
    pub on_open_poi: Callback<OverpassPoi>,
    pub on_open_filters: Callback<()>,
    pub on_toggle_amenity: Callback<Amenity>,
    pub on_recenter: Callback<()>,
    /// Fired when a pasted Google Maps link resolved, so the app can open
    /// the "Add a place" page prefilled.
    pub on_maps_link: Callback<MapsLinkResult>,
    /// Reports the live Leaflet viewport (once on init, then debounced on
    /// every pan/zoom) so the app can fetch the Overpass POI layer.
    pub on_bounds_changed: Callback<BBox>,
    pub on_toast: Callback<String>,
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
        let query_handle = query.clone();
        let on_maps_link = props.on_maps_link.clone();
        let on_toast = props.on_toast.clone();
        use_effect_with((*query).clone(), move |query| {
            *search_gen.borrow_mut() += 1;
            let generation = *search_gen.borrow();
            let query = query.trim().to_owned();
            if query.is_empty() {
                results.set(None);
                return;
            }
            if looks_like_google_maps_link(&query) {
                results.set(None);
                let query_handle = query_handle.clone();
                let on_maps_link = on_maps_link.clone();
                let on_toast = on_toast.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api::resolve_maps_link(&query).await {
                        Ok(place) => {
                            if *search_gen.borrow() == generation {
                                query_handle.set(String::new());
                                results.set(None);
                                on_maps_link.emit(place);
                            }
                        }
                        Err(msg) => {
                            if *search_gen.borrow() == generation {
                                on_toast.emit(msg);
                            }
                        }
                    }
                });
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
    let pick_poi = {
        let query = query.clone();
        let results = results.clone();
        let on_open_poi = props.on_open_poi.clone();
        Callback::from(move |p: OverpassPoi| {
            glue::sb_fly_to(p.lat, p.lng, 16.0);
            query.set(String::new());
            results.set(None);
            on_open_poi.emit(p);
        })
    };
    let searching = !query.trim().is_empty();
    let poi_matches: Vec<OverpassPoi> = if searching && props.show_unvisited {
        let needle = query.trim().to_lowercase();
        props
            .overpass_places
            .iter()
            .filter(|p| p.name.to_lowercase().contains(&needle))
            .cloned()
            .collect()
    } else {
        Vec::new()
    };

    // The pin-tap and bounds-changed callbacks must survive re-renders; the
    // JS side holds one function each for the map's lifetime, reading the
    // latest Yew callback (and, for pin taps, the latest Overpass POI list)
    // through these refs.
    let select_ref = use_mut_ref(|| props.on_select.clone());
    *select_ref.borrow_mut() = props.on_select.clone();
    let open_poi_ref = use_mut_ref(|| props.on_open_poi.clone());
    *open_poi_ref.borrow_mut() = props.on_open_poi.clone();
    let overpass_lookup_ref = use_mut_ref(|| props.overpass_places.clone());
    *overpass_lookup_ref.borrow_mut() = props.overpass_places.clone();
    let bounds_ref = use_mut_ref(|| props.on_bounds_changed.clone());
    *bounds_ref.borrow_mut() = props.on_bounds_changed.clone();
    let closure_slot = use_mut_ref(|| None::<Closure<dyn Fn(String)>>);
    let bounds_closure_slot = use_mut_ref(|| None::<Closure<dyn Fn(f64, f64, f64, f64)>>);

    {
        let closure_slot = closure_slot.clone();
        let bounds_closure_slot = bounds_closure_slot.clone();
        let (center, zoom) = match props.focus {
            Some(f) => (f, 16.0),
            None => (props.origin.unwrap_or(FALLBACK_CENTER), 14.0),
        };
        use_effect_with((), move |()| {
            let closure = Closure::<dyn Fn(String)>::new(move |id: String| {
                match Uuid::parse_str(&id) {
                    Ok(id) => select_ref.borrow().emit(id),
                    Err(_) => {
                        if let Some(poi) =
                            overpass_lookup_ref.borrow().iter().find(|p| p.id == id)
                        {
                            open_poi_ref.borrow().emit(poi.clone());
                        }
                    }
                }
            });
            let bounds_closure =
                Closure::<dyn Fn(f64, f64, f64, f64)>::new(move |min_lat, min_lng, max_lat, max_lng| {
                    bounds_ref.borrow().emit(BBox { min_lat, min_lng, max_lat, max_lng });
                });
            glue::sb_init_map(
                "sb-map",
                center.0,
                center.1,
                zoom,
                closure.as_ref().unchecked_ref(),
                bounds_closure.as_ref().unchecked_ref(),
            );
            *closure_slot.borrow_mut() = Some(closure);
            *bounds_closure_slot.borrow_mut() = Some(bounds_closure);
            || glue::sb_destroy_map()
        });
    }

    use_effect_with(
        (
            props.places.clone(),
            props.selected,
            props.overpass_places.clone(),
            props.show_unvisited,
            props.marker_density,
        ),
        |(places, selected, overpass_places, show_unvisited, marker_density)| {
            let mut pins: Vec<Pin> = places
                .iter()
                .map(|p| Pin {
                    id: p.id.to_string(),
                    lat: p.lat,
                    lng: p.lng,
                    color: p.place_type.color(PlaceSource::App),
                    icon: p.place_type.icon(),
                    name: p.name.clone(),
                    selected: *selected == Some(p.id),
                })
                .collect();
            if *show_unvisited {
                let cap = shared::marker_density_cap(*marker_density) as usize;
                pins.extend(thinned_indices(overpass_places.len(), cap).into_iter().map(|i| {
                    let p = &overpass_places[i];
                    Pin {
                        id: p.id.clone(),
                        lat: p.lat,
                        lng: p.lng,
                        color: p.place_type.color(PlaceSource::Overpass),
                        icon: p.place_type.icon(),
                        name: p.name.clone(),
                        selected: false,
                    }
                }));
            }
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
    let search_places = (*results).clone().unwrap_or_default();

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
                    { search_results(&search_places, &poi_matches, &pick_result, &pick_poi) }
                } else {
                    <FilterChips
                        active_amenities={props.active_amenities.clone()}
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

/// Picks up to `cap` evenly-spaced indices out of `0..len`, so a thinned
/// marker layer stays spread across the whole list instead of bunching at
/// the start (which — for viewport-ordered Overpass results — would bias
/// toward one corner of the map).
fn thinned_indices(len: usize, cap: usize) -> Vec<usize> {
    if cap == 0 || len <= cap {
        return (0..len).collect();
    }
    let step = len as f64 / cap as f64;
    let mut out = Vec::with_capacity(cap);
    let mut next = 0.0f64;
    for i in 0..len {
        if out.len() >= cap {
            break;
        }
        if i as f64 >= next {
            out.push(i);
            next += step;
        }
    }
    out
}

fn search_results(
    places: &[PlaceSummary],
    pois: &[OverpassPoi],
    pick_place: &Callback<PlaceSummary>,
    pick_poi: &Callback<OverpassPoi>,
) -> Html {
    if places.is_empty() && pois.is_empty() {
        return html! {
            <div class="search-results">
                <div class="sresult-empty">{"No places found"}</div>
            </div>
        };
    }
    html! {
        <div class="search-results sb-scroll">
            { for places.iter().map(|p| place_result_row(p, pick_place)) }
            { for pois.iter().map(|p| poi_result_row(p, pick_poi)) }
        </div>
    }
}

fn place_result_row(p: &PlaceSummary, pick: &Callback<PlaceSummary>) -> Html {
    let onclick = {
        let pick = pick.clone();
        let place = p.clone();
        Callback::from(move |_| pick.emit(place.clone()))
    };
    html! {
        <button class="sresult" key={p.id.to_string()} {onclick}>
            <span
                class="sresult-icon mi"
                style={format!("background:{}", p.place_type.color(PlaceSource::App))}
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
}

/// Like [`place_result_row`] but for an unpromoted Overpass POI — same
/// layout, blue/gray badge.
fn poi_result_row(p: &OverpassPoi, pick: &Callback<OverpassPoi>) -> Html {
    let onclick = {
        let pick = pick.clone();
        let poi = p.clone();
        Callback::from(move |_| pick.emit(poi.clone()))
    };
    html! {
        <button class="sresult" key={p.id.clone()} {onclick}>
            <span
                class="sresult-icon mi"
                style={format!("background:{}", p.place_type.color(PlaceSource::Overpass))}
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
}

fn featured_card(p: &PlaceSummary, props: &MapViewProps) -> Html {
    let near_tag = if props.selected == Some(p.id) { "Selected" } else { "Nearest to you" };
    let open_card = {
        let cb = props.on_open.clone();
        let id = p.id;
        Callback::from(move |_| cb.emit(id))
    };
    let directions = {
        let (lat, lng) = (p.lat, p.lng);
        Callback::from(move |_| ui::open_directions(lat, lng))
    };
    html! {
        <div class="sel-wrap" key={p.id.to_string()}>
            <button class="sel-card" onclick={open_card}>
                { ui::badge(p.place_type, PlaceSource::App) }
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
