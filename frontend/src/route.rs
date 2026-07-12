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
    /// Admin-only tools (backup export / import); reached from the Account
    /// page, not the tab bar.
    #[at("/admin")]
    Admin,
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
