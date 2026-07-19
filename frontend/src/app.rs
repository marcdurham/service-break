use std::collections::HashSet;

use gloo_storage::{LocalStorage, Storage};
use shared::{
    Amenity, AuthSession, BBox, MapsLinkResult, OverpassPoi, OverpassQuery, PlaceDetail,
    PlaceSummary, PlaceType, PlacesQuery,
};
use uuid::Uuid;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::components::account_view::AccountView;
use crate::components::activity_view::ActivityView;
use crate::components::change_password_view::ChangePasswordView;
use crate::components::add_form::AddForm;
use crate::components::admin_view::AdminView;
use crate::components::edit_user_view::EditUserView;
use crate::components::users_view::UsersView;
use crate::components::detail_view::DetailView;
use crate::components::filters_sheet::FiltersSheet;
use crate::components::invite_view::InviteView;
use crate::components::list_view::ListView;
use crate::components::map_view::MapView;
use crate::components::oauth_complete_view::OauthCompleteView;
use crate::components::onboarding::Onboarding;
use crate::components::poi_detail_view::PoiDetailView;
use crate::components::register_view::RegisterView;
use crate::components::saved_view::SavedView;
use crate::components::share_target_view::ShareTargetView;
use crate::components::tab_bar::TabBar;
use crate::route::Route;
use crate::{api, glue};

/// Fallback map center (downtown Seattle, where the demo seed data lives).
pub const FALLBACK_CENTER: (f64, f64) = (47.6097, -122.3422);

#[derive(Debug, Clone, PartialEq)]
pub struct Filters {
    pub types: HashSet<PlaceType>,
    /// Offerings the user is interested in; all of [`Amenity::FEATURED`]
    /// by default, which means "don't filter by what's offered".
    pub amenities: HashSet<Amenity>,
    pub clean_only: bool,
    pub no_purchase: bool,
    pub has_parking: bool,
    pub radius_mi: u8,
    /// Whether the Overpass POI layer (raw OSM places nobody has interacted
    /// with in the app yet) shows on the map/list and in search. On by
    /// default.
    pub show_unvisited: bool,
}

impl Default for Filters {
    fn default() -> Self {
        Filters {
            types: HashSet::new(),
            amenities: Amenity::FEATURED.into_iter().collect(),
            clean_only: false,
            no_purchase: false,
            has_parking: false,
            radius_mi: 5,
            show_unvisited: true,
        }
    }
}

impl Filters {
    pub fn is_active(&self) -> bool {
        self != &Filters::default()
    }

    pub fn to_query(&self, origin: Option<(f64, f64)>) -> PlacesQuery {
        let mut types: Vec<&str> = self.types.iter().map(|t| t.as_str()).collect();
        types.sort_unstable();
        // Every featured chip on (the default) means "show everything",
        // so only a proper, non-empty subset narrows the query.
        let all_featured = self.amenities.len() == Amenity::FEATURED.len();
        let mut amenities: Vec<&str> = self.amenities.iter().map(|a| a.as_str()).collect();
        amenities.sort_unstable();
        PlacesQuery {
            q: None,
            lat: origin.map(|(lat, _)| lat),
            lng: origin.map(|(_, lng)| lng),
            radius_mi: Some(f64::from(self.radius_mi)),
            types: (!types.is_empty()).then(|| types.join(",")),
            amenities: (!amenities.is_empty() && !all_featured).then(|| amenities.join(",")),
            clean_min: self.clean_only.then_some(4.0),
            no_purchase: self.no_purchase.then_some(true),
            has_parking: self.has_parking.then_some(true),
            sort: None,
        }
    }
}

fn device_id() -> String {
    if let Ok(id) = LocalStorage::get::<String>("sb_device") {
        return id;
    }
    let id = Uuid::new_v4().to_string();
    let _ = LocalStorage::set("sb_device", &id);
    id
}

#[function_component(App)]
pub fn app() -> Html {
    let device = use_memo((), |()| device_id());
    let started = use_state(|| LocalStorage::get::<bool>("sb_started").unwrap_or(false));
    let route = use_route::<Route>().unwrap_or(Route::Map);
    let navigator = use_navigator().expect("BrowserRouter provides a navigator");
    // The nav page rendered behind the place-detail overlay (and the page
    // the tab bar highlights); tracks `route` whenever it's a nav route.
    let background = use_state(|| if route.is_nav() { route } else { Route::Map });
    // Whether the current place overlay (if any) was the very first page
    // loaded, e.g. from a shared link — closing it then has no in-app page
    // to go "back" to, so it should navigate to the background route instead.
    let entered_directly = use_state(|| !route.is_nav());
    let origin = use_state(|| None::<(f64, f64)>);
    // Set only when the browser's geolocation actually succeeds, as
    // opposed to `origin`, which falls back to a fixed map center on
    // failure — used to show the user's real location on the map picker.
    let user_location = use_state(|| None::<(f64, f64)>);
    let places = use_state(Vec::<PlaceSummary>::new);
    let selected = use_state(|| None::<Uuid>);
    let detail = use_state(|| None::<PlaceDetail>);
    // A place the map should center on, set by "Show on map" in the viewer.
    let map_focus = use_state(|| None::<(f64, f64)>);
    let show_filters = use_state(|| false);
    let filters = use_state(Filters::default);
    // Marker density: index into `shared::MARKER_DENSITY_LEVELS`, setting
    // how closely packed Overpass POI pins may get on the map. A
    // device-wide display preference (remembered in localStorage, not reset
    // with filters); defaults to the sparsest level since an unfiltered
    // Overpass layer can flood a downtown viewport.
    let marker_density = use_state(|| LocalStorage::get::<u8>("sb_marker_density").unwrap_or(0));
    // Overpass POIs (raw OSM places, not yet in the app) currently in view,
    // the live Leaflet viewport reported by the map, and whichever one the
    // user has tapped open for the lightweight preview.
    let overpass_places = use_state(Vec::<OverpassPoi>::new);
    let map_bounds = use_state(|| None::<BBox>);
    let open_poi = use_state(|| None::<OverpassPoi>);
    let saved_ids = use_state(HashSet::<Uuid>::new);
    let saved_places = use_state(Vec::<PlaceSummary>::new);
    // Name/location resolved from a pasted Google Maps link, handed to the
    // Add page for one-time prefill; see `on_maps_link` below.
    let prefill = use_state(|| None::<MapsLinkResult>);
    let toast = use_state(|| None::<String>);
    // A persistent (non-auto-dismissing) banner for the Register page, set
    // when a Google login fails because no account is linked yet.
    let register_notice = use_state(|| None::<String>);
    let refresh = use_state(|| 0u32);
    let auth = use_state(api::stored_auth);
    let google_enabled = use_state(|| false);

    // Ask the backend once whether it has a Google OAuth client configured,
    // so the "Continue with Google" buttons can stay hidden without one.
    {
        let google_enabled = google_enabled.clone();
        use_effect_with((), move |()| {
            wasm_bindgen_futures::spawn_local(async move {
                google_enabled.set(api::google_sign_in_enabled().await);
            });
        });
    }

    let show_toast = {
        let toast = toast.clone();
        Callback::from(move |msg: String| {
            toast.set(Some(msg));
            let toast = toast.clone();
            wasm_bindgen_futures::spawn_local(async move {
                gloo_timers::future::TimeoutFuture::new(2400).await;
                toast.set(None);
            });
        })
    };

    let set_register_notice = {
        let register_notice = register_notice.clone();
        Callback::from(move |msg: String| register_notice.set(Some(msg)))
    };
    let on_register_notice_shown = {
        let register_notice = register_notice.clone();
        Callback::from(move |()| register_notice.set(None))
    };

    // Keep `background` in sync with the URL: any nav route becomes the new
    // background; the place-detail route leaves it untouched.
    {
        let background = background.clone();
        use_effect_with(route, move |route| {
            if route.is_nav() {
                background.set(*route);
            }
        });
    }

    // Unknown URLs fall back to the map.
    {
        let navigator = navigator.clone();
        use_effect_with(route, move |route| {
            if matches!(route, Route::NotFound) {
                navigator.replace(&Route::Map);
            }
        });
    }

    // Fetch the place behind `/place/:id` whenever the route points at one.
    {
        let detail = detail.clone();
        let origin = origin.clone();
        use_effect_with(route, move |route| {
            if let Route::Place { id } = *route {
                let origin = *origin;
                let detail = detail.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(d) = api::fetch_place(id, origin).await {
                        detail.set(Some(d));
                    }
                });
            } else {
                detail.set(None);
            }
        });
    }

    // Drop a stored session the server no longer accepts (expired or
    // logged out elsewhere); a failed check (offline) changes nothing.
    {
        let auth = auth.clone();
        use_effect_with((), move |()| {
            if auth.is_some() {
                wasm_bindgen_futures::spawn_local(async move {
                    if api::session_is_valid().await == Ok(false) {
                        api::clear_auth();
                        auth.set(None);
                    }
                });
            }
        });
    }

    // Ask for the user's location once the app has started.
    {
        let origin = origin.clone();
        let user_location = user_location.clone();
        use_effect_with(*started, move |started| {
            if *started && origin.is_none() {
                let ok_origin = origin.clone();
                let ok_user_location = user_location.clone();
                let ok = Closure::<dyn FnMut(f64, f64)>::new(move |lat: f64, lng: f64| {
                    ok_origin.set(Some((lat, lng)));
                    ok_user_location.set(Some((lat, lng)));
                });
                let err = Closure::<dyn FnMut(String)>::new(move |_: String| {
                    origin.set(Some(FALLBACK_CENTER));
                });
                glue::sb_locate(ok.as_ref().unchecked_ref(), err.as_ref().unchecked_ref());
                // One-shot callbacks: intentionally leaked.
                ok.forget();
                err.forget();
            }
        });
    }

    // Load places whenever position, filters or data change.
    {
        let places = places.clone();
        use_effect_with(
            (*started, *origin, (*filters).clone(), *refresh),
            move |(started, origin, filters, _)| {
                if *started {
                    let q = filters.to_query(*origin);
                    let places = places.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(list) = api::fetch_places(&q).await {
                            places.set(list);
                        }
                    });
                }
            },
        );
    }

    // The bbox to query the Overpass POI layer for: the live map viewport
    // on the Map screen (reported by Leaflet via `on_bounds_changed`), or a
    // synthetic bbox from the radius filter on List (which has no real map
    // to derive bounds from). `None` elsewhere, where the layer isn't shown.
    let effective_overpass_bbox = match *background {
        Route::Map => *map_bounds,
        Route::List => origin.map(|o| shared::radius_bbox(o, f64::from(filters.radius_mi))),
        _ => None,
    };

    // Load Overpass POIs whenever the effective viewport or the "Show
    // unvisited places" toggle changes; cleared immediately (no fetch) when
    // the toggle is off or there's no viewport to query yet.
    {
        let overpass_places = overpass_places.clone();
        use_effect_with(
            (effective_overpass_bbox, filters.show_unvisited),
            move |(bbox, show_unvisited)| {
                let Some(bbox) = (*show_unvisited).then_some(*bbox).flatten() else {
                    overpass_places.set(Vec::new());
                    return;
                };
                let q = OverpassQuery {
                    min_lat: bbox.min_lat,
                    min_lng: bbox.min_lng,
                    max_lat: bbox.max_lat,
                    max_lng: bbox.max_lng,
                    q: None,
                };
                let overpass_places = overpass_places.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(list) = api::fetch_overpass_pois(&q).await {
                        overpass_places.set(list);
                    }
                });
            },
        );
    }

    // Load saved ids once, and the saved list when relevant state changes.
    {
        let saved_ids = saved_ids.clone();
        let saved_places = saved_places.clone();
        let device = device.clone();
        use_effect_with((*started, *origin, *refresh), move |(started, origin, _)| {
            if *started {
                let origin = *origin;
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(list) = api::fetch_saved(&device, origin).await {
                        saved_ids.set(list.iter().map(|p| p.id).collect());
                        saved_places.set(list);
                    }
                });
            }
        });
    }

    let on_start = {
        let started = started.clone();
        Callback::from(move |()| {
            let _ = LocalStorage::set("sb_started", true);
            started.set(true);
        })
    };

    let on_nav = {
        let navigator = navigator.clone();
        let show_filters = show_filters.clone();
        let map_focus = map_focus.clone();
        Callback::from(move |r: Route| {
            show_filters.set(false);
            map_focus.set(None);
            navigator.push(&r);
        })
    };

    let on_select = {
        let selected = selected.clone();
        Callback::from(move |id: Uuid| selected.set(Some(id)))
    };

    let on_bounds_changed = {
        let map_bounds = map_bounds.clone();
        Callback::from(move |b: BBox| map_bounds.set(Some(b)))
    };

    let on_open_poi = {
        let open_poi = open_poi.clone();
        Callback::from(move |poi: OverpassPoi| open_poi.set(Some(poi)))
    };

    let close_poi = {
        let open_poi = open_poi.clone();
        Callback::from(move |()| open_poi.set(None))
    };

    let open_detail = {
        let navigator = navigator.clone();
        let selected = selected.clone();
        Callback::from(move |id: Uuid| {
            selected.set(Some(id));
            navigator.push(&Route::Place { id });
        })
    };

    let close_detail = {
        let navigator = navigator.clone();
        let background = background.clone();
        let entered_directly = entered_directly.clone();
        Callback::from(move |()| {
            if *entered_directly {
                entered_directly.set(false);
                navigator.push(&*background);
            } else {
                navigator.back();
            }
        })
    };

    let on_show_on_map = {
        let navigator = navigator.clone();
        let background = background.clone();
        let selected = selected.clone();
        let map_focus = map_focus.clone();
        Callback::from(move |p: PlaceSummary| {
            selected.set(Some(p.id));
            map_focus.set(Some((p.lat, p.lng)));
            background.set(Route::Map);
            navigator.push(&Route::Map);
        })
    };

    // Sends the user to the Account page when a gated action needs a login.
    let require_login = {
        let navigator = navigator.clone();
        let show_toast = show_toast.clone();
        Callback::from(move |msg: String| {
            show_toast.emit(msg);
            navigator.push(&Route::Account);
        })
    };

    // A pasted Google Maps link resolved to a place: send the user to the
    // Add page with the name and location prefilled (signing in first if
    // needed, same as any other add-a-place action).
    let on_maps_link = {
        let navigator = navigator.clone();
        let background = background.clone();
        let prefill = prefill.clone();
        let auth = auth.clone();
        let require_login = require_login.clone();
        Callback::from(move |m: MapsLinkResult| {
            if auth.is_none() {
                require_login.emit("Sign in to add a place from a link".to_owned());
                return;
            }
            prefill.set(Some(m));
            background.set(Route::Add);
            navigator.push(&Route::Add);
        })
    };

    let on_prefill_used = {
        let prefill = prefill.clone();
        Callback::from(move |()| prefill.set(None))
    };

    let on_login = {
        let auth = auth.clone();
        let show_toast = show_toast.clone();
        Callback::from(move |session: AuthSession| {
            api::store_auth(&session);
            show_toast.emit(format!("Signed in as {}", session.username));
            auth.set(Some(session));
        })
    };

    let on_logout = {
        let auth = auth.clone();
        let show_toast = show_toast.clone();
        Callback::from(move |()| {
            let auth = auth.clone();
            let show_toast = show_toast.clone();
            wasm_bindgen_futures::spawn_local(async move {
                // Invalidate server-side first: logout reads the stored token.
                api::logout().await;
                api::clear_auth();
                auth.set(None);
                show_toast.emit("Signed out".to_owned());
            });
        })
    };

    let on_toggle_save = {
        let saved_ids = saved_ids.clone();
        let device = device.clone();
        let show_toast = show_toast.clone();
        let refresh = refresh.clone();
        let auth = auth.clone();
        let require_login = require_login.clone();
        Callback::from(move |id: Uuid| {
            if auth.is_none() {
                require_login.emit("Sign in to save places".to_owned());
                return;
            }
            let mut ids = (*saved_ids).clone();
            let device = device.to_string();
            let removing = ids.contains(&id);
            if removing {
                ids.remove(&id);
                show_toast.emit("Removed from saved".to_owned());
            } else {
                ids.insert(id);
                show_toast.emit("Saved".to_owned());
            }
            saved_ids.set(ids);
            let refresh = refresh.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let res = if removing {
                    api::unsave_place(&device, id).await
                } else {
                    api::save_place(&device, id).await
                };
                if res.is_ok() {
                    refresh.set(refresh.wrapping_add(1));
                }
            });
        })
    };

    let on_toggle_type = {
        let filters = filters.clone();
        Callback::from(move |t: PlaceType| {
            let mut f = (*filters).clone();
            if !f.types.remove(&t) {
                f.types.insert(t);
            }
            filters.set(f);
        })
    };

    let on_toggle_amenity = {
        let filters = filters.clone();
        Callback::from(move |a: Amenity| {
            let mut f = (*filters).clone();
            if !f.amenities.remove(&a) {
                f.amenities.insert(a);
            }
            filters.set(f);
        })
    };

    let on_filters_change = {
        let filters = filters.clone();
        Callback::from(move |f: Filters| filters.set(f))
    };

    let on_reset_filters = {
        let filters = filters.clone();
        Callback::from(move |()| filters.set(Filters::default()))
    };

    let on_marker_density_change = {
        let marker_density = marker_density.clone();
        Callback::from(move |level: u8| {
            let _ = LocalStorage::set("sb_marker_density", level);
            marker_density.set(level);
        })
    };

    let open_filters = {
        let show_filters = show_filters.clone();
        Callback::from(move |()| show_filters.set(true))
    };

    let close_filters = {
        let show_filters = show_filters.clone();
        Callback::from(move |()| show_filters.set(false))
    };

    let on_created = {
        let navigator = navigator.clone();
        let background = background.clone();
        let selected = selected.clone();
        let detail = detail.clone();
        let refresh = refresh.clone();
        let show_toast = show_toast.clone();
        Callback::from(move |d: PlaceDetail| {
            let id = d.summary.id;
            selected.set(Some(id));
            detail.set(Some(d));
            background.set(Route::Map);
            refresh.set(refresh.wrapping_add(1));
            show_toast.emit("Place added — thanks, scout!".to_owned());
            navigator.push(&Route::Place { id });
        })
    };

    // An Overpass POI just promoted into a normal app place — the pin/card
    // flips from blue/gray to brown on the next refresh, and the user lands
    // on the normal `DetailView` (with Save/Rate/Edit) instead of the
    // lightweight preview.
    let on_poi_promoted = {
        let navigator = navigator.clone();
        let selected = selected.clone();
        let detail = detail.clone();
        let open_poi = open_poi.clone();
        let refresh = refresh.clone();
        Callback::from(move |d: PlaceDetail| {
            let id = d.summary.id;
            selected.set(Some(id));
            detail.set(Some(d));
            open_poi.set(None);
            refresh.set(refresh.wrapping_add(1));
            navigator.push(&Route::Place { id });
        })
    };

    let on_detail_updated = {
        let detail = detail.clone();
        let refresh = refresh.clone();
        Callback::from(move |d: PlaceDetail| {
            detail.set(Some(d));
            refresh.set(refresh.wrapping_add(1));
        })
    };

    let on_detail_deleted = {
        let detail = detail.clone();
        let selected = selected.clone();
        let refresh = refresh.clone();
        Callback::from(move |()| {
            detail.set(None);
            selected.set(None);
            refresh.set(refresh.wrapping_add(1));
        })
    };

    let on_recenter = {
        let origin = origin.clone();
        let selected = selected.clone();
        let map_focus = map_focus.clone();
        Callback::from(move |()| {
            let (lat, lng) = origin.unwrap_or(FALLBACK_CENTER);
            glue::sb_fly_to(lat, lng, 14.0);
            selected.set(None);
            map_focus.set(None);
        })
    };

    if !*started {
        return html! { <div class="app-shell"><Onboarding on_start={on_start} /></div> };
    }

    let list_count = places.len();

    html! {
        <div class="app-shell">
            {
                match *background {
                    Route::List => html! {
                        <ListView
                            places={(*places).clone()}
                            origin={*origin}
                            active_amenities={active_amenities(&filters)}
                            overpass_places={(*overpass_places).clone()}
                            show_unvisited={filters.show_unvisited}
                            on_open={open_detail.clone()}
                            on_open_poi={on_open_poi.clone()}
                            on_open_filters={open_filters.clone()}
                            on_toggle_amenity={on_toggle_amenity.clone()}
                            on_reset_filters={on_reset_filters.clone()}
                        />
                    },
                    Route::Add => html! {
                        <AddForm
                            device_id={(*device).clone()}
                            origin={*origin}
                            user_location={*user_location}
                            logged_in={auth.is_some()}
                            prefill={(*prefill).clone()}
                            on_prefill_used={on_prefill_used.clone()}
                            on_created={on_created}
                            on_sign_in={{
                                let require_login = require_login.clone();
                                Callback::from(move |()| {
                                    require_login.emit("Sign in to add places".to_owned())
                                })
                            }}
                            on_toast={show_toast.clone()}
                        />
                    },
                    Route::Saved => html! {
                        <SavedView
                            places={(*saved_places).clone()}
                            on_open={open_detail.clone()}
                        />
                    },
                    Route::Invite => html! {
                        <InviteView auth={(*auth).clone()} on_toast={show_toast.clone()} />
                    },
                    Route::Account => html! {
                        <AccountView
                            auth={(*auth).clone()}
                            on_login={on_login}
                            on_logout={on_logout}
                            on_toast={show_toast.clone()}
                            google_enabled={*google_enabled}
                        />
                    },
                    Route::ChangePassword => html! {
                        <ChangePasswordView
                            auth={(*auth).clone()}
                            on_toast={show_toast.clone()}
                        />
                    },
                    Route::Register => html! {
                        <RegisterView
                            auth={(*auth).clone()}
                            on_login={on_login}
                            on_toast={show_toast.clone()}
                            google_enabled={*google_enabled}
                            notice={(*register_notice).clone()}
                            on_notice_shown={on_register_notice_shown.clone()}
                        />
                    },
                    Route::OauthComplete => html! {
                        <OauthCompleteView
                            on_login={on_login}
                            on_toast={show_toast.clone()}
                            on_register_notice={set_register_notice.clone()}
                        />
                    },
                    Route::ShareTarget => html! {
                        <ShareTargetView on_maps_link={on_maps_link.clone()} on_toast={show_toast.clone()} />
                    },
                    Route::Admin => html! {
                        <AdminView auth={(*auth).clone()} on_toast={show_toast.clone()} />
                    },
                    Route::Users => html! {<UsersView />},
                    Route::UserEdit { .. } => html! {
                        <EditUserView auth={(*auth).clone()} on_toast={show_toast.clone()} />
                    },
                    Route::Activity => html! { <ActivityView /> },
                    Route::UserActivity { id } => html! {
                        <ActivityView user_id={Some(id)} back_route={Some(Route::UserEdit { id })} />
                    },
                    _ => html! {
                        <MapView
                            places={(*places).clone()}
                            selected={*selected}
                            origin={*origin}
                            focus={*map_focus}
                            filters_active={filters.is_active()}
                            active_amenities={active_amenities(&filters)}
                            overpass_places={(*overpass_places).clone()}
                            show_unvisited={filters.show_unvisited}
                            marker_density={*marker_density}
                            bounds={*map_bounds}
                            on_select={on_select}
                            on_open={open_detail.clone()}
                            on_open_poi={on_open_poi.clone()}
                            on_open_filters={open_filters.clone()}
                            on_toggle_amenity={on_toggle_amenity.clone()}
                            on_recenter={on_recenter}
                            on_maps_link={on_maps_link}
                            on_bounds_changed={on_bounds_changed}
                            on_toast={show_toast.clone()}
                        />
                    },
                }
            }

            <TabBar route={*background} on_change={on_nav} />

            if let Some(d) = (*detail).clone() {
                <DetailView
                    detail={d}
                    device_id={(*device).clone()}
                    saved={detail.as_ref().is_some_and(|d| saved_ids.contains(&d.summary.id))}
                    logged_in={auth.is_some()}
                    on_require_login={{
                        let require_login = require_login.clone();
                        Callback::from(move |()| {
                            require_login.emit("Sign in to post a review".to_owned())
                        })
                    }}
                    on_close={close_detail}
                    on_show_on_map={on_show_on_map}
                    on_toggle_save={on_toggle_save}
                    on_updated={on_detail_updated}
                    on_deleted={on_detail_deleted}
                    on_toast={show_toast.clone()}
                />
            }

            if let Some(poi) = (*open_poi).clone() {
                <PoiDetailView
                    poi={poi}
                    device_id={(*device).clone()}
                    logged_in={auth.is_some()}
                    on_require_login={{
                        let require_login = require_login.clone();
                        Callback::from(move |()| {
                            require_login.emit("Sign in to save places".to_owned())
                        })
                    }}
                    on_close={close_poi}
                    on_promoted={on_poi_promoted}
                    on_toast={show_toast.clone()}
                />
            }

            if *show_filters {
                <FiltersSheet
                    filters={(*filters).clone()}
                    count={list_count}
                    on_change={on_filters_change}
                    on_reset={on_reset_filters}
                    on_close={close_filters}
                    on_toggle_type={on_toggle_type.clone()}
                    marker_density={*marker_density}
                    on_marker_density_change={on_marker_density_change}
                />
            }

            if let Some(msg) = (*toast).clone() {
                <div class="toast"><span class="mi">{"check_circle"}</span>{msg}</div>
            }
        </div>
    }
}

fn active_amenities(filters: &Filters) -> Vec<Amenity> {
    let mut v: Vec<Amenity> = filters.amenities.iter().copied().collect();
    v.sort_unstable_by_key(|a| a.as_str());
    v
}
