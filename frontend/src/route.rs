//! URL routes for the app's top-level pages, plus per-place deep links.

use std::fmt;
use std::str::FromStr;

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
    /// Edit an unvisited Overpass POI (OSM ref `kind/num`, e.g.
    /// `node/123456`). Editing needs an account, so this URL shows the
    /// sign-in page to signed-out users; because the destination is the
    /// URL itself, finishing sign-in — including the full-page Google
    /// OAuth round trip — lands them right back in the edit flow, where
    /// the POI is promoted and its editor opened.
    #[at("/poi/:kind/:num/edit")]
    PoiEdit { kind: OsmKind, num: u64 },
    #[not_found]
    #[at("/404")]
    NotFound,
}

impl Route {
    /// True for the top-level pages shown in the tab bar; false for
    /// overlay-style routes (the place detail sheet, the POI edit gate)
    /// and the 404 fallback.
    pub fn is_nav(self) -> bool {
        !matches!(self, Route::Place { .. } | Route::PoiEdit { .. } | Route::NotFound)
    }

    /// The edit route for an Overpass POI id like `"node/123456"`;
    /// `None` when the id isn't in the `kind/number` form.
    pub fn poi_edit(poi_id: &str) -> Option<Route> {
        let (kind, num) = poi_id.split_once('/')?;
        Some(Route::PoiEdit { kind: kind.parse().ok()?, num: num.parse().ok()? })
    }
}

/// The element-kind half of an OSM ref (`node/123456` → `Node`), a small
/// dedicated `Copy` type so [`Route`] itself can stay `Copy`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OsmKind {
    Node,
    Way,
    Relation,
}

impl OsmKind {
    pub fn as_str(self) -> &'static str {
        match self {
            OsmKind::Node => "node",
            OsmKind::Way => "way",
            OsmKind::Relation => "relation",
        }
    }
}

impl fmt::Display for OsmKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for OsmKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        match s {
            "node" => Ok(OsmKind::Node),
            "way" => Ok(OsmKind::Way),
            "relation" => Ok(OsmKind::Relation),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poi_edit_parses_osm_refs() {
        assert_eq!(
            Route::poi_edit("node/123456"),
            Some(Route::PoiEdit { kind: OsmKind::Node, num: 123456 })
        );
        assert_eq!(
            Route::poi_edit("way/7"),
            Some(Route::PoiEdit { kind: OsmKind::Way, num: 7 })
        );
        assert_eq!(Route::poi_edit("building/9"), None);
        assert_eq!(Route::poi_edit("node/abc"), None);
        assert_eq!(Route::poi_edit("node"), None);
    }

    #[test]
    fn poi_edit_route_round_trips_through_its_url() {
        let route = Route::poi_edit("node/123456").unwrap();
        assert_eq!(route.to_path(), "/poi/node/123456/edit");
        assert_eq!(Route::recognize("/poi/node/123456/edit"), Some(route));
    }
}
