use crate::config::Config;
use crate::error::PutError;
use crate::index::cache::IndexCache;
use crate::index::segment_index::SegmentIndex;
use crate::segment::lifecycle;
use crate::storage::db::Database;
use crate::storage::queries;

pub struct SegmentManager {
    pub conversation_id: String,
    pub regular_open: Option<i64>,
    pub persist_open: Option<i64>,
    segment_ids: Vec<i64>,
}

impl SegmentManager {
    pub fn new(conversation_id: String) -> Self {
        Self {
            conversation_id,
            regular_open: None,
            persist_open: None,
            segment_ids: vec![],
        }
    }

    pub fn load(
        db: &Database,
        conversation_id: String,
    ) -> Result<Self, String> {
        let segment_ids = queries::get_conversation_segments(&db.conn, &conversation_id)
            .map_err(|e| e.to_string())?;

        let regular_open = queries::get_open_segment(&db.conn, &conversation_id, false)
            .map_err(|e| e.to_string())?;
        let persist_open = queries::get_open_segment(&db.conn, &conversation_id, true)
            .map_err(|e| e.to_string())?;

        Ok(Self {
            conversation_id,
            regular_open,
            persist_open,
            segment_ids,
        })
    }

    pub fn refresh(&mut self, db: &Database) -> Result<(), String> {
        self.segment_ids = queries::get_conversation_segments(&db.conn, &self.conversation_id)
            .map_err(|e| e.to_string())?;
        self.regular_open = queries::get_open_segment(&db.conn, &self.conversation_id, false)
            .map_err(|e| e.to_string())?;
        self.persist_open = queries::get_open_segment(&db.conn, &self.conversation_id, true)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_or_create_open_segment(
        &mut self,
        db: &Database,
        persist: bool,
        config: &Config,
        cache: &mut IndexCache,
        model_id: &str,
        dimensions: usize,
    ) -> Result<i64, PutError> {
        let open_ref = if persist {
            &mut self.persist_open
        } else {
            &mut self.regular_open
        };

        if let Some(seg_id) = *open_ref {
            // Check staleness
            let was_closed = lifecycle::check_staleness_and_close(db, seg_id, config, cache)
                .map_err(|e| PutError::StorageWrite(std::io::Error::other(e)))?;

            if was_closed {
                // Run eviction after close if needed
                if !persist {
                    lifecycle::run_eviction(db, &self.conversation_id, config, cache)
                        .map_err(|e| PutError::StorageWrite(std::io::Error::other(e)))?;
                }
                *open_ref = None;
            } else {
                return Ok(seg_id);
            }
        }

        // Check persist cap
        if persist {
            let count = queries::get_closed_persist_segment_count(&db.conn, &self.conversation_id)
                .map_err(|e| PutError::StorageWrite(std::io::Error::other(e)))?;
            if count >= config.max_persist_segments {
                return Err(PutError::PersistCapacityExceeded {
                    current: count,
                    max: config.max_persist_segments,
                });
            }
        }

        // Create new open segment
        let relevancy = if persist { 1.0 } else { config.base_score };
        let seg_id = queries::insert_segment(
            &db.conn,
            &self.conversation_id,
            persist,
            relevancy,
            model_id,
        )
        .map_err(|e| PutError::StorageWrite(std::io::Error::other(e)))?;

        // Create usearch index
        let index = SegmentIndex::create(&db.index_path(seg_id), config.segment_capacity, dimensions)
            .map_err(|e| PutError::StorageWrite(std::io::Error::other(e)))?;
        cache.insert(seg_id, index);

        *open_ref = Some(seg_id);
        self.segment_ids.push(seg_id);

        Ok(seg_id)
    }

    pub fn handle_post_put(
        &mut self,
        db: &Database,
        segment_id: i64,
        node_count: i64,
        persist: bool,
        config: &Config,
        cache: &mut IndexCache,
    ) -> Result<(), PutError> {
        let was_closed = lifecycle::check_capacity_and_close(db, segment_id, node_count, config, cache)
            .map_err(|e| PutError::StorageWrite(std::io::Error::other(e)))?;

        if was_closed {
            let open_ref = if persist {
                &mut self.persist_open
            } else {
                &mut self.regular_open
            };
            *open_ref = None;

            // Run eviction for regular segments
            if !persist {
                lifecycle::run_eviction(db, &self.conversation_id, config, cache)
                    .map_err(|e| PutError::StorageWrite(std::io::Error::other(e)))?;
            }
        }

        Ok(())
    }

    pub fn conversation_segments(&self) -> &[i64] {
        &self.segment_ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> (Database, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open_at(tmp.path().to_path_buf()).unwrap();
        (db, tmp)
    }

    #[test]
    fn lazy_segment_creation() {
        let (db, _tmp) = test_db();
        let config = Config::default();
        let mut cache = IndexCache::new(10);

        queries::insert_conversation(&db.conn, "conv-1").unwrap();
        let mut mgr = SegmentManager::new("conv-1".to_string());

        assert!(mgr.regular_open.is_none());
        assert!(mgr.persist_open.is_none());

        // Get regular segment — creates lazily
        let seg1 = mgr
            .get_or_create_open_segment(&db, false, &config, &mut cache, "m", 4)
            .unwrap();
        assert_eq!(mgr.regular_open, Some(seg1));
        assert!(mgr.persist_open.is_none());

        // Get again — returns same
        let seg1b = mgr
            .get_or_create_open_segment(&db, false, &config, &mut cache, "m", 4)
            .unwrap();
        assert_eq!(seg1, seg1b);

        // Get persist — creates separate
        let seg2 = mgr
            .get_or_create_open_segment(&db, true, &config, &mut cache, "m", 4)
            .unwrap();
        assert_ne!(seg1, seg2);
        assert_eq!(mgr.persist_open, Some(seg2));
    }

    #[test]
    fn capacity_close_creates_new_on_next_put() {
        let (db, _tmp) = test_db();
        let mut config = Config::default();
        config.segment_capacity = 2;
        let mut cache = IndexCache::new(10);

        queries::insert_conversation(&db.conn, "conv-1").unwrap();
        let mut mgr = SegmentManager::new("conv-1".to_string());

        let seg1 = mgr
            .get_or_create_open_segment(&db, false, &config, &mut cache, "m", 4)
            .unwrap();

        // Simulate two puts filling the segment
        for i in 0..2 {
            let idx = cache.get_or_load(seg1, &db.index_path(seg1)).unwrap();
            idx.add(i as u64, &[1.0, 0.0, 0.0, 0.0]).unwrap();
            let count = queries::increment_segment_node_count(&db.conn, seg1).unwrap();
            mgr.handle_post_put(&db, seg1, count, false, &config, &mut cache)
                .unwrap();
        }

        // Segment should be closed now
        let info = queries::get_segment_info(&db.conn, seg1).unwrap();
        assert_eq!(info.state, "closed");
        assert!(mgr.regular_open.is_none());

        // Next get_or_create should create a new one
        let seg2 = mgr
            .get_or_create_open_segment(&db, false, &config, &mut cache, "m", 4)
            .unwrap();
        assert_ne!(seg1, seg2);
    }

    #[test]
    fn persist_capacity_enforcement() {
        let (db, _tmp) = test_db();
        let mut config = Config::default();
        config.max_persist_segments = 1;
        config.segment_capacity = 1;
        let mut cache = IndexCache::new(10);

        queries::insert_conversation(&db.conn, "conv-1").unwrap();
        let mut mgr = SegmentManager::new("conv-1".to_string());

        // Create and fill first persist segment
        let seg1 = mgr
            .get_or_create_open_segment(&db, true, &config, &mut cache, "m", 4)
            .unwrap();
        let idx = cache.get_or_load(seg1, &db.index_path(seg1)).unwrap();
        idx.add(1, &[1.0, 0.0, 0.0, 0.0]).unwrap();
        let count = queries::increment_segment_node_count(&db.conn, seg1).unwrap();
        mgr.handle_post_put(&db, seg1, count, true, &config, &mut cache)
            .unwrap();

        // Try to create another persist segment — should fail
        let result = mgr.get_or_create_open_segment(&db, true, &config, &mut cache, "m", 4);
        assert!(matches!(result, Err(PutError::PersistCapacityExceeded { .. })));
    }

    #[test]
    fn load_existing_segments() {
        let (db, _tmp) = test_db();

        queries::insert_conversation(&db.conn, "conv-1").unwrap();
        let seg1 = queries::insert_segment(&db.conn, "conv-1", false, 0.3, "m").unwrap();
        let seg2 = queries::insert_segment(&db.conn, "conv-1", true, 1.0, "m").unwrap();

        let mgr = SegmentManager::load(&db, "conv-1".to_string()).unwrap();
        assert_eq!(mgr.regular_open, Some(seg1));
        assert_eq!(mgr.persist_open, Some(seg2));
        assert_eq!(mgr.conversation_segments().len(), 2);
    }
}
