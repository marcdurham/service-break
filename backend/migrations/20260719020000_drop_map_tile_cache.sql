-- Map tile caching moved from Postgres to a disk-based LRU cache
-- (backend/src/tile_cache.rs, TILE_CACHE_DIR); the table is no longer
-- read or written, and its blobs would only bloat the database.
DROP TABLE map_tiles;
