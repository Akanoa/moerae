use rusqlite::Connection;

pub const CURRENT_SCHEMA_VERSION: i64 = 1;

pub fn create_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS schema_version (
            id INTEGER PRIMARY KEY CHECK(id = 1),
            version INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS config (
            id INTEGER PRIMARY KEY CHECK(id = 1),
            data TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS conversations (
            uuid TEXT PRIMARY KEY,
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS segments (
            segment_id INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id TEXT NOT NULL REFERENCES conversations(uuid) ON DELETE CASCADE,
            state TEXT NOT NULL CHECK(state IN ('open', 'closed')),
            persist INTEGER NOT NULL DEFAULT 0,
            promoted INTEGER NOT NULL DEFAULT 0,
            relevancy_score REAL NOT NULL,
            hit_count INTEGER NOT NULL DEFAULT 0,
            node_count INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            last_hit_timestamp INTEGER,
            last_write_timestamp INTEGER NOT NULL,
            model_id TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS nodes (
            node_id INTEGER PRIMARY KEY AUTOINCREMENT,
            segment_id INTEGER NOT NULL REFERENCES segments(segment_id) ON DELETE CASCADE,
            data TEXT NOT NULL,
            metadata TEXT,
            embedding BLOB NOT NULL,
            content_hash BLOB NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_segments_conversation ON segments(conversation_id);
        CREATE INDEX IF NOT EXISTS idx_segments_promoted ON segments(promoted) WHERE promoted = 1;
        CREATE INDEX IF NOT EXISTS idx_segments_model ON segments(model_id);
        CREATE INDEX IF NOT EXISTS idx_nodes_segment ON nodes(segment_id);
        CREATE INDEX IF NOT EXISTS idx_nodes_content_hash ON nodes(content_hash, segment_id);
        ",
    )?;

    // Set initial schema version if not exists
    conn.execute(
        "INSERT OR IGNORE INTO schema_version (id, version) VALUES (1, ?1)",
        [CURRENT_SCHEMA_VERSION],
    )?;

    Ok(())
}

pub fn get_schema_version(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT version FROM schema_version WHERE id = 1",
        [],
        |row| row.get(0),
    )
}

pub fn run_migrations(conn: &Connection) -> rusqlite::Result<()> {
    let version = get_schema_version(conn)?;
    if version < CURRENT_SCHEMA_VERSION {
        // Future migrations go here:
        // if version < 2 { migrate_v1_to_v2(conn)?; }
        conn.execute(
            "UPDATE schema_version SET version = ?1 WHERE id = 1",
            [CURRENT_SCHEMA_VERSION],
        )?;
    }
    Ok(())
}
