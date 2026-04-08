use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{ConversationInfo, NodeContent, ProjectStats};

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

// --- Conversations ---

pub fn insert_conversation(conn: &Connection, uuid: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO conversations (uuid, created_at) VALUES (?1, ?2)",
        params![uuid, now_unix()],
    )?;
    Ok(())
}

pub fn conversation_exists(conn: &Connection, uuid: &str) -> rusqlite::Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conversations WHERE uuid = ?1",
        [uuid],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn delete_conversation(conn: &Connection, uuid: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM conversations WHERE uuid = ?1", [uuid])?;
    Ok(())
}

pub fn list_conversations(conn: &Connection) -> rusqlite::Result<Vec<ConversationInfo>> {
    let mut stmt = conn.prepare(
        "SELECT c.uuid, c.created_at,
                COALESCE(s.seg_count, 0),
                COALESCE(s.node_count, 0)
         FROM conversations c
         LEFT JOIN (
             SELECT conversation_id,
                    COUNT(DISTINCT segment_id) AS seg_count,
                    SUM(node_count) AS node_count
             FROM segments
             GROUP BY conversation_id
         ) s ON s.conversation_id = c.uuid
         ORDER BY c.created_at DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(ConversationInfo {
            uuid: row.get(0)?,
            created_at: row.get(1)?,
            segment_count: row.get::<_, i64>(2)? as usize,
            node_count: row.get::<_, i64>(3)? as usize,
        })
    })?;

    rows.collect()
}

// --- Segments ---

pub fn insert_segment(
    conn: &Connection,
    conversation_id: &str,
    persist: bool,
    relevancy_score: f32,
    model_id: &str,
) -> rusqlite::Result<i64> {
    let now = now_unix();
    conn.execute(
        "INSERT INTO segments (conversation_id, state, persist, promoted, relevancy_score,
                               hit_count, node_count, created_at, last_write_timestamp, model_id)
         VALUES (?1, 'open', ?2, 0, ?3, 0, 0, ?4, ?4, ?5)",
        params![conversation_id, persist as i32, relevancy_score, now, model_id],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn close_segment(
    conn: &Connection,
    segment_id: i64,
    promotion_threshold: f32,
) -> rusqlite::Result<()> {
    // Close and check promotion in one step
    conn.execute(
        "UPDATE segments SET
            state = 'closed',
            promoted = CASE
                WHEN persist = 1 THEN 1
                WHEN relevancy_score >= ?2 THEN 1
                ELSE 0
            END
         WHERE segment_id = ?1 AND state = 'open'",
        params![segment_id, promotion_threshold],
    )?;
    Ok(())
}

pub fn get_open_segment(
    conn: &Connection,
    conversation_id: &str,
    persist: bool,
) -> rusqlite::Result<Option<i64>> {
    let result = conn.query_row(
        "SELECT segment_id FROM segments
         WHERE conversation_id = ?1 AND state = 'open' AND persist = ?2",
        params![conversation_id, persist as i32],
        |row| row.get(0),
    );

    match result {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn get_segment_info(
    conn: &Connection,
    segment_id: i64,
) -> rusqlite::Result<SegmentInfo> {
    conn.query_row(
        "SELECT segment_id, conversation_id, state, persist, promoted, relevancy_score,
                hit_count, node_count, created_at, last_hit_timestamp, last_write_timestamp, model_id
         FROM segments WHERE segment_id = ?1",
        [segment_id],
        |row| {
            Ok(SegmentInfo {
                segment_id: row.get(0)?,
                conversation_id: row.get(1)?,
                state: row.get(2)?,
                persist: row.get::<_, i32>(3)? != 0,
                promoted: row.get::<_, i32>(4)? != 0,
                relevancy_score: row.get(5)?,
                hit_count: row.get(6)?,
                node_count: row.get(7)?,
                created_at: row.get(8)?,
                last_hit_timestamp: row.get(9)?,
                last_write_timestamp: row.get(10)?,
                model_id: row.get(11)?,
            })
        },
    )
}

#[derive(Debug)]
pub struct SegmentInfo {
    pub segment_id: i64,
    pub conversation_id: String,
    pub state: String,
    pub persist: bool,
    pub promoted: bool,
    pub relevancy_score: f32,
    pub hit_count: i64,
    pub node_count: i64,
    pub created_at: i64,
    pub last_hit_timestamp: Option<i64>,
    pub last_write_timestamp: i64,
    pub model_id: String,
}

pub fn get_conversation_segments(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT segment_id FROM segments WHERE conversation_id = ?1",
    )?;
    let rows = stmt.query_map([conversation_id], |row| row.get(0))?;
    rows.collect()
}

pub fn get_promoted_segments(conn: &Connection) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT segment_id FROM segments WHERE promoted = 1 AND state = 'closed'",
    )?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect()
}

pub fn get_orphan_open_segments(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT segment_id FROM segments
         WHERE conversation_id = ?1 AND state = 'open'",
    )?;
    let rows = stmt.query_map([conversation_id], |row| row.get(0))?;
    rows.collect()
}

pub fn increment_segment_node_count(
    conn: &Connection,
    segment_id: i64,
) -> rusqlite::Result<i64> {
    conn.execute(
        "UPDATE segments SET node_count = node_count + 1, last_write_timestamp = ?2
         WHERE segment_id = ?1",
        params![segment_id, now_unix()],
    )?;
    let count: i64 = conn.query_row(
        "SELECT node_count FROM segments WHERE segment_id = ?1",
        [segment_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn update_segment_hit(
    conn: &Connection,
    segment_id: i64,
    hit_boost: f32,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE segments SET
            relevancy_score = MIN(relevancy_score + ?2, 1.0),
            hit_count = hit_count + 1,
            last_hit_timestamp = ?3
         WHERE segment_id = ?1",
        params![segment_id, hit_boost, now_unix()],
    )?;
    Ok(())
}

pub fn get_segment_staleness(
    conn: &Connection,
    segment_id: i64,
) -> rusqlite::Result<(i64, i64)> {
    // Returns (last_write_timestamp, node_count)
    conn.query_row(
        "SELECT last_write_timestamp, node_count FROM segments WHERE segment_id = ?1",
        [segment_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
}

pub fn update_segment_last_write(conn: &Connection, segment_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE segments SET last_write_timestamp = ?2 WHERE segment_id = ?1",
        params![segment_id, now_unix()],
    )?;
    Ok(())
}

pub fn get_eviction_candidates(
    conn: &Connection,
    conversation_id: &str,
    max_segments: usize,
) -> rusqlite::Result<Vec<i64>> {
    // Count closed non-persist segments
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM segments
         WHERE conversation_id = ?1 AND state = 'closed' AND persist = 0",
        [conversation_id],
        |row| row.get(0),
    )?;

    if count as usize <= max_segments {
        return Ok(vec![]);
    }

    let to_evict = count as usize - max_segments;

    // Order by eviction priority descending (higher = evict first)
    // When last_hit_timestamp is NULL, use created_at
    let mut stmt = conn.prepare(
        "SELECT segment_id,
                (CAST((?3 - COALESCE(last_hit_timestamp, created_at)) AS REAL))
                / ((relevancy_score + 0.01) * (hit_count + 1)) AS priority
         FROM segments
         WHERE conversation_id = ?1 AND state = 'closed' AND persist = 0
         ORDER BY priority DESC
         LIMIT ?2",
    )?;

    let rows = stmt.query_map(
        params![conversation_id, to_evict as i64, now_unix()],
        |row| row.get(0),
    )?;

    rows.collect()
}

pub fn get_conversations_over_segment_limit(
    conn: &Connection,
    max_segments: usize,
) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT conversation_id FROM segments
         WHERE state = 'closed' AND persist = 0
         GROUP BY conversation_id
         HAVING COUNT(*) > ?1",
    )?;
    let rows = stmt.query_map([max_segments as i64], |row| row.get(0))?;
    rows.collect()
}

pub fn delete_segment(conn: &Connection, segment_id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM segments WHERE segment_id = ?1", [segment_id])?;
    Ok(())
}

pub fn get_all_segment_ids(conn: &Connection) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT segment_id FROM segments")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect()
}

pub fn get_mismatched_segments(
    conn: &Connection,
    current_model_id: &str,
) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT segment_id FROM segments WHERE model_id != ?1",
    )?;
    let rows = stmt.query_map([current_model_id], |row| row.get(0))?;
    rows.collect()
}

pub fn get_closed_persist_segment_count(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<usize> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM segments
         WHERE conversation_id = ?1 AND state = 'closed' AND persist = 1",
        [conversation_id],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

// --- Nodes ---

pub fn insert_node(
    conn: &Connection,
    segment_id: i64,
    data: &str,
    metadata: Option<&str>,
    embedding: &[u8],
    content_hash: &[u8],
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO nodes (segment_id, data, metadata, embedding, content_hash)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![segment_id, data, metadata, embedding, content_hash],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_node(conn: &Connection, node_id: i64) -> rusqlite::Result<NodeContent> {
    conn.query_row(
        "SELECT data, metadata FROM nodes WHERE node_id = ?1",
        [node_id],
        |row| {
            Ok(NodeContent {
                data: row.get(0)?,
                metadata: row.get(1)?,
            })
        },
    )
}

pub fn check_duplicate(
    conn: &Connection,
    segment_id: i64,
    content_hash: &[u8],
) -> rusqlite::Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM nodes WHERE content_hash = ?1 AND segment_id = ?2",
        params![content_hash, segment_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn get_segment_embeddings(
    conn: &Connection,
    segment_id: i64,
) -> rusqlite::Result<Vec<(i64, Vec<u8>)>> {
    let mut stmt = conn.prepare(
        "SELECT node_id, embedding FROM nodes WHERE segment_id = ?1",
    )?;
    let rows = stmt.query_map([segment_id], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?;
    rows.collect()
}

pub fn compute_content_hash(data: &str, metadata: Option<&str>) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hasher.update(b"\0");
    hasher.update(metadata.unwrap_or("").as_bytes());
    hasher.finalize().to_vec()
}

// --- Stats ---

pub fn get_project_stats(
    conn: &Connection,
    current_model_id: &str,
) -> rusqlite::Result<ProjectStats> {
    let total_segments: i64 = conn.query_row(
        "SELECT COUNT(*) FROM segments",
        [],
        |row| row.get(0),
    )?;
    let mismatched: i64 = conn.query_row(
        "SELECT COUNT(*) FROM segments WHERE model_id != ?1",
        [current_model_id],
        |row| row.get(0),
    )?;
    let total_nodes: i64 = conn.query_row(
        "SELECT COUNT(*) FROM nodes",
        [],
        |row| row.get(0),
    )?;
    Ok(ProjectStats {
        total_segments: total_segments as usize,
        mismatched_segments: mismatched as usize,
        total_nodes: total_nodes as usize,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db::Database;

    fn test_db() -> Database {
        let tmp = tempfile::tempdir().unwrap();
        Database::open_at(tmp.path().to_path_buf()).unwrap()
    }

    #[test]
    fn conversation_crud() {
        let db = test_db();
        let conn = &db.conn;

        assert!(!conversation_exists(conn, "abc-123").unwrap());
        insert_conversation(conn, "abc-123").unwrap();
        assert!(conversation_exists(conn, "abc-123").unwrap());

        let list = list_conversations(conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].uuid, "abc-123");
        assert_eq!(list[0].segment_count, 0);

        delete_conversation(conn, "abc-123").unwrap();
        assert!(!conversation_exists(conn, "abc-123").unwrap());
    }

    #[test]
    fn segment_lifecycle() {
        let db = test_db();
        let conn = &db.conn;

        insert_conversation(conn, "conv-1").unwrap();
        let seg_id = insert_segment(conn, "conv-1", false, 0.3, "model-v1").unwrap();

        let info = get_segment_info(conn, seg_id).unwrap();
        assert_eq!(info.state, "open");
        assert!(!info.persist);
        assert!(!info.promoted);
        assert_eq!(info.node_count, 0);

        // Increment node count
        let count = increment_segment_node_count(conn, seg_id).unwrap();
        assert_eq!(count, 1);

        // Close with promotion threshold above score
        close_segment(conn, seg_id, 0.7).unwrap();
        let info = get_segment_info(conn, seg_id).unwrap();
        assert_eq!(info.state, "closed");
        assert!(!info.promoted); // 0.3 < 0.7

        // Open another with high relevancy
        let seg2 = insert_segment(conn, "conv-1", false, 0.8, "model-v1").unwrap();
        close_segment(conn, seg2, 0.7).unwrap();
        let info2 = get_segment_info(conn, seg2).unwrap();
        assert!(info2.promoted); // 0.8 >= 0.7
    }

    #[test]
    fn persist_segment_promoted_on_close() {
        let db = test_db();
        let conn = &db.conn;

        insert_conversation(conn, "conv-1").unwrap();
        let seg_id = insert_segment(conn, "conv-1", true, 1.0, "model-v1").unwrap();
        close_segment(conn, seg_id, 0.7).unwrap();

        let info = get_segment_info(conn, seg_id).unwrap();
        assert!(info.persist);
        assert!(info.promoted);
    }

    #[test]
    fn node_insert_and_get() {
        let db = test_db();
        let conn = &db.conn;

        insert_conversation(conn, "conv-1").unwrap();
        let seg_id = insert_segment(conn, "conv-1", false, 0.3, "model-v1").unwrap();

        let hash = compute_content_hash("hello world", None);
        let embedding = vec![0u8; 3072]; // 768 * 4 bytes
        let node_id = insert_node(conn, seg_id, "hello world", None, &embedding, &hash).unwrap();

        let content = get_node(conn, node_id).unwrap();
        assert_eq!(content.data, "hello world");
        assert!(content.metadata.is_none());
    }

    #[test]
    fn duplicate_detection() {
        let db = test_db();
        let conn = &db.conn;

        insert_conversation(conn, "conv-1").unwrap();
        let seg_id = insert_segment(conn, "conv-1", false, 0.3, "model-v1").unwrap();

        let hash = compute_content_hash("hello", Some("summary"));
        assert!(!check_duplicate(conn, seg_id, &hash).unwrap());

        let embedding = vec![0u8; 3072];
        insert_node(conn, seg_id, "hello", Some("summary"), &embedding, &hash).unwrap();
        assert!(check_duplicate(conn, seg_id, &hash).unwrap());

        // Different segment, same hash — not a duplicate
        let seg2 = insert_segment(conn, "conv-1", false, 0.3, "model-v1").unwrap();
        assert!(!check_duplicate(conn, seg2, &hash).unwrap());
    }

    #[test]
    fn cascade_delete() {
        let db = test_db();
        let conn = &db.conn;

        insert_conversation(conn, "conv-1").unwrap();
        let seg_id = insert_segment(conn, "conv-1", false, 0.3, "model-v1").unwrap();
        let hash = compute_content_hash("data", None);
        let embedding = vec![0u8; 3072];
        insert_node(conn, seg_id, "data", None, &embedding, &hash).unwrap();

        // Verify data exists
        let stats = get_project_stats(conn, "model-v1").unwrap();
        assert_eq!(stats.total_segments, 1);
        assert_eq!(stats.total_nodes, 1);

        // Delete conversation cascades
        delete_conversation(conn, "conv-1").unwrap();
        let stats = get_project_stats(conn, "model-v1").unwrap();
        assert_eq!(stats.total_segments, 0);
        assert_eq!(stats.total_nodes, 0);
    }

    #[test]
    fn eviction_candidates() {
        let db = test_db();
        let conn = &db.conn;

        insert_conversation(conn, "conv-1").unwrap();

        // Create 3 closed segments with different relevancy
        for (score, model) in [(0.1, "m"), (0.5, "m"), (0.9, "m")] {
            let seg = insert_segment(conn, "conv-1", false, score, model).unwrap();
            increment_segment_node_count(conn, seg).unwrap();
            close_segment(conn, seg, 0.7).unwrap();
        }

        // max_segments=2, should evict 1 (lowest relevancy = highest priority)
        let candidates = get_eviction_candidates(conn, "conv-1", 2).unwrap();
        assert_eq!(candidates.len(), 1);

        // The evicted segment should be the one with score 0.1 (highest eviction priority)
        let info = get_segment_info(conn, candidates[0]).unwrap();
        assert!((info.relevancy_score - 0.1).abs() < f32::EPSILON);
    }

    #[test]
    fn relevancy_update() {
        let db = test_db();
        let conn = &db.conn;

        insert_conversation(conn, "conv-1").unwrap();
        let seg = insert_segment(conn, "conv-1", false, 0.3, "m").unwrap();

        update_segment_hit(conn, seg, 0.1).unwrap();
        let info = get_segment_info(conn, seg).unwrap();
        assert!((info.relevancy_score - 0.4).abs() < f32::EPSILON);
        assert_eq!(info.hit_count, 1);
        assert!(info.last_hit_timestamp.is_some());

        // Cap at 1.0
        for _ in 0..10 {
            update_segment_hit(conn, seg, 0.1).unwrap();
        }
        let info = get_segment_info(conn, seg).unwrap();
        assert!((info.relevancy_score - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn content_hash_determinism() {
        let a = compute_content_hash("hello", Some("meta"));
        let b = compute_content_hash("hello", Some("meta"));
        assert_eq!(a, b);

        let c = compute_content_hash("hello", None);
        assert_ne!(a, c);
    }

    #[test]
    fn mismatched_segments() {
        let db = test_db();
        let conn = &db.conn;

        insert_conversation(conn, "conv-1").unwrap();
        insert_segment(conn, "conv-1", false, 0.3, "model-v1").unwrap();
        insert_segment(conn, "conv-1", false, 0.3, "model-v2").unwrap();

        let mismatched = get_mismatched_segments(conn, "model-v2").unwrap();
        assert_eq!(mismatched.len(), 1);

        let stats = get_project_stats(conn, "model-v2").unwrap();
        assert_eq!(stats.mismatched_segments, 1);
    }
}
