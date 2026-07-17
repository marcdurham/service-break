//! URL routes for the app's top-level pages, plus per-place deep links.

use uuid::Uuid;
use yew_router::Routable;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Routable)]
pub enum Route {
    #[at("/")]
    Map,
    #[at("/list")]
    List,
    #[at("/add")]
    Add,
    #[at("/saved")]
    Saved,
    #[at("/invite")]
    Invite,
    #[at("/account")]
    Account,
    /// The signed-in user's own activity log: logins, failed logins,
    /// profile changes, place edits and ratings.
    #[at("/activity")]
    Activity,
    #[at("/change-password")]
    ChangePassword,
    #[at("/register")]
    Register,
    /// Where the backend sends the browser back to after the Google OAuth
    /// round trip, with the session (or an error) in the URL fragment.
    #[at("/oauth-complete")]
    OauthComplete,
    /// Where Android's Web Share Target intent lands (e.g. "Share" on a
    /// place in Google Maps), with the shared link in the query string.
    #[at("/share-target")]
    ShareTarget,
    /// Admin-only tools (backup export / import); reached from the Account
    /// page, not the tab bar.
    #[at("/admin")]
    Admin,
    /// Admin-only: list all accounts.
    #[at("/users")]
    Users,
    /// Admin-only: edit a single account (reached from /users).
    #[at("/users/:id")]
    UserEdit { id: Uuid },
    /// Admin-only: one account's activity log (reached from /users/:id).
    #[at("/users/:id/activity")]
    UserActivity { id: Uuid },
    #[at("/place/:id")]
    Place { id: Uuid },
    #[not_found]
    #[at("/404")]
    NotFound,
}

impl Route {
    /// True for the top-level pages shown in the tab bar; false for
    /// overlay-style routes (the place detail sheet) and the 404 fallback.
    pub fn is_nav(self) -> bool {
        !matches!(self, Route::Place { .. } | Route::NotFound)
    }
}
