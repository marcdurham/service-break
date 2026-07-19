use shared::PlaceType;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::app::Filters;

/// Selectable place types, in display order. `Other` stays excluded here,
/// matching the chip row this replaced.
const TYPE_OPTIONS: [PlaceType; 7] = [
    PlaceType::Shop,
    PlaceType::Store,
    PlaceType::Mall,
    PlaceType::Park,
    PlaceType::Public,
    PlaceType::Hall,
    PlaceType::FastFood,
];

#[derive(Properties, PartialEq)]
pub struct FiltersSheetProps {
    pub filters: Filters,
    pub count: usize,
    pub on_change: Callback<Filters>,
    pub on_reset: Callback<()>,
    pub on_close: Callback<()>,
    pub on_toggle_type: Callback<PlaceType>,
    /// Index into [`shared::MARKER_DENSITY_LEVELS`]. A device-wide display
    /// preference, not part of `Filters` — unaffected by "Reset".
    pub marker_density: u8,
    pub on_marker_density_change: Callback<u8>,
}

#[function_component(FiltersSheet)]
pub fn filters_sheet(props: &FiltersSheetProps) -> Html {
    let close_scrim = {
        let cb = props.on_close.clone();
        Callback::from(move |_| cb.emit(()))
    };
    let close_apply = {
        let cb = props.on_close.clone();
        Callback::from(move |_| cb.emit(()))
    };
    let reset = {
        let cb = props.on_reset.clone();
        Callback::from(move |_| cb.emit(()))
    };
    let stop = Callback::from(|e: MouseEvent| e.stop_propagation());

    type Toggle = (&'static str, &'static str, bool, fn(&mut Filters));
    let toggles: [Toggle; 4] = [
        ("Clean spots only (4.0+)", "mop", props.filters.clean_only,
            |f| f.clean_only = !f.clean_only),
        ("Has parking", "local_parking", props.filters.has_parking,
            |f| f.has_parking = !f.has_parking),
        ("No purchase required", "money_off", props.filters.no_purchase,
            |f| f.no_purchase = !f.no_purchase),
        ("Show unvisited places", "travel_explore", props.filters.show_unvisited,
            |f| f.show_unvisited = !f.show_unvisited),
    ];

    let on_radius = {
        let filters = props.filters.clone();
        let cb = props.on_change.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                if let Ok(pos) = el.value().parse::<u32>() {
                    let mut f = filters.clone();
                    f.radius_mi = shared::radius_from_slider(pos);
                    cb.emit(f);
                }
            }
        })
    };

    let on_density = {
        let cb = props.on_marker_density_change.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                if let Ok(level) = el.value().parse::<u8>() {
                    cb.emit(level);
                }
            }
        })
    };

    html! {
        <div class="sheet-scrim" onclick={close_scrim}>
            <div class="sheet" onclick={stop}>
                <div class="sheet-handle"></div>
                <div class="sheet-head">
                    <div class="sheet-title">{"Filters"}</div>
                    <button class="sheet-reset" onclick={reset}>{"Reset"}</button>
                </div>
                <div class="field-label">{"Type"}</div>
                <div class="chips sb-scroll">
                    { for TYPE_OPTIONS.into_iter().map(|t| {
                        let on = props.filters.types.contains(&t);
                        let onclick = {
                            let cb = props.on_toggle_type.clone();
                            Callback::from(move |_| cb.emit(t))
                        };
                        html! {
                            <button class={if on { "chip on" } else { "chip" }} {onclick}>
                                <span class="mi">{t.icon()}</span>{t.label()}
                            </button>
                        }
                    }) }
                </div>
                <div class="ftoggles">
                    { for toggles.into_iter().map(|(label, icon, on, flip)| {
                        let onclick = {
                            let filters = props.filters.clone();
                            let cb = props.on_change.clone();
                            Callback::from(move |_| {
                                let mut f = filters.clone();
                                flip(&mut f);
                                cb.emit(f);
                            })
                        };
                        html! {
                            <div class={if on { "ftoggle on" } else { "ftoggle" }} {onclick}>
                                <span class="mi">{icon}</span>
                                <span class="ftoggle-label">{label}</span>
                                <div class={if on { "switch on" } else { "switch" }}>
                                    <div class="switch-knob"></div>
                                </div>
                            </div>
                        }
                    }) }
                </div>
                <div class="field-label">{format!("Within {} mi", props.filters.radius_mi)}</div>
                <input
                    type="range"
                    class="range"
                    min="0"
                    max={shared::RADIUS_SLIDER_STEPS.to_string()}
                    step="1"
                    value={shared::radius_to_slider(props.filters.radius_mi).to_string()}
                    oninput={on_radius}
                />
                <div class="field-label">
                    {format!("Marker density: {}", shared::marker_density_label(props.marker_density))}
                </div>
                <input
                    type="range"
                    class="range"
                    min="0"
                    max={(shared::MARKER_DENSITY_LEVELS.len() - 1).to_string()}
                    step="1"
                    value={props.marker_density.to_string()}
                    oninput={on_density}
                />
                <button class="sheet-apply" onclick={close_apply}>
                    {format!("Show {} places", props.count)}
                </button>
            </div>
        </div>
    }
}
