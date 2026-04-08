use std::num::NonZero;
use std::path::Path;

use lru::LruCache;

use super::segment_index::SegmentIndex;

pub struct IndexCache {
    cache: LruCache<i64, SegmentIndex>,
}

impl IndexCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: LruCache::new(
                NonZero::new(capacity).expect("cache capacity must be > 0"),
            ),
        }
    }

    pub fn get_or_load(
        &mut self,
        segment_id: i64,
        path: &Path,
    ) -> Result<&SegmentIndex, String> {
        if !self.cache.contains(&segment_id) {
            let index = SegmentIndex::view(path)?;
            self.cache.put(segment_id, index);
        }
        Ok(self.cache.get(&segment_id).unwrap())
    }

    pub fn insert(&mut self, segment_id: i64, index: SegmentIndex) {
        self.cache.put(segment_id, index);
    }

    pub fn invalidate(&mut self, segment_id: i64) {
        self.cache.pop(&segment_id);
    }

    pub fn contains(&self, segment_id: i64) -> bool {
        self.cache.contains(&segment_id)
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_index(dir: &Path, id: i64) -> (SegmentIndex, std::path::PathBuf) {
        let path = dir.join(format!("{id}.usearch"));
        let index = SegmentIndex::create(&path, 10, 4).unwrap();
        index.add(id as u64, &[1.0, 0.0, 0.0, 0.0]).unwrap();
        index.save().unwrap();
        (index, path)
    }

    #[test]
    fn basic_cache_operations() {
        let tmp = tempfile::tempdir().unwrap();
        let mut cache = IndexCache::new(3);

        assert!(cache.is_empty());

        let (_, path1) = make_index(tmp.path(), 1);
        let (_, path2) = make_index(tmp.path(), 2);

        cache.get_or_load(1, &path1).unwrap();
        assert_eq!(cache.len(), 1);
        assert!(cache.contains(1));

        cache.get_or_load(2, &path2).unwrap();
        assert_eq!(cache.len(), 2);

        // Re-load same ID doesn't add another entry
        cache.get_or_load(1, &path1).unwrap();
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn lru_eviction() {
        let tmp = tempfile::tempdir().unwrap();
        let mut cache = IndexCache::new(2);

        let (_, path1) = make_index(tmp.path(), 1);
        let (_, path2) = make_index(tmp.path(), 2);
        let (_, path3) = make_index(tmp.path(), 3);

        cache.get_or_load(1, &path1).unwrap();
        cache.get_or_load(2, &path2).unwrap();
        assert_eq!(cache.len(), 2);

        // Adding 3 should evict 1 (LRU)
        cache.get_or_load(3, &path3).unwrap();
        assert_eq!(cache.len(), 2);
        assert!(!cache.contains(1));
        assert!(cache.contains(2));
        assert!(cache.contains(3));
    }

    #[test]
    fn invalidation() {
        let tmp = tempfile::tempdir().unwrap();
        let mut cache = IndexCache::new(5);

        let (_, path1) = make_index(tmp.path(), 1);
        cache.get_or_load(1, &path1).unwrap();
        assert!(cache.contains(1));

        cache.invalidate(1);
        assert!(!cache.contains(1));
        assert!(cache.is_empty());

        // Invalidating non-existent is fine
        cache.invalidate(999);
    }

    #[test]
    fn search_through_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let mut cache = IndexCache::new(5);

        let (_, path1) = make_index(tmp.path(), 1);
        let idx = cache.get_or_load(1, &path1).unwrap();

        let results = idx.search(&[1.0, 0.0, 0.0, 0.0], 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 1);
    }
}
