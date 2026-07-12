use std::collections::HashSet;

use gloo_storage::{LocalStorage, Storage};
use shared::{Amenity, AuthSession, PlaceDetail, PlaceSummary, PlaceType, PlacesQuery};
use uuid::Uuid;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::components::account_view::AccountView;
use crate::components::add_form::AddForm;
use crate::components::detail_view::DetailView;
use crate::components::filters_sheet::FiltersSheet;
use crate::components::invite_view::InviteView;
use crate::components::list_view::ListView;
use crate::components::map_view::MapView;
use crate::components::onboarding::Onboarding;
use crate::components::register_view::RegisterView;
use crate::components::saved_view::SavedView;
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
    let saved_ids = use_state(HashSet::<Uuid>::new);
    let saved_places = use_state(Vec::<PlaceSummary>::new);
    let toast = use_state(|| None::<String>);
    let refresh = use_state(|| 0u32);
    let auth = use_state(api::stored_auth);

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

    let on_detail_updated = {
        let detail = detail.clone();
        let refresh = refresh.clone();
        Callback::from(move |d: PlaceDetail| {
            detail.set(Some(d));
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
                            active_types={active_types(&filters)}
                            active_amenities={active_amenities(&filters)}
                            on_open={open_detail.clone()}
                            on_open_filters={open_filters.clone()}
                            on_toggle_type={on_toggle_type.clone()}
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
                        />
                    },
                    Route::Register => html! {
                        <RegisterView
                            auth={(*auth).clone()}
                            on_login={on_login}
                            on_toast={show_toast.clone()}
                        />
                    },
                    _ => html! {
                        <MapView
                            places={(*places).clone()}
                            selected={*selected}
                            origin={*origin}
                            focus={*map_focus}
                            filters_active={filters.is_active()}
                            active_types={active_types(&filters)}
                            active_amenities={active_amenities(&filters)}
                            on_select={on_select}
                            on_open={open_detail.clone()}
                            on_open_filters={open_filters.clone()}
                            on_toggle_type={on_toggle_type.clone()}
                            on_toggle_amenity={on_toggle_amenity.clone()}
                            on_recenter={on_recenter}
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
                />
            }

            if let Some(msg) = (*toast).clone() {
                <div class="toast"><span class="mi">{"check_circle"}</span>{msg}</div>
            }
        </div>
    }
}

fn active_types(filters: &Filters) -> Vec<PlaceType> {
    let mut v: Vec<PlaceType> = filters.types.iter().copied().collect();
    v.sort_unstable_by_key(|t| t.as_str());
    v
}

fn active_amenities(filters: &Filters) -> Vec<Amenity> {
    let mut v: Vec<Amenity> = filters.amenities.iter().copied().collect();
    v.sort_unstable_by_key(|a| a.as_str());
    v
}
