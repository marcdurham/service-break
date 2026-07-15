//! Lands here when the browser dispatches Android's Web Share Target intent
//! at us (e.g. the user tapped "Share" on a place in Google Maps and picked
//! this app), with `title`/`text`/`url` in the query string per the
//! `share_target` entry in `manifest.webmanifest`. This component's only
//! job is to find a Google Maps link among those params, resolve it once,
//! and hand off to the same flow the map search box uses for pasted links.

use shared::{query_param, MapsLinkResult};
use yew::prelude::*;
use yew_router::hooks::use_navigator;

use crate::api;
use crate::maps_link::extract_google_maps_link;
use crate::route::Route;

#[derive(Properties, PartialEq)]
pub struct ShareTargetViewProps {
    pub on_maps_link: Callback<MapsLinkResult>,
    pub on_toast: Callback<String>,
}

fn search() -> String {
    web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default()
}

#[function_component(ShareTargetView)]
pub fn share_target_view(props: &ShareTargetViewProps) -> Html {
    let navigator = use_navigator().expect("BrowserRouter provides a navigator");
    {
        let on_maps_link = props.on_maps_link.clone();
        let on_toast = props.on_toast.clone();
        let navigator = navigator.clone();
        use_effect_with((), move |()| {
            let search = search();
            let link = ["url", "text", "title"]
                .into_iter()
                .filter_map(|key| query_param(&search, key))
                .find_map(|v| extract_google_maps_link(&v));
            match link {
                Some(link) => {
                    wasm_bindgen_futures::spawn_local(async move {
                        match api::resolve_maps_link(&link).await {
                            Ok(place) => on_maps_link.emit(place),
                            Err(msg) => {
                                on_toast.emit(msg);
                                navigator.replace(&Route::Map);
                            }
                        }
                    });
                }
                None => {
                    on_toast.emit("No Google Maps link found in what was shared".to_owned());
                    navigator.replace(&Route::Map);
                }
            }
            || ()
        });
    }
    html! {
        <div class="screen sb-scroll">
            <div class="screen-title">{"Adding place from shared link…"}</div>
        </div>
    }
}
