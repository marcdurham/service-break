use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use yew::prelude::*;

use crate::glue;

#[derive(Properties, PartialEq)]
pub struct LocationPickerProps {
    /// Where to center the map when nothing is picked yet.
    pub center: (f64, f64),
    /// A previously picked/typed coordinate to start the pin at.
    pub initial: Option<(f64, f64)>,
    /// The user's live location, if they're sharing it — shown as a marker
    /// distinct from the dropped pin.
    pub user_location: Option<(f64, f64)>,
    pub on_confirm: Callback<(f64, f64)>,
    pub on_cancel: Callback<()>,
}

/// Full-screen map overlay for pointing at a spot (a street corner, a
/// building) when the address is unknown or you are no longer there.
#[function_component(LocationPicker)]
pub fn location_picker(props: &LocationPickerProps) -> Html {
    let picked = use_state(|| props.initial);

    {
        let picked = picked.clone();
        let initial = props.initial;
        let center = props.center;
        use_effect_with((), move |()| {
            let on_pick = {
                let picked = picked.clone();
                Closure::<dyn Fn(f64, f64)>::new(move |lat: f64, lng: f64| {
                    picked.set(Some((lat, lng)));
                })
            };
            let (lat, lng) = initial.unwrap_or(center);
            let zoom = if initial.is_some() { 17.0 } else { 15.0 };
            glue::sb_init_pick_map(
                "sb-pick-map",
                lat,
                lng,
                zoom,
                initial.is_some(),
                on_pick.as_ref().unchecked_ref(),
            );
            move || {
                glue::sb_destroy_pick_map();
                drop(on_pick);
            }
        });
    }

    use_effect_with(props.user_location, |user_location| {
        if let Some((lat, lng)) = *user_location {
            glue::sb_set_pick_user(lat, lng);
        }
    });

    let cancel = {
        let cb = props.on_cancel.clone();
        Callback::from(move |_| cb.emit(()))
    };
    let confirm = {
        let picked = picked.clone();
        let cb = props.on_confirm.clone();
        Callback::from(move |_| {
            if let Some(coords) = *picked {
                cb.emit(coords);
            }
        })
    };

    html! {
        <div class="picker-overlay">
            <div id="sb-pick-map"></div>
            <div class="picker-head">
                <button class="icon-btn" onclick={cancel}>
                    <span class="mi">{"arrow_back"}</span>
                </button>
                <div>
                    <div class="picker-title">{"Point to the spot"}</div>
                    <div class="picker-sub">{"Tap a street corner or building to drop a pin"}</div>
                </div>
            </div>
            <div class="picker-foot">
                <div class="picker-coords">
                    <span class="mi">{"pin_drop"}</span>
                    {
                        match *picked {
                            Some((lat, lng)) => shared::fmt_latlng(lat, lng),
                            None => "No pin dropped yet".to_owned(),
                        }
                    }
                </div>
                <button class="directions-btn" onclick={confirm} disabled={picked.is_none()}>
                    <span class="mi">{"where_to_vote"}</span>{"Use this spot"}
                </button>
            </div>
        </div>
    }
}
