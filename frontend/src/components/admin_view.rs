use shared::{AuthSession, ImportSummary};
use web_sys::HtmlTextAreaElement;
use yew::prelude::*;
use yew_router::hooks::use_navigator;

use crate::route::Route;
use crate::{api, glue};

#[derive(Properties, PartialEq)]
pub struct AdminViewProps {
    pub auth: Option<AuthSession>,
    pub on_toast: Callback<String>,
}

/// The admin menu (reached from the Account page): back up all data as a
/// single JSON download, and restore a backup by pasting it back in —
/// quick enough that "export → re-deploy → restore" is painless.
#[function_component(AdminView)]
pub fn admin_view(props: &AdminViewProps) -> Html {
    let busy = use_state(|| false);
    let import_text = use_state(String::new);
    let summary = use_state(|| None::<ImportSummary>);
    let navigator = use_navigator().expect("BrowserRouter provides a navigator");

    let go_to_users = {
        let navigator = navigator.clone();
        Callback::from(move |_| navigator.push(&Route::Users))
    };

    let is_admin = props.auth.as_ref().is_some_and(|s| s.is_admin);
    if !is_admin {
        let go_to_account = Callback::from(move |_| navigator.push(&Route::Account));
        return html! {
            <div class="screen sb-scroll">
                <div class="screen-title">{"Admin"}</div>
                <div class="screen-sub" style="margin-bottom:18px">
                    {"This page is for admin accounts only."}
                </div>
                <button class="alt-auth-btn" onclick={go_to_account}>
                    <span class="mi">{"login"}</span>{"Go to Account"}
                </button>
            </div>
        };
    }

    let export = {
        let busy = busy.clone();
        let on_toast = props.on_toast.clone();
        Callback::from(move |_| {
            if *busy {
                return;
            }
            busy.set(true);
            let busy = busy.clone();
            let on_toast = on_toast.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::export_backup().await {
                    Ok(json) => {
                        glue::sb_download("service-break-backup.json", &json);
                        on_toast.emit("Backup downloaded".to_owned());
                    }
                    Err(msg) => on_toast.emit(msg),
                }
                busy.set(false);
            });
        })
    };

    let on_import_input = {
        let import_text = import_text.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlTextAreaElement>() {
                import_text.set(el.value());
            }
        })
    };

    let import = {
        let busy = busy.clone();
        let import_text = import_text.clone();
        let summary = summary.clone();
        let on_toast = props.on_toast.clone();
        Callback::from(move |_| {
            let text = import_text.trim().to_owned();
            if *busy || text.is_empty() {
                return;
            }
            busy.set(true);
            let busy = busy.clone();
            let import_text = import_text.clone();
            let summary = summary.clone();
            let on_toast = on_toast.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::import_backup(&text).await {
                    Ok(s) => {
                        import_text.set(String::new());
                        on_toast.emit("Backup restored".to_owned());
                        summary.set(Some(s));
                    }
                    Err(msg) => on_toast.emit(msg),
                }
                busy.set(false);
            });
        })
    };

    html! {
        <div class="screen sb-scroll">
            <div class="screen-title">{"Admin"}</div>
            <div class="screen-sub" style="margin-bottom:18px">
                {"Back up all data, or restore a backup after a re-deploy."}
            </div>

            <button
                class="alt-auth-btn"
                onclick={go_to_users}
                style="margin-bottom:18px"
            >
                <span class="mi">{"people"}</span>{"View users"}
            </button>

            <div class="field-label">{"Export"}</div>
            <div class="auth-note" style="margin-bottom:8px">
                {"Downloads everything — places, reviews, saved lists, invitations \
                  and accounts — as one JSON file. Password hashes are never included."}
            </div>
            <button class="submit-btn" onclick={export} disabled={*busy}>
                <span class="mi">{"download"}</span>
                {if *busy { "One moment…" } else { "Export backup" }}
            </button>

            <div class="field-label" style="margin-top:18px">{"Import"}</div>
            <div class="auth-note" style="margin-bottom:8px">
                {"Paste the contents of a backup file and import it. This replaces \
                  the current data. Accounts that don't exist yet are recreated with \
                  new random passwords, shown below afterwards — passwords are never \
                  part of the backup itself."}
            </div>
            <textarea
                class="input"
                style="min-height:140px;font-family:monospace;font-size:12px"
                placeholder={"{ \"format_version\": 1, ... }"}
                value={(*import_text).clone()}
                oninput={on_import_input}
            />
            <button
                class="alt-auth-btn"
                onclick={import}
                disabled={*busy || import_text.trim().is_empty()}
            >
                <span class="mi">{"upload"}</span>
                {if *busy { "One moment…" } else { "Import backup" }}
            </button>

            if let Some(s) = &*summary {
                <div class="field-label" style="margin-top:18px">{"Import result"}</div>
                <div class="auth-note">
                    {format!(
                        "Restored {} accounts, {} places, {} reviews, {} saved places \
                         and {} invitations.",
                        s.users, s.places, s.reviews, s.saved_places, s.invitations
                    )}
                </div>
                if !s.new_passwords.is_empty() {
                    <div class="auth-note" style="margin-top:8px">
                        {"New passwords for recreated accounts — this is the only time \
                          they're shown, so pass them on now (or reset them later with \
                          scripts/change-password.sh):"}
                    </div>
                    <div style="font-family:monospace;font-size:13px;margin-top:8px">
                        { for s.new_passwords.iter().map(|(user, pass)| html! {
                            <div style="padding:2px 0">{format!("{user}: {pass}")}</div>
                        }) }
                    </div>
                }
            }
        </div>
    }
}
