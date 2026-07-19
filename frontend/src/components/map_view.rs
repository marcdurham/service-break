use std::collections::HashMap;

use gloo_storage::{LocalStorage, Storage};
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
    /// Index into [`shared::MARKER_DENSITY_LEVELS`] controlling how closely
    /// packed Overpass POI pins may get (one pin per grid cell).
    pub marker_density: u8,
    /// The live viewport (mirrors what `on_bounds_changed` last reported);
    /// used only to size the density grid to the current zoom.
    pub bounds: Option<BBox>,
    pub on_select: Callback<Uuid>,
    pub on_open: Callback<Uuid>,
    /// Fired when a tapped pin or search result is an unpromoted Overpass
    /// POI, so the app can open the lightweight preview.
    pub on_open_poi: Callback<OverpassPoi>,
    pub on_open_filters: Callback<()>,
    pub on_toggle_amenity: Callback<Amenity>,
    pub on_recenter: Callback<()>,
    /// Fired when the "Discover" button is tapped, flipping `show_unvisited`.
    pub on_toggle_unvisited: Callback<()>,
    /// Fired when the featured card is dismissed -- its close button, or a
    /// tap on empty map -- so the app can clear the current selection.
    pub on_deselect: Callback<()>,
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
    // Explicitly dismissed (close button or empty-map tap) -- hides the
    // featured card even though `selected`/`places.first()` would otherwise
    // still resolve to one. Cleared as soon as a new place is picked.
    let dismissed = use_state(|| false);
    // One-time tip bubble pointing at the Discover button, shown until the
    // user interacts with anything (dismissed via a window-level click
    // listener below, so any tap dismisses it, not just its own controls).
    let show_tip = use_state(|| !LocalStorage::get::<bool>("sb_seen_discover_tip").unwrap_or(false));
    // Whether the current search has been submitted (Enter, or tapping the
    // search icon) -- while true, the map's pins are narrowed to just the
    // matches instead of showing the live-suggestion dropdown. Editing the
    // query again drops back to live suggestions.
    let submitted = use_state(|| false);

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
        let submitted = submitted.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                query.set(el.value());
            }
            submitted.set(false);
        })
    };
    let submit_search: Callback<()> = {
        let query = query.clone();
        let submitted = submitted.clone();
        Callback::from(move |()| {
            if !query.trim().is_empty() {
                submitted.set(true);
            }
        })
    };
    let submit_search_click = {
        let submit_search = submit_search.clone();
        Callback::from(move |_: MouseEvent| submit_search.emit(()))
    };
    let on_search_keydown = {
        let submit_search = submit_search.clone();
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" {
                e.prevent_default();
                submit_search.emit(());
            }
        })
    };
    let clear_search = {
        let query = query.clone();
        let results = results.clone();
        let submitted = submitted.clone();
        Callback::from(move |_| {
            query.set(String::new());
            results.set(None);
            submitted.set(false);
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
    let search_places = (*results).clone().unwrap_or_default();
    // Total match count for the badge near the search box; `None` while a
    // debounced fetch is still pending, so the count doesn't flash to 0.
    let search_count = searching.then(|| results.as_ref().map(|r| r.len() + poi_matches.len())).flatten();

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
    let deselect_ref = use_mut_ref(|| props.on_deselect.clone());
    *deselect_ref.borrow_mut() = props.on_deselect.clone();
    let closure_slot = use_mut_ref(|| None::<Closure<dyn Fn(String)>>);
    let bounds_closure_slot = use_mut_ref(|| None::<Closure<dyn Fn(f64, f64, f64, f64)>>);
    let empty_click_closure_slot = use_mut_ref(|| None::<Closure<dyn Fn()>>);

    {
        let closure_slot = closure_slot.clone();
        let bounds_closure_slot = bounds_closure_slot.clone();
        let empty_click_closure_slot = empty_click_closure_slot.clone();
        let dismissed_for_select = dismissed.clone();
        let dismissed_for_empty_click = dismissed.clone();
        let (center, zoom) = match props.focus {
            Some(f) => (f, 16.0),
            None => (props.origin.unwrap_or(FALLBACK_CENTER), 14.0),
        };
        use_effect_with((), move |()| {
            let closure = Closure::<dyn Fn(String)>::new(move |id: String| {
                dismissed_for_select.set(false);
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
            let empty_click_closure = Closure::<dyn Fn()>::new(move || {
                dismissed_for_empty_click.set(true);
                deselect_ref.borrow().emit(());
            });
            glue::sb_init_map(
                "sb-map",
                center.0,
                center.1,
                zoom,
                closure.as_ref().unchecked_ref(),
                bounds_closure.as_ref().unchecked_ref(),
                empty_click_closure.as_ref().unchecked_ref(),
            );
            *closure_slot.borrow_mut() = Some(closure);
            *bounds_closure_slot.borrow_mut() = Some(bounds_closure);
            *empty_click_closure_slot.borrow_mut() = Some(empty_click_closure);
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
            props.bounds,
            *submitted,
            search_places.clone(),
            poi_matches.clone(),
        ),
        |(
            places,
            selected,
            overpass_places,
            show_unvisited,
            marker_density,
            bounds,
            submitted,
            search_places,
            poi_matches,
        )| {
            // A submitted search narrows the map to just its matches instead
            // of the full place/Overpass layers.
            let places = if *submitted { search_places } else { places };
            let overpass_places = if *submitted { poi_matches } else { overpass_places };
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
                pins.extend(
                    grid_thinned(overpass_places, *bounds, *marker_density)
                        .into_iter()
                        .map(|p| Pin {
                            id: p.id.clone(),
                            lat: p.lat,
                            lng: p.lng,
                            color: p.place_type.color(PlaceSource::Overpass),
                            icon: p.place_type.icon(),
                            name: p.name.clone(),
                            selected: false,
                        }),
                );
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

    // While the Discover tip is visible, any click anywhere dismisses it
    // (and remembers that in localStorage) -- including a tap on the
    // Discover button itself, which also runs its own toggle handler.
    {
        let show_tip_state = show_tip.clone();
        use_effect_with(*show_tip, move |visible| {
            let window = web_sys::window();
            let listener = visible.then(|| {
                Closure::<dyn FnMut()>::new(move || {
                    let _ = LocalStorage::set("sb_seen_discover_tip", true);
                    show_tip_state.set(false);
                })
            });
            if let (Some(win), Some(cl)) = (window.as_ref(), listener.as_ref()) {
                let _ = win.add_event_listener_with_callback("click", cl.as_ref().unchecked_ref());
            }
            move || {
                if let (Some(win), Some(cl)) = (window.as_ref(), listener.as_ref()) {
                    let _ =
                        win.remove_event_listener_with_callback("click", cl.as_ref().unchecked_ref());
                }
            }
        });
    }

    let featured = if *dismissed {
        None
    } else {
        props
            .selected
            .and_then(|id| props.places.iter().find(|p| p.id == id))
            .or_else(|| props.places.first())
    };

    let close_card = {
        let dismissed = dismissed.clone();
        let cb = props.on_deselect.clone();
        Callback::from(move |_| {
            dismissed.set(true);
            cb.emit(());
        })
    };

    let dismiss_tip = {
        let show_tip = show_tip.clone();
        Callback::from(move |_| {
            let _ = LocalStorage::set("sb_seen_discover_tip", true);
            show_tip.set(false);
        })
    };

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
                        <button class="search-submit" onclick={submit_search_click}>
                            <span class="mi">{"search"}</span>
                        </button>
                        <input
                            class="search-input"
                            placeholder="Search shops, malls, parks…"
                            value={(*query).clone()}
                            oninput={on_search_input}
                            onkeydown={on_search_keydown}
                        />
                        if let Some(count) = search_count {
                            <span class="search-count">{format!("{count} found")}</span>
                        }
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
                if searching && !props.show_unvisited {
                    <div class="search-warn">
                        <span class="mi">{"info"}</span>
                        {"Discovery is off — only our places are searched"}
                    </div>
                }
                if searching && !*submitted {
                    { search_results(&search_places, &poi_matches, &pick_result, &pick_poi) }
                } else if !searching {
                    <FilterChips
                        active_amenities={props.active_amenities.clone()}
                        on_toggle_amenity={props.on_toggle_amenity.clone()}
                    />
                }
            </div>

            { ui::discover_button(props.show_unvisited, featured.is_some(), &props.on_toggle_unvisited) }

            if *show_tip {
                { discover_tip(featured.is_some(), &dismiss_tip) }
            }

            <button
                class={if featured.is_some() { "recenter above-card" } else { "recenter" }}
                onclick={recenter}
            >
                <span class="mi">{"my_location"}</span>
            </button>

            if let Some(p) = featured {
                { featured_card(p, props, &close_card) }
            }
        </div>
    }
}

/// Thins the Overpass layer to one pin per grid cell, enforcing a minimum
/// on-screen spacing set by the density level. Every choice is stable under
/// panning: cells are anchored to the world (indexed from lat/lng 0, sized
/// on a power-of-two ladder that only moves on zoom, since a Mercator
/// viewport's longitude span is pan-invariant), and each cell keeps the POI
/// with the lowest id hash rather than anything position- or order-based.
/// The POI list itself still grows/shrinks at the viewport edge as fetches
/// come in, but a kept POI stays kept wherever it's loaded.
fn grid_thinned(
    pois: &[OverpassPoi],
    bounds: Option<BBox>,
    density_level: u8,
) -> Vec<&OverpassPoi> {
    let (Some(cells_across), Some(b)) =
        (shared::marker_density_cells(density_level), bounds)
    else {
        return pois.iter().collect();
    };
    let lng_span = b.max_lng - b.min_lng;
    if lng_span <= 0.0 {
        return pois.iter().collect();
    }
    let raw_cell = lng_span / f64::from(cells_across);
    let ladder = (360.0 / raw_cell).log2().round().clamp(0.0, 40.0);
    let cell = 360.0 / 2f64.powi(ladder as i32);

    // Per cell: (winning hash, index into `pois`), lowest hash wins.
    let mut best: HashMap<(i64, i64), (u64, usize)> = HashMap::new();
    for (i, p) in pois.iter().enumerate() {
        let key = ((p.lat / cell).floor() as i64, ((p.lng / cell).floor()) as i64);
        let hash = fnv1a(p.id.as_bytes());
        let entry = best.entry(key).or_insert((hash, i));
        if hash < entry.0 {
            *entry = (hash, i);
        }
    }
    let mut keep: Vec<usize> = best.into_values().map(|(_, i)| i).collect();
    keep.sort_unstable();
    keep.into_iter().map(|i| &pois[i]).collect()
}

/// FNV-1a, used as a stable per-POI priority for [`grid_thinned`] — unlike
/// `DefaultHasher` it's guaranteed identical across builds, so which POI
/// represents a cell never shifts between sessions.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        h ^= u64::from(byte);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
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

/// One-time coach mark pointing at the floating Discover button, shown
/// until the user's first interaction with the app (see the window click
/// listener in [`map_view`]). `above_card` mirrors the Discover button's own
/// offset so the tip still points at it when the featured card is showing.
fn discover_tip(above_card: bool, on_close: &Callback<MouseEvent>) -> Html {
    let class = if above_card { "discover-tip above-card" } else { "discover-tip" };
    html! {
        <div {class} onclick={|e: MouseEvent| e.stop_propagation()}>
            <button class="discover-tip-close" onclick={on_close.clone()}>
                <span class="mi">{"close"}</span>
            </button>
            <span class="discover-tip-text">
                {"Toggle discovery on to find new places, or off to show \"our places\"."}
            </span>
        </div>
    }
}

fn featured_card(p: &PlaceSummary, props: &MapViewProps, on_close: &Callback<MouseEvent>) -> Html {
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
            <button class="sel-close" onclick={on_close.clone()}>
                <span class="mi">{"close"}</span>
            </button>
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
