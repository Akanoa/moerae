use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::Config;
use crate::index::cache::IndexCache;
use crate::index::segment_index::SegmentIndex;
use crate::storage::db::Database;
use crate::storage::queries;

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub fn check_staleness_and_close(
    db: &Database,
    segment_id: i64,
    config: &Config,
    cache: &mut IndexCache,
) -> Result<bool, String> {
    let (last_write, node_count) = queries::get_segment_staleness(&db.conn, segment_id)
        .map_err(|e| e.to_string())?;

    let elapsed = now_unix() - last_write;
    if elapsed < config.segment_staleness as i64 {
        return Ok(false); // Not stale
    }

    if node_count == 0 {
        // Stale but empty — update timestamp, keep open
        queries::update_segment_last_write(&db.conn, segment_id)
            .map_err(|e| e.to_string())?;
        return Ok(false);
    }

    // Stale with content — close it
    close_segment(db, segment_id, config, cache)?;
    Ok(true)
}

pub fn close_segment(
    db: &Database,
    segment_id: i64,
    config: &Config,
    cache: &mut IndexCache,
) -> Result<(), String> {
    // Flush usearch index to disk
    if cache.contains(segment_id) {
        // The open segment's index is in the cache — save it
        let idx = cache.get_or_load(segment_id, &db.index_path(segment_id))?;
        idx.save()?;
    }

    // Mark as closed + check promotion in SQL
    queries::close_segment(&db.conn, segment_id, config.promotion_threshold)
        .map_err(|e| e.to_string())?;

    // Invalidate from cache (will be re-opened as read-only view when needed)
    cache.invalidate(segment_id);

    Ok(())
}

pub fn check_capacity_and_close(
    db: &Database,
    segment_id: i64,
    current_node_count: i64,
    config: &Config,
    cache: &mut IndexCache,
) -> Result<bool, String> {
    if current_node_count >= config.segment_capacity as i64 {
        close_segment(db, segment_id, config, cache)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn run_eviction(
    db: &Database,
    conversation_id: &str,
    config: &Config,
    cache: &mut IndexCache,
) -> Result<usize, String> {
    let candidates = queries::get_eviction_candidates(
        &db.conn,
        conversation_id,
        config.max_segments,
    )
    .map_err(|e| e.to_string())?;

    let evicted = candidates.len();

    for segment_id in &candidates {
        evict_segment(db, *segment_id, cache)?;
    }

    Ok(evicted)
}

pub fn evict_segment(
    db: &Database,
    segment_id: i64,
    cache: &mut IndexCache,
) -> Result<(), String> {
    // 1. Invalidate cache
    cache.invalidate(segment_id);

    // 2. Delete usearch file
    let index_path = db.index_path(segment_id);
    SegmentIndex::remove_file(&index_path).map_err(|e| e.to_string())?;

    // 3. Delete SQL row (nodes cascade)
    queries::delete_segment(&db.conn, segment_id).map_err(|e| e.to_string())?;

    Ok(())
}

pub fn close_orphan_segments(
    db: &Database,
    conversation_id: &str,
    _cache: &mut IndexCache,
    config: &Config,
) -> Result<usize, String> {
    let orphans = queries::get_orphan_open_segments(&db.conn, conversation_id)
        .map_err(|e| e.to_string())?;

    let count = orphans.len();

    for segment_id in orphans {
        // Close the segment in SQL
        queries::close_segment(&db.conn, segment_id, config.promotion_threshold)
            .map_err(|e| e.to_string())?;

        // Validate usearch index completeness
        let index_path = db.index_path(segment_id);
        let info = queries::get_segment_info(&db.conn, segment_id)
            .map_err(|e| e.to_string())?;

        if index_path.exists() {
            // Check if index is complete
            match SegmentIndex::load(&index_path) {
                Ok(idx) => {
                    if idx.len() != info.node_count as usize {
                        // Incomplete — rebuild
                        drop(idx);
                        rebuild_segment_index(db, segment_id)?;
                    } else {
                        // Complete — save and drop (will be re-opened as view when needed)
                        idx.save()?;
                    }
                }
                Err(_) => {
                    // Corrupted — rebuild
                    rebuild_segment_index(db, segment_id)?;
                }
            }
        } else if info.node_count > 0 {
            // Missing — rebuild
            rebuild_segment_index(db, segment_id)?;
        }
    }

    Ok(count)
}

pub fn rebuild_segment_index(db: &Database, segment_id: i64) -> Result<(), String> {
    let embeddings = queries::get_segment_embeddings(&db.conn, segment_id)
        .map_err(|e| e.to_string())?;

    if embeddings.is_empty() {
        return Ok(());
    }

    // Determine dimensions from first embedding
    let dims = embeddings[0].1.len() / 4; // f32 = 4 bytes

    let tmp_path = db.index_tmp_path(segment_id);
    let final_path = db.index_path(segment_id);

    let index = SegmentIndex::create(&tmp_path, embeddings.len(), dims)
        .map_err(|e| format!("failed to create index: {e}"))?;

    for (node_id, embedding_bytes) in &embeddings {
        let vector: Vec<f32> = embedding_bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        index
            .add(*node_id as u64, &vector)
            .map_err(|e| format!("failed to add vector: {e}"))?;
    }

    index.save_to(&tmp_path)?;
    drop(index);

    std::fs::rename(&tmp_path, &final_path)
        .map_err(|e| format!("failed to rename index: {e}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::model::EmbeddingModel;

    fn test_db() -> (Database, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open_at(tmp.path().to_path_buf()).unwrap();
        (db, tmp)
    }

    fn default_config() -> Config {
        Config::default()
    }

    #[test]
    fn close_segment_sets_state_and_invalidates_cache() {
        let (db, _tmp) = test_db();
        let config = default_config();
        let mut cache = IndexCache::new(10);

        queries::insert_conversation(&db.conn, "conv-1").unwrap();
        let seg = queries::insert_segment(&db.conn, "conv-1", false, 0.3, "m").unwrap();

        // Create an index for the segment
        let index = SegmentIndex::create(&db.index_path(seg), 100, 4).unwrap();
        index.add(1, &[1.0, 0.0, 0.0, 0.0]).unwrap();
        cache.insert(seg, index);

        assert!(cache.contains(seg));

        close_segment(&db, seg, &config, &mut cache).unwrap();

        let info = queries::get_segment_info(&db.conn, seg).unwrap();
        assert_eq!(info.state, "closed");
        assert!(!cache.contains(seg)); // invalidated
    }

    #[test]
    fn capacity_close_triggers_at_threshold() {
        let (db, _tmp) = test_db();
        let mut config = default_config();
        config.segment_capacity = 3;
        let mut cache = IndexCache::new(10);

        queries::insert_conversation(&db.conn, "conv-1").unwrap();
        let seg = queries::insert_segment(&db.conn, "conv-1", false, 0.3, "m").unwrap();

        // Create index
        let index = SegmentIndex::create(&db.index_path(seg), 10, 4).unwrap();
        cache.insert(seg, index);

        // Below capacity
        let closed = check_capacity_and_close(&db, seg, 2, &config, &mut cache).unwrap();
        assert!(!closed);

        // At capacity
        let closed = check_capacity_and_close(&db, seg, 3, &config, &mut cache).unwrap();
        assert!(closed);
    }

    #[test]
    fn eviction_removes_lowest_priority() {
        let (db, _tmp) = test_db();
        let mut config = default_config();
        config.max_segments = 1;
        let mut cache = IndexCache::new(10);

        queries::insert_conversation(&db.conn, "conv-1").unwrap();

        // Create two closed segments with different scores
        let seg1 = queries::insert_segment(&db.conn, "conv-1", false, 0.1, "m").unwrap();
        queries::increment_segment_node_count(&db.conn, seg1).unwrap();
        queries::close_segment(&db.conn, seg1, 0.7).unwrap();

        // Create index file for seg1
        let idx1 = SegmentIndex::create(&db.index_path(seg1), 10, 4).unwrap();
        idx1.add(1, &[1.0, 0.0, 0.0, 0.0]).unwrap();
        idx1.save().unwrap();

        let seg2 = queries::insert_segment(&db.conn, "conv-1", false, 0.9, "m").unwrap();
        queries::increment_segment_node_count(&db.conn, seg2).unwrap();
        queries::close_segment(&db.conn, seg2, 0.7).unwrap();

        let idx2 = SegmentIndex::create(&db.index_path(seg2), 10, 4).unwrap();
        idx2.add(2, &[0.0, 1.0, 0.0, 0.0]).unwrap();
        idx2.save().unwrap();

        let evicted = run_eviction(&db, "conv-1", &config, &mut cache).unwrap();
        assert_eq!(evicted, 1);

        // seg1 (low score) should be evicted, seg2 should remain
        assert!(queries::get_segment_info(&db.conn, seg1).is_err()); // deleted
        assert!(queries::get_segment_info(&db.conn, seg2).is_ok()); // still exists
        assert!(!db.index_path(seg1).exists()); // file deleted
    }

    #[test]
    fn persist_segments_not_evicted() {
        let (db, _tmp) = test_db();
        let mut config = default_config();
        config.max_segments = 0; // aggressive eviction
        let mut cache = IndexCache::new(10);

        queries::insert_conversation(&db.conn, "conv-1").unwrap();

        let seg = queries::insert_segment(&db.conn, "conv-1", true, 1.0, "m").unwrap();
        queries::increment_segment_node_count(&db.conn, seg).unwrap();
        queries::close_segment(&db.conn, seg, 0.7).unwrap();

        let evicted = run_eviction(&db, "conv-1", &config, &mut cache).unwrap();
        assert_eq!(evicted, 0); // persist segments excluded
        assert!(queries::get_segment_info(&db.conn, seg).is_ok());
    }

    #[test]
    fn staleness_closes_nonempty_segment() {
        let (db, _tmp) = test_db();
        let mut config = default_config();
        config.segment_staleness = 1; // 1 second
        let mut cache = IndexCache::new(10);

        queries::insert_conversation(&db.conn, "conv-1").unwrap();
        let seg = queries::insert_segment(&db.conn, "conv-1", false, 0.3, "m").unwrap();
        queries::increment_segment_node_count(&db.conn, seg).unwrap();

        let index = SegmentIndex::create(&db.index_path(seg), 10, 4).unwrap();
        index.add(1, &[1.0, 0.0, 0.0, 0.0]).unwrap();
        cache.insert(seg, index);

        // Force the last_write to be old
        db.conn
            .execute(
                "UPDATE segments SET last_write_timestamp = ?1 WHERE segment_id = ?2",
                rusqlite::params![now_unix() - 10, seg],
            )
            .unwrap();

        let closed = check_staleness_and_close(&db, seg, &config, &mut cache).unwrap();
        assert!(closed);

        let info = queries::get_segment_info(&db.conn, seg).unwrap();
        assert_eq!(info.state, "closed");
    }

    #[test]
    fn staleness_keeps_empty_segment_open() {
        let (db, _tmp) = test_db();
        let mut config = default_config();
        config.segment_staleness = 1;
        let mut cache = IndexCache::new(10);

        queries::insert_conversation(&db.conn, "conv-1").unwrap();
        let seg = queries::insert_segment(&db.conn, "conv-1", false, 0.3, "m").unwrap();

        // Force old timestamp
        db.conn
            .execute(
                "UPDATE segments SET last_write_timestamp = ?1 WHERE segment_id = ?2",
                rusqlite::params![now_unix() - 10, seg],
            )
            .unwrap();

        let closed = check_staleness_and_close(&db, seg, &config, &mut cache).unwrap();
        assert!(!closed);

        let info = queries::get_segment_info(&db.conn, seg).unwrap();
        assert_eq!(info.state, "open"); // still open
    }

    #[test]
    fn promotion_on_close_by_threshold() {
        let (db, _tmp) = test_db();
        let config = default_config(); // promotion_threshold = 0.7
        let mut cache = IndexCache::new(10);

        queries::insert_conversation(&db.conn, "conv-1").unwrap();

        // Below threshold
        let seg1 = queries::insert_segment(&db.conn, "conv-1", false, 0.5, "m").unwrap();
        let idx1 = SegmentIndex::create(&db.index_path(seg1), 10, 4).unwrap();
        cache.insert(seg1, idx1);
        close_segment(&db, seg1, &config, &mut cache).unwrap();
        assert!(!queries::get_segment_info(&db.conn, seg1).unwrap().promoted);

        // Above threshold
        let seg2 = queries::insert_segment(&db.conn, "conv-1", false, 0.8, "m").unwrap();
        let idx2 = SegmentIndex::create(&db.index_path(seg2), 10, 4).unwrap();
        cache.insert(seg2, idx2);
        close_segment(&db, seg2, &config, &mut cache).unwrap();
        assert!(queries::get_segment_info(&db.conn, seg2).unwrap().promoted);
    }

    #[test]
    fn promotion_on_close_by_persist() {
        let (db, _tmp) = test_db();
        let config = default_config();
        let mut cache = IndexCache::new(10);

        queries::insert_conversation(&db.conn, "conv-1").unwrap();

        // Persist with low score still gets promoted
        let seg = queries::insert_segment(&db.conn, "conv-1", true, 0.3, "m").unwrap();
        let idx = SegmentIndex::create(&db.index_path(seg), 10, 4).unwrap();
        cache.insert(seg, idx);
        close_segment(&db, seg, &config, &mut cache).unwrap();
        assert!(queries::get_segment_info(&db.conn, seg).unwrap().promoted);
    }

    #[test]
    fn rebuild_segment_index_from_embeddings() {
        let (db, _tmp) = test_db();

        queries::insert_conversation(&db.conn, "conv-1").unwrap();
        let seg = queries::insert_segment(&db.conn, "conv-1", false, 0.3, "m").unwrap();

        // Insert nodes with embeddings
        let v1: Vec<f32> = vec![1.0, 0.0, 0.0, 0.0];
        let v2: Vec<f32> = vec![0.0, 1.0, 0.0, 0.0];
        let hash1 = queries::compute_content_hash("hello", None);
        let hash2 = queries::compute_content_hash("world", None);
        let emb1 = EmbeddingModel::embedding_to_bytes(&v1);
        let emb2 = EmbeddingModel::embedding_to_bytes(&v2);

        let n1 = queries::insert_node(&db.conn, seg, "hello", None, &emb1, &hash1).unwrap();
        let _n2 = queries::insert_node(&db.conn, seg, "world", None, &emb2, &hash2).unwrap();

        // Rebuild
        rebuild_segment_index(&db, seg).unwrap();

        // Verify the rebuilt index works
        let index = SegmentIndex::view(&db.index_path(seg)).unwrap();
        assert_eq!(index.len(), 2);

        let results = index.search(&v1, 2);
        assert_eq!(results[0].0, n1 as u64); // closest to v1
    }

    #[test]
    fn orphan_close_and_rebuild() {
        let (db, _tmp) = test_db();
        let config = default_config();
        let mut cache = IndexCache::new(10);

        queries::insert_conversation(&db.conn, "conv-1").unwrap();
        let seg = queries::insert_segment(&db.conn, "conv-1", false, 0.3, "m").unwrap();

        // Insert a node but don't create a usearch file (simulating crash)
        let v1: Vec<f32> = vec![1.0, 0.0, 0.0, 0.0];
        let hash = queries::compute_content_hash("data", None);
        let emb = EmbeddingModel::embedding_to_bytes(&v1);
        queries::insert_node(&db.conn, seg, "data", None, &emb, &hash).unwrap();
        queries::increment_segment_node_count(&db.conn, seg).unwrap();

        // Orphan cleanup should close and rebuild
        let closed = close_orphan_segments(&db, "conv-1", &mut cache, &config).unwrap();
        assert_eq!(closed, 1);

        let info = queries::get_segment_info(&db.conn, seg).unwrap();
        assert_eq!(info.state, "closed");

        // Index should have been rebuilt
        assert!(db.index_path(seg).exists());
        let index = SegmentIndex::view(&db.index_path(seg)).unwrap();
        assert_eq!(index.len(), 1);
    }
}
