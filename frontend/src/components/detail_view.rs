use shared::{Aspect, NewReview, PlaceDetail, PlaceEdit, PlaceSummary};
use uuid::Uuid;
use web_sys::HtmlTextAreaElement;
use yew::prelude::*;

use crate::api;
use crate::components::edit_view::EditView;
use crate::components::ui;

const AVATAR_COLORS: [&str; 4] = ["#c05f38", "#6f8256", "#9b6a7d", "#4f7a86"];

#[derive(Properties, PartialEq)]
pub struct DetailViewProps {
    pub detail: PlaceDetail,
    pub device_id: String,
    pub saved: bool,
    /// Posting a review requires an account; when false the composer
    /// buttons emit `on_require_login` instead of opening the composer.
    pub logged_in: bool,
    pub on_require_login: Callback<()>,
    pub on_close: Callback<()>,
    pub on_show_on_map: Callback<PlaceSummary>,
    pub on_toggle_save: Callback<Uuid>,
    pub on_updated: Callback<PlaceDetail>,
    pub on_deleted: Callback<()>,
    pub on_toast: Callback<String>,
}

#[function_component(DetailView)]
pub fn detail_view(props: &DetailViewProps) -> Html {
    let composing = use_state(|| false);
    let clean_pick = use_state(|| 5i16);
    let coffee_pick = use_state(|| None::<i16>);
    let food_pick = use_state(|| None::<i16>);
    let text = use_state(String::new);
    let editing = use_state(|| false);
    let edits = use_state(Vec::<PlaceEdit>::new);
    // Bumped after each save so the change history below refetches.
    let edits_refresh = use_state(|| 0u32);

    let p = &props.detail.summary;
    let d = &props.detail;

    // The audited edit history for this place — what changed, when, by whom.
    {
        let edits = edits.clone();
        use_effect_with((p.id, *edits_refresh), move |(id, _)| {
            let id = *id;
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(list) = api::fetch_place_edits(id).await {
                    edits.set(list);
                }
            });
        });
    }

    let close = {
        let cb = props.on_close.clone();
        Callback::from(move |_| cb.emit(()))
    };
    let toggle_save = {
        let cb = props.on_toggle_save.clone();
        let id = p.id;
        Callback::from(move |_| cb.emit(id))
    };
    let directions = {
        let place = p.clone();
        Callback::from(move |_| ui::open_directions(&place))
    };
    let show_on_map = {
        let cb = props.on_show_on_map.clone();
        let place = p.clone();
        Callback::from(move |_| cb.emit(place.clone()))
    };
    let open_composer = {
        let composing = composing.clone();
        let logged_in = props.logged_in;
        let require_login = props.on_require_login.clone();
        Callback::from(move |_| {
            if logged_in {
                composing.set(true);
            } else {
                require_login.emit(());
            }
        })
    };
    let cancel_composer = {
        let composing = composing.clone();
        Callback::from(move |_| composing.set(false))
    };
    let open_editor = {
        let editing = editing.clone();
        let logged_in = props.logged_in;
        let require_login = props.on_require_login.clone();
        Callback::from(move |_| {
            if logged_in {
                editing.set(true);
            } else {
                require_login.emit(());
            }
        })
    };
    let close_editor = {
        let editing = editing.clone();
        Callback::from(move |()| editing.set(false))
    };
    let on_saved = {
        let editing = editing.clone();
        let edits_refresh = edits_refresh.clone();
        let on_updated = props.on_updated.clone();
        let on_toast = props.on_toast.clone();
        Callback::from(move |detail: PlaceDetail| {
            editing.set(false);
            edits_refresh.set(edits_refresh.wrapping_add(1));
            on_updated.emit(detail);
            on_toast.emit("Changes saved".to_owned());
        })
    };
    let delete_place = {
        let id = p.id;
        let logged_in = props.logged_in;
        let require_login = props.on_require_login.clone();
        let on_close = props.on_close.clone();
        let on_deleted = props.on_deleted.clone();
        let on_toast = props.on_toast.clone();
        Callback::from(move |_| {
            if !logged_in {
                require_login.emit(());
                return;
            }
            let on_toast = on_toast.clone();
            let on_close = on_close.clone();
            let on_deleted = on_deleted.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::delete_place(id).await {
                    Ok(()) => {
                        on_toast.emit("Place removed".to_owned());
                        on_deleted.emit(());
                        on_close.emit(());
                    }
                    Err(msg) => on_toast.emit(msg),
                }
            });
        })
    };

    let share_place = {
        let on_toast = props.on_toast.clone();
        Callback::from(move |_| {
            if let Some(window) = web_sys::window() {
                match window.location().href() {
                    Ok(url) => {
                        let navigator = window.navigator();
                        let clipboard = navigator.clipboard();
                        let on_toast = on_toast.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            match clipboard.write_text(&url).await {
                                Ok(_) => on_toast.emit("Link copied to clipboard".to_owned()),
                                Err(_) => on_toast.emit("Could not copy link".to_owned()),
                            }
                        });
                    }
                    Err(_) => {
                        on_toast.emit("Could not get URL".to_owned());
                    }
                }
            } else {
                on_toast.emit("Window not available".to_owned());
            }
        })
    };





    let send_review = {
        let composing = composing.clone();
        let clean_pick = clean_pick.clone();
        let coffee_pick = coffee_pick.clone();
        let food_pick = food_pick.clone();
        let text = text.clone();
        let device = props.device_id.clone();
        let id = p.id;
        let on_updated = props.on_updated.clone();
        let on_toast = props.on_toast.clone();
        Callback::from(move |_| {
            let review = NewReview {
                device_id: device.clone(),
                clean: *clean_pick,
                coffee: *coffee_pick,
                food: *food_pick,
                text: (*text).trim().to_owned(),
            };
            let composing = composing.clone();
            let coffee_pick = coffee_pick.clone();
            let food_pick = food_pick.clone();
            let text = text.clone();
            let on_updated = on_updated.clone();
            let on_toast = on_toast.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::create_review(id, &review).await {
                    Ok(detail) => {
                        composing.set(false);
                        coffee_pick.set(None);
                        food_pick.set(None);
                        text.set(String::new());
                        on_updated.emit(detail);
                        on_toast.emit("Review added — thanks, scout!".to_owned());
                    }
                    Err(msg) => on_toast.emit(msg),
                }
            });
        })
    };

    let rating = p.clean_avg.unwrap_or(0.0);
    let access = ui::access_label(p.purchase_required, p.code_required);
    let access_class = ui::access_class(p.purchase_required, p.code_required);

    html! {
        <div class="detail-overlay">
            <div class="detail-scroll sb-scroll">
                <div class="hero" style={format!("background:{}", p.place_type.color())}>
                    <span class="mi">{p.place_type.icon()}</span>
                    <div class="hero-btns">
                        <button class="hero-btn" onclick={close}>
                            <span class="mi">{"arrow_back"}</span>
                        </button>
                        <button class="hero-btn" onclick={toggle_save}>
                            <span class={if props.saved { "mi saved" } else { "mi" }}>
                                {if props.saved { "bookmark" } else { "bookmark_border" }}
                            </span>
                        </button>
                    </div>
                </div>
                <div class="detail-body">
                    <div class="detail-meta">
                        <span class="detail-type">{p.place_type.label()}</span>
                        if let Some(hours) = &d.hours {
                            <span class="open-dot"></span>
                            <span class="detail-hours-label">{hours.clone()}</span>
                        }
                    </div>
                    <div class="detail-name">{&p.name}</div>
                    <div class="stars-row">
                        { ui::stars(rating) }
                        <span class="rating-num">{ui::clean_label(p.clean_avg)}</span>
                        <span class="rating-count">
                            {format!("({} review{})", p.review_count, if p.review_count == 1 { "" } else { "s" })}
                        </span>
                    </div>

                    <div class="action-row">
                        <button class="directions-btn" onclick={directions}>
                            <span class="mi">{"directions"}</span>{"Directions"}
                        </button>
                        <button class="rate-btn" onclick={show_on_map}>
                            <span class="mi">{"map"}</span>{"Map"}
                        </button>
                        <button class="rate-btn" onclick={open_composer.clone()}>
                            <span class="mi">{"rate_review"}</span>{"Rate"}
                        </button>
                        <button class="rate-btn" onclick={open_editor}>
                            <span class="mi">{"edit"}</span>{"Edit"}
                        </button>
                        <button class="rate-btn" onclick={share_place}>
                            <span class="mi">{"share"}</span>{"Share"}
                        </button>
                        <button class="rate-btn delete-btn" onclick={delete_place}>
                            <span class="mi">{"delete_forever"}</span>{"Delete"}
                        </button>
                    </div>

                    <div class="breakdown">
                        // One bar per rated aspect: cleanliness always shows
                        // (it's the app's core rating); coffee and food only
                        // once at least one review has scored them.
                        { for Aspect::ALL.into_iter().filter_map(|a| {
                            let avg = match a {
                                Aspect::Cleanliness => p.clean_avg,
                                Aspect::Coffee => p.coffee_avg,
                                Aspect::Food => p.food_avg,
                            };
                            if a != Aspect::Cleanliness && avg.is_none() {
                                return None;
                            }
                            let pct = (avg.unwrap_or(0.0) / 5.0 * 100.0).clamp(0.0, 100.0);
                            Some(html! {
                                <div class="brow" key={a.as_str()}>
                                    <span class="mi" style={format!("color:{}", a.color())}>{a.icon()}</span>
                                    <div class="brow-main">
                                        <div class="brow-title">{a.label()}</div>
                                        <div class="bbar">
                                            <div class="bbar-fill" style={format!("width:{pct}%")}></div>
                                        </div>
                                    </div>
                                    <span class="brow-val">{ui::clean_label(avg)}</span>
                                </div>
                            })
                        }) }
                        <div class="brow">
                            <span class="mi" style="color:#b09a82">{"directions_walk"}</span>
                            <div class="brow-main">
                                <div class="brow-title">{"Bathroom distance from door"}</div>
                                if !d.door_note.is_empty() {
                                    <div class="brow-sub">{d.door_note.clone()}</div>
                                }
                            </div>
                            <span class="brow-val plain">{shared::door_short(p.door_ft)}</span>
                        </div>
                        <div class="brow">
                            <span class="mi" style="color:#c08a4a">{"local_parking"}</span>
                            <div class="brow-main">
                                <div class="brow-title">{"Parking"}</div>
                            </div>
                            { ui::parking_tag(p.parking) }
                        </div>
                        <div class="brow">
                            <span class="mi" style="color:#a8795a">{"key"}</span>
                            <div class="brow-main">
                                <div class="brow-title">{"Bathroom access"}</div>
                            </div>
                            <span class={access_class}>{access}</span>
                        </div>
                    </div>

                    if !p.amenities.is_empty() {
                        <div class="field-label">{"This place has"}</div>
                        <div class="tag-row">
                            { for p.amenities.iter().map(|a| html! {
                                <span class="tag tag-amenity" key={a.as_str()}>
                                    <span class="mi">{a.icon()}</span>{a.label()}
                                </span>
                            }) }
                        </div>
                    }

                    if !p.address.is_empty() {
                        <div class="addr-card">
                            <span class="mi">{"location_on"}</span>
                            <div>
                                <div class="addr-line">{&p.address}</div>
                                if let Some(hours) = &d.hours {
                                    <div class="addr-sub">{hours.clone()}</div>
                                }
                            </div>
                        </div>
                    }

                    <div class="reviews-head">
                        <div class="reviews-title">{"Reviews"}</div>
                        <button class="use-loc" onclick={open_composer}>
                            <span class="mi">{"add"}</span>{"Add yours"}
                        </button>
                    </div>

                    if *composing {
                        <div class="composer">
                            <div class="field-label">{"Bathroom cleanliness"}</div>
                            <div class="clean-row">
                                { for (1..=5i16).map(|n| {
                                    let on = n <= *clean_pick;
                                    let onclick = {
                                        let clean_pick = clean_pick.clone();
                                        Callback::from(move |_| clean_pick.set(n))
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
                            <div class="field-label">{"Coffee "}<span class="lite">{"· optional"}</span></div>
                            { ui::aspect_score_picker(*coffee_pick, &{
                                let coffee_pick = coffee_pick.clone();
                                Callback::from(move |v| coffee_pick.set(v))
                            }) }
                            <div class="field-label">{"Food "}<span class="lite">{"· optional"}</span></div>
                            { ui::aspect_score_picker(*food_pick, &{
                                let food_pick = food_pick.clone();
                                Callback::from(move |v| food_pick.set(v))
                            }) }
                            <textarea
                                class="textarea"
                                placeholder="How was it?"
                                value={(*text).clone()}
                                oninput={{
                                    let text = text.clone();
                                    Callback::from(move |e: InputEvent| {
                                        if let Some(el) = e.target_dyn_into::<HtmlTextAreaElement>() {
                                            text.set(el.value());
                                        }
                                    })
                                }}
                            ></textarea>
                            <div class="composer-actions">
                                <button class="composer-send" onclick={send_review}>{"Post review"}</button>
                                <button class="composer-cancel" onclick={cancel_composer}>{"Cancel"}</button>
                            </div>
                        </div>
                    }

                    <div class="cards" style="padding-bottom:24px">
                        { for d.reviews.iter().enumerate().map(|(i, r)| html! {
                            <div class="review-card" key={r.id.to_string()}>
                                <div class="review-top">
                                    <div
                                        class="avatar"
                                        style={format!("background:{}", AVATAR_COLORS[i % AVATAR_COLORS.len()])}
                                    >
                                        {r.author.chars().next().unwrap_or('S')}
                                    </div>
                                    <div class="review-who">
                                        <div class="review-name">{&r.author}</div>
                                        <div class="review-time">{&r.time_ago}</div>
                                    </div>
                                    { ui::stars(f64::from(r.clean)) }
                                </div>
                                if r.coffee.is_some() || r.food.is_some() {
                                    <div class="tag-row">
                                        if let Some(score) = r.coffee {
                                            <span class="tag tag-amenity">
                                                <span class="mi">{Aspect::Coffee.icon()}</span>
                                                {format!("Coffee {score}/5")}
                                            </span>
                                        }
                                        if let Some(score) = r.food {
                                            <span class="tag tag-amenity">
                                                <span class="mi">{Aspect::Food.icon()}</span>
                                                {format!("Food {score}/5")}
                                            </span>
                                        }
                                    </div>
                                }
                                if !r.text.is_empty() {
                                    <div class="review-text">{&r.text}</div>
                                }
                            </div>
                        }) }
                    </div>

                    if !edits.is_empty() {
                        <div class="reviews-head" style="margin-top:0">
                            <div class="reviews-title">{"Change history"}</div>
                        </div>
                        <div class="cards" style="padding-bottom:24px">
                            { for edits.iter().map(|e| html! {
                                <div class="review-card">
                                    <div class="review-name">
                                        {format!(
                                            "{}: {} → {}",
                                            shared::edit_field_label(&e.field),
                                            shared::edit_value_display(&e.old_value),
                                            shared::edit_value_display(&e.new_value),
                                        )}
                                    </div>
                                    <div class="review-time">
                                        {format!("by {} · {}", e.author, e.time_ago)}
                                    </div>
                                </div>
                            }) }
                        </div>
                    }
                </div>
            </div>

            if *editing {
                <EditView
                    detail={props.detail.clone()}
                    on_close={close_editor}
                    on_saved={on_saved}
                    on_toast={props.on_toast.clone()}
                />
            }
        </div>
    }
}
