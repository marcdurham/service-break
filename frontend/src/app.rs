use std::collections::HashSet;

use gloo_storage::{LocalStorage, Storage};
use shared::{PlaceDetail, PlaceSummary, PlaceType, PlacesQuery};
use uuid::Uuid;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use yew::prelude::*;

use crate::components::add_form::AddForm;
use crate::components::detail_view::DetailView;
use crate::components::filters_sheet::FiltersSheet;
use crate::components::list_view::ListView;
use crate::components::map_view::MapView;
use crate::components::onboarding::Onboarding;
use crate::components::saved_view::SavedView;
use crate::components::tab_bar::TabBar;
use crate::{api, glue};

/// Fallback map center (downtown Seattle, where the demo seed data lives).
pub const FALLBACK_CENTER: (f64, f64) = (47.6097, -122.3422);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Map,
    List,
    Add,
    Saved,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Filters {
    pub types: HashSet<PlaceType>,
    pub clean_only: bool,
    pub no_purchase: bool,
    pub has_parking: bool,
    pub radius_mi: u8,
}

impl Default for Filters {
    fn default() -> Self {
        Filters {
            types: HashSet::new(),
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
        PlacesQuery {
            lat: origin.map(|(lat, _)| lat),
            lng: origin.map(|(_, lng)| lng),
            radius_mi: Some(f64::from(self.radius_mi)),
            types: (!types.is_empty()).then(|| types.join(",")),
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
    let tab = use_state(|| Tab::Map);
    let origin = use_state(|| None::<(f64, f64)>);
    let places = use_state(Vec::<PlaceSummary>::new);
    let selected = use_state(|| None::<Uuid>);
    let detail = use_state(|| None::<PlaceDetail>);
    let show_filters = use_state(|| false);
    let filters = use_state(Filters::default);
    let saved_ids = use_state(HashSet::<Uuid>::new);
    let saved_places = use_state(Vec::<PlaceSummary>::new);
    let toast = use_state(|| None::<String>);
    let refresh = use_state(|| 0u32);

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

    // Ask for the user's location once the app has started.
    {
        let origin = origin.clone();
        use_effect_with(*started, move |started| {
            if *started && origin.is_none() {
                let ok_origin = origin.clone();
                let ok = Closure::<dyn FnMut(f64, f64)>::new(move |lat: f64, lng: f64| {
                    ok_origin.set(Some((lat, lng)));
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

    let on_tab = {
        let tab = tab.clone();
        let detail = detail.clone();
        let show_filters = show_filters.clone();
        Callback::from(move |t: Tab| {
            detail.set(None);
            show_filters.set(false);
            tab.set(t);
        })
    };

    let on_select = {
        let selected = selected.clone();
        Callback::from(move |id: Uuid| selected.set(Some(id)))
    };

    let open_detail = {
        let detail = detail.clone();
        let selected = selected.clone();
        let origin = origin.clone();
        Callback::from(move |id: Uuid| {
            selected.set(Some(id));
            let detail = detail.clone();
            let origin = *origin;
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(d) = api::fetch_place(id, origin).await {
                    detail.set(Some(d));
                }
            });
        })
    };

    let close_detail = {
        let detail = detail.clone();
        Callback::from(move |()| detail.set(None))
    };

    let on_toggle_save = {
        let saved_ids = saved_ids.clone();
        let device = device.clone();
        let show_toast = show_toast.clone();
        let refresh = refresh.clone();
        Callback::from(move |id: Uuid| {
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
        let tab = tab.clone();
        let selected = selected.clone();
        let detail = detail.clone();
        let refresh = refresh.clone();
        let show_toast = show_toast.clone();
        Callback::from(move |d: PlaceDetail| {
            selected.set(Some(d.summary.id));
            detail.set(Some(d));
            tab.set(Tab::Map);
            refresh.set(refresh.wrapping_add(1));
            show_toast.emit("Stop added — thanks, scout!".to_owned());
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
        Callback::from(move |()| {
            let (lat, lng) = origin.unwrap_or(FALLBACK_CENTER);
            glue::sb_fly_to(lat, lng, 14.0);
            selected.set(None);
        })
    };

    if !*started {
        return html! { <div class="app-shell"><Onboarding on_start={on_start} /></div> };
    }

    let list_count = places.len();

    html! {
        <div class="app-shell">
            {
                match *tab {
                    Tab::Map => html! {
                        <MapView
                            places={(*places).clone()}
                            selected={*selected}
                            origin={*origin}
                            filters_active={filters.is_active()}
                            active_types={active_types(&filters)}
                            on_select={on_select}
                            on_open={open_detail.clone()}
                            on_open_filters={open_filters.clone()}
                            on_toggle_type={on_toggle_type.clone()}
                            on_recenter={on_recenter}
                        />
                    },
                    Tab::List => html! {
                        <ListView
                            places={(*places).clone()}
                            active_types={active_types(&filters)}
                            on_open={open_detail.clone()}
                            on_open_filters={open_filters.clone()}
                            on_toggle_type={on_toggle_type.clone()}
                            on_reset_filters={on_reset_filters.clone()}
                        />
                    },
                    Tab::Add => html! {
                        <AddForm
                            device_id={(*device).clone()}
                            origin={*origin}
                            on_created={on_created}
                            on_toast={show_toast.clone()}
                        />
                    },
                    Tab::Saved => html! {
                        <SavedView
                            places={(*saved_places).clone()}
                            on_open={open_detail.clone()}
                        />
                    },
                }
            }

            <TabBar tab={*tab} on_change={on_tab} />

            if let Some(d) = (*detail).clone() {
                <DetailView
                    detail={d}
                    device_id={(*device).clone()}
                    saved={detail.as_ref().is_some_and(|d| saved_ids.contains(&d.summary.id))}
                    on_close={close_detail}
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
