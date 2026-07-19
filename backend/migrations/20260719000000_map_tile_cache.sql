-- Server-side cache of raster map tiles proxied from the OSM tile server
-- (see src/tiles.rs), so repeat map views are served locally instead of
-- re-hitting tile.openstreetmap.org. Rows older than the 30-day TTL are
-- refreshed in place on next request, never deleted.
CREATE TABLE map_tiles (
    z INT NOT NULL,
    x INT NOT NULL,
    y INT NOT NULL,
    body BYTEA NOT NULL,
    content_type TEXT NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (z, x, y)
);
