use yew::prelude::*;

use crate::glue;

#[derive(Properties, PartialEq)]
pub struct InviteViewProps {
    pub device_id: String,
    pub on_toast: Callback<String>,
}

/// A short, stable code derived from the device id so returning users keep
/// the same code without any backend support (there are no accounts in v1).
fn invite_code(device_id: &str) -> String {
    let hex: String = device_id.chars().filter(char::is_ascii_hexdigit).take(6).collect();
    format!("BREAK-{}", hex.to_uppercase())
}

#[function_component(InviteView)]
pub fn invite_view(props: &InviteViewProps) -> Html {
    let code = invite_code(&props.device_id);

    let copy_code = {
        let code = code.clone();
        let toast = props.on_toast.clone();
        Callback::from(move |_| {
            glue::sb_copy_text(&code);
            toast.emit("Invite code copied".to_owned());
        })
    };

    let share = {
        let code = code.clone();
        let toast = props.on_toast.clone();
        Callback::from(move |_| {
            let text =
                format!("Join me on Service Break — use code {code} to find clean bathrooms nearby.");
            let shared = glue::sb_share_invite(&text);
            let msg = if shared { "Opening share…" } else { "Invite link copied" };
            toast.emit(msg.to_owned());
        })
    };

    html! {
        <div class="screen sb-scroll">
            <div class="screen-title">{"Invite"}</div>
            <div class="screen-sub" style="margin-bottom:18px">
                {"Every scout makes the map better."}
            </div>
            <div class="invite-card">
                <div class="invite-badge"><span class="mi">{"group_add"}</span></div>
                <div class="invite-h1">{"Invite your fellow travelers"}</div>
                <div class="invite-p">
                    {"Share your code and help friends find a clean place to stop."}
                </div>
                <div class="invite-code-row">
                    <div>
                        <div class="invite-code-label">{"Your code"}</div>
                        <div class="invite-code">{code}</div>
                    </div>
                    <button class="invite-copy-btn" onclick={copy_code}>{"Copy"}</button>
                </div>
                <button class="invite-share-btn" onclick={share}>
                    <span class="mi">{"ios_share"}</span>{"Share invite"}
                </button>
            </div>
        </div>
    }
}
