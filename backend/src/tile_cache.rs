//! Disk-based LRU cache for the proxied OSM raster tiles (see
//! `src/tiles.rs`). Each tile lives at `{dir}/{z}/{x}/{y}.png` with its file
//! mtime recording when it was fetched, so the cache survives restarts: the
//! constructor rescans the directory and rebuilds the in-memory index,
//! seeding recency from mtime. Total size is capped; inserting past the cap
//! evicts least-recently-used tiles from disk.

use std::collections::{BTreeMap, HashMap};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// How long a cached tile counts as fresh before the next request for it
/// re-fetches from the tile server (matching the OSM usage policy's ask
/// that proxies cache aggressively).
pub const TILE_TTL: Duration = Duration::from_secs(30 * 86_400);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct TileKey {
    z: u32,
    x: u32,
    y: u32,
}

struct Entry {
    seq: u64,
    bytes: u64,
}

/// Access-ordered index of what's on disk. `by_seq` maps a monotonically
/// increasing access sequence to the tile touched at that point, so its
/// first entry is always the least recently used tile.
#[derive(Default)]
struct LruIndex {
    by_seq: BTreeMap<u64, TileKey>,
    entries: HashMap<TileKey, Entry>,
    next_seq: u64,
    total_bytes: u64,
}

impl LruIndex {
    /// Marks `key` as most recently used, recording `bytes` as its size
    /// (replacing any previous entry for the same tile).
    fn touch(&mut self, key: TileKey, bytes: u64) {
        if let Some(old) = self.entries.remove(&key) {
            self.by_seq.remove(&old.seq);
            self.total_bytes -= old.bytes;
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        self.by_seq.insert(seq, key);
        self.entries.insert(key, Entry { seq, bytes });
        self.total_bytes += bytes;
    }

    fn remove(&mut self, key: &TileKey) {
        if let Some(old) = self.entries.remove(key) {
            self.by_seq.remove(&old.seq);
            self.total_bytes -= old.bytes;
        }
    }

    /// Pops least-recently-used tiles until the total fits in `max_bytes`,
    /// always keeping at least the most recent entry (so one oversized
    /// tile can't evict itself into a permanently empty cache).
    fn evict_over(&mut self, max_bytes: u64) -> Vec<TileKey> {
        let mut evicted = Vec::new();
        while self.total_bytes > max_bytes && self.entries.len() > 1 {
            let (&seq, &key) = self.by_seq.first_key_value().expect("entries is non-empty");
            self.by_seq.remove(&seq);
            let entry = self.entries.remove(&key).expect("indexes stay in sync");
            self.total_bytes -= entry.bytes;
            evicted.push(key);
        }
        evicted
    }
}

pub struct CachedTile {
    pub body: Vec<u8>,
    /// False once the tile is older than [`TILE_TTL`].
    pub fresh: bool,
}

pub struct TileCache {
    dir: PathBuf,
    max_bytes: u64,
    index: Mutex<LruIndex>,
}

impl TileCache {
    /// Opens (creating if needed) the cache rooted at `dir`, capped at
    /// `max_bytes` of tile data, rescanning any tiles already on disk.
    pub fn open(dir: impl Into<PathBuf>, max_bytes: u64) -> std::io::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;

        // Oldest-mtime first, so pre-existing tiles enter the LRU order as
        // least recently used in roughly the order they were fetched.
        let mut found = scan_tiles(&dir)?;
        found.sort_by_key(|(_, _, mtime)| *mtime);
        let mut index = LruIndex::default();
        for (key, bytes, _) in found {
            index.touch(key, bytes);
        }

        let cache = Self { dir, max_bytes, index: Mutex::new(index) };
        cache.evict_and_delete();
        Ok(cache)
    }

    pub fn tile_path(&self, z: u32, x: u32, y: u32) -> PathBuf {
        self.dir.join(z.to_string()).join(x.to_string()).join(format!("{y}.png"))
    }

    /// The cached tile, if present on disk, marking it most recently used.
    pub async fn get(&self, z: u32, x: u32, y: u32) -> Option<CachedTile> {
        let path = self.tile_path(z, x, y);
        let key = TileKey { z, x, y };
        let (body, mtime) = match read_tile(&path).await {
            Ok(read) => read,
            Err(err) => {
                if err.kind() != ErrorKind::NotFound {
                    tracing::warn!(error = %err, z, x, y, "failed to read cached tile");
                }
                self.lock_index().remove(&key);
                return None;
            }
        };
        // Tiles seeded on disk behind the cache's back (or racing a
        // concurrent insert) still get indexed here.
        self.lock_index().touch(key, body.len() as u64);
        let fresh = mtime.elapsed().is_ok_and(|age| age < TILE_TTL);
        Some(CachedTile { body, fresh })
    }

    /// Stores a freshly fetched tile (replacing any expired copy), then
    /// evicts least-recently-used tiles if the cache is over its size cap.
    pub async fn insert(&self, z: u32, x: u32, y: u32, body: &[u8]) -> std::io::Result<()> {
        let path = self.tile_path(z, x, y);
        let parent = path.parent().expect("tile path always has a parent");
        tokio::fs::create_dir_all(parent).await?;

        // Unique temp name + rename: readers never see a half-written tile,
        // and concurrent inserts of the same tile don't clobber each
        // other's temp files.
        let tmp = parent.join(format!(".{y}.{}.tmp", uuid::Uuid::new_v4().simple()));
        tokio::fs::write(&tmp, body).await?;
        if let Err(err) = tokio::fs::rename(&tmp, &path).await {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(err);
        }

        self.lock_index().touch(TileKey { z, x, y }, body.len() as u64);
        self.evict_and_delete();
        Ok(())
    }

    /// Never hold this across an `.await`.
    fn lock_index(&self) -> std::sync::MutexGuard<'_, LruIndex> {
        self.index.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Drops least-recently-used tiles from the index and disk until the
    /// cache fits its byte budget.
    fn evict_and_delete(&self) {
        let evicted = self.lock_index().evict_over(self.max_bytes);
        for key in evicted {
            let path = self.tile_path(key.z, key.x, key.y);
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    tracing::debug!(z = key.z, x = key.x, y = key.y, "evicted cached tile")
                }
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => {
                    tracing::warn!(error = %err, path = %path.display(), "failed to evict tile")
                }
            }
        }
    }
}

async fn read_tile(path: &Path) -> std::io::Result<(Vec<u8>, SystemTime)> {
    let mtime = tokio::fs::metadata(path).await?.modified()?;
    Ok((tokio::fs::read(path).await?, mtime))
}

/// All `{z}/{x}/{y}.png` files under `dir` with their size and mtime.
/// Anything else (temp files, stray dirs) is ignored.
fn scan_tiles(dir: &Path) -> std::io::Result<Vec<(TileKey, u64, SystemTime)>> {
    fn numeric_dirs(dir: &Path) -> std::io::Result<Vec<(u32, PathBuf)>> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            if let Some(n) = entry.file_name().to_str().and_then(|s| s.parse().ok()) {
                if entry.file_type()?.is_dir() {
                    out.push((n, entry.path()));
                }
            }
        }
        Ok(out)
    }

    let mut tiles = Vec::new();
    for (z, z_dir) in numeric_dirs(dir)? {
        for (x, x_dir) in numeric_dirs(&z_dir)? {
            for entry in std::fs::read_dir(&x_dir)? {
                let entry = entry?;
                let name = entry.file_name();
                let Some(y) = name
                    .to_str()
                    .and_then(|s| s.strip_suffix(".png"))
                    .and_then(|s| s.parse().ok())
                else {
                    continue;
                };
                let meta = entry.metadata()?;
                if meta.is_file() {
                    tiles.push((TileKey { z, x, y }, meta.len(), meta.modified()?));
                }
            }
        }
    }
    Ok(tiles)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_cache_dir() -> PathBuf {
        std::env::temp_dir().join(format!("sb-tile-cache-test-{}", uuid::Uuid::new_v4().simple()))
    }

    #[tokio::test]
    async fn get_returns_inserted_tile_as_fresh() {
        let cache = TileCache::open(temp_cache_dir(), 1024).unwrap();
        cache.insert(3, 1, 2, b"png bytes").await.unwrap();

        let tile = cache.get(3, 1, 2).await.expect("tile should be cached");
        assert_eq!(tile.body, b"png bytes");
        assert!(tile.fresh);
        assert!(cache.get(3, 1, 3).await.is_none(), "different tile is a miss");
    }

    #[tokio::test]
    async fn eviction_drops_least_recently_used_first() {
        // Budget fits two 4-byte tiles; a third insert must evict one.
        let cache = TileCache::open(temp_cache_dir(), 8).unwrap();
        cache.insert(1, 0, 0, b"aaaa").await.unwrap();
        cache.insert(1, 0, 1, b"bbbb").await.unwrap();
        // Touch the older tile so the newer one becomes the LRU victim.
        cache.get(1, 0, 0).await.expect("still cached");

        cache.insert(1, 1, 0, b"cccc").await.unwrap();

        assert!(cache.get(1, 0, 1).await.is_none(), "LRU tile evicted");
        assert!(!cache.tile_path(1, 0, 1).exists(), "evicted file removed from disk");
        assert!(cache.get(1, 0, 0).await.is_some(), "recently used tile kept");
        assert!(cache.get(1, 1, 0).await.is_some(), "new tile kept");
    }

    #[tokio::test]
    async fn reinserting_a_tile_replaces_it_without_double_counting() {
        let cache = TileCache::open(temp_cache_dir(), 8).unwrap();
        cache.insert(1, 0, 0, b"aaaa").await.unwrap();
        cache.insert(1, 0, 0, b"AAAA").await.unwrap();
        cache.insert(1, 0, 1, b"bbbb").await.unwrap();

        // Both fit: the re-insert must not have counted 0/0 twice.
        assert_eq!(cache.get(1, 0, 0).await.expect("kept").body, b"AAAA");
        assert!(cache.get(1, 0, 1).await.is_some(), "kept");
    }

    #[tokio::test]
    async fn open_rebuilds_index_from_existing_files_oldest_first() {
        let dir = temp_cache_dir();
        {
            let cache = TileCache::open(&dir, 1024).unwrap();
            cache.insert(1, 0, 0, b"older").await.unwrap();
            cache.insert(1, 0, 1, b"newer").await.unwrap();
        }
        // Age the first tile so the rescan sees distinct mtimes.
        let f = std::fs::File::options()
            .write(true)
            .open(dir.join("1").join("0").join("0.png"))
            .unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(3600)).unwrap();

        // Reopen over the same dir with a budget of one 5-byte tile: the
        // older tile must be the one evicted at startup.
        let cache = TileCache::open(&dir, 5).unwrap();
        assert!(cache.get(1, 0, 0).await.is_none(), "older tile evicted on reopen");
        assert_eq!(cache.get(1, 0, 1).await.expect("kept").body, b"newer");
    }

    #[tokio::test]
    async fn tile_older_than_ttl_reads_as_stale() {
        let cache = TileCache::open(temp_cache_dir(), 1024).unwrap();
        cache.insert(3, 1, 2, b"old png").await.unwrap();
        let f = std::fs::File::options().write(true).open(cache.tile_path(3, 1, 2)).unwrap();
        f.set_modified(SystemTime::now() - (TILE_TTL + Duration::from_secs(60))).unwrap();

        let tile = cache.get(3, 1, 2).await.expect("stale tiles stay readable");
        assert!(!tile.fresh);
    }
}
