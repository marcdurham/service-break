use shared::{Amenity, OverpassPoi, PlaceSource, PlaceSummary, PlacesQuery};
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::components::ui::{self, FilterChips, PlaceCard};
use crate::api;

#[derive(Properties, PartialEq)]
pub struct ListViewProps {
    pub places: Vec<PlaceSummary>,
    pub origin: Option<(f64, f64)>,
    pub active_amenities: Vec<Amenity>,
    /// Overpass POIs currently in view; only rendered/searched when
    /// `show_unvisited` is on.
    pub overpass_places: Vec<OverpassPoi>,
    pub show_unvisited: bool,
    pub on_open: Callback<uuid::Uuid>,
    /// Fired when a tapped card or search result is an unpromoted Overpass
    /// POI, so the app can open the lightweight preview.
    pub on_open_poi: Callback<OverpassPoi>,
    pub on_open_filters: Callback<()>,
    pub on_toggle_amenity: Callback<Amenity>,
    pub on_reset_filters: Callback<()>,
}

#[function_component(ListView)]
pub fn list_view(props: &ListViewProps) -> Html {
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

    let open_filters = {
        let cb = props.on_open_filters.clone();
        Callback::from(move |_| cb.emit(()))
    };
    let reset = {
        let cb = props.on_reset_filters.clone();
        Callback::from(move |_| cb.emit(()))
    };

    let (places, overpass_places): (Vec<PlaceSummary>, Vec<OverpassPoi>) = if searching {
        ((*results).clone().unwrap_or_default(), poi_matches)
    } else {
        (
            props.places.clone(),
            if props.show_unvisited { props.overpass_places.clone() } else { Vec::new() },
        )
    };
    let is_empty = places.is_empty() && overpass_places.is_empty();
    let cards = merged_cards(&places, &overpass_places, &props.on_open, &props.on_open_poi);

    html! {
        <div class="screen sb-scroll">
            <div class="list-head">
                <div>
                    <div class="screen-title">{"Nearby places"}</div>
                    <div class="screen-sub">
                        {format!("{} places · sorted by distance", props.places.len())}
                    </div>
                </div>
                <button class="icon-btn" onclick={open_filters}>
                    <span class="mi">{"tune"}</span>
                </button>
            </div>
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
            if !searching {
                <FilterChips
                    active_amenities={props.active_amenities.clone()}
                    on_toggle_amenity={props.on_toggle_amenity.clone()}
                />
            }
            if is_empty {
                <div class="empty">
                    <span class="mi">{"travel_explore"}</span>
                    <div class="empty-title">
                        {if searching { "No places found" } else { "No places match your filters" }}
                    </div>
                    if !searching {
                        <button onclick={reset}>{"Clear filters"}</button>
                    }
                </div>
            } else {
                <div class="cards">
                    { for cards }
                </div>
            }
        </div>
    }
}

/// App places and Overpass POIs, sorted together by distance (unrated
/// distance sorts last) so the two sources interleave naturally.
fn merged_cards(
    places: &[PlaceSummary],
    overpass_places: &[OverpassPoi],
    on_open: &Callback<uuid::Uuid>,
    on_open_poi: &Callback<OverpassPoi>,
) -> Vec<Html> {
    let mut cards: Vec<(f64, Html)> = places
        .iter()
        .map(|p| {
            let html = html! {
                <PlaceCard key={p.id.to_string()} place={p.clone()} on_open={on_open.clone()} />
            };
            (p.distance_mi.unwrap_or(f64::MAX), html)
        })
        .collect();
    cards.extend(overpass_places.iter().map(|p| {
        (p.distance_mi.unwrap_or(f64::MAX), poi_card(p, on_open_poi))
    }));
    cards.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    cards.into_iter().map(|(_, html)| html).collect()
}

/// Like [`PlaceCard`] but for an unpromoted Overpass POI — same layout,
/// blue/gray badge, opens the lightweight preview instead of `DetailView`.
fn poi_card(p: &OverpassPoi, on_open_poi: &Callback<OverpassPoi>) -> Html {
    let onclick = {
        let cb = on_open_poi.clone();
        let poi = p.clone();
        Callback::from(move |_| cb.emit(poi.clone()))
    };
    html! {
        <button class="card" key={p.id.clone()} {onclick}>
            { ui::badge(p.place_type, PlaceSource::Overpass) }
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
            </div>
        </button>
    }
}
