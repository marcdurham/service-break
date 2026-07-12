use std::collections::HashSet;

use shared::{Amenity, NewPlace, Parking, PlaceDetail, PlaceType, Requirement};
use web_sys::{HtmlInputElement, HtmlTextAreaElement};
use yew::prelude::*;

use crate::api;
use crate::app::FALLBACK_CENTER;
use crate::components::location_picker::LocationPicker;
use crate::components::ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Door {
    AtEntrance,
    ShortWalk,
    Back,
    Upstairs,
}

impl Door {
    const ALL: [Door; 4] = [Door::AtEntrance, Door::ShortWalk, Door::Back, Door::Upstairs];

    fn label(self) -> &'static str {
        match self {
            Door::AtEntrance => "Right by door",
            Door::ShortWalk => "Short walk in",
            Door::Back => "Toward the back",
            Door::Upstairs => "Up a flight",
        }
    }

    fn ft(self) -> i32 {
        match self {
            Door::AtEntrance => 10,
            Door::ShortWalk => 40,
            Door::Back => 90,
            Door::Upstairs => 45,
        }
    }

    fn note(self) -> &'static str {
        match self {
            Door::AtEntrance => "Right by the entrance",
            Door::ShortWalk => "A short walk inside",
            Door::Back => "Toward the back",
            Door::Upstairs => "Up a flight of stairs",
        }
    }
}

#[derive(Clone, PartialEq)]
struct Form {
    name: String,
    address: String,
    place_type: PlaceType,
    clean: i16,
    coffee: Option<i16>,
    food: Option<i16>,
    door: Door,
    parking: Parking,
    purchase: Requirement,
    code: Requirement,
    amenities: HashSet<Amenity>,
    comment: String,
}

impl Default for Form {
    fn default() -> Self {
        Form {
            name: String::new(),
            address: String::new(),
            place_type: PlaceType::Shop,
            clean: 5,
            coffee: None,
            food: None,
            door: Door::AtEntrance,
            parking: Parking::Easy,
            purchase: Requirement::Unknown,
            code: Requirement::Unknown,
            amenities: HashSet::new(),
            comment: String::new(),
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct AddFormProps {
    pub device_id: String,
    pub origin: Option<(f64, f64)>,
    /// The user's live location, if they're sharing it (as opposed to
    /// `origin`, which may be a fallback center).
    pub user_location: Option<(f64, f64)>,
    /// Adding a place requires an account; when false the form is replaced
    /// by a sign-in prompt that emits `on_sign_in`.
    pub logged_in: bool,
    pub on_created: Callback<PlaceDetail>,
    pub on_sign_in: Callback<()>,
    pub on_toast: Callback<String>,
}

#[function_component(AddForm)]
pub fn add_form(props: &AddFormProps) -> Html {
    let form = use_state(Form::default);
    let submitting = use_state(|| false);
    let picking = use_state(|| false);

    // After the hooks so the hook count stays the same once the user logs in.
    if !props.logged_in {
        let sign_in = {
            let cb = props.on_sign_in.clone();
            Callback::from(move |_| cb.emit(()))
        };
        return html! {
            <div class="screen sb-scroll">
                <div class="screen-title">{"Add a place"}</div>
                <div class="screen-sub">{"Help the next traveler find a clean break."}</div>
                <div class="gate-card">
                    <span class="mi">{"lock"}</span>
                    <div class="gate-title">{"Sign in to add places"}</div>
                    <div class="gate-sub">
                        {"Adding places, reviews and saves needs an account, \
                          so every tip has a scout behind it."}
                    </div>
                    <button class="submit-btn" onclick={sign_in}>
                        <span class="mi">{"login"}</span>{"Sign in"}
                    </button>
                </div>
            </div>
        };
    }

    let set =|f: &UseStateHandle<Form>, update: fn(&mut Form, String)| {
        let f = f.clone();
        Callback::from(move |value: String| {
            let mut next = (*f).clone();
            update(&mut next, value);
            f.set(next);
        })
    };
    let on_name = set(&form, |f, v| f.name = v);
    let on_address = set(&form, |f, v| f.address = v);
    let on_comment = set(&form, |f, v| f.comment = v);

    let use_my_location = {
        let form = form.clone();
        let origin = props.origin;
        let toast = props.on_toast.clone();
        Callback::from(move |_| match origin {
            Some((lat, lng)) => {
                let mut next = (*form).clone();
                next.address = shared::fmt_latlng(lat, lng);
                form.set(next);
                toast.emit("Using your current location".to_owned());
            }
            None => toast.emit("Location not available yet".to_owned()),
        })
    };

    let open_picker = {
        let picking = picking.clone();
        Callback::from(move |_| picking.set(true))
    };
    let cancel_picker = {
        let picking = picking.clone();
        Callback::from(move |()| picking.set(false))
    };
    let confirm_picker = {
        let form = form.clone();
        let picking = picking.clone();
        let toast = props.on_toast.clone();
        Callback::from(move |(lat, lng): (f64, f64)| {
            let mut next = (*form).clone();
            next.address = shared::fmt_latlng(lat, lng);
            form.set(next);
            picking.set(false);
            toast.emit("Location picked from the map".to_owned());
        })
    };

    let submit = {
        let form = form.clone();
        let submitting = submitting.clone();
        let props_device = props.device_id.clone();
        let on_created = props.on_created.clone();
        let on_toast = props.on_toast.clone();
        Callback::from(move |_| {
            let f = (*form).clone();
            if f.name.trim().is_empty() {
                on_toast.emit("Add a place name first".to_owned());
                return;
            }
            if f.address.trim().is_empty() {
                on_toast.emit("Add an address or use your location".to_owned());
                return;
            }
            if *submitting {
                return;
            }
            submitting.set(true);
            let new = NewPlace {
                device_id: props_device.clone(),
                name: f.name.trim().to_owned(),
                place_type: f.place_type,
                lat: None,
                lng: None,
                address: Some(f.address.trim().to_owned()),
                clean: f.clean,
                coffee: f.coffee,
                food: f.food,
                door_ft: f.door.ft(),
                door_note: f.door.note().to_owned(),
                parking: f.parking,
                purchase_required: f.purchase,
                code_required: f.code,
                amenities: f.amenities.iter().copied().collect(),
                comment: f.comment.trim().to_owned(),
            };
            let form = form.clone();
            let submitting = submitting.clone();
            let on_created = on_created.clone();
            let on_toast = on_toast.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::create_place(&new).await {
                    Ok(detail) => {
                        form.set(Form::default());
                        on_created.emit(detail);
                    }
                    Err(msg) => on_toast.emit(msg),
                }
                submitting.set(false);
            });
        })
    };

    html! {
        <>
        <div class="screen sb-scroll">
            <div class="screen-title">{"Add a place"}</div>
            <div class="screen-sub">{"Help the next traveler find a clean break."}</div>

            <div class="field-label">{"Place name"}</div>
            <input
                class="input"
                placeholder="e.g. Camber Coffee"
                value={form.name.clone()}
                oninput={Callback::from(move |e: InputEvent| {
                    if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                        on_name.emit(el.value());
                    }
                })}
            />

            <div class="field-label">{"Address "}<span class="lite">{"· or lat, lng"}</span></div>
            <div class="input-row">
                <span class="mi">{"location_on"}</span>
                <input
                    placeholder="214 Maple Ave  ·  or  37.7749, -122.4194"
                    value={form.address.clone()}
                    oninput={Callback::from(move |e: InputEvent| {
                        if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                            on_address.emit(el.value());
                        }
                    })}
                />
            </div>
            <div class="loc-btns">
                <button class="use-loc" onclick={use_my_location}>
                    <span class="mi">{"my_location"}</span>{"Use my current location"}
                </button>
                <button class="use-loc" onclick={open_picker}>
                    <span class="mi">{"pin_drop"}</span>{"Pick on the map"}
                </button>
            </div>

            <div class="field-label">{"Type of place"}</div>
            <div class="type-grid">
                { for PlaceType::ALL.into_iter().map(|t| {
                    let on = form.place_type == t;
                    let onclick = {
                        let form = form.clone();
                        Callback::from(move |_| {
                            let mut next = (*form).clone();
                            next.place_type = t;
                            form.set(next);
                        })
                    };
                    html! {
                        <button class={if on { "tile on" } else { "tile" }} {onclick}>
                            <span class="mi">{t.icon()}</span>
                            <span>{t.label()}</span>
                        </button>
                    }
                }) }
            </div>

            <div class="field-label">{"This place has"}</div>
            <div class="type-grid">
                { for Amenity::ALL.into_iter().map(|a| {
                    let on = form.amenities.contains(&a);
                    let onclick = {
                        let form = form.clone();
                        Callback::from(move |_| {
                            let mut next = (*form).clone();
                            if !next.amenities.remove(&a) {
                                next.amenities.insert(a);
                            }
                            form.set(next);
                        })
                    };
                    html! {
                        <button class={if on { "tile on" } else { "tile" }} {onclick}>
                            <span class="mi">{a.icon()}</span>
                            <span>{a.label()}</span>
                        </button>
                    }
                }) }
            </div>

            <div class="field-label">{"Bathroom cleanliness"}</div>
            <div class="clean-row">
                { for (1..=5i16).map(|n| {
                    let on = n <= form.clean;
                    let onclick = {
                        let form = form.clone();
                        Callback::from(move |_| {
                            let mut next = (*form).clone();
                            next.clean = n;
                            form.set(next);
                        })
                    };
                    html! {
                        <button class={if on { "clean-pick on" } else { "clean-pick" }} {onclick}>
                            <span class="mi">
                                {if on { "sentiment_very_satisfied" } else { "sentiment_neutral" }}
                            </span>
                        </button>
                    }
                }) }
            </div>

            <div class="field-label">{"Coffee "}<span class="lite">{"· optional, if they serve it"}</span></div>
            { ui::aspect_score_picker(form.coffee, &{
                let form = form.clone();
                Callback::from(move |v| {
                    let mut next = (*form).clone();
                    next.coffee = v;
                    form.set(next);
                })
            }) }

            <div class="field-label">{"Food "}<span class="lite">{"· optional, if they serve it"}</span></div>
            { ui::aspect_score_picker(form.food, &{
                let form = form.clone();
                Callback::from(move |v| {
                    let mut next = (*form).clone();
                    next.food = v;
                    form.set(next);
                })
            }) }

            <div class="field-label">{"How far is bathroom from the door?"}</div>
            <div class="pick-grid-2">
                { for Door::ALL.into_iter().map(|d| {
                    let on = form.door == d;
                    let onclick = {
                        let form = form.clone();
                        Callback::from(move |_| {
                            let mut next = (*form).clone();
                            next.door = d;
                            form.set(next);
                        })
                    };
                    html! {
                        <button class={if on { "pill on" } else { "pill" }} {onclick}>{d.label()}</button>
                    }
                }) }
            </div>

            <div class="field-label">{"Parking"}</div>
            <div class="pick-grid-3">
                { for [Parking::Easy, Parking::Street, Parking::None].into_iter().map(|p| {
                    let on = form.parking == p;
                    let label = if p == Parking::None { "None" } else { p.label() };
                    let onclick = {
                        let form = form.clone();
                        Callback::from(move |_| {
                            let mut next = (*form).clone();
                            next.parking = p;
                            form.set(next);
                        })
                    };
                    html! {
                        <button class={if on { "pill on" } else { "pill" }} {onclick}>{label}</button>
                    }
                }) }
            </div>

            { requirement_row(&form, "Purchase required?", "Do you need to buy something to use it?",
                |f| f.purchase, |f, v| f.purchase = v) }
            { requirement_row(&form, "Code required?", "Do you need a door code or a key?",
                |f| f.code, |f, v| f.code = v) }

            <div class="field-label">{"Comment"}</div>
            <textarea
                class="textarea"
                placeholder="Anything worth knowing? (code location, accessibility, tips…)"
                value={form.comment.clone()}
                oninput={Callback::from(move |e: InputEvent| {
                    if let Some(el) = e.target_dyn_into::<HtmlTextAreaElement>() {
                        on_comment.emit(el.value());
                    }
                })}
            ></textarea>

            <button class="submit-btn" onclick={submit} disabled={*submitting}>
                <span class="mi">{"add_location_alt"}</span>
                {if *submitting { "Adding…" } else { "Add this place" }}
            </button>
        </div>

        if *picking {
            <LocationPicker
                center={props.origin.unwrap_or(FALLBACK_CENTER)}
                initial={shared::parse_latlng(&form.address)}
                user_location={props.user_location}
                on_confirm={confirm_picker}
                on_cancel={cancel_picker}
            />
        }
        </>
    }
}

fn requirement_row(
    form: &UseStateHandle<Form>,
    title: &'static str,
    sub: &'static str,
    get: fn(&Form) -> Requirement,
    set: fn(&mut Form, Requirement),
) -> Html {
    let current = get(form);
    html! {
        <div style="margin-top:18px">
            <div class="toggle-title">{title}</div>
            <div class="toggle-sub" style="margin-bottom:8px">{sub}</div>
            <div class="pick-grid-3">
                { for Requirement::ALL.into_iter().map(|r| {
                    let on = current == r;
                    let onclick = {
                        let form = form.clone();
                        Callback::from(move |_| {
                            let mut next = (*form).clone();
                            set(&mut next, r);
                            form.set(next);
                        })
                    };
                    html! {
                        <button class={if on { "pill on" } else { "pill" }} {onclick}>{r.label()}</button>
                    }
                }) }
            </div>
        </div>
    }
}
