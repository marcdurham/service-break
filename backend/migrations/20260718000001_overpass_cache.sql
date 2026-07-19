-- One row per fetched geohash tile; tracks TTL independent of how many (or
-- zero) POIs actually landed in it, so an empty-but-covered area doesn't get
-- re-fetched every pan.
CREATE TABLE overpass_tiles (
    tile_id TEXT PRIMARY KEY,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- One row per OSM element ever seen. `id` is the OSM ref ("node/123456",
-- "way/789012") and is globally unique across element types since the type
-- prefix disambiguates. Never deleted on promotion -- see app_place_id.
CREATE TABLE overpass_pois (
    id TEXT PRIMARY KEY,
    tile_id TEXT NOT NULL REFERENCES overpass_tiles(tile_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    place_type TEXT NOT NULL,
    lat DOUBLE PRECISION NOT NULL,
    lng DOUBLE PRECISION NOT NULL,
    address TEXT NOT NULL DEFAULT '',
    tags JSONB NOT NULL DEFAULT '{}',
    app_place_id UUID REFERENCES places(id) ON DELETE SET NULL,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_overpass_pois_bbox ON overpass_pois (lat, lng);
CREATE INDEX idx_overpass_pois_tile ON overpass_pois (tile_id);
CREATE INDEX idx_overpass_pois_app_place ON overpass_pois (app_place_id) WHERE app_place_id IS NOT NULL;
